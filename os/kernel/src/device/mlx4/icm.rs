use core::mem::size_of;

use alloc::vec::Vec;
use crate::infiniband::ib_core::ibv_access_flags;
use modular_bitfield_msb::{bitfield, prelude::{B10, B11, B21, B24, B28, B3, B4, B40, B7}};
use x86_64::{PhysAddr, VirtAddr};
use zerocopy::{AsBytes, BigEndian, FromBytes, U64};
use crate::memory::PAGE_SIZE;
use log::trace;

use super::{
    cmd::{CommandInterface, Opcode},
    fw::{Capabilities, VirtualPhysicalMapping},
    profile::{Profile, get_mgm_entry_size},
    queue_pair::QueuePair,
    Offsets,
    utils,
    utils::MappedPages
};

pub(super) const ICM_PAGE_SHIFT: u8 = 12;

#[repr(u64)]
#[derive(Default, Clone, Copy)]
enum CmptType {
    #[default] QP, SRQ, CQ, EQ,
}

/// A mapped ICM auxiliary area.
/// 
/// Instead of dropping, please unmap the area from the card.
pub(super) struct MappedIcmAuxiliaryArea {
    memory: Option<utils::PageToFrameMapping>,
}

impl MappedIcmAuxiliaryArea {
    pub(super) fn new(pages: MappedPages, physical: PhysAddr) -> Self {
        Self { memory: Some((pages, physical)), }
    }

