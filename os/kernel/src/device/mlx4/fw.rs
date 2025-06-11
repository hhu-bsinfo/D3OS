//! This module contains functionality to interact with the firmware.

use core::{mem::size_of};

use alloc::{format, string::String, vec::Vec};
use byteorder::BigEndian;
use crate::infiniband::ib_core::ibv_mtu;
use modular_bitfield_msb::{bitfield, specifiers::{B1, B10, B104, B11, B12, B15, B2, B20, B22, 
    B24, B25, B27, B3, B31, B36, B4, B42, B45, B5, B6, B63, B7, B72, B88, B91}};
use volatile::WriteOnly;
use x86_64::structures::paging::{page::Page, Size4KiB};
use zerocopy::{AsBytes, FromBytes, U16, U32, U64};
use log::{trace, warn};
use crate::memory::PAGE_SIZE;
use core::fmt::Debug as Debug;

use super::{
    cmd::{CommandInterface, MadDemuxOpcodeModifier, Opcode},
    device::{DEFAULT_UAR_PAGE_SHIFT, PAGE_SHIFT},
    icm::{MappedIcmAuxiliaryArea, ICM_PAGE_SHIFT},
    port::Port,
    utils::MappedPages,
    utils
};

#[derive(Clone, FromBytes)]
#[repr(C, packed)]
pub(super) struct Firmware {
    pages: U16<BigEndian>,
    pub(super) major: U16<BigEndian>,
    pub(super) sub_minor: U16<BigEndian>,
    pub(super) minor: U16<BigEndian>,
    _padding1: u16,
    ix_rev: U16<BigEndian>,
    _padding2: [u8; 22], // contains the build timestamp
    clr_int_base: U64<BigEndian>,
    clr_int_bar: u8,
    // many fields follow
}

impl Firmware {
    pub(super) fn query(cmd: &mut CommandInterface) -> Result<Self, &'static str> {
        trace!("asking the card to provide information about its firmware...");
        let page: MappedPages = cmd.execute_command(
            Opcode::QueryFw, (), (), 0,
        )?;
        let mut fw = page.as_type::<Firmware>(0)?.clone();
        fw.clr_int_bar = (fw.clr_int_bar >> 6) * 2;
        trace!("got firmware info: {fw:?}");
        Ok(fw)
    }
    
    pub(super) fn map_area(&self, cmd: &mut CommandInterface) -> Result<MappedFirmwareArea, &'static str> {
        const MAX_CHUNK_LOG2: u32 = 18;
        trace!("mapping firmware area...");

        let size = PAGE_SIZE * usize::from(self.pages);
        let (pages, physical) = utils::create_cont_mapping_with_dma_flags(
            utils::pages_required(size))?.fetch_in_addr()?;
        let mut align = physical.as_u64().trailing_zeros();
        if align > MAX_CHUNK_LOG2 {
            trace!("alignment greater than max chunk size, defaulting to 256KB");
            align = MAX_CHUNK_LOG2;
        }

        let mut num_entries = size / (1 << align);
        if size % (1 << align) != 0 {
            num_entries += 1;
        }
        // batch as many vpm entries as fit in a mailbox to make bootup faster
        let mut vpms = [VirtualPhysicalMapping::default(); 256];
        let mut pointer = physical;
        while num_entries > 0 {
            let mut chunk = PAGE_SIZE / size_of::<VirtualPhysicalMapping>();
            if num_entries < chunk {
                chunk = num_entries;
            }
            for i in 0..chunk {
                vpms[i].physical_address.set(pointer.as_u64() | (align as u64 - ICM_PAGE_SHIFT as u64));
                pointer += 1 << align;
            }
            let _ : () = cmd.execute_command(
                Opcode::MapFa, (), vpms.as_bytes(), chunk.try_into().unwrap(),
            )?;
            num_entries -= chunk;
        }
        trace!("mapped {} pages for firmware area", self.pages);

        Ok(MappedFirmwareArea {
            memory: Some((pages, physical)),
            icm_aux_area: None,
        })
    }
    
    /// Format the version as a string.
    pub(super) fn version(&self) -> String {
        format!("{}.{}.{}", self.major, self.minor, self.sub_minor)
    }
}

impl core::fmt::Debug for Firmware {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f
            .debug_struct("Firmware")
            .field("clr_int_bar", &self.clr_int_bar)
            .field("clr_int_base", &format_args!("{:#x}", self.clr_int_base))
            .field("version", &self.version())
            .field("ix_rev", &self.ix_rev.get())
            .field("size", &format_args!(
                "{}.{} KB",
                (self.pages.get() as usize * PAGE_SIZE) / 1024,
                (self.pages.get() as usize * PAGE_SIZE) % 1024,
            ))
            .finish()
    }
}


#[derive(Clone, AsBytes, Default, Copy)]
#[repr(C, packed)]
pub(super) struct VirtualPhysicalMapping {
    // actually just 52 bits
    pub(super) virtual_address: U64<BigEndian>,
    // actually just 52 bits and then log2size
    pub(super) physical_address: U64<BigEndian>,
}

/// A mapped firmware area.
/// 
/// Instead of dropping, please unmap the area from the card.
pub(super) struct MappedFirmwareArea {
    memory: Option<utils::PageToFrameMapping>,
    icm_aux_area: Option<MappedIcmAuxiliaryArea>,
}

