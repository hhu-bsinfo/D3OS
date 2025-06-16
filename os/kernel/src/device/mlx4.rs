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

use log::trace;
use alloc::vec::Vec;
use pci_types::{CommandRegister, EndpointHeader};
use cmd::CommandInterface;
use completion_queue::CompletionQueue;
use event_queue::{init_eqs, EventQueue};
use fw::{Capabilities, Hca, MappedFirmwareArea};
use icm::MappedIcmTables;

use crate::infiniband::ib_core::{ibv_access_flags, ibv_device_attr, ibv_port_attr, ibv_qp_attr, ibv_qp_attr_mask, ibv_qp_cap, ibv_qp_type, ibv_recv_wr, ibv_send_wr, ibv_wc};

use port::Port;
use queue_pair::QueuePair;
use spin::{Once, Mutex, RwLock}; 
use utils::MappedPages;
use crate::pci_bus;

use device::{Ownership, ResetRegisters};
use fw::Firmware;
use profile::Profile;

/// Vendor ID for Mellanox
pub const MLX_VEND: u16 = 0x15b3;
/// Device ID for the ConnectX-3 NIC
pub const CONNECTX3_DEV: u16 = 0x1003;

/// The singleton connectx-3 NIC.
/// TODO: Allow for multiple NICs
static CONNECTX3_NIC: Once<Mutex<ConnectX3Nic>> = Once::new();

/// Returns a reference to the NIC wrapped in a IrqSafeMutex,
/// if it exists and has been initialized.
pub fn get_mlx3_nic() -> Option<&'static Mutex<ConnectX3Nic>> {
    CONNECTX3_NIC.get()
}

/// Struct representing a ConnectX-3 card
pub struct ConnectX3Nic {
    config_regs: MappedPages,
    firmware: Firmware,
    firmware_area: Option<MappedFirmwareArea>,
    capabilities: Option<Capabilities>,
    offsets: Option<Offsets>,
    icm_tables: Option<MappedIcmTables>,
    hca: Option<Hca>,
    doorbells: Vec<MappedPages>,
    blueflame: Vec<MappedPages>,
    eqs: Vec<EventQueue>,
    // TODO: find some way to bind this to the relevant EQ
    cqs: Vec<CompletionQueue>,
    qps: Vec<QueuePair>,
    ports: Vec<Port>,
}

/// Functions that setup the struct.
impl ConnectX3Nic {
    /// Initializes the ConnectX-3 card that is connected as the given PciDevice.
    ///
    /// # Arguments
    /// * `mlx3_pci_dev`: Contains the pci device information.
    pub fn init(mlx3_pci_dev: &RwLock<EndpointHeader>) -> Result<&'static Mutex<ConnectX3Nic>, &'static str> {
        let config_space = pci_bus().config_space();
        // set the memory space bit for this PciDevice
        // set the bus mastering bit for this PciDevice, which allows it to use DMA
        
        let mut mlx3_pci_dev = mlx3_pci_dev.write();

        mlx3_pci_dev.update_command(config_space, |creg| 
            { creg | CommandRegister::MEMORY_ENABLE | CommandRegister::BUS_MASTER_ENABLE });

        // map the Global Device Configuration registers
        let mut config_regs = utils::pci_map_bar_mem(
            &mlx3_pci_dev, 
            0, 
            config_space)?;
    
        trace!("mlx3 configuration registers: {:?}", config_regs);
        // map the User Access Region

        let user_access_region = utils::pci_map_bar_mem(
            &mlx3_pci_dev,
            2, 
            &config_space)?;

        trace!("mlx3 user access region: {:?}", user_access_region);

        ResetRegisters::reset(&mlx3_pci_dev, &mut config_regs)?;

        // TODO: This shouldn't be necessary.
        // We should be restoring the config space in reset(),
        // but even now these bits are always set.

        mlx3_pci_dev.update_command(config_space, |creg| 
            { creg | CommandRegister::MEMORY_ENABLE | CommandRegister::BUS_MASTER_ENABLE });

        Ownership::get(&config_regs)?;
        let mut command_interface = CommandInterface::new(&mut config_regs)?;
        let firmware = Firmware::query(&mut command_interface)?;
        let firmware_area = firmware.map_area(&mut command_interface)?;
        let mut nic = Self {
            config_regs,
            firmware,
            firmware_area: Some(firmware_area),
            capabilities: None,
            offsets: None,
            icm_tables: None,
            hca: None,
            doorbells: Vec::new(),
            blueflame: Vec::new(),
            eqs: Vec::new(),
            cqs: Vec::new(),
            qps: Vec::new(),
            ports: Vec::new(),
        };
        let mut command_interface = CommandInterface::new(&mut nic.config_regs)?;
        let firmware_area = nic.firmware_area.as_mut().unwrap();
        firmware_area.run(&mut command_interface)?;
        nic.capabilities = Some(firmware_area.repeat_query_capabilities(&mut command_interface)?);
        let caps = nic.capabilities.as_ref().unwrap();
        // In the Nautilus driver, some of the port setup already happens here.
        nic.offsets = Some(Offsets::init(caps));
        let offsets = nic.offsets.as_mut().unwrap();
        let mut profile = Profile::new(caps)?;
        let aux_pages = firmware_area.set_icm(&mut command_interface, profile.total_size)?;
        let icm_aux_area = firmware_area.map_icm_aux(&mut command_interface, aux_pages)?;
        nic.icm_tables = Some(icm_aux_area.map_icm_tables(&mut command_interface, &profile, caps)?);
        nic.hca = Some(profile.init_hca.init_hca(&mut command_interface)?);
        let hca = nic.hca.as_ref().unwrap();
        // give us the interrupt pin
        hca.query_adapter(&mut command_interface)?;
        let memory_regions = nic.icm_tables.as_mut().unwrap().memory_regions();
        // get the doorbells and the BlueFlame section
        (nic.doorbells, nic.blueflame) = caps.get_doorbells_and_blueflame(
            user_access_region
        )?;
        nic.eqs = init_eqs(
            &mut command_interface, &mut nic.doorbells, caps, offsets,
            memory_regions,
        )?;
        // In the Nautilus driver, CQs and QPs are already allocated here.
        hca.config_mad_demux(&mut command_interface, &caps)?;
        nic.ports = hca.init_ports(&mut command_interface, &caps)?;

        let nic_ref = CONNECTX3_NIC.call_once(|| Mutex::new(nic));
        Ok(nic_ref)
    }

