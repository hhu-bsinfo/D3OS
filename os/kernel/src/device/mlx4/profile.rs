use strum::EnumCount;
use strum_macros::{Display, EnumCount, FromRepr};

use super::fw::InitHcaParameters;

use super::fw::Capabilities;
use log::trace;
use crate::memory::PAGE_SIZE;

#[repr(usize)]
#[derive(Default, Display, EnumCount, FromRepr, Clone, Copy)]
enum ResourceType {
    #[default] QP, RDMARC, ALTC, AUXC, SRQ, CQ, EQ, DMPT, CMPT, MTT, MCG,
}


#[repr(C)]
#[derive(Default, Clone, Copy)]
struct Resource {
    size: u64,
    start: u64,
    typ: ResourceType,
    num: u64,
}

impl Resource {
    fn lognum(&self) -> u32 {
        self.num.ilog2()
    }
}

const DEFAULT_NUM_QP: u64 = 1 << 17;
const DEFAULT_NUM_SRQ: u64 = 1 << 16;
const DEFAULT_RDMARC_PER_QP: u64 = 1 << 4;
const DEFAULT_NUM_CQ: u64 = 1 << 16;
const DEFAULT_NUM_MCG: u64 = 1 << 13;
const DEFAULT_NUM_MPT: u64 = 1 << 19;
const DEFAULT_NUM_MTT: u64 = 1 << 20; // based ON 1024 Ram Mem 1024 >> log_mtt_per_seg-1
const MAX_NUM_EQS: u64 = 1 << 9;

#[repr(usize)]
#[derive(EnumCount)]
#[allow(dead_code)]
enum CmptType {
    QP, SRQ, CQ, EQ,
}

/// This struct contains parameters that are needed to map the ICM tables,
/// but are not part of [`InitHcaParameters`].
pub(super) struct Profile {
    pub(super) num_mpts: usize,
    pub(super) num_mgms: usize,
    pub(super) num_amgms: usize,
    pub(super) num_mtts: usize,
    // TODO: do we need this?
    _max_qp_dest_rdma: usize,
    // the C driver doesn't have this here
    pub(super) rdmarc_shift: u8,
    // the rest of the parameters
    pub(super) init_hca: InitHcaParameters,
    // the ICM size in bytes
    pub(super) total_size: u64,
}

impl Profile {
    /// Construct a profile.
    pub(super) fn new(caps: &Capabilities) -> Result<Self, &'static str> {
        let mut num_mpts = 0;
        let mut num_mgms = 0;
        let mut num_amgms = 0;
        let mut num_mtts = 0;
        let mut max_qp_dest_rdma = 0;
        let mut init_hca = InitHcaParameters::new();
        let mut total_size = 0;
        let log_mtt_per_seg = 3;

        // TODO: this temporarily produces invalid values,
        // but that's how the C driver does it
        let mut profiles: [Resource; ResourceType::COUNT] = Default::default();

        profiles[ResourceType::QP as usize].size = caps.qpc_entry_sz().into();
        profiles[ResourceType::RDMARC as usize].size = caps.rdmarc_entry_sz().into();
        profiles[ResourceType::ALTC as usize].size = caps.altc_entry_sz().into();
        profiles[ResourceType::AUXC as usize].size = caps.aux_entry_sz().into();
        profiles[ResourceType::SRQ as usize].size = caps.srq_entry_sz().into();
        profiles[ResourceType::CQ as usize].size = caps.cqc_entry_sz().into();
        profiles[ResourceType::EQ as usize].size = caps.eqc_entry_sz().into();
        profiles[ResourceType::DMPT as usize].size = caps.d_mpt_entry_sz().into();
        profiles[ResourceType::CMPT as usize].size = caps.c_mpt_entry_sz().into();
        profiles[ResourceType::MTT as usize].size = caps.mtt_entry_sz().into();
        profiles[ResourceType::MCG as usize].size = get_mgm_entry_size();

        profiles[ResourceType::QP as usize].num = DEFAULT_NUM_QP;
        profiles[ResourceType::RDMARC as usize].num = DEFAULT_NUM_QP * DEFAULT_RDMARC_PER_QP;
        profiles[ResourceType::ALTC as usize].num = DEFAULT_NUM_QP;
        profiles[ResourceType::AUXC as usize].num = DEFAULT_NUM_QP;
        profiles[ResourceType::SRQ as usize].num = DEFAULT_NUM_SRQ;
        profiles[ResourceType::CQ as usize].num = DEFAULT_NUM_CQ;
        profiles[ResourceType::EQ as usize].num = MAX_NUM_EQS;
        profiles[ResourceType::DMPT as usize].num = DEFAULT_NUM_MPT;
        profiles[ResourceType::CMPT as usize].num = (CmptType::COUNT << 24).try_into().unwrap();
        profiles[ResourceType::MTT as usize].num = DEFAULT_NUM_MTT * (1 << log_mtt_per_seg);
        profiles[ResourceType::MCG as usize].num = DEFAULT_NUM_MCG;

