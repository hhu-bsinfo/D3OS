use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use globals::application::{APPLICATION_REGISTRY, Application};

use crate::{
    context::Context,
    service::{
        lexer_service::{AmbiguousState, ArgumentType, FindLastCommand, Token, TokenContext},
        service::{Error, Response, Service},
    },
};

#[derive(Debug)]
pub struct AutoCompleteService {
    applications: Vec<Application>,
    current_index: usize,
    current_app: Option<Application>,
    current_short_flag: Option<usize>,
    current_suggestion: Option<String>,
}

impl Service for AutoCompleteService {
    fn auto_complete(&mut self, context: &mut Context) -> Result<Response, Error> {
        if !context.is_cursor_at_end() {
            return Ok(Response::Skip);
        }

        self.revalidate(context);

        if !context.auto_completion.has_focus() {
            return self.focus_suggestion(context);
        }

        self.cycle_suggestion(context)
    }

    fn simple_key(&mut self, context: &mut Context, key: char) -> Result<Response, Error> {
        if !context.is_cursor_at_end() {
            return Ok(Response::Skip);
        }

        self.revalidate(context);

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
            current_short_flag: None,
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
        self.current_short_flag = None;
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

    fn revalidate(&mut self, context: &mut Context) {
        self.revalidate_application(context);
        self.revalidate_short_flag(context);
    }

    fn revalidate_short_flag(&mut self, context: &mut Context) {
        let Some(current_app) = &self.current_app else {
            self.current_short_flag = None;
            return;
        };
        let Some(last_short_flag) = context.tokens.find_last_short_flag() else {
            self.current_short_flag = None;
            return;
        };

        let target = last_short_flag.to_string();
        if self
            .current_short_flag
            .as_ref()
            .is_some_and(|&index| current_app.short_flags[index].0 == target)
        {
            return;
        }

        self.current_short_flag = self
            .current_app
            .as_ref()
            .unwrap()
            .short_flags
            .iter()
            .position(|&(key, _)| key == target);
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

        self.current_app = self
            .applications
            .iter()
            .find(|&app| app.command == last_command)
            .cloned();
    }

    fn cycle_suggestion(&mut self, context: &mut Context) -> Result<Response, Error> {
        let token = context.tokens.last().cloned();
        let suggestion = match &token {
            None => self.cycle_command(&String::new()),

            Some(Token::Command(_, cmd)) => self.cycle_command(&cmd),

            Some(Token::Argument(clx, arg)) => self.cycle_argument(clx, &arg),

            Some(Token::Whitespace(clx)) => match clx.ambiguous {
                AmbiguousState::Pending => self.cycle_command(&String::new()),
                AmbiguousState::Command => self.cycle_argument(clx, &String::new()),
                AmbiguousState::Argument => self.cycle_argument(clx, &String::new()),
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
        let commands: Vec<&'static str> = self.applications.iter().map(|app| app.command).collect();
        self.cycle(cmd, &commands)
    }

    fn cycle_argument(&mut self, clx: &TokenContext, arg: &String) -> Option<String> {
        match clx.argument_type {
            None | Some(ArgumentType::Unknown) => self.cycle_all_arguments(arg),
            Some(ArgumentType::Generic) => self.cycle_generic_argument(arg),
            Some(ArgumentType::ShortFlag) => self.cycle_short_flag(arg),
            Some(ArgumentType::ShortFlagValue) => self.cycle_short_flag_value(arg),
            Some(ArgumentType::LongFlag) => self.cycle_long_flag(arg),
        }
    }

    fn cycle_all_arguments(&mut self, arg: &String) -> Option<String> {
        let app = self.current_app.as_mut().unwrap();
        let mut args = Vec::new();
        args.extend(app.sub_commands.iter());
        args.extend(app.short_flags.into_iter().map(|&(key, _)| key));
        args.extend(app.long_flags.iter());

        self.cycle(arg, &args)
    }

    fn cycle_generic_argument(&mut self, arg: &String) -> Option<String> {
        let sub_commands = self.current_app.as_mut().unwrap().sub_commands;
        self.cycle(arg, &sub_commands)
    }

    fn cycle_short_flag(&mut self, arg: &String) -> Option<String> {
        let short_flags: Vec<&'static str> = self
            .current_app
            .as_ref()
            .unwrap()
            .short_flags
            .iter()
            .map(|&(key, _)| key)
            .collect();

        self.cycle(arg, &short_flags)
    }

    fn cycle_short_flag_value(&mut self, arg: &String) -> Option<String> {
        if self.current_app.is_none() || self.current_short_flag.is_none() {
            return None;
        }
        let (_key, values) =
            self.current_app.as_mut().unwrap().short_flags[self.current_short_flag.unwrap()];

        self.cycle(arg, &values)
    }

    fn cycle_long_flag(&mut self, arg: &String) -> Option<String> {
        if self.current_app.is_none() {
            return None;
        }
        let long_flags = self.current_app.as_mut().unwrap().long_flags;
        self.cycle(arg, &long_flags)
    }

    fn cycle(&mut self, target: &String, list: &[&'static str]) -> Option<String> {
        list.iter()
            .enumerate()
            .cycle()
            .skip(self.current_index)
            .take(list.len())
            .find_map(|(i, &found)| {
                if !found.starts_with(target) {
                    return None;
                }

                self.current_index = i + 1;
                return Some(found.to_string());
            })
    }
}
