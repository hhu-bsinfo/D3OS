use crate::token::token::Token;

pub struct TokenRule {
    pub condition: fn(prev: &Token) -> bool,
    pub reason: &'static str,
}

#[derive(Debug)]
pub struct FirstTokenDTO {
    pub cmd_pos: Option<usize>,
    pub in_quote: Option<char>,
    pub require_cmd: bool,
    pub require_file: bool,
    pub has_background: bool,
    pub error_reason: Option<&'static str>,
}

#[derive(Debug)]
pub struct NextTokenDTO {
    pub cmd_pos: Option<Option<usize>>,
    pub in_quote: Option<Option<char>>,
    pub require_cmd: Option<bool>,
    pub require_file: Option<bool>,
    pub has_background: Option<bool>,
}

pub struct TokenDefinition {
    pub first_token_fn: fn(content: &str) -> FirstTokenDTO,
    pub next_token_fn: fn(prev: &Token, content: &str) -> NextTokenDTO,
    pub error_rules: &'static [TokenRule],
}

pub const COMMAND_TOKEN_DEFINITION: TokenDefinition = TokenDefinition {
    first_token_fn: |_content| FirstTokenDTO {
        cmd_pos: Some(0),
        in_quote: None,
        require_cmd: false,
        require_file: false,
        has_background: false,
        error_reason: None,
    },
    next_token_fn: |prev, _content| NextTokenDTO {
        cmd_pos: Some(Some(prev.clx().pos + 1)),
        require_file: None,
        require_cmd: Some(false),
        has_background: None,
        in_quote: None,
    },
    error_rules: &[
        TokenRule {
            condition: |prev| prev.clx().cmd_pos.is_some(),
            reason: "Only one command per segment is allowed",
        },
        TokenRule {
            condition: |prev| prev.clx().require_file,
            reason: "Expected a filename but got command",
        },
        TokenRule {
            condition: |prev| prev.clx().has_background,
            reason: "Expected end of line but got command",
        },
    ],
};

pub const ARGUMENT_TOKEN_DEFINITION: TokenDefinition = TokenDefinition {
    first_token_fn: |_content| FirstTokenDTO {
        cmd_pos: None,
        in_quote: None,
        require_cmd: false,
        require_file: false,
        has_background: false,
        error_reason: Some("Segment must have a command to support arguments"),
    },
    next_token_fn: |_prev, _content| NextTokenDTO {
        cmd_pos: None,
        require_file: None,
        require_cmd: None,
        has_background: None,
        in_quote: None,
    },
    error_rules: &[
        TokenRule {
            condition: |prev| prev.clx().cmd_pos.is_none(),
            reason: "Segment must have a command to support arguments",
        },
        TokenRule {
            condition: |prev| prev.clx().require_file,
            reason: "Expected a filename but got argument",
        },
        TokenRule {
            condition: |prev| prev.clx().has_background,
            reason: "Expected end of line but got argument",
        },
    ],
};

pub const FILE_TOKEN_DEFINITION: TokenDefinition = TokenDefinition {
    first_token_fn: |_content| FirstTokenDTO {
        cmd_pos: None,
        in_quote: None,
        require_cmd: false,
        require_file: false,
        has_background: false,
        error_reason: Some("Did not expect a file"),
    },
    next_token_fn: |_prev, _content| NextTokenDTO {
        cmd_pos: None,
        require_file: Some(false),
        require_cmd: None,
        has_background: None,
        in_quote: None,
    },
    error_rules: &[
        TokenRule {
            condition: |prev| !prev.clx().require_file,
            reason: "Did not expect a file",
        },
        TokenRule {
            condition: |prev| prev.clx().require_cmd,
            reason: "Expected a command but got file",
        },
        TokenRule {
            condition: |prev| prev.clx().has_background,
            reason: "Expected end of line but got file",
        },
    ],
};

pub const BLANK_TOKEN_DEFINITION: TokenDefinition = TokenDefinition {
    first_token_fn: |_content| FirstTokenDTO {
        cmd_pos: None,
        in_quote: None,
        require_cmd: false,
        require_file: false,
        has_background: false,
        error_reason: None,
    },
    next_token_fn: |_prev, _content| NextTokenDTO {
        cmd_pos: None,
        require_file: None,
        require_cmd: None,
        has_background: None,
        in_quote: None,
    },
    error_rules: &[],
};

