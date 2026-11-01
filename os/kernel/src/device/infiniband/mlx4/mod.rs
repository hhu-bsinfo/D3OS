//! A driver for a ConnectX-3 card.
//!
//! This is based on [Theseus' mlx3 driver](https://github.com/YtvwlD/Theseus/tree/mlx3/kernel/mlx3)

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

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::sync::Arc;
use alloc::vec::Vec;
use byteorder::BigEndian;
use cmd::CommandInterface;
use completion_queue::CompletionQueue;
use event_queue::{ClrInt, EventQueue, init_eqs};
use fw::{Capabilities, Hca, MappedFirmwareArea};
use icm::{MappedIcmTables, MemoryRegion};
use log::{error, info, trace, warn};
use pci_types::{CommandRegister, EndpointHeader};
use zerocopy::U32;

use rdma::{AccessFlags, ContextHandle, DeviceAttr, MemoryRegionMetadata, PdHandle, PortAttr, QueuePairAttr, QueuePairAttrMask, QueuePairType};

use crate::pci_bus;
use port::Port;
use queue_pair::QueuePair;
use spin::{Mutex, Once, RwLock};
use utils::MappedPages;

use device::{Ownership, ResetRegisters};
use fw::Firmware;
use profile::Profile;

use crate::device::infiniband::mlx4::fw::DoorbellPage;
use crate::device::infiniband::mlx4::icm::map_icm_tables;
use crate::memory::vma::VmaType;
use crate::memory::{MemorySpace, PAGE_SIZE};
use crate::process::process::Process;
use core::sync::atomic::AtomicUsize;
use core::sync::atomic::Ordering::Relaxed;
use uuid::Uuid;
use x86_64::PhysAddr;
use x86_64::structures::paging::{Page, PageTableFlags, PhysFrame};
use rdma::uverbs_uapi::UserSlice;

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
static DEV_LIST: Once<Mutex<Vec<Mlx4Device>>> = Once::new();

fn next_device_handle() -> usize {
    CURRENT_DEVICE_HANDLE.fetch_add(1, Relaxed)
}

/// List of all initialized ConnectX-3 NICs
pub fn get_dev_list() -> &'static Mutex<Vec<Mlx4Device>> {
    DEV_LIST.call_once(|| Mutex::new(Vec::with_capacity(devices_supported())))
}

/// Struct representing a ConnectX-3 card
pub struct Mlx4Device {
    config_regs: MappedPages,
    cmd: CommandInterface,
    firmware: Firmware,
    firmware_area: MappedFirmwareArea,
    capabilities: Capabilities,
    offsets: Offsets,
    icm_tables: MappedIcmTables,
    hca: Hca,
    /// Every open context, each owning everything created in it.
    contexts: BTreeMap<ContextHandle, UContext>,
    /// Handle for the next context; handles are never reused.
    next_context: u32,
    /// Protection domain numbers in use, across all contexts.
    pd_numbers: BTreeSet<u32>,
    eqs: Vec<Arc<RwLock<EventQueue>>>,
    ports: Vec<Port>,
    /// Set once the internal error buffer has been dumped, so it is reported once and not on
    /// every poll afterwards.
    internal_error_reported: bool,
    uar_list: Vec<UarPage>,
    pub handle: usize,
}

/// Functions that setup the struct.
impl Mlx4Device {
    /// Initializes the ConnectX-3 card that is connected as the given PciDevice.
    /// Adds the device to the global List of ConnectX-3 NICs
    ///
    /// # Arguments
    /// * `mlx4_pci_dev`: Contains the pci device information.
    pub fn init(mlx4_pci_dev: &RwLock<EndpointHeader>) -> Result<usize, &'static str> {
        if CURRENT_DEVICE_HANDLE.load(Relaxed) > DEVICE_END {
            return Err("Max devices reached !");
        }

        let config_space = pci_bus().config_space();
        let mut mlx4_pci_dev = mlx4_pci_dev.write();