impl MappedFirmwareArea {
    pub(super) fn run(&self, cmd: &mut CommandInterface) -> Result<(), &'static str> {
        let _ : () = cmd.execute_command(Opcode::RunFw, (), (), 0)?;
        trace!("successfully run firmware");
        Ok(())
    }

     pub(super) fn query_capabilities(&self, cmd: &mut CommandInterface) -> Result<Capabilities, &'static str> {
        let page: MappedPages = cmd.execute_command(
            Opcode::QueryDevCap, (), (), 0,
        )?;
        let mut caps = Capabilities::from_bytes(page.as_slice(
            0, size_of::<Capabilities>()
        )?.try_into().unwrap());
        // each UAR has 4 EQ doorbells; so if a UAR is reserved,
        // then we can't use any EQs whose doorbell falls on that page,
        // even if the EQ itself isn't reserved
        if caps.num_rsvd_uars() * 4 > caps.num_rsvd_eqs() {
            caps.set_num_rsvd_eqs(caps.num_rsvd_uars() * 4);
        }
        // TODO: caps.reserved_qpt_cnt[MLX3_QP_REGION_FW] = 1 << caps.log2_rsvd_qps
        // no merge of flags and ext_flags here
        
        trace!("max BF pages: {}", 1 << caps.log_max_bf_pages());
        // TODO: caps.reserved_qpt_cnt[MLX3_QP_REGION_ETH_ADDR] = (1 << caps.log_num_macs) * (1 << caps.log_num_vlans) * caps.num_ports
        trace!("got caps: {:?}", caps);
        Ok(caps)
    }

    /// Query the capabilities a few times, as it fails sometimes.
    pub(crate) fn repeat_query_capabilities(
        &self, cmd: &mut CommandInterface,
    ) -> Result<Capabilities, &'static str> {
        let mut caps = None;
        for _ in 0..3 {
            match self.query_capabilities(cmd) {
                Ok(c) => if c.max_icm_sz() == 0 {
                    warn!("got wrong capabilities, trying again (ICM size = 0)");
                } else {
                    caps = Some(c);
                    break;
                },
                Err(e) => warn!("failed to query for capabilities, trying again: {e}"),
            }
        }
        caps.ok_or("couldn't query capabilities")
    }
    
    /// Unmaps the area from the card. Further usage requires a software reset.
    pub(super) fn unmap(mut self, cmd: &mut CommandInterface) -> Result<(), &'static str> {
        if let Some(icm_aux_area) = self.icm_aux_area.take() {
            icm_aux_area
                .unmap(cmd)
                .unwrap()
        }
        trace!("unmapping firmware area...");
        let _ : () = cmd.execute_command(Opcode::UnmapFa, (), (), 0)?;
        trace!("successfully unmapped firmware area");
        // actually free the memory
        self.memory.take().unwrap();
        Ok(())
    }
    
    /// Set the ICM size.
    /// 
    /// Returns `aux_pages`, the auxiliary ICM size in pages.
    pub(crate) fn set_icm(&self, cmd: &mut CommandInterface, icm_size: u64) -> Result<u64, &'static str> {
        let aux_pages = cmd.execute_command(
            Opcode::SetIcmSize, (), icm_size, 0,
        )?;
        // TODO: round up number of system pages needed if ICM_PAGE_SIZE < PAGE_SIZE
        trace!("ICM auxilliary area requires {aux_pages} 4K pages");
        Ok(aux_pages)
    }

    /// Map the ICM auxiliary area.
    pub(super) fn map_icm_aux(
        &mut self, cmd: &mut CommandInterface, aux_pages: u64,
    ) -> Result<&MappedIcmAuxiliaryArea, &'static str> {
        if self.icm_aux_area.is_some() {
            return Err("ICM auxiliary area has already been mapped");
        }
        // TODO: merge this with Firmware::map_area?
        trace!("mapping ICM auxiliary area...");
        let (pages, physical) = 
            utils::create_cont_mapping_with_dma_flags(aux_pages as usize)?.fetch_in_addr()?;

        let mut align = physical.as_u64().trailing_zeros();
        if align > PAGE_SIZE.ilog2() {
            trace!("alignment greater than max chunk size, defaulting to 256KB");
            align = PAGE_SIZE.ilog2();
        }
        let size = aux_pages * PAGE_SIZE as u64;
        let mut num_entries = usize::try_from(size).unwrap() / (1 << align);
        if size % (1 << align) != 0 {
            num_entries += 1;
        }
        // batch as many vpm entries as fit in a mailbox to make bootup faster
        let mut vpms = [VirtualPhysicalMapping::default(); 256];
        let mut pointer = physical;
        while num_entries > 0 {
            let mut chunk = PAGE_SIZE / size_of::<VirtualPhysicalMapping>();
            if num_entries < chunk {
                chunk = num_entries;
            }
            for i in 0..chunk {
                vpms[i].physical_address.set(pointer.as_u64() | (align as u64 - ICM_PAGE_SHIFT as u64));
                pointer += 1 << align;
            }
            let _ : () = cmd.execute_command(
                Opcode::MapIcmAux, (), vpms.as_bytes(),
                chunk.try_into().unwrap(),
            )?;
            num_entries -= chunk;
        }
        trace!("mapped {} pages for ICM auxiliary area", aux_pages);

        self.icm_aux_area = Some(MappedIcmAuxiliaryArea::new(pages, physical));
        Ok(self.icm_aux_area.as_ref().unwrap())
    }
}

