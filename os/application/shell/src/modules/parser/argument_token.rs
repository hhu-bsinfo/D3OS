use crate::{
    event::event_handler::Error,
    modules::parser::token::{ArgumentKind, TokenContext, TokenContextFactory, TokenKind},
};

pub struct ArgumentTokenContextFactory {}

const ARGUMENT_INSTEAD_OF_FILE_ERROR: Error =
    Error::new("Invalid command line", Some("Expected a filename but got argument"));

const ARGUMENT_AFTER_BACKGROUND_ERROR: Error = Error::new("Invalid command line", Some("Expected end of line"));

impl TokenContextFactory for ArgumentTokenContextFactory {
    fn create_first(_kind: &TokenKind, _ch: char) -> TokenContext {
        panic!("The first token can not be a argument");
    }

    fn create_after(prev_clx: &TokenContext, _kind: &TokenKind, ch: char) -> TokenContext {
        let error = prev_clx.error.or_else(|| {
            if prev_clx.require_file {
                Some(&ARGUMENT_INSTEAD_OF_FILE_ERROR)
            } else if prev_clx.has_background {
                Some(&ARGUMENT_AFTER_BACKGROUND_ERROR)
            } else {
                None
            }
        });
        let arg_kind: ArgumentKind;
        let short_flag_pos: Option<usize>;

        if prev_clx.arg_kind == ArgumentKind::ShortFlag {
            arg_kind = ArgumentKind::ShortFlagValue;
            short_flag_pos = prev_clx.short_flag_pos;
        } else if ch == '-' {
            arg_kind = ArgumentKind::ShortOrLongFlag;
            short_flag_pos = None;
        } else {
            arg_kind = ArgumentKind::Generic;
            short_flag_pos = None;
        };

        TokenContext {
            pos: prev_clx.pos + 1,
            cmd_pos: prev_clx.cmd_pos,
            short_flag_pos,
            in_quote: prev_clx.in_quote,
            arg_kind,
            error,
            require_cmd: false,
            require_file: false,
            has_background: prev_clx.has_background,
        }
    }

    fn revalidate(clx: &mut TokenContext, _kind: &TokenKind, string: &str) {
        if clx.arg_kind == ArgumentKind::ShortFlagValue {
            return;
        }

        if string == "-" {
            clx.arg_kind = ArgumentKind::ShortOrLongFlag;
            clx.short_flag_pos = None;
            return;
        }
        if string.starts_with("--") {
            clx.arg_kind = ArgumentKind::LongFlag;
            clx.short_flag_pos = None;
            return;
        }
        if string.starts_with("-") {
            clx.arg_kind = ArgumentKind::ShortFlag;
            clx.short_flag_pos = Some(clx.pos);
            return;
        }
        clx.arg_kind = ArgumentKind::Generic;
        clx.short_flag_pos = None;
    }
}
