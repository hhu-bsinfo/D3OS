use ime::{DecodedKey, Layout, wire};
use stream::event_to_u16;
use syscall::return_vals::Errno;

use crate::{keyboard, mouse, process::core_local_storage::scheduler};

pub extern "sysv64" fn sys_read_mouse() -> usize {
    match mouse() {
        Some(mouse) => mouse.read().unwrap_or(0x0) as usize,
        None => 0x0,
    }
}

/// Read from the shared keyboard or access its active layout.
pub extern "sysv64" fn sys_read_keyboard(option: usize, argument: usize) -> isize {
    let Some(keyboard) = keyboard() else {
        return match option {
            wire::READ_RAW | wire::READ_UNICODE | wire::READ_EVENT if argument == 0 => 0,
            _ => Errno::ENOENT as isize,
        };
    };

    match option {
        wire::GET_LAYOUT => return keyboard.layout() as isize,
        wire::SET_LAYOUT => {
            let Some(layout) = Layout::from_id(argument) else {
                return Errno::EINVAL as isize;
            };
            keyboard.set_layout(layout);
            return 0;
        }
        wire::READ_RAW | wire::READ_UNICODE | wire::READ_EVENT => {}
        _ => return Errno::EINVAL as isize,
    }

    loop {
        if let Some(input) = keyboard.read_input_nb() {
            match option {
                wire::READ_EVENT => return wire::encode_event(input) as isize,
                wire::READ_RAW => return event_to_u16(input.raw) as isize,
                _ => {
                    if let Some(DecodedKey::Unicode(character)) = input.decoded {
                        return character as isize;
                    }
                }
            }
        }
        if argument == 0 {
            return 0;
        }
        scheduler().yield_now();
    }
}
