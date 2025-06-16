#!/usr/bin/env bash

# minimal system for testing workings of mlx4

### Constants
# this is the symlink in /sys/bus/pci/drivers/mlx?_core/
BUS_ID="0000:05:00.0"
# this can be found with "lspci -nn | grep Mellanox"
DEVICE_ID="15b3 1003"

DEVICES_TO_REMOVE="pci0000:00/0000:00:1a.0"

MEMORY=$((2 * 1024))

# unbind the MLX card
# the ID is the symlink in /sys/bus/pci/drivers/mlx?_core/
echo "$BUS_ID" | sudo tee /sys/bus/pci/drivers/mlx4_core/unbind
# bind the MLX card to VFIO
# the ID can be found with "lspci -nn | grep Mellanox"
echo "$DEVICE_ID" | sudo tee /sys/bus/pci/drivers/vfio-pci/new_id

for device in "$DEVICES_TO_REMOVE"; do
    echo 1 | sudo tee /sys/devices/$device/remove
done

GROUP=$(basename $(readlink /sys/bus/pci/devices/$BUS_ID/iommu_group))

sudo chown $USER /dev/vfio/$GROUP

# allow the VM to pin its memory
# we need a bit more than $MEMORY, but in bytes

LIMIT=$(($(($MEMORY + 128)) * 1024 * 1024))
sudo prlimit --memlock="$LIMIT" --pid=$$

printf "starting system !"

prlimit --memlock="$LIMIT" qemu-system-x86_64 -machine q35 \
  -m ${MEMORY} \
  -cpu qemu64 \
  -bios RELEASEX64_OVMF.fd \
  -boot d \
  -vga std \
  -rtc base=localtime \
  -serial stdio \
  -device piix3-ide,id=ide \
  -device ahci,id=ahci \
  -drive driver=raw,if=none,id=boot,file.filename=d3os.img \
  -drive driver=raw,if=none,id=hdd,file.filename=hdd.img \
  -device ide-hd,bus=ahci.0,drive=boot \
  -device ide-hd,bus=ide.0,drive=hdd \
  -device vfio-pci,host=${BUS_ID} \
  -S -gdb tcp::1234

printf "shutdown system !"

echo 1 | sudo tee /sys/bus/pci/rescan

# re-bind the card to mlx4_core
echo "$DEVICE_ID" | sudo tee /sys/bus/pci/drivers/vfio-pci/remove_id
echo "$BUS_ID" | sudo tee /sys/bus/pci/drivers/vfio-pci/unbind
echo "$BUS_ID" | sudo tee /sys/bus/pci/drivers/mlx4_core/bind