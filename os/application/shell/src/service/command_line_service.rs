use alloc::{collections::vec_deque::VecDeque, string::String};
use terminal::{DecodedKey, KeyCode};

use crate::context::{Context, VisualType};

use super::service::{Service, ServiceError};

pub struct CommandLineService {
    history: VecDeque<String>,
    current_line: String,
    cursor_position: usize,
    history_position: isize,
}

impl Service for CommandLineService {
    fn run(&mut self, context: &mut Context) -> Result<(), ServiceError> {
        match { context.event } {
            DecodedKey::Unicode('\n') => self.on_submit(context),
            DecodedKey::Unicode('\x08') => self.on_backspace(context),
            DecodedKey::Unicode('\x7F') => self.on_del(context),
            DecodedKey::Unicode(ch) => self.on_other_char(context, ch),
            DecodedKey::RawKey(KeyCode::ArrowLeft) => self.on_arrow_left(context),
            DecodedKey::RawKey(KeyCode::ArrowRight) => self.on_arrow_right(context),
            _ => Ok(()),
        }
    }
}

impl CommandLineService {
    pub const fn new() -> Self {
        Self {
            //
            history: VecDeque::new(),
            current_line: String::new(),
            cursor_position: 0,
            history_position: -1,
        }
    }

    fn on_submit(&mut self, _context: &mut Context) -> Result<(), ServiceError> {
        Ok(())
    }

    fn on_del(&mut self, context: &mut Context) -> Result<(), ServiceError> {
        let cursor_pos = *context.cursor_position.get();
        if cursor_pos == context.line.get().len() {
            return Ok(());
        }

        let visual_index = nth_visual_index(context.visual_line.get(), cursor_pos);
        context.line.dirty_mut().remove(cursor_pos);
        context.visual_line.dirty_mut().remove(visual_index);
        Ok(())
    }

    fn on_backspace(&mut self, context: &mut Context) -> Result<(), ServiceError> {
        let cursor_pos = *context.cursor_position.get();
        if cursor_pos == 0 {
            return Ok(());
        }

        let visual_index = nth_visual_index(context.visual_line.get(), cursor_pos - 1);
        context.line.dirty_mut().remove(cursor_pos - 1);
        context.visual_line.dirty_mut().remove(visual_index);
        context.cursor_position.set(cursor_pos - 1);
        Ok(())
    }

    fn on_other_char(&mut self, context: &mut Context, ch: char) -> Result<(), ServiceError> {
        let cursor_pos = *context.cursor_position.get();
        let visual_index = nth_visual_index(context.visual_line.get(), cursor_pos);
        context.line.dirty_mut().insert(cursor_pos, ch);
        context
            .visual_line
            .dirty_mut()
            .insert(visual_index, VisualType::Default(ch));
        context.cursor_position.set(cursor_pos + 1);

        Ok(())
    }

    fn on_arrow_right(&mut self, context: &mut Context) -> Result<(), ServiceError> {
        let cursor_pos = *context.cursor_position.get();
        if cursor_pos >= context.line.get().len() {
            return Ok(());
        }

        context.cursor_position.set(cursor_pos + 1);
        Ok(())
    }

    fn on_arrow_left(&mut self, context: &mut Context) -> Result<(), ServiceError> {
        let cursor_pos = context.cursor_position.get();
        if *cursor_pos <= 0 {
            return Ok(());
        }

        context.cursor_position.set(*cursor_pos - 1);
        Ok(())
    }

    // pub fn submit(&mut self) -> String {
    //     let line = self.current_line.clone();
    //     self.history.push_front(line.clone());
    //     self.history_position = -1;
    //     match self.current_line.len() - self.cursor_position {
    //         0 => print!("\n"),
    //         x => print!("\x1b[{}C\n", x),
    //     };
    //     self.current_line.clear();
    //     self.cursor_position = 0;
    //     line
    // }

    // pub fn create_new_line(&self) {
    //     print!(
    //         "[38;2;140;177;16m{} > [0m",
    //         cwd().unwrap_or("/".to_string())
    //     );
    // }

