use alloc::{format, vec::Vec};
use drawer::{drawer::Drawer, rect_data::RectData, vertex::Vertex};

use crate::{
    components::component::{Casts, Component, ComponentStyling},
    signal::ComponentRef,
    utils::scale_rect_to_window,
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

    is_dirty: bool,
    cursor: Vertex,
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

            is_dirty: true,
            cursor: Vertex::zero(),
            styling: styling.unwrap_or_default(),
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

    fn apply_grid_layout(&mut self, rows: u32, cols: u32) {
        // TODO: Adjust for the new scaling function

        let mut cursor = Vertex { x: 0, y: 0 };
        let cell_width = self.abs_rect_data.width / cols;
        let cell_height = self.abs_rect_data.height / rows;

        for child in &self.childs {
            // Update layout
            child.write().rescale_after_move(RectData {
                top_left: self.abs_rect_data.top_left
                    + Vertex {
                        x: cursor.x * cell_width,
                        y: cursor.y * cell_height,
                    },
                width: self.abs_rect_data.width,
                height: self.abs_rect_data.height,
            });

            // Update the cursor
            cursor.x = (cursor.x + 1) % cols;
            if cursor.x == 0 {
                cursor.y += 1;
            }
        }
    }

    fn apply_stretch(&self, child: &ComponentRef) {
        if self.stretch == StretchMode::Fill {
            let abs_rect_data = child.read().get_abs_rect_data();
            if let Some(resizable) = child.write().as_resizable_mut() {
                match self.layout {
                    LayoutMode::Horizontal => {
                        resizable.resize(abs_rect_data.width, self.abs_rect_data.height);
                    }

                    LayoutMode::Vertical => {
                        resizable.resize(self.abs_rect_data.width, abs_rect_data.height);
                    }

                    _ => {}
                }
            }
        }
    }

    fn apply_layout(&mut self) {
        self.cursor = self.abs_rect_data.top_left;

        match self.layout {
            LayoutMode::Horizontal => self.apply_horizontal_layout(),
            LayoutMode::Vertical => self.apply_vertical_layout(),
            LayoutMode::Grid(rows, cols) => self.apply_grid_layout(rows, cols),

            _ => {}
        }
    }
}

impl Container for BasicContainer {
    fn add_child(&mut self, child: ComponentRef) {
        //self.apply_stretch(&child);
        self.childs.push(child);

        self.apply_layout();
    }

    fn scale_to_container(&self, rel_rect: RectData) -> RectData {
        let new_abs_rect = scale_rect_to_window(
            rel_rect,
            self.abs_rect_data,
            (0, 0),
            (1000, 1000),
            false,
            1.0,
        );

        let layout_abs_rect = match &self.layout {
            LayoutMode::Horizontal => RectData {
                top_left: self.cursor + rel_rect.top_left, // Orgininal rel = Offset
                height: match self.stretch {
                    StretchMode::Fill => self.abs_rect_data.height,
                    _ => new_abs_rect.height,
                },

                ..new_abs_rect
            },

            LayoutMode::Vertical => RectData {
                top_left: self.cursor + rel_rect.top_left, // Orgininal rel = Offset
                width: match self.stretch {
                    StretchMode::Fill => self.abs_rect_data.width,
                    _ => new_abs_rect.width,
                },

                ..new_abs_rect
            },

            _ => new_abs_rect,
        };

        layout_abs_rect
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
        let aspect_ratio = self.rel_rect_data.width as f64 / self.rel_rect_data.height as f64;

        self.abs_rect_data = scale_rect_to_window(
            self.rel_rect_data,
            new_window_rect,
            (10, 10),
            (1000, 1000),
            self.styling.maintain_aspect_ratio,
            aspect_ratio,
        );

        // Rescale all child components
        for child in &self.childs {
            child
                .write()
                .rescale_after_split(old_window_rect, new_window_rect);
        }

        self.apply_layout();
    }

    fn rescale_after_move(&mut self, new_window_rect: RectData) {
        let aspect_ratio = self.rel_rect_data.width as f64 / self.rel_rect_data.height as f64;

        self.abs_rect_data = scale_rect_to_window(
            self.rel_rect_data,
            new_window_rect,
            (10, 10),
            (1000, 1000),
            self.styling.maintain_aspect_ratio,
            aspect_ratio,
        );

        // Rescale all child components
        for child in &self.childs {
            child.write().rescale_after_move(new_window_rect);
        }

        self.apply_layout();
    }

    fn rescale_to_container(&mut self, parent: &dyn Container) {
        self.abs_rect_data = parent.scale_to_container(self.rel_rect_data);

        //self.apply_layout();
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