    /// Unmaps the area from the card.
    pub(super) fn unmap(
        mut self, cmd: &mut CommandInterface,
    ) -> Result<(), &'static str> {
        trace!("unmapping ICM auxiliary area...");
        let _ : () = cmd.execute_command(Opcode::UnmapIcmAux, (), (), 0)?;
        trace!("successfully unmapped ICM auxiliary area");
        // actually free the memory
        self.memory.take().unwrap();
        Ok(())
    }
    
    pub(super) fn map_icm_tables(
        &self, cmd: &mut CommandInterface,
        profile: &Profile, caps: &Capabilities,
    ) -> Result<MappedIcmTables, &'static str> {
        // first, map the cmpt tables
        const CMPT_SHIFT: u8 = 24;
        // TODO: do we really need to calculate the bases here?
        let qp_cmpt_table = self.init_icm_table(
            cmd, caps.c_mpt_entry_sz(), profile.init_hca.num_qps(),
            1 << caps.log2_rsvd_qps(),
            profile.init_hca.tpt_cmpt_base() + (CmptType::QP as u64 * caps.c_mpt_entry_sz() as u64) << CMPT_SHIFT,
        )?;
        trace!("mapped QP cMPT table");
        let srq_cmpt_table = self.init_icm_table(
            cmd, caps.c_mpt_entry_sz(), profile.init_hca.num_srqs(),
            1 << caps.log2_rsvd_srqs(),
            profile.init_hca.tpt_cmpt_base() + (CmptType::SRQ as u64 * caps.c_mpt_entry_sz() as u64) << CMPT_SHIFT,
        )?;
        trace!("mapped SRQ cMPT table");
        let cq_cmpt_table = self.init_icm_table(
            cmd, caps.c_mpt_entry_sz(), profile.init_hca.num_cqs(),
            1 << caps.log2_rsvd_cqs(),
            profile.init_hca.tpt_cmpt_base() + (CmptType::CQ as u64 * caps.c_mpt_entry_sz() as u64) << CMPT_SHIFT,
        )?;
        trace!("mapped CQ cMPT table");
        let eq_cmpt_table = self.init_icm_table(
            cmd, caps.c_mpt_entry_sz(), profile.init_hca.num_eqs(),
            profile.init_hca.num_eqs(),
            profile.init_hca.tpt_cmpt_base() + (CmptType::EQ as u64 * caps.c_mpt_entry_sz() as u64) << CMPT_SHIFT,
        )?;
        trace!("mapped EQ cMPT table");

        // then, the rest
        let eq_table = EqTable {
            table: self.init_icm_table(
                cmd, caps.eqc_entry_sz(), profile.init_hca.num_eqs(),
                profile.init_hca.num_eqs(), profile.init_hca.qpc_eqc_base(),
            )?,
            cmpt_table: eq_cmpt_table,
        };
        // Assuming Cache Line is 64 Bytes. Reserved MTT entries must be
        // aligned up to a cacheline boundary, since the FW will write to them,
        // while the driver writes to all other MTT entries. (The variable
        // caps.mtt_entry_sz below is really the MTT segment size, not the
        // raw entry size.)
        let reserved_mtts = (
            (1 << caps.log2_rsvd_mtts() as u64) * caps.mtt_entry_sz() as u64
        ).next_multiple_of(64) / caps.mtt_entry_sz() as u64;
        let mr_table = MrTable::new(
            self.init_icm_table(
                cmd, caps.mtt_entry_sz(), profile.num_mtts,
                reserved_mtts.try_into().unwrap(), profile.init_hca.tpt_mtt_base(),
            )?,
            self.init_icm_table(
                cmd, caps.d_mpt_entry_sz(), profile.num_mpts,
                1 << caps.log2_rsvd_mrws(), profile.init_hca.tpt_dmpt_base(),
            )?,
            reserved_mtts,
        );
        let qp_table = QpTable {
            table: self.init_icm_table(
                cmd, caps.qpc_entry_sz(), profile.init_hca.num_qps(),
                1 << caps.log2_rsvd_qps(), profile.init_hca.qpc_base(),
            )?,
            cmpt_table: qp_cmpt_table,
            auxc_table: self.init_icm_table(
                cmd, caps.aux_entry_sz(), profile.init_hca.num_qps(),
                1 << caps.log2_rsvd_qps(), profile.init_hca.qpc_auxc_base(),
            )?,
            altc_table: self.init_icm_table(
                cmd, caps.altc_entry_sz(), profile.init_hca.num_qps(),
                1 << caps.log2_rsvd_qps(), profile.init_hca.qpc_altc_base(),
            )?,
            rdmarc_table: self.init_icm_table(
                cmd, caps.rdmarc_entry_sz() << profile.rdmarc_shift,
                profile.init_hca.num_qps(), 1 << caps.log2_rsvd_qps(),
                profile.init_hca.qpc_rdmarc_base(),
            )?,
            _rdmarc_base: profile.init_hca.qpc_rdmarc_base(),
            _rdmarc_shift: profile.rdmarc_shift,
        };
        let cq_table = CqTable {
            table: self.init_icm_table(
                cmd, caps.cqc_entry_sz(), profile.init_hca.num_cqs(),
                1 << caps.log2_rsvd_cqs(), profile.init_hca.qpc_cqc_base(),
            )?,
            cmpt_table: cq_cmpt_table,
        };
        let srq_table = SrqTable {
            table: self.init_icm_table(
                cmd, caps.srq_entry_sz(), profile.init_hca.num_srqs(),
                1 << caps.log2_rsvd_srqs(), profile.init_hca.qpc_srqc_base(),
            )?,
            cmpt_table: srq_cmpt_table,
        };
        let mcg_table = self.init_icm_table(
            cmd, get_mgm_entry_size().try_into().unwrap(),
            profile.num_mgms + profile.num_amgms,
            profile.num_mgms + profile.num_amgms, profile.init_hca.mc_base(),
        )?;
        trace!("ICM tables mapped successfully");
        Ok(MappedIcmTables {
            cq_table: Some(cq_table),
            qp_table: Some(qp_table),
            eq_table: Some(eq_table),
            srq_table: Some(srq_table),
            mr_table: Some(mr_table),
            mcg_table: Some(mcg_table),
        })
    }
    
    fn init_icm_table(
        &self, cmd: &mut CommandInterface, obj_size: u16, obj_num: usize,
        reserved: usize, virt: u64,
    ) -> Result<IcmTable, &'static str> {
        // We allocate in as big chunks as we can,
        // up to a maximum of 256 KB per chunk.
        const TABLE_CHUNK_SIZE: usize = 1 << 18;

        let table_size = obj_size as usize * obj_num;
        let obj_per_chunk = TABLE_CHUNK_SIZE / obj_size as usize;
        let icm_num = (obj_num + obj_per_chunk - 1) / obj_per_chunk;
        let mut icm = Vec::new();
        // map the reserved entries
        let mut idx = 0;
        while idx * TABLE_CHUNK_SIZE < reserved * obj_size as usize {
            let mut chunk_size = TABLE_CHUNK_SIZE;
            // TODO: does this make sense?
            if (idx + 1) * chunk_size > table_size {
                chunk_size = (table_size - idx * TABLE_CHUNK_SIZE).next_multiple_of(PAGE_SIZE);
            }
            let mut num_pages: u32 = (chunk_size / PAGE_SIZE).try_into().unwrap();
            if num_pages == 0 {
                num_pages = 1;
                chunk_size = num_pages as usize * PAGE_SIZE;
            }
            icm.push(MappedIcm::new(
                cmd, chunk_size, num_pages,
                virt + (idx * TABLE_CHUNK_SIZE) as u64,
            )?);

            idx += 1;
        }
        Ok(IcmTable {
            _virt: virt, _obj_num: obj_num, _obj_size: obj_size,
            _icm_num: icm_num, icm,
        })
    }
    
}

