#![no_std]

extern crate alloc;

use core::sync::atomic::{AtomicUsize, Ordering};

use alloc::{boxed::Box, string::ToString, vec::Vec};
use api::Api;
use components::{component::Component, label::Label};
use config::*;
use drawer::drawer::{Drawer, RectData, Vertex};
use graphic::{
    color::{WHITE, YELLOW},
    lfb::{CHAR_HEIGHT, CHAR_WIDTH},
};
use hashbrown::HashMap;
use io::{read::read, Application};
#[allow(unused_imports)]
use runtime::*;
use spin::{once::Once, Mutex};
use window::Window;
use workspace::Workspace;

pub mod api;
mod apps;
mod components;
mod config;
mod window;
mod workspace;

/// Ids are unique across all components
static ID_COUNTER: AtomicUsize = AtomicUsize::new(0);
pub static SCREEN: Once<(u32, u32)> = Once::new();

static mut API: Once<Mutex<Api>> = Once::new();

enum SplitType {
    Horizontal,
    Vertical,
}

struct WindowManager {
    workspaces: Vec<Workspace>,
    current_workspace: usize,
    screen: (u32, u32),
    global_components: HashMap<usize, Box<dyn Component>>,
}

impl WindowManager {
    pub fn generate_id() -> usize {
        ID_COUNTER.fetch_add(1, Ordering::SeqCst)
    }

    fn new(screen: (u32, u32)) -> Self {
        Self {
            workspaces: Vec::new(),
            current_workspace: 0,
            screen,
            global_components: HashMap::new(),
        }
    }

    fn init(&mut self) {
        unsafe {
            API.call_once(|| Mutex::new(Api::new(self.screen)));
        }
        SCREEN.call_once(|| self.screen);

        self.create_workspace_selection_label();
        self.create_new_workspace(true);
    }

