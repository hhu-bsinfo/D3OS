use terminal::DecodedKey;

pub enum ViewMessage {
    // represent the chars to move up or down (not the lines)
    ScrollDown(u32),
    ScrollUp(u32),
}

pub enum Message {
    ViewMessage(ViewMessage),
    DecodedKey(DecodedKey),
}
