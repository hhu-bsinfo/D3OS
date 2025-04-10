/*
    Processes mouse packets into mouse events and
    manages the mouse state.
*/

use drawer::drawer::Drawer;
use alloc::{format, vec};
use input::mouse::MousePacket;
use terminal::write::log_debug;

use crate::config::DEFAULT_FG_COLOR;

pub use drawer::vertex::Vertex;

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
    pub button4: ButtonState,
    pub button5: ButtonState,
}

impl MouseButtonState {
    pub fn new() -> Self {
        Self {
            left: ButtonState::None,
            right: ButtonState::None,
            middle: ButtonState::None,
            button4: ButtonState::None,
            button5: ButtonState::None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollDirection {
    None,
    Up,
    Down,
    Left,
    Right,
}

// Events that will be sent to components
pub struct MouseEvent {
    pub buttons: MouseButtonState,
    pub position: Vertex,
    pub scroll: ScrollDirection,
}

pub struct MouseState {
    position: Vertex,
    last_position: Vertex,

    buttons: MouseButtonState,
}

impl MouseState {
    pub fn new() -> Self {
        Self {
            position: Vertex::new(0, 0),
            last_position: Vertex::new(0, 0),

            buttons: MouseButtonState::new(),
        }
    }

    pub fn process(&mut self, mouse_packet: &MousePacket) -> MouseEvent {
        self.position = self.position.add_signed(mouse_packet.dx as i32, -mouse_packet.dy as i32);

        // Update button states
        self.buttons = MouseButtonState {
            left: self.buttons.left.next_state(mouse_packet.left_button_down()),
            right: self.buttons.right.next_state(mouse_packet.right_button_down()),
            middle: self.buttons.middle.next_state(mouse_packet.middle_button_down()),
            button4: self.buttons.button4.next_state(mouse_packet.button4_down()),
            button5: self.buttons.button5.next_state(mouse_packet.button5_down()),
        };

        // Horizontal scrolling sends -2 or 2 for left/right
        let scroll_direction = match mouse_packet.dz {
            -1 => ScrollDirection::Up,
            1 => ScrollDirection::Down,
            -2 => ScrollDirection::Right,
            2 => ScrollDirection::Left,
            _ => ScrollDirection::None,
        };

        // Print button states
        /*log_debug(&format!(
            "Scroll: {:?}, Button 4: {}, Button 5: {}",
            scroll_direction, mouse_packet.button4_down(), mouse_packet.button5_down()
        ));*/

        MouseEvent {
            buttons: self.buttons,
            position: self.position,
            scroll: scroll_direction,
        }
    }

    pub fn position(&self) -> Vertex {
        self.position
    }

    pub fn draw_cursor(&mut self) {
        Drawer::flush_lines(self.last_position.y, 11);
            
        Drawer::draw_polygon_direct(
            vec![
                Vertex::new(self.position.x, self.position.y),
                Vertex::new(self.position.x + 10, self.position.y + 4),
                Vertex::new(self.position.x + 4, self.position.y + 10),
            ],
            DEFAULT_FG_COLOR,
        );
            
        self.last_position = self.position;
    }
}