#![no_std]
#![feature(linked_list_cursors)]

extern crate alloc;

use alloc::{borrow::ToOwned, vec::Vec};
use api::{
    Api, NewCompData, NewLoopIterFnData, Receivers, ScreenSplitType, Senders, WindowData,
    WindowManagerMessage, DEFAULT_APP,
};
use chrono::TimeDelta;
use components::selected_window_label::HEIGHT_WORKSPACE_SELECTION_LABEL_WINDOW;
use concurrent::process;
use config::{
    COMMAND_LINE_WINDOW_Y_PADDING, DIST_TO_SCREEN_EDGE, FLUSHING_DELAY_MS,
};
use core::sync::atomic::{AtomicUsize, Ordering};
use drawer::drawer::Drawer;
use drawer::rect_data::RectData;
use drawer::vertex::Vertex;
use globals::hotkeys::HKEY_TOGGLE_TERMINAL_WINDOW;
use graphic::lfb::DEFAULT_CHAR_HEIGHT;
use keyboard_decoder::KeyboardDecoder;
use nolock::queues::mpsc::jiffy;

use input::mouse::try_read_mouse;
use mouse_state::{MouseEvent, MouseState};
#[allow(unused_imports)]
#[cfg(feature = "with_runtime")]
use runtime::*;
use spin::{once::Once, Mutex, MutexGuard};
use terminal::DecodedKey;
use time::systime;
use windows::workspace_selection_window::WorkspaceSelectionWindow;
use windows::{app_window::AppWindow, command_line_window::CommandLineWindow};
use workspace::Workspace;

pub mod api;
mod apps;
mod components;
mod config;
mod keyboard_decoder;
mod mouse_state;
mod signal;
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

pub enum Interaction {
    Keyboard(DecodedKey),
    Mouse(MouseEvent),
}

struct WindowManager {
    workspaces: Vec<Workspace>,
    /// Currently selected workspace
    current_workspace: usize,
    /// This window not tied to workspaces, it exists once and persists through workspace-switches
    //workspace_selection_labels_window: WorkspaceSelectionLabelsWindow,
    workspace_selection_window: WorkspaceSelectionWindow,
    command_line_window: CommandLineWindow,
    /// Receivers from queues connected with API
    receivers: Receivers,
    /// List of closures to call on each loop-iteration, sent from API
    on_loop_iter_fns: Vec<NewLoopIterFnData>,
    /// Determines if a full redraw is required in the next loop-iteration
    is_dirty: bool,
    last_frame_time: TimeDelta,
    start_time: TimeDelta,
    frames: i64,

    mouse_state: MouseState,
    keyboard_decoder: KeyboardDecoder,
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

        // Queues/Chanells for api communication
        let (rx_components, tx_components) = jiffy::queue::<NewCompData>();
        let (rx_on_loop_iter, tx_on_loop_iter) = jiffy::queue::<NewLoopIterFnData>();
        let (rx_messages, tx_messages) = jiffy::queue::<WindowManagerMessage>();

        let senders = Senders {
            tx_components,
            tx_on_loop_iter,
            tx_messages,
        };

        let receivers = Receivers {
            rx_components,
            rx_on_loop_iter,
            rx_messages,
        };

        // Workspace selection window
        let workspace_selection_window = WorkspaceSelectionWindow::new(RectData {
            top_left: Vertex::new(DIST_TO_SCREEN_EDGE, DIST_TO_SCREEN_EDGE),
            width: screen.0 - DIST_TO_SCREEN_EDGE * 2,
            height: HEIGHT_WORKSPACE_SELECTION_LABEL_WINDOW,
        });

        // Command-line window
        let command_line_window_height = DEFAULT_CHAR_HEIGHT + COMMAND_LINE_WINDOW_Y_PADDING * 2;
        let command_line_window = CommandLineWindow::new(
            RectData {
                top_left: Vertex::new(
                    DIST_TO_SCREEN_EDGE,
                    screen.1 - DIST_TO_SCREEN_EDGE - command_line_window_height,
                ),
                width: screen.0 - DIST_TO_SCREEN_EDGE * 2,
                height: command_line_window_height,
            },
            None,
        );

        let time = systime();

