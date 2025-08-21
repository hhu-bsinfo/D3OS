pub mod apic;
pub mod pit;
pub mod ps2;
pub mod qemu_cfg;
pub mod speaker;
#[macro_use]
pub mod terminal;
pub mod cpu;
pub mod ide;
pub mod lfb_terminal;
pub mod pci;
pub mod rtl8139;
pub mod serial;

// make module public
pub mod ne2k;
