use crate::context::{
    executable_context::ExecutableContext, indicator_context::IndicatorContext, line_context::LineContext,
    suggestion_context::SuggestionContext, tokens_context::TokensContext,
};

#[derive(Debug, Clone)]
pub struct Context {
    pub(crate) line: LineContext,
    pub(crate) indicator: IndicatorContext,
    pub(crate) suggestion: SuggestionContext,
    pub(crate) tokens: TokensContext,
    pub(crate) executable: ExecutableContext,
}

impl Context {
    pub fn new() -> Self {
        Self {
            line: LineContext::new(),
            indicator: IndicatorContext::new(),
            suggestion: SuggestionContext::new(),
            tokens: TokensContext::new(),
            executable: ExecutableContext::new(),
        }
    }

    /// Returns total line len including prefix and suffix
    pub fn total_line_len(&self) -> usize {
        self.indicator.len() + self.line.len() + self.suggestion.len()
    }
}
