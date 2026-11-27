use ime::KeyEvent;

/// Read a raw event from the keyboard.
pub fn read_raw(blocking: bool) -> Option<KeyEvent> {
    ime::read_raw(blocking).ok().flatten()
}
