/* ╔══════════════════════════════════════════════════════════════════════════════════════════════════╗
   ║ Module: ide                                                                                      ║
   ╟──────────────────────────────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: The IDE driver is based on a bachelor's thesis, written by Tim Laurischkat.              ║
   ║          he original source code can be found here: https://git.hhu.de/bsinfo/thesis/ba-tilau101 ║
   ╟──────────────────────────────────────────────────────────────────────────────────────────────────╢
   ║ Author: Tim Laurischkat & Fabian Ruhland, HHU                                                    ║
   ╚══════════════════════════════════════════════════════════════════════════════════════════════════╝
*/

use crate::interrupt::interrupt_dispatcher::InterruptVector;
use crate::interrupt::interrupt_handler::InterruptHandler;
use crate::memory::PAGE_SIZE;
use crate::storage::block::BlockDevice;
use crate::storage::{add_block_device, block};
use crate::{apic, interrupt_dispatcher, memory, pci_bus, scheduler, timer};
use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use bitflags::bitflags;
use core::sync::atomic::{AtomicBool, Ordering};
use core::{ops::BitOr, slice, str};
use log::{error, info, warn};
use pci_types::{CommandRegister, ConfigRegionAccess, EndpointHeader};
use spin::{Mutex, RwLock};
use x86_64::instructions::port::{Port, PortReadOnly, PortWriteOnly};

/// Initialize all IDE controllers found on the PCI bus.
/// Each connected drive gets registered as a block device in the storage module.
pub fn init() {
    let devices = pci_bus().search_by_class(0x01, 0x01);
    for device in devices {
        let device_id = device.read().header().id(pci_bus().config_space());
        info!("Found IDE controller [{}:{}]", device_id.0, device_id.1);

        let ide_controller = Arc::new(IdeController::new(device));
        IdeController::plugin(Arc::clone(&ide_controller));

        let found_drives = ide_controller.init_drives();
        for drive in found_drives.iter() {
            let block_device = Arc::new(IdeDrive::new(Arc::clone(&ide_controller), *drive));
            add_block_device("ata", block_device);
        }
    }
}

/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Constants needed for the driver.                                        ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
const CHANNELS_PER_CONTROLLER: u8 = 2;
const DEVICES_PER_CHANNEL: u8 = 2;
const DEFAULT_BASE_ADDRESSES: [u16; DEVICES_PER_CHANNEL as usize] = [0x01f0, 0x0170];
const DEFAULT_CONTROL_BASE_ADDRESSES: [u16; DEVICES_PER_CHANNEL as usize] = [0x03f4, 0x0374];
const COMMAND_SET_WORD_COUNT: usize = 6;
const WAIT_ON_STATUS_TIMEOUT: usize = 4095;
const DMA_TIMEOUT: usize = 30000;
const ATAPI_CYLINDER_LOW_V1: u8 = 0x14;
const ATAPI_CYLINDER_HIGH_V1: u8 = 0xeb;
const ATAPI_CYLINDER_LOW_V2: u8 = 0x69;
const ATAPI_CYLINDER_HIGH_V2: u8 = 0x96;

