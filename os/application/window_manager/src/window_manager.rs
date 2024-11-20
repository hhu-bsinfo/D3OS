#![no_std]
#![feature(linked_list_cursors)]

extern crate alloc;

use core::sync::atomic::{AtomicUsize, Ordering};

use alloc::{borrow::ToOwned, vec::Vec};
use api::{Api, NewCompData, NewLoopIterFnData, Receivers, Senders, WindowData, DEFAULT_APP};
use components::selected_window_label::HEIGHT_WORKSPACE_SELECTION_LABEL_WINDOW;
use config::{BACKSPACE_UNICODE, COMMAND_LINE_WINDOW_Y_PADDING, DIST_TO_SCREEN_EDGE};
use drawer::drawer::Drawer;
use drawer::rect_data::RectData;
use drawer::vertex::Vertex;
use graphic::lfb::DEFAULT_CHAR_HEIGHT;
use io::{read::try_read, Application};
use nolock::queues::mpsc::jiffy;
#[allow(unused_imports)]
use runtime::*;
use spin::{once::Once, Mutex, MutexGuard};
use windows::workspace_selection_labels_window::WorkspaceSelectionLabelsWindow;
use windows::{app_window::AppWindow, command_line_window::CommandLineWindow};
use workspace::Workspace;

pub mod api;
mod apps;
mod components;
mod config;
mod utils;
mod window_tree;
mod windows;
mod workspace;

// IDs are unique across all components
static ID_COUNTER: AtomicUsize = AtomicUsize::new(1);
/// Global screen resolution, initialized in [`WindowManager::init()`]
static SCREEN: Once<(u32, u32)> = Once::new();
/// API instance to communicate between applications and window-manager
static mut API: Once<Mutex<Api>> = Once::new();

#[derive(Clone, Copy)]
enum ScreenSplitType {
    Horizontal,
    Vertical,
}

struct WindowManager {
    workspaces: Vec<Workspace>,
    /// Currently selected workspace
    current_workspace: usize,
    /// This window not tied to workspaces, it exists once and persists through workspace-switches
    workspace_selection_labels_window: WorkspaceSelectionLabelsWindow,
    command_line_window: CommandLineWindow,
    /// Receivers from queues connected with API
    receivers: Receivers,
    /// List of closures to call on each loop-iteration, sent from API
    on_loop_iter_fns: Vec<NewLoopIterFnData>,
    /// Determines if a full redraw is required in the next loop-iteration
    is_dirty: bool,
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

    fn new(screen: (u32, u32)) -> (Self, Senders) {
        SCREEN.call_once(|| screen);

        let (rx_components, tx_components) = jiffy::queue::<NewCompData>();
        let (rx_on_loop_iter, tx_on_loop_iter) = jiffy::queue::<NewLoopIterFnData>();

        let senders = Senders {
            tx_components,
            tx_on_loop_iter,
        };

        let receivers = Receivers {
            rx_components,
            rx_on_loop_iter,
        };

        let workspace_selection_labels_window = WorkspaceSelectionLabelsWindow::new(RectData {
            top_left: Vertex::new(DIST_TO_SCREEN_EDGE, DIST_TO_SCREEN_EDGE),
            width: screen.0 - DIST_TO_SCREEN_EDGE * 2,
            height: HEIGHT_WORKSPACE_SELECTION_LABEL_WINDOW,
        });

        let command_line_window_height = DEFAULT_CHAR_HEIGHT + COMMAND_LINE_WINDOW_Y_PADDING * 2;
        let command_line_window = CommandLineWindow::new(RectData {
            top_left: Vertex::new(
                DIST_TO_SCREEN_EDGE,
                screen.1 - DIST_TO_SCREEN_EDGE - command_line_window_height,
            ),
            width: screen.0 - DIST_TO_SCREEN_EDGE * 2,
            height: command_line_window_height,
        });

        (
            Self {
                workspaces: Vec::new(),
                current_workspace: 0,
                workspace_selection_labels_window,
                command_line_window,
                receivers,
                is_dirty: true,
                on_loop_iter_fns: Vec::new(),
            },
            senders,
        )
    }

    fn init(&mut self, senders: Senders) {
        unsafe {
            API.call_once(|| Mutex::new(Api::new(senders)));
        }

        self.create_new_workspace(true);
    }

    fn run(&mut self) {
        loop {
            self.draw();

            self.process_keyboard_input();

            self.add_new_components_from_api();

            self.add_new_closures_from_api();

            self.call_on_loop_iter_fns();
        }
    }

