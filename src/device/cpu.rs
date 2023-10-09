/*****************************************************************************
 *                                                                           *
 *                                   c p u                                   *
 *                                                                           *
 *---------------------------------------------------------------------------*
 * Beschreibung:    Implementierung einer Abstraktion fuer den Prozessor.    *
 *                                                                           *
 * Autor:           Michael Schoettner, 11.3.2023                            *
 *****************************************************************************/

#![allow(unused_variables)]
#![allow(dead_code)]

use core::arch::asm;
use x86_64::instructions::interrupts;

//
// Pruefe, ob Interrupts erlaubt sind
//
pub fn is_int_enabled() -> bool {
	let rflags: u64;

	unsafe { asm!("pushf; pop {}", lateout(reg) rflags, options(nomem, nostack, preserves_flags)) };
	if (rflags & (1u64 << 9)) != 0 {
		return true;
	}
	false
}


// 
// Sperre Interrupts und gebe zurueck, ob die Interrupts
// zuvor erlaubt waren. Dieses Funktion wird zusammen mit 
// 'enable_int_nested' verwendet und verhinert, dass 
// Interrupts versehentlich angeschaltet werden
//
pub fn disable_int_nested() -> bool {
	let was_enabled = is_int_enabled();
	disable_int();
	was_enabled
}


//
// Erlaube Interrups, falls 'was_enabled' true ist
// Diese Funktion wird zusammen mit 'disable_int_nested' verwendet
//
pub fn enable_int_nested(was_enabled: bool) {
	if was_enabled == true {
		enable_int();
	}
}


//
// Enable interrupts
//
pub fn enable_int () {
   interrupts::enable();
}
    
    
//
// Disable interrupts
//
pub fn disable_int () {
   interrupts::disable();
}


//
// Stop cpu
//
pub fn halt () {
   loop {
      x86_64::instructions::hlt();
   }
}



//
// Return rflags
//
pub fn getflags () -> u64 {
   let rflags: u64;
   unsafe {
       asm! ("pushfq; pop {}", out(reg) rflags, options(nomem, preserves_flags));
   }
   rflags  
}
