//! A mlx3 driver for a ConnectX-3 card.
//!
//! This is (very) roughly based on [the Nautilus driver](https://github.com/HExSA-Lab/nautilus/blob/master/src/dev/mlx3_ib.c)
//! and the existing mlx5 driver.

mod cmd;
mod completion_queue;
mod device;
mod event_queue;
mod fw;
mod icm;
mod port;
mod profile;
mod queue_pair;
mod utils;

use alloc::format;
use alloc::vec::Vec;
use cmd::CommandInterface;
use completion_queue::CompletionQueue;
use event_queue::{EventQueue, init_eqs};
use fw::{Capabilities, Hca, MappedFirmwareArea};
use byteorder::BigEndian;
use icm::MappedIcmTables;
use log::{error, trace, warn};
use pci_types::{Bar, CommandRegister, EndpointHeader};
use zerocopy::U32;

use rdma::{ibv_access_flags, ibv_device_attr, ibv_port_attr, ibv_qp_attr, ibv_qp_attr_mask, ibv_qp_type};

use crate::pci_bus;
use port::Port;
use queue_pair::QueuePair;
use spin::{Mutex, Once, RwLock};
use utils::MappedPages;

use device::{Ownership, ResetRegisters};
use fw::Firmware;
use profile::Profile;

use core::sync::atomic::AtomicUsize;
use core::sync::atomic::Ordering::Relaxed;
use x86_64::PhysAddr;
use x86_64::structures::paging::{Page, PageTableFlags, PhysFrame};
use crate::device::mlx4::fw::DoorbellPage;
use crate::device::mlx4::icm::DataMemoryProtectionTable;
use crate::memory::{MemorySpace, PAGE_SIZE};
use crate::memory::vma::VmaType;
use crate::process::process::Process;

/// Vendor ID for Mellanox
pub const MLX_VEND: u16 = 0x15b3;
/// Device ID for the ConnectX-3 NIC
pub const CONNECTX3_DEV: u16 = 0x1003;
pub const NUM_SPECIAL_QP: u32 = 8;

const DEVICE_END: usize = 10;
const DEVICE_START: usize = 1;

#[inline(always)]
pub const fn devices_supported() -> usize {
    (DEVICE_END - DEVICE_START) + 1
}

#[inline(always)]
pub fn device_in_range(handle: usize) -> bool {
    (DEVICE_START..=DEVICE_END).contains(&handle)
}

#[inline(always)]
pub fn device_handle_to_idx(handle: usize) -> usize {
    handle - DEVICE_START
}

static CURRENT_DEVICE_HANDLE: AtomicUsize = AtomicUsize::new(DEVICE_START);
static DEV_LIST: Once<Mutex<Vec<ConnectX3Nic>>> = Once::new();

fn next_device_handle() -> usize {
    CURRENT_DEVICE_HANDLE.fetch_add(1, Relaxed)
}

/// List of all initialized ConnectX-3 NICs
pub fn get_dev_list() -> &'static Mutex<Vec<ConnectX3Nic>> {
    DEV_LIST.call_once(|| Mutex::new(Vec::with_capacity(devices_supported())))
}

/// Struct representing a ConnectX-3 card
pub struct ConnectX3Nic {
    config_regs: MappedPages,
    cmd: CommandInterface,
    firmware: Firmware,
    firmware_area: MappedFirmwareArea,
    capabilities: Capabilities,
    offsets: Offsets,
    icm_tables: MappedIcmTables,
    hca: Hca,
    eqs: Vec<EventQueue>,
    // TODO: find some way to bind this to the relevant EQ
    cqs: Vec<CompletionQueue>,
    qps: Vec<QueuePair>,
    ports: Vec<Port>,
    /// Set once the internal error buffer has been dumped, so it is reported once and not on
    /// every poll afterwards.
    internal_error_reported: bool,
    /// Counts down to the next check of the internal error buffer, see [`Self::drain_events`].
    internal_error_countdown: u32,
    identity_mapped_uar: MappedPages,
    uar_bf_bar: Bar,
    pub handle: usize,
}

