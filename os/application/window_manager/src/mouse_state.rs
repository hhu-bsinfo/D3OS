use drawer::{drawer::Drawer, vertex::Vertex};
use alloc::{format, vec};
use input::mouse::MousePacket;
use terminal::write::log_debug;

use crate::config::DEFAULT_FG_COLOR;

// None -> Pressed -> Down -> Released -> None
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonState {
    None,
    Pressed,
    Down,
    Released,
}

impl ButtonState {
    fn next_state(&self, is_down: bool) -> ButtonState {
        match (*self, is_down) {
            // Button is pressed (None/Released -> Pressed -> Down/Released)
            (ButtonState::None, true) => ButtonState::Pressed,
            (ButtonState::Pressed, true) => ButtonState::Down,
            (ButtonState::Released, true) => ButtonState::Pressed,
            
            // Button is released (Down/Pressed -> Released -> None/Presed)
            (ButtonState::Pressed, false) => ButtonState::Released,
            (ButtonState::Down, false) => ButtonState::Released,
            (ButtonState::Released, false) => ButtonState::None,
            
            // Maintain current state in other cases
            (state, _) => state,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MouseButtonState {
    pub left: ButtonState,
    pub right: ButtonState,
    pub middle: ButtonState,
}

impl MouseButtonState {
    pub fn new() -> Self {
        Self {
            left: ButtonState::None,
            right: ButtonState::None,
            middle: ButtonState::None,
        }
    }
}

// Events that will be sent to components
pub struct MouseEvent {
    pub button_states: MouseButtonState,
    pub position: (u32, u32),
}

pub struct MouseState {
    position: (u32, u32),
    last_position: (u32, u32),

    button_states: MouseButtonState,
}

impl MouseState {
    pub fn new() -> Self {
        Self {
            position: (0, 0),
            last_position: (0, 0),

            button_states: MouseButtonState::new(),
        }
    }

    pub fn process(&mut self, mouse_packet: &MousePacket) -> MouseEvent {
        // Update position
        self.update_position(mouse_packet.dx as i32, mouse_packet.dy as i32);

        // Update button states
        self.button_states.left = self.button_states.left.next_state(mouse_packet.left_button_down());
        self.button_states.right = self.button_states.right.next_state(mouse_packet.right_button_down());
        self.button_states.middle = self.button_states.middle.next_state(mouse_packet.middle_button_down());

        // Print button states
        /*log_debug(&format!(
            "Mouse button states: Left: {:?}, Right: {:?}, Middle: {:?}",
            self.button_states.left, self.button_states.right, self.button_states.middle
        ));*/

        // Create and return the MouseEvent
        MouseEvent {
            button_states: self.button_states,
            position: self.position,
        }
    }

    fn update_position(&mut self, dx: i32, dy: i32) {
        self.position.0 = self.position.0.saturating_add_signed(dx);
        self.position.1 = self.position.1.saturating_add_signed(-dy);
    }

    pub fn position(&self) -> (u32, u32) {
        self.position
    }

    pub fn draw_cursor(&mut self) {
        Drawer::flush_lines(self.last_position.1, 11);
            
        Drawer::draw_polygon_direct(
            vec![
                Vertex::new(self.position.0, self.position.1),
                Vertex::new(self.position.0 + 10, self.position.1 + 4),
                Vertex::new(self.position.0 + 4, self.position.1 + 10),
            ],
            DEFAULT_FG_COLOR,
        );
            
        self.last_position = self.position;
    }
}