/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Enums and structs needed to communicate with the ide controller.        ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
enum DriveType {
    Ata,
    Atapi,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
enum TransferMode {
    Read,
    Write,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
enum AddressType {
    Chs = 0x00,
    Lba28 = 0x01,
    Lba48 = 0x02,
}

enum IdentifyFieldOffset {
    DeviceType = 0,
    Cylinders = 1,
    Heads = 3,
    Sectors = 6,
    Serial = 10,
    Firmware = 23,
    Model = 27,
    Capabilities = 49,
    MaxLba = 60,
    DmaMulti = 63,
    MajorVersion = 80,
    MinorVersion = 81,
    CommandSets = 82,
    UdmaModes = 88,
}

#[repr(u8)]
enum Command {
    ReadPioLba28 = 0x20,
    ReadPioLba48 = 0x24,
    ReadDmaLba28 = 0xC8,
    ReadDmaLba48 = 0x25,
    WritePioLba28 = 0x30,
    WritePioLba48 = 0x34,
    WriteDmaLba28 = 0xca,
    WriteDmaLba48 = 0x35,
    IdentifyAtaDrive = 0xec,
    IdentifyAtapiDrive = 0xa1,
}

bitflags! {
    #[repr(C)]
    #[derive(Debug, Clone, Copy)]
    struct Status: u8 {
        const None = 0x00;
        const Error = 0x01;
        const DataRequest = 0x08;
        const DriveReady = 0x40;
        const Busy = 0x80;
    }
}

#[repr(u8)]
enum DmaCommand {
    Enable = 0x01,
    Direction = 0x08,
}

bitflags! {
    #[repr(C)]
    #[derive(Debug, Clone, Copy)]
    struct DmaStatus: u8 {
        const BusMasterActive = 0x01;
        const DmaError = 0x02;
        const Interrupt = 0x04;
        const DmaSupportedDrive0 = 0x20;
        const DmaSupportedDrive1 = 0x40;
        const Simplex = 0x80;
    }
}

bitflags! {
    #[repr(C)]
    #[derive(Debug, Clone, Copy)]
    struct PrdFlags: u16 {
        const END_OF_TRANSMISSION = 1 << 15;
    }
}

#[derive(Copy, Clone)]
#[repr(C, packed)]
struct PrdEntry {
    base_address: u32,
    byte_count: u16,
    flags: PrdFlags,
}

/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Register structs.                                                       ║
   ║ The registers are divided into three blocks: Control, Command and DMA.  ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/

struct ControlRegisters {
    alternate_status: PortReadOnly<u8>,
    device_control: PortWriteOnly<u8>,
}

impl ControlRegisters {
    fn new(base_address: u16) -> Self {
        let alternate_status = PortReadOnly::new(base_address + 0x02);
        let device_control = PortWriteOnly::new(base_address + 0x02);

        Self {
            alternate_status,
            device_control,
        }
    }
}

struct CommandRegisters {
    data: Port<u16>,
    error: PortReadOnly<u8>,
    sector_count: Port<u8>,
    sector_number: Port<u8>,
    lba_low: Port<u8>,
    cylinder_low: Port<u8>,
    lba_mid: Port<u8>,
    cylinder_high: Port<u8>,
    lba_high: Port<u8>,
    drive_head: Port<u8>,
    status: PortReadOnly<u8>,
    command: PortWriteOnly<u8>,
}

impl CommandRegisters {
    fn new(base_address: u16) -> Self {
        let data = Port::new(base_address + 0x00);
        let error = PortReadOnly::new(base_address + 0x01);
        let sector_count = Port::new(base_address + 0x02);
        let sector_number = Port::new(base_address + 0x03);
        let lba_low = Port::new(base_address + 0x03);
        let cylinder_low = Port::new(base_address + 0x04);
        let lba_mid = Port::new(base_address + 0x04);
        let cylinder_high = Port::new(base_address + 0x05);
        let lba_high = Port::new(base_address + 0x05);
        let drive_head = Port::new(base_address + 0x06);
        let status = PortReadOnly::new(base_address + 0x07);
        let command = PortWriteOnly::new(base_address + 0x07);

        Self {
            data,
            error,
            sector_count,
            sector_number,
            lba_low,
            cylinder_low,
            lba_mid,
            cylinder_high,
            lba_high,
            drive_head,
            status,
            command,
        }
    }
}

struct DmaRegisters {
    command: Port<u8>,
    status: Port<u8>,
    address: Port<u32>,
}

impl DmaRegisters {
    fn new(base_address: u16) -> Self {
        let command = Port::new(base_address + 0x00);
        let status = Port::new(base_address + 0x02);
        let address = Port::new(base_address + 0x04);

        Self { command, status, address }
    }
}

/* ╔══════════════════════════════════════════════════════════════════════════════════════════════════╗
   ║ The actual driver implementation.                                                                ║
   ║ The driver is divided into two three structs: IdeController, IdeChannel and IdeDrive.            ║
   ║ Each controller manages two channels and each channel can have up to two drives connected to it. ║
   ║ IdeDrive implements the BlockDevice trait, so that the OS can access ide devices.                ║
   ╚══════════════════════════════════════════════════════════════════════════════════════════════════╝
*/

/// Each IDE controller has two channels, each of which can have up to two drives connected to it.
/// This struct only manages the controller itself. Drive access is implemented in the `IdeChannel` struct.
struct IdeController {
    channels: [Mutex<IdeChannel>; CHANNELS_PER_CONTROLLER as usize],
}

impl IdeController {
    fn new(pci_device: &RwLock<EndpointHeader>) -> Self {
        let mut channels: [Mutex<IdeChannel>; CHANNELS_PER_CONTROLLER as usize] = [Mutex::new(IdeChannel::default()), Mutex::new(IdeChannel::default())];

        let pci_config_space = pci_bus().config_space();
        let mut pci_device = pci_device.write();

        let id = pci_device.header().id(pci_config_space);
        let mut rev_and_class = pci_device.header().revision_and_class(pci_config_space);
        info!("Initializing IDE controller [0x{:04x}:0x{:04x}]", id.0, id.1);

        let mut supports_dma = false;
        pci_device.update_command(pci_config_space, |command| {
            if rev_and_class.3 & 0x80 == 0x80 {
                // Bit 7 in programming defines whether the controller supports DMA
                info!("IDE controller supports DMA");
                supports_dma = true;
                command.bitor(CommandRegister::IO_ENABLE | CommandRegister::BUS_MASTER_ENABLE)
            } else {
                info!("IDE controller does not support DMA");
                command.bitor(CommandRegister::IO_ENABLE)
            }
        });

        for i in 0..CHANNELS_PER_CONTROLLER {
            let dma_base_address: u16 = match supports_dma {
                true => pci_device.bar(4, pci_config_space).expect("Failed to read DMA base address").unwrap_io() as u16,
                false => 0,
            };

            let mut interface = rev_and_class.3 >> i * 2; // Each channel has two bits in the programming interface
            // First bit defines whether the channel is running in compatibility or native mode
            // Second bit defines whether mode change is supported
            if interface & 0x01 == 0x00 && interface & 0x02 == 0x02 {
                info!("Changing mode of channel [{i}] to native mode");
                unsafe {
                    // Set first bit of channel interface to 1
                    let value = pci_config_space.read(pci_device.header().address(), 0x08);
                    pci_config_space.write(pci_device.header().address(), 0x08, value | (0x01 << i * 2) << 8);
                }

                rev_and_class = pci_device.header().revision_and_class(pci_config_space);
                interface = rev_and_class.3 >> i * 2;
            }

            let mut interrupts: [InterruptVector; CHANNELS_PER_CONTROLLER as usize] = [InterruptVector::PrimaryAta, InterruptVector::SecondaryAta];
            let command_and_control_base_address = match interface & 0x01 {
                0x00 => {
                    // Channel is running in compatibility mode -> Use default base address
                    info!("Channel [{i}] is running in compatibility mode");
                    (DEFAULT_BASE_ADDRESSES[i as usize], DEFAULT_CONTROL_BASE_ADDRESSES[i as usize])
                }
                _ => {
                    // Channel is running in native mode -> Read base address from PCI registers
                    info!("Channel [{i}] is running in native mode");
                    interrupts[i as usize] = InterruptVector::try_from(pci_device.interrupt(pci_config_space).0).unwrap();

                    (
                        pci_device.bar(0, pci_config_space).expect("Failed to read command base address").unwrap_io() as u16,
                        pci_device.bar(1, pci_config_space).expect("Failed to read control base address").unwrap_io() as u16,
                    )
                }
            };

            channels[i as usize] = Mutex::new(IdeChannel::new(
                i,
                interrupts[i as usize],
                supports_dma,
                command_and_control_base_address.0,
                command_and_control_base_address.1,
                dma_base_address,
            ));
        }

        Self { channels }
    }