/// Functions that setup the struct.
impl ConnectX3Nic {
    /// Initializes the ConnectX-3 card that is connected as the given PciDevice.
    /// Adds the device to the global List of ConnectX-3 NICs
    ///
    /// # Arguments
    /// * `mlx3_pci_dev`: Contains the pci device information.
    pub fn init(mlx3_pci_dev: &RwLock<EndpointHeader>) -> Result<usize, &'static str> {
        if CURRENT_DEVICE_HANDLE.load(Relaxed) > DEVICE_END {
            return Err("Max devices reached !");
        }

        let config_space = pci_bus().config_space();
        let mut mlx3_pci_dev = mlx3_pci_dev.write();

        // Disable Memory Space decoding before reading the BARs. Sizing a BAR
        // transiently writes 0xFFFFFFFF to it, and if memory decoding is enabled
        // the device would briefly decode at that bogus (all-ones) address, which
        // the host (QEMU/KVM) tries to map and rejects.
        mlx3_pci_dev.update_command(config_space, |creg| creg & !CommandRegister::MEMORY_ENABLE);

        // map the Global Device Configuration registers
        let mut config_regs = utils::pci_map_bar_mem(
            mlx3_pci_dev.bar(0, config_space).ok_or("No config regs (BAR 0)")?,
            "mlx4-config-regs"
        );
        trace!("mlx4 configuration registers: {:?}", config_regs);

        // set the memory space bit for this PciDevice
        // set the bus mastering bit for this PciDevice, which allows it to use DMA
        mlx3_pci_dev.update_command(config_space, |creg| creg | CommandRegister::MEMORY_ENABLE | CommandRegister::BUS_MASTER_ENABLE);

        ResetRegisters::reset(&mlx3_pci_dev, &mut config_regs)?;

        // TODO: This shouldn't be necessary.
        // We should be restoring the config space in reset(),
        // but even now these bits are always set.

        mlx3_pci_dev.update_command(config_space, |creg| creg | CommandRegister::MEMORY_ENABLE | CommandRegister::BUS_MASTER_ENABLE);

        // In linux driver ownership is taken before reset
        Ownership::get(&config_regs)?;
        let mut cmd = CommandInterface::new(&mut config_regs)?;
        let firmware = Firmware::query(&mut cmd)?;
        let mut firmware_area = firmware.map_area(&mut cmd)?;
        firmware_area.run(&mut cmd)?;
        let capabilities = firmware_area.repeat_query_capabilities(&mut cmd)?;

        // In the Nautilus driver, some of the port setup already happens here.

        let mut offsets = Offsets::init(&capabilities);
        let mut profile = Profile::new(&capabilities)?;
        let aux_pages = firmware_area.set_icm(&mut cmd, profile.total_size)?;
        let icm_aux_area = firmware_area.map_icm_aux(&mut cmd, aux_pages)?;
        let mut icm_tables = icm_aux_area.map_icm_tables(&mut cmd, &profile, &capabilities)?;
        let hca = profile.init_hca.init_hca(&mut cmd)?;

        // give us the interrupt pin
        hca.query_adapter(&mut cmd)?;

        let uar_bf_bar = mlx3_pci_dev.bar(2, &config_space).ok_or("No UAR (BAR 2)")?;
        trace!("mlx4 User Access Region (UAR) Bar : {:?}", uar_bf_bar);

        // Identity Mapping of the UAR pages. This is only relevant for the kernel, mainly for EQ
        // Doorbells. A UAR page also has to be mapped individually for each process that open this
        // device and should not be shared with different processes
        let mut identity_mapped_uar = utils::pci_map_bar_mem(uar_bf_bar, "mlx4-uar");

        // The first 128 UAR pages are reserved for EQs
        let eq_doorbells: &mut [DoorbellPage] = identity_mapped_uar.as_slice_mut(0, 128)?;
        let eqs = init_eqs(&mut cmd, eq_doorbells, &capabilities, &mut offsets, icm_tables.memory_regions())?;

        hca.config_mad_demux(&mut cmd, &capabilities)?;

