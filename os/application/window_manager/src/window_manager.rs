#![no_std]

extern crate alloc;

use core::sync::atomic::{AtomicUsize, Ordering};

use alloc::{borrow::ToOwned, boxed::Box, string::ToString, vec::Vec};
use api::{Api, DispatchData};
use components::{
    component::Interaction, selected_window_label::SelectedWorkspaceLabel, window::Window,
};
use config::*;
use drawer::drawer::{Drawer, RectData, Vertex};
use graphic::{
    color::{WHITE, YELLOW},
    lfb::{DEFAULT_CHAR_HEIGHT, DEFAULT_CHAR_WIDTH},
};
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
    workspace_selection_labels_window: Window,
    // Receiver end of queue with API. Components created in API are transmitted this way
    receiver_api_components: Receiver<DispatchData>,
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

    fn new(screen: (u32, u32)) -> (Self, Sender<DispatchData>) {
        SCREEN.call_once(|| screen);

        let (rx_components, tx_components) = jiffy::queue::<DispatchData>();

        let workspace_selection_labels_window = Window::new(
            Self::generate_id(),
            0,
            RectData {
                top_left: Vertex::new(DIST_TO_SCREEN_EDGE, DIST_TO_SCREEN_EDGE),
                width: SCREEN.get().unwrap().0 - DIST_TO_SCREEN_EDGE * 2,
                height: HEIGHT_WORKSPACE_SELECTION_LABEL_WINDOW,
            },
        );

        (
            Self {
                workspaces: Vec::new(),
                current_workspace: 0,
                workspace_selection_labels_window,
                receiver_api_components: rx_components,
            },
            tx_components,
        )
    }

    fn init(&mut self, tx_components: Sender<DispatchData>) {
        unsafe {
            API.call_once(|| Mutex::new(Api::new(Self::get_screen_res(), tx_components)));
        }

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
                    self.split_window(
                        self.get_current_workspace().focused_window_id,
                        SplitType::Horizontal,
                    );
                }
                'v' => {
                    self.split_window(
                        self.get_current_workspace().focused_window_id,
                        SplitType::Vertical,
                    );
                }
                'w' => {
                    self.get_current_workspace_mut().focus_next_component();
                }
                's' => {
                    self.get_current_workspace_mut().focus_prev_component();
                }
                'f' => {
                    self.get_current_workspace()
                        .get_focused_window()
                        .interact_with_focused_component(Interaction::Press);
                }
                'a' => {
                    self.get_current_workspace_mut().focus_prev_window();
                }
                'd' => {
                    self.get_current_workspace_mut().focus_next_window();
                }
                //TODO: Add merge functionality. Make it buddy-style merging when both buddies finished
                // running their application
                'm' => {}
                'p' => {
                    break;
                }
                _ => {}
            }

            // Add all new components from queue to corresponding workspace
            while let Ok(DispatchData {
                workspace_index,
                window_id,
                component,
            }) = self.receiver_api_components.try_dequeue()
            {
                let curr_ws = &mut self.workspaces[workspace_index];
                let window = &mut curr_ws.windows.get_mut(&window_id);
                if let Some(window) = window {
                    window.insert_component(component, true);
                }
            }
        }
    }

    fn add_window_to_workspace(&mut self, rect_data: RectData, is_focusable: bool) {
        let window_id = Self::generate_id();
        let window = Window::new(window_id, self.current_workspace, rect_data);

        let curr_ws = self.get_current_workspace_mut();

        if is_focusable {
            let focused_window_id = curr_ws.focused_window_id;
            curr_ws.insert_focusable_window(window, Some(focused_window_id));
        } else {
            curr_ws.insert_unfocusable_window(window);
        }

        let _ = Self::get_api().register(self.current_workspace, window_id, rect_data);
    }

    fn split_window(&mut self, window_id: usize, split_type: SplitType) {
        let curr_ws = self.get_current_workspace_mut();

        if let Some(window) = curr_ws.windows.get_mut(&window_id) {
            let RectData {
                top_left: old_top_left,
                width: old_width,
                height: old_height,
            } = window.rect_data;
            match split_type {
                SplitType::Horizontal => {
                    window.rect_data.height /= 2;
                    let new_top_left = old_top_left.add(0, window.rect_data.height);
                    let new_rect_data = RectData {
                        top_left: new_top_left,
                        width: old_width,
                        height: window.rect_data.height,
                    };
                    self.add_window_to_workspace(new_rect_data, true);
                }
                SplitType::Vertical => {
                    window.rect_data.width /= 2;
                    let new_top_left = old_top_left.add(window.rect_data.width, 0);
                    let new_rect_data = RectData {
                        top_left: new_top_left,
                        width: window.rect_data.width,
                        height: old_height,
                    };
                    self.add_window_to_workspace(new_rect_data, true);
                }
            }
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
            RectData {
                top_left: Vertex::new(
                    DIST_TO_SCREEN_EDGE,
                    DIST_TO_SCREEN_EDGE + HEIGHT_WORKSPACE_SELECTION_LABEL_WINDOW,
                ),
                width: screen_res.0 - DIST_TO_SCREEN_EDGE * 2,
                height: screen_res.1
                    - (DIST_TO_SCREEN_EDGE * 2 + HEIGHT_WORKSPACE_SELECTION_LABEL_WINDOW),
            },
        );
        let window_id = window.id;

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

        self.workspace_selection_labels_window
            .insert_component(Box::new(workspace_selection_label), false);

        let workspace = Workspace::new_with_single_window((window_id, window), window_id);

        self.workspaces.insert(self.current_workspace, workspace);
    }

    fn switch_prev_workspace(&mut self) {
        self.current_workspace = self.current_workspace.saturating_sub(1);
    }

    fn switch_next_workspace(&mut self) {
        self.current_workspace = (self.current_workspace + 1) % self.workspaces.len();
    }

    fn get_current_workspace(&self) -> &Workspace {
        &self.workspaces[self.current_workspace]
    }

    fn get_current_workspace_mut(&mut self) -> &mut Workspace {
        &mut self.workspaces[self.current_workspace]
    }

    fn draw(&self) {
        Drawer::clear_screen();
        let curr_ws = self.get_current_workspace();

        // Redraw everything related to workspace-selection-labels
        self.workspace_selection_labels_window
            .draw(WHITE, curr_ws.focused_window_id);
        self.workspace_selection_labels_window
            .draw_selected_workspace_labels(self.current_workspace);

        // Redraw workspace windows
        for window in curr_ws.windows.values() {
            window.draw(WHITE, curr_ws.focused_window_id);
        }

        // Redraw focused element so yellow color is on top
        curr_ws
            .windows
            .get(&curr_ws.focused_window_id)
            .unwrap()
            .draw(YELLOW, curr_ws.focused_window_id);
    }
}

#[no_mangle]
fn main() {
    let resolution = Drawer::get_graphic_resolution();
    let (mut window_manager, tx_components) = WindowManager::new(resolution);
    window_manager.init(tx_components);
    window_manager.run();
}
