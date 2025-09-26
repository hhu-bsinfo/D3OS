/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: lib                                                             ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: All system call counterparts in kernel, starting with 'sys_'.   ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland & Michael Schoettner, 30.8.2024, HHU             ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/

pub mod sys_naming;
pub mod sys_terminal;
pub mod sys_concurrent;
pub mod sys_time;
pub mod sys_vmem;
pub mod sys_graphic;
pub mod sys_input;
pub mod sys_system_info;

pub mod syscall_dispatcher;
