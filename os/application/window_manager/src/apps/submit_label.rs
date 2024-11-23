use alloc::{boxed::Box, rc::Rc, string::String, vec};
use drawer::{rect_data::RectData, vertex::Vertex};
use spin::rwlock::RwLock;
use spin::Mutex;

use crate::{api::Command, WindowManager};

use super::runnable::Runnable;

pub struct SubmitLabel;

impl Runnable for SubmitLabel {
    fn run() {
        let handle = concurrent::thread::current().id();
        let api = WindowManager::get_api();

        let input_field_rc1 = Rc::new(RwLock::new(String::from("")));
        let input_field_rc2 = Rc::clone(&input_field_rc1);

        let label_text_rc1 = Rc::new(RwLock::new(String::from("")));
        let label_text_rc2 = Rc::clone(&label_text_rc1);

        let input_field = api.execute(
            handle,
            Command::CreateInputField {
                width_in_chars: 12,
                font_size: Some(2),
                log_pos: Vertex::new(100, 200),
                text: input_field_rc1,
                state_dependencies: vec![],
            },
        );

        let _ = api.execute(
            handle,
            Command::CreateLabel {
                log_pos: Vertex::new(400, 200),
                text: Rc::new(RwLock::new(String::from("Submitted Text: "))),
                on_loop_iter: None,
                font_size: Some(2),
                state_dependencies: vec![],
            },
        );

        let submitted_text = api.execute(
            handle,
            Command::CreateLabel {
                log_pos: Vertex::new(660, 200),
                text: label_text_rc1,
                on_loop_iter: None,
                font_size: Some(2),
                state_dependencies: vec![],
            },
        );

        let label_rc = Rc::new(Mutex::new(String::from("Submit")));
        let button_font = 3;
        let _ = api.execute(
            handle,
            Command::CreateButton {
                log_rect_data: RectData {
                    top_left: Vertex::new(100, 300),
                    width: 160,
                    height: 50,
                },
                label: Some((label_rc, button_font)),
                on_click: Box::new(move || {
                    let mut input_field = input_field_rc2.write();
                    let mut label_text = label_text_rc2.write();
                    *label_text = input_field.drain(..).collect();
                }),
                state_dependencies: vec![input_field.unwrap(), submitted_text.unwrap()],
            },
        );
    }
}
