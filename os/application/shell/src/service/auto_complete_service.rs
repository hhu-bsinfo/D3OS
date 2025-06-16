use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use globals::application::{APPLICATION_REGISTRY, Application};

use crate::{
    context::Context,
    service::{
        lexer_service::{AmbiguousState, ArgumentType, FindLastCommand, Token},
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
        if !context.is_cursor_at_end() {
            return Ok(Response::Skip);
        }

        self.revalidate_application(context);

        if !context.auto_completion.has_focus() {
            return self.focus_suggestion(context);
        }

        self.cycle_suggestion(context)
    }

    fn simple_key(&mut self, context: &mut Context, key: char) -> Result<Response, Error> {
        if !context.is_cursor_at_end() {
            return Ok(Response::Skip);
        }

        self.revalidate_application(context);

        match key {
            ' ' => self.adopt(context),
            '\x08' | '\x7F' => self.clear_suggestion(context),
            _ => {
                self.clear_suggestion(context);
                self.cycle_suggestion(context)
            }
        }
    }

    fn prepare(&mut self, context: &mut Context) -> Result<Response, Error> {
        self.reset(context)
    }

    fn submit(&mut self, context: &mut Context) -> Result<Response, Error> {
        self.clear_suggestion(context)
    }

    fn cursor_left(&mut self, context: &mut Context) -> Result<Response, Error> {
        self.clear_suggestion(context)
    }

    fn cursor_right(&mut self, context: &mut Context) -> Result<Response, Error> {
        self.clear_suggestion(context)
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
        let intercept_char = context
            .line
            .pop()
            .expect("Expected at least one char in line");
        context.line.push_str(&context.auto_completion.get());
        context.line.push(intercept_char);
        context.cursor_position += context.auto_completion.len();

        self.clear_suggestion(context);
        Ok(Response::Ok)
    }

    fn clear_suggestion(&mut self, context: &mut Context) -> Result<Response, Error> {
        self.current_index = 0;
        self.current_suggestion = None;
        context.auto_completion.reset();
        Ok(Response::Ok)
    }

    fn reset(&mut self, context: &mut Context) -> Result<Response, Error> {
        self.current_app = None;
        self.clear_suggestion(context)
    }

    fn focus_suggestion(&mut self, context: &mut Context) -> Result<Response, Error> {
        if self.current_suggestion.is_none() {
            self.cycle_suggestion(context);
            if self.current_suggestion.is_none() {
                return Ok(Response::Skip);
            }
        }

        context.auto_completion.focus();
        Ok(Response::Ok)
    }

    fn revalidate_application(&mut self, context: &mut Context) {
        let Some(last_command) = context.tokens.find_last_command() else {
            self.current_suggestion = None;
            return;
        };
        let last_command = last_command.to_string();
        if self
            .current_app
            .as_ref()
            .is_some_and(|app| app.command == last_command)
        {
            return;
        }

        self.current_app = self.find_application(&last_command).cloned();
    }

    fn find_application(&mut self, command: &str) -> Option<&Application> {
        self.applications.iter().find(|app| app.command == command)
    }

    fn cycle_suggestion(&mut self, context: &mut Context) -> Result<Response, Error> {
        let token = context.tokens.last().cloned();
        let suggestion = match &token {
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

        let start_at = if token.as_ref().is_some_and(|token| token.is_ambiguous()) {
            token.unwrap().len()
        } else {
            0
        };

        context
            .auto_completion
            .set(&suggestion.unwrap()[start_at..]);
        Ok(Response::Ok)
    }

    fn cycle_command(&mut self, cmd: &String) -> Option<String> {
        self.applications
            .iter()
            .enumerate()
            .cycle()
            .skip(self.current_index + 1)
            .take(self.applications.len())
            .find_map(|(i, app)| {
                if !app.command.starts_with(cmd) {
                    return None;
                }

                self.current_index = i;
                Some(app.command.to_string())
            })
    }

    fn cycle_argument(&mut self, arg_type: Option<&ArgumentType>, arg: &String) -> Option<String> {
        if arg_type.is_none() {
            return self.cycle_all_arguments(arg);
        }
        match arg_type.unwrap() {
            ArgumentType::Generic => self.cycle_generic_argument(arg),
            ArgumentType::ShortFlag => panic!("short flag auto complete not implemented"),
            ArgumentType::LongFlag => panic!("long flag auto complete not implemented"),
            ArgumentType::LongFlagValue => panic!("long flag value auto complete not implemented"),
        }
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
                if !sub_command.starts_with(arg) {
                    return None;
                }

                self.current_index = i;
                return Some(sub_command.to_string());
            })
    }
}
