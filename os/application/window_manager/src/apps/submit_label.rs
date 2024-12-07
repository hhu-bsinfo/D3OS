use alloc::{boxed::Box, rc::Rc, string::String};
use drawer::{rect_data::RectData, vertex::Vertex};

use crate::{api::Command, signal::Signal, WindowManager};

use super::runnable::Runnable;

pub struct SubmitLabel;

impl Runnable for SubmitLabel {
    fn run() {
        let handle = concurrent::thread::current().id();
        let api = WindowManager::get_api();

        let starting_text = String::from("");
        let input = Signal::new(starting_text.clone());
        let input_text = Rc::clone(&input);
        let submit_input = Rc::clone(&input);
        let submitted_text = Signal::new(starting_text.clone());

        let input_field = api.execute(
            handle,
            Command::CreateInputField {
                log_rect_data: RectData {
                    top_left: Vertex::new(50, 100),
                    width: 200,
                    height: 75,
                },
                width_in_chars: 12,
                font_size: Some(2),
                starting_text: starting_text.clone(),
                on_change: Some(Box::new(move |new_text| {
                    input.set(new_text);
                })),
                styling: None,
            },
        ).unwrap();

        let _submitted_text_label= api.execute(
            handle,
            Command::CreateLabel {
                log_pos: Vertex::new(50, 200),
                text: Signal::new(String::from("Submitted Text: ")),
                on_loop_iter: None,
                font_size: Some(2),
                styling: None,
            },
        );

        let _submitted_text = api.execute(
            handle,
            Command::CreateLabel {
                log_pos: Vertex::new(500, 200),
                text: Rc::clone(&submitted_text),
                on_loop_iter: None,
                font_size: Some(2),
                styling: None,
            },
        );

        let _on_change_text = api.execute(
            handle,
            Command::CreateLabel {
                log_pos: Vertex::new(500, 300),
                text: Rc::clone(&input_text),
                on_loop_iter: None,
                font_size: Some(2),
                styling: None,
            },
        );

        let _submit = api.execute(
            handle,
            Command::CreateButton {
                log_rect_data: RectData {
                    top_left: Vertex::new(50, 300),
                    width: 160,
                    height: 50,
                },
                label: Some((Signal::new(String::from("Submit")), 3)),
                on_click: Some(Box::new(move || {
                    let value = submit_input.get();
                    submitted_text.set(value);

                    if let Some(clearable) = input_field.write().as_clearable_mut() {
                        clearable.clear();
                    }
                })),
                styling: None,
            },
        );
    }
}
