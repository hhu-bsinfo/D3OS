#!/usr/bin/env bash

# minimal system for testing workings of mlx4
VFIO_DRIVER="vfio-pci"
MLX4_DRIVER="mlx4_core"
### Constants
# this is the symlink in /sys/bus/pci/drivers/mlx?_core/
BUS_ID="0000:05:00.0"
# this can be found with "lspci -nn | grep Mellanox"
DEVICE_ID="15b3 1003"

DEVICES_TO_REMOVE="pci0000:00/0000:00:1a.0"

MEMORY=$((2 * 1024))

# unbind the MLX card
# the ID is the symlink in /sys/bus/pci/drivers/mlx?_core/
echo "$BUS_ID" | sudo tee /sys/bus/pci/drivers/${MLX4_DRIVER}/unbind
echo "$VFIO_DRIVER" | sudo tee /sys/bus/pci/devices/${BUS_ID}/driver_override
#echo "$DEVICE_ID" | sudo tee /sys/bus/pci/drivers/${VFIO_DRIVER}/new_id
# bind the MLX card to VFIO
echo "$BUS_ID" | sudo tee /sys/bus/pci/drivers/${VFIO_DRIVER}/bind

# the ID can be found with "lspci -nn | grep Mellanox"
# echo "$DEVICE_ID" | sudo tee /sys/bus/pci/drivers/vfio-pci/new_id

for device in "$DEVICES_TO_REMOVE"; do
    echo 1 | sudo tee /sys/devices/$device/remove
done

GROUP=$(basename $(readlink /sys/bus/pci/devices/${BUS_ID}/iommu_group))

sudo chown ${USER} /dev/vfio/${GROUP}

# VXLAN Configuration
UDP_PORT="4789"
NET_IF="eno1"
VNI="123"
GW_IP=$(ip route | awk '/default/ {print $3}')

# Host configurations
ib3NIC_MAC="52:54:00:aa:bb:01"
ib4NIC_MAC="52:54:00:aa:bb:02"
ib3NIC_IP="192.168.0.27"
ib4NIC_IP="192.168.0.12"
ib3NIC_VM_IP="192.168.100.2"
ib4NIC_VM_IP="192.168.100.3"

# Determine this host's configuration
HOST=$(hostname)
eval "NIC_MAC_ADDRESS=\${${HOST}NIC_MAC}"
eval "NIC_IP_ADDRESS=\${${HOST}NIC_IP}"
eval "NIC_VM_IP_ADDRESS=\${${HOST}NIC_VM_IP}"

if [ "${NIC_MAC_ADDRESS}" = "${ib3NIC_MAC}" ]; then
    TARGET_MAC_ADDRESS="${ib4NIC_MAC}"
    TARGET_IP_ADDRESS="${ib4NIC_IP}"
    TARGET_VM_IP_ADDRESS="${ib4NIC_VM_IP}"
else
    TARGET_MAC_ADDRESS="${ib3NIC_MAC}"
    TARGET_IP_ADDRESS="${ib3NIC_IP}"
    TARGET_VM_IP_ADDRESS="${ib3NIC_VM_IP}"
fi

printf "=== Configuration ===\n"
printf "Host: %s\n" "$HOST"
printf "Using MAC-ADDR: %s\n" "${NIC_MAC_ADDRESS}"
printf "Targeting MAC-ADDR: %s\n" "${TARGET_MAC_ADDRESS}"
printf "Using IP-ADDR: %s\n" "${NIC_IP_ADDRESS}"
printf "Targeting IP-ADDR: %s\n" "${TARGET_IP_ADDRESS}"
printf "VM will use IP: %s\n" "${NIC_VM_IP_ADDRESS}"
printf "Remote VM IP: %s\n" "${TARGET_VM_IP_ADDRESS}"
printf "Bridge IP: %s\n" "${BRIDGE_IP}"

printf "\n=== Checking Prerequisites ===\n"

if ping -c 1 -W 2 "${TARGET_IP_ADDRESS}" >/dev/null 2>&1; then
    printf "Target host %s is reachable\n" "${TARGET_IP_ADDRESS}"
else
    printf "Target host %s is NOT reachable\n" "${TARGET_IP_ADDRESS}"
    printf "Fix connectivity before proceeding\n"
    exit 1
fi

printf "Testing VXLAN port accessibility...\n"

printf "\n=== Cleaning up existing interfaces ===\n"
sudo ip link del vxlan0 2>/dev/null || true
sudo ip link del br0 2>/dev/null || true
sudo ip link del tap0 2>/dev/null || true

printf "\n=== Creating VXLAN tunnel ===\n"
sudo ip link add vxlan0 type vxlan \
    id "${VNI}" \
    local "${NIC_IP_ADDRESS}" \
    remote "${TARGET_IP_ADDRESS}" \
    dstport "${UDP_PORT}" \
    dev "${NET_IF}"

if [ $? -eq 0 ]; then
    printf "VXLAN interface created\n"
else
    printf "Failed to create VXLAN interface\n"
    exit 1
fi

sudo ip link set vxlan0 up

