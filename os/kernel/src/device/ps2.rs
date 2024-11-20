use crate::interrupt::interrupt_dispatcher::InterruptVector;
use crate::interrupt::interrupt_handler::InterruptHandler;
use crate::{apic, buffered_lfb, interrupt_dispatcher, ps2_devices};
use alloc::boxed::Box;
use graphic::color::{self};
use log::info;
use nolock::queues::mpmc::bounded::scq::{Receiver, Sender};
use nolock::queues::{mpmc, DequeueError};
use pc_keyboard::layouts::{AnyLayout, De105Key};
use pc_keyboard::{DecodedKey, HandleControl, Keyboard as PcKeyboard, ScancodeSet1};
use ps2::error::{ControllerError, KeyboardError, MouseError};
use ps2::flags::{ControllerConfigFlags, KeyboardLedFlags};
use ps2::{Controller, KeyboardType};
use spin::Mutex;
use stream::InputStream;

const KEYBOARD_BUFFER_CAPACITY: usize = 128;

pub struct PS2 {
    controller: Mutex<Controller>,
    keyboard: Keyboard,
    mouse: Mouse,
}

pub struct Keyboard {
    buffer: (Receiver<u8>, Sender<u8>),
    decoder: Mutex<PcKeyboard<AnyLayout, ScancodeSet1>>,
}

pub struct Mouse;

#[derive(Default)]
struct KeyboardInterruptHandler;

#[derive(Default)]
struct MouseInterruptHandler {
    mouse_state: (u32, u32),
}

impl Keyboard {
    fn new(buffer_cap: usize) -> Self {
        Self {
            buffer: mpmc::bounded::scq::queue(buffer_cap),
            decoder: Mutex::new(PcKeyboard::new(
                ScancodeSet1::new(),
                AnyLayout::De105Key(De105Key),
                HandleControl::Ignore,
            )),
        }
    }

    pub fn plugin(&self) {
        interrupt_dispatcher().assign(
            InterruptVector::Keyboard,
            Box::new(KeyboardInterruptHandler::default()),
        );
        apic().allow(InterruptVector::Keyboard);
    }

    fn fetch_scancode_from_buffer(&self) -> i16 {
        let scancode = loop {
            match self.buffer.0.try_dequeue() {
                Ok(code) => break code as i16,
                Err(DequeueError::Closed) => break -1,
                Err(_) => {}
            }
        };

        if scancode == -1 {
            panic!("Keyboard stream closed!");
        }

        return scancode;
    }

    pub fn try_fetch_scancode_from_buffer(&self) -> Option<i16> {
        let scancode = match self.buffer.0.try_dequeue() {
            Ok(code) => Some(code as i16),
            Err(DequeueError::Closed) => Some(-1),
            Err(_) => None,
        };

        if scancode.is_some_and(|code| code == -1) {
            panic!("Keyboard stream closed!");
        }

        return scancode;
    }

    pub fn try_read_byte(&self) -> Option<i16> {
        let mut decoder = self.decoder.lock();
        let scancode = self.try_fetch_scancode_from_buffer()?;

        match decoder.add_byte(scancode as u8) {
            Ok(Some(event)) => {
                let key = decoder.process_keyevent(event)?;
                return match key {
                    DecodedKey::Unicode(c) => Some(c as i16),
                    _ => None,
                };
            }
            Ok(None) | Err(_) => return None,
        }
    }
}