    fn init_drives(&self) -> Vec<DriveInfo> {
        let mut drives: Vec<DriveInfo> = Vec::new();

        for channel in self.channels.iter() {
            let mut channel = channel.lock();
            for i in 0..DEVICES_PER_CHANNEL {
                if !channel.reset_drive(i) {
                    continue;
                }

                if let Some(info) = channel.identify_drive(i) {
                    info!(
                        "Found {:?} drive on channel [{}]: {} {} (Firmware: [{}])",
                        info.typ,
                        channel.index,
                        info.model(),
                        info.serial(),
                        info.firmware()
                    );
                    drives.push(info);
                }
            }
        }

        drives
    }

    fn plugin(controller: Arc<IdeController>) {
        let primary_channel = controller.channels[0].lock();
        let secondary_channel = controller.channels[1].lock();

        interrupt_dispatcher().assign(
            primary_channel.interrupt,
            Box::new(IdeInterruptHandler::new(Arc::clone(&primary_channel.received_interrupt))),
        );
        apic().allow(primary_channel.interrupt);

        interrupt_dispatcher().assign(
            secondary_channel.interrupt,
            Box::new(IdeInterruptHandler::new(Arc::clone(&secondary_channel.received_interrupt))),
        );
        apic().allow(secondary_channel.interrupt);
    }

    fn copy_byte_swapped_string(source: &[u16], target: &mut [u8]) {
        for i in (0..target.len()).step_by(2) {
            let bytes = source[i / 2];
            target[i] = ((bytes & 0xff00) >> 8) as u8;
            target[i + 1] = (bytes & 0x00ff) as u8;
        }
    }
}

/// A drive connected to an IDE controller
/// Each drive has a reference to its controller and knows its channel via the `info.channel` filed.
/// It implements the `BlockDevice` trait by calling `perform_ata_io()` on the channel.
pub struct IdeDrive {
    controller: Arc<IdeController>,
    info: DriveInfo,
}

impl IdeDrive {
    fn new(controller: Arc<IdeController>, info: DriveInfo) -> Self {
        Self { controller, info }
    }
}

impl BlockDevice for IdeDrive {
    fn read(&self, sector: u64, count: usize, buffer: &mut [u8]) -> usize {
        let channel = &mut self.controller.channels[self.info.channel as usize].lock();
        channel.perform_ata_io(&self.info, TransferMode::Read, sector, count, buffer)
    }

    fn write(&self, sector: u64, count: usize, buffer: &[u8]) -> usize {
        // Channel::perform_ata_io() expects a mutable buffer, so we need to cast it to a mutable slice.
        // This is safe, as the buffer is not modified by the function.
        let buffer = unsafe { slice::from_raw_parts_mut(buffer.as_ptr().cast_mut(), buffer.len()) };

        let channel = &mut self.controller.channels[self.info.channel as usize].lock();
        channel.perform_ata_io(&self.info, TransferMode::Write, sector, count, buffer)
    }

    fn sector_count(&self) -> u64 {
        self.info.sector_count()
    }