impl Drop for MappedFirmwareArea {
    fn drop(&mut self) {
        if self.icm_aux_area.is_some() || self.memory.is_some() {
            panic!("please unmap instead of dropping");
        }
    }
}

#[bitfield]
pub(super) struct Capabilities {
    #[skip] __: u128,
    #[skip] log_max_srq_sz: u8,
    #[skip(setters)] pub(super) log_max_qp_sz: u8,
    #[skip] __: B4,
    #[skip(setters)] pub(super) log2_rsvd_qps: B4,
    #[skip] __: B3,
    log_max_qp: B5,
    #[skip(setters)] pub(super) log2_rsvd_srqs: B4,
    #[skip] __: B7,
    #[skip(setters)] log_max_srqs: B5,
    #[skip] __: B2,
    #[skip] num_rsvd_eec: B6,
    #[skip] __: B4,
    #[skip] log_max_eec: B4,
    // deprecated
    pub(super) num_rsvd_eqs: u8,
    log_max_cq_sz: u8,
    #[skip] __: B4,
    pub(super) log2_rsvd_cqs: B4,
    #[skip] __: B3,
    #[skip(setters)] log_max_cq: B5,
    #[skip(setters)] log_max_eq_sz: u8,
    #[skip] __: B2,
    #[skip] log_max_d_mpts: B6,
    // deprecated
    #[skip] __: B4,
    #[skip(setters)] log2_rsvd_eqs: B4,
    #[skip] __: B4,
    #[skip(setters)] log_max_eq: B4,
    pub(super) log2_rsvd_mtts: B4,
    #[skip] __: B4,
    #[skip] __: B1,
    #[skip] log_max_mrw_sz: B7,
    #[skip] __: B4,
    #[skip(setters)] pub(super) log2_rsvd_mrws: B4,
    #[skip] __: B2,
    #[skip] log_max_mtts: B6,
    #[skip] __: u16,
    #[skip] __: B4,
    // not present in mlx3
    #[skip] num_sys_eq: B12,
    // max_av?
    #[skip] __: B10,
    #[skip] log_max_ra_req_qp: B6,
    #[skip] __: B10,
    #[skip] log_max_ra_res_qp: B6,
    #[skip] __: B11,
    #[skip] log2_max_gso_sz: B5,
    #[skip] rss: u8,
    #[skip] __: B2,
    #[skip] rdma: B6,
    #[skip] __: B31,
    #[skip] rsz_srq: B1,
    #[skip] port_beacon: B1,
    #[skip] __: B7,
    #[skip] ack_delay: u8,
    #[skip] mtu_width: u8,
    #[skip] __: B4,
    #[skip(setters)] num_ports: B4,
    #[skip] __: B3,
    #[skip(setters)] pub(super) log_max_msg: B5,
    #[skip] __: u16,
    #[skip] max_gid: u8,
    #[skip] rate_support: u16,
    #[skip] cq_timestamp: B1,
    #[skip] __: B15,
    // max_pkey?

    // flags: u64,
    #[skip] __: bool,
    #[skip(setters)] cqe_64b: bool,
    #[skip(setters)] eqe_64b: bool,
    #[skip] __: bool,
    #[skip] port_mng_chg_ev: bool,
    #[skip] __: B3,
    #[skip] sense_port: bool,
    #[skip] __: bool,
    #[skip] set_eth_shed: bool,
    #[skip] rss_ip_frag: bool,
    #[skip] __: B2,
    #[skip(setters)] ethernet_user_prio: bool,
    #[skip] counters: bool,
    #[skip(setters)] ptp1588: bool,
    #[skip] __: B2,
    #[skip(setters)] ethertype_steer: bool,
    #[skip(setters)] vlan_steer: bool,
    #[skip(setters)] vep_mc_steer: bool,
    #[skip(setters)] vep_uc_steer: bool,
    #[skip(setters)] udp_rss: bool,
    #[skip(setters)] thermal_warning: bool,
    #[skip(setters)] wol_port2: bool,
    #[skip(setters)] wol_port1: bool,
    #[skip(setters)] header_split: bool,
    #[skip] __: bool,
    #[skip] fcs_keep: bool,
    #[skip(setters)] mc_loopback: bool,
    #[skip(setters)] uc_loopback: bool,
    #[skip(setters)] fcoe_t11: bool,
    #[skip(setters)] roce: bool,
    #[skip(setters)] ipv6_checksum: bool,
    #[skip(setters)] ud_sw: bool,
    #[skip] __: bool,
    #[skip(setters)] l2_multicast: bool,
    #[skip(setters)] router_mode: bool,
    #[skip(setters)] paging: bool,
    #[skip] __: bool,
    #[skip(setters)] ud_mcast_ipv4: bool,
    #[skip(setters)] ud_mcast: bool,
    #[skip(setters)] avp: bool,
    #[skip(setters)] raw_mcast: bool,
    #[skip(setters)] atomic: bool,
    #[skip(setters)] apm: bool,
    #[skip(setters)] mem_window: bool,
    #[skip(setters)] blh: bool,
    #[skip(setters)] raw_ipv6: bool,
    #[skip(setters)] raw_ethertype: bool,
    #[skip(setters)] dpdp: bool,
    #[skip(setters)] fcoe: bool,
    #[skip(setters)] vmm: bool,
    #[skip(setters)] bad_qkey: bool,
    #[skip(setters)] bad_pkey: bool,
    #[skip(setters)] roce_checksum: bool,
    #[skip(setters)] srq: bool,
    #[skip(setters)] fcob: bool,
    #[skip(setters)] reliable_mc: bool,
    #[skip(setters)] xrc: bool,
    #[skip(setters)] ud: bool,
    #[skip(setters)] uc: bool,
    #[skip(setters)] rc: bool,

