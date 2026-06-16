use alloc::string::ToString;
use log::info;

use crate::{
    context::{
        context::ContextProvider,
        executable_context::{ExecutableBuilder, ExecutableContext, IoTarget, JobResult},
        tokens_context::TokensContext,
        working_directory_context::WorkingDirectoryContext,
    },
    event::{
        event_bus::EventBus,
        event_handler::{Error, EventHandler, Response},
    },
    token::token::{TokenKind, TokenStatus},
};

#[derive(Debug)]
enum IoType {
    InFile,
    OutAppend,
    OutTruncate,
}

pub struct ParserService {
    tokens_provider: ContextProvider<TokensContext>,
    executable_provider: ContextProvider<ExecutableContext>,
    wd_provider: ContextProvider<WorkingDirectoryContext>,

    current_io_type: Option<IoType>,
}

impl EventHandler for ParserService {
    fn on_prepare_next_line(&mut self, _event_bus: &mut EventBus) -> Result<Response, Error> {
        self.executable_provider.borrow_mut().reset();
        Ok(Response::Ok)
    }

    fn on_submit(&mut self, _event_bus: &mut EventBus) -> Result<Response, Error> {
        self.parse()
    }
}

impl ParserService {
    pub const fn new(
        tokens_provider: ContextProvider<TokensContext>,
        executable_provider: ContextProvider<ExecutableContext>,
        wd_provider: ContextProvider<WorkingDirectoryContext>,
    ) -> Self {
        Self {
            tokens_provider,
            executable_provider,
            wd_provider,
            current_io_type: None,
        }
    }

    fn parse(&mut self) -> Result<Response, Error> {
        let mut executable_clx = self.executable_provider.borrow_mut();
        let tokens_clx = self.tokens_provider.borrow();
        let tokens = tokens_clx.get();

        // warn!("{:#?}", tokens);

        let mut executable_builder = ExecutableBuilder::new();
        executable_builder.id(executable_clx.len());
        self.current_io_type = None;

        if let Some(last) = tokens.last() {
            match last.status() {
                TokenStatus::Error(error) => return Err((*error).clone()),
                TokenStatus::Incomplete(error) => return Err((*error).clone()),
                _ => (),
            }
        }

        for token in tokens {
            if !token.clx().cmd_pos_in_segment.is_some() {
                let Ok(executable) = executable_builder.build() else {
                    continue;
                };
                executable_clx.add_executable(executable);
                executable_builder = ExecutableBuilder::new();
                executable_builder.id(executable_clx.len());
            }

            match token.kind() {
                TokenKind::Command => {
                    executable_builder.command(token.to_string());
                }

                TokenKind::Argument => {
                    executable_builder.add_argument(token.to_string());
                }

                TokenKind::Background => {
                    for executable in &mut executable_clx.executables {
                        executable.background_execution = true
                    }
                    executable_builder.run_in_background(true);
                }

                TokenKind::And => {
                    let Some(last_executable) = executable_clx.last_executable() else {
                        return Err(Error::new(
                            "And condition requires a preceding executable".to_string(),
                            None,
                        ));
                    };
                    executable_builder.requires_executable(last_executable.id, JobResult::Success);
                }

                TokenKind::Or => {
                    let Some(last_executable) = executable_clx.last_executable() else {
                        return Err(Error::new(
                            "Or condition requires a preceding executable".to_string(),
                            None,
                        ));
                    };
                    executable_builder.requires_executable(last_executable.id, JobResult::Error);
                }

                TokenKind::Pipe => {
                    let Some(last_executable) = executable_clx.last_mut_executable() else {
                        return Err(Error::new("Pipe requires a preceding executable".to_string(), None));
                    };

                    last_executable.output = IoTarget::Job(
                        executable_builder
                            .peek_id()
                            .expect("Next executable id should be set by now"),
                    );
                    executable_builder.use_input(IoTarget::Job(last_executable.id));
                }

                TokenKind::File => {
                    let Some(ref current_io_type) = self.current_io_type else {
                        return Err(Error::new(
                            "Received file without redirection instruction".to_string(),
                            None,
                        ));
                    };

                    let wd_clx = self.wd_provider.borrow();
                    let abs_path = wd_clx.resolve(&token.to_string());
                    match current_io_type {
                        IoType::InFile => executable_builder.use_input(IoTarget::FileTruncate(abs_path)),
                        IoType::OutAppend => executable_builder.use_output(IoTarget::FileAppend(abs_path)),
                        IoType::OutTruncate => executable_builder.use_output(IoTarget::FileTruncate(abs_path)),
                    };
                    self.current_io_type = None;
                }

                TokenKind::RedirectInFile => self.current_io_type = Some(IoType::InFile),
                TokenKind::RedirectOutAppend => self.current_io_type = Some(IoType::OutAppend),
                TokenKind::RedirectOutTruncate => self.current_io_type = Some(IoType::OutTruncate),

                TokenKind::QuoteStart | TokenKind::QuoteEnd | TokenKind::Blank | TokenKind::Separator => (),
            }
        }

        match executable_builder.build() {
            Ok(executable) => executable_clx.add_executable(executable),
            Err(_) => (),
        };

        info!("{:#?}", executable_clx);
        Ok(Response::Ok)
    }
}
