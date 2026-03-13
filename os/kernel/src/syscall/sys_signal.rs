/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: lib                                                             ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: All system calls (starting with sys_).                          ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland & Michael Schoettner, 30.8.2024, HHU             ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/

use alloc::boxed::Box;
use crate::scheduler;
use signal::signal_handler::SignalHandler;
use signal::signal_vector::SignalVector;
use log::{debug, info, warn};

pub fn sys_signal_handler_register(index: SignalVector, handler: u64) {
    /* TODO: To make this safe, this should only allow user-mode applications to register
     *       syscalls for themselves.
     *       That means, the interrupt dispatcher would probably need to manage interrupt
     *       overrides per thread.
     */
    let thread = scheduler().current_thread();
    let process = thread.process();
    info!("Registering handler for thread {} in process {} for index {:?} at {:x}", thread.id(), process.id(), index, handler);
    process.signal_dispatcher.assign(index, handler);
}
