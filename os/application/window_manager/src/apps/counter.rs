use crate::{alloc::string::ToString, components::{checkbox, component::{ComponentStylingBuilder, Hideable}}, signal::{ComponentRef, Signal}};
use alloc::{boxed::Box, rc::Rc, string::String};
use drawer::{rect_data::RectData, vertex::Vertex};
use spin::RwLock;

use crate::{api::Command, WindowManager};

use super::runnable::Runnable;

pub struct Counter;

impl Runnable for Counter {
    fn run() {
        let handle = concurrent::thread::current().id();
        let api = WindowManager::get_api();

        let label = Signal::new(String::from("0"));
        let label_inc = Rc::clone(&label);
        let label_reset = Rc::clone(&label);

        let counter_button: Rc<RwLock<Option<ComponentRef>>> = Rc::new(RwLock::new(None));
        let reset_button: Rc<RwLock<Option<ComponentRef>>> = Rc::new(RwLock::new(None));

        let counter_button_checkbox = Rc::clone(&counter_button);
        let reset_button_checkbox = Rc::clone(&reset_button);

        api.execute(
            handle,
            Command::CreateCheckbox {
                log_rect_data: RectData {
                    top_left: Vertex::new(200, 50),
                    width: 25,
                    height: 25,
                },
                state: true,
                on_change: Some(Box::new(move |checked: bool| {
                    if let Some(counter_button) = counter_button_checkbox.read().as_ref() {
                        if let Some(disableable) = counter_button.write().as_disableable_mut() {
                            if checked {
                                disableable.enable();
                            } else {
                                disableable.disable();
                            }
                        }
                    }

                    if let Some(reset_button) = reset_button_checkbox.read().as_ref() {
                        if let Some(hideable) = reset_button.write().as_hideable_mut() {
                            if checked {
                                hideable.show();
                            } else {
                                hideable.hide();
                            }                        }
                    }
                })),
                styling: Some(ComponentStylingBuilder::new().maintain_aspect_ratio(true).build()),
            },
        );


        *counter_button.write() = Some(api.execute(
            handle,
            Command::CreateButton {
                log_rect_data: RectData {
                    top_left: Vertex::new(200, 150),
                    width: 200,
                    height: 50,
                },
                label: Some((Rc::clone(&label), 1)),
                on_click: Some(Box::new(move || {
                    let old = (label_inc.get()).parse::<usize>().unwrap();
                    label_inc.set((old + 1).to_string());
                })),
                styling: None,
            },
        ).unwrap());

        *reset_button.write() = Some(api.execute(
            handle,
            Command::CreateButton {
                log_rect_data: RectData {
                    top_left: Vertex::new(200, 250),
                    width: 200,
                    height: 50,
                },
                label: Some((Signal::new(String::from("Reset")), 1)),
                on_click: Some(Box::new(move || {
                    label_reset.set(String::from("0"));
                })),
                styling: None,
            },
        ).unwrap());
    }
}
