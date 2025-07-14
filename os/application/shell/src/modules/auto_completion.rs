use core::cell::RefCell;

use alloc::{
    rc::Rc,
    string::{String, ToString},
    vec::Vec,
};
use globals::application::{APPLICATION_REGISTRY, Application};
use terminal::DecodedKey;

use crate::{
    context::{line_context::LineContext, suggestion_context::SuggestionContext, tokens_context::TokensContext},
    event::{
        event::Event,
        event_bus::EventBus,
        event_handler::{Error, EventHandler, Response},
    },
    token::token::{Token, TokenKind},
};

#[derive(Debug)]
pub struct AutoCompletion {
    line_provider: Rc<RefCell<LineContext>>,
    tokens_provider: Rc<RefCell<TokensContext>>,
    suggestion_provider: Rc<RefCell<SuggestionContext>>,

    applications: &'static [Application],
    current_index: usize,
    current_app: Option<Application>,
    current_key_value_idx: Option<usize>,
    current_suggestion: Option<String>,
}

impl EventHandler for AutoCompletion {
    fn on_key_pressed(&mut self, event_bus: &mut EventBus, key: DecodedKey) -> Result<Response, Error> {
        if !self.line_provider.borrow().is_cursor_at_end() {
            return Ok(Response::Skip);
        }

        match key {
            DecodedKey::Unicode('\t') => {
                self.revalidate();
                if !self.suggestion_provider.borrow().has_focus() {
                    return self.focus_suggestion();
                }
                self.cycle_suggestion()
            }
            DecodedKey::Unicode(' ') => {
                self.revalidate();
                self.adopt(event_bus)
            }
            DecodedKey::Unicode('\x1B') | DecodedKey::Unicode('\x08') | DecodedKey::Unicode('\x7F') => {
                self.unfocus_suggestion()
            }
            _ => Ok(Response::Skip),
        }
    }

    fn on_tokens_written(&mut self, _event_bus: &mut EventBus) -> Result<Response, Error> {
        if !self.line_provider.borrow().is_cursor_at_end() {
            return Ok(Response::Skip);
        }

        self.revalidate();
        self.clear_suggestion();
        self.cycle_suggestion()
    }

    fn on_submit(&mut self, _event_bus: &mut EventBus) -> Result<Response, Error> {
        self.clear_suggestion()
    }

    fn on_cursor_moved(&mut self, _event_bus: &mut EventBus, _step: isize) -> Result<Response, Error> {
        self.clear_suggestion()
    }

    fn on_prepare_next_line(&mut self, _event_bus: &mut EventBus) -> Result<Response, Error> {
        self.reset()
    }

    fn on_history_restored(&mut self, _event_bus: &mut EventBus) -> Result<Response, Error> {
        self.reset()
    }
}

impl AutoCompletion {
    pub fn new(
        line_provider: Rc<RefCell<LineContext>>,
        tokens_provider: Rc<RefCell<TokensContext>>,
        suggestion_provider: Rc<RefCell<SuggestionContext>>,
    ) -> Self {
        Self {
            line_provider,
            tokens_provider,
            suggestion_provider,
            applications: APPLICATION_REGISTRY,
            current_index: 0,
            current_app: None,
            current_key_value_idx: None,
            current_suggestion: None,
        }
    }

    fn adopt(&mut self, event_bus: &mut EventBus) -> Result<Response, Error> {
        {
            let mut line_clx = self.line_provider.borrow_mut();
            let suggestion_clx = self.suggestion_provider.borrow();
            if suggestion_clx.is_empty() || !suggestion_clx.has_focus() {
                return Ok(Response::Skip);
            }

            let intercept_char = line_clx.pop().expect("Expected at least one char in line");
            line_clx.push_str(&suggestion_clx.get());
            line_clx.push(intercept_char);
            line_clx.move_cursor_right(suggestion_clx.len());
        }

        event_bus.trigger(Event::LineWritten);
        self.clear_suggestion();
        Ok(Response::Ok)
    }

    fn clear_suggestion(&mut self) -> Result<Response, Error> {
        self.current_index = 0;
        self.current_suggestion = None;
        self.suggestion_provider.borrow_mut().reset();
        Ok(Response::Ok)
    }

    fn reset(&mut self) -> Result<Response, Error> {
        self.current_app = None;
        self.current_key_value_idx = None;
        self.clear_suggestion()
    }