        // TODO: Configure Special QPs (QP0, QP1) for SMI and GSI MAD packets
        //       before initializing the ports
        //let _: () = cmd.execute_command(cmd::Opcode::ConfSpecialQp, (), (), offsets.base_qpn)?;

        let ports = hca.init_ports(&mut cmd, &capabilities, offsets.base_qpn)?;

        let handle = next_device_handle();

        let nic = Self {
            cmd,
            config_regs,
            firmware,
            firmware_area,
            capabilities,
            offsets,
            icm_tables,
            hca,
            eqs,
            cqs: Vec::new(),
            qps: Vec::new(),
            ports,
            internal_error_reported: false,
            internal_error_countdown: 0,
            handle,
            uar_bf_bar,
            identity_mapped_uar
        };
        get_dev_list().lock().push(nic);
        Ok(handle)
    }

    /// Get statistics about the device.
    ///
    /// This is used by ibv_query_device.
    pub fn query_device(&mut self) -> Result<ibv_device_attr, &'static str> {
        Ok(ibv_device_attr {
            fw_ver_major: self.firmware.major.get(),
            fw_ver_minor: self.firmware.minor.get(),
            fw_ver_subminor: self.firmware.sub_minor.get(),
            phys_port_cnt: self.ports.len().try_into().unwrap(),
        })
    }

    /// Map a single UAR page (used for ringing SQ/CQ doorbells) into `process`'s address space.
    ///
    /// Shared by QP creation (which also maps a BlueFlame page via [`Self::map_bf`]) and CQ
    /// creation (which only needs the UAR page).
    fn map_uar(&self, uar_idx: usize, process: &Process, label: &str) -> Result<Page, &'static str> {
        if uar_idx < self.capabilities.num_rsvd_uars() as usize {
            return Err("UAR is reserved");
        }

        if uar_idx > self.capabilities.num_uars() {
            return Err("UAR index out of range");
        }

        // TODO: add bitmap to check if uar is already mapped

        let (addr, _size) = self.uar_bf_bar.unwrap_mem();

        let uar_addr = addr + PAGE_SIZE * uar_idx;
        let uar_frame = PhysFrame::from_start_address(PhysAddr::new(uar_addr as u64)).map_err(|_| "UAR page not aligned")?;
        let uar_vma = process.virtual_address_space.alloc_vma(
            None,
            1,
            MemorySpace::User,
            VmaType::DeviceMemory,
            label,
        ).ok_or("Failed to allocate VMA for UAR")?;
        process.virtual_address_space.map_pfr_for_vma(
            &uar_vma,
            PhysFrame::range(uar_frame, uar_frame + 1),
            PageTableFlags::USER_ACCESSIBLE | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE | PageTableFlags::NO_CACHE,
        ).map_err(|_| "Failed to map UAR")?;
        Ok(uar_vma.range.start)
    }

    /// Map the BlueFlame page paired with UAR `uar_idx` into `process`'s address space.
    ///
    /// Used only by QP creation; CQs only need [`Self::map_uar`].
    fn map_bf(&self, uar_idx: usize, process: &Process) -> Result<Page, &'static str> {
        assert!(self.capabilities.bf(), "Blueflame is not supported");
        let (addr, _size) = self.uar_bf_bar.unwrap_mem();
        // The BlueFlame region follows the whole UAR doorbell region (`num_uars()` pages), one
        // BF page per UAR.
        let bf_addr = addr + self.capabilities.num_uars() * PAGE_SIZE + PAGE_SIZE * uar_idx;
        let bf_frame = PhysFrame::from_start_address(PhysAddr::new(bf_addr as u64)).map_err(|_| "BF page not aligned")?;
        let bf_vma = process.virtual_address_space.alloc_vma(
            None,
            1,
            MemorySpace::User,
            VmaType::DeviceMemory,
            format!("bf-{uar_idx}",).as_str()
        ).ok_or("Failed to allocate VMA for BF")?;
        process.virtual_address_space.map_pfr_for_vma(
            &bf_vma,
            PhysFrame::range(bf_frame, bf_frame + 1),
            PageTableFlags::USER_ACCESSIBLE | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE | PageTableFlags::NO_CACHE,
        ).map_err(|_| "Failed to map BF")?;
        Ok(bf_vma.range.start)
    }

    /// Drain the event queue, returning how many events were handled.
    ///
    /// The card reports a port going down, a queue pair failing or its own internal errors as
    /// events. Draining after every verb costs one read of the ring when it is empty and
    /// attributes an event to the operation that caused it. Now that posting and polling both
    /// happen directly from userspace against mapped memory, without going through a syscall,
    /// userspace also drives this directly (rate-limited, see `UverbsCmd::DrainEvents`) so
    /// events still get noticed during an otherwise syscall-free hot loop.
    ///
    /// This is also where the internal error buffer gets checked (an MMIO read, so also
    /// rate-limited, see [`Self::internal_error_countdown`]) since it used to run from the same
    /// place `CompletionQueue::poll` did.
    pub fn drain_events(&mut self) -> usize {
        const INTERNAL_ERROR_CHECK_INTERVAL: u32 = 4096;
        if self.internal_error_countdown == 0 {
            self.internal_error_countdown = INTERNAL_ERROR_CHECK_INTERVAL;
            self.check_internal_error();
        } else {
            self.internal_error_countdown -= 1;
        }
        let Some(eq) = self.eqs.first_mut() else {
            return 0;
        };
        let eq_doorbells: &mut [DoorbellPage] = self.identity_mapped_uar.as_slice_mut(0, 128).unwrap();
        match eq.handle_events(eq_doorbells, false) {
            Ok(handled) => handled,
            Err(e) => {
                warn!("draining the event queue failed: {e}");
                0
            }
        }
    }

    /// Read the card's internal error buffer and report it if it is not empty.
    ///
    /// The card signals a fatal internal error by writing it into a buffer in one of its BARs
    /// and then going quiet. It keeps the link up, but stops serving MADs, so the subnet
    /// manager's `SubnGet(NodeInfo)` times out, it drops the port from the subnet, and the port
    /// is left in the `Initializing` state with nothing in our own log to explain it. The
    /// reference driver maps this buffer and polls it every five seconds
    /// (`mlx4_start_catas_poll`); this is the same check, driven from the paths that run
    /// regularly here.
    ///
    /// Returns whether an error was found.
    pub fn check_internal_error(&mut self) -> bool {
        let (bar, offset, size) = self.firmware.internal_error_buffer();
        if size == 0 {
            // Say so rather than returning quietly: otherwise a wrong QUERY_FW offset and a
            // healthy card look exactly the same in the log.
            self.report_once(format_args!("the firmware reports no internal error buffer, so it cannot be checked"));
            return false;
        }
        // The buffer is in BAR 0 on this card, which is already mapped as the configuration
        // registers. Reaching any other BAR would mean mapping it first.
        if bar != 0 {
            self.report_once(format_args!("internal error buffer is in BAR {bar}, which is not mapped"));
            return false;
        }
        // `MappedPages` is `Copy`, so take one to read through while `self` stays available for
        // the reporting below.
        let config_regs = self.config_regs;
        let words: &[U32<BigEndian>] = match config_regs.as_slice(offset, size) {
            Ok(words) => words,
            Err(e) => {
                self.report_once(format_args!("cannot read the internal error buffer at {offset:#x} ({size} words): {e}"));
                return false;
            }
        };
        if words.iter().all(|word| word.get() == 0) {
            return false;
        }
        if !self.internal_error_reported {
            self.internal_error_reported = true;
            error!("the card reported an internal error:");
            for (i, word) in words.iter().enumerate() {
                error!("  internal error buffer[{:02}] = {:#010x}", i, word.get());
            }
        }
        true
    }

    /// Log a problem with the internal error buffer itself, but only the first time.
    fn report_once(&mut self, message: core::fmt::Arguments) {
        if !self.internal_error_reported {
            self.internal_error_reported = true;
            warn!("{}", message);
        }
    }

    /// Get statistics about a port.
    ///
    /// This is used by ibv_query_port.
    pub fn query_port(&mut self, port_num: u8) -> Result<ibv_port_attr, &'static str> {
        // Cheap enough to do on every query, and this is the call that notices a port dropping
        // back to `Initializing` — the two belong in the same log.
        self.check_internal_error();
        let port: Option<&mut Port> = self.ports.get_mut(port_num as usize - 1);
        if let Some(port) = port {
            port.query(&mut self.cmd)
        } else {
            Err("port does not exist")
        }
    }

    /// Create a completion queue and return its number, plus the UAR doorbell page mapped into
    /// the calling process.
    ///
    /// This is used by ibv_create_cq. `buffer` and `doorbell_ptr` are userspace-owned and
    /// -mapped; polling, CQE parsing and arming happen entirely in userspace against them (the
    /// latter through the returned UAR page), so from here on the kernel only needs the buffer
    /// for building its MTT.
    pub fn create_cq(&mut self, min_num_entries: i32, buffer: *const u8, doorbell_ptr: *const u64) -> Result<(u32, *mut u8), &'static str> {
        // TODO min_num_entries should be u32
        let mut cq = CompletionQueue::new(self, min_num_entries.try_into().unwrap(), buffer, doorbell_ptr)?;
        cq.query(&mut self.cmd)?;
        let number = cq.number();
        let doorbell_page = cq.uar_page_ptr();
        self.cqs.push(cq);
        Ok((number, doorbell_page))
    }

    /// Destroy a completion queue.
    pub fn destroy_cq(&mut self, number: u32) -> Result<(), &'static str> {
        let (index, _) = self
            .cqs
            .iter()
            .enumerate()
            .find(|(_, cq)| cq.number() == number)
            .ok_or("completion queue not found")?;
        let cq = self.cqs.remove(index);
        cq.destroy(&mut self.cmd)?;
        Ok(())
    }

    /// Create a queue pair and return its number.
    ///
    /// This is used by ibv_create_qp.
    /// Create a queue pair and return its number, plus the UAR and BlueFlame pages mapped into
    /// the calling process.
    pub fn create_qp(&mut self,
                     qp_type: ibv_qp_type::Type,
                     send_cq_number: u32,
                     receive_cq_number: u32,
                     buffer: *const u8,
                     doorbell_ptr: *const u32,
                     log_sq_bb_count: u8,
                     log_sq_stride: u8,
                     log_rq_wqe_count: u8,
                     log_rq_stride: u8,
    ) -> Result<(u32, *mut u8, *mut u8), &'static str> {
        let qp = QueuePair::new(
            self,
            qp_type,
            send_cq_number,
            receive_cq_number,
            buffer,
            doorbell_ptr,
            log_sq_bb_count,
            log_sq_stride,
            log_rq_wqe_count,
            log_rq_stride,
        )?;
        let number = qp.number();
        let doorbell_page = qp.uar_page_ptr();
        let blueflame_page = qp.bf_page_ptr();
        self.qps.push(qp);
        Ok((number, doorbell_page, blueflame_page))
    }

    /// Modify a queue pair.
    ///
    /// This is used by ibv_modify_qp.
    pub fn modify_qp(&mut self, number: u32, attr: &ibv_qp_attr, attr_mask: ibv_qp_attr_mask) -> Result<(), &'static str> {
        let qp = self.qps.iter_mut().find(|qp| qp.number() == number).ok_or("invalid queue pair number")?;
        qp.modify(&mut self.cmd, &mut self.capabilities, attr, attr_mask)
    }

    /// Destroy a queue pair.
    pub fn destroy_qp(&mut self, number: u32) -> Result<(), &'static str> {
        let (index, _) = self
            .qps
            .iter()
            .enumerate()
            .find(|(_, qp)| qp.number() == number)
            .ok_or("queue pair not found")?;
        let qp = self.qps.remove(index);
        qp.destroy(&mut self.cmd, &mut self.capabilities)?;
        Ok(())
    }

    /// Create a memory region and return its index, physical address, lkey and rkey.
    ///
    /// This is used by ibv_reg_mr.
    pub fn create_mr<T>(&mut self, data: &mut [T], access: ibv_access_flags) -> Result<DataMemoryProtectionTable, &'static str> {
        // TODO: this fails for large memory regions (>= 64 MB)
        self.icm_tables.memory_regions().alloc_dmpt(
            &mut self.cmd,
            &mut self.capabilities,
            &mut self.offsets,
            data,
            None,
            access,
        )
    }

    /// Destroy a memory region.
    pub fn destroy_mr(&mut self, index: u32) -> Result<(), &'static str> {
        self.icm_tables.memory_regions().destroy(&mut self.cmd, index)
    }
}

