use alloc::{boxed::Box, rc::Rc, string::ToString};
use concurrent::thread::sleep;
use drawer::drawer::{RectData, Vertex};
use spin::rwlock::RwLock;
use time::date;

use crate::{api::Command, WindowManager};

use super::runnable::Runnable;

pub struct Clock;

impl Runnable for Clock {
    fn run() {
        let handle = concurrent::thread::current().id();
        let api = WindowManager::get_api();
        let date_val = date().format("%Y-%m-%d %H:%M:%S").to_string();
        let text_rc = Rc::new(RwLock::new(date_val));
        let on_create_rc = Rc::clone(&text_rc);
        api.execute(
            handle,
            Command::CreateDynamicLabel {
                pos: RectData {
                    top_left: Vertex::new(400, 400),
                    width: 200,
                    height: 100,
                },
                text: text_rc,
                // This does not work, until we find a way to capture the environment when creating
                // a new thread
                on_create: Some(Box::new(move || loop {
                    sleep(1000);
                    let mut date_val = on_create_rc.write();
                    *date_val = date().format("%Y-%m-%d %H:%M:%S").to_string();
                })),
            },
        );
    }
}