pub const QUOTE_START_TOKEN_DEFINITION: TokenDefinition = TokenDefinition {
    first_token_fn: |content| FirstTokenDTO {
        cmd_pos: None,
        in_quote: Some(
            content
                .chars()
                .next()
                .expect("Expected token to have at least one char"),
        ),
        require_cmd: false,
        require_file: false,
        has_background: false,
        error_reason: None,
    },
    next_token_fn: |_prev, content| NextTokenDTO {
        cmd_pos: None,
        require_file: None,
        require_cmd: None,
        has_background: None,
        in_quote: Some(Some(
            content
                .chars()
                .next()
                .expect("Expected token to have at least one char"),
        )),
    },
    error_rules: &[TokenRule {
        condition: |prev| prev.clx().in_quote.is_some(),
        reason: "Nesting quotes is not supported",
    }],
};

pub const QUOTE_END_TOKEN_DEFINITION: TokenDefinition = TokenDefinition {
    first_token_fn: |_content| FirstTokenDTO {
        cmd_pos: None,
        in_quote: None,
        require_cmd: false,
        require_file: false,
        has_background: false,
        error_reason: Some("Unable to close nonexisting quote"),
    },
    next_token_fn: |_prev, _content| NextTokenDTO {
        cmd_pos: None,
        require_file: None,
        require_cmd: None,
        has_background: None,
        in_quote: Some(None),
    },
    error_rules: &[TokenRule {
        condition: |prev| prev.clx().in_quote.is_none(),
        reason: "Unable to close nonexisting quote",
    }],
};

pub const PIPE_TOKEN_DEFINITION: TokenDefinition = TokenDefinition {
    first_token_fn: |_content| FirstTokenDTO {
        cmd_pos: None,
        in_quote: None,
        require_cmd: true,
        require_file: false,
        has_background: false,
        error_reason: Some(
            "If you want to use a pipe, try moving | between commands (Example: cmd1 | cmd2)\nIf you want | as normal char, try wrapping it in parentheses (Example: echo 'No | pipe')",
        ),
    },
    next_token_fn: |_prev, _content| NextTokenDTO {
        cmd_pos: Some(None),
        require_file: None,
        require_cmd: Some(true),
        has_background: None,
        in_quote: None,
    },
    error_rules: &[
        TokenRule {
            condition: |prev| prev.clx().cmd_pos.is_none(),
            reason: "If you want to use a pipe, try moving | between commands (Example: cmd1 | cmd2)\nIf you want | as normal char, try wrapping it in parentheses (Example: echo 'No | pipe')",
        },
        TokenRule {
            condition: |prev| prev.clx().require_file,
            reason: "Expected a filename but got |",
        },
        TokenRule {
            condition: |prev| prev.clx().has_background,
            reason: "Expected end of line but got |",
        },
    ],
};

pub const REDIRECT_IN_TRUNCATE_TOKEN_DEFINITION: TokenDefinition = TokenDefinition {
    first_token_fn: |_content| FirstTokenDTO {
        cmd_pos: None,
        in_quote: None,
        require_cmd: false,
        require_file: true,
        has_background: false,
        error_reason: Some(
            "If you want to redirect some input, try moving < after a command (Example: cmd1 < file)\nIf you want < as normal char, try wrapping it in parentheses (Example: echo 'No < redirection')",
        ),
    },
    next_token_fn: |_prev, _content| NextTokenDTO {
        cmd_pos: Some(None),
        require_file: Some(true),
        require_cmd: None,
        has_background: None,
        in_quote: None,
    },
    error_rules: &[
        TokenRule {
            condition: |prev| prev.clx().cmd_pos.is_none(),
            reason: "If you want to redirect some input, try moving < after a command (Example: cmd1 < file)\nIf you want < as normal char, try wrapping it in parentheses (Example: echo 'No < redirection')",
        },
        TokenRule {
            condition: |prev| prev.clx().require_file,
            reason: "Expected a filename but got <",
        },
        TokenRule {
            condition: |prev| prev.clx().has_background,
            reason: "Expected end of line but got <",
        },
    ],
};

