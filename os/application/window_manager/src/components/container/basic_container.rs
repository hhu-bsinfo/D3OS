use alloc::{format, vec::Vec};
use drawer::{drawer::Drawer, rect_data::RectData, vertex::Vertex};
use graphic::color::Color;

use crate::{
    components::component::{Casts, Component},
    signal::ComponentRef,
    utils::scale_rect_to_window,
};

use super::Container;

pub struct BasicContainer {
    id: Option<usize>,
    childs: Vec<ComponentRef>,

    rel_rect_data: RectData,
    abs_rect_data: RectData,
    drawn_rect_data: RectData,

    is_dirty: bool,
}

impl BasicContainer {
    pub fn new(rel_rect_data: RectData, abs_rect_data: RectData) -> Self {
        Self {
            id: None,
            childs: Vec::new(),

            rel_rect_data,
            abs_rect_data,
            drawn_rect_data: abs_rect_data.clone(),

            is_dirty: true,
        }
    }
}

impl Container for BasicContainer {
    fn add_child(&mut self, child: ComponentRef) {
        self.childs.push(child);
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
            Drawer::draw_rectangle(
                self.abs_rect_data,
                Color {
                    red: 255,
                    green: 0,
                    blue: 0,
                    alpha: 100,
                },
            );

            self.drawn_rect_data = self.abs_rect_data.clone();
        } else {
            // Clear the area of dirty child components
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

    fn rescale_after_split(&mut self, old_window_rect: RectData, new_window_rect: RectData) {
        self.abs_rect_data = scale_rect_to_window(
            self.rel_rect_data,
            new_window_rect,
            (10, 10),
            (1000, 1000),
            false,
            1.0,
        );

        // Rescale all child components
        for child in &self.childs {
            child
                .write()
                .rescale_after_split(old_window_rect, new_window_rect);
        }
    }

    fn rescale_after_move(&mut self, new_window_rect: RectData) {
        self.abs_rect_data = scale_rect_to_window(
            self.rel_rect_data,
            new_window_rect,
            (10, 10),
            (1000, 1000),
            false,
            1.0,
        );

        // Rescale all child components
        for child in &self.childs {
            child.write().rescale_after_move(new_window_rect);
        }
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
