#![no_std]

mod config;

extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;
use alloc::boxed::Box;
use drawer::drawer::{Drawer, Vertex};
#[allow(unused_imports)]
use runtime::*;
use io::{print, println, read::read, Application};
use graphic::color::{self, WHITE, YELLOW};
use config as wm_config;

struct WindowManager {
    workspaces: Vec<Workspace>,
}

#[derive(Clone, Copy)]
struct RectPos {
    top_left: Vertex,
    width: u32,
    height: u32,
    /* These inner variables are for convenience and describe the width/height of 
    the rectangle minus the thickness of the rectangle-lines, normally 1px */
    inner_width: u32,
    inner_height: u32,
}

struct Workspace {
    container_tree: Box<dyn Container>,
    focused: bool,
}

struct Window {
    pos: RectPos,
    children: Vec<Box<dyn Container>>,
    focused: bool,
}

enum SplitDirection {
    Horizontal,
    Vertical,
}

struct Splitter {
    pos: RectPos,
    split_direction: SplitDirection,
    children: Vec<Box<dyn Container>>,
    focused: bool,
}

impl RectPos {
    fn new(top_left: Vertex, width: u32, height: u32) -> RectPos {
        RectPos { top_left, width, height, inner_width: width - 1, inner_height: height - 1 }
    }

    fn bottom_right(&self) -> Vertex {
        Vertex::new(self.top_left.x + self.width, self.top_left.y + self.height)
    }
}

impl WindowManager {
    fn new(root_width: u32, root_height: u32) -> WindowManager {
        let first_workspace = Workspace::new(root_width, root_height, true);
        Self {
            workspaces: vec![first_workspace],
        }
    }

    fn run(&mut self) {
        Drawer::create_context();
        self.workspaces[0].container_tree.layout_content();
        self.workspaces[0].container_tree.draw();

        loop {
            let keyboard_press = read(Application::WindowManager);

            match keyboard_press {
                ' ' => {
                },
                c => {},
            }
        }
    }
}

impl Workspace {
    fn new(root_width: u32, root_height: u32, focused: bool) -> Workspace {
        let dist = wm_config::DIST_SCREEN_WORKSPACE;
        let window = Window::new(
            RectPos::new(
                Vertex::new(dist, dist),
                root_width - 2*dist,
                root_height - 2*dist,
            ),
            true,
        );

        Self {
            container_tree: Box::new(window),
            focused,
        }
    }
}

impl Splitter {
    fn new(pos: RectPos, children: Vec<Box<dyn Container>>, split_direction: SplitDirection) -> Splitter {
        Self {
            pos,
            split_direction,
            children,
            focused: false,
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
    fn new(pos: RectPos, focused: bool) -> Window {
        Self {
            pos,
            children: Vec::new(),
            focused,
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
                let child_height = self.pos.inner_height / (self.children.len() as u32);
                
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
        let color = if self.is_focused() { YELLOW } else { WHITE };
        let bottom_right = self.pos.bottom_right();
        Drawer::draw_rectangle(self.pos.top_left, bottom_right, color);
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

trait Focusable {
    fn is_focused(&self) -> bool;
    fn toggle_focus(&mut self);
}

impl Focusable for Workspace {
    fn is_focused(&self) -> bool {
        self.focused
    }

    fn toggle_focus(&mut self) {
        self.focused = !self.focused;
    }
}

impl Focusable for Window {
    fn is_focused(&self) -> bool {
        self.focused
    }

    fn toggle_focus(&mut self) {
        self.focused = !self.focused;
    }
}

impl Focusable for Splitter {
    fn is_focused(&self) -> bool {
        self.focused
    }

    fn toggle_focus(&mut self) {
        self.focused = !self.focused;
    }
}

#[no_mangle]
pub fn main() {
    let (width, height) = Drawer::get_graphic_resolution();
    let mut wm = WindowManager::new(width, height);
    wm.run();
}