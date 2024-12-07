use alloc::boxed::Box;
use drawer::rect_data::RectData;
use graphic::color::{Color, CYAN, GREY, WHITE, YELLOW};
#[derive(Clone, Copy)]
pub struct ComponentStyling {
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
        Self::default()
    }
}

pub struct ComponentStylingBuilder {
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

impl ComponentStyling {
    pub fn default() -> ComponentStyling {
        ComponentStyling {
            border_color: WHITE,
            background_color: GREY,
            text_color: WHITE,

            focused_border_color: YELLOW.bright(),
            focused_background_color: GREY,
            focused_text_color: WHITE,

            selected_border_color: CYAN,
            selected_background_color: GREY,
            selected_text_color: WHITE,

            disabled_border_color: GREY.bright(),
            disabled_background_color: GREY.dim(),
            disabled_text_color: GREY,
        }
    }
}

impl ComponentStylingBuilder {
    pub fn new() -> Self {
        Self {
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
            border_color: self.border_color.unwrap_or(WHITE),
            background_color: self.background_color.unwrap_or(GREY),
            text_color: self.text_color.unwrap_or(WHITE),

            focused_border_color: self.focused_border_color.unwrap_or(YELLOW.bright()),
            focused_background_color: self.focused_background_color.unwrap_or(GREY),
            focused_text_color: self.focused_text_color.unwrap_or(WHITE),

            selected_border_color: self.selected_border_color.unwrap_or(CYAN),
            selected_background_color: self.selected_background_color.unwrap_or(GREY),
            selected_text_color: self.selected_text_color.unwrap_or(WHITE),

            disabled_background_color: self.disabled_background_color.unwrap_or(GREY.dim()),
            disabled_border_color: self.disabled_border_color.unwrap_or(GREY.dim()),
            disabled_text_color: self.disabled_text_color.unwrap_or(GREY),
        }
    }
}

/**
Any size-relations with the words "rel" or "relative" in them refer to the size inside the window
as if the window was occupying the full screen
*/

// pub trait Component: ComponentBehaviour + Observable {}

pub trait Component: Casts + {
    fn draw(&mut self, is_focused: bool);

    /// Defines how rescaling the component-geometry works after the containing window has been resized
    fn rescale_after_split(&mut self, old_rect_data: RectData, new_rect_data: RectData);

    fn rescale_after_move(&mut self, new_rect_data: RectData);

    fn rescale(&mut self, old_window: RectData, new_window: RectData) {
        //
    }

    fn get_abs_rect_data(&self) -> RectData;

    fn get_drawn_rect_data(&self) -> RectData;

    fn is_dirty(&self) -> bool;

    fn set_id(&mut self, id: usize);

    fn get_id(&self) -> Option<usize>;

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

pub trait Interactable {
    fn consume_keyboard_press(&mut self, keyboard_press: char) -> Option<Box<dyn FnOnce() -> ()>>;
}

pub trait Resizable {
    fn rescale(&mut self, scale_factor: f64);

    fn resize(&mut self, width: u32, height: u32);
}

pub trait Clearable {
    fn clear(&mut self);
}

pub trait Casts {
    fn as_disableable(&self) -> Option<&dyn Disableable> {
        None
    }

    fn as_hideable(&self) -> Option<&dyn Hideable> {
        None
    }

    fn as_interactable(&self) -> Option<&dyn Interactable> {
        None
    }

    fn as_disableable_mut(&mut self) -> Option<&mut dyn Disableable> {
        None
    }
 
    fn as_hideable_mut(&mut self) -> Option<&mut dyn Hideable> {
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

    fn as_clearable_mut(&mut self) -> Option<&mut dyn Clearable> {
        None
    }
}