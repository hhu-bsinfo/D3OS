use alloc::{format, vec::Vec};
use drawer::{drawer::Drawer, rect_data::RectData, vertex::Vertex};

use crate::{
    components::component::{Casts, Component, ComponentStyling},
    signal::ComponentRef,
    utils::{scale_pos_to_rect, scale_rect_to_rect},
    WindowManager,
};

use super::{Container, ContainerStyling, FocusManager};

#[derive(Copy, Clone, PartialEq)]
pub enum AlignmentMode {
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(PartialEq)]
pub enum LayoutMode {
    None,
    Horizontal(AlignmentMode),
    Vertical(AlignmentMode),
    Grid(u32),
}

#[derive(PartialEq)]
pub enum StretchMode {
    None,
    Fill,
}

pub struct BasicContainer {
    id: usize,
    childs: Vec<ComponentRef>,
    layout: LayoutMode,
    stretch: StretchMode,

    rel_rect_data: RectData,
    abs_rect_data: RectData,
    drawn_rect_data: RectData,

    /// Absolute offset from (0, 0)
    cursor: Vertex,
    focused_child_idx: Option<usize>,

    is_dirty: bool,
    styling: ContainerStyling,
}

impl BasicContainer {
    pub fn new(
        rel_rect_data: RectData,
        layout: LayoutMode,
        stretch: StretchMode,
        styling: Option<ContainerStyling>,
    ) -> Self {
        Self {
            id: WindowManager::generate_id(),
            childs: Vec::new(),
            layout,
            stretch,

            rel_rect_data,
            abs_rect_data: RectData::zero(),
            drawn_rect_data: RectData::zero(),

            cursor: Vertex::zero(),
            focused_child_idx: None,

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
            self.cursor.x += abs_rect_data.width + self.styling.child_padding;
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
            self.cursor.y += abs_rect_data.height + self.styling.child_padding;
        }
    }

    fn apply_grid_layout(&mut self, cols: u32) {
        let mut last_row_height: u32 = 0;

        for (i, child) in self.childs.iter().enumerate() {
            // Apply layout & scaling
            child
                .write()
                .rescale_to_container(self.as_container().unwrap());

            // Update the cursor position
            let abs_rect_data = child.read().get_abs_rect_data();
            if (i + 1) % cols as usize == 0 {
                self.cursor.x = 0;
                self.cursor.y += last_row_height + self.styling.child_padding;
                last_row_height = 0;
            } else {
                self.cursor.x += abs_rect_data.width + self.styling.child_padding;
                last_row_height = last_row_height.max(abs_rect_data.height);
            }
        }
    }

    fn apply_layout(&mut self) {
        self.cursor = Vertex::zero();

        match self.layout {
            LayoutMode::Horizontal(_) => self.apply_horizontal_layout(),
            LayoutMode::Vertical(_) => self.apply_vertical_layout(),
            LayoutMode::Grid(cols) => self.apply_grid_layout(cols),

            _ => self.apply_default_layout(),
        }
    }

    fn get_content_area(&self) -> RectData {
        if self.styling.show_border {
            RectData {
                top_left: self.abs_rect_data.top_left.add(1, 1),
                width: self.abs_rect_data.width.saturating_sub(1),
                height: self.abs_rect_data.height.saturating_sub(1),
            }
        } else {
            self.abs_rect_data
        }
    }
}

impl Container for BasicContainer {
    fn add_child(&mut self, child: ComponentRef) {
        self.childs.push(child);
        self.mark_dirty();
    }

    fn remove_child(&mut self, id: usize) {
        if let Some(pos) = self.childs.iter().position(|c| c.read().get_id() == id) {
            self.childs.remove(pos);
            self.mark_dirty();
        }
    }

