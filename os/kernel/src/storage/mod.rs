use alloc::format;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use log::info;
use smallmap::Map;
use spin::{Mutex, Once, RwLock};
use crate::device::ide;

static BLOCK_DEVICES: Once<RwLock<Map<String, Arc<dyn BlockDevice + Send + Sync>>>> = Once::new();
static DEVICE_TYPES: Once<Mutex<Map<String, usize>>> = Once::new();

/// Trait for accessing devices that can read and write data in fixed-size blocks (sectors)
/// This is the interface that the filesystems will use to access the storage devices
/// Sector addressing uses LBA (Logical Block Addressing) starting from 0
pub trait BlockDevice {
    fn read(&self, sector: u64, count: usize, buffer: &mut [u8]) -> usize;
    fn write(&self, sector: u64, count: usize, buffer: &[u8]) -> usize;
}

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
    let name = format!("{}{}", typ, index);
    types.insert(typ, index + 1);

    let mut drives = BLOCK_DEVICES.call_once(|| RwLock::new(Map::new())).write();
    drives.insert(name.clone(), drive);

    info!("Registered block device [{}]", name);
}

/// Get a block device by its name
pub fn block_device(name: &str) -> Option<Arc<dyn BlockDevice + Send + Sync>> {
    match BLOCK_DEVICES.call_once(|| RwLock::new(Map::new())).read().get(name) {
        None => None,
        Some(device) => Some(Arc::clone(device))
    }
}

/// Convert a Logical Block Address (LBA) to Cylinder-Head-Sector (CHS) addressing
/// This is a helper function, that may be used by drivers for legacy devices
pub fn lba_to_chs(lba: u64, heads: u8, sectors_per_cylinder: u8) -> (u16, u8, u8) {
    let cylinder = (lba / (heads as u64 * sectors_per_cylinder as u64)) as u16;
    let head = (lba % (heads as u64 * sectors_per_cylinder as u64)) as u8;
    let sector = (lba % sectors_per_cylinder as u64) as u8;

    (cylinder, head, sector)
}