    /// Get statistics about the device.
    /// 
    /// This is used by ibv_query_device.
    pub fn query_device(&mut self) -> Result<ibv_device_attr, &'static str> {
        Ok(ibv_device_attr {
            fw_ver: self.firmware.version(),
            phys_port_cnt: self.ports.len().try_into().unwrap(),
        })
    }

    /// Get statistics about a port.
    /// 
    /// This is used by ibv_query_port.
    pub fn query_port(&mut self, port_num: u8) -> Result<ibv_port_attr, &'static str> {
        let mut cmd = CommandInterface::new(&mut self.config_regs)?;
        let port: Option<&mut Port> = self.ports.get_mut(port_num as usize - 1);
        if let Some(port) = port {
            port.query(&mut cmd)
        } else {
            Err("port does not exist")
        }
    }

    /// Create a completion queue and return its number.
    /// 
    /// This is used by ibv_create_cq.
    pub fn create_cq(&mut self, min_num_entries: i32) -> Result<u32, &'static str> {
        let memory_regions = self.icm_tables.as_mut().unwrap().memory_regions();
        let mut cmd = CommandInterface::new(&mut self.config_regs)?;
        let mut cq = CompletionQueue::new(
            &mut cmd, self.capabilities.as_ref().unwrap(),
            self.offsets.as_mut().unwrap(), memory_regions,
            self.eqs.get(0), min_num_entries.try_into().unwrap(),
        )?;
        cq.arm(&mut self.doorbells)?;
        cq.query(&mut cmd)?;
        let number = cq.number();
        self.cqs.push(cq);
        Ok(number)
    }

    /// Poll a completion queue and return the number of new completions.
    /// 
    /// This is used by ibv_poll_cq.
    pub fn poll_cq(
        &mut self, number: u32, wc: &mut [ibv_wc],
    ) -> Result<usize, &'static str> {
        let cq = self.cqs.iter_mut()
            .find(|cq| cq.number() == number)
            .ok_or("invalid completion queue number")?;
        cq.poll(&mut self.eqs, &mut self.qps, &mut self.doorbells, wc)
    }

    /// Destroy a completion queue.
    pub fn destroy_cq(&mut self, number: u32) -> Result<(), &'static str> {
        let (index, _) = self.cqs
            .iter()
            .enumerate()
            .find(|(_, cq)| cq.number() == number)
            .ok_or("completion queue not found")?;
        let cq = self.cqs.remove(index);
        let mut cmd = CommandInterface::new(&mut self.config_regs)?;
        cq.destroy(&mut cmd)?;
        Ok(())
    }

    /// Create a queue pair and return its number.
    ///
    /// This is used by ibv_create_qp.
    pub fn create_qp(
        &mut self, qp_type: ibv_qp_type::Type, send_cq_number: u32,
        receive_cq_number: u32, ib_caps: &mut ibv_qp_cap,
    ) -> Result<u32, &'static str> {
        let memory_regions = self.icm_tables.as_mut().unwrap().memory_regions();
        let mut cmd = CommandInterface::new(&mut self.config_regs)?;
        let send_cq = self.cqs
            .iter()
            .find(|cq| cq.number() == send_cq_number)
            .ok_or("invalid send completion queue number")?;
        let receive_cq = self.cqs
            .iter()
            .find(|cq| cq.number() == receive_cq_number)
            .ok_or("invalid receive completion queue number")?;
        let qp = QueuePair::new(
            &mut cmd, self.capabilities.as_ref().unwrap(),
            self.offsets.as_mut().unwrap(), memory_regions, qp_type,
            send_cq, receive_cq, ib_caps,
        )?;
        let number = qp.number();
        self.qps.push(qp);
        Ok(number)
    }

    /// Modify a queue pair.
    /// 
    /// This is used by ibv_modify_qp.
    pub fn modify_qp(
        &mut self, number: u32,
        attr: &ibv_qp_attr, attr_mask: ibv_qp_attr_mask,
    ) -> Result<(), &'static str> {
        let qp = self.qps.iter_mut()
            .find(|qp| qp.number() == number)
            .ok_or("invalid queue pair number")?;
        let mut cmd = CommandInterface::new(&mut self.config_regs)?;
        qp.modify(
            &mut cmd, self.capabilities.as_ref().unwrap(), attr, attr_mask,
        )
    }

    /// Destroy a queue pair.
    pub fn destroy_qp(&mut self, number: u32) -> Result<(), &'static str> {
        let (index, _) = self.qps
            .iter()
            .enumerate()
            .find(|(_, qp)| qp.number() == number)
            .ok_or("queue pair not found")?;
        let qp = self.qps.remove(index);
        let mut cmd = CommandInterface::new(&mut self.config_regs)?;
        qp.destroy(&mut cmd, self.capabilities.as_ref().unwrap())?;
        Ok(())
    }

    /// Post a work request to receive data.
    /// 
    /// This is used by ibv_post_recv.
    pub fn post_receive(
        &mut self, qp_number: u32, wr: &mut ibv_recv_wr
    ) -> Result<(), &'static str> {
        let qp = self.qps.iter_mut()
            .find(|qp| qp.number() == qp_number)
            .ok_or("invalid queue pair number")?;
        qp.post_receive(wr)
    }

    /// Post a work request to send data.
    /// 
    /// This is used by ibv_post_send.
    pub fn post_send(
        &mut self, qp_number: u32, wr: &mut ibv_send_wr
    ) -> Result<(), &'static str> {
        let qp = self.qps.iter_mut()
            .find(|qp| qp.number() == qp_number)
            .ok_or("invalid queue pair number")?;
        // TODO: check if blue flame is available
        qp.post_send(
            self.capabilities.as_ref().unwrap(), &mut self.doorbells,
            Some(&mut self.blueflame), wr,
        )
    }

    /// Create a memory region and return its index, physical address, lkey and rkey.
    /// 
    /// This is used by ibv_reg_mr.
    pub fn create_mr<T>(
        &mut self, data: &mut [T], access: ibv_access_flags,
    ) -> Result<(u32, usize, u32, u32), &'static str> {
        // TODO: this fails for large memory regions (>= 64 MB)
        let memory_regions = self.icm_tables.as_mut().unwrap().memory_regions();
        let mut cmd = CommandInterface::new(&mut self.config_regs)?;
        memory_regions.alloc_dmpt(
            &mut cmd, self.capabilities.as_ref().unwrap(),
            self.offsets.as_mut().unwrap(), data, None, access,
        )
    }

    /// Destroy a memory region.
    pub fn destroy_mr(&mut self, index: u32) -> Result<(), &'static str> {
        let memory_regions = self.icm_tables.as_mut().unwrap().memory_regions();
        let mut cmd = CommandInterface::new(&mut self.config_regs)?;
        memory_regions.destroy(&mut cmd, index)
    }
}

