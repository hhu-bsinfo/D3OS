use alloc::{boxed::Box, rc::Rc, string::{String, ToString}};
use drawer::{rect_data::RectData, vertex::Vertex};
use crate::{api::Command, signal::Signal, WindowManager};

use super::runnable::Runnable;

pub struct SliderApp;

impl Runnable for SliderApp {
    fn run() {
        let handle = concurrent::thread::current().id();
        let api = WindowManager::get_api();

        let initial_value = 10;

        let label = Signal::new(initial_value.to_string());
        let label_slider = Rc::clone(&label);

        let _label_value = api.execute(
            handle,
            Command::CreateLabel {
                log_pos: Vertex::new(50, 100),
                text: label,
                on_loop_iter: None,
                font_size: Some(2),
                styling: None,
            },
        ).unwrap();

        let _slider = api.execute(
            handle,
            Command::CreateSlider {
                log_rect_data: RectData {
                    top_left: Vertex::new(50, 150),
                    width: 200,
                    height: 50,
                },
                on_change: Some(Box::new(move |value| {
                    label_slider.set(value.to_string());
                })),
                value: initial_value,
                min: 10,
                max: 100,
                steps: 1,
                styling: None,
            }
        );
    }
}
