#!/bin/bash

readonly LIMINE_VERSION="5.20230928.2"

if [[ ! -f "iso/limine-uefi-cd.bin" || ! -f "iso/limine-bios-cd.bin" || ! -f "iso/limine-bios.sys" || ! -f "iso/EFI/BOOT/BOOTX64.EFI" || ! -f "limine" ]]; then
  wget -O limine.zip "https://github.com/limine-bootloader/limine/archive/refs/tags/v${LIMINE_VERSION}-binary.zip" || exit 1
  unzip limine.zip || exit 1
  cd "limine-${LIMINE_VERSION}-binary" || exit 1

  make || exit 1
  cp "limine" ".." || exit 1

  cp "limine-uefi-cd.bin" "../iso" || exit 1
  cp "limine-bios-cd.bin" "../iso" || exit 1
  cp "limine-bios.sys" "../iso" || exit 1

  mkdir -p "../iso/EFI/BOOT" || exit 1
  cp "BOOTX64.EFI" "../iso/EFI/BOOT" || exit 1

  cd .. || exit 1
  rm -r limine.zip limine-${LIMINE_VERSION}-binary || exit 1
fi

cd iso || exit 1
xorriso -as mkisofs -b limine-bios-cd.bin -no-emul-boot -boot-load-size 4 -boot-info-table --efi-boot limine-uefi-cd.bin -efi-boot-part --efi-boot-image --protective-msdos-label . -o ../hhuTOSr-limine.iso || exit 1
cd .. || exit 1
./limine bios-install hhuTOSr-limine.iso || exit 1
