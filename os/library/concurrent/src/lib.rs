#![no_std]
#![feature(atomic_from_mut)]

extern crate alloc;

pub mod core;
pub mod process;
pub mod thread;
pub mod shm;
pub mod ipc;