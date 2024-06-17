#!/usr/bin/sh

qemu-system-x86_64 -machine q35,pcspk-audiodev=audio0 -m 128M -cpu qemu64 -bios RELEASEX64_OVMF.fd -boot d -vga std -rtc base=localtime -drive driver=raw,node-name=boot,file.driver=file,file.filename=d3os.img -audiodev id=audio0,driver=pa
