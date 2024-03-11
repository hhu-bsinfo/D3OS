#!/bin/bash

readonly CONST_QEMU_BIN="qemu-system-x86_64"
readonly CONST_QEMU_MACHINE_PC="pc"
readonly CONST_QEMU_CPU="qemu64"
readonly CONST_QEMU_MACHINE_PC_KVM="pc,accel=kvm,kernel-irqchip=split"
readonly CONST_QEMU_DEFAULT_RAM="128M"
readonly CONST_QEMU_BIOS_EFI="efi/OVMF.fd"
readonly CONST_QEMU_ARGS="-boot d -vga std -rtc base=localtime -device isa-debug-exit"
readonly CONST_QEMU_OLD_AUDIO_ARGS="-soundhw pcspk"
readonly CONST_QEMU_NEW_AUDIO_ARGS="-audiodev id=pa,driver=pa -machine pcspk-audiodev=pa"
readonly CONST_QEMU_BOOT_DEVICE="-drive driver=raw,node-name=boot,file.driver=file,file.filename=d3os.img"
readonly CONST_QEMU_GDB_PORT="1234"

QEMU_BIOS=""
QEMU_MACHINE="${CONST_QEMU_MACHINE_PC}"
QEMU_RAM="${CONST_QEMU_DEFAULT_RAM}"
QEMU_CPU="${CONST_QEMU_CPU}"
QEMU_AUDIO_ARGS="${CONST_QEMU_NEW_AUDIO_ARGS}"
QEMU_BOOT_DEVICE="${CONST_QEMU_BOOT_DEVICE}"
QEMU_ARGS="${CONST_QEMU_ARGS}"
QEMU_DEBUG_TYPE=""

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
  cd "efi" || exit 1
  ./build.sh || exit 1
  cd ".." || exit 1
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
}

parse_debug() {
  local type=$1

  if [ "${type}" == "default" ] || [ "${type}" == "background" ] || [ "${type}" == "vscode" ]; then
    QEMU_DEBUG_TYPE="${type}"
  else
    printf "Invalid debug type '%s'!\\n" "${type}"
    exit 1
  fi
}

parse_bios() {
  local bios=$1

  QEMU_BIOS="${bios}"
}

print_usage() {
  printf "Usage: ./run.sh [OPTION...]
    Available options:
    -f, --file
        Set the .iso or .img file, which qemu should boot (Default: d3os.img)
    -m, --machine
        Set the machine profile, which qemu should emulate ([pc] | [pc-kvm]) (Defualt: pc)
    -r, --ram
        Set the amount of ram, which qemu should use (e.g. 256, 1G, ...) (Default: 128M)
    -c, --cpu
        Set the CPU model, which qemu should emulate (e.g. 486, pentium, pentium2, ...) (Default: base)
    -d, --debug
        Enable debugging with a debug type ([default] | [vscode]) (default: Disabled)
    -b, --bios
        Set the BIOS file, which qemu should use (Default: Download OVMF from Ubuntu 20.04 packages)
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
    -b | --bios)
      parse_bios "$val"
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

  if [ "${QEMU_DEBUG_TYPE}" == "default" ]; then
    $command -gdb tcp::"${CONST_QEMU_GDB_PORT}" -S
  elif [ "${QEMU_DEBUG_TYPE}" == "background" ]; then
    $command -gdb tcp::"${CONST_QEMU_GDB_PORT}" -S &
  elif [ "${QEMU_DEBUG_TYPE}" == "vscode" ]; then
    echo "Debugging with VSCode..."; $command -gdb tcp::"${CONST_QEMU_GDB_PORT}" -S
  else
    $command
  fi
}

parse_args "$@"

if [ -z "${QEMU_BIOS}" ]; then
  QEMU_BIOS="${CONST_QEMU_BIOS_EFI}"
  get_ovmf
fi

QEMU_ARGS="${QEMU_ARGS}"
set_audio_parameters

run_qemu
