/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: lfb_terminal                                                    ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Framebuffer presenter for the terminal emulator.                ║
   ║                                                                         ║
   ║ `LFBTerminal` no longer parses ANSI or owns terminal state. It renders  ║
   ║ a session's semantic `TerminalModel` onto the linear framebuffer, draws ║
   ║ the status bar, and owns the blinking cursor overlay. All terminal      ║
   ║ semantics live per session in `TerminalModel`; this type is the single  ║
   ║ shared view onto the active session.                                    ║
   ║                                                                         ║
   ║ Presentation is either a full viewport repaint (`present_full`, used on ║
   ║ tab switch) or an incremental repaint of the damaged rows               ║
   ║ (`present_damage`, used for live active-session output).                ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Authors: Sebastian Keller, Aymen Sellami                                ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/

use alloc::{format, string::ToString};
use concurrent::{process, thread};
use graphic::{color, lfb};
use input::keyboard;
use pc_keyboard::KeyEvent;
use spin::Mutex;
use stream::RawInputStream;
use time::{date, systime};

use crate::util::system_info::system_info;

use super::{
    display::{Character, DisplayState},
    model::{Damage, ScreenSize, TerminalModel},
};

/// Cursor overlay glyph (full block, falling back to underscore).
const CURSOR: char = match char::from_u32(0x2588) {
    Some(cursor) => cursor,
    None => '_',
};

pub struct LFBTerminal {
    pub(crate) display: Mutex<DisplayState>,
}

unsafe impl Send for LFBTerminal {}
unsafe impl Sync for LFBTerminal {}

impl RawInputStream for LFBTerminal {
    fn read_event(&self) -> KeyEvent {
        keyboard::read_raw(true).unwrap()
    }

    fn read_event_nb(&self) -> Option<KeyEvent> {
        keyboard::read_raw(false)
    }
}

impl LFBTerminal {
    pub fn new(buffer: *mut u8, pitch: u32, width: u32, height: u32, bpp: u8) -> Self {
        Self {
            display: Mutex::new(DisplayState::new(buffer, pitch, width, height, bpp)),
        }
    }

    /// The grid size a session model must use to match this framebuffer.
    pub fn screen_size(&self) -> ScreenSize {
        let display = self.display.lock();
        ScreenSize {
            cols: display.size.0,
            rows: display.size.1,
        }
    }

    /// Repaint the entire content viewport plus the status bar from `model`.
    /// Used on tab switch and after returning from GUI mode; cost is bounded by
    /// the viewport size and never by output history.
    pub fn present_full(&self, model: &TerminalModel) {
        let mut display = self.display.lock();
        let rows = display.size.1;
        for r in 1..rows {
            Self::present_row(&mut display, model, r);
        }
        Self::draw_status_bar(&mut display);
        display.lfb.flush();

        display.cursor_pos = (model.cursor.col, model.cursor.row);
        display.cursor_visible = false;
    }

    /// Repaint only the rows a feed changed. `full` damage falls back to
    /// `present_full`; a feed that scrolled shifts the framebuffer instead of
    /// redrawing every glyph on screen.
    pub fn present_damage(&self, model: &TerminalModel, damage: Damage) {
        if damage.full {
            self.present_full(model);
            return;
        }

        let mut display = self.display.lock();

        // Erase the previous cursor overlay before touching content.
        Self::restore_cursor_cell(&mut display);

        let scrolled = damage.scrolled > 0;
        if scrolled {
            Self::scroll_presented(&mut display, damage.scrolled);
        }

        if let Some((min, max)) = damage.dirty_range() {
            let start = min.max(1);
            let last = max.min(display.size.1.saturating_sub(1));
            if start <= last {
                for r in start..=last {
                    Self::present_row(&mut display, model, r);
                }
                if !scrolled {
                    let y = start as u32 * lfb::DEFAULT_CHAR_HEIGHT;
                    let h = (last - start + 1) as u32 * lfb::DEFAULT_CHAR_HEIGHT;
                    display.lfb.flush_lines(y, h);
                }
            }
        }

        if scrolled {
            // The scroll dragged content through the status row, and every
            // remaining line moved, so the whole surface has to go out.
            Self::draw_status_bar(&mut display);
            display.lfb.flush();
        }

        display.cursor_pos = (model.cursor.col, model.cursor.row);
        display.cursor_visible = false;
    }

