use crate::{
    alloc::string::ToString,
    components::{
        component::ComponentStylingBuilder,
        container::{
            basic_container::{AlignmentMode, LayoutMode, StretchMode},
            ContainerStylingBuilder,
        },
    },
    signal::{ComponentRef, Signal},
};
use alloc::{boxed::Box, format, rc::Rc, string::String, vec};
use drawer::{rect_data::RectData, vertex::Vertex};
use graphic::color::{Color, GREY, WHITE};
use spin::rwlock::RwLock;

use crate::{api::Command, WindowManager};

use super::runnable::Runnable;

pub struct Calculator;

#[derive(Clone, Copy)]
enum CalculatorOperation {
    None,
    Add,
    Subtract,
    Multiply,
    Divide,
}

impl Runnable for Calculator {
    fn run() {
        let handle = concurrent::thread::current()
            .expect("Failed to get thread")
            .id();
        let api = WindowManager::get_api();

        let display_text = Signal::new(String::from("0"));
        let equals_button: Rc<RwLock<Option<ComponentRef>>> = Rc::new(RwLock::new(None));

        //let current_operator = Rc::new(RwLock::<Option<String>>::new(None));
        let current_operator = Rc::new(RwLock::<CalculatorOperation>::new(
            CalculatorOperation::None,
        ));
        let stored_value: Rc<RwLock<f64>> = Rc::new(RwLock::new(0.0));

        let display_sign = Rc::clone(&display_text);
        let display_decimal = Rc::clone(&display_text);

        // Equals Button
        let equals_clone = Rc::clone(&display_text);
        let equals_operator_clone = Rc::clone(&current_operator);
        let equals_stored_clone = Rc::clone(&stored_value);

        // Display Label
        api.execute(
            handle,
            None,
            Command::CreateLabel {
                log_pos: Vertex::new(50, 50),
                font_size: Some(1),
                text: Rc::clone(&display_text),
                on_loop_iter: None,
                styling: None,
            },
        );

        // Button container
        let button_container = api
            .execute(
                handle,
                None,
                Command::CreateContainer {
                    log_rect_data: RectData {
                        top_left: Vertex { x: 50, y: 120 },
                        width: 250,
                        height: 250,
                    },
                    layout: LayoutMode::Grid(4),
                    stretch: StretchMode::None,
                    styling: Some(
                        ContainerStylingBuilder::new()
                            .maintain_aspect_ratio(true)
                            .build(),
                    ),
                },
            )
            .unwrap();

        // Helper to create number buttons
        let create_number_button = |parent: ComponentRef, number: usize| {
            let text_clone = Rc::clone(&display_text);

            api.execute(
                handle,
                Some(parent),
                Command::CreateButton {
                    log_rect_data: RectData {
                        top_left: Vertex::zero(),
                        width: 200,
                        height: 200,
                    },
                    label: Some((Signal::new(number.to_string()), 1)),
                    on_click: Some(Box::new(move || {
                        let value = text_clone.get();
                        if value == "0" {
                            text_clone.set(number.to_string());
                        } else {
                            text_clone.set(format!("{}{}", value, number));
                        }
                    })),
                    styling: Some(
                        ComponentStylingBuilder::new()
                            .background_color(GREY.bright())
                            .focused_background_color(GREY.bright())
                            .text_color(WHITE.bright())
                            .focused_text_color(WHITE.bright())
                            .maintain_aspect_ratio(true)
                            .build(),
                    ),
                },
            );
        };

        // Helper to create operator buttons
        let create_operator_button =
            |parent: ComponentRef, label: &str, operation: CalculatorOperation| {
                let text_clone = Rc::clone(&display_text);
                let stored_value_clone = Rc::clone(&stored_value);
                let operator_clone = Rc::clone(&current_operator);
                let equals_button_in_operator = Rc::clone(&equals_button);

                api.execute(
                    handle,
                    Some(parent),
                    Command::CreateButton {
                        log_rect_data: RectData {
                            top_left: Vertex::zero(),
                            width: 200,
                            height: 200,
                        },
                        label: Some((Signal::new(label.to_string()), 1)),
                        on_click: Some(Box::new(move || {
                            let value = text_clone.get();
                            let mut stored = stored_value_clone.write();
                            *stored = value.parse::<f64>().unwrap_or(0.0);

                            text_clone.set(String::from("0"));

                            let mut operator = operator_clone.write();
                            *operator = operation;

                            if let Some(equals_button) = equals_button_in_operator.read().as_ref() {
                                if let Some(disableable) =
                                    equals_button.write().as_disableable_mut()
                                {
                                    disableable.enable();
                                }
                            }
                        })),
                        styling: Some(
                            ComponentStylingBuilder::new()
                                .maintain_aspect_ratio(true)
                                .build(),
                        ),
                    },
                );
            };

        // Buttons
        create_number_button(button_container.clone(), 0);
        create_number_button(button_container.clone(), 1);
        create_number_button(button_container.clone(), 2);
        create_operator_button(button_container.clone(), "+", CalculatorOperation::Add);

        create_number_button(button_container.clone(), 3);
        create_number_button(button_container.clone(), 4);
        create_number_button(button_container.clone(), 5);
        create_operator_button(button_container.clone(), "-", CalculatorOperation::Subtract);

        create_number_button(button_container.clone(), 6);
        create_number_button(button_container.clone(), 7);
        create_number_button(button_container.clone(), 8);
        create_operator_button(button_container.clone(), "*", CalculatorOperation::Multiply);

        create_number_button(button_container.clone(), 9);

        api.execute(
            handle,
            Some(button_container.clone()),
            Command::CreateButton {
                log_rect_data: RectData {
                    top_left: Vertex::zero(),
                    width: 200,
                    height: 200,
                },
                label: Some((Signal::new(String::from(".")), 1)),
                on_click: Some(Box::new(move || {
                    let value = display_decimal.get();
                    if !value.contains('.') {
                        display_decimal.set(format!("{}.", value));
                    }
                })),
                styling: Some(
                    ComponentStylingBuilder::new()
                        .maintain_aspect_ratio(true)
                        .build(),
                ),
            },
        );

        api.execute(
            handle,
            Some(button_container.clone()),
            Command::CreateButton {
                log_rect_data: RectData {
                    top_left: Vertex::zero(),
                    width: 200,
                    height: 200,
                },
                label: Some((Signal::new(String::from("+/-")), 1)),
                on_click: Some(Box::new(move || {
                    let mut value = display_sign.get().parse::<f64>().unwrap_or(0.0);
                    value *= -1.0;
                    display_sign.set(value.to_string());
                })),
                styling: Some(
                    ComponentStylingBuilder::new()
                        .maintain_aspect_ratio(true)
                        .build(),
                ),
            },
        );

        create_operator_button(button_container.clone(), "/", CalculatorOperation::Divide);

        let display_clear = Rc::clone(&display_text);
        let equals_button_in_clear = Rc::clone(&equals_button);
        let stored_value_clear = Rc::clone(&stored_value);

        const CLEAR_BUTTON_COLOR: Color = Color {
            red: 255,
            green: 165,
            blue: 0,
            alpha: 255,
        };

        // Clear Button
        api.execute(
            handle,
            None,
            Command::CreateButton {
                log_rect_data: RectData {
                    top_left: Vertex::new(50, 360),
                    width: 110,
                    height: 50,
                },
                label: Some((Signal::new(String::from("C")), 1)),
                on_click: Some(Box::new(move || {
                    display_clear.set(String::from("0"));

                    let mut stored = stored_value_clear.write();
                    *stored = 0.0;

                    let mut operator = current_operator.write();
                    *operator = CalculatorOperation::None;

                    if let Some(equals_button) = equals_button_in_clear.read().as_ref() {
                        if let Some(disableable) = equals_button.write().as_disableable_mut() {
                            disableable.disable();
                        }
                    }
                })),
                styling: Some(
                    ComponentStylingBuilder::new()
                        .text_color(WHITE.bright())
                        .background_color(CLEAR_BUTTON_COLOR)
                        .disabled_background_color(CLEAR_BUTTON_COLOR)
                        .focused_background_color(CLEAR_BUTTON_COLOR)
                        .selected_background_color(CLEAR_BUTTON_COLOR)
                        .border_color(CLEAR_BUTTON_COLOR)
                        .disabled_border_color(CLEAR_BUTTON_COLOR)
                        .build(),
                ),
            },
        );

        let equals_button_init = Rc::clone(&equals_button);
        *equals_button_init.write() = Some(
            api.execute(
                handle,
                None,
                Command::CreateButton {
                    log_rect_data: RectData {
                        top_left: Vertex::new(170, 360),
                        width: 110,
                        height: 50,
                    },
                    label: Some((Signal::new(String::from("=")), 1)),
                    on_click: Some(Box::new(move || {
                        let value = equals_clone.get();
                        let stored = *equals_stored_clone.write();
                        let operator = equals_operator_clone.read().clone();

                        let current_value = value.parse::<f64>().unwrap_or(0.0);
                        let result = match operator {
                            CalculatorOperation::Add => stored + current_value,
                            CalculatorOperation::Subtract => stored - current_value,
                            CalculatorOperation::Multiply => stored * current_value,
                            CalculatorOperation::Divide => {
                                if current_value != 0.0 {
                                    stored / current_value
                                } else {
                                    return; // Division durch 0 verhindern
                                }
                            }
                            _ => return,
                        };

                        equals_clone.set(result.to_string());
                    })),
                    styling: None,
                },
            )
            .unwrap(),
        );

        let equals_button_disabling = Rc::clone(&equals_button);
        if let Some(equals_button) = equals_button_disabling.read().as_ref() {
            if let Some(disableable) = equals_button.write().as_disableable_mut() {
                disableable.disable();
            }
        };
    }
}