        for (idx, profile) in profiles.iter_mut().enumerate() {
            profile.typ = ResourceType::from_repr(idx).unwrap();
            profile.num = profile.num.checked_next_power_of_two().unwrap();
            profile.size *= profile.num;
            if profile.size < PAGE_SIZE.try_into().unwrap() {
                profile.size = PAGE_SIZE.try_into().unwrap();
            }
        }

        // Sort the resources in decreasing order of size. Since they all have sizes
        // that are powers of 2, we'll be able to keep resources aligned to their
        // size and pack them without gaps using the sorted order.
        profiles.sort_unstable_by_key(|p| p.size);
        profiles.reverse();

        for (idx, profile) in profiles.iter_mut().enumerate() {
            profile.start = total_size;
            total_size += profile.size;
            if total_size > caps.max_icm_sz() {
                return Err("total size > maximum ICM size");
            }
            if profile.size > 0 {
                trace!(
                    " resource[{:02}] ({:>6}): 2^{:02} entries @ {:#010x} size {} KB",
                    idx, profile.typ, profile.lognum(), profile.start, profile.size >> 10,
                );
            }
        }
        // the C driver doesn't have this here
        let mut rdmarc_shift = 0;
        for profile in profiles.iter() {
            match profile.typ {
                ResourceType::CMPT => init_hca.set_tpt_cmpt_base(profile.start),
                ResourceType::CQ => {
                    init_hca.set_qpc_cqc_base(profile.start);
                    init_hca.set_qpc_log_cq(profile.lognum().try_into().unwrap());
                },
                ResourceType::SRQ => {
                    init_hca.set_qpc_srqc_base(profile.start);
                    init_hca.set_qpc_log_srq(profile.lognum().try_into().unwrap());
                },
                ResourceType::QP => {
                    init_hca.set_qpc_base(profile.start);
                    init_hca.set_qpc_log_qp(profile.lognum().try_into().unwrap());
                },
                ResourceType::ALTC => init_hca.set_qpc_altc_base(profile.start),
                ResourceType::AUXC => init_hca.set_qpc_auxc_base(profile.start),
                ResourceType::MTT => {
                    num_mtts = profile.num.try_into().unwrap();
                    init_hca.set_tpt_mtt_base(profile.start);
                },
                ResourceType::EQ => {
                    init_hca.set_qpc_eqc_base(profile.start);
                    init_hca.set_qpc_log_eq(MAX_NUM_EQS.ilog2().try_into().unwrap());
                },
                ResourceType::RDMARC => {
                    // TODO: this should be possible without a loop
                    while DEFAULT_NUM_QP << rdmarc_shift < profile.num {
                        max_qp_dest_rdma = 1 << rdmarc_shift;
                        init_hca.set_qpc_rdmarc_base(profile.start);
                        init_hca.set_qpc_log_rd(rdmarc_shift);
                        rdmarc_shift += 1;
                    }
                },
                ResourceType::DMPT => {
                    num_mpts = profile.num.try_into().unwrap();
                    init_hca.set_tpt_dmpt_base(profile.start);
                    init_hca.set_tpt_log_dmpt_sz(profile.lognum().try_into().unwrap());
                },
                ResourceType::MCG => {
                    init_hca.set_mc_base(profile.start);
                    init_hca.set_mc_log_entry_sz(get_mgm_entry_size().ilog2().try_into().unwrap());
                    init_hca.set_mc_log_table_sz(profile.lognum().try_into().unwrap());
                    init_hca.set_mc_log_hash_sz((profile.lognum() - 1).try_into().unwrap());
                    num_mgms = (profile.num >> 1).try_into().unwrap();
                    num_amgms = (profile.num >> 1).try_into().unwrap();
                },
            }
        }
        trace!("Max ICM size: {} GB", caps.max_icm_sz() >> 30);
        trace!("ICM memory reserving {} GB", total_size >> 30);
        trace!("HCA Pages Required: {}", total_size >> 12);
        Ok(Self {
            num_mpts, num_mgms, num_amgms, num_mtts,
            _max_qp_dest_rdma: max_qp_dest_rdma, rdmarc_shift, init_hca,
            total_size,
        })
    }
}

pub(super) fn get_mgm_entry_size() -> u64 {
    const DEFAULT_MGM_LOG_ENTRY_SIZE: usize = 10;

    // TODO: how do we actually choose this correctly?
    1 << DEFAULT_MGM_LOG_ENTRY_SIZE
}
