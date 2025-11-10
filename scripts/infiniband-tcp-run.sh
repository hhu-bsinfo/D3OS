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
SOCKET_P=1777

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

ARGS=(
  -machine q35
  -m "${MEMORY}"
  -cpu Broadwell
  -bios RELEASEX64_OVMF.fd
  -boot d
  -vga std
  -rtc base=localtime
  -serial stdio
  -device vfio-pci,host="${BUS_ID}"
  -device piix3-ide,id=ide
  -device ahci,id=ahci
  -drive driver=raw,if=none,id=boot,file.filename=d3os-"${HOST}".img
  -drive driver=raw,if=none,id=hdd,file.filename=hdd.img
  -device ide-hd,bus=ahci.0,drive=boot
  -device ide-hd,bus=ide.0,drive=hdd
  -S
  -gdb tcp::1234
)

mode=""

while [ "$1" != "" ]; do
    case "$1" in
        -c|--client)
            mode="client"
            ;;
        -s|--server)
            mode="server"
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
    shift
done

if [ "$mode" = "client" ]; then
    echo "Running in client mode"
    ARGS+=(
        -netdev socket,id=net0,connect="${TARGET_IP_ADDRESS}:${SOCKET_P}"
        -device rtl8139,netdev=net0,id=nic0,mac="${NIC_MAC_ADDRESS}"
    )

elif [ "$mode" = "server" ]; then
    echo "Running in server mode"
    ARGS+=(
        -netdev socket,id=net0,listen=:"${SOCKET_P}"
        -device rtl8139,netdev=net0,id=nic0,mac="${NIC_MAC_ADDRESS}"
    )
fi

# allow the VM to pin its memory
# we need a bit more than $MEMORY, but in bytes

LIMIT=$(($(($MEMORY + 128)) * 1024 * 1024))
sudo prlimit --memlock="$LIMIT" --pid=$$

IOMMU_ADDRESS_WIDTH=$(( ((0x$(cat /sys/devices/virtual/iommu/dmar0/intel-iommu/cap) & 0x3F0000) >> 16) + 1 ))

printf "starting system !"

prlimit --memlock="$LIMIT" qemu-system-x86_64 "${ARGS[@]}"

printf "shutdown system !"

echo 1 | sudo tee /sys/bus/pci/rescan

# re-bind the card to mlx4_core
echo "${MLX4_DRIVER}" | sudo tee /sys/bus/pci/devices/${BUS_ID}/driver_override
#echo "${DEVICE_ID}" | sudo tee /sys/bus/pci/drivers/${VFIO_DRIVER}/remove_id
echo "${BUS_ID}" | sudo tee /sys/bus/pci/drivers/${VFIO_DRIVER}/unbind
echo "${BUS_ID}" | sudo tee /sys/bus/pci/drivers/${MLX4_DRIVER}/bind