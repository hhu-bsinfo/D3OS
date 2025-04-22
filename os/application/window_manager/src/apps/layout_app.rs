use alloc::{boxed::Box, string::String};
use drawer::{rect_data::RectData, vertex::Vertex};

use crate::{
    api::{Api, Command},
    signal::{ComponentRef, Signal},
    WindowManager,
};

use super::runnable::Runnable;

pub struct LayoutApp;

impl Runnable for LayoutApp {
    fn run() {
        let window_handle = concurrent::thread::current().unwrap().id();
        let api = WindowManager::get_api();

        fn create_button(
            api: &Api,
            window_handle: usize,
            x: u32,
            y: u32,
            width: u32,
            height: u32,
            label: &str,
        ) -> Option<ComponentRef> {
            api.execute(
                window_handle,
                Command::CreateButton {
                    log_rect_data: RectData {
                        top_left: Vertex { x, y },
                        width,
                        height,
                    },
                    label: Some((Signal::new(String::from(label)), 1)),
                    on_click: Some(Box::new(move || {
                        terminal::write::log_debug("click!");
                    })),
                    styling: None,
                },
            )
            .ok()
        }

        // Buttons
        let button_1 = create_button(&api, window_handle, 0, 0, 80, 50, "A");
        let button_2 = create_button(&api, window_handle, 0, 0, 80, 50, "B");
        let button_3 = create_button(&api, window_handle, 0, 0, 80, 50, "C");
        let button_4 = create_button(&api, window_handle, 0, 0, 80, 50, "D");

        // Container
        let container_1 = api.execute(
            window_handle,
            Command::CreateContainer {
                log_rect_data: RectData {
                    top_left: Vertex { x: 50, y: 50 },
                    width: 300,
                    height: 200,
                },
                styling: None,
            },
        );
    }
}
