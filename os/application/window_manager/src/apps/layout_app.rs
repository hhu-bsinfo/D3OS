use alloc::{boxed::Box, format, string::String};
use drawer::{rect_data::RectData, vertex::Vertex};
use graphic::color::Color;
use log::debug;

use crate::{
    api::{Api, Command},
    components::{
        component::ComponentStylingBuilder,
        container::{
            container_layout::{
                AlignmentMode, ContainerLayoutBuilder, HorDirection, StretchMode, VertDirection,
            },
            ContainerStylingBuilder,
        },
    },
    signal::{ComponentRef, Signal},
    WindowManager,
};

use super::runnable::Runnable;

pub struct LayoutApp;

impl Runnable for LayoutApp {
    fn run() {
        let window_handle = concurrent::thread::current().unwrap().id();
        let api = WindowManager::get_api();

        // Containers
        let container_1 = api
            .execute(
                window_handle,
                None,
                Command::CreateContainer {
                    log_rect_data: RectData {
                        top_left: Vertex { x: 50, y: 50 },
                        width: 300,
                        height: 500,
                    },
                    layout: Some(
                        ContainerLayoutBuilder::new()
                            .alignment(AlignmentMode::Vertical(VertDirection::Top))
                            .stretch(StretchMode::Fill)
                            .build(),
                    ),
                    styling: None,
                },
            )
            .expect("failed to create container");

        let container_2 = api
            .execute(
                window_handle,
                Some(container_1.clone()),
                Command::CreateContainer {
                    log_rect_data: RectData {
                        top_left: Vertex::zero(),
                        width: 0,
                        height: 100,
                    },
                    layout: Some(
                        ContainerLayoutBuilder::new()
                            .alignment(AlignmentMode::Horizontal(HorDirection::Right))
                            .stretch(StretchMode::Fill)
                            .build(),
                    ),
                    styling: Some(
                        ContainerStylingBuilder::new()
                            .border_color(Color::new(255, 0, 0, 255))
                            .build(),
                    ),
                },
            )
            .expect("failed to create container");

        let container_3 = api
            .execute(
                window_handle,
                Some(container_1.clone()),
                Command::CreateContainer {
                    log_rect_data: RectData {
                        top_left: Vertex::zero(),
                        width: 0,
                        height: 400,
                    },
                    layout: Some(
                        ContainerLayoutBuilder::new()
                            .alignment(AlignmentMode::Vertical(VertDirection::Bottom))
                            .stretch(StretchMode::Fill)
                            .build(),
                    ),
                    styling: Some(
                        ContainerStylingBuilder::new()
                            .border_color(Color::new(0, 255, 0, 255))
                            .maintain_aspect_ratio(true)
                            .build(),
                    ),
                },
            )
            .expect("failed to create container");

        let container_4 = api
            .execute(
                window_handle,
                Some(container_1.clone()),
                Command::CreateContainer {
                    log_rect_data: RectData {
                        top_left: Vertex::zero(),
                        width: 0,
                        height: 100,
                    },
                    layout: Some(
                        ContainerLayoutBuilder::new()
                            .stretch(StretchMode::Fill)
                            .build(),
                    ),
                    styling: Some(
                        ContainerStylingBuilder::new()
                            .border_color(Color::new(255, 0, 255, 255))
                            .build(),
                    ),
                },
            )
            .expect("failed to create container");

        fn create_button(
            api: &Api,
            window_handle: usize,
            parent: Option<ComponentRef>,
            x: u32,
            y: u32,
            width: u32,
            height: u32,
            label: &str,
        ) -> Option<ComponentRef> {
            api.execute(
                window_handle,
                parent,
                Command::CreateButton {
                    log_rect_data: RectData {
                        top_left: Vertex { x, y },
                        width,
                        height,
                    },
                    label: Some((Signal::new(String::from(label)), 1)),
                    on_click: Some(Box::new(move || {
                        debug!("click!");
                    })),
                    styling: Some(
                        ComponentStylingBuilder::new()
                            .maintain_aspect_ratio(true)
                            .build(),
                    ),
                },
            )
            .ok()
        }

        let grid_container = api
            .execute(
                window_handle,
                None,
                Command::CreateContainer {
                    log_rect_data: RectData {
                        top_left: Vertex { x: 400, y: 50 },
                        width: 250,
                        height: 250,
                    },
                    layout: Some(
                        ContainerLayoutBuilder::new()
                            .alignment(AlignmentMode::Grid(3))
                            .build(),
                    ),
                    styling: Some(
                        ContainerStylingBuilder::new()
                            .maintain_aspect_ratio(true)
                            .build(),
                    ),
                },
            )
            .expect("failed to create container");

        // Buttons
        //let button_1 = create_button(&api, window_handle, container_1.clone(), 0, 0, 100, 750, "A");
        //let button_2 = create_button(&api, window_handle, container_1.clone(), 0, 0, 100, 750, "B");
        //let button_3 = create_button(&api, window_handle, container_1.clone(), 0, 0, 1000, 100, "C");
        //let button_4 = create_button(&api, window_handle, container_1.clone(), 0, 110, 1000, 100, "D");

        // Create some buttons
        for i in 0..3 {
            let _ = create_button(
                &api,
                window_handle,
                Some(container_2.clone()),
                0,
                0,
                150,
                0,
                &format!("{}", i),
            );
        }

        /*let _ = create_button(
            &api,
            window_handle,
            None,
            0,
            0,
            200,
            100,
            &format!("{}", 0),
        );*/

        for i in 0..3 {
            let _ = create_button(
                &api,
                window_handle,
                Some(container_3.clone()),
                0,
                0,
                0,
                200,
                &format!("{}", i),
            );
        }

        create_button(
            &api,
            window_handle,
            Some(container_4.clone()),
            0,
            0,
            0,
            0,
            "Hello!",
        );

        for i in 0..9 {
            let _ = create_button(
                &api,
                window_handle,
                Some(grid_container.clone()),
                0,
                0,
                200,
                200,
                &format!("{}", i),
            );
        }
    }
}
