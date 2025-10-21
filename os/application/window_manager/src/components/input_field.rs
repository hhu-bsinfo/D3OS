use alloc::{boxed::Box, rc::Rc, string::String};
use drawer::{drawer::Drawer, rect_data::RectData};
use graphic::{
    color::{Color, CYAN},
    lfb::{DEFAULT_CHAR_HEIGHT, DEFAULT_CHAR_WIDTH},
};
use terminal::DecodedKey;

use crate::{
    config::{BACKSPACE_UNICODE, INTERACT_BUTTON}, signal::{ComponentRef, ComponentRefExt}, WindowManager
};

use super::{component::{Casts, Clearable, Component, ComponentStyling, Disableable, Focusable, Hideable, Interactable}, container::Container};

pub const INPUT_BG_COLOR_ENABLED: Color = Color { red: 80, green: 80, blue: 80, alpha: 255 };
pub const INPUT_BG_COLOR_DISABLED: Color = Color { red: 50, green: 50, blue: 50, alpha: 255 };

pub const INPUT_BORDER_COLOR_ENABLED: Color = Color { red: 120, green: 120, blue: 120, alpha: 255 };
pub const INPUT_BORDER_COLOR_DISABLED: Color = Color { red: 80, green: 80, blue: 80, alpha: 255 };
pub const INPUT_BORDER_COLOR_SELECTED: Color = CYAN;

pub const TEXT_COLOR_ENABLED: Color = Color { red: 255, green: 255, blue: 255, alpha: 255 };
pub const TEXT_COLOR_DISABLED: Color = Color { red: 120, green: 120, blue: 120, alpha: 255 };

pub struct InputField {
    /**
    If we are selected, all keyboard input is redirected to us, unless
    command-line-window is opened
    */
    id: usize,
    is_dirty: bool,
    is_selected: bool,
    max_chars: usize,
    abs_rect_data: RectData,
    rel_rect_data: RectData,
    orig_rect_data: RectData,
    drawn_rect_data: RectData,
    rel_font_size: usize,
    font_scale: (u32, u32),
    current_text: String,

    // interactable
    on_change: Rc<Box<dyn Fn(String) -> ()>>,

    // disableable
    is_disabled: bool,
    // hideable
    is_hidden: bool,

    styling: ComponentStyling,
}

impl InputField {
    pub fn new(
        rel_rect_data: RectData,
        rel_font_size: usize,
        max_chars: usize,
        starting_text: String,
        on_submit: Option<Box<dyn Fn(String) -> ()>>,
        styling: Option<ComponentStyling>,
    ) -> ComponentRef {
        let input_field = Box::new(
            Self {
                id: WindowManager::generate_id(),
                is_dirty: true,
                is_selected: false,
                max_chars,
                abs_rect_data: RectData::zero(),
                rel_rect_data,
                rel_font_size,
                orig_rect_data: rel_rect_data.clone(),
                drawn_rect_data: RectData::zero(),
                font_scale: (1, 1),
                current_text: starting_text,

                on_change: Rc::new(on_submit.unwrap_or_else(|| Box::new(|_| {}))),

                is_disabled: false,
                is_hidden: false,

                styling: styling.unwrap_or_default(),
            }
        );

        let component = ComponentRef::from_component(input_field);

        component
    }
}

impl Component for InputField {
    fn draw(&mut self, focus_id: Option<usize>) {
        if !self.is_dirty {
            return;
        }

        if self.is_hidden {
            self.is_dirty = false;
            return;
        }

        let styling = &self.styling;
        let is_focused = focus_id == Some(self.id);

        let bg_color = if self.is_disabled {
            styling.disabled_background_color
        } else if is_focused {
            styling.focused_background_color
        } else {
            styling.background_color
        };

        let border_color = if self.is_selected {
            styling.selected_border_color
        } else if self.is_disabled {
            styling.disabled_border_color
        } else if is_focused {
            styling.focused_border_color
        } else {
            styling.border_color
        };

        let text_color = if self.is_disabled {
            styling.disabled_text_color
        } else {
            styling.text_color
        };

        Drawer::draw_filled_rectangle(self.abs_rect_data, bg_color, Some(border_color));

        self.drawn_rect_data = self.abs_rect_data;

        Drawer::draw_string(
            self.current_text.clone(),
            self.abs_rect_data.top_left.add(
                2,
                (self.abs_rect_data.height - DEFAULT_CHAR_HEIGHT * self.font_scale.1) / 2,
            ),
            text_color,
            None,
            self.font_scale,
        );

        self.is_dirty = false;
    }

