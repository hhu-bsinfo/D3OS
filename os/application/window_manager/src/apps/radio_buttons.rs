use crate::{alloc::string::ToString, components::component::ComponentStylingBuilder, signal::Signal};
use alloc::{boxed::Box, rc::Rc, string::String};
use drawer::vertex::Vertex;


use crate::{api::Command, WindowManager};

use super::runnable::Runnable;

pub struct RadioButtonApp;

impl Runnable for RadioButtonApp {
    fn run() {
        let handle = concurrent::thread::current().id();
        let api = WindowManager::get_api();
        let option = Signal::new(String::from("1"));

        let option_radio_buttons = Rc::clone(&option);
        api.execute(
            handle,
            Command::CreateRadioButtonGroup {
                center: Vertex::new(100, 50),
                radius: 20,
                spacing: 20,
                num_buttons: 3,
                selected_option: 1,
                on_change: Some(Box::new(move |selected_option: usize| {
                    option_radio_buttons.set(selected_option.to_string());
                })),
                styling: Some(ComponentStylingBuilder::new().maintain_aspect_ratio(true).build()),
            },
        );

        api.execute(
            handle,
            Command::CreateLabel {
                log_pos: Vertex::new(80, 100),
                text: Signal::new(String::from("Selected option: ")),
                on_loop_iter: None,
                font_size: Some(4),
                styling: None,
            },
        );

        api.execute(
            handle,
            Command::CreateLabel {
                log_pos: Vertex::new(450, 100),
                text: Rc::clone(&option),
                on_loop_iter: None,
                font_size: Some(4),
                styling: None,
            },
        );
    }
}
