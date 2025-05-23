use terminal::DecodedKey;

use crate::context::Context;

pub struct ServiceError {
    message: &'static str,
    reason: Option<&'static str>,
    hint: Option<&'static str>,
}

impl ServiceError {
    pub const fn new(
        message: &'static str,
        reason: Option<&'static str>,
        hint: Option<&'static str>,
    ) -> Self {
        Self {
            message,
            reason,
            hint,
        }
    }
}

pub trait Service {
    fn run(&mut self, event: DecodedKey, context: &mut Context) -> Result<(), ServiceError>;
}
