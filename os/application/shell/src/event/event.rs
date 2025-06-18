#[derive(Debug, Clone)]
pub enum Event {
    PrepareNewLine,
    Submit,
    HistoryUp,
    HistoryDown,
    CursorLeft,
    CursorRight,
    AutoComplete,
    SimpleKey(char),
}
