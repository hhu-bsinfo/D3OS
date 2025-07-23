#!/bin/bash
######################################################################
# @author      : Johann Spenrath (johann.spenrath@hhu.de)
# @file        : send_packets, listen for packets
# @created     : Dienstag Jul 22, 2025 14:55:26 CEST
#
# @description : 
# send_loop.sh — send a UDP datagram repeatedly with netcat
#
# Usage:
#   send_loop.sh <r/t> <host> <port> <payload> [interval_seconds] [count]
######################################################################
#
# Arguments:
#   host             remote hostname or IP
#   port             remote port number
#   payload          string to send (no newline)
#   interval_seconds time between sends (default: 1)
#   count            number of packets to send (default: infinite)
#

# -e : leave shell immediately if error occurs
# -u : 
set -eu

if [[ $# -lt 4 ]]; then
    cat <<EOF
Usage: $0 <mode> <host> <port> <payload> [interval_seconds] [count]

  mode             receive or send mode
  host             remote hostname or IP
  port             remote port number
  payload          string to send (no newline)
  interval_seconds time between sends (default: 1)
  count            number of packets to send (default: infinite)
EOF
    exit 1
fi

MODE="$1"
HOST="$2"
PORT="$3"
PAYLOAD="$4"
INTERVAL="${5:-1}"
COUNT="${6:-0}"

# If COUNT is zero, loop forever
case "$MODE" in 
    send)
    # SEND
    if [[ "$COUNT" -le 0 ]]; then
        while true; do
            printf '%s' "$PAYLOAD" | nc -u -w1 "$HOST" "$PORT"
            printf "%s : %d \n" "$PAYLOAD" "$COUNT"
            sleep "$INTERVAL"
        done
    else
        for ((i=1; i<=COUNT; i++)); do
            printf '%s' "$PAYLOAD" | nc -u -w1 "$HOST" "$PORT"
            printf "%s : %d \n" "$PAYLOAD" $i
            sleep "$INTERVAL"
        done
    fi
    ;;

    rec)
    # receive packets
    # -w : wait and close connection after n seconds
    # -l : listen for packets on the port
    # -u : udp 
    nc -u -w0 -l "$PORT"
    sleep "$INTERVAL"
    ;;
esac

