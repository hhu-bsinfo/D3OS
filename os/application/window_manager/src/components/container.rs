use drawer::{rect_data::RectData, vertex::Vertex};
use graphic::color::Color;

use crate::{
    config::{DEFAULT_BACKGROUND_COLOR, DEFAULT_BORDER_COLOR},
    signal::ComponentRef,
};

use super::component::Component;

pub mod basic_container;

#[derive(Clone, Copy)]
pub struct ContainerStyling {
    pub maintain_aspect_ratio: bool,
    pub show_border: bool,
    pub show_background: bool,
    pub child_padding: u32,

    pub border_color: Color,
    pub background_color: Color,
}

impl Default for ContainerStyling {
    fn default() -> Self {
        Self {
            maintain_aspect_ratio: false,
            show_border: true,
            show_background: false,
            child_padding: 5,

            border_color: DEFAULT_BORDER_COLOR,
            background_color: DEFAULT_BACKGROUND_COLOR,
        }
    }
}

pub struct ContainerStylingBuilder {
    maintain_aspect_ratio: Option<bool>,
    show_border: Option<bool>,
    show_background: Option<bool>,
    child_padding: Option<u32>,

    border_color: Option<Color>,
    background_color: Option<Color>,
}

impl ContainerStylingBuilder {
    pub fn new() -> Self {
        Self {
            maintain_aspect_ratio: None,
            show_border: None,
            show_background: None,
            child_padding: None,

            border_color: None,
            background_color: None,
        }
    }

    pub fn maintain_aspect_ratio(&mut self, maintain_aspect_ratio: bool) -> &mut Self {
        self.maintain_aspect_ratio = Some(maintain_aspect_ratio);
        self
    }

    pub fn show_border(&mut self, show_border: bool) -> &mut Self {
        self.show_border = Some(show_border);
        self
    }

    pub fn show_background(&mut self, show_background: bool) -> &mut Self {
        self.show_background = Some(show_background);
        self
    }

    pub fn child_padding(&mut self, child_padding: u32) -> &mut Self {
        self.child_padding = Some(child_padding);
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

    pub fn build(&mut self) -> ContainerStyling {
        ContainerStyling {
            maintain_aspect_ratio: self.maintain_aspect_ratio.unwrap_or(false),
            show_border: self.show_border.unwrap_or(true),
            show_background: self.show_background.unwrap_or(false),
            child_padding: self.child_padding.unwrap_or(5),

            border_color: self.border_color.unwrap_or(DEFAULT_BORDER_COLOR),
            background_color: self.background_color.unwrap_or(DEFAULT_BACKGROUND_COLOR),
        }
    }
}

/// A Component that can hold multiple child components.
pub trait Container: Component + FocusManager {
    fn add_child(&mut self, child: ComponentRef);

    fn remove_child(&mut self, id: usize);

    /// Scales a relative rect to the container and returns the absolute rect.
    fn scale_to_container(
        &self,
        rel_rect: RectData,
        min_dim: (u32, u32),
        max_dim: (u32, u32),
        maintain_aspect_ratio: bool,
    ) -> RectData;

    /// Scales a relative vertex to the container and returns the absolute vertex.
    fn scale_vertex_to_container(&self, rel_pos: Vertex) -> Vertex;

    fn scale_font_to_container(&self, font_size: usize) -> (u32, u32);

    fn scale_radius_to_window(&self, radius: u32, min_radius: u32) -> u32;

    /// Moves and scales the container to the given absolute rectangle.
    /// This should only be done on the root container to prevent layout issues.
    fn move_to(&mut self, abs_rect: RectData);
}

/// Allows the component (usually Containers) to manage their focused component
/// and provides an interface to change the focus.
pub trait FocusManager {
    fn get_focused_child(&self) -> Option<ComponentRef>;

    /// Returns the next focusable component or `None` if there are no more.
    fn focus_next_child(&mut self) -> Option<ComponentRef>;

    /// Returns the previous focusable component or `None` if there are no more.
    fn focus_prev_child(&mut self) -> Option<ComponentRef>;

    /// Returns the focusable component at the specified position or `None` if there is
    /// no component at the specified position.
    fn focus_child_at(&mut self, pos: Vertex) -> Option<ComponentRef>;
}
