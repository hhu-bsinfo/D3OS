// Julius Drodofsky

pub struct Canvas {
    pub id: Option<usize>,
    pub is_dirty: bool,
    pub abs_pos: Vertex,
    pub rel_pos: Vertex,
    pub styling: ComponentStyling,
    buffer: Vec<u32>,
    widht: usize,
    height: usize,
    ppb: u8,
} 

impl Canvas {
    pub fn new (
    abs_pos: Vertex,
    rel_pos: Vertex,
    styling: Option<ComponentStyling>
    ) {
        todo!()
    }

}