    fn sector_size(&self) -> u16 {
        self.info.sector_size
    }
}

/// Information about a drive connected to an IDE controller
/// This struct is created in the `IdeChannel::identify_drive()` method.
#[derive(Copy, Clone)]
struct DriveInfo {
    channel: u8,                                 // 0 (Primary Channel) or 1 (Secondary Channel)
    drive: u8,                                   // 0 (Master Drive) or 1 (Slave Drive)
    typ: DriveType,                              // 0 (ATA) or 1 (ATAPI)
    cylinders: u16,                              // Number of logical cylinders of the drive
    heads: u16,                                  // Number of logical heads of the drive
    sectors_per_track: u16,                      // Number of sectors per track of the drive
    signature: u16,                              // Drive Signature
    capabilities: u16,                           // Features
    multiword_dma: u8,                           // Supported versions of multiword dma
    ultra_dma: u8,                               // Supported versions of ultra dma
    command_sets: [u16; COMMAND_SET_WORD_COUNT], // Supported command sets
    max_sectors_lba48: u32,                      // Size in Sectors LBA48
    max_sectors_lba28: u32,                      // Size in Sectors LBA28 / CHS
    model: [u8; 40],                             // Model as string
    serial: [u8; 10],                            // Serial number as string
    firmware: [u8; 4],                           // Firmware revision as string
    major_version: u16,                          // Major ATA Version supported
    minor_version: u16,                          // Minor ATA Version supported
    addressing: AddressType,                     // CHS (0), LBA28 (1), LBA48 (2)
    sector_size: u16,                            // Sector size
}

impl Default for DriveInfo {
    fn default() -> Self {
        Self {
            channel: 0,
            drive: 0,
            typ: DriveType::Other,
            cylinders: 0,
            heads: 0,
            sectors_per_track: 0,
            signature: 0,
            capabilities: 0,
            multiword_dma: 0,
            ultra_dma: 0,
            command_sets: [0; COMMAND_SET_WORD_COUNT],
            max_sectors_lba48: 0,
            max_sectors_lba28: 0,
            model: [0; 40],
            serial: [0; 10],
            firmware: [0; 4],
            major_version: 0,
            minor_version: 0,
            addressing: AddressType::Chs,
            sector_size: 0,
        }
    }
}

impl DriveInfo {
    fn model(&self) -> &str {
        str::from_utf8(&self.model).expect("Failed to parse model string").trim()
    }

    fn serial(&self) -> &str {
        str::from_utf8(&self.serial).expect("Failed to parse serial string").trim()
    }

    fn firmware(&self) -> &str {
        str::from_utf8(&self.firmware).expect("Failed to parse firmware string").trim()
    }

    fn supports_dma(&self) -> bool {
        self.ultra_dma != 0 || self.multiword_dma != 0
    }

    fn sector_count(&self) -> u64 {
        match self.addressing {
            AddressType::Chs => self.cylinders as u64 * self.heads as u64 * self.sectors_per_track as u64,
            AddressType::Lba28 => self.max_sectors_lba28 as u64,
            AddressType::Lba48 => self.max_sectors_lba48 as u64,
        }
    }
}

/// The channel contains the main part of the IDE driver.
/// It manages the communication with the drives and the DMA controller.
struct IdeChannel {
    index: u8,                           // Channel number
    interrupt: InterruptVector,          // Interrupt number
    supports_dma: bool,                  // DMA support
    received_interrupt: Arc<AtomicBool>, // Received interrupt flag (shared with interrupt handler)
    last_device_control: u8,             // Saves current state of deviceControlRegister
    interrupts_disabled: bool,           // nIEN (No Interrupt)
    drive_types: [DriveType; 2],         // Initially found drive types
    command: CommandRegisters,           // Command registers (IO ports)
    control: ControlRegisters,           // Control registers (IO ports)
    dma: DmaRegisters,                   // DMA registers (IO ports)
}

impl Default for IdeChannel {
    fn default() -> Self {
        Self::new(0, InterruptVector::PrimaryAta, false, 0, 0, 0)
    }
}

impl IdeChannel {
    fn new(index: u8, interrupt: InterruptVector, supports_dma: bool, command_base_address: u16, control_base_address: u16, dma_base_address: u16) -> Self {
        let command = CommandRegisters::new(command_base_address);
        let control = ControlRegisters::new(control_base_address);
        let dma = DmaRegisters::new(dma_base_address);

        Self {
            index,
            interrupt,
            supports_dma,
            received_interrupt: Arc::new(AtomicBool::new(false)),
            last_device_control: u8::MAX,
            interrupts_disabled: false,
            drive_types: [DriveType::Other, DriveType::Other],
            command,
            control,
            dma,
        }
    }

    /// Wait for a specific status bit to be set in a register
    /// (Typically used to wait for the BUSY bit to be cleared)
    fn wait_status(port: &mut PortReadOnly<u8>, status: Status, timeout: usize) -> bool {
        let end_time = timer().systime_ms() + timeout;
        while timer().systime_ms() < end_time {
            let current_status = Status::from_bits_retain(unsafe { port.read() });
            if current_status.contains(Status::Busy) {
                continue;
            }

            if current_status.contains(Status::Error) {
                error!("Error while waiting for status: 0x{status:02x}");
                return false;
            }

            if current_status.contains(status) {
                return true;
            }
        }

        // Timeout occurred
        // Do not log an error, as this may be normal behavior (e.g. in 'determine_ata_sector_size()')
        false
    }