impl Drop for ConnectX3Nic {
    fn drop(&mut self) {
        let mut cmd = CommandInterface::new(&mut self.config_regs)
            .expect("failed to get command interface");
        if let Some(icm_tables) = self.icm_tables.as_mut() {
            icm_tables
                .memory_regions()
                .destroy_all(&mut cmd)
                .unwrap()
        }
        while let Some(qp) = self.qps.pop() {
            qp
                .destroy(&mut cmd, self.capabilities.as_ref().unwrap())
                .unwrap()
        }
        while let Some(cq) = self.cqs.pop() {
            cq
                .destroy(&mut cmd)
                .unwrap()
        }
        while let Some(port) = self.ports.pop() {
            port
                .close(&mut cmd)
                .unwrap()
        }
        while let Some(eq) = self.eqs.pop() {
            eq
                .destroy(&mut cmd)
                .unwrap()
        }
        if let Some(hca) = self.hca.take() {
            hca
                .close(&mut cmd)
                .unwrap()
        }
        if let Some(icm_tables) = self.icm_tables.take() {
            icm_tables
                .unmap(&mut cmd)
                .unwrap()
        }
        if let Some(firmware_area) = self.firmware_area.take() {
            firmware_area
                .unmap(&mut cmd)
                .unwrap()
        }
    }
}

struct Offsets {
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
        Self {
            // This should return the first non reserved cq, qp, eq number.
            next_cqn: 1 << caps.log2_rsvd_cqs(),
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
