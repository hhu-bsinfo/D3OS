use alloc::{string::String, vec::Vec};
use terminal::{DecodedKey, print};

use crate::context::{Context, VisualType};

use super::service::{Service, ServiceError};

pub struct DrawerService {
    last_cursor_pos: usize,
}

impl Service for DrawerService {
    fn run(&mut self, _event: DecodedKey, context: &mut Context) -> Result<(), ServiceError> {
        if !context.visual_line.is_dirty() && !context.cursor_position.is_dirty() {
            return Ok(());
        }

        self.draw_cursor_start_of_line();
        self.draw_clear_line();
        self.draw_visual_line(context);
        self.draw_restore_cursor(context);

        context.visual_line.cleanup();
        self.last_cursor_pos = *context.cursor_position.get();

        Ok(())
    }
}

impl DrawerService {
    pub const fn new() -> Self {
        Self { last_cursor_pos: 0 }
    }

    fn draw_clear_line(&self) {
        print!("\x1b[2K");
    }

    fn draw_cursor_start_of_line(&self) {
        if self.last_cursor_pos == 0 {
            return;
        }
        print!("\x1b[{}D", self.last_cursor_pos);
    }

    fn draw_visual_line(&mut self, context: &mut Context) {
        let string = self.visual_line_to_string(context.visual_line.get());
        print!("{}", string);
    }

    fn draw_restore_cursor(&self, context: &mut Context) {
        let current_pos = context.visual_line.get().len();
        let target_pos = *context.cursor_position.get();
        let offset = current_pos - target_pos;

        if offset == 0 {
            return;
        }

        print!("\x1b[{}D", offset);
    }

    fn visual_line_to_string(&self, visual_line: &Vec<VisualType>) -> String {
        visual_line
            .iter()
            .map(|vt| match vt {
                VisualType::Indicator(ch) => *ch,
                VisualType::Default(ch) => *ch,
                VisualType::AutoCompleteHint(ch) => *ch,
            })
            .collect()
    }
}
