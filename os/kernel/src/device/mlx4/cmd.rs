//! This module consists of functions to create a direct memory access mailbox for passing parameters to the hca
//! and getting output back from the hca during verb calls and functions to execute verb calls.

use core::sync::atomic::{compiler_fence, Ordering};

use super::utils::{OperationArgs, Operations, PageToFrameMapping};
use crate::device::mlx4::utils;
use crate::memory::PAGE_SIZE;
use alloc::boxed::Box;
use core::fmt::{Debug, Formatter};
use core::ops::{BitAnd, BitOr, BitOrAssign, Not, Shl, Shr};
use bitflags::bitflags;
use log::{debug, error, trace};
use strum_macros::{FromRepr, IntoStaticStr};
use tock_registers::interfaces::{Readable, Writeable};
use tock_registers::{register_bitfields, register_bitmasks,};
use tock_registers::registers::{ReadWrite, WriteOnly};
use x86_64::structures::paging::{Page, PhysFrame, Size4KiB};
use x86_64::structures::paging::frame::PhysFrameRange;
use x86_64::structures::paging::page::PageRange;
use zerocopy::AsBytes;

const HCR_BASE: usize = 0x80680;
const HCR_OPMOD_SHIFT: u32 = 12;
const HCR_T_BIT: u32 = 21;
const HCR_E_BIT: u32 = 22;
const HCR_GO_BIT: u32 = 23;
const POLL_TOKEN: u32 = 0xffff;

#[repr(u16)]
#[derive(Debug)]
#[allow(dead_code)]
pub(super) enum Opcode {
    // initialization and general commands
    QueryDevCap = 0x03,
    QueryFw = 0x04,
    QueryAdapter = 0x06,
    InitHca = 0x07,
    CloseHca = 0x08,
    InitPort = 0x09,
    ClosePort = 0x0a,
    QueryHca = 0x0b,
    QueryPort = 0x43,
    SetPort = 0x0c,
    RunFw = 0xff6,
    UnmapIcm = 0xff9,
    MapIcm = 0xffa,
    UnmapIcmAux = 0xffb,
    MapIcmAux = 0xffc,
    UnmapFa = 0xffe,
    SetIcmSize = 0xffd,
    MapFa = 0xfff,

    // TPT commands
    Sw2HwMpt = 0x0d,
    QueryMpt = 0x0e,
    Hw2SwMpt = 0x0f,
    ReadMtt = 0x10,
    WriteMtt = 0x11,

    // EQ commands
    MapEq = 0x12,
    Sw2HwEq = 0x13,
    Hw2SwEq = 0x14,
    QueryEq = 0x15,
    GenEqe = 0x58,

    // CQ commands
    Sw2HwCq = 0x16,
    Hw2SwCq = 0x17,
    QueryCq = 0x18,
    ModifyCq = 0x2c,

    // QP/EE commands
    // The "Any" is there because identifiers cannot start with a number.
    Rst2InitQp = 0x19,
    Init2RtrQp = 0x1a,
    Rtr2RtsQp = 0x1b,
    Rts2RtsQp = 0x1c,
    Sqerr2RtsQp = 0x1d,
    Any2ErrQp = 0x1e,
    Rts2SqdQp = 0x1f,
    Sqd2RtsQp = 0x20,
    Any2RstQp = 0x21,
    QueryQp = 0x22,
    Init2InitQp = 0x2d,
    SuspendQp = 0x32,
    UnsuspendQp = 0x33,
    Sqd2SqdQp = 0x38,
    UpdateQp = 0x61,
    State2StateQp = 0x82,

    // special QP and management commands
    ConfSpecialQp = 0x23,
    MadIfc = 0x24,
    MadDemux = 0x203,
    // miscellaneous commands
    // Ethernet specific commands
}

#[repr(u8)]
#[derive(Debug)]
#[allow(dead_code)]
/// Modifiers for MadDemux
pub(super) enum MadDemuxOpcodeModifier {
    Configure = 0,
    QueryState = 0x1,
    QueryRestrictions = 0x2,
}

impl Into<u8> for MadDemuxOpcodeModifier {
    fn into(self) -> u8 {
        self as u8
    }
}

bitflags! {
    /// Modifiers for MadIfc
    pub(super) struct MadIfcOpcodeModifier: u8 {
        const DISABLE_MKEY_VALIDATION = 1 << 0;
        const DISABLE_BKEY_VALIDATION = 1 << 1;
    }
}

