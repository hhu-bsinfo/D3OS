use alloc::sync::Arc;
use alloc::vec::Vec;
use mbrs::Mbr;

/// Trait for accessing devices that can read and write data in fixed-size blocks (sectors)
/// This is the interface that the filesystems will use to access the storage devices
/// Sector addressing uses LBA (Logical Block Addressing) starting from 0
pub trait BlockDevice {
    /// Read a given number of sectors into the provided buffer.
    fn read(&self, sector: u64, count: usize, buffer: &mut [u8]) -> usize;

    /// Write a given number of sectors from the provided buffer.
    fn write(&self, sector: u64, count: usize, buffer: &[u8]) -> usize;

    /// Get the size of the device in bytes.
    fn sector_count(&self) -> u64;

    /// Get the size of a sector in bytes.
    fn sector_size(&self) -> u16;
}

/// Convert a Logical Block Address (LBA) to Cylinder-Head-Sector (CHS) addressing.
/// This is a helper function, that may be used by drivers for legacy devices.
pub fn lba_to_chs(lba: u64, heads: u8, sectors_per_cylinder: u8) -> (u16, u8, u8) {
    let cylinder = (lba / (heads as u64 * sectors_per_cylinder as u64)) as u16;
    let head = (lba % (heads as u64 * sectors_per_cylinder as u64)) as u8;
    let sector = (lba % sectors_per_cylinder as u64) as u8;

    (cylinder, head, sector)
}

/// Scan a block device for partitions using the MBR (Master Boot Record) partition table.
/// The device is given as an Arc reference to allow sharing it between partitions.
pub fn scan_partitions(device: &Arc<dyn BlockDevice + Send + Sync>) -> Vec<Arc<dyn BlockDevice + Send + Sync>> {
    // Read the MBR (Master Boot Record) from the device
    let mut buffer = [0u8; 512];
    device.read(0, 1, &mut buffer);

    let mut partitions = Vec::<Arc<dyn BlockDevice + Send + Sync>>::new();

    // Iterate over the partition entries and create a Partition object for each valid one
    if let Ok(mbr) = Mbr::try_from_bytes(&buffer) {
        for entry in mbr.partition_table.entries {
            if entry.is_some() {
                let entry = entry.unwrap();
                partitions.push(Arc::new(Partition::new(Arc::clone(device), entry.start_sector_lba() as u64, entry.sector_count_lba() as u64)));
            }
        }
    }

    partitions
}

/// A partition on a block device.
/// Holds a reference to the device it is one and passes through read/write requests.
/// Sector boundaries are checked to prevent reading/writing outside the partition.
struct Partition {
    device: Arc<dyn BlockDevice + Send + Sync>,
    start_sector: u64,
    sector_count: u64
}

impl Partition {
    fn new(device: Arc<dyn BlockDevice + Send + Sync>, start_sector: u64, sector_count: u64) -> Self {
        Partition { device, start_sector, sector_count }
    }
}

impl BlockDevice for Partition {
    fn read(&self, sector: u64, count: usize, buffer: &mut [u8]) -> usize {
        if sector >= self.sector_count {
            return 0;
        }

        let sector = sector + self.start_sector;
        let count = count.min((self.sector_count - sector) as usize);
        self.device.read(sector, count, buffer)
    }

    fn write(&self, sector: u64, count: usize, buffer: &[u8]) -> usize {
        if sector >= self.sector_count {
            return 0;
        }

        let sector = sector + self.start_sector;
        let count = count.min((self.sector_count - sector) as usize);
        self.device.write(sector, count, buffer)
    }

    fn sector_count(&self) -> u64 {
        self.sector_count
    }

    fn sector_size(&self) -> u16 {
        self.device.sector_size()
    }
}