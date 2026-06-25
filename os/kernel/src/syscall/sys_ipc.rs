use alloc::collections::btree_map::BTreeMap;
use spin::Mutex;
use uuid::Uuid;


static MESSAGE_MAP: Mutex<BTreeMap<Uuid, usize>> = Mutex::new(BTreeMap::new());


pub extern "sysv64" fn sys_ipc_send(pid1: u64, pid2: u64, message: usize) -> isize {

    let pid = Uuid::from_u64_pair(pid1, pid2);

    MESSAGE_MAP.lock().insert(pid, message);

    return 0;
}