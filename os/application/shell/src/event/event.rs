pub enum Event {
    Prepare,
    Submit,
    HistoryUp,
    HistoryDown,
    CursorLeft,
    CursorRight,
    AutoComplete,
    SimpleKey(char),
}
