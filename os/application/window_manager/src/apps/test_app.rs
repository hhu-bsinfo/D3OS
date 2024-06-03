use core::cell::Cell;

use alloc::{boxed::Box, rc::Rc, string::String};
use drawer::drawer::{RectData, Vertex};

use crate::{api::Command, WindowManager};

use super::runnable::Runnable;

pub struct TestApp;

impl Runnable for TestApp {
    fn run() {
        let handle = concurrent::thread::current().id();
        let api = WindowManager::get_api();
        let qwe = Rc::new(Cell::new('0'));
        let qwe2 = Rc::clone(&qwe);
        api.execute(
            handle,
            Command::CreateButton {
                pos: RectData {
                    top_left: Vertex::new(400, 400),
                    width: 200,
                    height: 100,
                },
                label: Some(String::from("Hello")),
                on_click: Box::new(move || {
                    let old = qwe2.get().to_digit(10).unwrap();
                    qwe2.set(char::from_digit(old + 1, 10).unwrap());
                }),
            },
        );
    }
}