    fn scale_to_container(
        &self,
        rel_rect: RectData,
        min_dim: (u32, u32),
        max_dim: (u32, u32),
        maintain_aspect_ratio: bool,
    ) -> RectData {
        let content_area = self.get_content_area();

        // Adjust max dimensions based on stretching
        // TODO: Since the max dimension is always relative to the screen, do components really need
        // to calculate it themselves?
        let max_dim = match (&self.layout, &self.stretch) {
            (LayoutMode::Horizontal(_), StretchMode::Fill) => {
                (max_dim.0, u32::max(content_area.height, min_dim.1))
            }
            (LayoutMode::Vertical(_), StretchMode::Fill) => {
                (u32::max(content_area.width, min_dim.0), max_dim.1)
            }
            (LayoutMode::None, StretchMode::Fill) => (
                u32::max(content_area.width, min_dim.0),
                u32::max(content_area.height, min_dim.1),
            ),
            (_, _) => max_dim.max(min_dim),
        };

        // Calculate the new abs rect from the rel rect
        let new_abs_rect = scale_rect_to_rect(
            rel_rect,
            content_area,
            min_dim,
            max_dim,
            maintain_aspect_ratio,
        );

        // Adjust the position based on the layout/alignment
        let new_abs_pos = match &self.layout {
            LayoutMode::Horizontal(AlignmentMode::Left)
            | LayoutMode::Vertical(AlignmentMode::Top) => new_abs_rect.top_left + self.cursor,

            LayoutMode::Horizontal(AlignmentMode::Right) => {
                new_abs_rect.top_left.add(content_area.width, 0)
                    - self.cursor.add(new_abs_rect.width, 0)
            }

            LayoutMode::Vertical(AlignmentMode::Bottom) => {
                new_abs_rect.top_left.add(0, content_area.height)
                    - self.cursor.add(0, new_abs_rect.height)
            }

            _ => new_abs_rect.top_left + self.cursor,
        };

        // Adjust the size of the received abs rect
        // TODO: Use rel_rect as paddding, if stretching is active
        let layout_abs_rect = match &self.layout {
            LayoutMode::Horizontal(_) => RectData {
                top_left: new_abs_pos, // Use original rel as offset
                height: match self.stretch {
                    StretchMode::Fill => content_area.height,
                    _ => new_abs_rect.height,
                },

                ..new_abs_rect
            },

            LayoutMode::Vertical(_) => RectData {
                top_left: new_abs_pos, // Use original rel as offset
                width: match self.stretch {
                    StretchMode::Fill => content_area.width,
                    _ => new_abs_rect.width,
                },

                ..new_abs_rect
            },

            _ => match self.stretch {
                StretchMode::Fill => RectData {
                    top_left: new_abs_pos,
                    width: content_area.width,
                    height: content_area.height,
                },

                _ => RectData {
                    top_left: new_abs_pos,
                    ..new_abs_rect
                },
            },
        };

        layout_abs_rect
    }

    fn scale_vertex_to_container(&self, rel_pos: Vertex) -> Vertex {
        let abs_pos = scale_pos_to_rect(rel_pos, self.get_content_area());

        // Adjust the position
        let abs_pos = abs_pos + self.cursor;

        abs_pos
    }

    fn move_to(&mut self, abs_rect: RectData) {
        self.abs_rect_data = scale_rect_to_rect(
            self.rel_rect_data,
            abs_rect,
            (10, 10),
            (1000, 1000),
            self.styling.maintain_aspect_ratio,
        );

        self.mark_dirty();
    }

    fn scale_font_to_container(&self, _font_size: usize) -> (u32, u32) {
        (1, 1)
    }
}

