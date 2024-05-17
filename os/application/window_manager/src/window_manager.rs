#![no_std]

extern crate alloc;

use core::sync::atomic::{AtomicUsize, Ordering};

use alloc::vec;
use alloc::vec::Vec; 
use drawer::drawer::{Drawer, Vertex};
use graphic::color;
use hashbrown::HashMap;
use runtime::*;

static ID_COUNTER: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug)]
enum SplitType {
    Horizontal,
    Vertical,
}

#[derive(Debug)]
struct Window {
    id: usize,
    parent_id: Option<usize>,
    pos: Vertex,
    width: u32,
    height: u32,
    split_type: SplitType,
}

#[derive(Debug)]
struct WindowManager {
    workspaces: Vec<Workspace>,
    current_workspace: usize,
    screen: (u32, u32),
}

#[derive(Debug)]
struct Workspace {
    windows: HashMap<usize, Window>,
    focused_window_id: Option<usize>,
}

impl Workspace {
    fn new() -> Self {
        Self {
            windows: HashMap::new(),
            focused_window_id: None,
        }
    }
}

impl Window {
    fn draw(&self, focused_window_id: Option<usize>) {
        let color = if focused_window_id.is_some_and(|focused| focused == self.id) {
            color::YELLOW
        } else {
            color::WHITE
        };
        Drawer::draw_rectangle(
            Vertex::new(self.pos.x, self.pos.y), 
            Vertex::new(self.pos.x + self.width, self.pos.y + self.height), 
            color,
        );
    }
}

impl WindowManager {
    fn new(screen: (u32, u32)) -> Self {
        Self {
            workspaces: vec![Workspace::new()],
            current_workspace: 0,
            screen,
        }
    }

    fn generate_id(&self) -> usize {
        ID_COUNTER.fetch_add(1, Ordering::SeqCst)
    }

    fn switch_workspace(&mut self, workspace_index: usize) {
        if workspace_index < self.workspaces.len() {
            self.current_workspace = workspace_index;
        }
    }

    fn add_window(&mut self, pos: Vertex, width: u32, height: u32) {
        let window_id = self.generate_id();
        let window = Window {
            id: window_id,
            parent_id: None,
            pos,
            width,
            height,
            split_type: SplitType::Horizontal,
        };

        self.workspaces[self.current_workspace].windows.insert(window_id, window);
    }

    fn split_window(&mut self, window_id: usize) {
        let id = self.generate_id();
        let workspace = &mut self.workspaces[self.current_workspace];

        if let Some(window) = workspace.windows.get_mut(&window_id) {
            let parent_id = window.id;
            match window.split_type {
                SplitType::Horizontal => {
                    window.height /= 2;
                    let new_window = Window {
                        id,
                        parent_id: Some(parent_id),
                        pos: Vertex::new(window.pos.x, window.pos.y + window.height),
                        width: window.width,
                        height: window.height,
                        split_type: SplitType::Horizontal,
                    };
                    workspace.windows.insert(new_window.id, new_window);
                }
                SplitType::Vertical => {
                    window.width /= 2;
                    let new_window = Window {
                        id,
                        parent_id: Some(parent_id),
                        pos: Vertex::new(window.pos.x + window.width, window.pos.y),
                        width: window.width,
                        height: window.height,
                        split_type: SplitType::Horizontal,
                    };
                    workspace.windows.insert(new_window.id, new_window);
                }
            }
        }
    }

    fn focus_next_window(&mut self) {
        let workspace = &mut self.workspaces[self.current_workspace];
        if let Some(current_id) = workspace.focused_window_id {
            // Get the next window id to focus
            let next_id = (current_id + 1) % workspace.windows.len();
            workspace.focused_window_id = Some(next_id);
        }
    }

    fn focus_prev_window(&mut self) {
        let workspace = &mut self.workspaces[self.current_workspace];
        if let Some(current_id) = workspace.focused_window_id {
            // Get the previous window id to focus
            let prev_id = if current_id == 0 {
                workspace.windows.len() - 1
            } else {
                current_id - 1
            };
            workspace.focused_window_id = Some(prev_id);
        }
    }

    fn draw(&self) {
        Drawer::clear_screen();
        let curr_ws = &self.workspaces[self.current_workspace];
        for (_, window) in curr_ws.windows.iter() {
            window.draw(curr_ws.focused_window_id);
        }
    }

    fn run(&self) {

    }
}

#[no_mangle]
fn main() {
    let resolution = Drawer::get_graphic_resolution();
    let mut window_manager = WindowManager::new(resolution);
    // window_manager.add_window(Vertex::new(100, 100), 300, 400);

    window_manager.draw();

    // // Split the first window
    // window_manager.split_window(0);
    // window_manager.draw();
}