    #[skip(setters)] num_rsvd_uars: B4,
    #[skip] __: B6,
    #[skip(setters)] uar_sz: B6,
    #[skip] __: u8,
    #[skip(setters)] log_page_sz: u8,
    #[skip(setters)] pub(super) bf: bool,
    #[skip] __: B10,
    #[skip(setters)] log_bf_reg_sz: B5,
    #[skip] __: B2,
    #[skip(setters)] log_max_bf_regs_per_page: B6,
    #[skip] __: B2,
    #[skip(setters)] log_max_bf_pages: B6,
    #[skip] __: u8,
    #[skip(setters)] pub(super) max_sg_sq: u8,
    #[skip(setters)] pub(super) max_desc_sz_sq: u16,
    #[skip] __: u8,
    #[skip(setters)] pub(super) max_sg_rq: u8,
    #[skip(setters)] max_desc_sz_rq: u16,
    // user_mac_en?
    // svlan_by_qp?
    #[skip] __: B72,
    #[skip] log_max_qp_mcg: u8,
    #[skip] num_rsvd_mcgs: u8,
    #[skip] log_max_mcg: u8,
    #[skip] num_rsvd_pds: B4,
    #[skip] __: B7,
    #[skip] log_max_pd: B5,
    #[skip] num_rsvd_xrcds: B4,
    #[skip] __: B7,
    #[skip] log_max_xrcd: B5,
    #[skip] max_if_cnt_basic: u32,
    #[skip] max_if_cnt_extended: u32,
    #[skip] ext2_flags: u16,
    #[skip] __: u16,
    #[skip] flow_steering_flags: u16,
    #[skip] flow_steering_range: u8,
    #[skip] flow_steering_max_qp_per_entry: u8,
    #[skip] sl2vl_event: u8,
    #[skip] __: u8,
    #[skip] cq_eq_cache_line_stride: u8,
    #[skip] __: B7,
    #[skip] ecn_qcn_ver: B1,
    #[skip ]__: u32,
    #[skip(setters)] pub(super) rdmarc_entry_sz: u16,
    #[skip(setters)] pub(super) qpc_entry_sz: u16,
    #[skip(setters)] pub(super) aux_entry_sz: u16,
    #[skip(setters)] pub(super) altc_entry_sz: u16,
    #[skip(setters)] pub(super) eqc_entry_sz: u16,
    #[skip(setters)] pub(super) cqc_entry_sz: u16,
    #[skip(setters)] pub(super) srq_entry_sz: u16,
    #[skip(setters)] pub(super) c_mpt_entry_sz: u16,
    #[skip(setters)] pub(super) mtt_entry_sz: u16,
    #[skip(setters)] pub(super) d_mpt_entry_sz: u16,
    #[skip] bmme_flags: u16,
    #[skip] phv_en: u16,
    #[skip(setters)] pub(super) reserved_lkey: u32,
    #[skip] diag_flags: u32,
    #[skip(setters)] pub(super) max_icm_sz: u64,
    #[skip] __: u8,
    #[skip] dmfs_high_rate_qpn_base: B24,
    #[skip] __: u8,
    #[skip] dmfs_high_rate_qpn_range: B24,
    #[skip] __: B31,
    #[skip] mad_demux: B1,
    #[skip] __: u128,
    #[skip] __: u128,
    #[skip] __: B36,
    #[skip] qp_rate_limit_max: B12,
    // actually just u12
    #[skip] __: B4,
    #[skip] qp_rate_limit_min: B12,
    // reserved space follows
}

impl Capabilities {
    fn bf_regs_per_page(&self) -> usize {
        if self.bf() {
            if 1 << self.log_max_bf_regs_per_page() > PAGE_SIZE / self.bf_reg_size() {
                3
            } else {
                1 << self.log_max_bf_regs_per_page()
            }
        } else {
            0
        }
    }

    pub(super) fn bf_reg_size(&self) -> usize {
        if self.bf() {
            1 << self.log_bf_reg_sz()
        } else {
            0
        }
    }

    fn num_uars(&self) -> usize {
        usize::try_from(self.uar_size()).unwrap() / PAGE_SIZE
    }

    fn uar_size(&self) -> u64 {
        1 << (self.uar_sz() + 20)
    }

