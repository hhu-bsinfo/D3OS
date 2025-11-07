#!/usr/bin/env bash

readonly INFINIBAND_FEATURE_MLX4="infiniband_mlx4"
readonly INFINIBAND_FEATURE_MLX5="infiniband_mlx5"
readonly BOOTLOADER_DIRECTORY="$(pwd)/loader"
readonly PROFILE_DIRECTORY="${BOOTLOADER_DIRECTORY}/profile"
readonly LINKER="ld"
readonly TOWBOOT_TARGET="$(pwd)/d3os.img"
readonly HOST_IB3="ib3"
readonly HOST_IB4="ib4"
readonly INFINIBAND_OP_READ="read"
readonly INFINIBAND_OP_WRITE="write"
readonly INFINIBAND_OP_STAT="stat"
readonly BENCH_OP_LATENCY="latency"
readonly BENCH_OP_HIT="hit"
readonly BENCH_OP_THROUGHPUT="throughput"

function bench {
  local FEATURE="$1"
  local OP="$2"
  local HOST="$3"
  local SOURCE_IP="$4"
  local TARGET_IP="$5"
  local TARGET_PORT="$6"
  local IS_SENDER="$7"
  local GW_IP="$8"
  local BENCH_OP="$9"

  local current_profile="${PROFILE_DIRECTORY}/${HOST}"

  cp -R "${BOOTLOADER_DIRECTORY}/initrd" "${current_profile}"

  ln -sf "${BOOTLOADER_DIRECTORY}/towboot.toml" "${current_profile}/towboot-cfg"

  CARGO_ROOT_DIR=$(git rev-parse --show-toplevel)

  printf "Root level directory : %s" "${CARGO_ROOT_DIR}"

  cargo make --cwd os/kernel --no-workspace \
      --env IB_PROFILE="${current_profile}" --env CARGO_INFINIBAND_FEATURE="${FEATURE}" --env BOOTLOADER_DIRECTORY="${BOOTLOADER_DIRECTORY}" \
      --env LINKER="${LINKER}" --env HOST_MACHINE="${HOST}" --env SOURCE_IP="${SOURCE_IP}" \
      --env TARGET_IP="${TARGET_IP}" --env GATEWAY_IP="${GW_IP}" --env CARGO_ROOT_DIR="${CARGO_ROOT_DIR}" bench
  
  cargo make --cwd os/application/rdma/mlx4 --no-workspace \
      --env IB_PROFILE="${current_profile}" --env CARGO_INFINIBAND_OPERATION="${OP}" --env BENCH_OPERATION="${BENCH_OP}" --env BOOTLOADER_DIRECTORY="${BOOTLOADER_DIRECTORY}" \
      --env LINKER="${LINKER}" --env HOST_MACHINE="${HOST}" --env SOURCE_IP="${SOURCE_IP}" \
      --env TARGET_IP="${TARGET_IP}" --env TARGET_PORT="${TARGET_PORT}" --env IS_SENDER="${IS_SENDER}" --env CARGO_ROOT_DIR="${CARGO_ROOT_DIR}" bench

  cargo make --no-workspace towbootctl
  TARGET="${TOWBOOT_TARGET%d3os\.img}d3os-${HOST}.img"

  pushd "${current_profile}" > /dev/null
  pushd "initrd" > /dev/null
  tar -cf "${current_profile}/initrd.tar" *

  popd > /dev/null # move stack pointer to prior dir

  ${CARGO_ROOT_DIR}/towbootctl image --target "${TARGET}" -- -config "towboot-cfg"

  popd > /dev/null # move stack pointer to prior dir
}

function test {
  local FEATURE="$1"
  local OP="$2"
  local HOST="$3"
  local SOURCE_IP="$4"
  local TARGET_IP="$5"
  local TARGET_PORT="$6"
  local IS_SENDER="$7"
  local GW_IP="$8"

  local current_profile="${PROFILE_DIRECTORY}/${HOST}"

  cp -R "${BOOTLOADER_DIRECTORY}/initrd" "${current_profile}"

  ln -sf "${BOOTLOADER_DIRECTORY}/towboot.toml" "${current_profile}/towboot-cfg"

  CARGO_ROOT_DIR=$(git rev-parse --show-toplevel)

  printf "Root level directory : %s" "${CARGO_ROOT_DIR}"

  cargo make --cwd os/kernel --no-workspace \
      --env IB_PROFILE="${current_profile}" --env CARGO_INFINIBAND_FEATURE="${FEATURE}" --env BOOTLOADER_DIRECTORY="${BOOTLOADER_DIRECTORY}" \
      --env LINKER="${LINKER}" --env HOST_MACHINE="${HOST}" --env SOURCE_IP="${SOURCE_IP}" \
      --env TARGET_IP="${TARGET_IP}" --env GATEWAY_IP="${GW_IP}" --env CARGO_ROOT_DIR="${CARGO_ROOT_DIR}" test
  
  cargo make --cwd os/application/rdma/mlx4 --no-workspace \
      --env IB_PROFILE="${current_profile}" --env CARGO_INFINIBAND_OPERATION="${OP}" --env BOOTLOADER_DIRECTORY="${BOOTLOADER_DIRECTORY}" \
      --env LINKER="${LINKER}" --env HOST_MACHINE="${HOST}" --env SOURCE_IP="${SOURCE_IP}" \
      --env TARGET_IP="${TARGET_IP}" --env TARGET_PORT="${TARGET_PORT}" --env IS_SENDER="${IS_SENDER}" --env CARGO_ROOT_DIR="${CARGO_ROOT_DIR}" test

  cargo make --no-workspace towbootctl
  TARGET="${TOWBOOT_TARGET%d3os\.img}d3os-${HOST}.img"

  pushd "${current_profile}" > /dev/null
  pushd "initrd" > /dev/null
  tar -cf "${current_profile}/initrd.tar" *

  popd > /dev/null # move stack pointer to prior dir

  ${CARGO_ROOT_DIR}/towbootctl image --target "${TARGET}" -- -config "towboot-cfg"

  popd > /dev/null # move stack pointer to prior dir
}