pub const REDIRECT_IN_APPEND_TOKEN_DEFINITION: TokenDefinition = TokenDefinition {
    first_token_fn: |_content| FirstTokenDTO {
        cmd_pos: None,
        in_quote: None,
        require_cmd: false,
        require_file: true,
        has_background: false,
        error_reason: Some(
            "If you want to redirect some input, try moving << after a command (Example: cmd1 << file)\nIf you want << as normal char, try wrapping it in parentheses (Example: echo 'No << redirection')",
        ),
    },
    next_token_fn: |_prev, _content| NextTokenDTO {
        cmd_pos: Some(None),
        require_file: Some(true),
        require_cmd: None,
        has_background: None,
        in_quote: None,
    },
    error_rules: &[
        TokenRule {
            condition: |prev| prev.clx().cmd_pos.is_none(),
            reason: "If you want to redirect some input, try moving << after a command (Example: cmd1 << file)\nIf you want << as normal char, try wrapping it in parentheses (Example: echo 'No << redirection')",
        },
        TokenRule {
            condition: |prev| prev.clx().require_file,
            reason: "Expected a filename but got <<",
        },
        TokenRule {
            condition: |prev| prev.clx().has_background,
            reason: "Expected end of line but got <<",
        },
    ],
};

pub const REDIRECT_OUT_TRUNCATE_TOKEN_DEFINITION: TokenDefinition = TokenDefinition {
    first_token_fn: |_content| FirstTokenDTO {
        cmd_pos: None,
        in_quote: None,
        require_cmd: false,
        require_file: true,
        has_background: false,
        error_reason: Some(
            "If you want to redirect some input, try moving > after a command (Example: cmd1 > file)\nIf you want > as normal char, try wrapping it in parentheses (Example: echo 'No > redirection')",
        ),
    },
    next_token_fn: |_prev, _content| NextTokenDTO {
        cmd_pos: Some(None),
        require_file: Some(true),
        require_cmd: None,
        has_background: None,
        in_quote: None,
    },
    error_rules: &[
        TokenRule {
            condition: |prev| prev.clx().cmd_pos.is_none(),
            reason: "If you want to redirect some input, try moving > after a command (Example: cmd1 > file)\nIf you want > as normal char, try wrapping it in parentheses (Example: echo 'No > redirection')",
        },
        TokenRule {
            condition: |prev| prev.clx().require_file,
            reason: "Expected a filename but got >",
        },
        TokenRule {
            condition: |prev| prev.clx().has_background,
            reason: "Expected end of line but got >",
        },
    ],
};

pub const REDIRECT_OUT_APPEND_TOKEN_DEFINITION: TokenDefinition = TokenDefinition {
    first_token_fn: |_content| FirstTokenDTO {
        cmd_pos: None,
        in_quote: None,
        require_cmd: false,
        require_file: true,
        has_background: false,
        error_reason: Some(
            "If you want to redirect some input, try moving >> after a command (Example: cmd1 >> file)\nIf you want >> as normal char, try wrapping it in parentheses (Example: echo 'No >> redirection')",
        ),
    },
    next_token_fn: |_prev, _content| NextTokenDTO {
        cmd_pos: Some(None),
        require_file: Some(true),
        require_cmd: None,
        has_background: None,
        in_quote: None,
    },
    error_rules: &[
        TokenRule {
            condition: |prev| prev.clx().cmd_pos.is_none(),
            reason: "If you want to redirect some input, try moving >> after a command (Example: cmd1 >> file)\nIf you want >> as normal char, try wrapping it in parentheses (Example: echo 'No >> redirection')",
        },
        TokenRule {
            condition: |prev| prev.clx().require_file,
            reason: "Expected a filename but got >>",
        },
        TokenRule {
            condition: |prev| prev.clx().has_background,
            reason: "Expected end of line but got >>",
        },
    ],
};