    pub(super) fn get_doorbells_and_blueflame(
        &self, uar: MappedPages,
    ) -> Result<(Vec<MappedPages>, Vec<MappedPages>), &'static str> {
        let mut doorbells = Vec::new();
        let mut blueflame = Vec::new();

        let uar_range = uar.into_range();
        let take_n = uar_range.len() - 1; // exclude last page

        for (idx, page) in &mut uar_range.enumerate().take(take_n as usize) {
            let mapped_page = MappedPages::from(Page::<Size4KiB>::range(page, page + 1));
            if idx <= self.num_uars() {
                doorbells.push(mapped_page);
            } else {
                blueflame.push(mapped_page);
            }
        }

        Ok((doorbells, blueflame))
    }
}

impl core::fmt::Debug for Capabilities {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f
            .debug_struct("Capabilities")
            .field("BlueFlame available", &self.bf())
            .field("BlueFlame reg size", &self.bf_reg_size())
            .field("BlueFlame regs/page", &self.bf_regs_per_page())
            .field("Max ICM size (PB)", &(self.max_icm_sz() >> 50))
            .field("Max QPs", &(1 << self.log_max_qp()) as &dyn Debug)
            .field("reserved QPs", &(1 << self.log2_rsvd_qps()) as &dyn Debug)
            .field("QPC entry size", &self.qpc_entry_sz())
            .field("Max SRQs", &(1 << self.log_max_srqs()) as &dyn Debug)
            .field("reserved SRQs", &(1 << self.log2_rsvd_srqs()) as &dyn Debug)
            .field("SRQ entry size", &self.srq_entry_sz())
            .field("Max CQs", &(1 << self.log_max_cq()) as &dyn Debug)
            .field("reserved CQs", &(1 << self.log2_rsvd_cqs()) as &dyn Debug)
            .field("CQC entry size", &self.cqc_entry_sz())
            .field("Max EQs", &(1 << self.log_max_eq()) as &dyn Debug)
            .field("reserved EQs", &(1 << self.log2_rsvd_eqs()) as &dyn Debug)
            .field("EQC entry size", &self.eqc_entry_sz())
            .field("reserved MPTs", &(1 << self.log2_rsvd_mrws()) as &dyn Debug)
            .field("reserved MTTs", &(1 << self.log2_rsvd_mtts()) as &dyn Debug)
            .field("Max CQE count", &(1 << self.log_max_cq_sz()) as &dyn Debug)
            .field("max QPE count", &(1 << self.log_max_qp_sz()) as &dyn Debug)
            .field("max SRQe count", &(1 << self.log_max_eq_sz()) as &dyn Debug)
            .field("MTT Entry Size", &self.mtt_entry_sz())
            .field("Reserved MTTs", &(1 << self.log2_rsvd_mtts()) as &dyn Debug)
            .field("cMPT Entry Size", &self.c_mpt_entry_sz())
            .field("dMPT Entry Size", &self.d_mpt_entry_sz())
            .field("Reserved UAR", &self.num_rsvd_uars())
            .field("UAR Size", &self.uar_size())
            .field("Num UAR", &self.num_uars())
            .field("Network Port count", &self.num_ports())
            .field("Min Page Size", &(1 << self.log_page_sz()) as &dyn Debug)
            .field("Max SQ desc size WQE Entry Size", &self.max_desc_sz_sq())
            .field("max SQ S/G WQE Entries", &self.max_sg_sq())
            .field("Max RQ desc size", &self.max_desc_sz_rq())
            .field("max RQ S/G", &self.max_sg_rq())
            .field("Max Message Size", &(1 << self.log_max_msg()) as &dyn Debug)
            .field("Unicast loopback support", &self.uc_loopback())
            .field("Multicast loopback support", &self.mc_loopback())
            .field("Header-data split support", &self.header_split())
            .field("Wake on LAN (port 1) support", &self.wol_port1())
            .field("Wake on LAN (port 2) support", &self.wol_port2())
            .field("Thermal warning event", &self.thermal_warning())
            .field("UDP RSS support", &self.udp_rss())
            .field("Unicast VEP steering support", &self.vep_uc_steer())
            .field("Multicast VEP steering support", &self.vep_mc_steer())
            .field("VLAN steering support", &self.vlan_steer())
            .field("EtherType steering support", &self.ethertype_steer())
            // WQE v1 support
            .field("PTP1588 support", &self.ptp1588())
            .field("QPC Ethernet user priority support", &self.ethernet_user_prio())
            .field("64B EQE support", &self.eqe_64b())
            .field("64B CQE support", &self.cqe_64b())
            .field("RC transport support", &self.rc())
            .field("UC transport support", &self.uc())
            .field("UD transport support", &self.ud())
            .field("XRC transport support", &self.xrc())
            .field("Reliable Multicast support", &self.reliable_mc())
            .field("FCoB support", &self.fcob())
            .field("SRQ support", &self.srq())
            .field("RoCE checksum support", &self.roce_checksum())
            .field("Pkey Violation Counter support", &self.bad_pkey())
            .field("Qkey Violation Counter support", &self.bad_qkey())
            .field("VMM support", &self.vmm())
            .field("FCoE support", &self.fcoe())
            .field("DPDP support", &self.dpdp())
            .field("Raw Ethertype support", &self.raw_ethertype())
            .field("Raw IPv6 support", &self.raw_ipv6())
            .field("LSO header support", &self.blh())
            .field("Memory window support", &self.mem_window())
            .field("Automatic Path Migration support", &self.apm())
            .field("Atomic op support", &self.atomic())
            .field("Raw multicast support", &self.raw_mcast())
            .field("AVP support", &self.avp())
            .field("UD Multicast support", &self.ud_mcast())
            .field("UD IPv4 Multicast support", &self.ud_mcast_ipv4())
            // DIF support
            .field("Paging on Demand support", &self.paging())
            .field("Router mode support", &self.router_mode())
            .field("L2 Multicast support", &self.l2_multicast())
            .field("UD transport SW parsing support", &self.ud_sw())
            .field("TCP checksum support for IPv6 support", &self.ipv6_checksum())
            .field("RoCE support", &self.roce())
            .field("FCoE T11 frame support", &self.fcoe_t11())
            .finish()
    }
}

