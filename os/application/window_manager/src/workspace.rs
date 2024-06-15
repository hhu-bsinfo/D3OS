use alloc::collections::LinkedList;
use hashbrown::HashMap;

use crate::components::component::Interaction;
use crate::utils::get_element_cursor_from_orderer;
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
    pub window_orderer: LinkedList<usize>,
}

impl Workspace {
    pub fn new_with_single_window(window: (usize, AppWindow), focused_window_id: usize) -> Self {
        let mut window_orderer = LinkedList::new();
        window_orderer.push_back(window.0);

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
                let mut cursor =
                    get_element_cursor_from_orderer(&mut self.window_orderer, after_window_id)
                        .unwrap();
                cursor.move_next();
                cursor.insert_before(new_window_id);
            }
            None => self.window_orderer.push_back(new_window_id),
        }
    }

    pub fn focus_next_window(&mut self) {
        self.get_focused_window_mut().is_dirty = true;

        let mut cursor =
            get_element_cursor_from_orderer(&mut self.window_orderer, self.focused_window_id)
                .unwrap();
        cursor.move_next();

        self.focused_window_id = match cursor.current() {
            Some(next_focused_el) => next_focused_el.clone(),
            None => cursor.peek_next().unwrap().clone(),
        };

        self.get_focused_window_mut().is_dirty = true;
    }

    pub fn focus_prev_window(&mut self) {
        self.get_focused_window_mut().is_dirty = true;

        let mut cursor =
            get_element_cursor_from_orderer(&mut self.window_orderer, self.focused_window_id)
                .unwrap();
        cursor.move_prev();

        self.focused_window_id = match cursor.current() {
            Some(next_focused_el) => next_focused_el.clone(),
            None => cursor.peek_prev().unwrap().clone(),
        };

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

    pub fn close_focused_window(&mut self) {
        let to_be_deleted_window = self.windows.remove(&self.focused_window_id).unwrap();

        if let Some(buddy_id) = to_be_deleted_window.buddy_window_id {
            let buddy_window = self.windows.get_mut(&buddy_id).unwrap();
            if buddy_window.is_elligible_for_merging(&to_be_deleted_window) {
                buddy_window.merge(&to_be_deleted_window)
            }
        }

        let mut cursor =
            get_element_cursor_from_orderer(&mut self.window_orderer, self.focused_window_id)
                .unwrap();
        cursor.move_next();

        let new_focused_window_id = match cursor.current() {
            Some(next_focused_el) => next_focused_el.clone(),
            None => cursor.peek_next().unwrap().clone(),
        };

        cursor.move_prev();
        cursor.remove_current();

        self.focused_window_id = new_focused_window_id;
    }

    pub fn get_focused_window(&self) -> &AppWindow {
        self.windows.get(&self.focused_window_id).unwrap()
    }

    pub fn get_focused_window_mut(&mut self) -> &mut AppWindow {
        self.windows.get_mut(&self.focused_window_id).unwrap()
    }
}