impl Into<u8> for MadIfcOpcodeModifier {
    fn into(self) -> u8 {
        self.bits()
    }
}

#[repr(u8)]
#[derive(Debug)]
#[allow(dead_code)]
pub(super) enum SetPortOpcodeModifier {
    IB = 0x0,
    ETH = 0x1,
    BEACON = 0x4,
}

impl Into<u8> for SetPortOpcodeModifier {
    fn into(self) -> u8 {
        self as u8
    }
}

pub(super) struct CommandInterface {
    hcr: &'static mut Hcr,
    input_mailbox: CmdMailbox,
    output_mailbox: CmdMailbox,
    exp_toggle: u32,
}

register_bitfields![u32,
    StatusOpcode [
        OPCODE     OFFSET(0)  NUMBITS(12) [],
        OPCODE_MOD OFFSET(12) NUMBITS(4) [],
        T          OFFSET(21) NUMBITS(1) [],
        E          OFFSET(22) NUMBITS(1) [
            NoReport = 0,
            Report = 1
        ],
        GO         OFFSET(23) NUMBITS(1) [
            SoftwareOwnership = 0,
            HardwareOwnership = 1
    ],
        STATUS     OFFSET(24) NUMBITS(8) [],
    ]
];

#[repr(C)]
struct Hcr {
    in_param_h: WriteOnly<u32>,
    in_param_l: WriteOnly<u32>,
    in_mod: WriteOnly<u32>,
    out_param_h: ReadWrite<u32>,
    out_param_l: ReadWrite<u32>,
    /// only the first 16 bits are usable
    token: WriteOnly<u32>,
    /// status includes go, e, t and 5 reserved bits;
    /// opcode includes the opcode modifier
    status_opcode: ReadWrite<u32, StatusOpcode::Register>,
}

pub struct CmdMailbox {
    page: Page<Size4KiB>,
    frame: PhysFrame,
}

impl CmdMailbox {
    fn allocate() -> Option<Self> {
        match utils::create_cont_mapping_with_dma_flags(1).ok()?.fetch_in_frame() {
            Ok((pages, frame)) => {
                Some(Self {
                    page: pages.into_range().start,
                    frame
                })
            }
            Err(e) => {
                error!("{e}");
                None
            }
        }
    }

    /// Clears the Mailbox
    #[inline]
    fn clear(&mut self) {
        unsafe {
            core::ptr::write_bytes(self.page.start_address().as_mut_ptr::<u8>(), 0, self.page.size() as usize);
        }
    }

    #[inline]
    fn as_bytes(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.page.start_address().as_ptr::<u8>(), self.page.size() as usize) }
    }

    #[inline]
    pub fn copy_from_bytes(&mut self, bytes: &[u8]) {
        assert!(bytes.len() <= self.page.size() as usize);
        unsafe { core::ptr::copy_nonoverlapping(bytes.as_ptr(), self.page.start_address().as_mut_ptr::<u8>(), bytes.len()) }
    }

}

pub(super) enum InputParam<'a> {
    Empty,
    Immediate(u64),
    Mailbox(&'a [u8]),
}

pub(super) enum OutputParam {
    Empty,
    Immediate,
    Mailbox,
}

impl CommandInterface {
    pub(super) fn new(config_regs: &mut utils::MappedPages) -> Result<Self, &'static str> {
        let hcr_ptr = config_regs.as_type_mut::<Hcr>(HCR_BASE)? as *mut Hcr;
        let hcr = unsafe { &mut *hcr_ptr };

        let input_mailbox = CmdMailbox::allocate().ok_or("failed to allocate input mailbox")?;
        let output_mailbox = CmdMailbox::allocate().ok_or("failed to allocate output mailbox")?;

