use crate::alloc::string::ToString;
use alloc::{boxed::Box, rc::Rc, string::String, vec};
use drawer::{rect_data::RectData, vertex::Vertex};
use spin::mutex::Mutex;

use crate::{api::Command, WindowManager};

use super::runnable::Runnable;

pub struct Counter;

impl Runnable for Counter {
    fn run() {
        let handle = concurrent::thread::current().id();
        let api = WindowManager::get_api();

        let label_text_rc1 = Rc::new(Mutex::new(String::from("0")));
        let label_text_rc2 = Rc::clone(&label_text_rc1);
        let label_text_rc3 = Rc::clone(&label_text_rc1);
        
        let counter_button = api.execute(
            handle,
            Command::CreateButton {
                log_rect_data: RectData {
                    top_left: Vertex::new(200, 200),
                    width: 200,
                    height: 50,
                },
                label: Some((label_text_rc1, 1)),
                on_click: Box::new(move || {
                    let mut value = label_text_rc2.lock();
                    let old = (*value).parse::<usize>().unwrap();
                    *value = (old + 1).to_string();
                }),
                state_dependencies: vec![],
            },
        ).unwrap();

        let _ = api.execute(
            handle,
            Command::CreateButton {
                log_rect_data: RectData {
                    top_left: Vertex::new(200, 350),
                    width: 200,
                    height: 50,
                },
                label: Some((Rc::new(Mutex::new(String::from("Reset"))), 1)),
                on_click: Box::new(move || {
                    let mut value = label_text_rc3.lock();
                    *value = String::from("0");
                }),
                state_dependencies: vec![Rc::clone(&counter_button)],
            },
        );
    }
}