pub const LOGICAL_AND_TOKEN_DEFINITION: TokenDefinition = TokenDefinition {
    first_token_fn: |_content| FirstTokenDTO {
        cmd_pos: None,
        in_quote: None,
        require_cmd: true,
        require_file: false,
        has_background: false,
        error_reason: Some(
            "If you want to use a and condition, try moving && between commands (Example: cmd1 && cmd2)\nIf you want && as normal char, try wrapping it in parentheses (Example: echo 'No && condition')",
        ),
    },
    next_token_fn: |_prev, _content| NextTokenDTO {
        cmd_pos: Some(None),
        require_file: None,
        require_cmd: Some(true),
        has_background: None,
        in_quote: None,
    },
    error_rules: &[
        TokenRule {
            condition: |prev| prev.clx().cmd_pos.is_none(),
            reason: "If you want to use a and condition, try moving && between commands (Example: cmd1 && cmd2)\nIf you want && as normal char, try wrapping it in parentheses (Example: echo 'No && condition')",
        },
        TokenRule {
            condition: |prev| prev.clx().require_file,
            reason: "Expected a filename but got &&",
        },
        TokenRule {
            condition: |prev| prev.clx().has_background,
            reason: "Expected end of line but got &&",
        },
    ],
};

pub const LOGICAL_OR_TOKEN_DEFINITION: TokenDefinition = TokenDefinition {
    first_token_fn: |_content| FirstTokenDTO {
        cmd_pos: None,
        in_quote: None,
        require_cmd: true,
        require_file: false,
        has_background: false,
        error_reason: Some(
            "If you want to use a or condition, try moving || between commands (Example: cmd1 || cmd2)\nIf you want && as normal char, try wrapping it in parentheses (Example: echo 'No || condition')",
        ),
    },
    next_token_fn: |_prev, _content| NextTokenDTO {
        cmd_pos: Some(None),
        require_file: None,
        require_cmd: Some(true),
        has_background: None,
        in_quote: None,
    },
    error_rules: &[
        TokenRule {
            condition: |prev| prev.clx().cmd_pos.is_none(),
            reason: "If you want to use a or condition, try moving || between commands (Example: cmd1 || cmd2)\nIf you want && as normal char, try wrapping it in parentheses (Example: echo 'No || condition')",
        },
        TokenRule {
            condition: |prev| prev.clx().require_file,
            reason: "Expected a filename but got ||",
        },
        TokenRule {
            condition: |prev| prev.clx().has_background,
            reason: "Expected end of line but got ||",
        },
    ],
};

pub const SEPARATOR_TOKEN_DEFINITION: TokenDefinition = TokenDefinition {
    first_token_fn: |_content| FirstTokenDTO {
        cmd_pos: None,
        in_quote: None,
        require_cmd: false,
        require_file: false,
        has_background: false,
        error_reason: None,
    },
    next_token_fn: |_prev, _content| NextTokenDTO {
        cmd_pos: Some(None),
        require_file: None,
        require_cmd: None,
        has_background: None,
        in_quote: None,
    },
    error_rules: &[TokenRule {
        condition: |prev| prev.clx().require_file,
        reason: "Expected a filename but got ;",
    }],
};

pub const BACKGROUND_TOKEN_DEFINITION: TokenDefinition = TokenDefinition {
    first_token_fn: |_content| FirstTokenDTO {
        cmd_pos: None,
        in_quote: None,
        require_cmd: false,
        require_file: false,
        has_background: true,
        error_reason: Some(
            "If you want to use a background execution, try moving & after the command (Example: cmd1 arg1 arg2 &)\nIf you want & as normal char, try wrapping it in parentheses (Example: echo 'No & background execution')",
        ),
    },
    next_token_fn: |_prev, _content| NextTokenDTO {
        cmd_pos: Some(None),
        require_file: None,
        require_cmd: None,
        has_background: Some(true),
        in_quote: None,
    },
    error_rules: &[
        TokenRule {
            condition: |prev| prev.clx().cmd_pos.is_none(),
            reason: "If you want to use a background execution, try moving & after the command (Example: cmd1 arg1 arg2 &)\nIf you want & as normal char, try wrapping it in parentheses (Example: echo 'No & background execution')",
        },
        TokenRule {
            condition: |prev| prev.clx().require_file,
            reason: "Expected a filename but got &",
        },
        TokenRule {
            condition: |prev| prev.clx().require_cmd,
            reason: "Expected a command but got &",
        },
        TokenRule {
            condition: |prev| prev.clx().has_background,
            reason: "Expected end of line but got &",
        },
    ],
};
