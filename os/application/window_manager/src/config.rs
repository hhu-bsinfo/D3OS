use graphic::color::{Color, HHU_BLUE, WHITE, YELLOW};

pub const INTERACT_BUTTON: char = 'f';
pub const BACKSPACE_UNICODE: char = '\u{0008}';

pub const DEFAULT_FG_COLOR: Color = WHITE;
pub const FOCUSED_FG_COLOR: Color = YELLOW;
pub const FOCUSED_BG_COLOR: Color = HHU_BLUE;

pub const DIST_TO_SCREEN_EDGE: u32 = 10;
pub const COMMAND_LINE_WINDOW_Y_PADDING: u32 = 2;
pub const DEFAULT_FONT_SCALE: (u32, u32) = (1, 1);

pub const PADDING_BORDERS_AND_CHARS: u32 = 2;

pub const FLUSHING_DELAY_MS: u32 = 10;