impl Drop for MappedIcmAuxiliaryArea {
    fn drop(&mut self) {
        if self.memory.is_some() {
            panic!("please unmap instead of dropping")
        }
    }
}

// TODO: do we need those fields?
struct IcmTable {
    _virt: u64,
    _obj_num: usize,
    _obj_size: u16,
    /// the available number of Icms
    _icm_num: usize,
    /// must contain less than icm_num entries
    icm: Vec<MappedIcm>,
}

impl IcmTable {
    fn unmap(mut self, cmd: &mut CommandInterface) -> Result<(), &'static str> {
        while let Some(icm) = self.icm.pop() {
            icm.unmap(cmd)?;
        }
        Ok(())
    }
}

struct CqTable {
    table: IcmTable,
    cmpt_table: IcmTable,
}

struct QpTable {
    table: IcmTable,
    cmpt_table: IcmTable,
    auxc_table: IcmTable,
    altc_table: IcmTable,
    rdmarc_table: IcmTable,
    // TODO: these two do not seem to be used?
    _rdmarc_base: u64,
    _rdmarc_shift: u8,
}

struct EqTable {
    table: IcmTable,
    cmpt_table: IcmTable,
}

struct SrqTable {
    table: IcmTable,
    cmpt_table: IcmTable,
}

pub(super) struct MrTable {
    mtt_table: IcmTable,
    dmpt_table: IcmTable,
    reserved_mtts: u64,
    offset: u64,
    regions: Vec<MemoryRegion>,
    // TODO
}
impl MrTable {
    fn new(
        mtt_table: IcmTable, dmpt_table: IcmTable, reserved_mtts: u64,
    ) -> Self {
        Self {
            mtt_table, dmpt_table, reserved_mtts, offset: 0, regions: Vec::new(),
        }
    }

    /// Allocate MTT entries for an existing buffer.
    pub(crate) fn alloc_mtt(
        &mut self, cmd: &mut CommandInterface, caps: &Capabilities,
        num_entries: usize, data_address: PhysAddr,
    ) -> Result<u64, &'static str> {
        let mut num_entries: u64 = num_entries.try_into().unwrap();
        assert_ne!(num_entries, 0);
        // get the next free entry
        let addr = (
            self.reserved_mtts + self.offset
        ) * caps.mtt_entry_sz() as u64;
        self.offset += num_entries;
        