impl Component for BasicContainer {
    fn draw(&mut self, focus_id: Option<usize>) {
        // Apply the layout & styling BEFORE redrawing components
        if self.is_dirty {
            if self.styling.show_background {
                Drawer::draw_filled_rectangle(
                    self.abs_rect_data,
                    self.styling.background_color,
                    self.styling
                        .show_border
                        .then_some(self.styling.border_color),
                );
            } else if self.styling.show_border {
                Drawer::draw_rectangle(self.abs_rect_data, self.styling.border_color);
            }

            self.drawn_rect_data = self.abs_rect_data.clone();
            self.apply_layout();
        }

        // Retrieve all dirty components
        let dirty_components = self
            .childs
            .iter()
            .filter(|child| child.read().is_dirty())
            .collect::<Vec<_>>();

        // Is there anything to do?
        if dirty_components.is_empty() {
            self.is_dirty = false;
            return;
        }

        // Clear the area of dirty child components
        if !self.is_dirty {
            for child in &dirty_components {
                // We don't want to redraw entire containers
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

    fn rescale_to_container(&mut self, parent: &dyn Container) {
        let parent_abs_rect = parent.get_abs_rect_data();

        self.abs_rect_data = parent.scale_to_container(
            self.rel_rect_data,
            (5, 5),
            (parent_abs_rect.width, parent_abs_rect.height),
            self.styling.maintain_aspect_ratio,
        );

        self.mark_dirty();
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

    fn get_id(&self) -> usize {
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

impl FocusManager for BasicContainer {
    fn get_focused_child(&self) -> Option<ComponentRef> {
        match self.focused_child_idx {
            Some(idx) => self.childs.get(idx).cloned(),
            None => None,
        }
    }

    fn focus_next_child(&mut self) -> Option<ComponentRef> {
        if let Some(current_focus) = self.get_focused_child() {
            let mut current_focus_lock = current_focus.write();

            // Ask the previous container for its next child
            if let Some(container) = current_focus_lock.as_container_mut() {
                if let Some(next_child) = container.focus_next_child() {
                    return Some(next_child);
                }
            }
        }

        // Get the next or first child index
        let start_idx = self
            .focused_child_idx
            .map_or(Some(0), |idx| idx.checked_add(1));

        // Find the next focusable child
        if let Some(start_idx) = start_idx {
            for i in start_idx..self.childs.len() {
                let child = &self.childs[i];
                let mut child_mut = child.write();

                // Ask the next container for its child
                if let Some(container) = child_mut.as_container_mut() {
                    if let Some(next_child) = container.focus_next_child() {
                        self.focused_child_idx = Some(i);
                        return Some(next_child);
                    }
                }

                // Try to focus the component directly
                if child_mut.as_focusable().is_some() {
                    self.focused_child_idx = Some(i);
                    return Some(child.clone());
                }
            }
        }

        // No focusable child found
        self.focused_child_idx = None;
        None
    }

    fn focus_prev_child(&mut self) -> Option<ComponentRef> {
        if let Some(current_focus) = self.get_focused_child() {
            let mut current_focus_lock = current_focus.write();

            // Ask the previous container for its next child
            if let Some(container) = current_focus_lock.as_container_mut() {
                if let Some(next_child) = container.focus_prev_child() {
                    return Some(next_child);
                }
            }
        }

        // Get the next or first child index
        let start_idx = self
            .focused_child_idx
            .map_or(self.childs.len().checked_sub(1), |idx| idx.checked_sub(1));

        // Find the next focusable child
        if let Some(start_idx) = start_idx {
            for i in (0..=start_idx).rev() {
                let child = &self.childs[i];
                let mut child_mut = child.write();

                // Ask the next container for its child
                if let Some(container) = child_mut.as_container_mut() {
                    if let Some(next_child) = container.focus_prev_child() {
                        self.focused_child_idx = Some(i);
                        return Some(next_child);
                    }
                }

                // Otherwise try to focus directly
                if child_mut.as_focusable().is_some() {
                    self.focused_child_idx = Some(i);
                    return Some(child.clone());
                }
            }
        }

        // No focusable child found
        self.focused_child_idx = None;
        None
    }

    fn focus_child_at(&mut self, pos: Vertex) -> Option<ComponentRef> {
        for child in self.childs.iter() {
            let mut child_lock = child.write();
            if child_lock.get_abs_rect_data().contains_vertex(&pos) {
                // Ask the container
                if let Some(container) = child_lock.as_container_mut() {
                    return container.focus_child_at(pos);
                }

                // Return child at position
                return Some(child.clone());
            }
        }

        None
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
