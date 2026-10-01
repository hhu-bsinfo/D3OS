use core::mem::size_of;

use super::{
    Offsets, PdHandle,
    cmd::{CommandInterface, Opcode},
    fw::{Capabilities, VirtualPhysicalMapping},
    profile::{Profile, get_mgm_entry_size},
    queue_pair::QueuePair,
    utils,
};
use crate::device::infiniband::mlx4::cmd::{InputParam, OutputParam};
use crate::device::infiniband::mlx4::device::PAGE_SHIFT;
use crate::device::infiniband::mlx4::utils::MappedPages;
use crate::memory::PAGE_SIZE;
use crate::process::process::Process;
use crate::{memory, process_manager};
use alloc::vec::Vec;
use log::{debug, error, trace};
use modular_bitfield_msb::{bitfield, prelude::*};
use rdma::{AccessFlags, MemoryRegionMetadata};
use uuid::Uuid;
use x86_64::structures::paging::frame::PhysFrameRange;
use x86_64::structures::paging::page::PageRange;
use x86_64::structures::paging::Page;
use x86_64::VirtAddr;
use zerocopy::AsBytes;
use rdma::uverbs_uapi::UserSlice;

pub(super) const ICM_PAGE_SHIFT: u8 = 12;
const TABLE_CHUNK_SIZE: usize = 1 << 18;
const MAX_CHUNK_SIZE: usize = PAGE_SIZE / size_of::<VirtualPhysicalMapping>();