    /// Wait for the BUSY bit to be cleared
    fn wait_busy(&mut self, timeout: usize) -> bool {
        Self::wait_status(&mut self.command.status, Status::None, timeout)
    }

    fn select_drive(&mut self, drive: u8, prepare_lba: bool, lba_head: u8) -> bool {
        // Check if the drive is already selected (We still need to execute the select operation, if an LBA access is prepared)
        if !prepare_lba && self.last_device_control != u8::MAX && (self.last_device_control >> 4 & 0x01) == drive {
            return true;
        }

        // Prepare selector byte
        let selector = 0xa0 | (prepare_lba as u8) << 6 | drive << 4 | lba_head;
        if selector == self.last_device_control {
            return true;
        }

        if !self.wait_busy(WAIT_ON_STATUS_TIMEOUT) {
            error!("Failed to select drive [{}] on channel [{}]", drive, self.index);
            return false;
        }

        // Select drive and wait 400 ns for the controller to process the command
        unsafe { self.command.drive_head.write(selector) };
        scheduler().sleep(1);

        // Wait for the BUSY bit to be cleared
        if !self.wait_busy(WAIT_ON_STATUS_TIMEOUT) {
            error!("Failed to select drive [{}] on channel [{}]", drive, self.index);
            return false;
        }

        self.last_device_control = selector;
        true
    }

    fn reset_drive(&mut self, drive: u8) -> bool {
        // Select drive
        if !self.select_drive(drive, false, 0) {
            self.drive_types[drive as usize] = DriveType::Other;
            return false;
        }

        // Check drive presence
        let status = unsafe { self.control.alternate_status.read() };
        if status == 0 {
            self.drive_types[drive as usize] = DriveType::Other;
            return false;
        }

        // Set software reset bit and give the device 5 ms to reset
        unsafe { self.control.device_control.write(0x04) };
        scheduler().sleep(5);

        // Clear software reset bit and disable interrupts
        unsafe { self.control.device_control.write(0x02) };
        self.interrupts_disabled = true;

        if !self.wait_busy(WAIT_ON_STATUS_TIMEOUT) {
            error!("Failed to reset drive [{}] on channel [{}]", drive, self.index);
            self.drive_types[drive as usize] = DriveType::Other;
            return false;
        }

        // Check error register
        let error = unsafe { self.command.error.read() };
        if error != 0x00 && error != 0x01 {
            error!("Failed to reset drive [{}] on channel [{}] (Error: 0x{:02x})", drive, self.index, error);
            self.drive_types[drive as usize] = DriveType::Other;
            return false;
        }

        let sector_count = unsafe { self.command.sector_count.read() };
        let sector_number = unsafe { self.command.sector_number.read() };
        if sector_count != 0x01 || sector_number != 0x01 {
            error!(
                "Failed to reset drive [{}] on channel [{}] (Got unexpected values from sector registers)",
                drive, self.index
            );
            self.drive_types[drive as usize] = DriveType::Other;
            return false;
        }

        let cylinder_low = unsafe { self.command.cylinder_low.read() };
        let cylinder_high = unsafe { self.command.cylinder_high.read() };

        if (cylinder_low == ATAPI_CYLINDER_LOW_V1 && cylinder_high == ATAPI_CYLINDER_HIGH_V1)
            || (cylinder_low == ATAPI_CYLINDER_LOW_V2 && cylinder_high == ATAPI_CYLINDER_HIGH_V2)
        {
            self.drive_types[drive as usize] = DriveType::Atapi;
            return true;
        }

        if cylinder_low == 0x00 && cylinder_high == 0x00 {
            self.drive_types[drive as usize] = DriveType::Ata;
            return true;
        }

        error!(
            "Failed to reset drive [{}] on channel [{}] (Got unexpected values from cylinder registers)",
            drive, self.index
        );
        false
    }

