use alloc::{
    boxed::Box,
    rc::Rc,
    string::{String, ToString},
};
use drawer::{drawer::Drawer, rect_data::RectData, vertex::Vertex};
use graphic::lfb::{DEFAULT_CHAR_HEIGHT, DEFAULT_CHAR_WIDTH};
use terminal::DecodedKey;

use crate::{
    config::INTERACT_BUTTON,
    mouse_state::MouseEvent,
    signal::{ComponentRef, ComponentRefExt, Signal, Stateful},
    WindowManager,
};

use super::{
    component::{
        Casts, Component, ComponentStyling, Disableable, Focusable, Hideable, Interactable,
        Resizable,
    },
    container::Container,
};

pub struct Button {
    id: usize,
    is_dirty: bool,
    abs_rect_data: RectData,
    rel_rect_data: RectData,
    orig_rect_data: RectData,
    drawn_rect_data: RectData,
    label: Option<Stateful<String>>,
    rel_font_size: usize,
    font_scale: (u32, u32),
    on_click: Rc<Box<dyn Fn() -> ()>>,
    is_disabled: bool,
    is_hidden: bool,
    styling: ComponentStyling,
}

impl Button {
    pub fn new(
        rel_rect_data: RectData,
        orig_rect_data: RectData,
        label: Option<Rc<Signal<String>>>,
        rel_font_size: usize,
        on_click: Option<Box<dyn Fn() -> ()>>,
        styling: Option<ComponentStyling>,
    ) -> ComponentRef {
        let signal_copy = label.clone();

        let button = Box::new(Self {
            id: WindowManager::generate_id(),
            is_dirty: true,
            abs_rect_data: RectData::zero(),
            orig_rect_data,
            drawn_rect_data: RectData::zero(),
            rel_rect_data,
            rel_font_size,
            font_scale: (1, 1),
            label,
            on_click: Rc::new(on_click.unwrap_or_else(|| Box::new(|| {}))),
            is_disabled: false,
            is_hidden: false,
            styling: styling.unwrap_or_default(),
        });

        let component = ComponentRef::from_component(button);

        if let Some(signal) = signal_copy {
            signal.register_component(Rc::clone(&component));
        };

        component
    }

    fn calc_label_pos(&self, label: &str) -> Vertex {
        let RectData {
            top_left,
            width,
            height,
        } = self.abs_rect_data;

        top_left.add(
            width.saturating_sub(
                DEFAULT_CHAR_WIDTH * self.font_scale.0 * (label.chars().count() as u32),
            ) / 2,
            height.saturating_sub(DEFAULT_CHAR_HEIGHT * self.font_scale.1) / 2,
        )
    }

    fn handle_click(&mut self) -> Option<Box<dyn FnOnce() -> ()>> {
        let on_click = Rc::clone(&self.on_click);
        self.mark_dirty();

        Some(Box::new(move || {
            (on_click)();
        }))
    }
}

impl Component for Button {
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

        let border_color = if is_focused {
            styling.focused_border_color
        } else if self.is_disabled {
            styling.disabled_border_color
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

        if let Some(label) = &self.label {
            let label = &label.get();
            let label_pos = self.calc_label_pos(label);

            Drawer::draw_string(
                label.to_string(),
                label_pos,
                text_color,
                None,
                self.font_scale,
            );
        }

        self.is_dirty = false;
    }

    fn rescale_to_container(&mut self, parent: &dyn Container) {
        let styling = &self.styling;

        let min_width = match &self.label {
            Some(label) => label.get().len() as u32 * DEFAULT_CHAR_WIDTH * self.font_scale.0,
            None => 0,
        };

        self.abs_rect_data = parent.scale_to_container(
            self.rel_rect_data,
            (min_width, DEFAULT_CHAR_HEIGHT * self.font_scale.1),
            (self.orig_rect_data.width, self.orig_rect_data.height),
            styling.maintain_aspect_ratio,
        );

        self.font_scale = parent.scale_font_to_container(self.rel_font_size);

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

impl Casts for Button {
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

    fn as_resizable(&self) -> Option<&dyn Resizable> {
        Some(self)
    }

    fn as_resizable_mut(&mut self) -> Option<&mut dyn Resizable> {
        Some(self)
    }
}

impl Focusable for Button {
    fn focus(&mut self) {
        self.mark_dirty();
    }

    fn unfocus(&mut self) {
        self.mark_dirty();
    }
}

impl Interactable for Button {
    fn consume_keyboard_press(&mut self, keyboard_press: DecodedKey) -> Option<Box<dyn FnOnce() -> ()>> {
        if keyboard_press == INTERACT_BUTTON && !self.is_disabled {
            self.handle_click()
        } else {
            None
        }
    }

    fn consume_mouse_event(&mut self, mouse_event: &MouseEvent) -> Option<Box<dyn FnOnce() -> ()>> {
        if mouse_event.buttons.left.is_pressed() && !self.is_disabled {
            self.handle_click()
        } else {
            None
        }
    }
}

impl Disableable for Button {
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

impl Hideable for Button {
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

impl Resizable for Button {
    fn rescale(&mut self, scale_factor: f64) {
        let min_width = match &self.label {
            Some(label) => label.get().len() as u32 * DEFAULT_CHAR_WIDTH * self.font_scale.0,
            None => 0,
        };

        let min_height = DEFAULT_CHAR_HEIGHT * self.font_scale.1;

        self.abs_rect_data.width =
            ((f64::from(self.abs_rect_data.width) * scale_factor) as u32).max(min_width);
        self.abs_rect_data.height =
            ((f64::from(self.abs_rect_data.height) * scale_factor) as u32).max(min_height);

        self.mark_dirty();
    }

    fn resize(&mut self, width: u32, height: u32) {
        let scaling_factor_x = width as f32 / self.abs_rect_data.width as f32;
        let scaling_factor_y = height as f32 / self.abs_rect_data.height as f32;

        self.abs_rect_data.width = width;
        self.abs_rect_data.height = height;

        self.rel_rect_data.width = (self.rel_rect_data.width as f32 * scaling_factor_x) as u32;
        self.rel_rect_data.height = (self.rel_rect_data.height as f32 * scaling_factor_y) as u32;

        self.mark_dirty();
    }
}
