use alloc::{
    boxed::Box,
    rc::Rc,
    string::{String, ToString},
};
use drawer::drawer::{RectData, Vertex};
use spin::Mutex;

use crate::{api::Command, WindowManager};

use super::runnable::Runnable;

pub struct TestApp;

impl Runnable for TestApp {
    fn run() {
        let handle = concurrent::thread::current().id();
        let api = WindowManager::get_api();
        let qwe = Rc::new(Mutex::new(String::from("0")));
        let qwe2 = Rc::clone(&qwe);
        api.execute(
            handle,
            Command::CreateButton {
                pos: RectData {
                    top_left: Vertex::new(400, 400),
                    width: 200,
                    height: 100,
                },
                label: Some(qwe),
                on_click: Box::new(move || {
                    let mut value = qwe2.lock();
                    let old = (*value).parse::<usize>().unwrap();
                    *value = (old + 1).to_string();
                }),
            },
        );

        api.execute(
            handle,
            Command::CreateButton {
                pos: RectData {
                    top_left: Vertex::new(200, 200),
                    width: 150,
                    height: 130,
                },
                label: Some(Rc::new(Mutex::new(String::from("ABC")))),
                on_click: Box::new(move || {}),
            },
        );
    }
}