impl InputStream for Keyboard {
    fn read_byte(&self) -> i16 {
        loop {
            let mut decoder = self.decoder.lock();
            let scancode = self.fetch_scancode_from_buffer();

            if let Ok(Some(event)) = decoder.add_byte(scancode as u8) {
                if let Some(key) = decoder.process_keyevent(event) {
                    match key {
                        DecodedKey::Unicode(c) => {
                            return c as i16;
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

impl InterruptHandler for KeyboardInterruptHandler {
    fn trigger(&mut self) {
        if let Some(mut controller) = ps2_devices().controller.try_lock() {
            if let Ok(data) = controller.read_data() {
                let keyboard = ps2_devices().keyboard();
                while keyboard.buffer.1.try_enqueue(data).is_err() {
                    if keyboard.buffer.0.try_dequeue().is_err() {
                        panic!("Keyboard: Failed to store received byte in buffer!");
                    }
                }
            }
        } else {
            panic!("Keyboard: Controller is locked during interrupt!");
        }
    }
}

impl Mouse {
    fn new() -> Self {
        Self {}
    }

    pub fn plugin(&self) {
        interrupt_dispatcher().assign(
            InterruptVector::Mouse,
            Box::new(MouseInterruptHandler::default()),
        );
        apic().allow(InterruptVector::Mouse);
    }
}

impl InterruptHandler for MouseInterruptHandler {
    fn trigger(&mut self) {
        if let Some(mut controller) = ps2_devices().controller.try_lock() {
            if let Ok((_flags, x_delta, y_delta)) = controller.mouse().read_data_packet() {
                self.mouse_state.0 = self.mouse_state.0.wrapping_add_signed(x_delta.into());
                self.mouse_state.1 = self
                    .mouse_state
                    .1
                    .wrapping_add_signed(y_delta.wrapping_neg().into());
                buffered_lfb().lock().direct_lfb().draw_pixel(
                    self.mouse_state.0,
                    self.mouse_state.1,
                    color::WHITE,
                );
            }
        } else {
            panic!("Mouse: Controller is locked during interrupt!");
        }
    }
}

impl PS2 {
    pub fn new() -> Self {
        Self {
            controller: unsafe { Mutex::new(Controller::with_timeout(1000000)) },
            keyboard: Keyboard::new(KEYBOARD_BUFFER_CAPACITY),
            mouse: Mouse::new(),
        }
    }

    pub fn init_controller(&self) -> Result<(), ControllerError> {
        info!("Initializing controller");
        let mut controller = self.controller.lock();

        // Disable ports
        controller.disable_keyboard()?;
        controller.disable_mouse()?;

        // Flush output buffer
        let _ = controller.read_data();

        // Disable interrupts and translation
        let mut config = controller.read_config()?;
        config.set(
            ControllerConfigFlags::ENABLE_KEYBOARD_INTERRUPT
                | ControllerConfigFlags::ENABLE_MOUSE_INTERRUPT
                | ControllerConfigFlags::ENABLE_TRANSLATE,
            false,
        );
        controller.write_config(config)?;

        // Perform self test on controller
        controller.test_controller()?;
        info!("Self test result is OK");

        // Check if the controller has reset itself during the self test and if so, write the configuration byte again
        if controller.read_config()? != config {
            controller.write_config(config)?;
        }

        // Check if keyboard is present
        let keyboard_test_result = controller.test_keyboard();
        if keyboard_test_result.is_ok() {
            // Enable keyboard
            info!("First port detected");
            controller.enable_keyboard()?;
            config.set(ControllerConfigFlags::DISABLE_KEYBOARD, false);
            config.set(ControllerConfigFlags::ENABLE_KEYBOARD_INTERRUPT, true);
            controller.write_config(config)?;
            info!("First port enabled");
        }

        // // Check if mouse is present
        // let mouse_test_result = controller.test_mouse();
        // if mouse_test_result.is_ok() {
        //     // Enable mouse
        //     controller.enable_mouse()?;
        //     config.set(ControllerConfigFlags::DISABLE_MOUSE, false);
        //     config.set(ControllerConfigFlags::ENABLE_MOUSE_INTERRUPT, true);
        //     controller.write_config(config)?;
        //     info!("Mouse enabled");
        // }

        return keyboard_test_result//.and(mouse_test_result);
    }

    pub fn init_keyboard(&mut self) -> Result<(), KeyboardError> {
        info!("Initializing keyboard");
        let mut controller = self.controller.lock();

        // Perform self test on keyboard
        controller.keyboard().reset_and_self_test()?;
        info!("Keyboard has been reset and self test result is OK");

        // Enable keyboard translation if needed
        controller.keyboard().disable_scanning()?;
        let kb_type = controller.keyboard().get_keyboard_type()?;
        info!("Detected keyboard type [{:?}]", kb_type);

        match kb_type {
            KeyboardType::ATWithTranslation
            | KeyboardType::MF2WithTranslation
            | KeyboardType::ThinkPadWithTranslation => {
                info!("Enabling keyboard translation");
                let mut config = controller.read_config()?;
                config.set(ControllerConfigFlags::ENABLE_TRANSLATE, true);
                controller.write_config(config)?;
            }
            _ => info!("Keyboard does not need translation"),
        }

        // Setup keyboard
        info!("Enabling keyboard");
        controller.keyboard().set_defaults()?;
        controller.keyboard().set_scancode_set(1)?;
        controller.keyboard().set_typematic_rate_and_delay(0)?;
        controller.keyboard().set_leds(KeyboardLedFlags::empty())?;
        controller.keyboard().enable_scanning()
    }

    pub fn keyboard(&self) -> &Keyboard {
        return &self.keyboard;
    }

    pub fn init_mouse(&mut self) -> Result<(), MouseError> {
        info!("Initializing mouse");
        let mut controller = self.controller.lock();

        // Perform self test on mouse
        controller.mouse().reset_and_self_test()?;
        info!("Mouse has been reset and self test result is OK");

        // Setup mouse
        info!("Enabling mouse");
        // BUG: When setting the sample_rate, you get an Error-Command: "Resend" back
        // controller.mouse().set_sample_rate(10)?;
        match controller.mouse().set_sample_rate(10) {
            Ok(_) => {}
            Err(_) => {}
        };
        controller.mouse().set_resolution(0u8)?;
        controller.mouse().set_scaling_one_to_one()?;
        controller.mouse().set_stream_mode()?;
        controller.mouse().enable_data_reporting()
    }

    pub fn mouse(&self) -> &Mouse {
        return &self.mouse;
    }
}
