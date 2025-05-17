#![no_std]

extern crate alloc;

use alloc::format;
use num_enum::{FromPrimitive, IntoPrimitive};
use syscall::{SystemCall, syscall};

#[derive(Debug, PartialEq, IntoPrimitive, FromPrimitive)]
#[repr(usize)]
pub enum LogLevel {
    #[num_enum(default)]
    Error = 0,
    Warn = 1,
    Info = 2,
    Debug = 3,
    Trace = 4,
}

pub fn log(message: &str, level: LogLevel) {
    syscall(
        SystemCall::Log,
        &[
            message.as_bytes().as_ptr() as usize,
            message.len(),
            level as usize,
        ],
    )
    .expect(&format!("Unable to log {}", message));
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {{
        $crate::log(
            &alloc::format!($($arg)*),
            $crate::LogLevel::Error,
        )
    }};
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {{
        $crate::log(
            &alloc::format!($($arg)*),
            $crate::LogLevel::Warn,
        )
    }};
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {{
        $crate::log(
            &alloc::format!($($arg)*),
            $crate::LogLevel::Info,
        )
    }};
}

#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {{
        $crate::log(
            &alloc::format!($($arg)*),
            $crate::LogLevel::Debug,
        )
    }};
}

#[macro_export]
macro_rules! trace {
    ($($arg:tt)*) => {{
        $crate::log(
            &alloc::format!($($arg)*),
            $crate::LogLevel::Trace,
        )
    }};
}
