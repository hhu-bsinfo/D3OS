#![no_std]

extern crate alloc;

use core::sync::atomic::{AtomicUsize, Ordering};

use alloc::{borrow::ToOwned, boxed::Box, string::ToString, vec::Vec};
use api::Api;
use components::{
    component::Component, selected_window_label::SelectedWorkspaceLabel, window::Window,
};
use config::*;
use drawer::drawer::{Drawer, RectData, Vertex};
use graphic::{
    color::{WHITE, YELLOW},
    lfb::{DEFAULT_CHAR_HEIGHT, DEFAULT_CHAR_WIDTH},
};
use hashbrown::HashMap;
use io::{read::read, Application};
use nolock::queues::mpsc::jiffy::{self, Receiver, Sender};
#[allow(unused_imports)]
use runtime::*;
use spin::{once::Once, Mutex, MutexGuard};
use workspace::Workspace;

pub mod api;
mod apps;
mod components;
mod config;
mod workspace;

// Ids are unique across all components
static ID_COUNTER: AtomicUsize = AtomicUsize::new(1);
/// Global screen resolution, initialized in [`WindowManager::init()`]
static SCREEN: Once<(u32, u32)> = Once::new();

static mut API: Once<Mutex<Api>> = Once::new();

// Screen-split types
enum SplitType {
    Horizontal,
    Vertical,
}

struct WindowManager {
    workspaces: Vec<Workspace>,
    // Currently selected workspace
    current_workspace: usize,
    // Global windows are not tied to workspaces, they exist once and persist through workspace-switches
    global_components: HashMap<usize, Box<dyn Component>>,
    // Receiver end of queue with API. Components created in API are transmitted this way
    receiver_api: Receiver<Box<dyn Component>>,
}

impl WindowManager {
    pub fn generate_id() -> usize {
        ID_COUNTER.fetch_add(1, Ordering::SeqCst)
    }

    fn get_screen_res() -> (u32, u32) {
        SCREEN
            .get()
            .expect("Screen-resolution accessed before init")
            .to_owned()
    }

