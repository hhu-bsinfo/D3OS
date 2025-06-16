#!/usr/bin/env bash

readonly INFINIBAND_FEATURE_MLX4="infiniband_mlx4"
readonly INFINIBAND_FEATURE_MLX5="infiniband_mlx5"

run () {
	local FEATURE="$1"

	cargo make --no-workspace \
        --env CARGO_INFINIBAND_FEATURE="$FEATURE" image
}

INFINIBAND_FEATURE=""

while getopts "d:" opt; do
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
    \?)
      echo "Usage: $0 -d [mlx4|mlx5]" >&2
      exit 1
      ;;
  esac
done

if [ $((OPTIND - 1)) -eq 0 ]; then
  echo "No options were specified - but at least one device required !"
  exit 1
fi

run "$INFINIBAND_FEATURE"