        // Disable Memory Space decoding before reading the BARs. Sizing a BAR
        // transiently writes 0xFFFFFFFF to it, and if memory decoding is enabled
        // the device would briefly decode at that bogus (all-ones) address, which
        // the host (QEMU/KVM) tries to map and rejects.
        mlx4_pci_dev.update_command(config_space, |creg| creg & !CommandRegister::MEMORY_ENABLE);

        // map the Global Device Configuration registers
        let mut config_regs =
            utils::pci_map_bar_mem(mlx4_pci_dev.bar(0, config_space).ok_or("No config regs (BAR 0)")?, "mlx4-config-regs").ok_or("failed map BAR 0")?;
        trace!("mlx4 configuration registers: {:?}", config_regs);

        // set the memory space bit for this PciDevice
        // set the bus mastering bit for this PciDevice, which allows it to use DMA
        mlx4_pci_dev.update_command(config_space, |creg| creg | CommandRegister::MEMORY_ENABLE | CommandRegister::BUS_MASTER_ENABLE);

        ResetRegisters::reset(&mlx4_pci_dev, &mut config_regs)?;

        // TODO: This shouldn't be necessary.
        // We should be restoring the config space in reset(),
        // but even now these bits are always set.

        mlx4_pci_dev.update_command(config_space, |creg| creg | CommandRegister::MEMORY_ENABLE | CommandRegister::BUS_MASTER_ENABLE);

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
        firmware_area.map_icm_aux(&mut cmd, aux_pages)?;
        let mut icm_tables = map_icm_tables(&mut cmd, &profile, &capabilities)?;
        let mut hca = profile.init_hca.init_hca(&mut cmd)?;
        let mut eqs = Vec::new();

        // From here on the card holds resources that panic when dropped, so an error has to
        // release them first; otherwise the panic replaces the actual error.
        // give us the interrupt pin
        let adapter = hca
            .query_adapter(&mut cmd)
            .or_else(|e| Self::abort_init(e, &mut cmd, &mut hca, &mut eqs, &mut icm_tables, &mut firmware_area))?;

        let uar_bf_bar = mlx4_pci_dev
            .bar(2, &config_space)
            .ok_or("No UAR (BAR 2)")
            .or_else(|e| Self::abort_init(e, &mut cmd, &mut hca, &mut eqs, &mut icm_tables, &mut firmware_area))?;
        trace!("mlx4 User Access Region (UAR) Bar : {:?}", uar_bf_bar);
        let num_uars = capabilities.num_uars();
        let mut uar_list = Vec::with_capacity(num_uars);
        let first_uar = capabilities.num_rsvd_uars() as usize;
        for index in first_uar..num_uars {
            let (start_addr, _) = uar_bf_bar.unwrap_mem();
            let uar_addr = start_addr + PAGE_SIZE * index;
            let bf_addr = start_addr + PAGE_SIZE * (num_uars + index);
            let doorbell = PhysFrame::from_start_address(PhysAddr::new(uar_addr as u64)).expect("Doorbell page not aligned");
            let blueflame = if capabilities.bf() {
                Some(PhysFrame::from_start_address(PhysAddr::new(bf_addr as u64)).expect("BlueFlame page not aligned"))
            } else {
                None
            };
            uar_list.push(UarPage { index, doorbell, blueflame })
        }

        // Identity Mapping of the UAR pages. This is only relevant for the kernel, mainly for EQ
        // Doorbells. A UAR page also has to be mapped individually for each process that open this
        // device and should not be shared with different processes
        let mut identity_mapped_uar = utils::pci_map_bar_mem(uar_bf_bar, "mlx4-uar").ok_or("Failed to map UAR BAR")?;

        // The clr_int register lives in whichever BAR QUERY_FW reported; on this card that's
        // always one of the two we already have mapped (config regs or UAR).
        let (clr_int_bar, clr_int_offset) = firmware.clr_int();
        let clr_int_regs = match clr_int_bar {
            0 => config_regs,
            2 => identity_mapped_uar,
            _ => Err("legacy interrupt clear register is in an unmapped BAR")
                .or_else(|e| Self::abort_init(e, &mut cmd, &mut hca, &mut eqs, &mut icm_tables, &mut firmware_area))?,
        };
        let clr_int = ClrInt::new(clr_int_regs, clr_int_offset, adapter.inta_pin());

