#!/bin/bash
# Usage: ./send_init_nc.sh <host> <port>

######################################################################
# @author      : john (john@$HOSTNAME)
# @file        : netcat
# @created     : Mittwoch Aug 20, 2025 16:11:58 CEST
#
# @description : 
######################################################################


HOST="${1:-127.0.0.1}"
PORT="${2:-12345}"

echo -n "Init" | nc -u -w1 "$HOST" "$PORT"




