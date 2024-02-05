#!/bin/bash

readonly OVMF_URL="http://de.archive.ubuntu.com/ubuntu/pool/main/e/edk2/ovmf_2022.02-3_all.deb"

cleanup_and_exit() {
  local exit_code=$1

  rm -f edk2-ovmf.deb
  rm -f control.tar.zst
  rm -f data.tar.zst
  rm -f data.tar
  rm -f debian-binary
  rm -rf usr/

  exit $exit_code
}

if [[ -f "OVMF.fd" ]]; then
  exit 0
fi

wget -O edk2-ovmf.deb "${OVMF_URL}" || cleanup_and_exit 1

ar x edk2-ovmf.deb || cleanup_and_exit 1
zstd -d data.tar.zst || cleanup_and_exit 1
tar -xf data.tar || cleanup_and_exit 1

cp "usr/share/ovmf/OVMF.fd" "OVMF.fd" || cleanup_and_exit 1

cleanup_and_exit 0
