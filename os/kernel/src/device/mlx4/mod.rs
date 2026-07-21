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

use alloc::vec::Vec;
use cmd::CommandInterface;
use completion_queue::CompletionQueue;
use event_queue::{EventQueue, init_eqs};
use fw::{Capabilities, Hca, MappedFirmwareArea};
use icm::MappedIcmTables;
use log::trace;
use pci_types::{CommandRegister, EndpointHeader};

use rdma::{ibv_access_flags, ibv_device_attr, ibv_port_attr, ibv_qp_attr, ibv_qp_attr_mask, ibv_qp_cap, ibv_qp_type, ibv_recv_wr, ibv_send_wr, ibv_wc};

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
pub fn device_in_range(minor: usize) -> bool {
    (DEVICE_START..=DEVICE_END).contains(&minor)
}

#[inline(always)]
pub fn minor_to_idx(minor: usize) -> usize {
    minor - DEVICE_START
}

static MINOR: AtomicUsize = AtomicUsize::new(DEVICE_START);
static DEV_LIST: Once<Mutex<Vec<ConnectX3Nic>>> = Once::new();

fn next_minor() -> usize {
    MINOR.fetch_add(1, Relaxed)
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
    doorbells: Vec<MappedPages>,
    blueflame: Vec<MappedPages>,
    eqs: Vec<EventQueue>,
    // TODO: find some way to bind this to the relevant EQ
    cqs: Vec<CompletionQueue>,
    qps: Vec<QueuePair>,
    ports: Vec<Port>,
    pub minor: usize,
}

/// Functions that setup the struct.
impl ConnectX3Nic {
    /// Initializes the ConnectX-3 card that is connected as the given PciDevice.
    /// Adds the device to the global List of ConnectX-3 NICs
    ///
    /// # Arguments
    /// * `mlx3_pci_dev`: Contains the pci device information.
    pub fn init(mlx3_pci_dev: &RwLock<EndpointHeader>) -> Result<usize, &'static str> {
        if MINOR.load(Relaxed) > DEVICE_END {
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
        let mut config_regs = utils::pci_map_bar_mem(&mlx3_pci_dev, 0, config_space)?;
        trace!("mlx3 configuration registers: {:?}", config_regs);

        // map the User Access Region
        let user_access_region = utils::pci_map_bar_mem(&mlx3_pci_dev, 2, &config_space)?;
        trace!("mlx3 user access region: {:?}", user_access_region);

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

        // get the doorbells and the BlueFlame section
        let (mut doorbells, blueflame) = capabilities.get_doorbells_and_blueflame(user_access_region)?;
        let eqs = init_eqs(&mut cmd, &mut doorbells, &capabilities, &mut offsets, icm_tables.memory_regions())?;

        hca.config_mad_demux(&mut cmd, &capabilities)?;

        // TODO: Configure Special QPs (QP0, QP1) for SMI and GSI MAD packets
        //       before initializing the ports
        //let _: () = cmd.execute_command(cmd::Opcode::ConfSpecialQp, (), (), offsets.base_qpn)?;

        let ports = hca.init_ports(&mut cmd, &capabilities, offsets.base_qpn)?;

        let minor = next_minor();

        let nic = Self {
            cmd,
            config_regs,
            firmware,
            firmware_area,
            capabilities,
            offsets,
            icm_tables,
            hca,
            doorbells,
            blueflame,
            eqs,
            cqs: Vec::new(),
            qps: Vec::new(),
            ports,
            minor,
        };
        get_dev_list().lock().push(nic);
        Ok(minor)
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

    /// Get statistics about a port.
    ///
    /// This is used by ibv_query_port.
    pub fn query_port(&mut self, port_num: u8) -> Result<ibv_port_attr, &'static str> {
        let port: Option<&mut Port> = self.ports.get_mut(port_num as usize - 1);
        if let Some(port) = port {
            port.query(&mut self.cmd)
        } else {
            Err("port does not exist")
        }
    }

    /// Create a completion queue and return its number.
    ///
    /// This is used by ibv_create_cq.
    pub fn create_cq(&mut self, min_num_entries: i32) -> Result<u32, &'static str> {
        // TODO min_num_entries should be u32
        let mut cq = CompletionQueue::new(
            &mut self.cmd,
            &mut self.capabilities,
            &mut self.offsets,
            self.icm_tables.memory_regions(),
            self.eqs.get(0),
            min_num_entries.try_into().unwrap(),
        )?;
        cq.arm(&mut self.doorbells)?;
        cq.query(&mut self.cmd)?;
        let number = cq.number();
        self.cqs.push(cq);
        Ok(number)
    }

    /// Poll a completion queue and return the number of new completions.
    ///
    /// This is used by ibv_poll_cq.
    pub fn poll_cq(&mut self, number: u32, wc: &mut [ibv_wc]) -> Result<usize, &'static str> {
        let cq = self.cqs.iter_mut().find(|cq| cq.number() == number).ok_or("invalid completion queue number")?;
        cq.poll(&mut self.eqs, &mut self.qps, &mut self.doorbells, wc)
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
    pub fn create_qp(
        &mut self, qp_type: ibv_qp_type::Type, send_cq_number: u32, receive_cq_number: u32, ib_caps: &mut ibv_qp_cap,
    ) -> Result<u32, &'static str> {
        let send_cq = self
            .cqs
            .iter()
            .find(|cq| cq.number() == send_cq_number)
            .ok_or("invalid send completion queue number")?;
        let receive_cq = self
            .cqs
            .iter()
            .find(|cq| cq.number() == receive_cq_number)
            .ok_or("invalid receive completion queue number")?;
        let qp = QueuePair::new(
            &mut self.cmd,
            &mut self.capabilities,
            &mut self.offsets,
            self.icm_tables.memory_regions(),
            qp_type,
            send_cq,
            receive_cq,
            ib_caps,
        )?;
        let number = qp.number();
        self.qps.push(qp);
        Ok(number)
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

    /// Post a work request to receive data.
    ///
    /// This is used by ibv_post_recv.
    pub fn post_receive(&mut self, qp_number: u32, wr: &mut ibv_recv_wr) -> Result<(), &'static str> {
        let qp = self.qps.iter_mut().find(|qp| qp.number() == qp_number).ok_or("invalid queue pair number")?;
        qp.post_receive(wr)
    }

    /// Post a work request to send data.
    ///
    /// This is used by ibv_post_send.
    pub fn post_send(&mut self, qp_number: u32, wr: &mut ibv_send_wr) -> Result<(), &'static str> {
        let qp = self.qps.iter_mut().find(|qp| qp.number() == qp_number).ok_or("invalid queue pair number")?;
        // TODO: check if blue flame is available
        qp.post_send(&mut self.capabilities, &mut self.doorbells, Some(&mut self.blueflame), wr)
    }

    /// Create a memory region and return its index, physical address, lkey and rkey.
    ///
    /// This is used by ibv_reg_mr.
    pub fn create_mr<T>(&mut self, data: &mut [T], access: ibv_access_flags) -> Result<(u32, usize, u32, u32), &'static str> {
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
    next_sqc_doorbell_index: usize,
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
            next_sqc_doorbell_index: 128,
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
    pub(in crate::device::mlx4) fn alloc_scq_db(&mut self) -> usize {
        let res = self.next_sqc_doorbell_index;
        self.next_sqc_doorbell_index += 1;
        res
    }

    /// Allocate a dmpt offset.
    pub(in crate::device::mlx4) fn alloc_dmpt(&mut self) -> usize {
        let res = self.next_dmpt;
        self.next_dmpt += 256;
        res
    }
}
