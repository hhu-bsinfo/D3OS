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

# allow the VM to pin its memory
# we need a bit more than $MEMORY, but in bytes

# Host configurations
ib3NIC_MAC="52:54:00:aa:bb:01"
ib4NIC_MAC="52:54:00:aa:bb:02"
HOST_PORT=1797 # must match the configured port in the build script !
GUEST_PORT=1324
GUEST_SUBNET="192.168.100.0/24"
DHCP_START="192.168.100.10"

HOST=$(hostname)
eval "NIC_MAC_ADDRESS=\${${HOST}NIC_MAC}"

HUGEPAGE_SIZE_MB=2

echo "Hugepages information:"
cat /sys/kernel/mm/hugepages/hugepages-2048kB/{nr_hugepages,free_hugepages,resv_hugepages}

AVAILABLE=$(< /sys/kernel/mm/hugepages/hugepages-2048kB/free_hugepages)
#TOTAL=$(< /sys/kernel/mm/hugepages/hugepages-2048kB/nr_hugepages)

printf "%dMB available using hugepages\n" $(( AVAILABLE * HUGEPAGE_SIZE_MB ))

REQUIRED=$(( MEMORY / HUGEPAGE_SIZE_MB ))

if (( AVAILABLE < REQUIRED )); then
    TARGET=$REQUIRED
    echo "Not enough hugepages. Setting nr_hugepages to $TARGET..."
    echo $TARGET | sudo tee /proc/sys/vm/nr_hugepages
fi

sudo chown ${USER} /dev/hugepages

LIMIT=$(($(($MEMORY + 128)) * 1024 * 1024))
sudo prlimit --memlock="$LIMIT" --pid=$$
IOMMU_ADDRESS_WIDTH=$(( ((0x$(cat /sys/devices/virtual/iommu/dmar0/intel-iommu/cap) & 0x3F0000) >> 16) + 1 ))

touch /dev/hugepages/qemu.mem
sudo chmod u=rw /dev/hugepages/qemu.mem 

printf "starting system !"

prlimit --memlock="$LIMIT" qemu-system-x86_64 \
  -machine q35 \
  -m ${MEMORY} \
  -mem-path /dev/hugepages/qemu.mem \
  -cpu Broadwell \
  -bios RELEASEX64_OVMF.fd \
  -boot d \
  -vga std \
  -rtc base=localtime \
  -serial stdio \
  -device vfio-pci,host="${BUS_ID}" \
  -device piix3-ide,id=ide \
  -device ahci,id=ahci \
  -drive driver=raw,if=none,id=boot,file.filename=d3os-$(hostname).img \
  -drive driver=raw,if=none,id=hdd,file.filename=hdd.img \
  -device ide-hd,bus=ahci.0,drive=boot \
  -device ide-hd,bus=ide.0,drive=hdd \
  -netdev user,id=net0,net="${GUEST_SUBNET}",dhcpstart="${DHCP_START}",hostfwd=udp:0.0.0.0:"${HOST_PORT}"-:"${GUEST_PORT}" \
  -device rtl8139,netdev=net0,id=nic0,mac="${NIC_MAC_ADDRESS}" \
  -S -gdb tcp::1234

printf "shutdown system !"

echo 1 | sudo tee /sys/bus/pci/rescan

# re-bind the card to mlx4_core
echo "${MLX4_DRIVER}" | sudo tee /sys/bus/pci/devices/${BUS_ID}/driver_override
#echo "${DEVICE_ID}" | sudo tee /sys/bus/pci/drivers/${VFIO_DRIVER}/remove_id
echo "${BUS_ID}" | sudo tee /sys/bus/pci/drivers/${VFIO_DRIVER}/unbind
echo "${BUS_ID}" | sudo tee /sys/bus/pci/drivers/${MLX4_DRIVER}/bind