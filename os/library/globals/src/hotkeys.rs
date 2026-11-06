/*!
 * Add hotkeys here, that are shared between applications
 */

use pc_keyboard::KeyCode;

/// Toggle between Text Mode (Terminal Emulator) and GUI (Window Manager)
pub const HKEY_TOGGLE_TERMINAL_WINDOW: KeyCode = KeyCode::F1;

/// Create a new terminal session/tab.
pub const HKEY_NEW_TAB: KeyCode = KeyCode::F2;

/// Close the currently active terminal session/tab.
pub const HKEY_CLOSE_TAB: KeyCode = KeyCode::F3;

/// Switch to the next terminal session/tab.
pub const HKEY_NEXT_TAB: KeyCode = KeyCode::F4;

/// Switch to the previous terminal session/tab.
pub const HKEY_PREV_TAB: KeyCode = KeyCode::F5;