        // The first 128 UAR pages are reserved for EQs
        let eq_doorbells: &mut [DoorbellPage] = identity_mapped_uar
            .as_slice_mut(0, 128)
            .or_else(|e| Self::abort_init(e, &mut cmd, &mut hca, &mut eqs, &mut icm_tables, &mut firmware_area))?;
        eqs = init_eqs(&mut cmd, eq_doorbells, &capabilities, &mut offsets, icm_tables.memory_regions(), clr_int)
            .or_else(|e| Self::abort_init(e, &mut cmd, &mut hca, &mut eqs, &mut icm_tables, &mut firmware_area))?;

        hca.config_mad_demux(&mut cmd, &capabilities)
            .or_else(|e| Self::abort_init(e, &mut cmd, &mut hca, &mut eqs, &mut icm_tables, &mut firmware_area))?;

        // TODO: Configure Special QPs (QP0, QP1) for SMI and GSI MAD packets
        //       before initializing the ports

        let ports = hca
            .init_ports(&mut cmd, &capabilities, offsets.base_qpn)
            .or_else(|e| Self::abort_init(e, &mut cmd, &mut hca, &mut eqs, &mut icm_tables, &mut firmware_area))?;

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
            contexts: BTreeMap::new(),
            next_context: 1,
            pd_numbers: BTreeSet::new(),
            eqs,
            ports,
            internal_error_reported: false,
            handle,
            uar_list,
        };
        get_dev_list().lock().push(nic);
        Ok(handle)
    }

    /// Release what `init` had set up when it fails after INIT_HCA, in the same order as `drop`,
    /// and pass the error on.
    ///
    /// Failures while releasing are only logged: the card is in an unknown state at this point
    /// anyway, and the error that got us here is the one worth returning.
    fn abort_init<T>(
        error: &'static str, cmd: &mut CommandInterface, hca: &mut Hca, eqs: &mut Vec<Arc<RwLock<EventQueue>>>, icm_tables: &mut MappedIcmTables,
        firmware_area: &mut MappedFirmwareArea,
    ) -> Result<T, &'static str> {
        error!("mlx4 initialization failed after INIT_HCA: {error}");
        while let Some(eq) = eqs.pop() {
            if let Err(e) = eq.write().destroy(cmd) {
                warn!("failed to destroy an event queue while cleaning up: {e}");
            }
        }
        if let Err(e) = hca.close(cmd) {
            warn!("failed to close the HCA while cleaning up: {e}");
        }
        if let Err(e) = icm_tables.unmap(cmd) {
            warn!("failed to unmap the ICM tables while cleaning up: {e}");
        }
        if let Err(e) = firmware_area.unmap(cmd) {
            warn!("failed to unmap the firmware area while cleaning up: {e}");
        }
        Err(error)
    }

    /// Open a new context on the device for `process`.
    pub fn open(&mut self, process: &Process) -> Result<&UContext, &'static str> {
        let handle = ContextHandle(self.next_context);
        let next_context = self.next_context.checked_add(1).ok_or("No context handle available")?;
        let uar_page = self.uar_list.pop().ok_or("No UAR page available")?;
        let pages = uar_page.map_doorbell_page(process).and_then(|doorbell| Ok((doorbell, uar_page.map_blueflame_page(process)?)));
        let (doorbell_page, blueflame_page) = match pages {
            Ok(pages) => pages,
            Err(e) => {
                self.uar_list.push(uar_page);
                return Err(e);
            }
        };
        self.next_context = next_context;
        let context = UContext {
            handle,
            owner: process.id(),
            uar_page,
            doorbell_page,
            blueflame_page,
            pds: Vec::new(),
            cqs: Vec::new(),
            qps: Vec::new(),
            mrs: Vec::new(),
        };
        Ok(self.contexts.entry(handle).or_insert(context))
    }

    /// Destroy everything `pid` created and close its contexts.
    ///
    /// Used right before the process is dropped. No thread of it can still be in a verb then,
    /// since each holds a reference to the process, so nothing can open a new context. If the
    /// card refuses to let go of any resource, that context is kept, so its UAR page is never
    /// handed out again, and an error is returned: the card may still access the process's
    /// memory.
    pub fn release(&mut self, pid: Uuid) -> Result<(), &'static str> {
        let handles: Vec<ContextHandle> = self.contexts.iter().filter(|(_, ctx)| ctx.owner == pid).map(|(handle, _)| *handle).collect();
        let mut result = Ok(());
        for handle in handles {
            let Some(mut ctx) = self.contexts.remove(&handle) else {
                continue;
            };
            let counts = (ctx.qps.len(), ctx.mrs.len(), ctx.cqs.len(), ctx.pds.len());
            if let Err(e) = ctx.destroy(&mut self.cmd, &self.capabilities) {
                self.contexts.insert(handle, ctx);
                result = Err(e);
                continue;
            }
            for pd in &ctx.pds {
                self.pd_numbers.remove(&pd.0);
            }
            self.uar_list.push(ctx.uar_page);
            info!(
                "released context {} of process {pid}: {} QPs, {} MRs, {} CQs, {} PDs",
                handle.0, counts.0, counts.1, counts.2, counts.3
            );
        }
        result
    }

    /// Get statistics about the device.
    ///
    /// This is used by ibv_query_device.
    pub fn query_device(&mut self) -> Result<DeviceAttr, &'static str> {
        Ok(DeviceAttr {
            fw_ver_major: self.firmware.major.get(),
            fw_ver_minor: self.firmware.minor.get(),
            fw_ver_subminor: self.firmware.sub_minor.get(),
            phys_port_cnt: self.ports.len().try_into().unwrap(),
        })
    }

    /// Read the card's internal error buffer and report it if it is not empty.
    ///
    /// After a fatal internal error the card stops serving MADs and the port falls back to
    /// `Initializing`. Equivalent to Linux's `mlx4_start_catas_poll`.
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
    pub fn query_port(&mut self, port_num: u8) -> Result<PortAttr, &'static str> {
        // Cheap enough to do on every query, and this is the call that notices a port dropping
        // back to `Initializing` — the two belong in the same log.
        self.check_internal_error();
        let port: Option<&mut Port> = port_num.checked_sub(1)
            .and_then(|i| self.ports.get_mut(i as usize));
        if let Some(port) = port {
            port.query(&mut self.cmd)
        } else {
            Err("port does not exist")
        }
    }

    pub fn alloc_pd(&mut self, process: &Process, context: ContextHandle) -> Result<PdHandle, &'static str> {
        let ctx = context_of(&mut self.contexts, context, process)?;
        // TODO: impl random pd sampling
        let first = self.capabilities.num_rsvd_pds() as u32;
        let count = 1 << self.capabilities.log_max_pd();
        let pd = (first..first + count)
            .find(|pd| !self.pd_numbers.contains(pd))
            .ok_or("No protection domains available")?;
        self.pd_numbers.insert(pd);
        ctx.pds.push(PdHandle(pd));
        Ok(PdHandle(pd))
    }

    pub fn dealloc_pd(&mut self, process: &Process, context: ContextHandle, pd: PdHandle) -> Result<(), &'static str> {
        // todo check if some qp or mr is register with this pd before allowing it to be deallocated
        let ctx = context_of(&mut self.contexts, context, process)?;
        let index = ctx.pds.iter().position(|p| *p == pd).ok_or("PD not found")?;
        ctx.pds.swap_remove(index);
        self.pd_numbers.remove(&pd.0);
        info!("deallocated PD {pd:?}");
        Ok(())
    }

    /// Create a completion queue and return its number
    pub fn create_cq(
        &mut self, process: &Process, context: ContextHandle, num_entries: u32, buffer: *const u8, doorbell_ptr: *const u64,
    ) -> Result<u32, &'static str> {
        let ctx = context_of(&mut self.contexts, context, process)?;
        let uar_index = ctx.uar_index();
        let eq_number = self.eqs.first().map(|eq| eq.read().number());
        let mut cq = CompletionQueue::new(
            &mut self.cmd,
            &self.capabilities,
            &mut self.offsets,
            self.icm_tables.memory_regions(),
            eq_number,
            process,
            num_entries,
            buffer,
            doorbell_ptr,
            uar_index,
        )?;
        // Only for the log, so a failure here must not leave the CQ untracked.
        if let Err(e) = cq.query(&mut self.cmd) {
            warn!("failed to query CQ {}: {e}", cq.number());
        }
        let number = cq.number();
        ctx.cqs.push(cq);
        Ok(number)
    }

    /// Destroy a completion queue.
    pub fn destroy_cq(&mut self, process: &Process, context: ContextHandle, number: u32) -> Result<(), &'static str> {
        let ctx = context_of(&mut self.contexts, context, process)?;
        let index = ctx.cqs.iter().position(|cq| cq.number() == number).ok_or("completion queue not found")?;
        ctx.cqs[index].destroy(&mut self.cmd)?;
        ctx.cqs.remove(index);
        Ok(())
    }

    /// Create a queue pair and return its number
    pub fn create_qp(
        &mut self, process: &Process, context: ContextHandle, pd: PdHandle, qp_type: QueuePairType, send_cq_number: u32, receive_cq_number: u32,
        buffer: *const u8, doorbell_ptr: *const u32, log_sq_bb_count: u8, log_sq_stride: u8, log_rq_wqe_count: u8, log_rq_stride: u8,
    ) -> Result<u32, &'static str> {
        let ctx = context_of(&mut self.contexts, context, process)?;
        let uar_index = ctx.uar_index();
        ctx.validate_pd(pd)?;
        if !ctx.cqs.iter().any(|cq| cq.number() == send_cq_number) {
            return Err("send completion queue not found");
        }
        if !ctx.cqs.iter().any(|cq| cq.number() == receive_cq_number) {
            return Err("receive completion queue not found");
        }

        let qp = QueuePair::new(
            &self.capabilities,
            &mut self.offsets,
            self.icm_tables.memory_regions(),
            process,
            qp_type,
            pd,
            send_cq_number,
            receive_cq_number,
            buffer,
            doorbell_ptr,
            uar_index,
            log_sq_bb_count,
            log_sq_stride,
            log_rq_wqe_count,
            log_rq_stride,
        )?;
        let number = qp.number();
        ctx.qps.push(qp);
        Ok(number)
    }

    /// Modify a queue pair.
    ///
    /// This is used by ibv_modify_qp.
    pub fn modify_qp(&mut self, process: &Process, context: ContextHandle, number: u32, attr: &QueuePairAttr, attr_mask: QueuePairAttrMask) -> Result<(), &'static str> {
        let ctx = context_of(&mut self.contexts, context, process)?;
        let qp = ctx.qps.iter_mut().find(|qp| qp.number() == number).ok_or("queue pair not found")?;
        qp.modify(&mut self.cmd, &self.capabilities, attr, attr_mask)
    }

    /// Destroy a queue pair.
    pub fn destroy_qp(&mut self, process: &Process, context: ContextHandle, number: u32) -> Result<(), &'static str> {
        let ctx = context_of(&mut self.contexts, context, process)?;
        let index = ctx.qps.iter().position(|qp| qp.number() == number).ok_or("queue pair not found")?;
        ctx.qps[index].destroy(&mut self.cmd, &self.capabilities)?;
        ctx.qps.swap_remove(index);
        Ok(())
    }

    /// Create a memory region and return its index, physical address, lkey and rkey.
    ///
    /// This is used by ibv_reg_mr.
    pub fn create_mr(&mut self, process: &Process, context: ContextHandle, pd: PdHandle, data: UserSlice, access: AccessFlags) -> Result<MemoryRegionMetadata, &'static str> {
        let ctx = context_of(&mut self.contexts, context, process)?;
        ctx.validate_pd(pd)?;
        let (mr, metadata) = self.icm_tables.memory_regions().alloc_dmpt(
            &mut self.cmd,
            &self.capabilities,
            &mut self.offsets,
            process,
            pd,
            data,
            None,
            access,
        )?;
        ctx.mrs.push(mr);
        Ok(metadata)
    }

    /// Destroy a memory region.
    pub fn destroy_mr(&mut self, process: &Process, context: ContextHandle, index: u32) -> Result<(), &'static str> {
        let ctx = context_of(&mut self.contexts, context, process)?;
        let position = ctx.mrs.iter().position(|mr| mr.index() == Some(index)).ok_or("dmpt entry not found")?;
        ctx.mrs[position].destroy(&mut self.cmd)?;
        ctx.mrs.swap_remove(position);
        Ok(())
    }
}

