use terminal::DecodedKey;

#[derive(Debug, PartialEq)]
pub enum ViewMessage {
    // represent the chars to move up or down (not the lines)
    ScrollDown(u32),
    ScrollUp(u32),
}

pub enum CommandMessage {
    // represent the chars to move up or down (not the lines)
    Undo,
    Redo,
    // toggle between markdown and normal view
    Markdown,
    //clike syntax highlighting
    CLike,
}

pub enum Message {
    ViewMessage(ViewMessage),
    DecodedKey(DecodedKey),
    CommandMessage(CommandMessage),
}