pub(super) fn map_icm_tables(cmd: &mut CommandInterface, profile: &Profile, caps: &Capabilities) -> Result<MappedIcmTables, &'static str> {
    // first, map the cmpt tables
    const CMPT_SHIFT: u8 = 24;
    // TODO: do we really need to calculate the bases here?
    let qp_cmpt_table = IcmTable::init(
        cmd,
        caps.c_mpt_entry_sz(),
        profile.init_hca.num_qps(),
        1 << caps.log2_rsvd_qps(),
        profile.init_hca.tpt_cmpt_base() + (CmptType::QP as u64 * caps.c_mpt_entry_sz() as u64) << CMPT_SHIFT,
    )?;
    trace!("mapped QP cMPT table");
    let srq_cmpt_table = IcmTable::init(
        cmd,
        caps.c_mpt_entry_sz(),
        profile.init_hca.num_srqs(),
        1 << caps.log2_rsvd_srqs(),
        profile.init_hca.tpt_cmpt_base() + (CmptType::SRQ as u64 * caps.c_mpt_entry_sz() as u64) << CMPT_SHIFT,
    )?;
    trace!("mapped SRQ cMPT table");
    let cq_cmpt_table = IcmTable::init(
        cmd,
        caps.c_mpt_entry_sz(),
        profile.init_hca.num_cqs(),
        1 << caps.log2_rsvd_cqs(),
        profile.init_hca.tpt_cmpt_base() + (CmptType::CQ as u64 * caps.c_mpt_entry_sz() as u64) << CMPT_SHIFT,
    )?;
    trace!("mapped CQ cMPT table");
    let eq_cmpt_table = IcmTable::init(
        cmd,
        caps.c_mpt_entry_sz(),
        profile.init_hca.num_eqs(),
        profile.init_hca.num_eqs(),
        profile.init_hca.tpt_cmpt_base() + (CmptType::EQ as u64 * caps.c_mpt_entry_sz() as u64) << CMPT_SHIFT,
    )?;
    trace!("mapped EQ cMPT table");

    // then, the rest
    let eq_table = EqTable {
        table: IcmTable::init(
            cmd,
            caps.eqc_entry_sz(),
            profile.init_hca.num_eqs(),
            profile.init_hca.num_eqs(),
            profile.init_hca.qpc_eqc_base(),
        )?,
        cmpt_table: eq_cmpt_table,
    };
    // Assuming Cache Line is 64 Bytes. Reserved MTT entries must be
    // aligned up to a cacheline boundary, since the FW will write to them,
    // while the driver writes to all other MTT entries. (The variable
    // caps.mtt_entry_sz below is really the MTT segment size, not the
    // raw entry size.)
    let reserved_mtts = ((1 << caps.log2_rsvd_mtts() as u64) * caps.mtt_entry_sz() as u64).next_multiple_of(64) / caps.mtt_entry_sz() as u64;
    let mr_table = MrTable::new(
        IcmTable::init(
            cmd,
            caps.mtt_entry_sz(),
            profile.num_mtts,
            reserved_mtts.try_into().unwrap(),
            profile.init_hca.tpt_mtt_base(),
        )?,
        IcmTable::init(
            cmd,
            caps.d_mpt_entry_sz(),
            profile.num_mpts,
            1 << caps.log2_rsvd_mrws(),
            profile.init_hca.tpt_dmpt_base(),
        )?,
        reserved_mtts,
    );
    let qp_table = QpTable {
        table: IcmTable::init(
            cmd,
            caps.qpc_entry_sz(),
            profile.init_hca.num_qps(),
            1 << caps.log2_rsvd_qps(),
            profile.init_hca.qpc_base(),
        )?,
        cmpt_table: qp_cmpt_table,
        auxc_table: IcmTable::init(
            cmd,
            caps.aux_entry_sz(),
            profile.init_hca.num_qps(),
            1 << caps.log2_rsvd_qps(),
            profile.init_hca.qpc_auxc_base(),
        )?,
        altc_table: IcmTable::init(
            cmd,
            caps.altc_entry_sz(),
            profile.init_hca.num_qps(),
            1 << caps.log2_rsvd_qps(),
            profile.init_hca.qpc_altc_base(),
        )?,
        rdmarc_table: IcmTable::init(
            cmd,
            caps.rdmarc_entry_sz() << profile.rdmarc_shift,
            profile.init_hca.num_qps(),
            1 << caps.log2_rsvd_qps(),
            profile.init_hca.qpc_rdmarc_base(),
        )?,
        _rdmarc_base: profile.init_hca.qpc_rdmarc_base(),
        _rdmarc_shift: profile.rdmarc_shift,
    };
    let cq_table = CqTable {
        table: IcmTable::init(
            cmd,
            caps.cqc_entry_sz(),
            profile.init_hca.num_cqs(),
            1 << caps.log2_rsvd_cqs(),
            profile.init_hca.qpc_cqc_base(),
        )?,
        cmpt_table: cq_cmpt_table,
    };
    let srq_table = SrqTable {
        table: IcmTable::init(
            cmd,
            caps.srq_entry_sz(),
            profile.init_hca.num_srqs(),
            1 << caps.log2_rsvd_srqs(),
            profile.init_hca.qpc_srqc_base(),
        )?,
        cmpt_table: srq_cmpt_table,
    };
    let mcg_table = IcmTable::init(
        cmd,
        get_mgm_entry_size().try_into().unwrap(),
        profile.num_mgms + profile.num_amgms,
        profile.num_mgms + profile.num_amgms,
        profile.init_hca.mc_base(),
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

#[repr(u64)]
#[derive(Default, Clone, Copy)]
enum CmptType {
    #[default]
    QP,
    SRQ,
    CQ,
    EQ,
}

/// A mapped ICM auxiliary area.
///
/// Instead of dropping, please unmap the area from the card.
pub(super) struct MappedIcmAuxiliaryArea {
    frame_ranges: Vec<PhysFrameRange>,
}

impl MappedIcmAuxiliaryArea {
    pub(super) fn new(frame_ranges: Vec<PhysFrameRange>) -> Self {
        Self { frame_ranges }
    }

    /// Unmaps the area from the card.
    pub(super) fn unmap(mut self, cmd: &mut CommandInterface) -> Result<(), &'static str> {
        trace!("unmapping ICM auxiliary area...");
        cmd.execute_command(Opcode::UnmapIcmAux, None, InputParam::Empty, None, OutputParam::Empty)?;
        trace!("successfully unmapped ICM auxiliary area");
        // actually free the memory
        while let Some(frame_range) = self.frame_ranges.pop() {
            memory::free_frames(frame_range);
        }
        Ok(())
    }
}

