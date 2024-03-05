#!/bin/bash

readonly TOWBOOT_VERSION="0.7.1"
readonly FILE_LIST=("towboot-x64.efi" "towboot.toml" "../kernel.elf" "../initrd.tar")
readonly IMAGE="../../d3os.img"

if [[ ! -f "towboot-x64.efi" ]]; then
  wget -O towboot-x64.efi "https://github.com/hhuOS/towboot/releases/download/v${TOWBOOT_VERSION}/towboot-v${TOWBOOT_VERSION}-x86_64.efi" || exit 1
fi

SIZE=0;
for file in "${FILE_LIST[@]}"; do
  SIZE=$(($SIZE + $(wc -c ${file} | cut -d ' ' -f 1)))
done

readonly SECTORS=$(((${SIZE} / 512) + 2048))

mformat -i part.img -C -T ${SECTORS} || exit 1
mmd -i part.img efi || exit 1
mmd -i part.img efi/boot || exit 1
mcopy -i part.img towboot-x64.efi ::efi/boot/bootx64.efi || exit 1
mcopy -i part.img towboot.toml :: || exit 1
mcopy -i part.img ../kernel.elf :: || exit 1
mcopy -i part.img ../initrd.tar :: || exit 1

fallocate -l 1M fill.img || exit 1
cat fill.img part.img fill.img > "${IMAGE}" || exit 1
echo -e "g\\nn\\n1\\n2048\\n+${SECTORS}\\nt\\n1\\nw\\n" | fdisk "${IMAGE}" || exit 1

rm -f fill.img part.img || exit 1