function build {
  local FEATURE="$1"
  
	cargo make --no-workspace \
        --env CARGO_INFINIBAND_FEATURE="$FEATURE" image
}

run () {
	local FEATURE="$1"
  local OP="$2"
  local HOST="$3"
  local SOURCE_IP="$4"
  local TARGET_IP="$5"
  local TARGET_PORT="$6"
  local IS_SENDER="$7"
  local GW_IP="$8"
  local BENCH_OP="$9"
  local PROCEDURE="${10}"

  cargo make --no-workspace hdd
  cargo make --no-workspace ovmf

  printf "$PROCEDURE"

  if declare -f "$PROCEDURE" > /dev/null; then 
    "$PROCEDURE" "$FEATURE" "$OP" "$HOST" "$SOURCE_IP" "$TARGET_IP" "$TARGET_PORT" "$IS_SENDER" "$GW_IP" "$BENCH_OP"
  else
    echo "Unknown command: $PROCEDURE"
    exit 1
  fi
}

INFINIBAND_FEATURE=""
INFINIBAND_OP=""
BENCH_OP=""
HOST_NAME=""
TARGET_IP=""
SOURCE_IP=""
TARGET_PORT=""
IS_SENDER="true"
GW_IP=""

while getopts "d:o:h:s:t:rg:p:b:" opt; do
  case "$opt" in
    d)
      case "${OPTARG}" in
        mlx4)
          INFINIBAND_FEATURE="$INFINIBAND_FEATURE_MLX4"
          ;;
        mlx5)
          INFINIBAND_FEATURE="$INFINIBAND_FEATURE_MLX5"
          ;;
        *)
          echo "Unsupported device type: ${OPTARG}" >&2
          exit 1
          ;;
      esac
      ;;
    o)
      case "${OPTARG}" in
        read | r)
          INFINIBAND_OP="$INFINIBAND_OP_READ"
          ;;
        write | w)
          INFINIBAND_OP="$INFINIBAND_OP_WRITE"
          ;;
        stat | s)
          INFINIBAND_OP="$INFINIBAND_OP_STAT"
          ;;
        *)
          echo "Unsupported operation : ${OPTARG}" >&2
          exit 1
          ;;
      esac
      ;;
    b)
      case "${OPTARG}" in
        latency)
          BENCH_OP="$BENCH_OP_LATENCY"
          ;;
        hit)
          BENCH_OP="$BENCH_OP_HIT"
          ;;
        throughput)
          BENCH_OP="$BENCH_OP_THROUGHPUT"
          ;;
        *)
          echo "Unsupported operation : ${OPTARG}" >&2
          exit 1
          ;;
      esac
      ;;
    h)
      case "${OPTARG}" in
        ib3)
          HOST_NAME="$HOST_IB3"
          ;;
        ib4)
          HOST_NAME="$HOST_IB4"
          ;;
        *)
          echo "Unsupported host: ${OPTARG}" >&2
          exit 1
          ;;
      esac
      ;;
    s)
      SOURCE_IP="${OPTARG}" # assuming a valid ip
      ;;
    t)
      TARGET_IP="${OPTARG}" # assuming a valid ip
      ;;
    r)
      IS_SENDER="false"
      ;;
    g)
      GW_IP="${OPTARG}" # assuming a valid ip
      ;;
    p)
      TARGET_PORT="${OPTARG}" # assuming a valid port
      ;;
    \?)
      echo "Usage: $0 -d [mlx4|mlx5] -o [[read|r]|[write|w]|[stat|s]] -h [ib3|ib4] -s [ip4] -t [ip4] -r [-- procedure]" >&2
      exit 1
      ;;
  esac
done

if [ $((OPTIND - 1)) -eq 0 ]; then
  echo "No options were specified - but at least one device required !"
  exit 1
fi

shift $((OPTIND - 2))

[ "$1" = "--" ] && [[ ! "$2" =~ ^(bench|build|test)$ ]] && { echo "Usage: $0 -d [mlx4|mlx5] -o [[read|r]|[write|w]|[stat|s]] -h [ib3|ib4] -s [ip4] -t [ip4] -r [-- procedure]" >&2; exit 1; }

printf "$2"

run "$INFINIBAND_FEATURE" "$INFINIBAND_OP" "$HOST_NAME" "$SOURCE_IP" "$TARGET_IP" "$TARGET_PORT" "$IS_SENDER" "$GW_IP" "$BENCH_OP" "$2"