    fn run(&mut self) {
        loop {
            self.draw();
            let keyboard_press = read(Application::WindowManager);

            match keyboard_press {
                'g' => {
                    self.create_new_workspace(false);
                }
                'q' => {
                    self.switch_prev_workspace();
                }
                'e' => {
                    self.switch_next_workspace();
                }
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
                    self.workspaces[self.current_workspace].focus_prev_window();
                }
                'd' => {
                    self.workspaces[self.current_workspace].focus_next_window();
                }
                //TODO: Add merge functionality. Make it buddy-style merging when both buddies finished
                // running their application
                'm' => {}
                'p' => {
                    break;
                }
                _ => {}
            }
        }
    }

    fn create_workspace_selection_label(&mut self) {
        self.add_global_window(
            Vertex::new(DIST_TO_SCREEN_EDGE, DIST_TO_SCREEN_EDGE),
            self.screen.0 - DIST_TO_SCREEN_EDGE * 2,
            HEIGHT_WORKSPACE_SELECTION_LABEL_WDW,
        );
        Drawer::draw_char(
            '1',
            Vertex::new(DIST_TO_SCREEN_EDGE + 1, DIST_TO_SCREEN_EDGE + 1),
            WHITE,
        );
    }

    fn switch_workspace(&mut self, workspace_index: usize) {
        if workspace_index < self.workspaces.len() {
            self.current_workspace = workspace_index;
        }
    }

    /// Global windows are not tied to workspaces, they exist once and persist through workspace-switches
    fn add_global_window(&mut self, pos: Vertex, width: u32, height: u32) {
        let window_id = Self::generate_id();
        let window = Window::new(window_id, pos, width, height);

        self.global_components.insert(window_id, Box::new(window));
    }

    fn add_window_to_workspace(
        &mut self,
        pos: Vertex,
        width: u32,
        height: u32,
        is_focusable: bool,
    ) {
        let window_id = Self::generate_id();
        let window = Window::new(window_id, pos, width, height);

        let curr_ws = &mut self.workspaces[self.current_workspace];

        if is_focusable {
            let focused_window_id = curr_ws.focused_window_id;
            curr_ws.insert_focusable_window(Box::new(window), focused_window_id);
        } else {
            curr_ws.insert_unfocusable_window(Box::new(window));
        }

        let _handle = unsafe {
            API.get_mut().unwrap().lock().register(
                self.current_workspace,
                window_id,
                RectData {
                    top_left: pos,
                    width,
                    height,
                },
            )
        };
    }

    fn split_window(&mut self, window_id: usize, split_type: SplitType) {
        let curr_ws = &mut self.workspaces[self.current_workspace];

        if let Some(component) = curr_ws.components.get_mut(&window_id) {
            if let Some(window) = component.as_any_mut().downcast_mut::<Window>() {
                match split_type {
                    SplitType::Horizontal => {
                        window.height /= 2;
                        let (width, height) = (window.width, window.height);
                        let top_left = Vertex::new(window.pos.x, window.pos.y + window.height);
                        self.add_window_to_workspace(top_left, width, height, true);
                    }
                    SplitType::Vertical => {
                        window.width /= 2;
                        let (width, height) = (window.width, window.height);
                        let top_left = Vertex::new(window.pos.x + window.width, window.pos.y);
                        self.add_window_to_workspace(top_left, width, height, true);
                    }
                }
            }
        }
    }

    fn focus_next_window(&mut self) {
        let curr_ws = &mut self.workspaces[self.current_workspace];
        if let Some(current_id) = curr_ws.focused_window_id {
            // Get the next window id to focus
            let next_id = (current_id + 1) % curr_ws.components.len();
            curr_ws.focused_window_id = Some(next_id);
        }
    }

    fn focus_prev_window(&mut self) {
        let curr_ws = &mut self.workspaces[self.current_workspace];
        if let Some(current_id) = curr_ws.focused_window_id {
            // Get the previous window id to focus
            let prev_id = if current_id == 0 {
                curr_ws.components.len() - 1
            } else {
                current_id - 1
            };
            curr_ws.focused_window_id = Some(prev_id);
        }
    }

    fn create_new_workspace(&mut self, is_initial: bool) {
        let window = Window::new(
            Self::generate_id(),
            Vertex::new(
                DIST_TO_SCREEN_EDGE,
                DIST_TO_SCREEN_EDGE + HEIGHT_WORKSPACE_SELECTION_LABEL_WDW,
            ),
            self.screen.0 - DIST_TO_SCREEN_EDGE * 2,
            self.screen.1 - (DIST_TO_SCREEN_EDGE * 2 + HEIGHT_WORKSPACE_SELECTION_LABEL_WDW),
        );
        let window_id = window.id;

        if !is_initial {
            self.current_workspace += 1;
        }
        let mut workspace =
            Workspace::new_with_single_window((window_id, Box::new(window)), Some(window_id));

        let workspaces_len = (self.workspaces.len() + 1) as u32;

        let workspace_selection_label = Label::new(
            Self::generate_id(),
            Vertex::new(
                DIST_TO_SCREEN_EDGE + workspaces_len * CHAR_WIDTH,
                DIST_TO_SCREEN_EDGE + CHAR_HEIGHT,
            ),
            char::from_digit(workspaces_len, 10).unwrap().to_string(),
        );
        workspace.insert_label(Box::new(workspace_selection_label));

        self.workspaces.insert(self.current_workspace, workspace);
    }

    fn switch_prev_workspace(&mut self) {
        self.current_workspace = self.current_workspace.saturating_sub(1);
    }

    fn switch_next_workspace(&mut self) {
        self.current_workspace = (self.current_workspace + 1).min(self.workspaces.len() - 1);
    }

    fn draw(&self) {
        Drawer::clear_screen();
        // Redraw global windows
        for (_, component) in self.global_components.iter() {
            component.draw(WHITE);
        }
        // Redraw workspace components
        let curr_ws = &self.workspaces[self.current_workspace];
        // Redraw windows
        let mut focused_window: Option<&Window> = None;
        for (_, component) in curr_ws.components.iter() {
            if let Some(window) = component.as_any().downcast_ref::<Window>() {
                if curr_ws
                    .focused_window_id
                    .is_some_and(|focused_id| focused_id == window.id)
                {
                    let _ = focused_window.insert(window);
                    continue;
                }
            }
            if let Some(window) = component.as_any().downcast_ref::<Window>() {
                component.draw(WHITE);
            }
        }
        // We draw the focused window last to prevent drawing over diff-colored edges
        if let Some(window) = focused_window {
            window.draw(YELLOW);
        };
        // TODO: Redraw labels
    }
}

#[no_mangle]
fn main() {
    let resolution = Drawer::get_graphic_resolution();
    let mut window_manager = WindowManager::new(resolution);
    window_manager.init();
    window_manager.run();
}