//#[derive(FromBytes)]
#[repr(C, packed)]
pub(super) struct DoorbellEq {
    pub(super) val: WriteOnly<U32<BigEndian>>,
    _padding: u32,
}

//#[derive(FromBytes)]
#[repr(C, packed)]
pub(super) struct DoorbellPage {
    _padding1: u128,
    _padding2: u32,
    pub(super) send_queue_number: WriteOnly<U32<BigEndian>>,
    _padding3: u64,

    // CQ
    /// contains the sequence number, the command and the cq number
    pub(super) cq_sn_cmd_num: WriteOnly<U32<BigEndian>>,
    pub(super) cq_consumer_index: WriteOnly<U32<BigEndian>>,

    // skip 502 u32
    _padding4: [u32; 502],

    // EQ
    // for the EQ number n the relevant doorbell is in
    // DoorbellPage (n / 4) and eq (n % 4)
    pub(super) eqs: [DoorbellEq; 4],

    // skip 503 u32
    _padding9: [u32; 503],
}

#[bitfield]
pub(super) struct InitHcaParameters {
    #[skip(getters)] version: u8,
    #[skip] __: B104,
    #[skip] cacheline_sz: B3,
    // vxlan?
    #[skip] __: B45,
    #[skip(getters)] flags: u32,
    #[skip] recoverable_error_event: bool,
    #[skip] __: B63,

    // QPC parameters
    #[skip] __: u128,
    /// contains both the base (in the upper 59 bits) and log_num (in the lower 5 bits)
    qpc_base_num: u64,
    #[skip] __: u128,
    /// contains both the base (in the upper 59 bits) and log_num (in the lower 5 bits)
    qpc_srqc_base_num: u64,
    /// contains both the base (in the upper 59 bits) and log_num (in the lower 5 bits)
    qpc_cqc_base_num: u64,
    #[skip] __: bool,
    #[skip] qpc_cqe: bool,
    #[skip] qpc_eqe: bool,
    #[skip] __: B22,
    #[skip] qpc_eqe_stride: B3,
    #[skip] __: bool,
    #[skip] qpc_cqe_stride: B3,
    #[skip] __: u32,
    pub(super) qpc_altc_base: u64,
    #[skip] __: u64,
    pub(super) qpc_auxc_base: u64,
    #[skip] __: u64,
    /// contains both the base (in the upper 59 bits) and log_num (in the lower 5 bits)
    qpc_eqc_base_num: u64,
    #[skip] __: B20,
    #[skip] qpc_num_sys_eqs: B12,
    #[skip] __: u32,
    /// contains both the base (in the upper 59 bits) and log_num (in the lower 3 bits)
    qpc_rdmarc_base_num: u64,
    #[skip] __: u64,

    // skip 8 u32
    #[skip] __: u128,
    #[skip] __: u128,

    // multicast parameters
    pub(super) mc_base: u64,
    #[skip] __: B91,
    #[skip(getters)] pub(super) mc_log_entry_sz: B5,
    #[skip] __: B27,
    #[skip(getters)] pub(super) mc_log_hash_sz: B5,
    #[skip] __: B4,
    #[skip] mc_uc_steering: bool,
    #[skip] __: B22,
    #[skip(getters)] pub(super) mc_log_table_sz: B5,
    #[skip] __: u32,

    #[skip] __: u128,
    
    // translation and protection table parameters
    pub(super) tpt_dmpt_base: u64,
    /// enable memory windows
    #[skip] tpt_mw: bool,
    #[skip] __: B25,
    #[skip(getters)] pub(super) tpt_log_dmpt_sz: B6,
    #[skip] __: u32,
    pub(super) tpt_mtt_base: u64,
    pub(super) tpt_cmpt_base: u64,
    #[skip] __: u64,

    #[skip] __: u64,

    // UAR parameters
    #[skip] __: B88,
    /// log page size in 4k chunks
    #[skip(getters)] uar_log_sz: u8,
    #[skip] __: u128,

    // skip 36 u32
    #[skip] __: u128,
    #[skip] __: u128,
    #[skip] __: u128,
    #[skip] __: u128,
    #[skip] __: u128,
    #[skip] __: u128,
    #[skip] __: u128,
    #[skip] __: u128,
    #[skip] __: u128,

