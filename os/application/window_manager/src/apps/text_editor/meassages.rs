use terminal::DecodedKey;

pub enum ViewMessage{
    ScrollDown(u32),
    ScrollUp(u32),
}

pub enum Message{
    ViewMessage(ViewMessage),
    DecodedKey(DecodedKey),
}