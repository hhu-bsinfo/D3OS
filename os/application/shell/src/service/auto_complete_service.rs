use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use globals::application::{APPLICATION_REGISTRY, Application};
use logger::warn;

use crate::{
    context::Context,
    service::{
        lexer_service::{AmbiguousState, ArgumentType, FindLastCommand, Token, TokenType},
        service::{Error, Response, Service},
    },
};

#[derive(Debug)]
pub struct AutoCompleteService {
    applications: Vec<Application>,
    current_index: usize,
    current_app: Option<Application>,
    current_suggestion: Option<String>,
}

impl Service for AutoCompleteService {
    fn auto_complete(&mut self, context: &mut Context) -> Result<Response, Error> {
        warn!("{:?}", self.current_app);
        if context.auto_completion.has_focus() {
            return self.cycle(context);
        }
        if context.auto_completion.is_empty() {
            self.activate(context);
            return self.cycle(context);
        }

        self.activate(context)
    }

    fn simple_key(&mut self, context: &mut Context, key: char) -> Result<Response, Error> {
        match key {
            ' ' => self.adopt(context),
            '\x08' | '\x7F' => {
                self.revalidate_application(context);
                self.restore(context)
            }
            _ => {
                self.revalidate_application(context);
                self.restore(context);
                self.cycle(context)
            }
        }
    }

    fn prepare(&mut self, context: &mut Context) -> Result<Response, Error> {
        self.reset(context)
    }

    fn submit(&mut self, context: &mut Context) -> Result<Response, Error> {
        self.restore(context)
    }

    fn cursor_left(&mut self, context: &mut Context) -> Result<Response, Error> {
        self.restore(context)
    }

    fn cursor_right(&mut self, context: &mut Context) -> Result<Response, Error> {
        self.restore(context)
    }

    fn history_down(&mut self, context: &mut Context) -> Result<Response, Error> {
        self.reset(context)
    }

    fn history_up(&mut self, context: &mut Context) -> Result<Response, Error> {
        self.reset(context)
    }
}

impl AutoCompleteService {
    pub fn new() -> Self {
        Self {
            applications: Vec::from(APPLICATION_REGISTRY.applications),
            current_index: 0,
            current_app: None,
            current_suggestion: None,
        }
    }

    fn adopt(&mut self, context: &mut Context) -> Result<Response, Error> {
        if !context.is_cursor_at_end() {
            return Ok(Response::Skip);
        }

        if context
            .tokens
            .last()
            .is_some_and(|token| token.token_type() == TokenType::Command)
        {
            self.current_app = Some(self.applications[self.current_index].clone());
        }

        self.current_index = 0;

        let intercept_char = context
            .line
            .pop()
            .expect("Expected at least one char in line");
        context.line.push_str(&context.auto_completion.get());
        context.line.push(intercept_char);
        context.cursor_position += context.auto_completion.len();
        context.auto_completion.reset();

        Ok(Response::Ok)
    }

    fn restore(&mut self, context: &mut Context) -> Result<Response, Error> {
        if context.auto_completion.is_empty() {
            return Ok(Response::Skip);
        }
        self.current_index = 0;

        context.auto_completion.reset();
        Ok(Response::Ok)
    }

    fn reset(&mut self, context: &mut Context) -> Result<Response, Error> {
        self.restore(context);
        self.current_app = None;
        Ok(Response::Ok)
    }

    fn activate(&mut self, context: &mut Context) -> Result<Response, Error> {
        if context.tokens.last().is_none() {
            return Ok(Response::Skip);
        }
        if self
            .current_suggestion
            .as_ref()
            .is_some_and(|suggestion| *suggestion == context.tokens.last().unwrap().to_string())
        {
            return Ok(Response::Skip);
        }

        context.auto_completion.focus();
        Ok(Response::Ok)
    }