    fn identify_drive(&mut self, drive: u8) -> Option<DriveInfo> {
        // Check if drive type is valid
        let drive_type = self.drive_types[drive as usize];
        if drive_type != DriveType::Ata && drive_type != DriveType::Atapi {
            return None;
        }

        // Disable interrupts
        unsafe { self.control.device_control.write(0x02) };
        self.interrupts_disabled = true;

        // Select drive
        if !self.select_drive(drive, false, 0) {
            return None;
        }

        let mut info = DriveInfo::default();
        let mut buffer: [u16; 256] = [0; 256];
        let identify_command = if drive_type == DriveType::Ata {
            Command::IdentifyAtaDrive
        } else {
            Command::IdentifyAtapiDrive
        };

        unsafe { self.command.command.write(identify_command as u8) };
        scheduler().sleep(1);

        if !Self::wait_status(&mut self.control.alternate_status, Status::DataRequest, WAIT_ON_STATUS_TIMEOUT) {
            error!("Failed to identify drive [{}] on channel [{}]", drive, self.index);
            return None;
        }

        for item in &mut buffer {
            *item = unsafe { self.command.data.read() };
        }

        info.typ = drive_type;
        info.channel = self.index;
        info.drive = drive;
        info.signature = buffer[IdentifyFieldOffset::DeviceType as usize];
        info.cylinders = buffer[IdentifyFieldOffset::Cylinders as usize];
        info.heads = buffer[IdentifyFieldOffset::Heads as usize];
        info.sectors_per_track = buffer[IdentifyFieldOffset::Sectors as usize];
        info.capabilities = buffer[IdentifyFieldOffset::Capabilities as usize];
        info.major_version = buffer[IdentifyFieldOffset::MajorVersion as usize];
        info.minor_version = buffer[IdentifyFieldOffset::MinorVersion as usize];
        info.max_sectors_lba28 = (buffer[IdentifyFieldOffset::MaxLba as usize] as u32) | (buffer[IdentifyFieldOffset::MaxLba as usize + 1] as u32) << 16;
        info.max_sectors_lba48 = (buffer[IdentifyFieldOffset::MaxLba as usize] as u32) | (buffer[IdentifyFieldOffset::MaxLba as usize + 1] as u32) << 16;
        info.multiword_dma = (buffer[IdentifyFieldOffset::DmaMulti as usize] >> 8) as u8;
        info.ultra_dma = (buffer[IdentifyFieldOffset::UdmaModes as usize] >> 8) as u8;
        for j in 0..COMMAND_SET_WORD_COUNT {
            info.command_sets[j] = buffer[IdentifyFieldOffset::CommandSets as usize + j];
        }

        info.addressing = if info.command_sets[1] & 0x400 != 0 {
            AddressType::Lba48
        } else if info.capabilities & 0x200 != 0 {
            AddressType::Lba28
        } else {
            AddressType::Chs
        };

        IdeController::copy_byte_swapped_string(&buffer[(IdentifyFieldOffset::Model as usize)..], &mut info.model);
        IdeController::copy_byte_swapped_string(&buffer[(IdentifyFieldOffset::Serial as usize)..], &mut info.serial);
        IdeController::copy_byte_swapped_string(&buffer[(IdentifyFieldOffset::Firmware as usize)..], &mut info.firmware);

        info.sector_size = self.determine_ata_sector_size(&info);
        Some(info)
    }

    fn determine_ata_sector_size(&mut self, info: &DriveInfo) -> u16 {
        // Prepare reading the first sector
        self.prepare_ata_io(info, 0, 1);
        unsafe { self.command.command.write(Command::ReadPioLba28 as u8) };

        let mut timeout = WAIT_ON_STATUS_TIMEOUT;
        let mut sector_size: u16 = 0;

        // Read 256 bytes in each iteration until a timeout occurs
        while Self::wait_status(&mut self.control.alternate_status, Status::DataRequest, timeout) {
            for _ in 0..128 {
                unsafe {
                    self.command.data.read();
                }
            }

            sector_size += 256;
            timeout = 0xff; // After the first iteration, we can use a shorter timeout
        }

        sector_size
    }

    fn prepare_ata_io(&mut self, info: &DriveInfo, sector: u64, count: u16) {
        match info.addressing {
            AddressType::Chs => {
                // Convert LBA address to old CHS format
                let (cylinder, head, sector) = block::lba_to_chs(sector, info.heads as u8, info.sectors_per_track as u8);

                unsafe {
                    // Select drive
                    self.select_drive(info.drive, false, head);

                    // Prepare sector registers
                    // NOTE: In CHS addressing mode, the maximum sector count is 255
                    self.command.sector_count.write(count as u8);
                    self.command.sector_number.write(sector);
                    self.command.cylinder_low.write(cylinder as u8);
                    self.command.cylinder_high.write((cylinder >> 8) as u8);
                }
            }
            AddressType::Lba28 => {
                unsafe {
                    // Select drive
                    self.select_drive(info.drive, true, (sector >> 24) as u8);

                    // Prepare sector registers
                    // NOTE: In LBA28 addressing mode, the maximum sector count is 255
                    self.command.sector_count.write(count as u8);
                    self.command.lba_low.write(sector as u8);
                    self.command.lba_mid.write((sector >> 8) as u8);
                    self.command.lba_high.write((sector >> 16) as u8);
                }
            }
            AddressType::Lba48 => {
                unsafe {
                    // Select drive
                    self.select_drive(info.drive, true, 0);

                    // Prepare sector registers (first wave)
                    self.command.sector_count.write((count >> 8) as u8);
                    self.command.lba_low.write((sector >> 24) as u8);
                    self.command.lba_mid.write((sector >> 32) as u8);
                    self.command.lba_high.write((sector >> 40) as u8);

                    // Prepare sector registers (second wave)
                    self.command.sector_count.write(count as u8);
                    self.command.lba_low.write(sector as u8);
                    self.command.lba_mid.write((sector >> 8) as u8);
                    self.command.lba_high.write((sector >> 16) as u8);
                }
            }
        }
    }

