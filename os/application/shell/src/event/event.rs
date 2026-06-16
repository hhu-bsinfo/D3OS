use terminal::DecodedKey;

use crate::event::event_handler::Error;

#[derive(Debug, Clone)]
pub enum Event {
    KeyPressed(DecodedKey),
    PrepareNewLine,
    CursorMoved(isize),
    HistoryRestored,
    LineWritten,
    TokensWritten,
    Submit,
    ProcessCompleted,
    ProcessFailed(Error),
}
