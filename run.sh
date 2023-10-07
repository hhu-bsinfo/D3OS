#!/bin/bash

# Copyright (C) 2018-2023 Heinrich-Heine-Universitaet Duesseldorf,
# Institute of Computer Science, Department Operating Systems
# Burak Akguel, Christian Gesse, Fabian Ruhland, Filip Krakowski, Michael Schoettner
#
#
# This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public
# License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any
# later version.
#
# This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied
# warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more
# details.
#
# You should have received a copy of the GNU General Public License
# along with this program.  If not, see <http://www.gnu.org/licenses/>

readonly CONST_QEMU_BIN="qemu-system-x86_64"
readonly CONST_QEMU_MACHINE_PC="pc"
readonly CONST_QEMU_CPU="qemu64"
readonly CONST_QEMU_MACHINE_PC_KVM="pc,accel=kvm,kernel-irqchip=split"
readonly CONST_QEMU_DEFAULT_RAM="256M"
readonly CONST_QEMU_BIOS_EFI="bios/ovmf/x64/OVMF.fd"
readonly CONST_QEMU_ARGS="-boot d -vga std -rtc base=localtime -device isa-debug-exit"
readonly CONST_QEMU_OLD_AUDIO_ARGS="-soundhw pcspk"
readonly CONST_QEMU_NEW_AUDIO_ARGS="-audiodev id=pa,driver=pa -machine pcspk-audiodev=pa"

QEMU_BIOS="${CONST_QEMU_BIOS_EFI}"
QEMU_MACHINE="${CONST_QEMU_MACHINE_PC}"
QEMU_RAM="${CONST_QEMU_DEFAULT_RAM}"
QEMU_CPU="${CONST_QEMU_CPU}"
QEMU_CPU_OVERWRITE="false"
QEMU_AUDIO_ARGS="${CONST_QEMU_NEW_AUDIO_ARGS}"
QEMU_ARGS="${CONST_QEMU_ARGS}"

QEMU_GDB_PORT=""

if [ -f "hhuTOSr-towboot.img" ]; then
  QEMU_BOOT_DEVICE="-drive driver=raw,node-name=boot,file.driver=file,file.filename=hhuTOSr-towboot.img"
elif [ -f "hhuTOSr-limine.iso" ]; then
  QEMU_BOOT_DEVICE="-boot d -cdrom hhuTOSr-limine.iso"
elif [ -f "hhuTOSr-grub.iso" ]; then
  QEMU_BOOT_DEVICE="-boot d -cdrom hhuTOSr-grub.iso"
fi

version_lt() {
  test "$(printf "%s\n" "$@" | sort -V | tr ' ' '\n' | head -n 1)" != "${2}"
}

set_audio_parameters() {
  qemu_version=$(${CONST_QEMU_BIN} --version | head -n 1 | cut -c 23-)

  if version_lt "$qemu_version" "5.0.0"; then
    QEMU_AUDIO_ARGS="${CONST_QEMU_OLD_AUDIO_ARGS}"
  fi
}

get_ovmf() {
  cd "bios/ovmf" || exit 1
  ./build.sh || exit 1
  cd "../.." || exit 1
}

check_file() {
  local file=$1

  if [ ! -f "$file" ]; then
    printf "File '%s' does not exist!\\n" "${file}"
    exit 1
  fi
}

parse_file() {
  local path=$1
  
  if [[ $path == *.iso ]]; then
    QEMU_BOOT_DEVICE="-boot d -cdrom ${path}"
  elif [[ $path == *.img ]]; then
    QEMU_BOOT_DEVICE="-drive driver=raw,node-name=boot,file.driver=file,file.filename=${path}"
  else
    printf "Invalid file '%s'!\\n" "${path}"
    exit 1
  fi
  
  check_file $path
}

parse_machine() {
  local machine=$1

  if [ "${machine}" == "pc" ]; then
    QEMU_MACHINE="${CONST_QEMU_MACHINE_PC}"
  elif [ "${machine}" == "pc-kvm" ]; then
    QEMU_MACHINE="${CONST_QEMU_MACHINE_PC_KVM}"
  else
    printf "Invalid machine '%s'!\\n" "${machine}"
    exit 1
  fi
}

parse_ram() {
  local memory=$1

  QEMU_RAM="${memory}"
}

parse_cpu() {
  local cpu=$1

  QEMU_CPU="${cpu}"
  QEMU_CPU_OVERWRITE="true"
}

parse_debug() {
  local port=$1

  echo "set architecture i386
      set disassembly-flavor intel
      target remote 127.0.0.1:${port}" >/tmp/gdbcommands."$(id -u)"

  QEMU_GDB_PORT="${port}"
}

start_gdb() {
  gdb -x "/tmp/gdbcommands.$(id -u)" "loader/boot/hhuOS.bin"
  exit $?
}

print_usage() {
  printf "Usage: ./run.sh [OPTION...]
    Available options:
    -f, --file
        Set the .iso or .img file, which qemu should boot (Default: hhuOS.img)
    -m, --machine
        Set the machine profile, which qemu should emulate ([pc] | [pc-kvm]) (Defualt: pc)
    -r, --ram
        Set the amount of ram, which qemu should use (e.g. 256, 1G, ...) (Default: 128M)
    -c, --cpu
        Set the CPU model, which qemu should emulate (e.g. 486, pentium, pentium2, ...) (Default: base)
    -d, --debug
        Set the port, on which qemu should listen for GDB clients (default: disabled)
    -h, --help
        Show this help message\\n"
}

parse_args() {
  while [ "${1}" != "" ]; do
    local arg=$1
    local val=$2

    case $arg in
    -f | --file)
      parse_file "$val"
      ;;
    -m | --machine)
      parse_machine "$val"
      ;;
    -r | --ram)
      parse_ram "$val"
      ;;
    -c | --cpu)
      parse_cpu "$val"
      ;;
    -d | --debug)
      parse_debug "$val"
      ;;
    -g | --gdb)
      start_gdb
      ;;
    -h | --help)
      print_usage
      exit 0
      ;;
    *)
      printf "Unknown option '%s'\\n" "${arg}"
      print_usage
      exit 1
      ;;
    esac
    shift 2
  done
}

run_qemu() {
  local command="${CONST_QEMU_BIN}"

  if [ -n "${QEMU_MACHINE}" ]; then
    command="${command} -machine ${QEMU_MACHINE}"
  fi

  command="${command} -m ${QEMU_RAM} -cpu ${QEMU_CPU} -bios ${QEMU_BIOS} ${QEMU_ARGS} ${QEMU_BOOT_DEVICE} ${QEMU_AUDIO_ARGS}"
  
  printf "Running: %s\\n" "${command}"

  if [ -n "${QEMU_GDB_PORT}" ]; then
    if [ "${QEMU_GDB_PORT}" == "1234" ]; then
      $command -gdb tcp::"${QEMU_GDB_PORT}" -S &
    else
      $command -gdb tcp::"${QEMU_GDB_PORT}" -S
    fi
  else
    $command
  fi
}

parse_args "$@"

if [ "${QEMU_BOOT_DEVICE}" == "" ]; then
  printf "No bootable image found!\\n"
  exit 1
fi

get_ovmf

QEMU_ARGS="${QEMU_ARGS}"
set_audio_parameters

run_qemu
