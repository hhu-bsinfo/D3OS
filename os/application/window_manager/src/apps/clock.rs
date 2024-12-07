use alloc::{boxed::Box, rc::Rc, string::ToString};
use drawer::vertex::Vertex;
use time::date;

use crate::{api::Command, signal::Signal, WindowManager};

use super::runnable::Runnable;

pub struct Clock;

impl Runnable for Clock {
    fn run() {
        let handle = concurrent::thread::current().id();
        let api = WindowManager::get_api();
        let date_val = date().format("%Y-%m-%d %H:%M:%S").to_string();
        
        let clock = Signal::new(date_val);
        let second_clock = Rc::clone(&clock);
        
        let _clock = api.execute(
            handle,
            Command::CreateLabel {
                log_pos: Vertex::new(200, 200),
                text: Rc::clone(&clock),
                on_loop_iter: Some(Box::new(move || {
                    let old_date = clock.get();
                    let new_date = date().format("%Y-%m-%d %H:%M:%S").to_string();

                    if old_date != new_date {
                        clock.set(new_date);
                        return true;
                    }

                    return false;
                })),
                font_size: Some(4),
                styling: None,
            },
        );

        let _second_clock = api.execute(
            handle,
            Command::CreateLabel {
                log_pos: Vertex::new(200, 300),
                text: Rc::clone(&second_clock),
                on_loop_iter: None,
                font_size: Some(4),
                styling: None,
            },
        );
    }
}
