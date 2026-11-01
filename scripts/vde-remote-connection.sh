# start this script from ib3

REMOTE_MACHINE="192.168.0.12"

dpipe vde_plug /tmp/vde-switch-ib3.ctl = ssh "${REMOTE_MACHINE}" vde_plug /tmp/vde-switch-ib4.ctl