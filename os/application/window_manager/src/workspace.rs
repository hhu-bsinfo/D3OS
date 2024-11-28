use alloc::collections::LinkedList;
use hashbrown::HashMap;

use crate::utils::get_element_cursor_from_orderer;
use crate::window_tree::WindowNode;
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
    /**
    Proxy-structure to determine buddies to be merged together
    */
    buddy_tree_root: WindowNode,
}

impl Workspace {
    pub fn new_with_single_window(window: (usize, AppWindow), focused_window_id: usize) -> Self {
        let mut window_orderer = LinkedList::new();
        window_orderer.push_back(window.0);

        let mut windows: HashMap<usize, AppWindow> = HashMap::new();
        windows.insert(window.0, window.1);

        let buddy_tree_root = WindowNode::new_leaf(window.0);

        Self {
            windows,
            focused_window_id,
            window_orderer,
            buddy_tree_root,
        }
    }

    pub fn insert_window(&mut self, window: AppWindow, after: usize) {
        let new_window_id = window.id;
        self.windows.insert(new_window_id, window);

        self.buddy_tree_root.insert_value(after, new_window_id);

        let mut cursor = get_element_cursor_from_orderer(&mut self.window_orderer, after).unwrap();
        cursor.move_next();
        cursor.insert_before(new_window_id);
    }

    pub fn focus_next_window(&mut self) {
        self.get_focused_window_mut().mark_window_dirty();
        let mut cursor =
            get_element_cursor_from_orderer(&mut self.window_orderer, self.focused_window_id)
                .unwrap();

        cursor.move_next();

        self.focused_window_id = match cursor.current() {
            Some(next_focused_el) => next_focused_el.clone(),
            None => {
                cursor.move_next();
                cursor.current().unwrap().clone()
            }
        };

        self.get_focused_window_mut().mark_window_dirty();
    }

    pub fn focus_prev_window(&mut self) {
        self.get_focused_window_mut().mark_window_dirty();

        let mut cursor =
            get_element_cursor_from_orderer(&mut self.window_orderer, self.focused_window_id)
                .unwrap();

        cursor.move_prev();

        self.focused_window_id = match cursor.current() {
            Some(next_focused_el) => next_focused_el.clone(),
            None => {
                cursor.move_prev();
                cursor.current().unwrap().clone()
            }
        };

        self.get_focused_window_mut().mark_window_dirty();
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

    pub fn close_focused_window(&mut self) -> bool {
        if self.windows.len() == 1 {
            return false;
        }

        let buddy_id = self.buddy_tree_root.get_sibling(self.focused_window_id);
        match buddy_id {
            Some(buddy_id) => {
                let [to_be_deleted_window, buddy_window] = self
                    .windows
                    .get_many_mut([&self.focused_window_id, &buddy_id])
                    .unwrap();

                buddy_window.merge(&to_be_deleted_window);

                self.windows.remove(&self.focused_window_id);
                self.buddy_tree_root.remove_leaf(self.focused_window_id);
            }
            None => {
                return false;
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

        return true;
    }

    pub fn move_focused_window_forward(&mut self) {
        if self.windows.len() == 1 {
            return;
        }

        let focused_rect_data = {
            let focused_window = self.get_focused_window_mut();
            let focused_rect_data = focused_window.rect_data.clone();
            focused_window.mark_window_dirty();

            focused_rect_data
        };

        let swapped_rect_data = {
            let mut cursor =
                get_element_cursor_from_orderer(&mut self.window_orderer, self.focused_window_id)
                    .unwrap();

            let focused = cursor.remove_current().unwrap();

            match cursor.current() {
                Some(_) => cursor.insert_after(focused),
                None => {
                    cursor.move_next();
                    cursor.insert_after(focused);
                }
            }

            let swapped_id = cursor.current().unwrap().clone();

            let swapped_window = self.windows.get_mut(&swapped_id).unwrap();
            let swapped_rect_data: drawer::rect_data::RectData = swapped_window.rect_data.clone();
            swapped_window.mark_window_dirty();
            swapped_window.rescale_window_after_move(focused_rect_data);

            self.buddy_tree_root
                .swap_values(self.focused_window_id, swapped_id);

            swapped_rect_data
        };

        let focused_window = self.get_focused_window_mut();
        focused_window.rect_data = swapped_rect_data;
        focused_window.rescale_window_after_move(swapped_rect_data);
    }

    pub fn move_focused_window_backward(&mut self) {
        if self.windows.len() == 1 {
            return;
        }

        let focused_rect_data = {
            let focused_window = self.get_focused_window_mut();
            let focused_rect_data = focused_window.rect_data.clone();
            focused_window.mark_window_dirty();

            focused_rect_data
        };

        let swapped_rect_data = {
            let mut cursor =
                get_element_cursor_from_orderer(&mut self.window_orderer, self.focused_window_id)
                    .unwrap();

            let focused = cursor.remove_current().unwrap();
            cursor.move_prev();

            match cursor.current() {
                Some(_) => cursor.insert_before(focused),
                None => {
                    cursor.move_prev();
                    cursor.insert_before(focused);
                }
            }

            let swapped_id = cursor.current().unwrap().clone();

            let swapped_window = self.windows.get_mut(&swapped_id).unwrap();
            let swapped_rect_data = swapped_window.rect_data.clone();
            swapped_window.mark_window_dirty();
            swapped_window.rescale_window_after_move(focused_rect_data);

            self.buddy_tree_root
                .swap_values(self.focused_window_id, swapped_id);

            swapped_rect_data
        };

        let focused_window = self.get_focused_window_mut();
        focused_window.rect_data = swapped_rect_data;
        focused_window.rescale_window_after_move(swapped_rect_data);
    }

    #[allow(dead_code)]
    pub fn get_focused_window(&self) -> &AppWindow {
        self.windows.get(&self.focused_window_id).unwrap()
    }

    pub fn get_focused_window_mut(&mut self) -> &mut AppWindow {
        self.windows.get_mut(&self.focused_window_id).unwrap()
    }
}
