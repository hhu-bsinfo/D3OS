use drawer::{rect_data::RectData, vertex::Vertex};

use crate::signal::ComponentRef;

use super::component::Component;

pub mod basic_container;

pub trait Container: Component {
    fn add_child(&mut self, child: ComponentRef);

    /// Scales a relative rect to the container and returns the absolute rect
    fn scale_to_container(
        &self,
        rel_rect: RectData,
        min_dim: (u32, u32),
        max_dim: (u32, u32),
        aspect_ratio: Option<f64>,
    ) -> RectData;

    /// Scales a relative vertex to the container and returns the absolute vertex
    fn scale_vertex_to_container(&self, rel_pos: Vertex) -> Vertex;
}