        // send it to the card
        const MTT_FLAG_PRESENT: u64 = 1;
        // we could possibly also write single entries, but this is way slower
        // and also doesn't work sometimes
        let mut start_index = 0;
        while num_entries > 0 {
            let mut chunk: u64 = (PAGE_SIZE / size_of::<u64>() - 2)
                .try_into().unwrap();
            if num_entries < chunk {
                chunk = num_entries;
            }
            let mut write_cmd = WriteMttCommand::new_zeroed();
            write_cmd.offset.set(addr + start_index);
            for i in 0..chunk {
                write_cmd.entries[usize::try_from(i).unwrap()].set((
                    data_address.as_u64() + (i + start_index) * PAGE_SIZE as u64
                ) | MTT_FLAG_PRESENT);
            }
            let _ : () = cmd.execute_command(
                Opcode::WriteMtt, (), write_cmd.as_bytes(),
                chunk.try_into().unwrap(),
            )?;
            num_entries -= chunk;
            start_index += chunk;
        }
        Ok(addr)
    }
    
    /// Allocate an entry in the Data Memory Protection Table and return its index, physical address, lkey and rkey.
    /// 
    /// This is used by ibv_reg_mr.
    pub(super) fn alloc_dmpt<T>(
        &mut self, cmd: &mut CommandInterface, caps: &Capabilities,
        offsets: &mut Offsets, data: &mut [T], queue_pair: Option<&QueuePair>,
        access: ibv_access_flags,
    ) -> Result<(u32, usize, u32, u32), &'static str> {
        let size = data.len() * size_of::<T>();
        let address = utils::get_physical_address(VirtAddr::from_ptr(
            data.as_ptr()));
        //println!("Physical address = {:x} => is aligend = {}", address, address.is_aligned(PAGE_SIZE as u64));
        
        let mut num_pages = size / PAGE_SIZE;
        if num_pages == 0 {
            num_pages = 1;
        }
        // TODO: check if icm has sufficient space available for the new dmpt entry
        let mtt = self.alloc_mtt(cmd, caps, num_pages, address)?;
        let mut dmpt = DmptEntry::new();
        dmpt.set_key(offsets.alloc_dmpt().try_into().unwrap());
        dmpt.set_rae(true);
        if let Some(qp) = queue_pair {
            dmpt.set_bound_to_qp(true);
            dmpt.set_qp_number(qp.number().try_into().unwrap());
        }
        dmpt.set_start(address.as_u64().try_into().unwrap());
        dmpt.set_length(size.try_into().unwrap());
        dmpt.set_entity_size(PAGE_SIZE.ilog2()); // used PAGE_SIZE mappings in the mtt,
        // hence the granularity also has to match PAGE_SIZE, setting to buffer size doesn't make sense !
        dmpt.set_mtt_addr(mtt);
        dmpt.set_mtt_size(num_pages.try_into().unwrap());
        dmpt.set_mio(true);
        dmpt.set_region(true);
        // local read is always allowed
        dmpt.set_local_read(true);
        if access.contains(ibv_access_flags::IBV_ACCESS_LOCAL_WRITE) {
            dmpt.set_local_write(true);
        }
        if access.contains(ibv_access_flags::IBV_ACCESS_REMOTE_READ) {
            dmpt.set_remote_read(true);
        }
        if access.contains(ibv_access_flags::IBV_ACCESS_REMOTE_WRITE) {
            dmpt.set_remote_write(true);
        }
        let dmpt_index = dmpt.index();
        let _ : () = cmd.execute_command(
            Opcode::Sw2HwMpt, (), &dmpt.into_bytes()[..], dmpt_index,
        )?;
        // get the updated version back
        let dmpt_output_page: MappedPages = cmd.execute_command(
            Opcode::QueryMpt, (), (), dmpt_index,
        )?;
        let dmpt = DmptEntry::from_bytes(dmpt_output_page.as_slice(
            0, size_of::<DmptEntry>()
        )?.try_into().unwrap());
        assert_eq!(dmpt_index, dmpt.index());
        trace!(
            "memory region of size {} with mem key {} created successfully",
            dmpt.length(), dmpt.key(),
        );
        // dmpt.lkey() would be the lkey if we were using protection domains.
        // Just put the reserved lkey here, so that addresses are physical.
        let dmpt_lkey = caps.reserved_lkey();
        // .key is the rkey
        let dmpt_key = dmpt.key();

        self.regions.push(MemoryRegion { dmpt: Some(dmpt) });
        Ok((dmpt_index, address.as_u64() as usize, dmpt_lkey, dmpt_key))
    }
    
    /// Tear down all memory regions.
    pub(super) fn destroy_all(
        &mut self, cmd: &mut CommandInterface,
    ) -> Result<(), &'static str> {
        while let Some(region) = self.regions.pop() {
            region.destroy(cmd)?;
        }
        Ok(())
    }
    
    /// Tear down a memory region.
    pub(super) fn destroy(
        &mut self, cmd: &mut CommandInterface, index: u32,
    ) -> Result<(), &'static str> {
        let (idx, _) = self.regions
            .iter()
            .enumerate()
            .find(|(_, region)| region.dmpt.as_ref().unwrap().index() == index)
            .ok_or("dmpt entry not found")?;
        let dmpt = self.regions.remove(idx);
        dmpt.destroy(cmd)
    }
}

