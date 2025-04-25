use alloc::{boxed::Box, format, string::String};
use drawer::{rect_data::RectData, vertex::Vertex};

use crate::{
    api::{Api, Command}, components::container::basic_container::ContainerLayout, signal::{ComponentRef, Signal}, WindowManager
};

use super::runnable::Runnable;

pub struct LayoutApp;

impl Runnable for LayoutApp {
    fn run() {
        let window_handle = concurrent::thread::current().unwrap().id();
        let api = WindowManager::get_api();

        // Container
        let container_1 = api.execute(
            window_handle,
            None,
            Command::CreateContainer {
                log_rect_data: RectData {
                    top_left: Vertex { x: 50, y: 50 },
                    width: 300,
                    height: 200,
                },
                layout: ContainerLayout::Vertical,
                styling: None,
            },
        ).expect("failed to create container");

        fn create_button(
            api: &Api,
            window_handle: usize,
            parent: ComponentRef,
            x: u32,
            y: u32,
            width: u32,
            height: u32,
            label: &str,
        ) -> Option<ComponentRef> {
            api.execute(
                window_handle,
                Some(parent),
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
        //let button_1 = create_button(&api, window_handle, container_1.clone(), 0, 0, 100, 750, "A");
        //let button_2 = create_button(&api, window_handle, container_1.clone(), 0, 0, 100, 750, "B");
        //let button_3 = create_button(&api, window_handle, container_1.clone(), 0, 0, 1000, 100, "C");
        //let button_4 = create_button(&api, window_handle, container_1.clone(), 0, 110, 1000, 100, "D");

        // Create 5 buttons in a loop
        for i in 0..5 {
            let button = create_button(
                &api,
                window_handle,
                container_1.clone(),
                0,
                0,
                200,
                200,
                &format!("{}", i),
            );
        }
    }
}
