use alloc::boxed::Box;

/// Used to determine and manage buddy-IDs of AppWindows
#[derive(Clone)]
pub enum WindowNode {
    Leaf(usize),
    Twig {
        left: Box<WindowNode>,
        right: Box<WindowNode>,
    },
}

impl WindowNode {
    pub fn new_leaf(value: usize) -> Self {
        WindowNode::Leaf(value)
    }

    pub fn new_twig(left: WindowNode, right: WindowNode) -> Self {
        WindowNode::Twig {
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    pub fn insert_value(&mut self, old_value: usize, new_value: usize) {
        match self {
            WindowNode::Leaf(value) => {
                if *value == old_value {
                    *self = WindowNode::new_twig(
                        WindowNode::new_leaf(old_value),
                        WindowNode::new_leaf(new_value),
                    );
                }
            }
            WindowNode::Twig { left, right } => {
                left.insert_value(old_value, new_value);
                right.insert_value(old_value, new_value);
            }
        }
    }

    pub fn get_sibling(&self, value: usize) -> Option<usize> {
        match self {
            WindowNode::Twig { left, right } => match (&**left, &**right) {
                (WindowNode::Leaf(left_value), WindowNode::Leaf(right_value)) => {
                    if *left_value == value {
                        return Some(*right_value);
                    } else if *right_value == value {
                        return Some(*left_value);
                    } else {
                        return None;
                    }
                }
                _ => {
                    if let Some(sibling) = left.get_sibling(value) {
                        return Some(sibling);
                    }

                    if let Some(sibling) = right.get_sibling(value) {
                        return Some(sibling);
                    }

                    return None;
                }
            },
            WindowNode::Leaf(_) => None,
        }
    }

    /// Returns whether the leaf has been successfully removed
    pub fn remove_leaf(&mut self, value: usize) -> bool {
        if let WindowNode::Twig { left, right } = self {
            if let (WindowNode::Leaf(left_value), WindowNode::Leaf(right_value)) =
                (&**left, &**right)
            {
                if *left_value == value {
                    *self = *right.clone();
                    return true;
                } else if *right_value == value {
                    *self = *left.clone();
                    return true;
                } else {
                    return false;
                }
            } else {
                let is_removed = left.remove_leaf(value);

                if !is_removed {
                    right.remove_leaf(value);
                }
            }
        }
        return false;
    }

    pub fn swap_values(&mut self, value1: usize, value2: usize) {
        let mut node1 = None;
        let mut node2 = None;
        self.find_nodes_with_values(value1, value2, &mut node1, &mut node2);

        if let (Some(n1), Some(n2)) = (node1, node2) {
            if let (WindowNode::Leaf(ref mut v1), WindowNode::Leaf(ref mut v2)) =
                (&mut *n1, &mut *n2)
            {
                core::mem::swap(v1, v2);
            }
        }
    }

    fn find_nodes_with_values<'a>(
        &'a mut self,
        value1: usize,
        value2: usize,
        node1: &mut Option<&'a mut WindowNode>,
        node2: &mut Option<&'a mut WindowNode>,
    ) {
        match self {
            WindowNode::Leaf(v) => {
                if *v == value1 {
                    *node1 = Some(self);
                } else if *v == value2 {
                    *node2 = Some(self);
                }
            }
            WindowNode::Twig { left, right } => {
                left.find_nodes_with_values(value1, value2, node1, node2);
                right.find_nodes_with_values(value1, value2, node1, node2);
            }
        }
    }
}
