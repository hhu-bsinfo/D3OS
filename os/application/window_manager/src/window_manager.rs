#![no_std]

extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;
use alloc::boxed::Box;
use drawer::drawer::{Drawer, Vertex};
#[allow(unused_imports)]
use runtime::*;
use io::{print, println};

struct WindowManager {
    workspaces: Vec<Workspace>,
}

struct RectPos {
    top_left: Vertex,
    width: u32,
    height: u32,
}

struct Workspace {
    container_tree: Box<dyn Container>,
}

struct Window {
    pos: RectPos,
    children: Vec<Box<dyn Container>>,
}

enum SplitDirection {
    Horizontal,
    Vertical,
}

struct Splitter {
    pos: RectPos,
    split_direction: SplitDirection,
    children: Vec<Box<dyn Container>>
}

impl RectPos {
    fn bottom_right(&self) -> Vertex {
        Vertex::new(self.top_left.x + self.width, self.top_left.y + self.height)
    }
}

impl WindowManager {
    fn new(root_width: u32, root_height: u32) -> WindowManager {
        let first_workspace = Workspace::new(root_width, root_height);
        Self {
            workspaces: vec![first_workspace],
        }
    }

    fn run(&mut self) {
        Drawer::create_context();
        self.workspaces[0].container_tree.layout_content();
        self.workspaces[0].container_tree.draw();
    }
}

impl Workspace {
    fn new(root_width: u32, root_height: u32) -> Workspace {
        let window = Window::new(RectPos {
            top_left: Vertex::new(5, 5),
            width: root_width - 6,
            height: root_height - 6,
        });

        Self {
            container_tree: Box::new(window),
        }
    }
}

impl Splitter {
    fn new(pos: RectPos, children: Vec<Box<dyn Container>>, split_direction: SplitDirection) -> Splitter {
        Self {
            pos,
            split_direction,
            children,
        }
    }

    fn toggle_split_direction(&mut self) {
        self.split_direction = match self.split_direction {
            SplitDirection::Horizontal => SplitDirection::Vertical,
            SplitDirection::Vertical => SplitDirection::Horizontal,
        };
    }
}

impl Window {
    fn new(pos: RectPos) -> Window {
        Self {
            pos,
            children: Vec::new(),
        }
    }
}

trait Container {
    fn position(&self) -> Vertex;
    fn width(&self) -> u32;
    fn height(&self) -> u32;
    fn set_position(&mut self, new_pos: Vertex);
    fn set_width(&mut self, new_width: u32);
    fn set_height(&mut self, new_height: u32);
    fn children(&mut self) -> &mut Vec<Box<dyn Container>>;
    fn layout_content(&mut self);
    fn draw(&self);
}

impl Container for Splitter {
    fn position(&self) -> Vertex {
        self.pos.top_left
    }

    fn width(&self) -> u32 {
        self.pos.width
    }

    fn height(&self) -> u32 {
        self.pos.height
    }

    fn children(&mut self) -> &mut Vec<Box<dyn Container>> {
        &mut self.children
    }

    fn layout_content(&mut self) {
        let mut top_left = self.pos.top_left;
        /* TODO: For proper drawing, you might need to differentiate between first/last and in-between
           content blocks. There you might need to add one-diffs only for in-between elements */
        match self.split_direction {
            SplitDirection::Horizontal => {
                let child_height = self.pos.height / (self.children.len() as u32);
                
                for child in self.children.iter_mut() {
                    child.set_position(top_left.add_one());
                    child.set_width(self.pos.width - 1);
                    child.set_height(child_height - 1);

                    top_left.y += child_height;
                }
            },
            SplitDirection::Vertical => {
                let child_width = self.pos.width / (self.children.len() as u32);
                
                for child in self.children.iter_mut() {
                    child.set_position(top_left.add_one());
                    child.set_height(self.pos.height - 1);
                    child.set_width(child_width - 1);

                    top_left.x += child_width;
                }
            },
        }

        self.children.iter_mut().for_each(|child| child.layout_content());
    }

    fn draw(&self) {
        self.children.iter().for_each(|child| child.draw());
    }

    fn set_position(&mut self, new_pos: Vertex) {
        self.pos.top_left = new_pos;
    }
    
    fn set_width(&mut self, new_width: u32) {
        self.pos.width = new_width;
    }
    
    fn set_height(&mut self, new_height: u32) {
        self.pos.height = new_height;
    }
}

impl Container for Window {
    fn layout_content(&mut self) {
        let pos = &self.pos;
        let mut top_left = pos.top_left;
        let mut bottom_right = Vertex::new(top_left.x + pos.width, top_left.y + pos.height);
        // TODO: Add content
    }
    
    fn position(&self) -> Vertex {
        self.pos.top_left
    }

    fn width(&self) -> u32 {
        self.pos.width
    }
    
    fn height(&self) -> u32 {
        self.pos.height
    }
        
    fn children(&mut self) -> &mut Vec<Box<dyn Container>> {
        &mut self.children
    }
    
    fn draw(&self) {
        let bottom_right = self.pos.bottom_right();
        Drawer::draw_rectangle(self.pos.top_left, bottom_right);
    }
    
    fn set_position(&mut self, new_pos: Vertex) {
        self.pos.top_left = new_pos;
    }
    
    fn set_width(&mut self, new_width: u32) {
        self.pos.width = new_width;
    }
    
    fn set_height(&mut self, new_height: u32) {
        self.pos.height = new_height;
    }
}

#[no_mangle]
pub fn main() {
    let (width, height) = Drawer::get_graphic_resolution();
    let mut wm = WindowManager::new(width, height);
    wm.run();
}