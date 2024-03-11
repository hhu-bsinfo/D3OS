use crate::interrupt::interrupt_dispatcher::InterruptVector;
use crate::interrupt::interrupt_handler::InterruptHandler;
use stream::InputStream;
use alloc::boxed::Box;
use log::info;
use nolock::queues::mpmc::bounded::scq::{Receiver, Sender};
use nolock::queues::{mpmc, DequeueError};
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
            controller: unsafe { Mutex::new(Controller::with_timeout(1000000)) },
            keyboard: Keyboard::new(KEYBOARD_BUFFER_CAPACITY),
        }
    }

    pub fn init_controller(&self) {
        info!("Initializing controller");
        let mut controller = self.controller.lock();

        // Disable ports
        controller.disable_keyboard().expect("Failed to disable first port");
        controller.disable_mouse().expect("Failed to disable second port");

        // Flush output buffer
        let _ = controller.read_data();

        // Disable interrupts and translation
        let mut config = controller.read_config().expect("Failed to read config");
        config.set(ControllerConfigFlags::ENABLE_KEYBOARD_INTERRUPT | ControllerConfigFlags::ENABLE_MOUSE_INTERRUPT | ControllerConfigFlags::ENABLE_TRANSLATE, false);
        controller.write_config(config).expect("Failed to write config");

        // Perform self test on controller
        controller.test_controller().expect("Self test failed");
        info!("Self test result is OK");

        // Check if the controller has reset itself during the self test and if so, write the configuration byte again
        if controller.read_config().expect("Failed to read config") != config {
            controller.write_config(config).expect("Failed to write config");
        }

        // Check if keyboard is present
        if controller.test_keyboard().is_ok() {
            // Enable keyboard
            info!("First port detected");
            controller.enable_keyboard().expect("Failed to enable first port");
            config.set(ControllerConfigFlags::DISABLE_KEYBOARD, false);
            config.set(ControllerConfigFlags::ENABLE_KEYBOARD_INTERRUPT, true);
            controller.write_config(config).expect("Failed to write config");
            info!("First port enabled");
        } else {
            panic!("No keyboard detected!");
        }
    }

    pub fn init_keyboard(&mut self) {
        info!("Initializing keyboard");
        let mut controller = self.controller.lock();

        // Perform self test on keyboard
        controller.keyboard().reset_and_self_test().expect("Keyboard self test failed");
        info!("Keyboard has been reset and self test result is OK");

        // Enable keyboard translation if needed
        controller.keyboard().disable_scanning().expect("Failed to disable scanning");
        let kb_type = controller.keyboard().get_keyboard_type().expect("Failed to query keyboard type");
        info!("Detected keyboard type [{:?}]", kb_type);

        match kb_type {
            KeyboardType::ATWithTranslation | KeyboardType::MF2WithTranslation | KeyboardType::ThinkPadWithTranslation => {
                info!("Enabling keyboard translation");
                let mut config = controller.read_config().expect("Failed to read config");
                config.set(ControllerConfigFlags::ENABLE_TRANSLATE, true);
                controller.write_config(config).expect("Failed to write config");
            }
            _ => info!("Keyboard does not need translation"),
        }

        // Setup keyboard
        info!("Enabling keyboard");
        controller.keyboard().set_defaults().expect("Failed to set default keyboard configuration");
        controller.keyboard().set_scancode_set(1).expect("Failed to set scancode set");
        controller.keyboard().set_typematic_rate_and_delay(0).expect("Failed to set typematic rate");
        controller.keyboard().set_leds(KeyboardLedFlags::empty()).expect("Failed to set LEDs");
        controller.keyboard().enable_scanning().expect("Failed to enable scanning");
    }

    pub fn keyboard(&self) -> &Keyboard {
        return &self.keyboard;
    }
}
