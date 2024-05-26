use alloc::boxed::Box;
use drawer::drawer::RectData;

pub struct Button {
    pub comp_id: usize,
    pub parent_id: usize,
    pub pos: RectData,
    pub label: &'static str,
    pub on_click: Box<dyn FnMut() -> ()>,
}

impl Button {
    pub fn new(
        comp_id: usize,
        parent_id: usize,
        pos: RectData,
        label: &'static str,
        on_click: Box<dyn FnMut() -> ()>,
    ) -> Self {
        Self {
            comp_id,
            parent_id,
            pos, 
            label,
            on_click,
        }
    }

    
}