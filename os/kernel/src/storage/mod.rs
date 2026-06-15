use alloc::format;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use log::info;
use smallmap::Map;
use spin::{Mutex, Once, RwLock};
use crate::device::ide;
use crate::storage::block::BlockDevice;

pub mod block;

static BLOCK_DEVICES: Once<RwLock<Map<String, Arc<dyn BlockDevice + Send + Sync>>>> = Once::new();
static DEVICE_TYPES: Once<Mutex<Map<String, usize>>> = Once::new();

/// Initialize all storage drivers
pub fn init() {
    ide::init();
}

/// Register a block device with the given type
/// The type is used to generate a unique name for the device (e.g. type "ata" will generate names "ata0", "ata1", etc.)
pub fn add_block_device(typ: &str, drive: Arc<dyn BlockDevice + Send + Sync>) {
    let typ = typ.to_string();
    let mut types = DEVICE_TYPES.call_once(|| Mutex::new(Map::new())).lock();
    let index = *types.get(&typ).unwrap_or(&0);
    let name = format!("{typ}{index}");
    types.insert(typ, index + 1);

    let partitions = block::scan_partitions(&drive);

    let mut drives = BLOCK_DEVICES.call_once(|| RwLock::new(Map::new())).write();
    drives.insert(name.clone(), drive);
    info!("Registered block device [{name}]");

    for (index, partition) in partitions.into_iter().enumerate() {
        let name = format!("{name}p{index}");
        drives.insert(name.clone(), partition);
        info!("Registered partition [{name}]");
    }
}

/// Get a block device by its name
pub fn block_device(name: &str) -> Option<Arc<dyn BlockDevice + Send + Sync>> {
    match BLOCK_DEVICES.call_once(|| RwLock::new(Map::new())).read().get(name) {
        None => None,
        Some(device) => Some(Arc::clone(device))
    }
}