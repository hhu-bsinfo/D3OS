use byteorder::BigEndian;
use core::{
    fmt::{self, Debug},
    mem::size_of,
};
use modular_bitfield_msb::{
    bitfield,
    prelude::{B3, B5, B11, B28, B60, B84},
    specifiers::{B2, B4, B9, B48},
};
use rdma::{PhysicalPortState, Mtu, PortAttr, PortState};
use zerocopy::{AsBytes, FromBytes, U16, U32, U64};
use super::cmd::{CommandInterface, InputParam, MadIfcOpcodeModifier, Opcode, OutputParam, SetPortOpcodeModifier};
use log::{debug, trace, warn};

#[derive(Debug)]
pub struct Port {
    number: u8,
    open: bool,
    capabilities: Option<PortCapabilities>,
    madifc_output: Option<MadPacket>,
    smi_qpn: u32,
    gsi_qpn: u32,
}

impl Port {
    pub(super) fn new(
        cmd: &mut CommandInterface, number: u8, smi_qpn: u32, gsi_qpn: u32, mtu: Mtu, pkey_table_size: Option<u16>,
    ) -> Result<Self, &'static str> {
        trace!("initializing port {number}...");
        // create the struct
        let mut port = Self {
            number,
            open: false,
            capabilities: None,
            madifc_output: None,
            smi_qpn,
            gsi_qpn,
        };
        // then, get all port capabilities
        let port_attr = port.query(cmd)?;
        // set the capability mask
        let mut set_port_input = SetPortCommand::new();
        set_port_input.set_capabilities(port_attr.port_cap_flags);
        if let Some(size) = pkey_table_size {
            set_port_input.set_change_port_pkey(true);
            set_port_input.set_max_pkey(size);
        }
        set_port_input.set_change_port_mtu(true);
        set_port_input.set_change_port_vl(true);
        set_port_input.set_mtu_cap(mtu as u8);
        for vl_cap_shift in (0..=3).rev() {
            set_port_input.set_vl_cap(1 << vl_cap_shift);
            cmd.execute_command(Opcode::SetPort, Some(SetPortOpcodeModifier::IB.into()), InputParam::Mailbox(&set_port_input.bytes), Some(number.into()), OutputParam::Empty)?;
        }

        // get the current state
        port.query(cmd)?;

        // finally, bring the port up
        cmd.execute_command(Opcode::InitPort, None, InputParam::Empty, Some(number.into()), OutputParam::Empty)?;
        // and update the state again
        port.query(cmd)?;
        trace!("initialized {port:?}");
        // port.query might fail. In that case we won't get the real error,
        // if we already have set open to true.
        port.open = true;
        Ok(port)
    }

    pub(super) fn close(mut self, cmd: &mut CommandInterface) -> Result<(), &'static str> {
        cmd.execute_command(Opcode::ClosePort, None, InputParam::Empty, Some(self.number.into()), OutputParam::Empty)?;
        self.open = false;
        Ok(())
    }

    /// Query the port capabilities, configuration and current settings.
    ///
    /// This is called by ibv_query_port.
    pub(super) fn query(&mut self, cmd: &mut CommandInterface) -> Result<PortAttr, &'static str> {
        // Querying the port might fail, so try this a few times.
        let mut attr = None;
        let mut err = None;
        for _ in 0..5 {
            match self.query_single(cmd) {
                Ok(a) => {
                    attr = Some(a);
                    break;
                }
                Err(e) => {
                    warn!("querying the port failed with: {e:?}");
                    err = Some(e);
                }
            }
        }
        attr.ok_or_else(|| err.unwrap())
    }

    /// Actually query the port.
    fn query_single(&mut self, cmd: &mut CommandInterface) -> Result<PortAttr, &'static str> {
        // QUERY_PORT gives us some details
        cmd.execute_command(Opcode::QueryPort, None, InputParam::Empty, Some(self.number.into()), OutputParam::Mailbox)?;
        let caps_bytes: &[u8; size_of::<PortCapabilities>()] = unsafe { cmd.output_mailbox_as_ref() };
        self.capabilities = Some(PortCapabilities::from_bytes(*caps_bytes));

        // MAD_IFC gives us even more
        const MGMT_CLASS_SUBN_LID_ROUTED: u8 = 0x1;
        const MGMT_METHOD_GET: u8 = 0x1;
        const SMP_ATTR_PORT_INFO: u16 = 0x15;
        let mut madifc_modifier = MadIfcOpcodeModifier::empty();
        madifc_modifier.insert(MadIfcOpcodeModifier::DISABLE_MKEY_VALIDATION);
        madifc_modifier.insert(MadIfcOpcodeModifier::DISABLE_BKEY_VALIDATION);
        let mut madifc_input = MadPacket::new_zeroed();
        madifc_input.base_version = 1;
        madifc_input.mgmt_class = MGMT_CLASS_SUBN_LID_ROUTED;
        madifc_input.class_version = 1;
        madifc_input.method = MGMT_METHOD_GET;
        madifc_input.attr_id = SMP_ATTR_PORT_INFO.into();
        madifc_input.attr_mod = u32::from(self.number).into();
        cmd.execute_command(Opcode::MadIfc, Some(madifc_modifier.into()), InputParam::Mailbox(madifc_input.as_bytes()), Some(self.number.into()), OutputParam::Mailbox)?;
        let madifc_output: &MadPacket = unsafe { cmd.output_mailbox_as_ref() };
        self.madifc_output = Some(madifc_output.clone());
        let madifc_output_data = MadPacketData::from_bytes(self.madifc_output.as_ref().unwrap().data);

        // finally, format it nicely for the application
        Ok(PortAttr {
            state: PortState::from_repr(madifc_output_data.state().into()).ok_or("invalid state")?,
            max_mtu: Mtu::from_repr(madifc_output_data.max_mtu().into()).ok_or("invalid max MTU")?,
            active_mtu: Mtu::from_repr(madifc_output_data.active_mtu()).ok_or("invalid MTU")?,
            port_cap_flags: madifc_output_data.port_cap_flags(),
            lid: madifc_output_data.lid(),
            sm_lid: madifc_output_data.sm_lid(),
            lmc: madifc_output_data.lmc(),
            phys_state: PhysicalPortState::from_repr(madifc_output_data.phys_state()).ok_or("invalid physical port state")?,
            link_layer: 0, // TODO
        })
    }
}

