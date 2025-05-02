use alloc::{format, vec::Vec};
use drawer::{drawer::Drawer, rect_data::RectData, vertex::Vertex};

use crate::{
    components::component::{Casts, Component, ComponentStyling},
    signal::ComponentRef,
    utils::{scale_pos_to_window, scale_rect_to_window},
};

use super::Container;

const CHILD_SPACING: u32 = 5;

#[derive(PartialEq)]
pub enum LayoutMode {
    None,
    Horizontal,
    Vertical,
    Grid(u32, u32),
}

#[derive(PartialEq)]
pub enum StretchMode {
    None,
    Fill,
}

pub struct BasicContainer {
    id: Option<usize>,
    childs: Vec<ComponentRef>,
    layout: LayoutMode,
    stretch: StretchMode,

    rel_rect_data: RectData,
    abs_rect_data: RectData,
    drawn_rect_data: RectData,

    /// Absolute offset from (0, 0)
    cursor: Vertex,
    
    is_dirty: bool,
    styling: ComponentStyling,
}

impl BasicContainer {
    pub fn new(
        rel_rect_data: RectData,
        abs_rect_data: RectData,
        layout: LayoutMode,
        stretch: StretchMode,
        styling: Option<ComponentStyling>,
    ) -> Self {
        Self {
            id: None,
            childs: Vec::new(),
            layout,
            stretch,

            rel_rect_data,
            abs_rect_data,
            drawn_rect_data: abs_rect_data.clone(),
            
            cursor: Vertex::zero(),

            is_dirty: true,
            styling: styling.unwrap_or_default(),
        }
    }

    fn apply_default_layout(&mut self) {
        for child in &self.childs {
            child
                .write()
                .rescale_to_container(self.as_container().unwrap());
        }
    }

    fn apply_horizontal_layout(&mut self) {
        for child in &self.childs {
            // Apply layout & scaling
            child
                .write()
                .rescale_to_container(self.as_container().unwrap());

            // Update the cursor position
            let abs_rect_data = child.read().get_abs_rect_data();
            self.cursor.x += abs_rect_data.width + CHILD_SPACING;
        }
    }

    fn apply_vertical_layout(&mut self) {
        for child in &self.childs {
            // Apply layout & scaling
            child
                .write()
                .rescale_to_container(self.as_container().unwrap());

            // Update the cursor position
            let abs_rect_data = child.read().get_abs_rect_data();
            self.cursor.y += abs_rect_data.height + CHILD_SPACING;
        }
    }

    fn apply_layout(&mut self) {
        self.cursor = Vertex::zero();

        match self.layout {
            LayoutMode::Horizontal => self.apply_horizontal_layout(),
            LayoutMode::Vertical => self.apply_vertical_layout(),
            LayoutMode::Grid(_, _) => todo!("needs rework for new scaling system"),

            _ => self.apply_default_layout(),
        }
    }
}

impl Container for BasicContainer {
    fn add_child(&mut self, child: ComponentRef) {
        //self.apply_stretch(&child);
        self.childs.push(child);

        self.apply_layout();
    }

    fn scale_to_container(
        &self,
        rel_rect: RectData,
        min_dim: (u32, u32),
        max_dim: (u32, u32),
        maintain_aspect_ratio: bool,
    ) -> RectData {
        // Adjust max dimensions based on stretching
        // TODO: Since the max dimension is always relative to the screen, do components really need
        // to calculate it themselves?
        let max_dim = match (&self.layout, &self.stretch) {
            (LayoutMode::Horizontal, StretchMode::Fill) => (max_dim.0, self.abs_rect_data.height),
            (LayoutMode::Vertical, StretchMode::Fill) => (self.abs_rect_data.width, max_dim.1),
            (_, _) => max_dim,
        };

        // Calculate the new abs rect from the rel rect
        let new_abs_rect = scale_rect_to_window(
            rel_rect,
            self.abs_rect_data,
            min_dim,
            max_dim,
            maintain_aspect_ratio,
        );

        // Adjust the position and size of the received abs rect
        // TODO: Use rel_rect as paddding, if stretching is active
        let layout_abs_rect = match &self.layout {
            LayoutMode::Horizontal => RectData {
                top_left: new_abs_rect.top_left + rel_rect.top_left + self.cursor, // Use original rel as offset
                height: match self.stretch {
                    StretchMode::Fill => self.abs_rect_data.height,
                    _ => new_abs_rect.height,
                },

                ..new_abs_rect
            },

            LayoutMode::Vertical => RectData {
                top_left: new_abs_rect.top_left + rel_rect.top_left + self.cursor, // Use original rel as offset
                width: match self.stretch {
                    StretchMode::Fill => self.abs_rect_data.width,
                    _ => new_abs_rect.width,
                },

                ..new_abs_rect
            },

            _ => RectData {
                top_left: new_abs_rect.top_left + self.cursor,
                ..new_abs_rect
            },
        };

        layout_abs_rect
    }