        (
            Self {
                workspaces: Vec::new(),
                current_workspace: 0,
                workspace_selection_window,
                command_line_window,
                receivers,
                is_dirty: true,
                on_loop_iter_fns: Vec::new(),
                last_frame_time: time,
                start_time: time,
                frames: 0,
                mouse_state: MouseState::new(),
                keyboard_decoder: KeyboardDecoder::new(),
            },
            senders,
        )
    }

    fn init(&mut self, senders: Senders) {
        unsafe {
            API.call_once(|| Mutex::new(Api::new(senders)));
        }

        self.create_new_workspace();
    }

    fn run(&mut self) {
        loop {
            self.draw();

            self.flush();

            self.mouse_state.draw_cursor();

            // debug!("loop");

            self.process_keyboard_input();

            self.process_mouse_input();

            self.add_new_components_from_api();

            self.add_new_closures_from_api();

            self.call_on_loop_iter_fns();

            self.process_messages();
        }
    }

    fn call_on_loop_iter_fns(&mut self) {
        for NewLoopIterFnData {
            window_data: _,
            fun,
        } in self.on_loop_iter_fns.iter()
        {
            (*fun)();
        }
    }

    fn add_new_closures_from_api(&mut self) {
        while let Ok(data) = self.receivers.rx_on_loop_iter.try_dequeue() {
            self.on_loop_iter_fns.push(data);
        }
    }

    /// Processes all WindowManagerMessages that have been enqueued by the API
    fn process_messages(&mut self) {
        while let Ok(msg) = self.receivers.rx_messages.try_dequeue() {
            match msg {
                WindowManagerMessage::CreateNewWorkspace => {
                    self.create_new_workspace();
                }
                WindowManagerMessage::CloseCurrentWorkspace => {
                    self.remove_current_workspace();
                }
                WindowManagerMessage::SwitchToWorkspace(id) => {
                    if let Some(index) = self
                        .workspaces
                        .iter()
                        .position(|workspace| workspace.get_id() == id)
                    {
                        self.switch_workspace(index);
                    }
                }
                WindowManagerMessage::CloseCurrentWindow => {
                    let was_closed = self.get_current_workspace_mut().close_focused_window();
                    if was_closed {
                        self.is_dirty = true;
                    }
                }
                WindowManagerMessage::MoveCurrentWindowForward => {
                    self.get_current_workspace_mut()
                        .move_focused_window_forward();
                }
                WindowManagerMessage::MoveCurrentWindowBackward => {
                    self.get_current_workspace_mut()
                        .move_focused_window_backward();
                }
                WindowManagerMessage::LaunchApp(app_name, split_type) => {
                    if Self::get_api().is_app_name_valid(&app_name) {
                        self.split_window(
                            self.get_current_workspace().focused_window_id,
                            split_type,
                            &app_name,
                        );
                    }
                }
            }
        }
    }

    fn process_keyboard_input(&mut self) {
        let read_option = self.keyboard_decoder.read_decoded();

        if let Some(keyboard_press) = read_option {
            // `enter_app_mode` overrides all other keyboard-interactions
            if self.command_line_window.is_active() {
                let keep_enter_app_mode = self
                    .command_line_window
                    .process_keyboard_input(keyboard_press);

                if !keep_enter_app_mode {
                    self.is_dirty = true;
                }

                return;
            }

            // Pass the input to the focused component
            let block_interact = self
                .get_current_workspace_mut()
                .get_focused_window_mut()
                .interact_with_focused_component(Interaction::Keyboard(keyboard_press));

            if block_interact {
                return;
            }

            match keyboard_press {
                DecodedKey::RawKey(HKEY_TOGGLE_TERMINAL_WINDOW) => {
                    self.enter_text_mode();
                }
                DecodedKey::Unicode('c') => {
                    self.create_new_workspace();
                }
                DecodedKey::Unicode('x') => {
                    self.remove_current_workspace();
                }
                DecodedKey::Unicode('q') => {
                    self.switch_prev_workspace();
                }
                DecodedKey::Unicode('e') => {
                    self.switch_next_workspace();
                }
                DecodedKey::Unicode('o') => {
                    self.get_current_workspace_mut()
                        .move_focused_window_forward();
                }
                DecodedKey::Unicode('i') => {
                    self.get_current_workspace_mut()
                        .move_focused_window_backward();
                }
                DecodedKey::Unicode('h') => {
                    self.command_line_window
                        .activate_enter_app_mode(ScreenSplitType::Horizontal);
                }
                DecodedKey::Unicode('v') => {
                    self.command_line_window
                        .activate_enter_app_mode(ScreenSplitType::Vertical);
                }
                DecodedKey::Unicode('w') => {
                    self.get_current_workspace_mut().focus_next_component();
                }
                DecodedKey::Unicode('s') => {
                    self.get_current_workspace_mut().focus_prev_component();
                }
                DecodedKey::Unicode('a') => {
                    self.get_current_workspace_mut().focus_prev_window();
                }
                DecodedKey::Unicode('d') => {
                    self.get_current_workspace_mut().focus_next_window();
                }
                DecodedKey::Unicode('m') => {
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

    fn process_mouse_input(&mut self) {
        let mouse_packet = try_read_mouse();
        if let Some(mouse_packet) = mouse_packet {
            let mouse_event = self.mouse_state.process(&mouse_packet);

            // Ask the workspace manager first
            if let Some(callback) = self
                .workspace_selection_window
                .handle_mouse_event(&mouse_event)
            {
                callback();
                return;
            }

            // Focus component under the cursor
            let cursor_pos = self.mouse_state.position();
            self.get_current_workspace_mut().focus_window_at(cursor_pos);
            self.get_current_workspace_mut()
                .focus_component_at(cursor_pos);

            // Pass mouse event to focused component
            self.get_current_workspace_mut()
                .get_focused_window_mut()
                .interact_with_focused_component(Interaction::Mouse(mouse_event));
        }
    }

    /// Adds all components to the window that have been queued by the api
    /// since the last frame.
    fn add_new_components_from_api(&mut self) {
        while let Ok(NewCompData {
            window_data:
                WindowData {
                    workspace_index,
                    window_id,
                },
            parent,
            component,
        }) = self.receivers.rx_components.try_dequeue()
        {
            if workspace_index >= self.workspaces.len() {
                continue;
            }

            let curr_ws = &mut self.workspaces[workspace_index];
            let window = &mut curr_ws.windows.get_mut(&window_id);
            if let Some(window) = window {
                window.insert_component(component, parent);
            }
        }
    }

    fn add_window_to_workspace(&mut self, rect_data: RectData, app_name: &str) {
        let window = AppWindow::new(rect_data);
        let window_id = window.get_id();
        let root_container = window.root_container();

        let curr_ws = self.get_current_workspace_mut();

        curr_ws.insert_window(window, curr_ws.focused_window_id);

        self.is_dirty = true;

        Self::get_api()
            .register(
                self.current_workspace,
                window_id,
                rect_data,
                app_name,
                root_container,
            )
            .expect("Failed to create window!");
    }

    /// Opens a new app window in split mode.
    fn split_window(&mut self, window_id: usize, split_type: ScreenSplitType, app_name: &str) {
        let curr_ws = self.get_current_workspace_mut();

        if let Some(window) = curr_ws.windows.get_mut(&window_id) {
            let _old_rect @ RectData {
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
                    window.rescale_window_in_place(window.rect_data.clone());
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
                    window.rescale_window_in_place(window.rect_data.clone());
                    self.add_window_to_workspace(new_rect_data, app_name);
                }
            }
        }
    }

    fn create_new_workspace(&mut self) {
        if self.workspaces.len() == 9 {
            return;
        }

        let new_workspace_index = self.workspaces.len();

        // Calculate the rect for the workspace
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

        // Create a new window for the workspace
        let window = AppWindow::new(window_rect_data);
        let window_id = window.get_id();
        let root_container = window.root_container();

        // Create the workspace
        let workspace = Workspace::new_with_single_window((window_id, window), window_id);

        self.workspace_selection_window
            .register_workspace(workspace.get_id());
        self.workspaces.insert(new_workspace_index, workspace);

        Self::get_api()
            .register(
                new_workspace_index,
                window_id,
                window_rect_data,
                DEFAULT_APP,
                root_container,
            )
            .expect("Failed to launch default app!");

        self.switch_workspace(new_workspace_index);
        self.workspace_selection_window.mark_dirty();
    }

    /// Removes the currently active workspace and switches to the previous workspace.
    /// This will only work if more than one workspace is available.
    fn remove_current_workspace(&mut self) {
        // Keep at least one workspace
        if self.workspaces.len() <= 1 {
            return;
        }

        let current_workspace_index = self.current_workspace;

        // Remove workspace button
        self.workspace_selection_window
            .unregister_workspace(self.get_current_workspace().get_id());
        self.workspace_selection_window.mark_dirty();

        // Remove workspace
        self.workspaces.remove(current_workspace_index);

        // Update workspace indices
        // TODO: Don't do this! Use the workspace id instead.
        self.on_loop_iter_fns
            .retain_mut(|fun| fun.window_data.workspace_index != current_workspace_index);
        self.on_loop_iter_fns
            .iter_mut()
            .filter(|fun| fun.window_data.workspace_index > current_workspace_index)
            .for_each(|fun| fun.window_data.workspace_index -= 1);

        Self::get_api().remove_all_handles_tied_to_workspace(current_workspace_index);

        // Switch to the previous workspace
        self.switch_prev_workspace();
        self.is_dirty = true;
    }

    fn switch_workspace(&mut self, workspace_index: usize) {
        if self.current_workspace != workspace_index {
            self.current_workspace = workspace_index;
            self.is_dirty = true;
            self.workspace_selection_window.mark_dirty();
        }
    }

    fn switch_prev_workspace(&mut self) {
        let prev_workspace =
            (self.current_workspace + self.workspaces.len() - 1) % self.workspaces.len();
        self.switch_workspace(prev_workspace);
    }

    fn switch_next_workspace(&mut self) {
        let next_workspace = (self.current_workspace + 1) % self.workspaces.len();
        self.switch_workspace(next_workspace);
    }

    fn get_current_workspace(&self) -> &Workspace {
        &self.workspaces[self.current_workspace]
    }

    fn get_current_workspace_mut(&mut self) -> &mut Workspace {
        &mut self.workspaces[self.current_workspace]
    }

    fn draw(&mut self) {
        // In enter_app_mode, we freeze everything else and only redraw the command-line-window
        if self.command_line_window.is_active() {
            self.command_line_window.draw();
            return;
        }

        let is_dirty: bool = self.is_dirty;

        if is_dirty {
            Drawer::full_clear_screen(false);
            self.workspace_selection_window.mark_dirty();
        }

        let focused_window_id = self.get_current_workspace().focused_window_id;
        let curr_ws = self.get_current_workspace_mut();

        // Redraw all unfocused workspace windows
        for window in curr_ws
            .windows
            .values_mut()
            .filter(|w| w.get_id() != focused_window_id)
        {
            window.draw(focused_window_id, is_dirty);
        }

        // Draw the focused window
        if let Some(window) = curr_ws.windows.get_mut(&focused_window_id) {
            window.draw(focused_window_id, is_dirty);
        }

        // Draw workspace selection window last
        let current_workspace_id = curr_ws.get_id();
        self.workspace_selection_window
            .draw(Some(current_workspace_id));

        self.is_dirty = false;
    }

    fn fps(&self) -> i64 {
        let time = systime();
        let elapsed = time
            .checked_sub(&self.start_time)
            .unwrap()
            .num_milliseconds()
            / 1000;

        if elapsed > 0 {
            self.frames / elapsed
        } else {
            0
        }
    }

    /// Returns, whether it is time to flush the screen.
    /// This depends on the target frame time specified in `FLUSHING_DELAY_MS`.
    fn should_flush(&self) -> bool {
        let time = systime();
        let elapsed = time
            .checked_sub(&self.last_frame_time)
            .unwrap()
            .num_milliseconds() as u32;

        elapsed >= FLUSHING_DELAY_MS
    }

    /// Flushes the changes from the backbuffer and displays them on the front buffer,
    /// if allowed by `should_flush()`.
    fn flush(&mut self) {
        if self.should_flush() {
            Drawer::flush();
            self.last_frame_time = systime();
            self.frames = self
                .frames
                .checked_add(1)
                .or_else(|| {
                    // Reset frames counter if it overflows also requires to reset start_time
                    self.start_time = self.last_frame_time;
                    Some(0)
                })
                .unwrap();
        }
    }

    fn enter_text_mode(&mut self) {
        Drawer::full_clear_screen(true);
        process::exit();
    }
}

#[cfg(feature = "with_runtime")]
#[no_mangle]
fn main() {
    let resolution = Drawer::get_graphic_resolution();
    let (mut window_manager, senders) = WindowManager::new(resolution);
    window_manager.init(senders);
    window_manager.run();
}
