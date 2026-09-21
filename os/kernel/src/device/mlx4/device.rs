//! This module consists of functions that work close to the hardware of the hca.
use tock_registers::{interfaces::{Readable, Writeable}, register_structs, registers::{ReadOnly, WriteOnly}};

use super::utils::MappedPages;
use crate::pci_bus;
use crate::process::core_local_storage::scheduler;
use log::trace;
use pci_types::EndpointHeader;

const RESET_BASE: usize = 0xf0000;
const OWNER_BASE: usize = 0x8069c;
pub(super) const DEFAULT_UAR_PAGE_SHIFT: u8 = 12;
pub(super) const PAGE_SHIFT: u8 = 12;

register_structs! {
    pub ResetRegisters {
        (0x0 => _reserved),
        (0x10 => reset: WriteOnly<u32>),
        (0x14 => _reserved2),
        (0x3fc => semaphore: ReadOnly<u32>),
        (0x400 => @END),
    }
}

impl ResetRegisters {
    pub(super) fn reset(mlx3_pci_dev: &EndpointHeader, config_regs: &mut MappedPages) -> Result<(), &'static str> {
        // See ConnectX Programmer’s Reference Manual (RPM) Rev 2.1 / Appendix A ConnectX Software Reset
        let config_space = pci_bus().config_space();
        trace!("Initiating card reset for ConnectX-3...");

        // get the reset registers
        let reset_registers: &mut ResetRegisters = config_regs.as_type_mut(RESET_BASE)?;

        // TODO: save config space. See RPM/A2.1

        // grab HW semaphore to lock out flash updates
        let mut sem = 1;
        for _ in 0..1000 {
            sem = u32::from_be(reset_registers.semaphore.get());
            if sem == 0 {
                break;
            }
            trace!("waiting for semaphore...");
            scheduler().sleep(1);
        }
        if sem != 0 {
            return Err("Failed to acquire HW semaphore");
        }

        // actually hit reset
        reset_registers.reset.set(1_u32.to_be());

        // docs say to wait one second before accessing device
        scheduler().sleep(1000);

        for _ in 0..100 {
            // wait for it to respond to PCI cycles

            if mlx3_pci_dev.header().id(config_space).0 != 0xffff {
                return Ok(());
            }
            trace!("waiting for card...");
            scheduler().sleep(1);
        }
        Err("Card failed to reset")
    }
}

#[repr(transparent)]
pub struct Ownership {
    value: ReadOnly<u32>,
}

impl Ownership {
    pub(super) fn get(config_regs: &MappedPages) -> Result<(), &'static str> {
        let ownership: &Ownership = config_regs.as_type(OWNER_BASE)?;
        if ownership.value.get().to_be() == 0 {
            Ok(())
        } else {
            Err("We don't have card ownership")
        }
    }
}

pub(super) fn uar_index_to_hw(index: u32) -> u32 {
    index << (PAGE_SHIFT - DEFAULT_UAR_PAGE_SHIFT)
}