    // flow steering parameters
    #[skip] fs_base: u64,
    #[skip] __: B91,
    #[skip] fs_log_entry_sz: B5,
    #[skip] __: u32,
    #[skip] fs_a0: B2,
    #[skip] __: B25,
    #[skip] fs_log_table_sz: B5,
    #[skip] __: B42,
    #[skip] fs_eth_bits: B6,
    #[skip] fs_eth_num_addrs: u16,
    #[skip] __: B12,
    #[skip] fs_ib_bits: B3,
    #[skip] __: bool,
    #[skip] fs_ib_num_addrs: u16,

    // skip 66 u32
    #[skip] __: u128,
    #[skip] __: u128,
    #[skip] __: u128,
    #[skip] __: u128,
    #[skip] __: u128,
    #[skip] __: u128,
    #[skip] __: u128,
    #[skip] __: u128,
    #[skip] __: u128,
    #[skip] __: u128,
    #[skip] __: u128,
    #[skip] __: u128,
    #[skip] __: u128,
    #[skip] __: u128,
    #[skip] __: u128,
    #[skip] __: u128,
    #[skip] __: u64,
}

impl InitHcaParameters {
    pub(super) fn init_hca(
        &mut self, cmd: &mut CommandInterface,
    ) -> Result<Hca, &'static str> {

        // set the needed values
        self.set_version(2); // version must be 2
        // TODO: use a library for this
        let mut flags = 0;
        flags &= !(1 << 1); // little endian on the host
        flags |= 1 << 4; // enable counters / checksums
        flags |= 1; // check port for UD adress vector
        self.set_flags(flags);
        self.set_uar_log_sz(DEFAULT_UAR_PAGE_SHIFT - PAGE_SHIFT);
        
        // execute the command
        let _ : () = cmd.execute_command(Opcode::InitHca, (), &self.bytes[..], 0)?;
        trace!("HCA initialized");
        Ok(Hca { initialized: true, })
    }

    /// Get the number of queue pairs out of qpc_base_num.
    pub(super) fn num_qps(&self) -> usize {
        1 << (self.qpc_base_num() & 0x1f)
    }

    /// Set the (log) number of queue pairs in qpc_base_num.
    pub(super) fn set_qpc_log_qp(&mut self, new: u64) {
        assert_eq!(new & 0x1f, new);
        self.set_qpc_base_num(
            self.qpc_base_num() & 0xffffffffffffffe0 | new & 0x1f
        );
    }

    /// Get the QPC base out of qpc_base_num.
    pub(super) fn qpc_base(&self) -> u64 {
        self.qpc_base_num() & 0xffffffffffffffe0
    }

    /// Set the QPC base in qpc_base_num
    pub(super) fn set_qpc_base(&mut self, new: u64) {
        assert_eq!(new & 0xffffffffffffffe0, new);
        self.set_qpc_base_num(
            self.qpc_base_num() & 0x1f | new & 0xffffffffffffffe0
        );
    }

    /// Get the number of SRQs out of qpc_srqc_base_num.
    pub(super) fn num_srqs(&self) -> usize {
        1 << (self.qpc_srqc_base_num() & 0x1f)
    }

    /// Set the (log) number of SRQs in qpc_srqc_base_num.
    pub(super) fn set_qpc_log_srq(&mut self, new: u64) {
        assert_eq!(new & 0x1f, new);
        self.set_qpc_srqc_base_num(
            self.qpc_srqc_base_num() & 0xffffffffffffffe0 | new & 0x1f
        );
    }

    /// Get the SRQ base out of qpc_srqc_base_num
    pub(super) fn qpc_srqc_base(&self) -> u64 {
        self.qpc_srqc_base_num() & 0xffffffffffffffe0
    }

    /// Set the SRQ base in qpc_srqc_base_num
    pub(super) fn set_qpc_srqc_base(&mut self, new: u64) {
        assert_eq!(new & 0xffffffffffffffe0, new);
        self.set_qpc_srqc_base_num(
            self.qpc_srqc_base_num() & 0x1f | new & 0xffffffffffffffe0
        );
    }

    /// Get the number of completion queues out of qpc_cqc_base_num.
    pub(super) fn num_cqs(&self) -> usize {
        1 << (self.qpc_cqc_base_num() & 0x1f)
    }

    /// Set the (log) number of completions queues in qpc_cqc_base_num.
    pub(super) fn set_qpc_log_cq(&mut self, new: u64) {
        assert_eq!(new & 0x1f, new);
        self.set_qpc_cqc_base_num(
            self.qpc_cqc_base_num() & 0xffffffffffffffe0 | new & 0x1f
        );
    }

    /// Get the CQC base out of qpc_cqc_base_num
    pub(super) fn qpc_cqc_base(&self) -> u64 {
        self.qpc_cqc_base_num() & 0xffffffffffffffe0
    }

    /// Set the CQC base in qpc_cqc_base_num
    pub(super) fn set_qpc_cqc_base(&mut self, new: u64) {
        assert_eq!(new & 0xffffffffffffffe0, new);
        self.set_qpc_cqc_base_num(
            self.qpc_cqc_base_num() & 0x1f | new & 0xffffffffffffffe0
        );
    }

    /// Get the number of event queues out of qpc_eqc_base_num.
    pub(super) fn num_eqs(&self) -> usize {
        1 << (self.qpc_eqc_base_num() & 0x1f)
    }

    /// Set the (log) number of event queues in qpc_eqc_base_num.
    pub(super) fn set_qpc_log_eq(&mut self, new: u64) {
        assert_eq!(new & 0x1f, new);
        self.set_qpc_eqc_base_num(
            self.qpc_eqc_base_num() & 0xffffffffffffffe0 | new & 0x1f
        );
    }

    /// Get the EQC base out of qpc_eqc_base_num.
    pub(super) fn qpc_eqc_base(&self) -> u64 {
        self.qpc_eqc_base_num() & 0xffffffffffffffe0
    }

    /// Set the EQC base in qpc_eqc_base_num
    pub(super) fn set_qpc_eqc_base(&mut self, new: u64) {
        assert_eq!(new & 0xffffffffffffffe0, new);
        self.set_qpc_eqc_base_num(
            self.qpc_eqc_base_num() & 0x1f | new & 0xffffffffffffffe0
        );
    }

    /// Set the (log) number of RDs in qpc_rdmarc_base_num.
    pub(super) fn set_qpc_log_rd(&mut self, new: u8) {
        assert_eq!(new & 0x7, new);
        self.set_qpc_rdmarc_base_num(
            self.qpc_rdmarc_base_num() & 0xffffffffffffffe0 | new as u64 & 0x7
        );
    }

    /// Get the RDMARC base out of qpc_rdmarc_base_num.
    pub(super) fn qpc_rdmarc_base(&self) -> u64 {
        self.qpc_rdmarc_base_num() & 0xffffffffffffffe0
    }

    /// Set the RDMARC base in qpc_rdmarc_base_num
    pub(super) fn set_qpc_rdmarc_base(&mut self, new: u64) {
        assert_eq!(new & 0xffffffffffffffe0, new);
        self.set_qpc_rdmarc_base_num(
            self.qpc_rdmarc_base_num() & 0x7 | new & 0xffffffffffffffe0
        );
    }
}