    fn perform_ata_pio(&mut self, info: &DriveInfo, mode: TransferMode, sector: u64, count: u16, buffer: &mut [u8]) -> u16 {
        // Prepare I/O operation
        self.prepare_ata_io(info, sector, count);

        // Find the correct command for the operation
        let command = match mode {
            TransferMode::Read => {
                if info.addressing == AddressType::Lba48 {
                    Command::ReadPioLba48
                } else {
                    Command::ReadPioLba28
                }
            }
            TransferMode::Write => {
                if info.addressing == AddressType::Lba48 {
                    Command::WritePioLba48
                } else {
                    Command::WritePioLba28
                }
            }
        };

        // Start the operation by writing the command
        unsafe { self.command.command.write(command as u8) };
        if !Self::wait_status(&mut self.control.alternate_status, Status::DataRequest, WAIT_ON_STATUS_TIMEOUT) {
            error!(
                "Failed to perform PIO {:?} operation on drive [{}] on channel [{}]: Data request not answered",
                mode, info.drive, self.index
            );
            return 0;
        }

        match mode {
            TransferMode::Read => {
                let mut read = 0;

                while read < count {
                    // Wait for the drive to be ready
                    if read > 0 && !Self::wait_status(&mut self.control.alternate_status, Status::DriveReady, WAIT_ON_STATUS_TIMEOUT) {
                        warn!("Drive did not answer after reading {read}/{count} sectors");
                        break;
                    }

                    // Read sector from the drive and write it to the buffer (one word at a time)
                    for j in 0..(info.sector_size / 2) {
                        let word = unsafe { self.command.data.read() };
                        buffer[(read * info.sector_size + j * 2) as usize] = (word & 0xff) as u8;
                        buffer[(read * info.sector_size + j * 2 + 1) as usize] = ((word >> 8) & 0xff) as u8;
                    }

                    read += 1;
                }

                read
            }
            TransferMode::Write => {
                let mut written = 0;
                while written < count {
                    // Wait for the drive to be ready
                    if written > 0 && !Self::wait_status(&mut self.control.alternate_status, Status::DriveReady, WAIT_ON_STATUS_TIMEOUT) {
                        warn!("Drive did not answer after writing {written}/{count} sectors");
                        break;
                    }

                    // Write sector to the drive (one word at a time)
                    for j in 0..(info.sector_size / 2) {
                        let low_byte = buffer[(written * info.sector_size + j * 2) as usize];
                        let high_byte = buffer[(written * info.sector_size + j * 2 + 1) as usize];
                        unsafe { self.command.data.write((low_byte as u16) | (high_byte as u16) << 8) };
                    }

                    written += 1;
                }

                written
            }
        }
    }

