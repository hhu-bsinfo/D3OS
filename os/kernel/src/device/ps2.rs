use alloc::boxed::Box;
use alloc::sync::Arc;
use crate::interrupt::interrupt_dispatcher::InterruptVector;
use crate::interrupt::interrupt_handler::InterruptHandler;
use stream::InputStream;
use log::info;
use nolock::queues::{DequeueError, mpmc};
use ps2::flags::{ControllerConfigFlags, KeyboardLedFlags};
use ps2::{Controller, KeyboardType};
use ps2::error::{ControllerError, KeyboardError};
use spin::Mutex;
use spin::once::Once;
use crate::{apic, interrupt_dispatcher};
use pc_keyboard::layouts::{AnyLayout, De105Key};
use pc_keyboard::{DecodedKey, HandleControl, Keyboard as PcKeyboard, ScancodeSet1};

const KEYBOARD_BUFFER_CAPACITY: usize = 128;

pub struct PS2 {
    controller: Arc<Mutex<Controller>>,
    keyboard: Once<Arc<Keyboard>>,
}

pub struct Keyboard {
    controller: Arc<Mutex<Controller>>,
    buffer: (mpmc::bounded::scq::Receiver<u8>, mpmc::bounded::scq::Sender<u8>),
    decoder: Mutex<PcKeyboard<AnyLayout, ScancodeSet1>>,
}

struct KeyboardInterruptHandler {
    keyboard: Arc<Keyboard>,
}

impl Keyboard {
    fn new(controller: Arc<Mutex<Controller>>, buffer_cap: usize) -> Self {
        Self {
            controller,
            buffer: mpmc::bounded::scq::queue(buffer_cap),
            decoder: Mutex::new(PcKeyboard::new(
                ScancodeSet1::new(),
                AnyLayout::De105Key(De105Key),
                HandleControl::Ignore,
            )),
        }
    }

    pub fn plugin(keyboard: Arc<Keyboard>) {
        interrupt_dispatcher().assign(InterruptVector::Keyboard, Box::new(KeyboardInterruptHandler::new(Arc::clone(&keyboard))));
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
            match self.buffer.0.try_dequeue() {
                Ok(code) => return code as i16,
                Err(DequeueError::Closed) => return -1,
                Err(_) => {}
            }
        }
    }
}

impl KeyboardInterruptHandler {
    pub fn new(keyboard: Arc<Keyboard>) -> Self {
        Self { keyboard }
    }
}

impl InterruptHandler for KeyboardInterruptHandler {
    fn trigger(&self) {
        if let Some(mut controller) = self.keyboard.controller.try_lock() {
            if let Ok(data) = controller.read_data() {
                while self.keyboard.buffer.1.try_enqueue(data).is_err() {
                    if self.keyboard.buffer.0.try_dequeue().is_err() {
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
            controller: unsafe { Arc::new(Mutex::new(Controller::with_timeout(1000000))) },
            keyboard: Once::new()
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
        config.set(ControllerConfigFlags::ENABLE_KEYBOARD_INTERRUPT | ControllerConfigFlags::ENABLE_MOUSE_INTERRUPT | ControllerConfigFlags::ENABLE_TRANSLATE, false);
        controller.write_config(config)?;

        // Perform self test on controller
        controller.test_controller()?;
        info!("Self test result is OK");

        // Check if the controller has reset itself during the self test and if so, write the configuration byte again
        if controller.read_config()? != config {
            controller.write_config(config)?;
        }

        // Check if keyboard is present
        let test_result = controller.test_keyboard();
        if test_result.is_ok() {
            // Enable keyboard
            info!("First port detected");
            controller.enable_keyboard()?;
            config.set(ControllerConfigFlags::DISABLE_KEYBOARD, false);
            config.set(ControllerConfigFlags::ENABLE_KEYBOARD_INTERRUPT, true);
            controller.write_config(config)?;
            info!("First port enabled");
        }

        test_result
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
            KeyboardType::ATWithTranslation | KeyboardType::MF2WithTranslation | KeyboardType::ThinkPadWithTranslation => {
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

        self.keyboard.call_once(|| {
            Arc::new(Keyboard::new(Arc::clone(&self.controller), KEYBOARD_BUFFER_CAPACITY))
        });

        Ok(())
    }

    pub fn keyboard(&self) -> Option<Arc<Keyboard>> {
        match self.keyboard.is_completed() {
            true => Some(Arc::clone(self.keyboard.get().unwrap())),
            false => None
        }    }
}
