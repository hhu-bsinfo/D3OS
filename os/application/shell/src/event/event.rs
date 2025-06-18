use terminal::DecodedKey;

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
}
