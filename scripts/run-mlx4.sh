#!/bin/sh

### Constants
# this is the Linux kernel module that's used for the card
LINUX_MODULE="mlx4_core"
# this is the symlink in /sys/bus/pci/drivers/LINUX_MODULE/
SLOT_ID="0000:01:00.0"
# this can be found with "lspci -nn | grep VENDOR"
DEVICE_ID="15b3 1003"
# qemu fails because IRQ 16 is allocated to the EHCI controller (dmesg, /proc/interrupts)
# it should support INTx, but no idea on how to use it
# just disable the controller
DEVICES_TO_REMOVE="pci0000:00/0000:00:1a.0"
# how much RAM the VM gets (in MB)
MEMORY=512
### End of constants

# For more information see https://www.theseus-os.com/Theseus/book/running/virtual_machine/pci_passthrough.html

# unbind the card
echo $SLOT_ID | sudo tee /sys/bus/pci/drivers/$LINUX_MODULE/unbind
# bind the card to VFIO
sudo modprobe vfio_pci
echo $DEVICE_ID | sudo tee /sys/bus/pci/drivers/vfio-pci/new_id

for device in $DEVICES_TO_REMOVE; do
    echo 1 | sudo tee /sys/devices/$device/remove
done

# chown the device, so that qemu doesn't have to run as root
GROUP_ID="$(basename $(readlink /sys/bus/pci/devices/$SLOT_ID/iommu_group))"
sudo chown $USER /dev/vfio/$GROUP_ID

# allow the VM to pin its memory
# we need a bit more than $MEMORY, but in bytes
LIMIT=$(($(($MEMORY + 128)) * 1024 * 1024))
sudo prlimit --memlock=$LIMIT --pid=$$

# run qemu
prlimit --memlock=$LIMIT qemu-system-x86_64 \
    -m $MEMORY  \
    -machine q35 \
    -cpu Broadwell \
    -bios RELEASEX64_OVMF.fd \
    -serial stdio \
    -boot d \
    -rtc base=localtime \
    -nic model=rtl8139,id=rtl8139,hostfwd=udp::1797-:1797,hostfwd=tcp::1797-:1797,hostfwd=tcp::18515-:18515,hostfwd=udp::18515-:18515\
    -object filter-dump,id=filter1,netdev=rtl8139,file=rtl8139.dump \
    -device vfio-pci,host=${SLOT_ID} \
    $@

# re-scan the bus to get the removed devices back
echo 1 | sudo tee /sys/bus/pci/devices/$SLOT_ID/remove
echo 1 | sudo tee /sys/bus/pci/rescan

# re-bind the card to Linux
echo $DEVICE_ID | sudo tee /sys/bus/pci/drivers/vfio-pci/remove_id
echo $SLOT_ID | sudo tee /sys/bus/pci/drivers/vfio-pci/unbind
echo $SLOT_ID | sudo tee /sys/bus/pci/drivers/$LINUX_MODULE/bind

