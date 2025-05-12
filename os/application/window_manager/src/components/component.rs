use alloc::boxed::Box;
use drawer::rect_data::RectData;
use graphic::color::Color;

use crate::{config::{DEFAULT_BACKGROUND_COLOR, DEFAULT_BORDER_COLOR, DEFAULT_DISABLED_BACKGROUND_COLOR, DEFAULT_DISABLED_BORDER_COLOR, DEFAULT_DISABLED_TEXT_COLOR, DEFAULT_FOCUSED_BACKGROUND_COLOR, DEFAULT_FOCUSED_BORDER_COLOR, DEFAULT_FOCUSED_TEXT_COLOR, DEFAULT_SELECTED_BACKGROUND_COLOR, DEFAULT_SELECTED_BORDER_COLOR, DEFAULT_SELECTED_TEXT_COLOR, DEFAULT_TEXT_COLOR}, signal::ComponentRef};

pub use crate::mouse_state::MouseEvent;

use super::container::Container;

#[derive(Clone, Copy)]
pub struct ComponentStyling {
    pub maintain_aspect_ratio: bool,

    pub border_color: Color,
    pub background_color: Color,
    pub text_color: Color,

    pub focused_border_color: Color,
    pub focused_background_color: Color,
    pub focused_text_color: Color,

    pub selected_border_color: Color,
    pub selected_background_color: Color,
    pub selected_text_color: Color,

    pub disabled_border_color: Color,
    pub disabled_background_color: Color,
    pub disabled_text_color: Color,
}

impl Default for ComponentStyling {
    fn default() -> Self {
        ComponentStyling {
            maintain_aspect_ratio: false,

            border_color: DEFAULT_BORDER_COLOR,
            background_color: DEFAULT_BACKGROUND_COLOR,
            text_color: DEFAULT_TEXT_COLOR,

            focused_border_color: DEFAULT_FOCUSED_BORDER_COLOR,
            focused_background_color: DEFAULT_FOCUSED_BACKGROUND_COLOR,
            focused_text_color: DEFAULT_FOCUSED_TEXT_COLOR,

            selected_border_color: DEFAULT_SELECTED_BORDER_COLOR,
            selected_background_color: DEFAULT_SELECTED_BACKGROUND_COLOR,
            selected_text_color: DEFAULT_SELECTED_TEXT_COLOR,

            disabled_border_color: DEFAULT_DISABLED_BORDER_COLOR,
            disabled_background_color: DEFAULT_DISABLED_BACKGROUND_COLOR,
            disabled_text_color: DEFAULT_DISABLED_TEXT_COLOR,
        }
    }
}

pub struct ComponentStylingBuilder {
    maintain_aspect_ratio: Option<bool>,

    border_color: Option<Color>,
    background_color: Option<Color>,
    text_color: Option<Color>,

    focused_border_color: Option<Color>,
    focused_background_color: Option<Color>,
    focused_text_color: Option<Color>,

    selected_border_color: Option<Color>,
    selected_background_color: Option<Color>,
    selected_text_color: Option<Color>,

    disabled_border_color: Option<Color>,
    disabled_background_color: Option<Color>,
    disabled_text_color: Option<Color>,
}

impl ComponentStylingBuilder {
    pub fn new() -> Self {
        Self {
            maintain_aspect_ratio: None,

            border_color: None,
            background_color: None,
            text_color: None,

            focused_border_color: None,
            focused_background_color: None,
            focused_text_color: None,

            selected_border_color: None,
            selected_background_color: None,
            selected_text_color: None,

            disabled_border_color: None,
            disabled_background_color: None,
            disabled_text_color: None,
        }
    }

    pub fn maintain_aspect_ratio(&mut self, maintain_aspect_ratio: bool) -> &mut Self {
        self.maintain_aspect_ratio = Some(maintain_aspect_ratio);
        self
    }

    pub fn border_color(&mut self, color: Color) -> &mut Self {
        self.border_color = Some(color);
        self
    }

    pub fn background_color(&mut self, color: Color) -> &mut Self {
        self.background_color = Some(color);
        self
    }

    pub fn text_color(&mut self, color: Color) -> &mut Self {
        self.text_color = Some(color);
        self
    }

    pub fn focused_border_color(&mut self, color: Color) -> &mut Self {
        self.focused_border_color = Some(color);
        self
    }

    pub fn focused_background_color(&mut self, color: Color) -> &mut Self {
        self.focused_background_color = Some(color);
        self
    }

    pub fn focused_text_color(&mut self, color: Color) -> &mut Self {
        self.focused_text_color = Some(color);
        self
    }

    pub fn selected_border_color(&mut self, color: Color) -> &mut Self {
        self.selected_border_color = Some(color);
        self
    }

    pub fn selected_background_color(&mut self, color: Color) -> &mut Self {
        self.selected_background_color = Some(color);
        self
    }

    pub fn selected_text_color(&mut self, color: Color) -> &mut Self {
        self.selected_text_color = Some(color);
        self
    }

    pub fn disabled_border_color(&mut self, color: Color) -> &mut Self {
        self.disabled_border_color = Some(color);
        self
    }

