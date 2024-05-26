use crate::window::Window;
use alloc::vec;
use alloc::vec::Vec;
use hashbrown::HashMap;

#[derive(Debug)]
pub(crate) struct Workspace {
    pub(crate) windows: HashMap<usize, Window>,
    pub(crate) focused_window_id: Option<usize>,
    pub(crate) window_orderer: Vec<usize>,
}

impl Workspace {
    pub(crate) fn new_with_single_window(
        window: (usize, Window),
        focused_window_id: Option<usize>,
    ) -> Self {
        let window_orderer = vec![window.0];
        let mut windows = HashMap::new();
        windows.insert(window.0, window.1);

        Self {
            windows,
            focused_window_id,
            window_orderer,
        }
    }

    pub(crate) fn insert_window(&mut self, window: Window, after: Option<usize>) {
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
                    }
                    self.window_orderer.insert(index + 1, new_window_id);
                }
            }
            None => self.window_orderer.push(new_window_id),
        }
    }

    pub(crate) fn focus_next_window(&mut self) {
        if let Some(focused_window_id) = self.focused_window_id {
            let index = self
                .window_orderer
                .iter()
                .position(|id| *id == focused_window_id)
                .unwrap();
            let next_index = (index + 1) % self.windows.len();
            self.focused_window_id = Some(self.window_orderer[next_index]);
        }
    }

    pub(crate) fn focus_prev_window(&mut self) {
        if let Some(focused_window_id) = self.focused_window_id {
            let index = self
                .window_orderer
                .iter()
                .position(|id| *id == focused_window_id)
                .unwrap();
            let prev_index = if index == 0 {
                self.windows.len() - 1
            } else {
                index - 1
            };

            self.focused_window_id = Some(self.window_orderer[prev_index]);
        }
    }
}