impl Drop for ConnectX3Nic {
    fn drop(&mut self) {
        self.icm_tables.memory_regions().destroy_all(&mut self.cmd).unwrap();
        while let Some(qp) = self.qps.pop() {
            qp.destroy(&mut self.cmd, &mut self.capabilities).unwrap()
        }
        while let Some(cq) = self.cqs.pop() {
            cq.destroy(&mut self.cmd).unwrap()
        }
        while let Some(port) = self.ports.pop() {
            port.close(&mut self.cmd).unwrap()
        }
        while let Some(eq) = self.eqs.pop() {
            eq.destroy(&mut self.cmd).unwrap()
        }
        self.hca.close(&mut self.cmd).unwrap();
        self.icm_tables.unmap(&mut self.cmd).unwrap();
        self.firmware_area.unmap(&mut self.cmd).unwrap();
    }
}

struct Offsets {
    base_qpn: u32,
    next_cqn: usize,
    next_qpn: usize,
    next_dmpt: usize,
    next_eqn: usize,
    next_uar_index: usize,
    // TODO: EventQueue does not seem to need this.
    // Should it use this to be more similar to QueuePair?
    _next_eq_doorbell_index: usize,
}

impl Offsets {
    /// Initialize the queue offsets.
    pub(in crate::device::mlx4) fn init(caps: &Capabilities) -> Self {
        let end_reserved_cpn: u32 = 1 << caps.log2_rsvd_cqs();
        // Reserve numbers for special qp. Base_qpn must be naturally aliged
        let base_qpn = end_reserved_cpn.next_multiple_of(NUM_SPECIAL_QP);
        Self {
            base_qpn,
            // This should return the first non reserved cq, qp, eq number.
            next_cqn: (base_qpn + NUM_SPECIAL_QP) as usize,
            next_qpn: 1 << caps.log2_rsvd_qps(),
            next_dmpt: 1 << caps.log2_rsvd_mrws(),
            next_eqn: caps.num_rsvd_eqs().into(),
            // For SQ and CQ Uar Doorbell index starts from 128
            next_uar_index: 128,
            // Each UAR has 4 EQ doorbells; so if a UAR is reserved,
            // then we can't use any EQs whose doorbell falls on that page,
            // even if the EQ itself isn't reserved.
            _next_eq_doorbell_index: caps.num_rsvd_eqs() as usize / 4,
        }
    }

