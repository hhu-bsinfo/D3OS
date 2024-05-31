use crate::components::component::Component;
use crate::window::Window;
use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use hashbrown::HashMap;

pub struct Workspace {
    pub components: HashMap<usize, Box<dyn Component>>,
    pub focused_window_id: Option<usize>,
    pub window_orderer: Vec<usize>,
}

impl Workspace {
    pub fn new_with_single_window(
        window: (usize, Box<Window>),
        focused_window_id: Option<usize>,
    ) -> Self {
        let window_orderer = vec![window.0];
        let mut windows: HashMap<usize, Box<dyn Component>> = HashMap::new();
        windows.insert(window.0, window.1);

        Self {
            components: windows,
            focused_window_id,
            window_orderer,
        }
    }

    pub fn insert_focusable_window(&mut self, window: Box<Window>, after: Option<usize>) {
        let new_window_id = window.id;
        self.components.insert(new_window_id, window);
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

    pub fn insert_unfocusable_window(&mut self, window: Box<Window>) {
        let new_window_id = window.id;
        self.components.insert(new_window_id, window);
    }

    pub fn focus_next_window(&mut self) {
        if let Some(focused_window_id) = self.focused_window_id {
            let index = self
                .window_orderer
                .iter()
                .position(|id| *id == focused_window_id)
                .unwrap();
            let next_index = (index + 1) % self.window_orderer.len();
            self.focused_window_id = Some(self.window_orderer[next_index]);
        }
    }

    pub fn focus_prev_window(&mut self) {
        if let Some(focused_window_id) = self.focused_window_id {
            let index = self
                .window_orderer
                .iter()
                .position(|id| *id == focused_window_id)
                .unwrap();
            let prev_index = if index == 0 {
                self.window_orderer.len() - 1
            } else {
                index - 1
            };

            self.focused_window_id = Some(self.window_orderer[prev_index]);
        }
    }

    pub fn insert_component(&mut self, component: Box<dyn Component>) {
        self.components.insert(component.id(), component);
    }
}
