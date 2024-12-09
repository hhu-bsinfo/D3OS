use alloc::sync::Arc;
use alloc::vec::Vec;
use log::info;
use spin::{Once, RwLock};
use crate::device::ide::{IdeController, IdeDrive};
use crate::pci_bus;

static IDE_CONTROLLER: Once<Arc<IdeController>> = Once::new();
static IDE_DRIVES: RwLock<Vec<Arc<IdeDrive>>> = RwLock::new(Vec::new());

pub fn init() {
    let devices = pci_bus().search_by_class(0x01, 0x01);
    if devices.len() > 0 {
        IDE_CONTROLLER.call_once(|| {
            info!("Found IDE controller");
            let ide_controller = Arc::new(IdeController::new(devices[0]));
            let found_drives = ide_controller.init_drives();
            let mut drives = IDE_DRIVES.write();

            for drive in found_drives.iter() {
                drives.push(Arc::new(IdeDrive::new(Arc::clone(&ide_controller), *drive)));
            }

            IdeController::plugin(Arc::clone(&ide_controller));
            ide_controller
        });
    }
}

pub fn ide_drive(num: usize) -> Option<Arc<IdeDrive>> {
    let drives = IDE_DRIVES.read();
    if num < drives.len() {
        Some(Arc::clone(&drives[num]))
    } else {
        None
    }
}

pub fn lba_to_chs(lba: u64, heads: u8, sectors_per_cylinder: u8) -> (u16, u8, u8) {
    let cylinder = (lba / (heads as u64 * sectors_per_cylinder as u64)) as u16;
    let head = (lba % (heads as u64 * sectors_per_cylinder as u64)) as u8;
    let sector = (lba % sectors_per_cylinder as u64) as u8;

    (cylinder, head, sector)
}