impl Drop for Mlx4Device {
    fn drop(&mut self) {
        let pids: Vec<Uuid> = self.contexts.values().map(|ctx| ctx.owner).collect();
        for pid in pids {
            self.release(pid).unwrap();
        }
        while let Some(port) = self.ports.pop() {
            port.close(&mut self.cmd).unwrap()
        }
        while let Some(eq) = self.eqs.pop() {
            eq.write().destroy(&mut self.cmd).unwrap()
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
    // TODO: EventQueue does not seem to need this.
    // Should it use this to be more similar to QueuePair?
    _next_eq_doorbell_index: usize,
}

impl Offsets {
    /// Initialize the queue offsets.
    pub(in crate::device::infiniband::mlx4) fn init(caps: &Capabilities) -> Self {
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
            // Each UAR has 4 EQ doorbells; so if a UAR is reserved,
            // then we can't use any EQs whose doorbell falls on that page,
            // even if the EQ itself isn't reserved.
            _next_eq_doorbell_index: caps.num_rsvd_eqs() as usize / 4,
        }
    }

    /// Allocate an event queue number.
    pub(in crate::device::infiniband::mlx4) fn alloc_eqn(&mut self) -> usize {
        let res = self.next_eqn;
        self.next_eqn += 1;
        res
    }

    /// Allocate a completion queue number.
    pub(in crate::device::infiniband::mlx4) fn alloc_cqn(&mut self) -> usize {
        let res = self.next_cqn;
        self.next_cqn += 1;
        res
    }

    /// Allocate a queue pair number.
    pub(in crate::device::infiniband::mlx4) fn alloc_qpn(&mut self) -> usize {
        let res = self.next_qpn;
        self.next_qpn += 1;
        res
    }

    /// Allocate an entry in the data memory protection table.
    ///
    /// This is an *index* into that table, which is why it starts above the entries the firmware
    /// reserved for itself and counts up by one. The memory key the application gets is derived
    /// from it (`DmptEntry::key`), not the other way around.
    /// TODO: add mechanism to free dmpt, e.g. a bit map
    pub(in crate::device::infiniband::mlx4) fn alloc_dmpt(&mut self) -> usize {
        let res = self.next_dmpt;
        self.next_dmpt += 1;
        res
    }
}

/// Look up the context `handle`, provided `process` opened it.
///
/// A context of another process is reported as not found, so its handle reveals nothing.
fn context_of<'a>(
    contexts: &'a mut BTreeMap<ContextHandle, UContext>, handle: ContextHandle, process: &Process,
) -> Result<&'a mut UContext, &'static str> {
    contexts.get_mut(&handle).filter(|ctx| ctx.owner == process.id()).ok_or("context not found")
}

