#![no_std]

pub mod write;
pub mod read;

pub enum Application {
    Shell,
    WindowManager
}