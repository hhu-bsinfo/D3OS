#![no_std]

extern crate alloc;

use core::sync::atomic::{AtomicUsize, Ordering};

use alloc::vec;
use alloc::vec::Vec;
use api::Api;
use drawer::drawer::{Drawer, RectData, Vertex};
use graphic::color;
use hashbrown::HashMap;
use io::{read::read, Application};
#[allow(unused_imports)]
use runtime::*;
use spin::{once::Once, Mutex};

pub mod api;
mod components;

static ID_COUNTER: AtomicUsize = AtomicUsize::new(0);
pub static SCREEN: Once<(u32, u32)> = Once::new();

static mut API: Once<Mutex<Api>> = Once::new();

#[derive(Debug)]
enum SplitType {
    Horizontal,
    Vertical,
}

#[derive(Debug)]
struct Window {
    id: usize,
    parent_id: Option<usize>,
    partner_id: Option<usize>,
    pos: Vertex,
    width: u32,
    height: u32,
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
    fn new(windows: HashMap<usize, Window>, focused_window_id: Option<usize>) -> Self {
        Self {
            windows,
            focused_window_id,
        }
    }
}

impl Window {
    fn new(
        id: usize,
        parent_id: Option<usize>,
        partner_id: Option<usize>,
        pos: Vertex,
        width: u32,
        height: u32,
    ) -> Self {
        Self {
            id,
            parent_id,
            partner_id,
            pos,
            width,
            height,
        }
    }

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
    fn generate_id() -> usize {
        ID_COUNTER.fetch_add(1, Ordering::SeqCst)
    }

    fn new(screen: (u32, u32)) -> Self {
        unsafe {
            API.call_once(|| Mutex::new(Api::new(screen)));
        }
        SCREEN.call_once(|| screen);

        let window = Window::new(
            Self::generate_id(),
            None,
            None,
            Vertex::new(10, 10),
            screen.0 - 20,
            screen.1 - 20,
        );

        let mut windows = HashMap::new();
        let window_id = window.id;
        windows.insert(window_id, window);

        Self {
            workspaces: vec![Workspace::new(windows, Some(window_id))],
            current_workspace: 0,
            screen,
        }
    }

    fn run(&mut self) {
        loop {
            self.draw();
            let keyboard_press = read(Application::WindowManager);

            match keyboard_press {
                'h' => {
                    let window_id = self.workspaces[self.current_workspace].focused_window_id;
                    if window_id.is_some() {
                        self.split_window(window_id.unwrap(), SplitType::Horizontal);
                    }
                }
                'v' => {
                    let window_id = self.workspaces[self.current_workspace].focused_window_id;
                    if window_id.is_some() {
                        self.split_window(window_id.unwrap(), SplitType::Vertical);
                    }
                }
                'a' => {
                    self.focus_prev_window();
                }
                'd' => {
                    self.focus_next_window();
                }
                'p' => {
                    break;
                }
                _ => {}
            }
        }
    }

    fn switch_workspace(&mut self, workspace_index: usize) {
        if workspace_index < self.workspaces.len() {
            self.current_workspace = workspace_index;
        }
    }

    fn add_window(&mut self, pos: Vertex, parent_id: Option<usize>, width: u32, height: u32) {
        let window_id = Self::generate_id();
        let window = Window::new(window_id, None, parent_id, pos, width, height);

        self.workspaces[self.current_workspace]
            .windows
            .insert(window_id, window);
    }

    fn split_window(&mut self, window_id: usize, split_type: SplitType) {
        // let id = Self::generate_id();
        let curr_ws = &mut self.workspaces[self.current_workspace];

        if let Some(window) = curr_ws.windows.get_mut(&window_id) {
            let parent_id = window.id;
            match split_type {
                SplitType::Horizontal => {
                    window.height /= 2;
                    let (width, height) = (window.width, window.height);
                    let top_left = Vertex::new(window.pos.x, window.pos.y + window.height);
                    self.add_window(top_left, Some(parent_id), width, height);

                    let handle = unsafe {
                        API.get_mut().unwrap().lock().register(RectData {
                            top_left,
                            width,
                            height,
                        })
                    };
                }
                SplitType::Vertical => {
                    window.width /= 2;
                    let (width, height) = (window.width, window.height);
                    let top_left = Vertex::new(window.pos.x + window.width, window.pos.y);
                    self.add_window(top_left, Some(parent_id), width, height);

                    let handle = unsafe {
                        API.get_mut().unwrap().lock().register(RectData {
                            top_left,
                            width,
                            height,
                        })
                    };
                }
            }
        }
    }

    fn focus_next_window(&mut self) {
        let curr_ws = &mut self.workspaces[self.current_workspace];
        if let Some(current_id) = curr_ws.focused_window_id {
            // Get the next window id to focus
            let next_id = (current_id + 1) % curr_ws.windows.len();
            curr_ws.focused_window_id = Some(next_id);
        }
    }

    fn focus_prev_window(&mut self) {
        let curr_ws = &mut self.workspaces[self.current_workspace];
        if let Some(current_id) = curr_ws.focused_window_id {
            // Get the previous window id to focus
            let prev_id = if current_id == 0 {
                curr_ws.windows.len() - 1
            } else {
                current_id - 1
            };
            curr_ws.focused_window_id = Some(prev_id);
        }
    }

    fn draw(&self) {
        Drawer::clear_screen();
        let curr_ws = &self.workspaces[self.current_workspace];
        let mut focused_window: Option<&Window> = None;
        for (_, window) in curr_ws.windows.iter() {
            if curr_ws
                .focused_window_id
                .is_some_and(|focused_id| focused_id == window.id)
            {
                let _ = focused_window.insert(window);
                continue;
            }
            window.draw(curr_ws.focused_window_id);
        }
        focused_window.inspect(|wdw| wdw.draw(Some(wdw.id)));
    }
}

#[no_mangle]
fn main() {
    let resolution = Drawer::get_graphic_resolution();
    let mut window_manager = WindowManager::new(resolution);
    window_manager.run();
}