    // TODO docs: NOT FOR '\n', '\x08'
    // pub fn add_char(&mut self, ch: char) -> Result<String, ()> {
    //     self.current_line.insert(self.cursor_position, ch);
    //     let line_since_cursor = self.current_line.get(self.cursor_position..).unwrap();
    //     match line_since_cursor.len() - 1 {
    //         0 => print!("{}", line_since_cursor), // Note: \x1b[0D will still do \x1b[1D
    //         cursor_offset => print!("{}\x1b[{}D", line_since_cursor, cursor_offset),
    //     }
    //     self.cursor_position += 1;
    //     Ok(self.current_line.clone())
    // }

    // pub fn remove_at_cursor(&mut self) -> Result<String, ()> {
    //     if self.cursor_position == self.current_line.len() {
    //         return Err(());
    //     }

    //     self.current_line.remove(self.cursor_position);
    //     let line_since_cursor = self.current_line.get(self.cursor_position..).unwrap();
    //     let cursor_offset = line_since_cursor.len() + 1; // line_since_cursor.len() + 1 (we added whitespace to remove trailing char)
    //     print!("{} \x1b[{}D", line_since_cursor, cursor_offset);
    //     Ok(self.current_line.clone())
    // }

    // pub fn remove_before_cursor(&mut self) -> Result<String, ()> {
    //     if self.cursor_position == 0 {
    //         return Err(());
    //     }

    //     self.cursor_position -= 1;
    //     self.current_line.remove(self.cursor_position);
    //     let line_since_cursor = self.current_line.get(self.cursor_position..).unwrap();
    //     let cursor_offset = line_since_cursor.len() + 1; // line_since_cursor.len() - 1 + (1 + 1) (we moved cursor already & we added whitespace to remove trailing char)
    //     print!("\x1b[1D{} \x1b[{}D", line_since_cursor, cursor_offset);
    //     Ok(self.current_line.clone())
    // }

    // pub fn move_cursor_left(&mut self) {
    //     if self.cursor_position == 0 {
    //         return;
    //     }

    //     self.cursor_position -= 1;
    //     print!("\x1b[1D");
    // }

    // pub fn move_cursor_right(&mut self) {
    //     if self.cursor_position >= self.current_line.len() {
    //         return;
    //     }

    //     self.cursor_position += 1;
    //     print!("\x1b[1C");
    // }

    // pub fn move_history_up(&mut self) -> Result<String, ()> {
    //     if self.history_position == self.history.len() as isize - 1 {
    //         return Err(());
    //     }
    //     Ok(self.move_history(1))
    // }

    // pub fn move_history_down(&mut self) -> Result<String, ()> {
    //     if self.history_position <= -1 {
    //         return Err(());
    //     }
    //     if self.history_position == 0 {
    //         self.history_position = -1;
    //         self.clear_line();
    //         return Ok(self.current_line.clone());
    //     }
    //     Ok(self.move_history(-1))
    // }

    // fn move_history(&mut self, step: isize) -> String {
    //     self.history_position += step;
    //     let history = self
    //         .history
    //         .get(self.history_position as usize)
    //         .unwrap()
    //         .clone();
    //     self.clear_line();
    //     print!("{}", history);
    //     self.cursor_position = history.len();
    //     self.current_line = history;
    //     info!("Current history position: {}", self.history_position);
    //     self.current_line.clone()
    // }

    // pub fn clear_line(&mut self) {
    //     match self.cursor_position {
    //         0 => print!("\x1b[0K \x1b[1D"), // Note: \x1b[0D will still do \x1b[1D
    //         offset => print!("\x1b[{}D\x1b[0K \x1b[1D", offset),
    //     };
    //     self.cursor_position = 0;
    //     self.current_line.clear();
    // }
}

fn nth_visual_index(v: &[VisualType], n: usize) -> usize {
    v.iter()
        .enumerate()
        .filter(|(_, visual_type)| match visual_type {
            VisualType::Default(_) => true,
            _ => false,
        })
        .nth(n)
        .map(|(index, _)| index)
        .unwrap_or(v.len())
}