    /// Move the presented image up by `rows` character rows, matching the
    /// model's own scroll. Only the rows the scroll exposed still need to be
    /// drawn from the model, which is the whole point of doing it this way.
    fn scroll_presented(display: &mut DisplayState, rows: u16) {
        let shift = rows.min(display.size.1);
        if shift == 0 {
            return;
        }

        display
            .lfb
            .lfb()
            .scroll_up(shift as u32 * lfb::DEFAULT_CHAR_HEIGHT);

        // Keep the presented-cell snapshot in step, so the cursor overlay
        // restores the character that is actually on screen.
        let stride = display.size.0 as usize;
        let moved = shift as usize * stride;
        if moved < display.visible.len() {
            display.visible.copy_within(moved.., 0);
        }
        let tail = display.visible.len().saturating_sub(moved);
        display.visible[tail..].fill(Character::BLANK);
    }

    /// Draw one content row of `model` into the buffered framebuffer and update
    /// the presented-cell snapshot. Continuation cells of wide glyphs are
    /// recorded but not drawn (their lead cell already covered the columns).
    ///
    /// Cells the model does not have are drawn blank rather than indexed: the
    /// grid is sized from this framebuffer today, but nothing in the type
    /// system says so, and a mismatch must not be a panic.
    fn present_row(display: &mut DisplayState, model: &TerminalModel, r: u16) {
        let cols = display.size.0;
        for c in 0..cols {
            let cell = model.cell(c, r);
            let idx = (r * cols + c) as usize;
            display.visible[idx] = cell.map_or(Character::BLANK, |cell| Character {
                value: cell.value,
                fg_color: cell.fg_color,
                bg_color: cell.bg_color,
                width: cell.width,
            });

            let Some(cell) = cell else {
                continue;
            };

            if cell.width == 0 {
                continue;
            }

            let glyph = if cell.value == '\0' { ' ' } else { cell.value };
            display.lfb.lfb().draw_char(
                c as u32 * lfb::DEFAULT_CHAR_WIDTH,
                r as u32 * lfb::DEFAULT_CHAR_HEIGHT,
                cell.fg_color,
                cell.bg_color,
                glyph,
            );
        }
    }

    /// Redraw the presented character under the cursor overlay on the visible
    /// framebuffer, clearing any block that was drawn there.
    fn restore_cursor_cell(display: &mut DisplayState) {
        let (col, row) = display.cursor_pos;
        if row >= display.size.1 || col >= display.size.0 {
            return;
        }
        let idx = (row * display.size.0 + col) as usize;
        let cell = display.visible[idx];
        if cell.width == 0 {
            // Nothing was drawn over a continuation cell, so there is nothing
            // to put back - and redrawing it would erase half a wide glyph.
            display.cursor_visible = false;
            return;
        }
        let glyph = if cell.value == '\0' { ' ' } else { cell.value };
        display.lfb.direct_lfb().draw_char(
            col as u32 * lfb::DEFAULT_CHAR_WIDTH,
            row as u32 * lfb::DEFAULT_CHAR_HEIGHT,
            cell.fg_color,
            cell.bg_color,
            glyph,
        );
        display.cursor_visible = false;
    }

    /// Toggle the blinking cursor overlay. Drawn on the direct framebuffer only,
    /// so it never corrupts the buffered content that presentation flushes.
    pub fn toggle_cursor(&self) {
        let mut display = self.display.lock();
        let (col, row) = display.cursor_pos;
        if row >= display.size.1 || col >= display.size.0 {
            return;
        }
        let idx = (row * display.size.0 + col) as usize;
        let cell = display.visible[idx];
        if cell.width == 0 {
            // Blinking on the second half of a wide glyph would chop it up.
            return;
        }
        let show_block = !display.cursor_visible;

        let glyph = if show_block {
            CURSOR
        } else if cell.value == '\0' {
            ' '
        } else {
            cell.value
        };

        display.lfb.direct_lfb().draw_char(
            col as u32 * lfb::DEFAULT_CHAR_WIDTH,
            row as u32 * lfb::DEFAULT_CHAR_HEIGHT,
            cell.fg_color,
            cell.bg_color,
            glyph,
        );
        display.cursor_visible = show_block;
    }

    /// Update the status-bar tab snapshot from the session multiplexer.
    ///
    /// `ids` must be the live session ids in ascending order; `active` is the
    /// internal id of the active session. This only mutates the snapshot behind
    /// the `display` lock and returns immediately, so callers must release it
    /// before invoking any drawing method (which re-locks `display`).
    pub fn update_tabs(&self, ids: &[u8], active: u8) {
        let mut display = self.display.lock();
        display.tab_ids.clear();
        display.tab_ids.extend_from_slice(ids);
        display.active_tab = active;
    }

