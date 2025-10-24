IB_TARGET_NETWORK="${IB_TARGET_NET}"
IB_TARGET_PORT="${IB_TARGET_PORT}"
IB_TARGET_BREAK="${IB_TARGET_BREAK}" # format file:line_number
IB_TARGET_HOST="${IB_TARGET_HOST}"

printf "target net => ${IB_TARGET_NETWORK}\n"
printf "target port => ${IB_TARGET_PORT}\n"
printf "target break => ${IB_TARGET_BREAK}\n"

rust-gdb -ex "target remote ${IB_TARGET_NETWORK}:${IB_TARGET_PORT}" \
    -iex "set disassembly-flavor intel" loader/kernel-${IB_TARGET_HOST}.elf \
    -ex "break ${IB_TARGET_BREAK}" -ex "continue"