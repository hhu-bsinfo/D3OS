#![allow(dead_code)]

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
    // Arguments
    pub generic_arg: &'static str,
    pub short_flag_arg: &'static str,
    pub short_flag_value_arg: &'static str,
    pub long_flag_arg: &'static str,
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
    pub file: &'static str,
    pub pipe: &'static str,
    pub background: &'static str,
    pub separator: &'static str,
    pub suggestion: &'static str,
}

const DEFAULT: &'static str = "";
const WHITE: &'static str = "\x1b[38;2;255;255;255m";
const LIME: &'static str = "\x1b[38;2;0;255;0m";
const GOLD: &'static str = "\x1b[38;2;255;215;0m";
const PALE_BLUE: &'static str = "\x1b[38;2;192;192;255m";
const BLUE: &'static str = "\x1b[38;2;64;64;255m";
const VIVID_BLUE: &'static str = "\x1b[38;2;0;0;255m";
const DEEP_BLUE: &'static str = "\x1b[38;2;0;0;200m";
const GRAY: &'static str = "\x1b[38;2;128;128;128m";
const RED: &'static str = "\x1b[38;2;255;0;0m";
const MUTED_RED: &'static str = "\x1b[38;2;200;80;80m";
const ORANGE: &'static str = "\x1b[38;2;255;165;0m";
const PURPLE: &'static str = "\x1b[38;2;128;0;128m";
const YELLOW: &'static str = "\x1b[38;2;255;255;0m";
const TAN: &'static str = "\x1b[38;2;210;180;140m";

pub const DEBUG_THEME: Theme = Theme {
    id: "debug",
    // Status
    indicator: DEFAULT,
    indicator_warning: YELLOW,
    indicator_error: RED,
    error_msg: RED,
    error_hint: MUTED_RED,
    // quotes
    quote_start: LIME,
    quote_end: LIME,
    in_quote: LIME,
    // Arguments
    generic_arg: PALE_BLUE,
    short_flag_arg: BLUE,
    short_flag_value_arg: VIVID_BLUE,
    long_flag_arg: DEEP_BLUE,
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
    file: TAN,
    pipe: ORANGE,
    background: ORANGE,
    separator: ORANGE,
    suggestion: GRAY,
};

pub const THEME_REGISTRY: ThemeRegistry = ThemeRegistry {
    default: &DEBUG_THEME,
    themes: &[DEBUG_THEME],
};