    pub fn draw_status_bar(display: &mut DisplayState) {
        let total_cols = display.size.0 as u32;

        Self::draw_status_background(display, total_cols);
        let used_cols = Self::draw_status_tabs(display, total_cols);
        let date_start = Self::draw_status_date(display, total_cols, used_cols);
        Self::draw_status_info(display, used_cols, date_start);

        display.lfb.flush_lines(0, lfb::DEFAULT_CHAR_HEIGHT);
    }

    fn draw_status_background(display: &mut DisplayState, total_cols: u32) {
        for i in 0..total_cols * lfb::DEFAULT_CHAR_WIDTH {
            for j in 0..lfb::DEFAULT_CHAR_HEIGHT {
                display.lfb.lfb().draw_pixel(i, j, color::HHU_GREEN);
            }
        }
    }

    fn draw_status_tabs(display: &mut DisplayState, total_cols: u32) -> u32 {
        // Snapshot the tab state so we don't hold a borrow of `display` while
        // issuing the mutable framebuffer calls below.
        let tab_ids = display.tab_ids.clone();
        let active = display.active_tab;

        // Tabs (highest priority) are drawn left-aligned. The user-facing label
        // is a dense display ordinal derived from the live-session order, never
        // the internal session id. Only whole tabs that fit are drawn.
        let mut used_cols: u32 = 0;
        for (index, &id) in tab_ids.iter().enumerate() {
            let label = format!(" {} ", index + 1);
            let label_cols = label.chars().count() as u32;
            if used_cols + label_cols > total_cols {
                break;
            }
            let (fg, bg) = if id == active {
                (color::WHITE, color::HHU_BLUE) // inverse / high-contrast
            } else {
                (color::HHU_BLUE, color::INVISIBLE) // normal bar styling
            };
            display
                .lfb
                .lfb()
                .draw_string(used_cols * lfb::DEFAULT_CHAR_WIDTH, 0, fg, bg, &label);
            used_cols += label_cols;
        }

        used_cols
    }

    fn draw_status_date(display: &mut DisplayState, total_cols: u32, used_cols: u32) -> u32 {
        // Date/time (second priority), right-aligned, drawn only when it fits
        // after the tabs with at least one gap column. `total_cols` doubles as a
        // "no date drawn" sentinel for the info budget below.
        let date_str = date().format("%Y-%m-%d %H:%M:%S").to_string();
        let date_cols = date_str.chars().count() as u32;
        let mut date_start = total_cols;
        if date_cols < total_cols && total_cols - date_cols > used_cols {
            date_start = total_cols - date_cols;
            display.lfb.lfb().draw_string(
                date_start * lfb::DEFAULT_CHAR_WIDTH,
                0,
                color::HHU_BLUE,
                color::INVISIBLE,
                &date_str,
            );
        }

        date_start
    }

    fn draw_status_info(display: &mut DisplayState, used_cols: u32, date_start: u32) {
        // System info (lowest priority) fills the gap between tabs and date,
        // choosing the most detailed variant that fits and dropping entirely
        // when there is no room.
        let info_start = used_cols + 1; // one gap column after the tabs
        let info_end = date_start.saturating_sub(1); // one gap column before date
        if info_end > info_start {
            let available = info_end - info_start;

            let uptime = systime();
            let uptime_str = format!(
                "{:0>2}:{:0>2}:{:0>2}",
                uptime.num_hours(),
                uptime.num_minutes() % 60,
                uptime.num_seconds() - (uptime.num_minutes() * 60),
            );
            let system_info = system_info();

            let full = format!(
                "D³OS v{} ({}) | Uptime: {} | Processes: {} | Threads: {}",
                system_info.pkg_version,
                system_info.profile,
                uptime_str,
                process::count(),
                thread::count(),
            );
            let medium = format!(
                "Up {} | P:{} T:{}",
                uptime_str,
                process::count(),
                thread::count()
            );
            let short = format!("Up {}", uptime_str);

            let info = [full, medium, short]
                .into_iter()
                .find(|candidate| candidate.chars().count() as u32 <= available);

            if let Some(info) = info {
                display.lfb.lfb().draw_string(
                    info_start * lfb::DEFAULT_CHAR_WIDTH,
                    0,
                    color::HHU_BLUE,
                    color::INVISIBLE,
                    &info,
                );
            }
        }
    }
}
