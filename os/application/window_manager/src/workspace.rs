use alloc::vec;
use alloc::vec::Vec;
use hashbrown::HashMap;

use crate::components::component::Interaction;
use crate::windows::app_window::AppWindow;

/**
A workspace is a unit of one screen, containing windows. You can switch between workspaces
and they will retain their state and even continue execution of their threads, but not draw
anything to the screen while not selected of course.
*/
pub struct Workspace {
    pub windows: HashMap<usize, AppWindow>,
    pub focused_window_id: usize,
    // Windows are stored additionally in ordered fashion in here
    pub window_orderer: Vec<usize>,
}

impl Workspace {
    pub fn new_with_single_window(window: (usize, AppWindow), focused_window_id: usize) -> Self {
        let window_orderer = vec![window.0];
        let mut windows: HashMap<usize, AppWindow> = HashMap::new();
        windows.insert(window.0, window.1);

        Self {
            windows,
            focused_window_id,
            window_orderer,
        }
    }

    pub fn insert_window(&mut self, window: AppWindow, after: Option<usize>) {
        let new_window_id = window.id;
        self.windows.insert(new_window_id, window);
        match after {
            Some(after_window_id) => {
                if let Some(index) = self
                    .window_orderer
                    .iter()
                    .position(|x| *x == after_window_id)
                {
                    if index == self.window_orderer.len() - 1 {
                        self.window_orderer.push(new_window_id);
                        return;
                    }
                    self.window_orderer.insert(index + 1, new_window_id);
                }
            }
            None => self.window_orderer.push(new_window_id),
        }
    }

    pub fn focus_next_window(&mut self) {
        self.get_focused_window_mut().is_dirty = true;

        let index = self
            .window_orderer
            .iter()
            .position(|id| *id == self.focused_window_id)
            .unwrap();
        let next_index = (index + 1) % self.window_orderer.len();
        self.focused_window_id = self.window_orderer[next_index];

        self.get_focused_window_mut().is_dirty = true;
    }

    pub fn focus_prev_window(&mut self) {
        self.get_focused_window_mut().is_dirty = true;

        let index = self
            .window_orderer
            .iter()
            .position(|id| *id == self.focused_window_id)
            .unwrap();
        let prev_index = if index == 0 {
            self.window_orderer.len() - 1
        } else {
            index - 1
        };

        self.focused_window_id = self.window_orderer[prev_index];

        self.get_focused_window_mut().is_dirty = true;
    }

    /// Moves focus to the next component in currently focused window
    pub fn focus_next_component(&mut self) {
        let focused_window = self.get_focused_window_mut();
        focused_window.focus_next_component();
    }

    /// Moves focus to the previous component in currently focused window
    pub fn focus_prev_component(&mut self) {
        let focused_window = self.get_focused_window_mut();
        focused_window.focus_prev_component();
    }

    pub fn interact_with_focused_component(&mut self, interaction: Interaction) {
        let focused_window = self.get_focused_window_mut();
        focused_window.interact_with_focused_component(interaction);
    }

    pub fn get_focused_window(&self) -> &AppWindow {
        self.windows.get(&self.focused_window_id).unwrap()
    }

    pub fn get_focused_window_mut(&mut self) -> &mut AppWindow {
        self.windows.get_mut(&self.focused_window_id).unwrap()
    }
}