printf "\n=== Creating tap interface ===\n"
sudo ip tuntap add dev tap0 mode tap user $(whoami)
sudo ip link set tap0 promisc on up

printf "\n=== Creating bridge ===\n"
sudo ip link add name br-vxlan type bridge
sudo ip link set br-vxlan up
sudo ip link set tap0 master br-vxlan
sudo ip link set vxlan0 master br-vxlan

printf "\n=== Enabling IP forwarding ===\n"
sudo sysctl -w net.ipv4.ip_forward=1

printf "\n=== Firewall Configuration ===\n"

sudo iptables -A INPUT -p udp --dport "$UDP_PORT" -j ACCEPT

printf "\n=== Setup Complete ===\n"
printf "Bridge configuration:\n"
sudo bridge link show

printf "\nVXLAN configuration:\n"
ip -d link show vxlan0

printf "\nBridge FDB table:\n"
sudo bridge fdb show dev vxlan0

printf "\n=== Testing Connectivity ===\n"
printf "Testing VXLAN tunnel...\n"

ping -c 1 -W 2 "${TARGET_VM_IP_ADDRESS}" && \
    printf "Can reach remote VM network\n" || \
    printf "Cannot reach remote VM network (expected until VM starts)\n"

# allow the VM to pin its memory
# we need a bit more than $MEMORY, but in bytes
LIMIT=$(($(($MEMORY + 128)) * 1024 * 1024))
sudo prlimit --memlock="$LIMIT" --pid=$$

IOMMU_ADDRESS_WIDTH=$(( ((0x$(cat /sys/devices/virtual/iommu/dmar0/intel-iommu/cap) & 0x3F0000) >> 16) + 1 ))

printf "starting system !"

prlimit --memlock="$LIMIT" qemu-system-x86_64 \
  -machine q35 \
  -m ${MEMORY} \
  -cpu Broadwell \
  -bios RELEASEX64_OVMF.fd \
  -boot d \
  -vga std \
  -rtc base=localtime \
  -serial stdio \
  -device vfio-pci,host=${BUS_ID} \
  -device piix3-ide,id=ide \
  -device ahci,id=ahci \
  -drive driver=raw,if=none,id=boot,file.filename=d3os-${HOST}.img \
  -drive driver=raw,if=none,id=hdd,file.filename=hdd.img \
  -device ide-hd,bus=ahci.0,drive=boot \
  -device ide-hd,bus=ide.0,drive=hdd \
  -netdev tap,id=net0,ifname=tap0,script=no,downscript=no \
  -device rtl8139,netdev=net0,id=nic0,mac=${NIC_MAC_ADDRESS} \
  -S -gdb tcp::1234

printf "shutdown system !"

printf "Removing tap interface...\n"
if ip link show tap0 >/dev/null 2>&1; then
    sudo ip link set tap0 down 2>/dev/null || true
    sudo ip link set dev tap0 nomaster 2>/dev/null || true
    sudo ip tuntap del dev tap0 mode tap 2>/dev/null || true
    printf "tap0 removed\n"
else
    printf "tap0 not found (already removed)\n"
fi

printf "Removing VXLAN from bridge...\n"
if ip link show vxlan0 >/dev/null 2>&1; then
    sudo ip link set dev vxlan0 nomaster 2>/dev/null || true
    printf "vxlan0 removed from bridge\n"
fi

printf "Removing bridge...\n"
if ip link show br0 >/dev/null 2>&1; then
    sudo ip link set br0 down 2>/dev/null || true
    sudo ip link del br0 2>/dev/null || true
    printf "br0 removed\n"
else
    printf "br0 not found (already removed)\n"
fi

printf "Removing VXLAN tunnel...\n"
if ip link show vxlan0 >/dev/null 2>&1; then
    sudo ip link set vxlan0 down 2>/dev/null || true
    sudo ip link del vxlan0 2>/dev/null || true
    printf "vxlan0 removed\n"
else
    printf "vxlan0 not found (already removed)\n"
fi

printf "\n=== Cleanup Verification ===\n"
 
printf "Checking interface cleanup:\n"
for iface in tap0 vxlan0 br0; do
    if ip link show "$iface" >/dev/null 2>&1; then
        printf "Warning: %s still exists\n" "$iface"
    else
        printf "%s removed successfully\n" "$iface"
    fi
done

printf "\nCurrent network interfaces:\n"
ip link show | grep -E "^[0-9]+:" | grep -v "lo:"

printf "\n=== Cleanup Complete ===\n"

echo 1 | sudo tee /sys/bus/pci/rescan

# re-bind the card to mlx4_core
echo "${MLX4_DRIVER}" | sudo tee /sys/bus/pci/devices/${BUS_ID}/driver_override
#echo "${DEVICE_ID}" | sudo tee /sys/bus/pci/drivers/${VFIO_DRIVER}/remove_id
echo "${BUS_ID}" | sudo tee /sys/bus/pci/drivers/${VFIO_DRIVER}/unbind
echo "${BUS_ID}" | sudo tee /sys/bus/pci/drivers/${MLX4_DRIVER}/bind