impl Drop for Port {
    fn drop(&mut self) {
        if self.open {
            panic!("Please close instead of dropping")
        }
    }
}

#[bitfield]
struct SetPortCommand {
    #[skip]
    __: B9,
    #[skip(getters)]
    change_port_mtu: bool,
    #[skip(getters)]
    change_port_vl: bool,
    #[skip(getters)]
    change_port_pkey: bool,
    #[skip]
    __: B4,
    #[skip(getters)]
    mtu_cap: B4,
    #[skip]
    __: B4,
    #[skip(getters)]
    vl_cap: B4,
    #[skip]
    __: B4,
    #[skip(getters)]
    capabilities: u32,
    #[skip]
    __: u64,
    #[skip]
    __: u64,
    #[skip]
    __: u64,
    #[skip]
    __: u32,
    #[skip]
    __: u32,
    #[skip(getters)]
    max_pkey: u16,
    // ...
}

#[bitfield]
struct PortCapabilities {
    #[skip(setters)]
    link_up: bool,
    // dmfs_optimized_state
    #[skip]
    __: B2,
    #[skip]
    default_sense: bool,
    #[skip]
    default_type: bool,
    #[skip]
    __: bool,
    #[skip(setters)]
    eth: bool,
    #[skip(setters)]
    ib: bool,
    #[skip]
    __: B4,
    #[skip(setters)]
    ib_mtu: B4,
    #[skip(setters)]
    eth_mtu: u16,
    #[skip]
    ib_link_speed: u8,
    #[skip]
    eth_link_speed: u8,
    #[skip]
    ib_port_width: u8,
    #[skip]
    log_max_gids: B4,
    #[skip]
    log_max_pkeys: B4,
    #[skip]
    __: u16,
    #[skip]
    log_max_vlan: B4,
    #[skip]
    log_max_mac: B4,
    #[skip]
    max_tc_eth: B4,
    #[skip]
    max_vl_ib: B4,
    #[skip]
    __: B48,
    #[skip(setters)]
    mac: B48,
    // ...
}

impl Debug for PortCapabilities {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PortCapabilities")
            .field("IB supported", &self.ib())
            .field("Ethernet supported", &self.eth())
            .field("Link", &self.link_up())
            .field("IB MTU", &Mtu::from_repr(self.ib_mtu()))
            .field("Eth MTU", &self.eth_mtu())
            .field("Port MAC", &self.mac())
            .finish()
    }
}

const SMP_DATA_SIZE: usize = 64;
const SMP_MAX_PATH_HOPS: usize = 64;

#[derive(AsBytes, FromBytes, Clone)]
#[repr(C, packed)]
struct MadPacket {
    base_version: u8,
    mgmt_class: u8,
    class_version: u8,
    method: u8,
    status: U16<BigEndian>,
    hop_ptr: u8,
    hop_cnt: u8,
    tid: U64<BigEndian>,
    attr_id: U16<BigEndian>,
    resv: U16<BigEndian>,
    attr_mod: U32<BigEndian>,
    mkey: U64<BigEndian>,
    dr_slid: U16<BigEndian>,
    dr_dlid: U16<BigEndian>,
    _reserved: [u8; 28],
    data: [u8; SMP_DATA_SIZE],
    initial_path: [u8; SMP_MAX_PATH_HOPS],
    return_path: [u8; SMP_MAX_PATH_HOPS],
}

impl fmt::Debug for MadPacket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MadPacket").finish_non_exhaustive()
    }
}

#[bitfield]
struct MadPacketData {
    #[skip]
    __: u128,
    #[skip(setters)]
    lid: u16,
    #[skip(setters)]
    sm_lid: u16,
    #[skip(setters)]
    port_cap_flags: u32,
    #[skip]
    __: B60,
    #[skip]
    active_width: B4,
    #[skip]
    __: B4,
    #[skip(setters)]
    state: B4,
    #[skip(setters)]
    phys_state: B4,
    #[skip]
    __: B9,
    #[skip(setters)]
    lmc: B3,
    #[skip]
    active_speed: B4,
    #[skip]
    __: B4,
    #[skip(setters)]
    active_mtu: B4,
    #[skip]
    __: B4,
    #[skip]
    max_vl_num: B4,
    #[skip]
    __: B28,
    #[skip]
    init_type_reply: B4,
    #[skip(setters)]
    max_mtu: B4,
    #[skip]
    __: u32,
    #[skip]
    bad_pkey_cntr: u16,
    #[skip]
    qkey_viol_cnt: u16,
    #[skip]
    __: B11,
    #[skip]
    subnet_timeout: B5,
    #[skip]
    __: B84,
    #[skip]
    ext_active_speed: B4,
    #[skip]
    __: u8,
}
