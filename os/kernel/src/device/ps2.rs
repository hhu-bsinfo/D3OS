use alloc::boxed::Box;
use alloc::sync::Arc;
use crate::interrupt::interrupt_dispatcher::InterruptVector;
use crate::interrupt::interrupt_handler::InterruptHandler;
use stream::{DecodedInputStream, InputStream};
use log::{debug, info};
use nolock::queues::{DequeueError, mpmc};
use ps2::flags::{ControllerConfigFlags, KeyboardLedFlags};
use ps2::{Controller, KeyboardType};
use ps2::error::{ControllerError, KeyboardError, MouseError};
use pc_keyboard::layouts::{AnyLayout, De105Key};
use pc_keyboard::{DecodedKey, HandleControl, Keyboard as PcKeyboard, ScancodeSet1};
use spin::Mutex;
use spin::once::Once;
use crate::{apic, interrupt_dispatcher};

const KEYBOARD_BUFFER_CAPACITY: usize = 128;
const MOUSE_BUFFER_CAPACITY: usize = 128;

pub struct PS2 {
    controller: Arc<Mutex<Controller>>,
    keyboard: Once<Arc<Keyboard>>,
    mouse: Once<Arc<Mouse>>,
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
}

impl DecodedInputStream for Keyboard {
    fn decoded_read_byte(&self) -> i16 {
        loop {
            let mut decoder = self.decoder.lock();

            let scancode = match self.buffer.0.try_dequeue() {
                Ok(code) => Some(code as i16),
                Err(DequeueError::Closed) => {
                    panic!("Keyboard stream closed!");
                },
                Err(_) => {
                    panic!("An error occured!");
                },
            };

            if let Ok(Some(event)) = decoder.add_byte(scancode.unwrap() as u8) {
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

    fn decoded_try_read_byte(&self) -> Option<i16> {
        let mut decoder = self.decoder.lock();

        let scancode = match self.buffer.0.try_dequeue() {
            Ok(code) => Some(code as i16),
            Err(DequeueError::Closed) => {
                panic!("Keyboard stream closed!");
            },
            Err(_) => None
        };

        match decoder.add_byte(scancode? as u8) {
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

#[derive(Default, Clone, Copy)]
pub struct MousePacket {
    pub flags: u8,
    pub dx: i8,
    pub dy: i8,
}

pub struct Mouse {
    controller: Arc<Mutex<Controller>>,
    buffer: (mpmc::bounded::scq::Receiver<MousePacket>, mpmc::bounded::scq::Sender<MousePacket>),
}

#[derive(Default)]
struct MouseState {
    cycle: i8,
    current_packet: MousePacket
}

struct MouseInterruptHandler {
    mouse: Arc<Mouse>,
    state: Mutex<MouseState>
}

impl Mouse {
    fn new(controller: Arc<Mutex<Controller>>, buffer_cap: usize) -> Self {
        Self {
            controller,
            buffer: mpmc::bounded::scq::queue(buffer_cap)
        }
    }

    pub fn plugin(mouse: Arc<Mouse>) {
        interrupt_dispatcher().assign(InterruptVector::Mouse, Box::new(MouseInterruptHandler::new(Arc::clone(&mouse))));
        apic().allow(InterruptVector::Mouse);
    }
}

impl MouseInterruptHandler {
    pub fn new(mouse: Arc<Mouse>) -> Self {
        Self { mouse, state: Mutex::new(MouseState::default()) }
    }
}

impl InterruptHandler for MouseInterruptHandler {
    fn trigger(&self) {
        if let Some(mut controller) = self.mouse.controller.try_lock() {
            if let Ok(data) = controller.read_data() {
                let mut mouse_state = self.state.lock();

                match mouse_state.cycle {
                    0 => {
                        // Is it really the first byte?
                        if data & 0x08 == 0 {
                            debug!("Mouse: Discarding invalid first byte");
                            return;
                        }

                        // Read first byte (flags)
                        mouse_state.current_packet = MousePacket::default();
                        mouse_state.current_packet.flags = data;

                        mouse_state.cycle += 1;
                    },

                    1 => {
                        // Read second byte (delta x)
                        mouse_state.current_packet.dx = data as i8;

                        mouse_state.cycle += 1;
                    }

                    2 => {
                        // Read third byte (delta y)
                        mouse_state.current_packet.dy = data as i8;

                        let packet = mouse_state.current_packet;
                        debug!("Mouse: flags = {}, dx = {}, dy = {}", packet.flags, packet.dx, packet.dy);

                        // The packet is complete. Enqueue it!
                        while self.mouse.buffer.1.try_enqueue(mouse_state.current_packet.clone()).is_err() {
                            if self.mouse.buffer.0.try_dequeue().is_err() {
                                panic!("Mouse: Failed to store received packet in buffer!");
                            }
                        }

                        mouse_state.cycle = 0;
                    }

                    _ => {
                        mouse_state.cycle = 0;
                    }
                }
            }
            //debug!("Mouse interrupt handler called");
        } else {
            panic!("Mouse: Controller is locked during interrupt!");
        }
    }
}

impl PS2 {
    pub fn new() -> Self {
        Self {
            controller: unsafe { Arc::new(Mutex::new(Controller::with_timeout(1000000))) },
            keyboard: Once::new(),
            mouse: Once::new()
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

        // Check if mouse is present
        let test_result = controller.test_mouse();
        if test_result.is_ok() {
            // Enable mouse
            info!("Second port detected");
            controller.enable_mouse()?;
            config.set(ControllerConfigFlags::DISABLE_MOUSE, false);
            config.set(ControllerConfigFlags::ENABLE_MOUSE_INTERRUPT, true);
            controller.write_config(config)?;
            info!("Second port enabled");
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

    pub fn init_mouse(&mut self) -> Result<(), MouseError> {
        info!("Initializing Mouse");
        let mut controller = self.controller.lock();

        // Perform self test on mouse
        controller.mouse().reset_and_self_test()?;
        info!("Mouse has been reset and self test result is OK");

        // Setup mouse
        controller.mouse().set_defaults()?;
        //controller.mouse().set_resolution(2)?;
        //controller.mouse().set_sample_rate(80)?;
        //controller.mouse().set_scaling_one_to_one()?;
        //controller.mouse().set_stream_mode()?;
        controller.mouse().enable_data_reporting()?;

        self.mouse.call_once(|| {
            Arc::new(Mouse::new(Arc::clone(&self.controller), MOUSE_BUFFER_CAPACITY))
        });
        
        Ok(())
    }

    pub fn keyboard(&self) -> Option<Arc<Keyboard>> {
        match self.keyboard.is_completed() {
            true => Some(Arc::clone(self.keyboard.get().unwrap())),
            false => None
        }
    }

    pub fn mouse(&self) -> Option<Arc<Mouse>> {
        match self.mouse.is_completed() {
            true => Some(Arc::clone(self.mouse.get().unwrap())),
            false => None
        }
    }
}
