use input::ReadKeyboardOption;
use log::error;
use stream::{event_to_u16, DecodedInputStream, RawInputStream};

use crate::{keyboard, mouse, process::core_local_storage::scheduler};

pub extern "sysv64" fn sys_read_mouse() -> usize {
    match mouse() {
        Some(mouse) => mouse.read().unwrap_or(0x0) as usize,
        None => 0x0,
    }
}

/// SystemCall implementation for SystemCall::KeyboardRead.
/// Reads from keyboard with given mode (Raw or Decoded).
pub extern "sysv64" fn sys_read_keyboard(option: ReadKeyboardOption, blocking: bool) -> isize {
    // if VIRTIO_INPUT_PENDING.load(Ordering::Acquire) {
    //     if let Some(input_dev_mutex) = virtio_input() {
    //         if let Some(mut input_dev) = input_dev_mutex.try_lock() {
    //             let mut event_processed = false;
    //             while let Some(event) = input_dev.pop_pending_event(){
    //                 event_processed = true;
    //                 if event.event_type == 1 && event.value == 1 {
    //                     info!("VirtIO Input Event (from Terminal): type={}, code={}, value={}", event.event_type, event.code, event.value);
    //                 }
    //             }
    //         }
    //     }
    // }
    if let Some(keyboard) = keyboard() {
        match option {
            ReadKeyboardOption::Raw => {
                let event = if blocking {
                    Some(keyboard.read_event())
                } else {
                    keyboard.read_event_nb()
                };
                if let Some(event) = event {
                    event_to_u16(event).try_into().unwrap()
                } else { 0 }
            },
            ReadKeyboardOption::Decode => (if blocking {
                keyboard.decoded_read_byte()
            } else {
                keyboard.decoded_try_read_byte().unwrap_or_default()
            } as isize),
        }
    } else {
        if blocking {
            error!("failed to read from keyboard");
            loop {
                scheduler().yield_now();
            }
        } else {
            0
        }
    }
}
