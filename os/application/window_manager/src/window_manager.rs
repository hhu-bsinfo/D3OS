#![no_std]

extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;
use alloc::boxed::Box;
use drawer::drawer::{Drawer, Vertex};
#[allow(unused_imports)]
use runtime::*;

struct WindowManager {
    workspaces: Vec<Workspace>,
    root_width: u32,
    root_height: u32,
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
    is_visible: bool,
    is_focused: bool,
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
    fn bottom_right(&self) -> Vertex{
        Vertex::new(self.top_left.x + self.width, self.top_left.y + self.height)
    }
}

impl WindowManager {
    fn new(root_width: u32, root_height: u32) -> WindowManager {
        let first_workspace = Workspace::new();
        Self {
            workspaces: vec![first_workspace],
            root_width,
            root_height,
        }
    }

    fn run(&mut self) {
        Drawer::create_context();
        self.workspaces[0].container_tree.layout_content();
        self.workspaces[0].container_tree.draw();
    }
}

impl Workspace {
    fn new() -> Workspace {
        let splitter = Splitter::new();
        Self {
            container_tree: Box::new(splitter)
        }
    }
}

impl Splitter {
    fn new() -> Splitter {
        let children: Vec<Box<dyn Container>> = vec![
            Box::new(Window::new()),
            Box::new(Window::new()),
        ];
        Self {
            pos: RectPos {
                top_left: Vertex::new(0, 0),
                width: 300,
                height: 300,
            },
            split_direction: SplitDirection::Vertical,
            children,
        }
    }
}

impl Window {
    fn new() -> Window {
        Self {
            pos: RectPos { top_left: Vertex::new(0, 0), width: 500, height: 400 },
            is_visible: true,
            is_focused: true,
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
        match self.split_direction {
            SplitDirection::Horizontal => {
                let child_height = self.pos.height / (self.children.len() as u32);
                
                for child in self.children.iter_mut() {
                    child.set_position(top_left);
                    child.set_width(self.pos.width);
                    child.set_height(child_height);

                    top_left.y += child_height;
                }
            },
            SplitDirection::Vertical => {
                let child_width = self.pos.width / (self.children.len() as u32);
                
                for child in self.children.iter_mut() {
                    child.set_position(top_left);
                    child.set_height(self.pos.height);
                    child.set_width(child_width);

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
        let mut _top_left: Vertex = pos.top_left;
        let mut _bottom_right: Vertex = Vertex::new(_top_left.x + pos.width, _top_left.y + pos.height);
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