use alloc::boxed::Box;
use lazy_static::lazy_static;
use nolock::queues::DequeueError;
use nolock::queues::spsc::bounded;
use nolock::queues::spsc::bounded::{BoundedReceiver, BoundedSender};
use ps2::{Controller, KeyboardType};
use ps2::flags::{ControllerConfigFlags, KeyboardLedFlags};
use spin::Mutex;
use crate::device::apic;
use crate::kernel::int_disp;
use crate::kernel::int_disp::InterruptVector;
use crate::kernel::isr::ISR;
use crate::library::io::stream::InputStream;

static CONTROLLER: Mutex<Controller> = Mutex::new(unsafe { Controller::new() });

lazy_static! {
static ref KEYBOARD: Mutex<Keyboard> = Mutex::new(Keyboard::new(128));
}

pub fn get_keyboard() -> &'static Mutex<Keyboard> {
    return &KEYBOARD;
}

pub struct Keyboard {
    buffer: (BoundedReceiver<u8>, BoundedSender<u8>)
}

#[derive(Default)]
pub struct KeyboardISR;

impl Keyboard {
    fn new(buffer_cap: usize) -> Self {
        Self { buffer: bounded::queue::<u8>(buffer_cap) }
    }
}

impl InputStream for Keyboard {
    fn read_byte(&mut self) -> i16 {
        loop {
            match self.buffer.0.try_dequeue() {
                Ok(code) => return code as i16,
                Err(DequeueError::Closed) => return -1,
                Err(_) => {}
            }
        }
    }
}

impl ISR for KeyboardISR {
    fn trigger(&self) {
        if let Some(mut controller) = CONTROLLER.try_lock() {
            if let Ok(data) = controller.read_data() {
                unsafe {
                    KEYBOARD.force_unlock();
                    KEYBOARD.lock().buffer.1.try_enqueue(data).expect("Keyboard: Buffer is full!");
                }
            }
        } else {
            panic!("Keyboard: Controller is locked during interrupt!")
        }
    }
}

pub fn init_controller() {
    let mut controller = CONTROLLER.lock();

    // Disable ports
    controller.disable_keyboard().unwrap();
    controller.disable_mouse().unwrap();

    // Flush output buffer
    let _ = controller.read_data();

    // Disable interrupts and translation
    let mut config = controller.read_config().unwrap();
    config.set(ControllerConfigFlags::ENABLE_KEYBOARD_INTERRUPT | ControllerConfigFlags::ENABLE_MOUSE_INTERRUPT | ControllerConfigFlags::ENABLE_TRANSLATE, false);
    controller.write_config(config).unwrap();

    // Perform self test on controller
    controller.test_controller().unwrap();

    // Check if the controller has reset itself during the self test and if so, write the configuration byte again
    if controller.read_config().unwrap() != config {
        controller.write_config(config).unwrap();
    }

    // Check if keyboard is present
    if controller.test_keyboard().is_ok() {
        // Enable keyboard
        controller.enable_keyboard().unwrap();
        config.set(ControllerConfigFlags::DISABLE_KEYBOARD, false);
        config.set(ControllerConfigFlags::ENABLE_KEYBOARD_INTERRUPT, true);
        controller.write_config(config).unwrap();
    } else {
        panic!("No keyboard detected!");
    }

    // Check if mouse is present
    if controller.test_mouse().is_ok() {
        // Enable mouse
        controller.enable_keyboard().unwrap();
        config.set(ControllerConfigFlags::DISABLE_MOUSE, false);
        config.set(ControllerConfigFlags::ENABLE_MOUSE_INTERRUPT, true);
        controller.write_config(config).unwrap();
    }
}

pub fn init_keyboard() {
    let mut controller = CONTROLLER.lock();

    // Perform self test on keyboard
    if controller.keyboard().reset_and_self_test().is_err() {
        panic!("Keyboard is not working!");
    }

    // Enable keyboard translation if needed
    controller.keyboard().disable_scanning().unwrap();
    match controller.keyboard().get_keyboard_type().unwrap() {
        KeyboardType::ATWithTranslation | KeyboardType::MF2WithTranslation | KeyboardType::ThinkPadWithTranslation => {
            let mut config = controller.read_config().unwrap();
            config.set(ControllerConfigFlags::ENABLE_TRANSLATE, true);
            controller.write_config(config).unwrap();
        }
        _ => {}
    }

    // Setup keyboard
    controller.keyboard().set_defaults().unwrap();
    controller.keyboard().set_scancode_set(1).unwrap();
    controller.keyboard().set_typematic_rate_and_delay(0).unwrap();
    controller.keyboard().set_leds(KeyboardLedFlags::empty()).unwrap();
    controller.keyboard().enable_scanning().unwrap();
}

pub fn plugin_keyboard() {
    int_disp::assign(InterruptVector::Keyboard, Box::new(KeyboardISR::default()));
    apic::get_apic().lock().allow(InterruptVector::Keyboard);
}