    fn rescale_to_container(&mut self, parent: &dyn Container) {
        let styling: &ComponentStyling = &self.styling;

        self.font_scale = parent.scale_font_to_container(self.rel_font_size);

        let min_dim = (
            self.max_chars as u32 * DEFAULT_CHAR_WIDTH * self.font_scale.0,
            DEFAULT_CHAR_HEIGHT * self.font_scale.1,
        );
        let max_dim = (self.orig_rect_data.width, self.orig_rect_data.height);

        self.abs_rect_data = parent.scale_to_container(
            self.rel_rect_data,
            min_dim,
            max_dim,
            styling.maintain_aspect_ratio,
        );

        self.mark_dirty();
    }

    fn get_abs_rect_data(&self) -> RectData {
        self.abs_rect_data
    }

    fn get_id(&self) -> usize {
        self.id
    }

    fn is_dirty(&self) -> bool {
        self.is_dirty
    }

    fn mark_dirty(&mut self) {
        self.is_dirty = true;
    }

    fn get_drawn_rect_data(&self) -> RectData {
        self.drawn_rect_data
    }
}

impl Casts for InputField {
    fn as_disableable(&self) -> Option<&dyn Disableable> {
        Some(self)
    }

    fn as_hideable(&self) -> Option<&dyn Hideable> {
        Some(self)
    }

    fn as_focusable(&self) -> Option<&dyn Focusable> {
        Some(self)
    }

    fn as_focusable_mut(&mut self) -> Option<&mut dyn Focusable> {
        Some(self)
    }

    fn as_interactable(&self) -> Option<&dyn Interactable> {
        Some(self)
    }

    fn as_disableable_mut(&mut self) -> Option<&mut dyn Disableable> {
        Some(self)
    }

    fn as_hideable_mut(&mut self) -> Option<&mut dyn Hideable> {
        Some(self)
    }

    fn as_interactable_mut(&mut self) -> Option<&mut dyn Interactable> {
        Some(self)
    }

    fn as_clearable_mut(&mut self) -> Option<&mut dyn Clearable> {
        Some(self)
    }
}

impl Focusable for InputField {
    fn can_unfocus(&self) -> bool {
        !self.is_selected
    }

    fn focus(&mut self) {
        self.mark_dirty();
    }

    fn unfocus(&mut self) {
        self.is_selected = false;
        self.mark_dirty();
    }
}

impl Interactable for InputField {
    fn consume_keyboard_press(&mut self, keyboard_press: DecodedKey) -> Option<Box<dyn FnOnce() -> ()>> {
        if keyboard_press == INTERACT_BUTTON && !self.is_selected && !self.is_disabled {
            self.is_selected = true;
            self.mark_dirty();
        } else if self.is_selected {
            self.mark_dirty();
            match keyboard_press {
                DecodedKey::Unicode('\n') => {
                    self.is_selected = false;
                    self.mark_dirty();
                }
                BACKSPACE_UNICODE => {
                    self.current_text.pop();
                    self.mark_dirty();

                    let on_change = Rc::clone(&self.on_change);
                    let value = self.current_text.clone();

                    return Some(
                        Box::new(move || {
                            (on_change)(value);
                        })
                    );
                }
                DecodedKey::Unicode(c) => {
                    if self.current_text.len() < self.max_chars {
                        self.current_text.push(c);
                        self.mark_dirty();

                    }
                    
                    let on_change = Rc::clone(&self.on_change);
                    let value = self.current_text.clone();

                    return Some(
                        Box::new(move || {
                            (on_change)(value);
                        })
                    );
                },
                _ => {}
            }
        }

        return None;
    }

    fn consume_mouse_event(&mut self, mouse_event: &crate::mouse_state::MouseEvent) -> Option<Box<dyn FnOnce() -> ()>> {
        if mouse_event.buttons.left.is_pressed() && !self.is_disabled {
            self.is_selected = !self.is_selected;
            self.mark_dirty();
        }

        None
    }
}

impl Disableable for InputField {
    fn disable(&mut self) {
        self.is_disabled = true;
        self.mark_dirty();
    }

    fn enable(&mut self) {
        self.is_disabled = false;
        self.mark_dirty();
    }

    fn is_disabled(&self) -> bool {
        self.is_disabled
    }
}

impl Hideable for InputField {
    fn is_hidden(&self) -> bool {
        self.is_hidden
    }

    fn hide(&mut self) {
        self.is_hidden = true;
        self.disable();
        self.mark_dirty();
    }

    fn show(&mut self) {
        self.is_hidden = false;
        self.enable();
        self.mark_dirty();
    }
}

impl Clearable for InputField {
    fn clear(&mut self) {
        self.current_text.clear();
        (self.on_change)(self.current_text.clone());
        self.mark_dirty();
    }
}