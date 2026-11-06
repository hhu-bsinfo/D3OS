use alloc::vec;
use alloc::vec::Vec;
use graphic::{
    buffered_lfb::BufferedLFB,
    color::{self, Color},
    lfb::{self, LFB},
};

/// A presented character cell: the framebuffer renderer's snapshot of what is
/// currently on screen, used to restore the cell under a blinking cursor.
/// `width` mirrors the model's cell span, so the cursor overlay can leave the
/// continuation cells of a wide glyph alone.
#[derive(Copy, Clone)]
pub struct Character {
    pub value: char,
    pub fg_color: Color,
    pub bg_color: Color,
    pub width: u8,
}

impl Character {
    pub const BLANK: Character = Character {
        value: '\0',
        fg_color: color::WHITE,
        bg_color: color::BLACK,
        width: 1,
    };
}

/// Framebuffer presentation state. This owns only what is needed to draw the
/// active session's semantic model onto the screen; the authoritative terminal
/// state (grid, cursor, colors, parser) lives per session in `TerminalModel`.
pub struct DisplayState {
    /// Full framebuffer grid size in cells: `(cols, rows)`. Row 0 is the status
    /// bar; content is drawn on rows `1..rows`.
    pub(crate) size: (u16, u16),
    pub(crate) lfb: BufferedLFB,
    /// Snapshot of the presented cells (`cols * rows`), row-major.
    pub(crate) visible: Vec<Character>,
    /// Grid position of the cursor overlay.
    pub(crate) cursor_pos: (u16, u16),
    /// Whether the cursor block is currently drawn over `cursor_pos`.
    pub(crate) cursor_visible: bool,
    pub(crate) tab_ids: Vec<u8>,
    pub(crate) active_tab: u8,
}

impl DisplayState {
    pub fn new(buffer: *mut u8, pitch: u32, width: u32, height: u32, bpp: u8) -> Self {
        let raw_lfb = LFB::new(buffer, pitch, width, height, bpp);
        let mut lfb = BufferedLFB::new(raw_lfb);
        let size = (
            (width / lfb::DEFAULT_CHAR_WIDTH) as u16,
            (height / lfb::DEFAULT_CHAR_HEIGHT) as u16,
        );

        let cell_count = size.0 as usize * size.1 as usize;
        let visible = vec![Character::BLANK; cell_count];

        lfb.lfb().clear();
        lfb.flush();

        Self {
            size,
            lfb,
            visible,
            cursor_pos: (0, 1),
            cursor_visible: false,
            tab_ids: Vec::new(),
            active_tab: u8::MAX,
        }
    }
}