/// One opened instance of a device and everything created in it, like Linux's `ib_ucontext`.
///
/// Every verb names a context and looks its objects up in that context only, so a handle from
/// another context is simply not found, and closing the context destroys everything in it.
pub struct UContext {
    handle: ContextHandle,
    /// The process that opened this context, i.e. the only one allowed to use it.
    owner: Uuid,
    uar_page: UarPage,
    doorbell_page: Page,
    blueflame_page: Page,
    pds: Vec<PdHandle>,
    cqs: Vec<CompletionQueue>,
    qps: Vec<QueuePair>,
    mrs: Vec<MemoryRegion>,
}

impl UContext {
    pub fn handle(&self) -> ContextHandle {
        self.handle
    }

    fn uar_index(&self) -> u32 {
        self.uar_page.index() as u32
    }

    /// Where the doorbell page is mapped in the process.
    pub fn doorbell_page(&self) -> Page {
        self.doorbell_page
    }

    /// Where the BlueFlame page is mapped in the process.
    pub fn blueflame_page(&self) -> Page {
        self.blueflame_page
    }

    fn validate_pd(&self, pd: PdHandle) -> Result<(), &'static str> {
        if self.pds.contains(&pd) { Ok(()) } else { Err("PD not found") }
    }

    /// Destroy every QP, MR and CQ, in the same order as Linux's `ib_uverbs_cleanup_ucontext`:
    /// resetting the QPs first stops all work queue DMA, and the CQs go last because the QPs
    /// complete to them.
    ///
    /// Keeps going past failures, so as much as possible is released, and keeps whatever could
    /// not be destroyed.
    fn destroy(&mut self, cmd: &mut CommandInterface, caps: &Capabilities) -> Result<(), &'static str> {
        let mut ok = true;
        self.qps.retain_mut(|qp| match qp.destroy(cmd, caps) {
            Ok(()) => false,
            Err(e) => {
                error!("failed to destroy QP {}: {e}", qp.number());
                ok = false;
                true
            }
        });
        self.mrs.retain_mut(|mr| match mr.destroy(cmd) {
            Ok(()) => false,
            Err(e) => {
                error!("failed to destroy MR {:?}: {e}", mr.index());
                ok = false;
                true
            }
        });
        self.cqs.retain_mut(|cq| match cq.destroy(cmd) {
            Ok(()) => false,
            Err(e) => {
                error!("failed to destroy CQ {}: {e}", cq.number());
                ok = false;
                true
            }
        });
        if ok { Ok(()) } else { Err("failed to destroy all resources of the context") }
    }
}

