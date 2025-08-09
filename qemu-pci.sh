#!/bin/sh

### Constants
# this is the Linux kernel module that's used for the card
LINUX_MODULE="foo_core"
# this is the symlink in /sys/bus/pci/drivers/LINUX_MODULE/
BUS_ID="0123:45:67.8"
# this can be found with "lspci -nn | grep VENDOR"
DEVICE_ID="1234 5678"
# qemu might fail because an IRQ is allocated to a different device (dmesg, /proc/interrupts)
# we could probably use INTx instead, but for now just disable them
DEVICES_TO_REMOVE="pci0000:00/1234:56:78.9"
# how much RAM the VM gets (in MB)
MEMORY=512
### End of constants

sudo modprobe vfio_pci

# unbind the card
echo $BUS_ID | sudo tee /sys/bus/pci/drivers/$LINUX_MODULE/unbind
# bind the card to VFIO
echo $DEVICE_ID | sudo tee /sys/bus/pci/drivers/vfio-pci/new_id

for device in $DEVICES_TO_REMOVE; do
    echo 1 | sudo tee /sys/devices/$device/remove
done

# chown the device, so that qemu doesn't have to run as root
# TODO: actually look at what device we need
sudo chown $USER /dev/vfio/?
# allow the VM to pin its memory
# we need a bit more than $MEMORY, but in bytes
LIMIT=$(($(($MEMORY + 128)) * 1024 * 1024))
sudo prlimit --memlock=$LIMIT --pid=$$
# run qemu
prlimit --memlock=$LIMIT qemu-system-x86_64 $@ \
    -m $MEMORY -cpu Broadwell \
    -device vfio-pci,host=$BUS_ID

# re-scan the bus to get the removed devices back
echo 1 | sudo tee /sys/bus/pci/rescan

# re-bind the card to Linux
echo $DEVICE_ID | sudo tee /sys/bus/pci/drivers/vfio-pci/remove_id
echo $BUS_ID | sudo tee /sys/bus/pci/drivers/vfio-pci/unbind
echo $BUS_ID | sudo tee /sys/bus/pci/drivers/$LINUX_MODULE/bind