/// the struct passed to WriteMtt
#[derive(AsBytes, FromBytes)]
#[repr(C, packed)]
struct WriteMttCommand {
    offset: U64<BigEndian>,
    _reserved: u64,
    /// the physical address, except for the last three bits
    /// (those must be zero); the last bit is the present bit
    entries: [U64<BigEndian>; 510],
}

/// This is a wrapper around DmptEntry, so that we can implement Drop.
struct MemoryRegion {
    dmpt: Option<DmptEntry>
}

impl MemoryRegion {
    /// Tear down this region.
    fn destroy(
        mut self, cmd: &mut CommandInterface,
    ) -> Result<(), &'static str> {
        let dmpt = self.dmpt.take().unwrap();
        // TODO: free ICM space
        let _ : () = cmd.execute_command(Opcode::Hw2SwMpt, (), (), dmpt.index())?;
        Ok(())
    }
}

impl Drop for MemoryRegion {
    fn drop(&mut self) {
        if self.dmpt.is_some() {
            panic!("please destroy instead of dropping")
        }
    }
}

/// An entry of the Data Memory Protection Table.
// TODO: keep actual references, so that data, eq and qp live long enough
#[bitfield]
struct DmptEntry {
    #[skip] status: B4,
    #[skip] __: B10,
    #[skip(getters)] mio: bool,
    #[skip] __: B3,
    #[skip(getters)] remote_write: bool,
    #[skip(getters)] remote_read: bool,
    #[skip(getters)] local_write: bool,
    #[skip(getters)] local_read: bool,
    #[skip] __: bool,
    #[skip(getters)] region: bool,
    #[skip] __: u8,
    #[skip(getters)] qp_number: B24,
    #[skip(getters)] bound_to_qp: bool,
    #[skip] __: B7,
    /// This index is the key, but formatted as `key[7:0],key[31:8]`,
    /// so we have to provide our own getter and setter implementation.
    index: u32,
    #[skip] __: B3,
    #[skip(getters)] rae: bool,
    #[skip] __: B4,
    #[skip] pd: B24,
    #[skip(getters)] start: u64,
    length: u64,
    #[skip] lkey: u32,
    #[skip] __: u8,
    #[skip] win_cnt: B24,
    #[skip] __: B28,
    #[skip] mtt_rep: B4,
    #[skip] __: B24,
    // the last three bits must be zero
    #[skip(getters)] mtt_addr: B40,
    #[skip(getters)] mtt_size: u32,
    #[skip] __: B11,
    #[skip(getters)] entity_size: B21,
    #[skip] __: B11,
    #[skip] first_byte_offset: B21,
    #[skip] __: u128,
    #[skip] __: u128,
    #[skip] __: u128,
    #[skip] __: u128,
}

impl DmptEntry {
    /// Get the memory key.
    fn key(&self) -> u32 {
        self.index() >> 24 | self.index() << 8
    }

    /// Set the memory key.
    fn set_key(&mut self, key: u32) {
        self.set_index(key << 24 | key >> 8)
    }
}

/// An ICM mapping.
struct MappedIcm {
    memory: Option<utils::PageToFrameMapping>,
    card_virtual: u64,
    num_pages: u32,
}