pub struct UarPage {
    index: usize,
    doorbell: PhysFrame,
    blueflame: Option<PhysFrame>,
}

impl UarPage {
    pub fn index(&self) -> usize {
        self.index
    }

    /// Map a single Doorbell page (used for ringing SQ/CQ doorbells) into `process`'s address space.
    ///
    /// Shared by QP creation (which also maps a BlueFlame page via [`Self::map_blueflame_page`]) and CQ
    /// creation (which only needs the UAR page).
    pub fn map_doorbell_page(&self, process: &Process) -> Result<Page, &'static str> {
        let uar_vma = process
            .virtual_address_space
            .alloc_vma(None, 1, MemorySpace::User, VmaType::DeviceMemory, format!("db-{}", self.index).as_str())
            .ok_or("Failed to allocate VMA for UAR")?;
        process
            .virtual_address_space
            .map_pfr_for_vma(
                &uar_vma,
                PhysFrame::range(self.doorbell, self.doorbell + 1),
                PageTableFlags::USER_ACCESSIBLE | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE | PageTableFlags::NO_CACHE,
            )
            .map_err(|_| "Failed to map UAR")?;
        Ok(uar_vma.range.start)
    }

    /// Map the BlueFlame page paired with the context into `process`'s address space.
    ///
    /// Used only by QP creation; CQs only need [`Self::map_doorbell_page`].
    pub fn map_blueflame_page(&self, process: &Process) -> Result<Page, &'static str> {
        let bf_frame = self.blueflame.ok_or("No BlueFlame Page present")?;
        let bf_vma = process
            .virtual_address_space
            .alloc_vma(None, 1, MemorySpace::User, VmaType::DeviceMemory, format!("bf-{}", self.index).as_str())
            .ok_or("Failed to allocate VMA for BF")?;
        process
            .virtual_address_space
            .map_pfr_for_vma(
                &bf_vma,
                PhysFrame::range(bf_frame, bf_frame + 1),
                PageTableFlags::USER_ACCESSIBLE | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE | PageTableFlags::NO_CACHE,
            )
            .map_err(|_| "Failed to map BF")?;
        Ok(bf_vma.range.start)
    }
}
