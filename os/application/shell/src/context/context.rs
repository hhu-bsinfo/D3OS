use alloc::vec::Vec;

use crate::{
    context::{indicator_context::IndicatorContext, line_context::LineContext, suggestion_context::SuggestionContext},
    executable::Executable,
    modules::lexer::Token,
};

#[derive(Debug, Clone)]
pub struct Context {
    /// Current command line
    pub(crate) line: LineContext,
    /// Command line indicator
    pub(crate) indicator: IndicatorContext,
    /// Command line suggestion (Auto complete)
    pub(crate) auto_completion: SuggestionContext,
    /// Generated tokens based on line
    pub(crate) tokens: Vec<Token>, // TODO CREATE OWN CONTEXT
    /// Generated executable based on tokens
    pub(crate) executable: Option<Executable>, // TODO CREATE OWN CONTEXT
}

impl Context {
    pub fn new() -> Self {
        Self {
            line: LineContext::new(),
            indicator: IndicatorContext::new(),
            auto_completion: SuggestionContext::new(),
            tokens: Vec::new(),
            executable: None,
        }
    }

    /// Returns total line len including prefix and suffix
    pub fn total_line_len(&self) -> usize {
        self.indicator.len() + self.line.len() + self.auto_completion.len()
    }
}
