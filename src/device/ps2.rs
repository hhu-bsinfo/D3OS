use alloc::boxed::Box;
use nolock::queues::{DequeueError, mpmc};
use nolock::queues::mpmc::bounded::scq::{Receiver, Sender};
use ps2::{Controller, KeyboardType};
use ps2::error::{ControllerError, KeyboardError};
use ps2::flags::{ControllerConfigFlags, KeyboardLedFlags};
use spin::Mutex;
use crate::kernel;
use crate::kernel::interrupt_dispatcher::InterruptVector;
use crate::kernel::isr::ISR;
use crate::library::io::stream::InputStream;

pub struct Keyboard {
    buffer: Option<(Receiver<u8>, Sender<u8>)>
}

pub struct PS2 {
    controller: Mutex<Controller>,
    keyboard: Keyboard
}

#[derive(Default)]
pub struct KeyboardISR;

impl Keyboard {
    const fn new() -> Self {
        Self { buffer: None }
    }

    fn init(&mut self, buffer_cap: usize) {
        self.buffer = Some(mpmc::bounded::scq::queue(buffer_cap));
    }
}

impl InputStream for Keyboard {
    fn read_byte(&mut self) -> i16 {
        loop {
            if let Some(buffer) = self.buffer.as_mut() {
                match buffer.0.try_dequeue() {
                    Ok(code) => return code as i16,
                    Err(DequeueError::Closed) => return -1,
                    Err(_) => {}
                }
            } else {
                panic!("Keyboard: Trying to read before initialization!");
            }
        }
    }
}

impl ISR for KeyboardISR {
    fn trigger(&self) {
        if let Some(mut controller) = kernel::get_device_service().get_ps2().controller.try_lock() {
            if let Ok(data) = controller.read_data() {
                let keyboard = kernel::get_device_service().get_ps2().get_keyboard();
                keyboard.buffer.as_mut().expect("Keyboard: ISR happened before initialization!").1.try_enqueue(data).expect("Keyboard: Buffer is full!");
            }
        } else {
            panic!("Keyboard: Controller is locked during interrupt!");
        }
    }
}

impl PS2 {
    pub const fn new() -> Self {
        Self { controller: unsafe { Mutex::new(Controller::new()) }, keyboard: Keyboard::new() }
    }

    pub fn init_controller(&mut self) -> Result<(), ControllerError> {
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

        // Check if the controller has reset itself during the self test and if so, write the configuration byte again
        if controller.read_config()? != config {
            controller.write_config(config)?;
        }

        // Check if keyboard is present
        if controller.test_keyboard().is_ok() {
            // Enable keyboard
            controller.enable_keyboard()?;
            config.set(ControllerConfigFlags::DISABLE_KEYBOARD, false);
            config.set(ControllerConfigFlags::ENABLE_KEYBOARD_INTERRUPT, true);
            controller.write_config(config)?;
        } else {
            panic!("No keyboard detected!");
        }

        // Check if mouse is present
        if controller.test_mouse().is_ok() {
            // Enable mouse
            controller.enable_keyboard()?;
            config.set(ControllerConfigFlags::DISABLE_MOUSE, false);
            config.set(ControllerConfigFlags::ENABLE_MOUSE_INTERRUPT, true);
            controller.write_config(config)?;
        }

        return Ok(());
    }

    pub fn init_keyboard(&mut self) -> Result<(), KeyboardError> {
        let mut controller = self.controller.lock();
        
        // Perform self test on keyboard
        if controller.keyboard().reset_and_self_test().is_err() {
            panic!("Keyboard is not working!");
        }

        // Enable keyboard translation if needed
        controller.keyboard().disable_scanning()?;
        match controller.keyboard().get_keyboard_type()? {
            KeyboardType::ATWithTranslation | KeyboardType::MF2WithTranslation | KeyboardType::ThinkPadWithTranslation => {
                let mut config = controller.read_config()?;
                config.set(ControllerConfigFlags::ENABLE_TRANSLATE, true);
                controller.write_config(config)?;
            }
            _ => {}
        }

        // Setup keyboard
        controller.keyboard().set_defaults()?;
        controller.keyboard().set_scancode_set(1)?;
        controller.keyboard().set_typematic_rate_and_delay(0)?;
        controller.keyboard().set_leds(KeyboardLedFlags::empty())?;
        controller.keyboard().enable_scanning()?;

        self.keyboard.init(128);

        return Ok(());
    }

    pub fn get_keyboard(&mut self) -> &mut Keyboard {
        return &mut self.keyboard;
    }

    pub fn plugin_keyboard(&self) {
        let int_service = kernel::get_interrupt_service();
        int_service.get_dispatcher().assign(InterruptVector::Keyboard, Box::new(KeyboardISR::default()));
        int_service.get_apic().allow(InterruptVector::Keyboard);
    }
}