    /// Allocate an event queue number.
    pub(in crate::device::mlx4) fn alloc_eqn(&mut self) -> usize {
        let res = self.next_eqn;
        self.next_eqn += 1;
        res
    }

    /// Allocate a completion queue number.
    pub(in crate::device::mlx4) fn alloc_cqn(&mut self) -> usize {
        let res = self.next_cqn;
        self.next_cqn += 1;
        res
    }

    /// Allocate a queue pair number.
    pub(in crate::device::mlx4) fn alloc_qpn(&mut self) -> usize {
        let res = self.next_qpn;
        self.next_qpn += 1;
        res
    }

    /// Allocate a doorbell for SCQs.
    pub(in crate::device::mlx4) fn alloc_uar(&mut self) -> usize {
        // FIXME: can overflow
        let res = self.next_uar_index;
        self.next_uar_index += 1;
        res
    }

    /// Allocate an entry in the data memory protection table.
    ///
    /// This is an *index* into that table, which is why it starts above the entries the firmware
    /// reserved for itself and counts up by one. The memory key the application gets is derived
    /// from it (`DmptEntry::key`), not the other way around.
    /// TODO: add mechanism to free dmpt, e.g. a bit map
    pub(in crate::device::mlx4) fn alloc_dmpt(&mut self) -> usize {
        let res = self.next_dmpt;
        self.next_dmpt += 1;
        res
    }
}
