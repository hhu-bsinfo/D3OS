#!/bin/bash
TEMPLATE="source configs/defaults/infiniband-target-ib"
REMOTE_RUN="/bin/bash scripts/infiniband-remote-gdb.sh"

PTS_0="/dev/pts/0"
PTS_1="/dev/pts/1"
PTS_2="/dev/pts/2" # logger pts, could as well just be sub by logger file

if [[ ! -c "${PTS_0}" || ! -c "${PTS_1}" ]]; then
    printf "2 pts must exists before running this script, not including the master !";;
    exit 1;;
fi

CMD_1_STR="${TEMPLATE}3"
CMD_2_STR="${TEMPLATE}4"

[ -p /tmp/cmdpipe1 ] || mkfifo /tmp/cmdpipe1
[ -p /tmp/cmdpipe2 ] || mkfifo /tmp/cmdpipe2

cat < /tmp/cmdpipe1 > /dev/null &
DUMMY1_PID=$!

cat < /tmp/cmdpipe2 > /dev/null &
DUMMY2_PID=$!

exec 3> /tmp/cmdpipe1
exec 4> /tmp/cmdpipe2

kill "$DUMMY1_PID"
kill "$DUMMY2_PID"

bash -i < /tmp/cmdpipe1 > "${PTS_0}" 2>&1 &
BASH1_PID=$!
bash -i < /tmp/cmdpipe2 > "${PTS_1}" 2>&1 &
BASH2_PID=$!

sleep 2

echo "${CMD_1_STR}" >&3
echo "${REMOTE_RUN}" >&3

echo "${CMD_2_STR}" >&4
echo "${REMOTE_RUN}" >&4

jobs -l >& "${PTS_2}"
echo "Current PID ===> $$" > "${PTS_2}" 

wait "${BASH1_PID}" "${BASH2_PID}"