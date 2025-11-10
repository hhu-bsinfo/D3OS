IB_TARGET_NETWORK="${IB_TARGET_NET}"
IB_TARGET_PORT="${IB_TARGET_PORT}"
IB_TARGET_BREAK="${IB_TARGET_BREAK}" # format file:line_number
IB_TARGET_HOST="${IB_TARGET_HOST}"

exec_file="loader/profile/${IB_TARGET_HOST}/kernel.elf"

printf "target net => ${IB_TARGET_NETWORK}\n"
printf "target port => ${IB_TARGET_PORT}\n"
printf "target break => ${IB_TARGET_BREAK}\n"

rust-gdb -ex "target remote ${IB_TARGET_NETWORK}:${IB_TARGET_PORT}" \
    -iex "set disassembly-flavor intel" "${exec_file}" \
    -ex "break ${IB_TARGET_BREAK}" -ex "continue"