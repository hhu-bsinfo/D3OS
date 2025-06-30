use crate::{
    event::event_handler::Error,
    modules::parser::token::{ArgumentKind, TokenContext, TokenContextFactory, TokenKind},
};

const MORE_THAN_ONE_CMD_IN_SEGMENT_ERROR: Error = Error::new(
    "Invalid command line",
    Some("Can not handle more than one command per segment"),
);

const COMMAND_INSTEAD_OF_FILE_ERROR: Error =
    Error::new("Invalid command line", Some("Expected a filename but got command"));

const COMMAND_AFTER_BACKGROUND_ERROR: Error = Error::new("Invalid command line", Some("Expected end of line"));

pub struct CommandTokenContextFactory {}

impl TokenContextFactory for CommandTokenContextFactory {
    fn create_first(_kind: &TokenKind, _ch: char) -> TokenContext {
        TokenContext {
            pos: 0,
            cmd_pos: Some(0),
            short_flag_pos: None,
            in_quote: None,
            arg_kind: ArgumentKind::None,
            error: None,
            require_cmd: false,
            require_file: false,
            has_background: false,
        }
    }

    fn create_after(prev_clx: &TokenContext, _kind: &TokenKind, _ch: char) -> TokenContext {
        let error = prev_clx.error.or_else(|| {
            if prev_clx.cmd_pos.is_some() {
                Some(&MORE_THAN_ONE_CMD_IN_SEGMENT_ERROR)
            } else if prev_clx.require_file {
                Some(&COMMAND_INSTEAD_OF_FILE_ERROR)
            } else if prev_clx.has_background {
                Some(&COMMAND_AFTER_BACKGROUND_ERROR)
            } else {
                None
            }
        });

        TokenContext {
            pos: prev_clx.pos + 1,
            cmd_pos: Some(prev_clx.pos + 1),
            short_flag_pos: None,
            in_quote: prev_clx.in_quote,
            arg_kind: ArgumentKind::None,
            error,
            require_cmd: false,
            require_file: false,
            has_background: prev_clx.has_background,
        }
    }
}
