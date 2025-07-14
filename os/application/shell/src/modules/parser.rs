use core::cell::RefCell;

use alloc::{rc::Rc, string::ToString};
use logger::info;

use crate::{
    context::{
        executable_context::{ExecutableContext, Io, JobBuilder, JobResult},
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
    InAppend,
    InTruncate,
    OutAppend,
    OutTruncate,
}

pub struct Parser {
    current_io_type: Option<IoType>,
    tokens_provider: Rc<RefCell<TokensContext>>,
    executable_provider: Rc<RefCell<ExecutableContext>>,
    wd_provider: Rc<RefCell<WorkingDirectoryContext>>,
}

impl EventHandler for Parser {
    fn on_prepare_next_line(&mut self, _event_bus: &mut EventBus) -> Result<Response, Error> {
        self.executable_provider.borrow_mut().reset();
        Ok(Response::Ok)
    }

    fn on_submit(&mut self, _event_bus: &mut EventBus) -> Result<Response, Error> {
        self.parse()
    }
}

impl Parser {
    pub const fn new(
        tokens_provider: Rc<RefCell<TokensContext>>,
        executable_provider: Rc<RefCell<ExecutableContext>>,
        wd_provider: Rc<RefCell<WorkingDirectoryContext>>,
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

        let mut job_builder = JobBuilder::new();
        job_builder.id(executable_clx.len());
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
                let Ok(job) = job_builder.build() else {
                    continue;
                };
                executable_clx.add_job(job);
                job_builder = JobBuilder::new();
                job_builder.id(executable_clx.len());
            }

            match token.kind() {
                TokenKind::Command => {
                    job_builder.command(token.to_string());
                }

                TokenKind::Argument => {
                    job_builder.add_argument(token.to_string());
                }

                TokenKind::Background => {
                    for job in &mut executable_clx.jobs {
                        job.background_execution = true
                    }
                    job_builder.run_in_background(true);
                }

                TokenKind::And => {
                    let Some(last_job) = executable_clx.last_job() else {
                        return Err(Error::new("And condition requires a preceding job".to_string(), None));
                    };
                    job_builder.requires_job(last_job.id, JobResult::Success);
                }

                TokenKind::Or => {
                    let Some(last_job) = executable_clx.last_job() else {
                        return Err(Error::new("Or condition requires a preceding job".to_string(), None));
                    };
                    job_builder.requires_job(last_job.id, JobResult::Error);
                }

                TokenKind::Pipe => {
                    let Some(last_job) = executable_clx.last_job_mut() else {
                        return Err(Error::new("Pipe requires a preceding job".to_string(), None));
                    };

                    last_job.output = Io::Job(job_builder.peek_id().expect("Next job id should be set by now"));
                    job_builder.use_input(Io::Job(last_job.id));
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
                        IoType::InAppend => job_builder.use_input(Io::FileAppend(abs_path)),
                        IoType::InTruncate => job_builder.use_input(Io::FileTruncate(abs_path)),
                        IoType::OutAppend => job_builder.use_output(Io::FileAppend(abs_path)),
                        IoType::OutTruncate => job_builder.use_output(Io::FileTruncate(abs_path)),
                    };
                    self.current_io_type = None;
                }

                TokenKind::RedirectInAppend => self.current_io_type = Some(IoType::InAppend),
                TokenKind::RedirectInTruncate => self.current_io_type = Some(IoType::InTruncate),
                TokenKind::RedirectOutAppend => self.current_io_type = Some(IoType::OutAppend),
                TokenKind::RedirectOutTruncate => self.current_io_type = Some(IoType::OutTruncate),

                TokenKind::QuoteStart | TokenKind::QuoteEnd | TokenKind::Blank | TokenKind::Separator => (),
            }
        }

        match job_builder.build() {
            Ok(job) => executable_clx.add_job(job),
            Err(_) => (),
        };

        info!("{:#?}", executable_clx);
        Ok(Response::Ok)
    }
}
