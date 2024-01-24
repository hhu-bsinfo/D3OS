use crate::interrupt::interrupt_dispatcher::InterruptVector;
use crate::interrupt::interrupt_handler::InterruptHandler;
use io::stream::InputStream;
use alloc::boxed::Box;
use log::info;
use nolock::queues::mpmc::bounded::scq::{Receiver, Sender};
use nolock::queues::{mpmc, DequeueError};
use ps2::error::{ControllerError, KeyboardError};
use ps2::flags::{ControllerConfigFlags, KeyboardLedFlags};
use ps2::{Controller, KeyboardType};
use spin::Mutex;
use crate::{apic, interrupt_dispatcher, ps2_devices};

const KEYBOARD_BUFFER_CAPACITY: usize = 128;

pub struct PS2 {
    controller: Mutex<Controller>,
    keyboard: Keyboard,
}

pub struct Keyboard {
    buffer: (Receiver<u8>, Sender<u8>),
}

#[derive(Default)]
struct KeyboardInterruptHandler;

impl Keyboard {
    fn new(buffer_cap: usize) -> Self {
        Self {
            buffer: mpmc::bounded::scq::queue(buffer_cap),
        }
    }

    pub fn plugin(&self) {
        interrupt_dispatcher().assign(InterruptVector::Keyboard, Box::new(KeyboardInterruptHandler::default()));
        apic().allow(InterruptVector::Keyboard);
    }
}

impl InputStream for Keyboard {
    fn read_byte(&self) -> i16 {
        loop {
            match self.buffer.0.try_dequeue() {
                Ok(code) => return code as i16,
                Err(DequeueError::Closed) => return -1,
                Err(_) => {}
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

impl PS2 {
    pub fn new() -> Self {
        Self {
            controller: unsafe { Mutex::new(Controller::new()) },
            keyboard: Keyboard::new(KEYBOARD_BUFFER_CAPACITY),
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
        if controller.test_keyboard().is_ok() {
            // Enable keyboard
            info!("First port detected");
            controller.enable_keyboard()?;
            config.set(ControllerConfigFlags::DISABLE_KEYBOARD, false);
            config.set(ControllerConfigFlags::ENABLE_KEYBOARD_INTERRUPT, true);
            controller.write_config(config)?;
            info!("First port enabled");
        } else {
            panic!("No keyboard detected!");
        }

        // Check if mouse is present
        if controller.test_mouse().is_ok() {
            // Enable mouse
            info!("Second port detected");
            controller.enable_keyboard()?;
            config.set(ControllerConfigFlags::DISABLE_MOUSE, false);
            config.set(ControllerConfigFlags::ENABLE_MOUSE_INTERRUPT, true);
            controller.write_config(config)?;
            info!("Second port enabled");
        }

        return Ok(());
    }

    pub fn init_keyboard(&mut self) -> Result<(), KeyboardError> {
        info!("Initializing keyboard");
        let mut controller = self.controller.lock();

        // Perform self test on keyboard
        if controller.keyboard().reset_and_self_test().is_err() {
            panic!("Keyboard is not working!");
        }
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
        controller.keyboard().enable_scanning()?;

        return Ok(());
    }

    pub fn keyboard(&self) -> &Keyboard {
        return &self.keyboard;
    }
}