    fn call_on_loop_iter_fns(&mut self) {
        for NewLoopIterFnData { window_data, fun } in self.on_loop_iter_fns.iter() {
            let is_dirty = (*fun)();

            let window = self.workspaces[window_data.workspace_index]
                .windows
                .get_mut(&window_data.window_id);

            if is_dirty {
                if let Some(window) = window {
                    window.is_dirty = true;
                }
            }
        }
    }

    fn add_new_closures_from_api(&mut self) {
        while let Ok(data) = self.receivers.rx_on_loop_iter.try_dequeue() {
            self.on_loop_iter_fns.push(data);
        }
    }

    fn process_keyboard_input(&mut self) {
        let read_option = try_read(Application::WindowManager);

        if let Some(keyboard_press) = read_option {
            // `enter_app_mode` overrides all other keyboard-interactions
            if self.command_line_window.enter_app_mode {
                self.process_enter_app_mode(keyboard_press);
            } else {
                let block_interact = self
                    .get_current_workspace_mut()
                    .get_focused_window_mut()
                    .interact_with_focused_component(keyboard_press);

                if block_interact {
                    return;
                }

                match keyboard_press {
                    'c' => {
                        self.create_new_workspace(false);
                        self.workspace_selection_labels_window.is_dirty = true;
                    }
                    'x' => {
                        self.remove_current_workspace();
                        self.workspace_selection_labels_window.is_dirty = true;
                    }
                    'q' => {
                        self.switch_prev_workspace();
                        self.workspace_selection_labels_window.is_dirty = true;
                    }
                    'e' => {
                        self.switch_next_workspace();
                        self.workspace_selection_labels_window.is_dirty = true;
                    }
                    'o' => {
                        self.get_current_workspace_mut()
                            .move_focused_window_forward();
                    }
                    'i' => {
                        self.get_current_workspace_mut()
                            .move_focused_window_backward();
                    }
                    'h' => {
                        self.command_line_window
                            .activate_enter_app_mode(ScreenSplitType::Horizontal);
                    }
                    'v' => {
                        self.command_line_window
                            .activate_enter_app_mode(ScreenSplitType::Vertical);
                    }
                    'w' => {
                        self.get_current_workspace_mut().focus_next_component();
                    }
                    's' => {
                        self.get_current_workspace_mut().focus_prev_component();
                    }
                    'a' => {
                        self.get_current_workspace_mut().focus_prev_window();
                    }
                    'd' => {
                        self.get_current_workspace_mut().focus_next_window();
                    }
                    'm' => {
                        /* Only works, if both buddies don't have subwindows inside them.
                        Move windows up before merging, if that is a problem */
                        let was_closed = self.get_current_workspace_mut().close_focused_window();
                        if was_closed {
                            self.is_dirty = true;
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    fn process_enter_app_mode(&mut self, keyboard_press: char) {
        match keyboard_press {
            '\n' => {
                if !self.command_line_window.command.is_empty()
                    && Self::get_api().is_app_name_valid(&self.command_line_window.command)
                {
                    let command = self.command_line_window.command.to_owned();
                    self.split_window(
                        self.get_current_workspace().focused_window_id,
                        self.command_line_window.split_type,
                        command.as_str(),
                    );
                }

                self.command_line_window.deactivate_enter_app_mode();
                self.is_dirty = true;
            }
            BACKSPACE_UNICODE => {
                self.command_line_window.is_dirty = true;
                self.command_line_window.pop_char();
            }
            c => {
                self.command_line_window.is_dirty = true;
                self.command_line_window.push_char(c);
            }
        }
    }

    fn add_new_components_from_api(&mut self) {
        while let Ok(NewCompData {
            window_data:
                WindowData {
                    workspace_index,
                    window_id,
                },
            component,
        }) = self.receivers.rx_components.try_dequeue()
        {
            let curr_ws = &mut self.workspaces[workspace_index];
            let window = &mut curr_ws.windows.get_mut(&window_id);
            if let Some(window) = window {
                window.insert_component(component, true);
            }
        }
    }

    fn add_window_to_workspace(&mut self, rect_data: RectData, app_name: &str) {
        let window_id = Self::generate_id();
        let window = AppWindow::new(window_id, rect_data);

        let curr_ws = self.get_current_workspace_mut();

        curr_ws.insert_window(window, curr_ws.focused_window_id);

        self.is_dirty = true;

        Self::get_api()
            .register(self.current_workspace, window_id, rect_data, app_name)
            .expect("Failed to create window!");
    }

    fn split_window(&mut self, window_id: usize, split_type: ScreenSplitType, app_name: &str) {
        let curr_ws = self.get_current_workspace_mut();

        if let Some(window) = curr_ws.windows.get_mut(&window_id) {
            let old_rect @ RectData {
                top_left: old_top_left,
                width: old_width,
                height: old_height,
            } = window.rect_data;
            match split_type {
                ScreenSplitType::Horizontal => {
                    window.rect_data.height /= 2;
                    let new_top_left = old_top_left.add(0, window.rect_data.height);
                    let new_rect_data = RectData {
                        top_left: new_top_left,
                        width: old_width,
                        height: window.rect_data.height,
                    };

                    // Rescale components for old window
                    window.rescale_window_in_place(old_rect, window.rect_data.clone());

                    self.add_window_to_workspace(new_rect_data, app_name);
                }
                ScreenSplitType::Vertical => {
                    window.rect_data.width /= 2;
                    let new_top_left = old_top_left.add(window.rect_data.width, 0);
                    let new_rect_data = RectData {
                        top_left: new_top_left,
                        width: window.rect_data.width,
                        height: old_height,
                    };

                    // Rescale components for old window
                    window.rescale_window_in_place(old_rect, window.rect_data.clone());

                    self.add_window_to_workspace(new_rect_data, app_name);
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
        let window_rect_data = RectData {
            top_left: Vertex::new(
                DIST_TO_SCREEN_EDGE,
                DIST_TO_SCREEN_EDGE + HEIGHT_WORKSPACE_SELECTION_LABEL_WINDOW,
            ),
            width: screen_res.0 - DIST_TO_SCREEN_EDGE * 2,
            height: screen_res.1
                - (DIST_TO_SCREEN_EDGE * 2 + HEIGHT_WORKSPACE_SELECTION_LABEL_WINDOW),
        };

        let window = AppWindow::new(Self::generate_id(), window_rect_data);
        let window_id = window.id;

        self.workspace_selection_labels_window
            .insert_label(self.workspaces.len());

        let workspace = Workspace::new_with_single_window((window_id, window), window_id);

        self.workspaces.insert(self.current_workspace, workspace);

        Self::get_api()
            .register(
                self.current_workspace,
                window_id,
                window_rect_data,
                DEFAULT_APP,
            )
            .expect("Failed to launch default app!");
    }

    fn remove_current_workspace(&mut self) {
        if self.workspaces.len() == 1 {
            return;
        }

        self.workspaces.remove(self.current_workspace);
        self.workspace_selection_labels_window
            .remove_label(self.current_workspace);

        self.on_loop_iter_fns
            .retain_mut(|fun| fun.window_data.workspace_index != self.current_workspace);
        self.on_loop_iter_fns
            .iter_mut()
            .filter(|fun| fun.window_data.workspace_index > self.current_workspace)
            .for_each(|fun| fun.window_data.workspace_index -= 1);

        Self::get_api().remove_all_handles_tied_to_workspace(self.current_workspace);

        self.current_workspace = if self.current_workspace == 0 {
            self.workspaces.len() - 1
        } else {
            self.current_workspace - 1
        };

        self.is_dirty = true;
    }

    fn switch_prev_workspace(&mut self) {
        self.current_workspace = if self.current_workspace == 0 {
            self.workspaces.len() - 1
        } else {
            self.current_workspace - 1
        };
        self.is_dirty = true;
    }

    fn switch_next_workspace(&mut self) {
        self.current_workspace = (self.current_workspace + 1) % self.workspaces.len();
        self.is_dirty = true;
    }

    fn get_current_workspace(&self) -> &Workspace {
        &self.workspaces[self.current_workspace]
    }

    fn get_current_workspace_mut(&mut self) -> &mut Workspace {
        &mut self.workspaces[self.current_workspace]
    }

    fn draw(&mut self) {
        // In enter_app_mode, we freeze everything else and only redraw the command-line-window
        if self.command_line_window.enter_app_mode {
            self.command_line_window.draw();
            return;
        }

        let is_dirty = self.is_dirty;

        if is_dirty {
            Drawer::full_clear_screen(false);
        }

        let focused_window_id = self.get_current_workspace().focused_window_id;

        // // Redraw everything related to workspace-selection-labels
        self.workspace_selection_labels_window
            .draw(self.current_workspace, is_dirty);

        let curr_ws = self.get_current_workspace_mut();
        // Redraw workspace windows
        for window in curr_ws.windows.values_mut() {
            window.draw(focused_window_id, is_dirty);
        }

        self.is_dirty = false;
    }
}

#[no_mangle]
fn main() {
    let resolution = Drawer::get_graphic_resolution();
    let (mut window_manager, senders) = WindowManager::new(resolution);
    window_manager.init(senders);
    window_manager.run();
}
