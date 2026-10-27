#!/bin/bash

BASE_IP="${BASE_IP:=192.168.0}"
k=5 

while getopts "k:" opt; do
  case $opt in
    k)
      k=$OPTARG
      ;;
    *)
      echo "Usage: $0 [-k number_of_unused_ips]"
      exit 1
      ;;
  esac
done

count=0

for i in {1..254}; do
  if ping -c 1 -W 1 "${BASE_IP}.${i}" &> /dev/null; then
    echo "${BASE_IP}.${i} is alive"
  else
    echo "${BASE_IP}.${i} is not in use"
    ((count++))
    if (( count >= k )); then
      echo "Found $count unused IPs. Exiting."
      exit 0
    fi
  fi
done

echo "Scanned all IPs but found only $count unused IP(s)."
exit 0
