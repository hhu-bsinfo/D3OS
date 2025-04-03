use drawer::{drawer::Drawer, rect_data::RectData, vertex::Vertex};
use alloc::{format, vec};
use input::mouse::MousePacket;
use terminal::write::log_debug;

use crate::config::DEFAULT_FG_COLOR;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

// None -> Pressed -> Down -> Released -> None
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButtonState {
    None,
    Pressed,
    Down,
    Released,
}

// Events that will be sent to components
pub struct MouseEvent {
    pub button_states: [MouseButtonState; 3],
    pub position: (u32, u32),
}

pub struct MouseState {
    position: (u32, u32),
    last_position: (u32, u32),

    button_states: [MouseButtonState; 3],
}

impl MouseState {
    pub fn new() -> Self {
        Self {
            position: (0, 0),
            last_position: (0, 0),

            button_states: [MouseButtonState::None; 3],
        }
    }

    pub fn process(&mut self, mouse_packet: &MousePacket) -> MouseEvent {
        // Update position
        self.update_position(mouse_packet.dx as i32, mouse_packet.dy as i32);

        // Update button states
        self.update_button_state(MouseButton::Left, mouse_packet.left_button_down());
        self.update_button_state(MouseButton::Right, mouse_packet.right_button_down());

        // Print button states
        log_debug(&format!(
            "Mouse button states: Left: {:?}, Right: {:?}",
            self.button_states[MouseButton::Left as usize],
            self.button_states[MouseButton::Right as usize]
        ));

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

    fn update_button_state(&mut self, button: MouseButton, is_down: bool) {
        let button_idx = button as usize;
        let current_state = self.button_states[button_idx];
        
        self.button_states[button_idx] = match (current_state, is_down) {
            // Button is pressed (None -> Pressed -> Down)
            (MouseButtonState::None, true) => MouseButtonState::Pressed,
            (MouseButtonState::Pressed, true) => MouseButtonState::Down,
            (MouseButtonState::Released, true) => MouseButtonState::Pressed,
            
            // Button is released (Down -> Released -> None)
            (MouseButtonState::Pressed, false) => MouseButtonState::Released,
            (MouseButtonState::Down, false) => MouseButtonState::Released,
            (MouseButtonState::Released, false) => MouseButtonState::None,
            
            // Maintain current state in other cases
            (state, _) => state,
        };
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