impl Drop for MappedIcmAuxiliaryArea {
    fn drop(&mut self) {
        if !self.frame_ranges.is_empty() {
            panic!("please unmap instead of dropping")
        }
    }
}

// TODO: do we need those fields?
struct IcmTable {
    virt: u64,
    entry_size: u16,
    /// must contain less than icm_num entries
    icm: Vec<MappedIcm>,
}

impl IcmTable {
    fn init(cmd: &mut CommandInterface, entry_size: u16, entry_capacity: usize, reserved: usize, virt: u64) -> Result<IcmTable, &'static str> {
        trace!(
            "Creating icm table of {} objects with size {} at {:016x}, reserved = {}",
            entry_capacity, entry_size, virt, reserved
        );
        assert!(entry_capacity >= reserved);

        let mut icm = IcmTable {
            virt,
            entry_size,
            icm: Vec::new(),
        };

        icm.map_entries(cmd, 0, reserved)?;

        Ok(icm)
    }

    fn map_entries(&mut self, cmd: &mut CommandInterface, index: usize, count: usize) -> Result<(), &'static str> {
        let bytes_start = index * self.entry_size as usize;
        let byte_end = bytes_start + count * self.entry_size as usize;
        for table_offset in (bytes_start..byte_end).step_by(TABLE_CHUNK_SIZE) {
            self.map_chunk(cmd, table_offset, (TABLE_CHUNK_SIZE / PAGE_SIZE) as u32)?;
        }
        Ok(())
    }

    /// Allocate and map an ICM.
    // TODO: merge this with Firmware::map_area and MappedFirmwareArea::map_icm_aux?
    // TODO: Support higher alignment then one 4KB page to reduce the number of MAP_ICM commands.
    //       Theoretically can the alignment be: page_size * 2^log2size = 4KB * 2^32 = 16TB
    //       See PRM p. 373: Table 161 - Virtual_Physical_Mapping Field Descriptions
    fn map_chunk(&mut self, cmd: &mut CommandInterface, byte_offset: usize, num_pages: u32) -> Result<(), &'static str> {
        trace!(
            "Mapping a chunk of {num_pages} pages at byte offset 0x{byte_offset:x} for ICM Table at 0x{:x}",
            self.virt
        );
        assert!(num_pages > 0);
        assert!(num_pages as usize <= MAX_CHUNK_SIZE);

        // batch as many vpm entries as fit in a mailbox to make bootup faster
        let mut vpms = [VirtualPhysicalMapping::default(); MAX_CHUNK_SIZE];

        // TODO: retry with smaller size if allocation failed
        let memory = utils::create_cont_mapping_with_dma_flags(num_pages as usize)?;
        let phys_start = memory.start_frame().start_address().as_u64();
        let card_virtual = self.virt + byte_offset as u64;

        let chunk_size = vpms.len().min(num_pages as usize);

        for i in 0..chunk_size {
            let offset: u64 = (i * PAGE_SIZE) as u64;
            // We assume that the pages are identity mapped
            vpms[i].physical_address.set(phys_start + offset | (PAGE_SHIFT - ICM_PAGE_SHIFT) as u64);
            vpms[i].virtual_address.set(card_virtual + offset);
        }

        cmd.execute_command(
            Opcode::MapIcm,
            None,
            InputParam::Mailbox(vpms.as_bytes()),
            Some(chunk_size.try_into().unwrap()),
            OutputParam::Empty,
        )?;

        self.icm.push(MappedIcm {
            memory: Some(memory),
            card_virtual,
            num_pages,
        });
        Ok(())
    }

    /// Resolve a byte offset within this table to the host memory backing it.
    ///
    /// ICM is ordinary host memory that the card reads by DMA, so the driver can update table
    /// entries in place.
    fn host_bytes_mut(&mut self, byte_offset: usize, len: usize) -> Result<&mut [u8], &'static str> {
        let index = self
            .icm
            .iter()
            .position(|chunk| {
                let start = (chunk.card_virtual - self.virt) as usize;
                let end = chunk.num_pages as usize * PAGE_SIZE;
                start <= byte_offset && byte_offset < end
            })
            .ok_or("chunk not found")?;
        let memory = self.icm[index].memory.as_mut().ok_or("ICM chunk has no memory")?;
        memory.as_slice_mut(byte_offset % TABLE_CHUNK_SIZE, len)
    }

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
    fn new(mtt_table: IcmTable, dmpt_table: IcmTable, reserved_mtts: u64) -> Self {
        Self {
            mtt_table,
            dmpt_table,
            reserved_mtts,
            offset: 0,
            regions: Vec::new(),
        }
    }

    /// Allocate MTT entries for an existing buffer.
    /// Returns the byte offset in the global mtt to the first entry
    pub(crate) fn alloc_mtt_for_pages(&mut self, caps: &Capabilities, pages: PageRange) -> Result<u64, &'static str> {
        if pages.is_empty() {
            return Err("No pages provides");
        }

        let process = process_manager().read().current_process();
        let kernel = process_manager().read().kernel_process().ok_or("No Kernel Process")?;
        if process.id() != kernel.id() && !process.virtual_address_space.access_ok(pages.start.start_address(), pages.size() as usize) {
            return Err("User has no access to all pages");
        }

        debug!("Create MTT mappings for {:?}", pages);
        // get the next free entry. The Linux driver uses a buddy allocator for MTT
        let addr = (self.reserved_mtts + self.offset) * caps.mtt_entry_sz() as u64;
        self.offset += pages.len();

        const MTT_FLAG_PRESENT: u64 = 1;
        // Write the entries straight into the ICM memory backing the table. The WRITE_MTT command is
        // only used there when the device is a virtual function, which cannot reach ICM itself.
        for (i, page) in pages.enumerate() {
            let physical = match process.virtual_address_space.get_phys(page.start_address().as_u64()) {
                Some(phys_addr) => phys_addr.as_u64(),
                None => {
                    error!("page {:?} is not mapped", page);
                    return Err("page not mapped");
                }
            };
            // A zero or unaligned translation would point the card at memory that is not the
            // buffer, and the card gives no indication when that happens.
            if physical == 0 || physical % PAGE_SIZE as u64 != 0 {
                error!("page {:?} resolved to the invalid physical address {physical:#x}", page);
                return Err("invalid physical address for MTT entry");
            }
            let byte_offset = addr as usize + i * caps.mtt_entry_sz() as usize;
            let entry = self.mtt_table.host_bytes_mut(byte_offset, size_of::<u64>())?;
            entry.copy_from_slice(&(physical | MTT_FLAG_PRESENT).to_be_bytes());
        }
        // Make sure the entries are in memory before anything points the card at them.
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        Ok(addr)
    }

    /// Allocate an entry in the Data Memory Protection Table and return its index, physical address, lkey and rkey.
    ///
    /// This is used by ibv_reg_mr.
    pub(super) fn alloc_dmpt(
        &mut self, cmd: &mut CommandInterface,
        caps: &Capabilities,
        offsets: &mut Offsets,
        owner: &Process,
        pd: PdHandle,
        data: UserSlice,
        queue_pair: Option<&QueuePair>,
        access: AccessFlags,
    ) -> Result<MemoryRegionMetadata, &'static str> {
        if data.is_empty() {
            return Err("MR must not be empty");
        }
        let size = data.size as u64;
        let addr = VirtAddr::try_new(data.address).map_err(|_| "MR address is not canonical")?;
        // Checked before computing the page range, which would panic past the canonical range.
        if !owner.virtual_address_space.access_ok(addr, data.size) {
            return Err("User has no access to MR");
        }
        let pages = Page::range(Page::containing_address(addr), Page::containing_address(addr + size - 1) + 1);
        debug!("Create dMTP for addr: 0x{:016x}, size: 0x{:x}", addr, size);

        // TODO: check if icm has sufficient space available for the new dmpt entry
        let mtt = self.alloc_mtt_for_pages(caps, pages)?;
        let mut dmpt = DmptEntry::new();
        // Set the index directly, not via `set_key`: rotating it would land in the
        // firmware-reserved range.
        dmpt.set_index(offsets.alloc_dmpt().try_into().unwrap());
        dmpt.set_rae(true);
        dmpt.set_pd(pd.0);
        if let Some(qp) = queue_pair {
            dmpt.set_bound_to_qp(true);
            dmpt.set_qp_number(qp.number().try_into().unwrap());
        }
        // This is the start of the region (not the start of the first page of mtt)
        // The offset in the first mtt page (fbo) is taken from the mtt_fbo field if fbo_en=1.
        // When fbo_en=0, fbo is calculated as: start_addr & (2^(entity_size-1))
        dmpt.set_start(addr.as_u64());
        dmpt.set_length(size);
        dmpt.set_entity_size(pages.start.size().ilog2());
        dmpt.set_mtt_addr(mtt);
        dmpt.set_mtt_size(pages.len() as u32);
        dmpt.set_mio(true);
        dmpt.set_region(true);
        // local read is always allowed
        dmpt.set_local_read(true);
        if access.contains(AccessFlags::LOCAL_WRITE) {
            dmpt.set_local_write(true);
        }
        if access.contains(AccessFlags::REMOTE_READ) {
            dmpt.set_remote_read(true);
        }
        if access.contains(AccessFlags::REMOTE_WRITE) {
            dmpt.set_remote_write(true);
        }
        let dmpt_index = dmpt.index();
        cmd.execute_command(Opcode::Sw2HwMpt, None, InputParam::Mailbox(&dmpt.bytes), Some(dmpt_index), OutputParam::Empty)?;
        // get the updated version back
        cmd.execute_command(Opcode::QueryMpt, None, InputParam::Empty, Some(dmpt_index), OutputParam::Mailbox)?;
        let mut dmpt_bytes = [0u8; size_of::<DmptEntry>()];
        dmpt_bytes.copy_from_slice(&cmd.output_mailbox_as_bytes()[..size_of::<DmptEntry>()]);
        let dmpt = DmptEntry::from_bytes(dmpt_bytes);
        assert_eq!(dmpt_index, dmpt.index());
        trace!(
            "memory region of size {} with mem key {}, lkey {}, index {} created successfully",
            dmpt.length(),
            dmpt.key(),
            dmpt.lkey(),
            dmpt.index()
        );

        // The `lkey` field of the entry is owned by the firmware and is not a
        // usable key (Linux writes a zero there and never reads it back). The
        // memory key doubles as both the local and the remote key.
        let lkey = dmpt.key();
        let rkey = dmpt.key();

        self.regions.push(MemoryRegion {
            owner: owner.id(),
            dmpt: Some(dmpt),
        });
        Ok(MemoryRegionMetadata {
            handle: dmpt_index,
            lkey,
            rkey,
        })
    }

    /// Tear down all memory regions.
    pub(super) fn destroy_all(&mut self, cmd: &mut CommandInterface) -> Result<(), &'static str> {
        while let Some(region) = self.regions.pop() {
            region.destroy(cmd)?;
        }
        Ok(())
    }

    /// Tear down a memory region, provided it belongs to `owner`.
    pub(super) fn destroy(&mut self, cmd: &mut CommandInterface, owner: Uuid, index: u32) -> Result<(), &'static str> {
        let idx = self
            .regions
            .iter()
            .position(|region| region.dmpt.as_ref().unwrap().index() == index && region.owner == owner)
            .ok_or("dmpt entry not found")?;
        let dmpt = self.regions.remove(idx);
        dmpt.destroy(cmd)
    }
}

