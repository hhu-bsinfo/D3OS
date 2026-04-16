/**********************************************************************
 *                                                                    *
 *                  I P I                                             *
 *                                                                    *
 *--------------------------------------------------------------------*
 * Description:     Inter-Processor-Interrupt (IPI) commands used for *
 *                  booting application cores.                        *
 *                                                                    *
 * Author:          Michael Schoetter, Univ. Duesseldorf, 28.8.2022   *
 **********************************************************************/

use bitfield_struct::bitfield;
use core::intrinsics::{volatile_load, volatile_store};
use raw_cpuid::CpuId;
use x86_64::registers::model_specific::Msr;


// Interrupt Command Register 1, R/W
pub const INTERRUPT_COMMAND_REGISTER_HIGH:u32 = 0x310;

// Interrupt Command Register 2, R/W
pub const INTERRUPT_COMMAND_REGISTER_LOW:u32  = 0x300;
const X2APIC_ICR_MSR: u32 = 0x830;

// Default base address of APIC memory-mapped registers
pub const APIC_BASE:u32 = 0xfee00000;

//
// read register
//
pub fn read_reg32(reg: u32) -> u32 {
	unsafe {volatile_load((APIC_BASE + reg) as *const u32)}
}

//
// Write register
//
pub fn write_reg32(reg: u32, value: u32) {
	unsafe {volatile_store((APIC_BASE + reg) as *mut u32, value)};
}

fn cpu_has_x2apic() -> bool {
    CpuId::new()
        .get_feature_info()
        .is_some_and(|features| features.has_x2apic())
}

// Trigger mode
#[derive(Clone, Copy, Debug)]
#[repr(u8)]
pub enum IpiTriggerMode {
    EdgeTriggered   = 0,
    LevelTriggered  = 1,
}

// Interrupt level
#[derive(Clone, Copy, Debug)]
#[repr(u8)]
enum IpiLevel {
	Deassert = 0,  // Must be zero when IpiDeliveryMode = Init
	Assert   = 1,   // Must be one for all other delivery modes
}

// Way of interpreting the value written to the destination field.
#[derive(Clone, Copy, Debug)]
#[repr(u8)]
enum IpiDestinationMode {
	Physical = 0,  // Destination contains the physical destination APIC ID
	Logical  = 1,  // Destination contains a mask of logical APIC IDs
}

// Interrupt state
#[derive(PartialEq, Clone, Copy, Debug)]
#[repr(u8)]
enum IpiDeliveryStatus {
    Idle        = 0,   // no activity for this interrupt
    SendPending = 1,   // interrupt will be sent as soon as the LAPIC is ready
}

// Destinations
#[derive(Clone, Copy, Debug)]
#[repr(u8)]
enum IpiTarget {
    Current             = 1,
    AllIncludingCurrent = 2,
    AllExcludingCurrent = 3,
}

// Delivery mode specifies the type of interrupt sent to the CPU.
#[derive(Clone, Copy, Debug)]
#[repr(u8)]
enum IpiDeliveryMode {
	// "ordinary" interrupt; send to ALL cores listed in the destination bit mask
	Fixed           = 0,  
	
	// "ordinary" interrupt; send to the lowest priority core from destination mask
	LowestPriority  = 1,  
	
	// System Management Interrupt; vector number required to be 0
	Smi             = 2,  
	
	// Non-Maskable Interrupt, vector number ignored, only edge triggered
	Nmi             = 4,  
	
	// Initialization interrupt (always treated as edge triggered)
	Init            = 5,  
	
	// Dedicated Startup-Interrupt (SIPI)
	Startup         = 6,  
}

#[bitfield(u64)]
struct InterruptCommand {
	// Interrupt vector in the IDT 
	#[bits(8)]
	vector: u8,

	// We use 'LowestPriority', as all CPU cores have the same
	// priority and we want to distribute interrupts evenly among them.
	#[bits(3)]
	delivery_mode: u8,

	// We use 'Logical'
	#[bits(1)]
	destination_mode: u8,