impl MappedIcm {
    /// Allocate and map an ICM.
    // TODO: merge this with Firmware::map_area and MappedFirmwareArea::map_icm_aux?
    fn new(
        cmd: &mut CommandInterface, chunk_size: usize, num_pages: u32,
        card_virtual: u64,
    ) -> Result<Self, &'static str> {
        let (pages, physical) = utils::create_cont_mapping_with_dma_flags(
            utils::pages_required(chunk_size))?.fetch_in_addr()?;
        let mut align = physical.as_u64().trailing_zeros();
        if align > PAGE_SIZE.ilog2() {
            // TODO: fw.rs says it's 256KB?
            trace!("alignment greater than max size, defaulting to 4KB");
            align = PAGE_SIZE.ilog2();
        }
        let size = num_pages as usize * PAGE_SIZE;
        let mut num_entries = size / (1 << align);
        if size % (1 << align) != 0 {
            num_entries += 1;
        }
        // batch as many vpm entries as fit in a mailbox to make bootup faster
        let mut vpms = [VirtualPhysicalMapping::default(); 256];
        let mut phys_pointer = physical;
        let mut virt_pointer = card_virtual;
        while num_entries > 0 {
            let mut chunk = PAGE_SIZE / size_of::<VirtualPhysicalMapping>();
            if num_entries < chunk {
                chunk = num_entries;
            }
            for i in 0..chunk {
                vpms[i].physical_address.set(phys_pointer.as_u64() | (align as u64 - ICM_PAGE_SHIFT as u64));
                vpms[i].virtual_address.set(virt_pointer);

                phys_pointer += 1 << align;
                virt_pointer += 1 << align;
            }
            let _ : () = cmd.execute_command(
                Opcode::MapIcm, (), vpms.as_bytes(), chunk.try_into().unwrap(),
            )?;
            num_entries -= chunk;
        }
        Ok(Self { memory: Some((pages, physical)), card_virtual, num_pages, })
    }

    /// Unmaps the area from the card.
    pub(super) fn unmap(
        mut self, cmd: &mut CommandInterface,
    ) -> Result<(), &'static str> {
        let _ : () = cmd.execute_command(
            Opcode::UnmapIcm, (), self.card_virtual, self.num_pages,
        )?;
        // actually free the memory
        self.memory.take().unwrap();
        Ok(())
    }
}

impl Drop for MappedIcm {
    fn drop(&mut self) {
        if self.memory.is_some() {
            panic!("please unmap instead of dropping")
        }
    }
}

pub(super) struct MappedIcmTables {
    cq_table: Option<CqTable>,
    qp_table: Option<QpTable>,
    eq_table: Option<EqTable>,
    srq_table: Option<SrqTable>,
    mr_table: Option<MrTable>,
    mcg_table: Option<IcmTable>,
}

impl MappedIcmTables {
    /// Unmaps the area from the card.
    pub(super) fn unmap(
        mut self, cmd: &mut CommandInterface,
    ) -> Result<(), &'static str> {
        trace!("unmapping ICM tables...");
        if let Some(eq_table) = self.eq_table.take() {
            eq_table.table.unmap(cmd)?;
            eq_table.cmpt_table.unmap(cmd)?;
        }
        if let Some(cq_table) = self.cq_table.take() {
            cq_table.table.unmap(cmd)?;
            cq_table.cmpt_table.unmap(cmd)?;
        }
        if let Some(qp_table) = self.qp_table.take() {
            qp_table.table.unmap(cmd)?;
            qp_table.rdmarc_table.unmap(cmd)?;
            qp_table.altc_table.unmap(cmd)?;
            qp_table.auxc_table.unmap(cmd)?;
            qp_table.cmpt_table.unmap(cmd)?;
        }
        if let Some(mr_table) = self.mr_table.take() {
            mr_table.dmpt_table.unmap(cmd)?;
            mr_table.mtt_table.unmap(cmd)?;
        }
        if let Some(mcg_table) = self.mcg_table.take() {
            mcg_table.unmap(cmd)?;
        }
        if let Some(srq_table) = self.srq_table.take() {
            srq_table.table.unmap(cmd)?;
            srq_table.cmpt_table.unmap(cmd)?;
        }
        trace!("successfully unmapped ICM tables");
        Ok(())
    }
    
    // Get the memory regions table.
    pub(crate) fn memory_regions(&mut self) -> &mut MrTable {
        self.mr_table.as_mut().unwrap()
    }
}
