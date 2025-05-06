// Julius Drodofsky

pub struct Canvas {
    pub id: Option<usize>,
    is_dirty: bool,
    abs_pos: Vertex,
    rel_pos: Vertex,
    drawn_rect_data: RectData,
    styling: ComponentStyling,
    buffer: Vec<u32>,
    widht: usize,
    height: usize,
    // default 4
    // bpp: u8,
} 

impl Canvas {
    pub fn new (
    abs_pos: Vertex,
    rel_pos: Vertex,
    styling: Option<ComponentStyling>,
    width: usize,
    height: usize,
    ) {
    let drawn_rect_data = RectData {
         top_left: abs_center.sub(abs_radius, abs_radius),
        width: width,
        height: height,
    };
    Self {
        id: None,
        is_dirt: false,
        abs_pos,
        rel_pos,
        drawn_rect_data,
        styling: styling.unwrap_or_default(),
        }
    }

}

impl Component for Canvas {
    fn draw(&mut self, is_focused: bool) {
        todo!()
    }
    fn is_dirty(&self) -> bool {
        self.is_dirty
    }

    fn mark_dirty(&mut self) {
        self.is_dirty = true;
    }

    fn get_id(&self) -> Option<usize> {
        self.id
    }

    fn set_id(&mut self, id: usize) {
        self.id = Some(id);
    }
    fn get_abs_rect_data(&self) -> RectData {
       self.drawn_rect_data 
    }

    fn get_drawn_rect_data(&self) -> RectData {
        self.drawn_rect_data
    }
}