        Ok(Self {
            hcr,
            exp_toggle: 1,
            input_mailbox,
            output_mailbox
        })
    }

    /// Post a command and wait for its completion.
    ///
    /// Input and Output are optional can either be a Mail or an immediate u64 value depending on the cmd.
    /// If Output is set to Immediate the Result will contain the value otherwise None is returned inside the Result.
    ///
    /// Input has to be written to input_mailbox using input_mailbox_as_mut
    /// Output mailbox is clear before executing the command
    ///
    /// ## Safety
    ///
    /// This function does not check whether the specified opcode takes the
    /// provided type of input or output.
    pub(super) fn execute_command(&mut self, opcode: Opcode, opcode_modifier: Option<u8>, input: InputParam, input_modifier: Option<u32>, output: OutputParam) -> Result<Option<u64>, ReturnStatus> {
        // TODO: timeout
        trace!("executing command: {opcode:?}");

        // wait until the previous command is done
        while self.is_pending() {
            trace!("wait until the previous command is done");
        }

        let input_param = match input {
            InputParam::Empty => 0,
            InputParam::Immediate(v) => v,
            InputParam::Mailbox(s) => {
                self.input_mailbox.clear();
                self.input_mailbox.copy_from_bytes(s);
                self.input_mailbox.frame.start_address().as_u64()
            },
        };

        let (output_mailbox, immediate)  = match output {
            OutputParam::Empty => (0_u64, false),
            OutputParam::Immediate => (0, true),
            OutputParam::Mailbox => {
                self.output_mailbox.clear();
                (self.output_mailbox.frame.start_address().as_u64(), false)
            }
        };

        // post the command
        self.hcr.in_param_h.set(((input_param >> 32) as u32).to_be());
        self.hcr.in_param_l.set((input_param as u32).to_be());
        self.hcr.in_mod.set(input_modifier.unwrap_or(0).to_be());
        self.hcr.out_param_h.set(((output_mailbox >> 32) as u32).to_be());
        self.hcr.out_param_l.set((output_mailbox as u32).to_be());
        self.hcr.token.set((POLL_TOKEN << 16).to_be());
        compiler_fence(Ordering::SeqCst);
        let status_opcode = (1 << HCR_GO_BIT)
            | (self.exp_toggle << HCR_T_BIT)
            | (0 << HCR_E_BIT) // TODO: event
            | ((opcode_modifier.unwrap_or(0) as u32) << HCR_OPMOD_SHIFT)
            | opcode as u16 as u32;
        self.hcr.status_opcode.set(status_opcode.to_be());
        self.exp_toggle ^= 1;

        trace!("polling for completion");
        while self.is_pending() {}

        let status_opcode = u32::from_be(self.hcr.status_opcode.get());
        let status = ReturnStatus::from_repr(status_opcode >> 24).expect("return status invalid");
        trace!("status: {status:?}");

        match status {
            ReturnStatus::Ok => {
                if immediate {
                    let out_param_h = u32::from_be(self.hcr.out_param_h.get()) as u64;
                    let out_param_l = u32::from_be(self.hcr.out_param_l.get()) as u64;
                    let out_param = out_param_h << 32 | out_param_l;
                    trace!("out_param: 0x{out_param:x}");
                    Ok(Some(out_param))
                } else {
                    Ok(None)
                }
            }
            err => Err(err)
        }
    }

    fn is_pending(&self) -> bool {
        let status = u32::from_be(self.hcr.status_opcode.get());
        status & (1 << HCR_GO_BIT) != 0 || (status & (1 << HCR_T_BIT)) == self.exp_toggle
    }

    /// Reinterpret the output mailbox's contents as `&T`.
    ///
    /// ## Safety
    ///
    /// `T` must match the layout written by the device and fit within one page.
    pub(super) unsafe fn output_mailbox_as_ref<T>(&self) -> &T {
        unsafe { &*self.output_mailbox.as_bytes().as_ptr().cast::<T>() }
    }

    pub(super) fn output_mailbox_as_bytes(&self) -> &[u8] {
        self.output_mailbox.as_bytes()
    }

}

#[repr(u32)]
#[derive(Debug, FromRepr, IntoStaticStr)]
pub(super) enum ReturnStatus {
    // general
    Ok = 0x00,
    InternalErr = 0x01,
    BadOp = 0x02,
    BadParam = 0x03,
    BadSysState = 0x04,
    BadResource = 0x05,
    ResourceBusy = 0x06,
    ExceedLim = 0x08,
    BadResState = 0x09,
    BadIndex = 0x0a,
    BadNvmem = 0x0b,
    IcmError = 0x0c,
    BadPerm = 0x0d,

    // QP state
    BadQpState = 0x10,

    // TPT
    RegBound = 0x21,

    // MAD
    BadPkt = 0x30,

    // CQ
    BadSize = 0x40,
}
