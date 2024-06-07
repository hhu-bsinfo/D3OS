use crate::components::window::Window;
use alloc::vec;
use alloc::vec::Vec;
use hashbrown::HashMap;

pub struct Workspace {
    pub windows: HashMap<usize, Window>,
    pub focused_window_id: usize,
    // focusable windows are stored additionally in ordered fashion in here
    pub window_orderer: Vec<usize>,
}

impl Workspace {
    pub fn new_with_single_window(window: (usize, Window), focused_window_id: usize) -> Self {
        let window_orderer = vec![window.0];
        let mut windows: HashMap<usize, Window> = HashMap::new();
        windows.insert(window.0, window.1);

        Self {
            windows,
            focused_window_id,
            window_orderer,
        }
    }

    pub fn insert_focusable_window(&mut self, window: Window, after: Option<usize>) {
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
        let index = self
            .window_orderer
            .iter()
            .position(|id| *id == self.focused_window_id)
            .unwrap();
        let next_index = (index + 1) % self.window_orderer.len();
        self.focused_window_id = self.window_orderer[next_index];
    }

    pub fn focus_prev_window(&mut self) {
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
    }

    pub fn insert_unfocusable_window(&mut self, new_window: Window) {
        self.windows.insert(new_window.id, new_window);
    }

    /// Moves focus to the next component in currently focused window
    pub fn focus_next_component(&mut self) {
        let focused_window = self.windows.get_mut(&self.focused_window_id).unwrap();
        focused_window.focus_next_component();
    }

    /// Moves focus to the previous component in currently focused window
    pub fn focus_prev_component(&mut self) {
        let focused_window = self.windows.get_mut(&self.focused_window_id).unwrap();
        focused_window.focus_prev_component();
    }

    pub fn get_focused_window(&self) -> &Window {
        self.windows.get(&self.focused_window_id).unwrap()
    }

    pub fn get_focused_window_mut(&mut self) -> &mut Window {
        self.windows.get_mut(&self.focused_window_id).unwrap()
    }
}