    fn scale_vertex_to_container(&self, rel_pos: Vertex) -> Vertex {
        let abs_pos = scale_pos_to_window(rel_pos, self.abs_rect_data);

        // Adjust the position
        let abs_pos = abs_pos + self.cursor;

        abs_pos
    }
}

impl Component for BasicContainer {
    fn draw(&mut self, focus_id: Option<usize>) {
        let dirty_components = self
            .childs
            .iter()
            .filter(|child| child.read().is_dirty())
            .collect::<Vec<_>>();

        // Bail out if there's nothing to do
        if dirty_components.is_empty() && !self.is_dirty {
            return;
        }

        if self.is_dirty {
            // Redraw the container, as it has been cleared by now
            Drawer::draw_rectangle(self.abs_rect_data, self.styling.border_color);

            self.drawn_rect_data = self.abs_rect_data.clone();
        } else {
            // Clear the area of dirty child components
            for child in &dirty_components {
                // We don't want to redraw entire containers
                // TODO: Make components responsible for clearing their own area?
                if child.read().as_container().is_some() {
                    continue;
                }

                let rect_data = child.read().get_drawn_rect_data();
                Drawer::partial_clear_screen(rect_data);
            }
        }

        // Draw dirty child components
        for child in dirty_components {
            child.write().draw(focus_id);
        }

        self.is_dirty = false;
    }

    fn rescale_after_split(&mut self, old_window_rect: RectData, new_window_rect: RectData) {
        self.abs_rect_data = scale_rect_to_window(
            self.rel_rect_data,
            new_window_rect,
            (10, 10),
            (1000, 1000),
            self.styling.maintain_aspect_ratio,
        );

        // Rescale all child components
        /*for child in &self.childs {
            child
                .write()
                .rescale_after_split(old_window_rect, new_window_rect);
        }*/

        self.apply_layout();
    }

    fn rescale_after_move(&mut self, new_window_rect: RectData) {
        self.abs_rect_data = scale_rect_to_window(
            self.rel_rect_data,
            new_window_rect,
            (10, 10),
            (1000, 1000),
            self.styling.maintain_aspect_ratio,
        );

        // Rescale all child components
        /*for child in &self.childs {
            child.write().rescale_after_move(new_window_rect);
        }*/

        self.apply_layout();
    }

    fn rescale_to_container(&mut self, parent: &dyn Container) {
        // TODO: max_dim should be parent abs, right?
        self.abs_rect_data = parent.scale_to_container(
            self.rel_rect_data,
            (0, 0),
            (1000, 1000),
            self.styling.maintain_aspect_ratio,
        );

        self.apply_layout();
    }

    fn get_abs_rect_data(&self) -> RectData {
        self.abs_rect_data
    }

    fn get_drawn_rect_data(&self) -> RectData {
        self.drawn_rect_data
    }

    /// Returns whether the container or any child is dirty
    fn is_dirty(&self) -> bool {
        self.is_dirty || self.childs.iter().any(|child| child.read().is_dirty())
    }

    fn set_id(&mut self, id: usize) {
        self.id = Some(id);
    }

    fn get_id(&self) -> Option<usize> {
        self.id
    }

    /// Marks the container and all child components as dirty
    fn mark_dirty(&mut self) {
        self.childs
            .iter()
            .for_each(|child| child.write().mark_dirty());

        self.is_dirty = true;
    }
}

impl Casts for BasicContainer {
    fn as_container(&self) -> Option<&dyn Container> {
        Some(self)
    }

    fn as_container_mut(&mut self) -> Option<&mut dyn Container> {
        Some(self)
    }
}
