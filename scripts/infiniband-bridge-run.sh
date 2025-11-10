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

# Host configurations
ib3NIC_MAC="52:54:00:aa:bb:01"
ib4NIC_MAC="52:54:00:aa:bb:02"
ib3NIC_IP="192.168.0.27"
ib4NIC_IP="192.168.0.12"
ib3NIC_VM_IP="192.168.100.2"
ib4NIC_VM_IP="192.168.100.3"

HOST=$(hostname)
eval "NIC_MAC_ADDRESS=\${${HOST}NIC_MAC}"
eval "NIC_IP_ADDRESS=\${${HOST}NIC_IP}"
eval "NIC_VM_IP_ADDRESS=\${${HOST}NIC_VM_IP}"

if [ "${NIC_MAC_ADDRESS}" = "${ib3NIC_MAC}" ]; then
    TARGET_MAC_ADDRESS="${ib4NIC_MAC}"
    TARGET_IP_ADDRESS="${ib4NIC_IP}"
    TARGET_VM_IP_ADDRESS="${ib4NIC_VM_IP}"
    BRIDGE_IP="192.168.100.1"
else
    TARGET_MAC_ADDRESS="${ib3NIC_MAC}"
    TARGET_IP_ADDRESS="${ib3NIC_IP}"
    TARGET_VM_IP_ADDRESS="${ib3NIC_VM_IP}"
    BRIDGE_IP="192.168.100.4"
fi

printf "=== Configuration ===\n"
printf "Host: %s\n" "$HOST"
printf "Using MAC-ADDR: %s\n" "${NIC_MAC_ADDRESS}"
printf "Targeting MAC-ADDR: %s\n" "${TARGET_MAC_ADDRESS}"
printf "Using IP-ADDR: %s\n" "${NIC_IP_ADDRESS}"
printf "Targeting IP-ADDR: %s\n" "${TARGET_IP_ADDRESS}"
printf "VM will use IP: %s\n" "${NIC_VM_IP_ADDRESS}"
printf "Remote VM IP: %s\n" "${TARGET_VM_IP_ADDRESS}"
printf "Bride will use IP: %s\n" "${BRIDGE_IP}"

sudo systemctl stop dhcpcd.service  # prevent assigning ip
sudo sysctl -w net.ipv4.ip_forward=1 

sudo ip link add name br0 type bridge
sudo ip link set dev br0 up
sudo ip addr add "${BRIDGE_IP}" brd + dev br0

sudo ip tuntap add user $(whoami) dev tap0 mode tap
sudo ip link set dev tap0 promisc on up
sudo ip link set dev tap0 master br0

sudo iptables -t nat -A POSTROUTING -s "${VM_SUBNET}" -o eno1 -j MASQUERADE
sudo iptables -t nat -A PREROUTING -i eno1 -p tcp --dport 1324 -j DNAT --to-destination "${NIC_VM_IP_ADDRESS}:1324"
sudo iptables -A FORWARD -p tcp -d "${NIC_VM_IP_ADDRESS}" --dport 1324 -j ACCEPT
sudo iptables -I FORWARD -m physdev --physdev-is-bridged -j ACCEPT

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
  -object filter-dump,id=filter1,netdev=net0,file=rtl8139.dump \
  -device rtl8139,netdev=net0,id=nic0,mac=${NIC_MAC_ADDRESS} \
  -S -gdb tcp::1234

printf "shutdown system !"

sudo ip link set tap0 promisc off down
sudo ip link set dev tap0 nomaster
sudo ip link set dev tap0 down
sudo ip tuntap del dev tap0 mode tap

sudo ip link set eno1 promisc off down
sudo ip addr add "$IP" dev eno1
sudo ip link set dev eno1 nomaster

sudo ip addr del dev br0 "$IP" 
sudo ip link set dev br0 down
sudo ip link del br0

sudo ip link set dev eno1 up
sudo ip route del default via "$ROUTER" dev br0 2>/dev/null
sudo ip route add default via "$ROUTER" dev eno1 onlink

sudo systemctl start dhcpcd.service

echo 1 | sudo tee /sys/bus/pci/rescan

# re-bind the card to mlx4_core
echo "${MLX4_DRIVER}" | sudo tee /sys/bus/pci/devices/${BUS_ID}/driver_override
#echo "${DEVICE_ID}" | sudo tee /sys/bus/pci/drivers/${VFIO_DRIVER}/remove_id
echo "${BUS_ID}" | sudo tee /sys/bus/pci/drivers/${VFIO_DRIVER}/unbind
echo "${BUS_ID}" | sudo tee /sys/bus/pci/drivers/${MLX4_DRIVER}/bind