/// This is a wrapper around DmptEntry, so that we can implement Drop.
struct MemoryRegion {
    /// The process that registered this region, i.e. the only process allowed to deregister it.
    owner: Uuid,
    dmpt: Option<DmptEntry>,
}

impl MemoryRegion {
    /// Tear down this region.
    fn destroy(mut self, cmd: &mut CommandInterface) -> Result<(), &'static str> {
        let dmpt = self.dmpt.take().unwrap();
        // TODO: free ICM space
        cmd.execute_command(Opcode::Hw2SwMpt, None, InputParam::Empty, Some(dmpt.index()), OutputParam::Empty)?;
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
    #[skip]
    status: B4,
    #[skip]
    __: B10,
    #[skip(getters)]
    mio: bool,
    #[skip]
    __: B3,
    #[skip(getters)]
    remote_write: bool,
    #[skip(getters)]
    remote_read: bool,
    #[skip(getters)]
    local_write: bool,
    #[skip(getters)]
    local_read: bool,
    #[skip]
    __: bool,
    #[skip(getters)]
    region: bool,
    #[skip]
    __: u8,
    #[skip(getters)]
    qp_number: B24,
    #[skip(getters)]
    bound_to_qp: bool,
    #[skip]
    __: B7,
    /// This index is the key, but formatted as `key[7:0],key[31:8]`,
    /// so we have to provide our own getter and setter implementation.
    index: u32,
    #[skip]
    __: B3,
    #[skip(getters)]
    rae: bool,
    #[skip]
    __: B4,
    #[skip(getters)]
    pd: B24,
    #[skip(getters)]
    /// Start Address - Virtual Address where this region/window starts
    start: u64,
    /// Region/Window Length
    length: u64,
    /// Written by the firmware; must be zero when handing the entry over.
    #[skip(setters)]
    lkey: u32,
    #[skip]
    __: u8,
    #[skip]
    win_cnt: B24,
    #[skip]
    __: B28,
    #[skip]
    mtt_rep: B4,
    #[skip]
    __: B24,
    // the last three bits must be zero
    #[skip(getters)]
    mtt_addr: B40,
    #[skip(getters)]
    mtt_size: u32,
    #[skip]
    __: B11,
    #[skip(getters)]
    /// Page/Block size:
    /// If block_mode == 0, it is log2 of page_size
    /// if block_mode == 1, it is block_size
    /// Minimum value 512
    entity_size: B21,
    #[skip]
    __: B11,
    #[skip]
    first_byte_offset: B21,
    #[skip]
    __: u128,
    #[skip]
    __: u128,
    #[skip]
    __: u128,
    #[skip]
    __: u128,
}

impl DmptEntry {
    /// Get the memory key this entry's index corresponds to.
    ///
    /// This is `hw_index_to_key` in the reference driver: the key is derived from the index, and
    /// the card recovers the index from a key in a work request by rotating it back. Only the
    /// index may be handed to `SW2HW_MPT`, and only the key may be handed to an application.
    fn key(&self) -> u32 {
        self.index() >> 24 | self.index() << 8
    }
}

/// An ICM mapping.
struct MappedIcm {
    memory: Option<MappedPages>,
    card_virtual: u64,
    num_pages: u32,
}

impl MappedIcm {
    /// Unmaps the area from the card.
    pub(super) fn unmap(mut self, cmd: &mut CommandInterface) -> Result<(), &'static str> {
        cmd.execute_command(
            Opcode::UnmapIcm,
            None,
            InputParam::Immediate(self.card_virtual),
            Some(self.num_pages),
            OutputParam::Empty,
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
    pub(super) fn unmap(&mut self, cmd: &mut CommandInterface) -> Result<(), &'static str> {
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
