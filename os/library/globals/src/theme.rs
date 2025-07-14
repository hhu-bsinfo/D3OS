#[derive(Debug, Clone)]
pub struct ThemeRegistry {
    pub default: &'static Theme,
    pub themes: &'static [Theme],
}

#[derive(Debug, Clone)]
pub struct Theme {
    pub id: &'static str,
    // Status
    pub indicator: &'static str,
    pub indicator_warning: &'static str,
    pub indicator_error: &'static str,
    pub error_msg: &'static str,
    pub error_hint: &'static str,
    // quotes
    pub quote_start: &'static str,
    pub quote_end: &'static str,
    pub in_quote: &'static str,
    // Redirection
    pub redirection_in_truncate: &'static str,
    pub redirection_in_append: &'static str,
    pub redirection_out_truncate: &'static str,
    pub redirection_out_append: &'static str,
    // Condition
    pub logical_or: &'static str,
    pub logical_and: &'static str,
    // Other
    pub cmd: &'static str,
    pub arg: &'static str,
    pub file: &'static str,
    pub pipe: &'static str,
    pub background: &'static str,
    pub separator: &'static str,
    pub suggestion: &'static str,
}

const DEFAULT: &'static str = "";
const LIME: &'static str = "\x1b[38;2;0;255;0m";
const LIME_ACCENT: &'static str = "\x1b[38;2;0;200;0m";
const GOLD: &'static str = "\x1b[38;2;255;215;0m";
const PALE_BLUE: &'static str = "\x1b[38;2;192;192;255m";
const GRAY: &'static str = "\x1b[38;2;128;128;128m";
const RED: &'static str = "\x1b[38;2;255;0;0m";
const MUTED_RED: &'static str = "\x1b[38;2;200;80;80m";
const ORANGE: &'static str = "\x1b[38;2;255;165;0m";
const PURPLE: &'static str = "\x1b[38;2;128;0;128m";
const YELLOW: &'static str = "\x1b[38;2;255;255;0m";
const TAN: &'static str = "\x1b[38;2;210;180;140m";
const D3OS_BLUE: &'static str = "\x1b[38;2;0;106;179m";
const D3OS_GREEN: &'static str = "\x1b[38;2;140;177;16m";

pub const DEBUG_THEME: Theme = Theme {
    id: "debug",
    // Status
    indicator: DEFAULT,
    indicator_warning: YELLOW,
    indicator_error: RED,
    error_msg: RED,
    error_hint: MUTED_RED,
    // quotes
    quote_start: LIME_ACCENT,
    quote_end: LIME_ACCENT,
    in_quote: LIME,
    // Redirection
    redirection_in_truncate: ORANGE,
    redirection_in_append: PURPLE,
    redirection_out_truncate: ORANGE,
    redirection_out_append: PURPLE,
    // Condition
    logical_or: PURPLE,
    logical_and: PURPLE,
    // Other
    cmd: GOLD,
    arg: PALE_BLUE,
    file: TAN,
    pipe: ORANGE,
    background: ORANGE,
    separator: ORANGE,
    suggestion: GRAY,
};

pub const D3OS_THEME: Theme = Theme {
    id: "d3os",
    // Status
    indicator: DEFAULT,
    indicator_warning: YELLOW,
    indicator_error: RED,
    error_msg: RED,
    error_hint: MUTED_RED,
    // quotes
    quote_start: LIME_ACCENT,
    quote_end: LIME_ACCENT,
    in_quote: LIME,
    // Redirection
    redirection_in_truncate: DEFAULT,
    redirection_in_append: DEFAULT,
    redirection_out_truncate: DEFAULT,
    redirection_out_append: DEFAULT,
    // Condition
    logical_or: DEFAULT,
    logical_and: DEFAULT,
    // Other
    cmd: D3OS_BLUE,
    arg: D3OS_GREEN,
    file: PALE_BLUE,
    pipe: DEFAULT,
    background: DEFAULT,
    separator: DEFAULT,
    suggestion: GRAY,
};

pub const BORING_THEME: Theme = Theme {
    id: "boring",
    // Status
    indicator: DEFAULT,
    indicator_warning: YELLOW,
    indicator_error: RED,
    error_msg: RED,
    error_hint: MUTED_RED,
    // quotes
    quote_start: DEFAULT,
    quote_end: DEFAULT,
    in_quote: DEFAULT,
    // Redirection
    redirection_in_truncate: DEFAULT,
    redirection_in_append: DEFAULT,
    redirection_out_truncate: DEFAULT,
    redirection_out_append: DEFAULT,
    // Condition
    logical_or: DEFAULT,
    logical_and: DEFAULT,
    // Other
    cmd: DEFAULT,
    arg: DEFAULT,
    file: DEFAULT,
    pipe: DEFAULT,
    background: DEFAULT,
    separator: DEFAULT,
    suggestion: GRAY,
};

pub const THEME_REGISTRY: ThemeRegistry = ThemeRegistry {
    default: &D3OS_THEME,
    themes: &[D3OS_THEME, BORING_THEME, DEBUG_THEME],
};
