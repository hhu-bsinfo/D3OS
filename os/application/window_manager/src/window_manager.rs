#![no_std]

mod config;

extern crate alloc;

use alloc::{boxed::Box, rc::Rc};
use alloc::vec;
use alloc::vec::Vec;
use config as wm_config;
use drawer::drawer::{Drawer, Vertex};
use graphic::color::{WHITE, YELLOW};
use io::{print, println, read::read, Application};
use oorandom::Rand64;
#[allow(unused_imports)]
use runtime::*;

const SEED: u128 = 956782903219087648534056987234;

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
    id: u64,
    pos: RectPos,
    parent: Box<*mut dyn Container>,
    children: Vec<Box<dyn Container>>,
    focused: bool,
}

enum SplitDirection {
    Horizontal,
    Vertical,
}

struct Splitter {
    id: u64,
    pos: RectPos,
    split_direction: SplitDirection,
    parent: Option<*mut dyn Container>,
    children: Vec<Box<dyn Container>>,
    focused: bool,
}

impl RectPos {
    fn new(top_left: Vertex, width: u32, height: u32) -> RectPos {
        RectPos {
            top_left,
            width,
            height,
            inner_width: width - 1,
            inner_height: height - 1,
        }
    }

    /// Often times we set the properties via [`layout_content`], this is for convenience
    fn new_empty() -> RectPos {
        RectPos {
            top_left: Vertex { x: 0, y: 0 },
            width: 1,
            height: 1,
            inner_width: 0,
            inner_height: 0,
        }
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

    fn redraw(&mut self) {
        Drawer::clear_screen();
        let borrowed_root = &mut self.workspaces[0].container_tree.borrow_mut();
        borrowed_root.layout_content();
        borrowed_root.draw();
    }

    fn run(&mut self) {
        loop {
            self.redraw();
            let keyboard_press = read(Application::WindowManager);

            match keyboard_press {
                ' ' => {
                    let mut curr_focused = self.find_focused().borrow_mut();
                
                    let focused_pos = RectPos::new(
                        curr_focused.position(), 
                        curr_focused.width(), 
                        curr_focused.height()
                    );

                    if curr_focused.parent().is_none() {
                        curr_focused = curr_focused.children()[0].borrow_mut();
                    }

                    let parent = unsafe { curr_focused.parent().unwrap().as_mut().unwrap() };
                    // 1. Find index of window as child
                    let index_children_vec = parent.children()
                        .iter()
                        .position(|child| child.id() == curr_focused.id())
                        .unwrap();
                
                    // 2. Create splitter and insert old window
                    let splitter = Splitter::new(
                        focused_pos, 
                        Some(parent),
                        Vec::new(),
                        SplitDirection::Horizontal,
                    );
                    let splitter_id = splitter.id;

                    parent.children().push(Box::new(splitter));

                    // 3. Swap out window with splitter
                    let old_window = parent.children().swap_remove(index_children_vec);

                    parent.children()
                        .iter_mut()
                        .find(|child| child.id() == splitter_id)
                        .unwrap()
                        .children()
                        .push(old_window);

                    parent.children()
                        .iter_mut()
                        .find(|child| child.id() == splitter_id)
                        .unwrap()
                        .children()
                        .push(
                            Box::new(Window::new(
                                RectPos::new_empty(),
                                curr_focused.parent(),
                                false,
                            )),
                        );

                        // None => {
                            
                        //     // Just swap out root-element with new splitter, assigning the old-root's
                        //     // children to the new splitter
                        //     let old_root = self.workspaces.iter_mut()
                        //         .find(|workspace| workspace.is_focused())
                        //         .unwrap()
                        //         .container_tree;

                        //     let splitter = Splitter::new(
                        //         focused_pos, 
                        //         None,
                        //         vec![
                        //             Box::new(Window::new(
                        //                 RectPos::new_empty(),
                        //                 None,
                        //                 false,
                        //             )),
                        //         ],
                        //         SplitDirection::Horizontal,
                        //     );

                        //     let boxed_splitter: Box<dyn Container> = Box::new(splitter);

                        //     // splitter.children.push(Box::new())

                        //     self.workspaces.iter_mut()
                        //         .find(|workspace| workspace.is_focused())
                        //         .unwrap()
                        //         .container_tree = boxed_splitter;

                        //     let qwe: &mut Box<dyn Container> = self.workspaces.iter_mut()
                        //         .find(|workspace| workspace.is_focused())
                        //         .unwrap()
                        //         .container_tree
                        //         .borrow_mut();
                        //     qwe.children().push(unsafe {
                        //         *(old_root.as_mut().unwrap())
                        //     })

                        // },
                }
                'a' => {
                    let curr_focused = self.find_focused();
                    curr_focused.toggle_focus();
                    let parent = unsafe { curr_focused.parent().unwrap().as_mut().unwrap() };
                    let siblings = parent.children();

                    if siblings[0].id() != curr_focused.id() && siblings.len() > 1 {
                        let mut curr_ptr = siblings[1..].as_mut_ptr();
                        let end = unsafe { curr_ptr.add(siblings.len()) };

                        while curr_ptr < end {
                            unsafe {
                                if (*curr_ptr).id() == curr_focused.id() {
                                    curr_ptr = curr_ptr.sub(1);
                                    (*curr_ptr).toggle_focus();
                                    break;
                                }
                                curr_ptr = curr_ptr.add(1);
                            }
                        }
                    }
                }
                'd' => {
                    let curr_focused = self.find_focused();
                    curr_focused.toggle_focus();
                    let parent = unsafe { curr_focused.parent().unwrap().as_mut().unwrap() };
                    let siblings = parent.children();
                    let last = siblings.last().unwrap();

                    if last.id() != curr_focused.id() && siblings.len() > 1 {
                        let len = siblings.len();
                        let mut curr_ptr = siblings[0..len - 1].as_mut_ptr();
                        let end = unsafe { curr_ptr.add(siblings.len()) };

                        while curr_ptr < end {
                            unsafe {
                                if (*curr_ptr).id() == curr_focused.id() {
                                    curr_ptr = curr_ptr.add(1);
                                    (*curr_ptr).toggle_focus();
                                    break;
                                }
                                curr_ptr = curr_ptr.add(1);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    // TODO: Find a way to keep going into the focused-tree until you hit a leaf
    fn find_focused(&mut self) -> &mut Box<dyn Container> {
        let mut top_level_con: &mut Box<dyn Container> = self.workspaces
            .iter_mut()
            .find(|workspace| workspace.focused)
            .expect("At least one workspace should be focused")
            .container_tree
            .borrow_mut();

        
        let mut prev = core::ptr::addr_of_mut!(top_level_con);
        let mut curr = top_level_con.children()
        .iter_mut()
        .find(|container| container.is_focused());
    
        loop {
            println!("10");
            if curr.is_none() {
                println!("11");
                // This is safe, since we only update prev until now, never reading it. 
                // On return, all other refs are dropped anyway
                return unsafe { *prev };
            }
            println!("42");
            let mut unwrapped_curr = curr.unwrap();
            println!("43");
            if unwrapped_curr.children().is_empty() {
                println!("44");
                return unwrapped_curr;
            }
            println!("45");
            prev = core::ptr::addr_of_mut!(unwrapped_curr);
            println!("46");
            curr = unwrapped_curr.children()
            .iter_mut()
            .find(|container| container.is_focused());
            println!("47");
        }
    }
}

impl Workspace {
    fn new(root_width: u32, root_height: u32, focused: bool) -> Workspace {
        let dist = wm_config::DIST_SCREEN_WORKSPACE;
        let mut splitter = Splitter::new(
            RectPos::new(
                Vertex::new(dist, dist),
                root_width - 2 * dist,
                root_height - 2 * dist,
            ),
            None,
            vec![
                Box::new(Window::new(
                    RectPos::new_empty(),
                    None,
                    true,
                )),
                Box::new(Window::new(
                    RectPos::new_empty(),
                    None,
                    false,
                )),
            ],
            SplitDirection::Horizontal,
        );

        for child in splitter.children.iter_mut() {
            child.set_parent(Some(core::ptr::addr_of_mut!(splitter)));
        }

        Self {
            container_tree: Box::new(splitter),
            focused,
        }
    }
}

impl Splitter {
    fn new(
        pos: RectPos,
        parent: Option<*mut dyn Container>,
        children: Vec<Box<dyn Container>>,
        split_direction: SplitDirection,
    ) -> Splitter {
        Self {
            id: Rand64::new(SEED).rand_u64(),
            pos,
            split_direction,
            parent,
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
    fn new(pos: RectPos, parent: Option<*mut dyn Container>, focused: bool) -> Window {
        Self {
            id: Rand64::new(SEED).rand_u64(),
            pos,
            parent,
            children: Vec::new(),
            focused,
        }
    }
}

trait Container: Focusable {
    fn id(&self) -> u64;
    fn position(&self) -> Vertex;
    fn width(&self) -> u32;
    fn height(&self) -> u32;
    fn set_position(&mut self, new_pos: Vertex);
    fn set_width(&mut self, new_width: u32);
    fn set_height(&mut self, new_height: u32);
    fn parent(&mut self) -> Option<*mut dyn Container>;
    fn set_parent(&mut self, parent: Option<*mut dyn Container>);
    fn children(&mut self) -> &mut Vec<Box<dyn Container>>;
    fn layout_content(&mut self);
    fn draw(&self);
}

impl Container for Splitter {
    fn id(&self) -> u64 {
        self.id
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
            }
            SplitDirection::Vertical => {
                let child_width = self.pos.width / (self.children.len() as u32);

                for child in self.children.iter_mut() {
                    child.set_position(top_left);
                    child.set_height(self.pos.height);
                    child.set_width(child_width);

                    top_left.x += child_width;
                }
            }
        }

        self.children
            .iter_mut()
            .for_each(|child| child.layout_content());
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

    fn parent(&mut self) -> Option<*mut dyn Container> {
        self.parent
    }

    fn set_parent(&mut self, parent: Option<*mut dyn Container>) {
        self.parent = parent;
    }
}

impl Container for Window {
    fn layout_content(&mut self) {
        // TODO: Add content
    }

    fn id(&self) -> u64 {
        self.id
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

    fn parent(&mut self) -> Option<*mut dyn Container> {
        self.parent
    }

    fn set_parent(&mut self, parent: Option<*mut dyn Container>) {
        self.parent = parent;
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
