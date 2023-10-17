use alloc::string::String;
use crate::kernel;
use crate::kernel::service::log_service::LogLevel;

pub struct Logger {
    name: String
}

#[allow(dead_code)]
impl Logger {
    pub fn new(name: &str) -> Self {
        Self { name: String::from(name) }
    }

    pub fn trace(&self, msg: &str) {
        kernel::get_log_service().log(LogLevel::TRACE, &self.name, &msg);
    }

    pub fn debug(&self, msg: &str) {
        kernel::get_log_service().log(LogLevel::DEBUG, &self.name, &msg);
    }

    pub fn info(&self, msg: &str) {
        kernel::get_log_service().log(LogLevel::INFO, &self.name, &msg);
    }

    pub fn warn(&self, msg: &str) {
        kernel::get_log_service().log(LogLevel::WARN, &self.name, &msg);
    }

    pub fn error(&self, msg: &str) {
        kernel::get_log_service().log(LogLevel::ERROR, &self.name, &msg);
    }
}