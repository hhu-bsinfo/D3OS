#! /bin/sh

ERROR_GLOB=1
REMOTE_USER="sandro"
REMOTE_HOST="192.168.0.217"
REMOTE_DIR="/home/sandro/Desktop/OS/InfiniBand/D3OS"
MOUNT_POINT="./mnt"

RUN_NAME="$1"
RUN_SCRIPT="$2"

[ ! -e "$(pwd)/configs" ] && printf "must be located in home to run script ! ...\nExiting due to error !" && exit "$ERROR_GLOB"

if [ "$(ls -A ${MOUNT_POINT} | wc -l)" -eq 0 ]; then
  printf "mounting remote directory ${REMOTE_DIR} -> ${MOUNT_POINT}\n"
  sshfs "${REMOTE_USER}@${REMOTE_HOST}:${REMOTE_DIR}" "${MOUNT_POINT}" -o uid=$(id -u) -o gid=$(id -g) -o auto_unmount
else
  printf "skipping mounting ...\n"
fi

printf "switching to projects root directory and executing run-script, to start D30S!\n"
printf "run script => ${RUN_NAME} \n"

cd mnt

shift 2

/bin/sh "${RUN_SCRIPT}" "$@"

[ $? -ne 0 ] && printf "Script failed ..." && exit "$ERROR_GLOB"

printf "Script ran successfully ..."