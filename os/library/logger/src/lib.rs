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

/// Forward log to kernel logger at given level.
///
/// Author: Sebastian Keller
pub fn log(message: &str, level: LogLevel) {
    syscall(
        SystemCall::Log,
        &[message.as_bytes().as_ptr() as usize, message.len(), level as usize],
    )
    .expect(&format!("Unable to log {}", message));
}

/// Forward error-log to kernel logger.
///
/// Author: Sebastian Keller
#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {{
        $crate::log(
            &alloc::format!($($arg)*),
            $crate::LogLevel::Error,
        )
    }};
}

/// Forward warn-log to kernel logger.
///
/// Author: Sebastian Keller
#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {{
        $crate::log(
            &alloc::format!($($arg)*),
            $crate::LogLevel::Warn,
        )
    }};
}

/// Forward info-log to kernel logger.
///
/// Author: Sebastian Keller
#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {{
        $crate::log(
            &alloc::format!($($arg)*),
            $crate::LogLevel::Info,
        )
    }};
}

/// Forward debug-log to kernel logger.
///
/// Author: Sebastian Keller
#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {{
        $crate::log(
            &alloc::format!($($arg)*),
            $crate::LogLevel::Debug,
        )
    }};
}

/// Forward trace-log to kernel logger.
///
/// Author: Sebastian Keller
#[macro_export]
macro_rules! trace {
    ($($arg:tt)*) => {{
        $crate::log(
            &alloc::format!($($arg)*),
            $crate::LogLevel::Trace,
        )
    }};
}