    fn perform_ata_dma(&mut self, info: &DriveInfo, mode: TransferMode, sector: u64, count: u16, buffer: &mut [u8]) -> u16 {
        // Find the correct command for the operation
        let command = match mode {
            TransferMode::Read => {
                if info.addressing == AddressType::Lba48 {
                    Command::ReadDmaLba48
                } else {
                    Command::ReadDmaLba28
                }
            }
            TransferMode::Write => {
                if info.addressing == AddressType::Lba48 {
                    Command::WriteDmaLba48
                } else {
                    Command::WriteDmaLba28
                }
            }
        };

        // Calculate the amount of pages needed for the operation
        let size = count as usize * info.sector_size as usize;
        let pages = size / PAGE_SIZE + if (size % PAGE_SIZE) == 0 { 0 } else { 1 };

        // Each page corresponds to an 8-byte entry in the PRD
        let prd_size = pages * 8;
        let prd_pages = prd_size / PAGE_SIZE + if (prd_size % PAGE_SIZE) == 0 { 0 } else { 1 };

        let prd_frames = memory::vmm::alloc_frames(prd_pages);
        let prd = unsafe { slice::from_raw_parts_mut(prd_frames.start.start_address().as_u64() as *mut PrdEntry, pages) };

        // Allocate memory for the DMA transfer
        let dma_frames = memory::vmm::alloc_frames(pages);
        let dma_buffer = unsafe { slice::from_raw_parts_mut(dma_frames.start.start_address().as_u64() as *mut u8, buffer.len()) };

        // Copy data to the DMA buffer if we are writing
        if mode == TransferMode::Write {
            dma_buffer.copy_from_slice(buffer);
        }

        // Fill PRD
        for i in 0..(pages - 1) {
            prd[i] = PrdEntry {
                base_address: (dma_frames.start.start_address().as_u64() as usize + i * PAGE_SIZE) as u32,
                byte_count: PAGE_SIZE as u16,
                flags: PrdFlags::empty(),
            };
        }

        // Set last PRD entry with the EOT flag
        prd[pages - 1] = PrdEntry {
            base_address: (dma_frames.start.start_address().as_u64() as usize + (pages - 1) * PAGE_SIZE) as u32,
            byte_count: PAGE_SIZE as u16,
            flags: PrdFlags::END_OF_TRANSMISSION,
        };

        // Prepare DMA transfer
        unsafe {
            self.dma.address.write(prd_frames.start.start_address().as_u64() as u32); // Set PRD address
            self.dma
                .command
                .write(if mode == TransferMode::Read { 0x00 } else { DmaCommand::Direction as u8 }); // Set DMA direction
            self.dma.status.write(!(DmaStatus::DmaError | DmaStatus::Interrupt).bits()); // Clear interrupt and error flags
        }

        // Select drive and sector
        self.prepare_ata_io(info, sector, count);

        // Send command to the drive
        unsafe { self.command.command.write(command as u8) };
        if !Self::wait_status(&mut self.control.alternate_status, Status::DataRequest, WAIT_ON_STATUS_TIMEOUT) {
            error!(
                "Failed to perform DMA {:?} operation on drive [{}] on channel [{}]: Data request not answered",
                mode, info.drive, self.index
            );

            memory::vmm::free_frames(dma_frames);
            memory::vmm::free_frames(prd_frames);
            return 0;
        }

        // Start DMA transfer
        self.received_interrupt.store(false, Ordering::Relaxed);
        unsafe { self.dma.command.write(DmaCommand::Enable as u8) };

        // Wait for the DMA transfer to finish
        let timeout = timer().systime_ms() + DMA_TIMEOUT;
        while timer().systime_ms() < timeout {
            if self.received_interrupt.load(Ordering::Relaxed) {
                // Stop DMA transfer and check flags
                unsafe { self.dma.command.write(0x00) };

                let dma_status = DmaStatus::from_bits(unsafe { self.dma.status.read() }).expect("Failed to read DMA status");
                if dma_status.contains(DmaStatus::Interrupt) {
                    // An interrupt has been fired -> Check if bus master is still enabled
                    if dma_status.contains(DmaStatus::BusMasterActive) {
                        // Bus master is still active -> Clear interrupt flag and continue DMA transfer
                        self.received_interrupt.store(false, Ordering::Relaxed);
                        unsafe { self.dma.command.write(DmaCommand::Enable as u8) };
                    } else {
                        // Bus master is not active anymore -> DMA transfer has finished
                        break;
                    }
                }
            }
        }

        if timer().systime_ms() >= timeout {
            error!(
                "Failed to perform DMA {:?} operation on drive [{}] on channel [{}]: Timeout occurred",
                mode, info.drive, self.index
            );

            unsafe {
                memory::vmm::free_frames(dma_frames);
                memory::vmm::free_frames(prd_frames);
            }
            return 0;
        }

        // Copy data from the DMA buffer if we are reading
        if mode == TransferMode::Read {
            buffer.copy_from_slice(dma_buffer);
        }

        // Free allocated page frames
        memory::vmm::free_frames(dma_frames);
        memory::vmm::free_frames(prd_frames);

        count
    }

    fn perform_ata_io(&mut self, info: &DriveInfo, mode: TransferMode, sector: u64, count: usize, buffer: &mut [u8]) -> usize {
        // Select drive
        if !self.select_drive(info.drive, false, 0) {
            return 0;
        }

        // Clear interrupt flag
        self.received_interrupt.store(false, Ordering::Relaxed);

        // Enable interrupts
        if self.interrupts_disabled {
            unsafe { self.control.device_control.write(0x00) };
            self.interrupts_disabled = false;
        }

        if !Self::wait_status(&mut self.control.alternate_status, Status::DriveReady, WAIT_ON_STATUS_TIMEOUT) {
            error!(
                "Failed to perform {:?} operation on drive [{}] on channel [{}]: Drive not ready",
                mode, info.drive, self.index
            );
            return 0;
        }

        let max_sectors = if info.addressing == AddressType::Lba48 { u16::MAX } else { u8::MAX as u16 };
        let mut processed_sectors = 0;
        while processed_sectors < count {
            let remaining = count - processed_sectors;
            let start = sector + processed_sectors as u64;
            let count = if remaining > max_sectors as usize { max_sectors } else { remaining as u16 };
            let buffer_index = processed_sectors * info.sector_size as usize;
            let buffer_end = buffer_index + count as usize * info.sector_size as usize;

            let sectors = if self.supports_dma && info.supports_dma() {
                self.perform_ata_dma(info, mode, start, count, &mut buffer[buffer_index..buffer_end])
            } else {
                self.perform_ata_pio(info, mode, start, count, &mut buffer[buffer_index..buffer_end])
            };

            if sectors == 0 {
                break;
            }

            processed_sectors += sectors as usize;
        }

        processed_sectors
    }
}

/// Each channel has its own interrupt handler with a reference to the channel's `received_interrupt` flag.
/// Once an interrupt occurs, the handler sets the flag to `true`. This usually means, that a DMA transfer has finished.
/// It must be set to `false` manually by the channel before starting a new DMA transfer.
pub struct IdeInterruptHandler {
    received_interrupt: Arc<AtomicBool>,
}

impl IdeInterruptHandler {
    fn new(received_interrupt: Arc<AtomicBool>) -> Self {
        Self { received_interrupt }
    }
}

impl InterruptHandler for IdeInterruptHandler {
    fn trigger(&self) {
        self.received_interrupt.store(true, Ordering::Relaxed);
    }
}