    fn focus_suggestion(&mut self) -> Result<Response, Error> {
        if self.current_suggestion.is_none() {
            self.cycle_suggestion();
            if self.current_suggestion.is_none() {
                return Ok(Response::Skip);
            }
        }

        self.suggestion_provider.borrow_mut().focus();
        Ok(Response::Ok)
    }

    fn unfocus_suggestion(&mut self) -> Result<Response, Error> {
        self.suggestion_provider.borrow_mut().unfocus();
        Ok(Response::Ok)
    }

    fn revalidate(&mut self) {
        self.revalidate_application();
        self.revalidate_key_value_idx();
    }

    fn revalidate_key_value_idx(&mut self) {
        let Some(current_app) = &self.current_app else {
            self.current_key_value_idx = None;
            return;
        };

        let tokens_clx = self.tokens_provider.borrow_mut();
        let Some(last_arg_token) = tokens_clx.find_last_argument_in_segment() else {
            self.current_key_value_idx = None;
            return;
        };

        if let Some(idx) = self.current_key_value_idx {
            let (current_key, _values) = current_app.key_value_pair[idx];
            if current_key == last_arg_token.as_str() {
                return;
            }
        }

        let Some(found_idx) = current_app
            .key_value_pair
            .iter()
            .position(|(key, _value)| *key == last_arg_token.as_str())
        else {
            self.current_key_value_idx = None;
            return;
        };

        self.current_key_value_idx = Some(found_idx);
    }

    fn revalidate_application(&mut self) {
        let tokens_clx = self.tokens_provider.borrow();
        let Some(last_command) = tokens_clx.find_last_command() else {
            self.current_suggestion = None;
            return;
        };
        let last_command = last_command.as_str();
        if self
            .current_app
            .as_ref()
            .is_some_and(|app| app.namespace == last_command)
        {
            return;
        }

        self.current_app = self
            .applications
            .iter()
            .find(|&app| app.namespace == last_command)
            .cloned();
    }

    fn cycle_suggestion(&mut self) -> Result<Response, Error> {
        let (token, start_at) = {
            let tokens_clx = self.tokens_provider.borrow();
            let token = tokens_clx.last().cloned();
            let start_at = token
                .as_ref()
                .filter(|t| t.is_ambiguous())
                .map(|t| t.len())
                .unwrap_or(0);
            (token, start_at)
        };

        let Some(suggestion) = self.cycle_token(token.as_ref()) else {
            self.current_suggestion = None;
            return Ok(Response::Skip);
        };
        self.current_suggestion = Some(suggestion.clone());
        let mut suggestion_clx = self.suggestion_provider.borrow_mut();
        suggestion_clx.set(&suggestion[start_at..]);
        Ok(Response::Ok)
    }

    fn cycle_token(&mut self, token: Option<&Token>) -> Option<String> {
        let Some(token) = token else {
            return self.cycle_command(&String::new());
        };

        match token.kind() {
            TokenKind::Command => self.cycle_command(token.as_str()),

            TokenKind::Argument => self.cycle_argument(token.as_str()),

            TokenKind::Blank => match token.clx().cmd_pos_in_segment {
                Some(_) => self.cycle_argument(&String::new()),
                None => self.cycle_command(&String::new()),
            },

            _ => None,
        }
    }

    fn cycle_command(&mut self, cmd: &str) -> Option<String> {
        let commands: Vec<&'static str> = self.applications.iter().map(|app| app.namespace).collect();
        self.cycle(cmd, &commands)
    }

    fn cycle_argument(&mut self, arg: &str) -> Option<String> {
        let Some(ref current_app) = self.current_app else {
            return None;
        };
        match self.current_key_value_idx {
            Some(idx) => self.cycle(arg, current_app.key_value_pair[idx].1),
            None => self.cycle_all_arguments(arg),
        }
    }

    fn cycle_all_arguments(&mut self, arg: &str) -> Option<String> {
        let app = self.current_app.as_mut().unwrap();
        let mut args = Vec::new();
        args.extend(app.single_value.iter());
        args.extend(app.key_value_pair.into_iter().map(|&(key, _)| key));

        self.cycle(arg, &args)
    }

    fn cycle(&mut self, target: &str, list: &[&'static str]) -> Option<String> {
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