    fn get_api() -> MutexGuard<'static, Api> {
        unsafe { API.get_mut().expect("API accessed before init").lock() }
    }

    fn new(screen: (u32, u32)) -> (Self, Sender<Box<dyn Component>>) {
        SCREEN.call_once(|| screen);

        let (rx, tx) = jiffy::queue::<Box<dyn Component>>();

        (
            Self {
                workspaces: Vec::new(),
                current_workspace: 0,
                global_components: HashMap::new(),
                receiver_api: rx,
            },
            tx,
        )
    }

    fn init(&mut self, tx: Sender<Box<dyn Component>>) {
        unsafe {
            API.call_once(|| Mutex::new(Api::new(Self::get_screen_res(), tx)));
        }

        self.create_workspace_selection_labels_window();
        self.create_new_workspace(true);
    }

    fn run(&mut self) {
        loop {
            self.draw();

            let keyboard_press = read(Application::WindowManager);

            match keyboard_press {
                'c' => {
                    self.create_new_workspace(false);
                }
                'q' => {
                    self.switch_prev_workspace();
                }
                'e' => {
                    self.switch_next_workspace();
                }
                'h' => {
                    if let Some(window_id) =
                        self.workspaces[self.current_workspace].focused_window_id
                    {
                        self.split_window(window_id, SplitType::Horizontal);
                    }
                }
                'v' => {
                    if let Some(window_id) =
                        self.workspaces[self.current_workspace].focused_window_id
                    {
                        self.split_window(window_id, SplitType::Vertical);
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

            // Add all new components to corresponding workspace
            while let Ok(new_comp) = self.receiver_api.try_dequeue() {
                let workspace_index = new_comp.workspace_index();
                self.workspaces[workspace_index].insert_component(new_comp);
            }
        }
    }

    // This contains the numerical labels which show what workspace you are currently on
    fn create_workspace_selection_labels_window(&mut self) {
        self.add_global_window(
            Vertex::new(DIST_TO_SCREEN_EDGE, DIST_TO_SCREEN_EDGE),
            SCREEN.get().unwrap().0 - DIST_TO_SCREEN_EDGE * 2,
            HEIGHT_WORKSPACE_SELECTION_LABEL_WINDOW,
        );
    }

    fn switch_workspace(&mut self, workspace_index: usize) {
        if workspace_index < self.workspaces.len() {
            self.current_workspace = workspace_index;
        }
    }

    fn add_global_window(&mut self, pos: Vertex, width: u32, height: u32) {
        let window_id = Self::generate_id();
        let window = Window::new(window_id, 0, pos, width, height);

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
        let window = Window::new(window_id, self.current_workspace, pos, width, height);

        let curr_ws = &mut self.workspaces[self.current_workspace];

        if is_focusable {
            let focused_window_id = curr_ws.focused_window_id;
            curr_ws.insert_focusable_window(Box::new(window), focused_window_id);
        } else {
            curr_ws.insert_component(Box::new(window));
        }

        let _ = Self::get_api().register(
            self.current_workspace,
            window_id,
            RectData {
                top_left: pos,
                width,
                height,
            },
        );
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
        if self.workspaces.len() == 9 {
            return;
        }

        if !is_initial {
            self.current_workspace += 1;
        }

        let screen_res = Self::get_screen_res();
        let window = Window::new(
            Self::generate_id(),
            self.current_workspace,
            Vertex::new(
                DIST_TO_SCREEN_EDGE,
                DIST_TO_SCREEN_EDGE + HEIGHT_WORKSPACE_SELECTION_LABEL_WINDOW,
            ),
            screen_res.0 - DIST_TO_SCREEN_EDGE * 2,
            screen_res.1 - (DIST_TO_SCREEN_EDGE * 2 + HEIGHT_WORKSPACE_SELECTION_LABEL_WINDOW),
        );
        let window_id = window.id;

        let workspace =
            Workspace::new_with_single_window((window_id, Box::new(window)), Some(window_id));

        let new_workspace_len = (self.workspaces.len() + 1) as u32;

        let workspace_selection_label = SelectedWorkspaceLabel::new(
            Self::generate_id(),
            0,
            Vertex::new(
                DIST_TO_SCREEN_EDGE
                    + new_workspace_len * DEFAULT_CHAR_WIDTH
                    + SELECTED_WINDOW_LABEL_SPACING * (new_workspace_len - 1),
                DIST_TO_SCREEN_EDGE + DEFAULT_CHAR_HEIGHT,
            ),
            char::from_digit(new_workspace_len, 10).unwrap().to_string(),
            (new_workspace_len - 1) as usize,
        );

        self.global_components.insert(
            workspace_selection_label.id,
            Box::new(workspace_selection_label),
        );

        self.workspaces.insert(self.current_workspace, workspace);
    }

    fn switch_prev_workspace(&mut self) {
        self.current_workspace = self.current_workspace.saturating_sub(1);
    }

    fn switch_next_workspace(&mut self) {
        self.current_workspace = (self.current_workspace + 1) % self.workspaces.len();
    }

    fn draw(&self) {
        Drawer::clear_screen();
        let curr_ws = &self.workspaces[self.current_workspace];
        // Redraw global components
        for (_, component) in self.global_components.iter() {
            match component.as_any().downcast_ref::<SelectedWorkspaceLabel>() {
                Some(selected_window_label) => {
                    if self.current_workspace == selected_window_label.tied_workspace {
                        component.draw(YELLOW);
                        continue;
                    }
                    component.draw(WHITE);
                }
                None => component.draw(WHITE),
            }
        }
        // Redraw workspace components
        let mut focused_components: Vec<&Box<dyn Component>> = Vec::new();
        for (_, component) in curr_ws.components.iter() {
            if curr_ws
                .focused_window_id
                .is_some_and(|focused_id| focused_id == component.id())
            {
                focused_components.push(component);
                continue;
            }
            component.draw(WHITE);
        }
        // We draw the focused window last to prevent drawing over diff-colored edges
        focused_components.iter().for_each(|comp| comp.draw(YELLOW));
    }
}

#[no_mangle]
fn main() {
    let resolution = Drawer::get_graphic_resolution();
    let (mut window_manager, tx) = WindowManager::new(resolution);
    window_manager.init(tx);
    window_manager.run();
}