    fn revalidate_application(&mut self, context: &mut Context) {
        let last_command = match context.tokens.find_last_command() {
            Some(command) => command,
            None => {
                // self.reset(context);
                self.current_app = None;
                return;
            }
        };

        // If no changes to last command => Do nothing
        if self
            .current_app
            .as_ref()
            .is_some_and(|app| app.command == last_command.to_string())
        {
            return;
        }

        // Else => Try find matching application
        let found = self.cycle_app(&last_command.to_string()).cloned();
        warn!("{:?}", found);
        self.current_suggestion = match &found {
            Some(app) => Some(app.command.to_string()),
            None => None,
        };
        self.current_app = found;
    }

    fn cycle(&mut self, context: &mut Context) -> Result<Response, Error> {
        if !context.is_cursor_at_end() {
            self.revalidate_application(context);
            return Ok(Response::Ok);
        }

        let suggestion = match context.tokens.last().cloned() {
            None => self.cycle_command(&String::new()),
            Some(Token::Command(_, cmd)) => self.cycle_command(&cmd),
            Some(Token::Argument(_, arg_type, arg)) => self.cycle_argument(Some(&arg_type), &arg),
            Some(Token::Whitespace(clx)) => match clx.ambiguous {
                AmbiguousState::Pending => self.cycle_command(&String::new()),
                AmbiguousState::Command => self.cycle_argument(None, &String::new()),
                AmbiguousState::Argument => self.cycle_argument(None, &String::new()),
            },
            _ => None,
        };
        self.current_suggestion = suggestion.clone();

        if suggestion.is_none() {
            return Ok(Response::Skip);
        }

        context.auto_completion.set(&suggestion.unwrap());
        Ok(Response::Ok)
    }

    fn cycle_app(&mut self, cmd: &String) -> Option<&Application> {
        self.applications
            .iter()
            .enumerate()
            .cycle()
            .skip(self.current_index + 1)
            .take(self.applications.len())
            .find_map(|(i, app)| {
                if !app.command.starts_with(cmd) {
                    self.current_app = None;
                    return None;
                }

                self.current_app = Some(app.clone());
                self.current_index = i;
                Some(app)
            })
    }

    fn cycle_command(&mut self, cmd: &String) -> Option<String> {
        let complete_cmd = match self.cycle_app(cmd) {
            Some(app) => &app.command[cmd.len()..],
            None => return None,
        };

        if complete_cmd.is_empty() {
            return None;
        }

        Some(complete_cmd.to_string())
    }

    fn cycle_argument(&mut self, arg_type: Option<&ArgumentType>, arg: &String) -> Option<String> {
        let found_arg = match arg_type {
            Some(ArgumentType::Generic) => self.cycle_generic_argument(arg),
            Some(ArgumentType::ShortFlag) => panic!("Not short flag auto complete not implemented"),
            Some(ArgumentType::LongFlag) => panic!("Not long flag auto complete not implemented"),
            Some(ArgumentType::LongFlagValue) => {
                panic!("Not long flag value auto complete not implemented")
            }
            None => self.cycle_all_arguments(arg),
        };

        let complete_arg = match found_arg {
            Some(found_arg) => &found_arg[arg.len()..].to_string(),
            None => return None,
        };

        if complete_arg.is_empty() {
            return None;
        }

        Some(complete_arg.to_string())
    }

    fn cycle_all_arguments(&mut self, arg: &String) -> Option<String> {
        if let Some(found_arg) = self.cycle_generic_argument(arg) {
            return Some(found_arg);
        }

        // TODO other arg types

        None
    }

    fn cycle_generic_argument(&mut self, arg: &String) -> Option<String> {
        if self.current_app.is_none() {
            return None;
        }
        let sub_commands = self.current_app.as_mut().unwrap().sub_commands;
        sub_commands
            .iter()
            .enumerate()
            .cycle()
            .skip(self.current_index + 1)
            .take(sub_commands.len())
            .find_map(|(i, &sub_command)| {
                if sub_command.starts_with(arg) {
                    self.current_index = i;
                    return Some(sub_command.to_string());
                }
                None
            })
    }
}