    pub fn disabled_background_color(&mut self, color: Color) -> &mut Self {
        self.disabled_background_color = Some(color);
        self
    }

    pub fn disabled_text_color(&mut self, color: Color) -> &mut Self {
        self.disabled_text_color = Some(color);
        self
    }

    pub fn build(&mut self) -> ComponentStyling {
        ComponentStyling {
            maintain_aspect_ratio: self.maintain_aspect_ratio.unwrap_or(false),

            border_color: self.border_color.unwrap_or(DEFAULT_BORDER_COLOR),
            background_color: self.background_color.unwrap_or(DEFAULT_BACKGROUND_COLOR),
            text_color: self.text_color.unwrap_or(DEFAULT_TEXT_COLOR),

            focused_border_color: self.focused_border_color.unwrap_or(DEFAULT_FOCUSED_BORDER_COLOR),
            focused_background_color: self.focused_background_color.unwrap_or(DEFAULT_FOCUSED_BACKGROUND_COLOR),
            focused_text_color: self.focused_text_color.unwrap_or(DEFAULT_FOCUSED_TEXT_COLOR),

            selected_border_color: self.selected_border_color.unwrap_or(DEFAULT_SELECTED_BORDER_COLOR),
            selected_background_color: self.selected_background_color.unwrap_or(DEFAULT_SELECTED_BACKGROUND_COLOR),
            selected_text_color: self.selected_text_color.unwrap_or(DEFAULT_SELECTED_TEXT_COLOR),

            disabled_background_color: self.disabled_background_color.unwrap_or(DEFAULT_DISABLED_BACKGROUND_COLOR),
            disabled_border_color: self.disabled_border_color.unwrap_or(DEFAULT_DISABLED_BORDER_COLOR),
            disabled_text_color: self.disabled_text_color.unwrap_or(DEFAULT_DISABLED_TEXT_COLOR),
        }
    }
}

/**
Any size-relations with the words "rel" or "relative" in them refer to the size inside the window
as if the window was occupying the full screen
*/

// pub trait Component: ComponentBehaviour + Observable {}

pub trait Component: Casts + {
    fn draw(&mut self, focus_id: Option<usize>);

    /// Called when the component is required to adjust its absolute bounds during the layout phase.
    /// The `parent` Container offers methods to scale relative bound to absolute bounds.
    fn rescale_to_container(&mut self, parent: &dyn Container);

    fn get_abs_rect_data(&self) -> RectData;

    fn get_drawn_rect_data(&self) -> RectData;

    fn is_dirty(&self) -> bool;

    fn get_id(&self) -> usize;

    fn mark_dirty(&mut self);
}

pub trait Disableable {
    fn is_disabled(&self) -> bool;

    fn disable(&mut self);
    
    fn enable(&mut self);
} 

pub trait Hideable {
    fn is_hidden(&self) -> bool;

    fn hide(&mut self);

    fn show(&mut self);
}

pub trait Focusable {
    fn can_unfocus(&self) -> bool { true }

    fn focus(&mut self);

    /// This will only be called if `can_unfocus()` returns true, unless the parent
    /// needs to force the unfocus NOW.
    fn unfocus(&mut self);
}

pub trait Interactable {
    fn consume_keyboard_press(&mut self, keyboard_press: char) -> Option<Box<dyn FnOnce() -> ()>>;

    fn consume_mouse_event(&mut self, mouse_event: &MouseEvent) -> Option<Box<dyn FnOnce() -> ()>>;
}

pub trait Resizable {
    fn rescale(&mut self, scale_factor: f64);

    /// Resizes the component to the given abs width and height by calculating a scaling factor.
    fn resize(&mut self, width: u32, height: u32);
}

pub trait Clearable {
    fn clear(&mut self);
}

pub trait Casts {
    fn as_disableable(&self) -> Option<&dyn Disableable> {
        None
    }
    
    fn as_disableable_mut(&mut self) -> Option<&mut dyn Disableable> {
        None
    }
    
    fn as_hideable(&self) -> Option<&dyn Hideable> {
        None
    }

    fn as_hideable_mut(&mut self) -> Option<&mut dyn Hideable> {
        None
    }

    fn as_focusable(&self) -> Option<&dyn Focusable> {
        None
    }
 
    fn as_focusable_mut(&mut self) -> Option<&mut dyn Focusable> {
        None
    }
    
    fn as_interactable(&self) -> Option<&dyn Interactable> {
        None
    }
 
    fn as_interactable_mut(&mut self) -> Option<&mut dyn Interactable> {
        None
    }

    fn as_resizable(&self) -> Option<&dyn Resizable> {
        None
    }

    fn as_resizable_mut(&mut self) -> Option<&mut dyn Resizable> {
        None
    }

    fn as_clearable(&self) -> Option<&dyn Clearable> {
        None
    }

    fn as_clearable_mut(&mut self) -> Option<&mut dyn Clearable> {
        None
    }

    fn as_container(&self) -> Option<&dyn Container> {
        None
    }

    fn as_container_mut(&mut self) -> Option<&mut dyn Container> {
        None
    }
}