// an initialized Host Channel Adapter
pub(super) struct Hca {
    initialized: bool,
}

impl Hca {
    pub(super) fn close(
        mut self, cmd: &mut CommandInterface,
    ) -> Result<(), &'static str> {
        trace!("Closing HCA...");
        let _ : () = cmd.execute_command(Opcode::CloseHca, (), (), 0)?;
        self.initialized = false;
        trace!("HCA closed successfully");
        Ok(())
    }

    pub(super) fn query_adapter(
        &self, cmd: &mut CommandInterface,
    ) -> Result<Adapter, &'static str> {
        let page: MappedPages = cmd.execute_command(
            Opcode::QueryAdapter, (), (), 0,
        )?;
        Ok(Adapter::from_bytes(page.as_slice(
            0, size_of::<Adapter>(),
        )?.try_into().unwrap()))
    }
    
    pub(super) fn config_mad_demux(
        &self, cmd: &mut CommandInterface, _caps: &Capabilities,
    ) -> Result<(), &'static str> {
        // TODO: check if mad_demux is supported

        // Query mad_demux to find out which MADs are handled by internal sma
        const SUBNET_MANAGEMENT_CLASS: u32 = 0x1;
        let page: MappedPages = cmd.execute_command(
            Opcode::MadDemux, MadDemuxOpcodeModifier::QueryRestrictions,
            (), SUBNET_MANAGEMENT_CLASS,
        )?;
        // TODO: create a struct for this
        // Config mad_demux to handle all MADs returned by the query above
        let _ : () = cmd.execute_command(
            Opcode::MadDemux, MadDemuxOpcodeModifier::Configure,
            page.as_slice(0, PAGE_SIZE)?, SUBNET_MANAGEMENT_CLASS,
        )?;
        Ok(())
    }
    
    pub(crate) fn init_ports(
        &self, cmd: &mut CommandInterface, caps: &Capabilities,
    ) -> Result<Vec<Port>, &'static str> {
        let mut ports = Vec::with_capacity(caps.num_ports().into());
        for number in 1..=caps.num_ports() {
            let port = Port::new(cmd, number, ibv_mtu::Mtu4096, None)?;
            ports.push(port);
        }
        Ok(ports)
    }
}

impl Drop for Hca {
    fn drop(&mut self) {
        if self.initialized {
            panic!("please close instead of dropping")
        }
    }
}

#[bitfield]
pub(super) struct Adapter {
    #[skip] __: u128,
    /// When PCIe interrupt messages are being used, this value is used for
    /// clearing an interrupt. To clear an interrupt, the driver should write
    /// the value (1<<intapin) into the clr_int register. When using an MSI-X,
    /// this register is not used.
    #[skip] inta_pin: u8,
    #[skip] __: B24,
    // skip 58 u32
    #[skip] __: u128,
    #[skip] __: u128,
    #[skip] __: u128,
    #[skip] __: u128,
    #[skip] __: u128,
    #[skip] __: u128,
    #[skip] __: u128,
    #[skip] __: u128,
    #[skip] __: u128,
    #[skip] __: u128,
    #[skip] __: u128,
    #[skip] __: u128,
    #[skip] __: u128,
    #[skip] __: u128,
    #[skip] __: u64
}
