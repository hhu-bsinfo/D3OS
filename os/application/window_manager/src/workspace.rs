use hashbrown::HashMap;
use crate::window::Window;

#[derive(Debug)]
pub(crate) struct Workspace {
    pub(crate) windows: HashMap<usize, Window>,
    pub(crate) focused_window_id: Option<usize>,
}

impl Workspace {
    pub(crate) fn new(windows: HashMap<usize, Window>, focused_window_id: Option<usize>) -> Self {
        Self {
            windows,
            focused_window_id,
        }
    }
}