	// Current status of interrupt delivery
	#[bits(1)]
	delivery_status: u8,

	// Reserved
	#[bits(1)]
	res1: u8,

	// We use 'High'
	#[bits(1)]
	level: u8,

	// We use 'Edge'
	#[bits(1)]
	trigger_mode:u8,

	// Reserved
	#[bits(2)]
	res2: u8,

	// Destination target
	#[bits(2)]
	destination_target: u8,
	
	// Reserved
	#[bits(36)]
	res3: u64,

    //	Interrupt destination; meaning depends on the destination mode
	#[bits(8)]
	destination: u8,
}

// Read Interrupt Command Register
#[allow(unused_assignments)]
fn read_icr_register() -> InterruptCommand {
	let mut icr = InterruptCommand::new();

    if cpu_has_x2apic() {
        let msr = Msr::new(X2APIC_ICR_MSR);
        let value = unsafe { msr.read() };
        return value.into();
    }

    // read low 32 bit
	let mut low_value:u64;
	loop {
		low_value = read_reg32(INTERRUPT_COMMAND_REGISTER_LOW) as u64;
		icr = low_value.into();
		if icr.delivery_status() == IpiDeliveryStatus::Idle as u8  {
			break;
		}
	}

    // read hight 32 bit
	let high_value:u64;
	high_value = read_reg32(INTERRUPT_COMMAND_REGISTER_HIGH) as u64;

    // fill InterruptCommand  
	let mut existing_value: u64 = icr.into();
	existing_value = existing_value | (high_value << 32);
	icr = existing_value.into();

	icr
}

pub unsafe fn send(val: u64) {
    if cpu_has_x2apic() {
        let mut msr = Msr::new(X2APIC_ICR_MSR);
        unsafe { msr.write(val); }
        return;
    }

	let high_val:u32 = (val >> 32) as u32;
	write_reg32(INTERRUPT_COMMAND_REGISTER_HIGH, high_val);
	
	let low_val:u32 = (val & 0xFFFFFFFF) as u32;
	write_reg32(INTERRUPT_COMMAND_REGISTER_LOW, low_val);
}

#[allow(unused_assignments)]
pub fn send_init() {
	let mut icr = InterruptCommand::new();
	icr = read_icr_register();
				
	icr.set_vector(0);
	icr.set_delivery_mode(IpiDeliveryMode::Init as u8);
	icr.set_level(IpiLevel::Assert as u8);
	icr.set_trigger_mode(IpiTriggerMode::EdgeTriggered as u8);
	icr.set_destination_target(IpiTarget::AllExcludingCurrent as u8);
		
	let val:u64  = icr.into();
	unsafe { send(val); }
}
		
#[allow(unused_assignments)]
pub fn send_startup(vector: u8) {
	let mut icr = InterruptCommand::new();
	icr = read_icr_register();
	
	icr.set_vector(vector);
	icr.set_delivery_mode(IpiDeliveryMode::Startup as u8);
	icr.set_level(IpiLevel::Assert as u8);
	icr.set_trigger_mode(IpiTriggerMode::EdgeTriggered as u8);
	icr.set_destination_target(IpiTarget::AllExcludingCurrent as u8);

	let val:u64  = icr.into();
	unsafe { send(val); }
	
}

/// Sends a Fixed IPI to a specific physical APIC ID with the given vector.
pub fn send_fixed_to_apic(apic_id: usize, vector: u8) {
	let mut icr = read_icr_register();

	const DEST_SHORTHAND_NONE: u8 = 0;

	icr.set_vector(vector);
	icr.set_delivery_mode(IpiDeliveryMode::Fixed as u8);
	icr.set_destination_mode(IpiDestinationMode::Physical as u8);
	icr.set_level(IpiLevel::Assert as u8);
	icr.set_trigger_mode(IpiTriggerMode::EdgeTriggered as u8);
	icr.set_destination_target(DEST_SHORTHAND_NONE);
	icr.set_destination(apic_id as u8);

	let val: u64 = icr.into();
	unsafe { send(val); }
}
