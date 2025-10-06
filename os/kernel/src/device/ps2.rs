use alloc::boxed::Box;
use alloc::sync::Arc;
use crate::interrupt::interrupt_dispatcher::InterruptVector;
use crate::interrupt::interrupt_handler::InterruptHandler;
use stream::{DecodedInputStream, RawInputStream};
use log::{debug, error, info};
use nolock::queues::{DequeueError, mpmc};
use ps2::flags::{ControllerConfigFlags, KeyboardLedFlags};
use ps2::{Controller, KeyboardType, MouseType};
use ps2::error::{ControllerError, KeyboardError, MouseError};
use pc_keyboard::layouts::{AnyLayout, De105Key};
use pc_keyboard::{DecodedKey, Error as PcError, HandleControl, KeyEvent, Keyboard as PcKeyboard, ScancodeSet1, ScancodeSet2};
use spin::{Mutex, MutexGuard};
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
    decoder: Mutex<KeyboardDecoder>,
}

enum KeyboardDecoder {
    Set1(PcKeyboard<AnyLayout, ScancodeSet1>),
    Set2(PcKeyboard<AnyLayout, ScancodeSet2>),
}

impl KeyboardDecoder {
    fn add_byte(&mut self, byte: u8) -> Result<Option<KeyEvent>, PcError> {
        match self {
            KeyboardDecoder::Set1(keyboard) => keyboard.add_byte(byte),
            KeyboardDecoder::Set2(keyboard) => keyboard.add_byte(byte),
        }
    }
    
    fn process_keyevent(&mut self, ev: KeyEvent) -> Option<DecodedKey> {
        match self {
            KeyboardDecoder::Set1(keyboard) => keyboard.process_keyevent(ev),
            KeyboardDecoder::Set2(keyboard) => keyboard.process_keyevent(ev),
        }
    }
}

struct KeyboardInterruptHandler {
    keyboard: Arc<Keyboard>,
}

impl Keyboard {
    fn new(controller: Arc<Mutex<Controller>>, buffer_cap: usize, scancode_set: u8) -> Result<Self, KeyboardError> {
        let decoder = match scancode_set {
            1 => KeyboardDecoder::Set1(PcKeyboard::new(
                ScancodeSet1::new(),
                AnyLayout::De105Key(De105Key),
                HandleControl::Ignore,
            )),
            2 => KeyboardDecoder::Set2(PcKeyboard::new(
                ScancodeSet2::new(),
                AnyLayout::De105Key(De105Key),
                HandleControl::Ignore,
            )),
            s => {
                error!("invalid scancode set: {s}");
                return Err(KeyboardError::KeyDetectionError)
            },
        };
        Ok(Self {
            controller,
            buffer: mpmc::bounded::scq::queue(buffer_cap),
            decoder: Mutex::new(decoder),
        })
    }

    pub fn plugin(keyboard: Arc<Keyboard>) {
        interrupt_dispatcher().assign(InterruptVector::Keyboard, Box::new(KeyboardInterruptHandler::new(Arc::clone(&keyboard))));
        apic().allow(InterruptVector::Keyboard);
    }
    
    /// Parse a byte and get the next key event.
    /// 
    /// This also returns the held decoder lock, in case you want to decode the key event.
    fn get_next_keyevent(&self) -> (MutexGuard<'_, KeyboardDecoder>, Option<KeyEvent>) {
        let mut decoder = self.decoder.lock();

        let scancode = match self.buffer.0.try_dequeue() {
            Ok(code) => code,
            Err(DequeueError::Closed) => panic!("Keyboard stream closed!"),
            Err(DequeueError::Empty) => return (decoder, None),
        };
        let key_event = decoder.add_byte(scancode).unwrap();
        (decoder, key_event)
    }
}

impl DecodedInputStream for Keyboard {
    fn decoded_read_byte(&self) -> i16 {
        loop {
            if let Some(byte) = self.decoded_try_read_byte() {
                return byte
            }
        }
    }

    fn decoded_try_read_byte(&self) -> Option<i16> {
        let (mut decoder, key_event) = self.get_next_keyevent();

        if let Some(event) = key_event {
            let key = decoder.process_keyevent(event)?;
            return match key {
                DecodedKey::Unicode(c) => Some(c as i16),
                _ => None,
            };
        } else {
            None
        }
    }
}

impl RawInputStream for Keyboard {
    fn read_event(&self) -> KeyEvent {
        loop {
            match self.read_event_nb() {
                Some(code) => return code,
                None => {}
            }
        }
    }
    
    fn read_event_nb(&self) -> Option<KeyEvent> {
        let (_decoder, key_event) = self.get_next_keyevent();
        key_event
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

pub struct Mouse {
    controller: Arc<Mutex<Controller>>,
    buffer: (mpmc::bounded::scq::Receiver<u32>, mpmc::bounded::scq::Sender<u32>),
    mouse_type: MouseType,
}

#[derive(Default)]
struct MouseState {
    cycle: i8,
    packet: u32
}

struct MouseInterruptHandler {
    mouse: Arc<Mouse>,
    state: Mutex<MouseState>
}

impl Mouse {
    fn new(controller: Arc<Mutex<Controller>>, buffer_cap: usize, mouse_type: MouseType) -> Self {
        Self {
            controller,
            buffer: mpmc::bounded::scq::queue(buffer_cap),
            mouse_type,
        }
    }

    pub fn plugin(mouse: Arc<Mouse>) {
        interrupt_dispatcher().assign(InterruptVector::Mouse, Box::new(MouseInterruptHandler::new(Arc::clone(&mouse))));
        apic().allow(InterruptVector::Mouse);
    }

    pub fn read(&self) -> Option<u32> {
        match self.buffer.0.try_dequeue() {
            Ok(data) => Some(data),
            Err(DequeueError::Closed) => panic!("Mouse stream closed!"),
            Err(_) => None
        }
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
                        // Verify the always-one bit of the first packet
                        if data & 0x08 == 0 {
                            debug!("Mouse: Discarding invalid first byte");
                            return;
                        }

                        // Read first byte (flags)
                        mouse_state.packet = 0x0;
                        mouse_state.packet |= (data as u32) << 0;

                        mouse_state.cycle += 1;
                    },

                    1 => {
                        // Read second byte (delta x)
                        mouse_state.packet |= (data as u32) << 8;

                        mouse_state.cycle += 1;
                    }

                    2 => {
                        // Read third byte (delta y)
                        mouse_state.packet |= (data as u32) << 16;

                        // IntelliMouse sends another 4th byte
                        if self.mouse.mouse_type == MouseType::IntelliMouse
                            || self.mouse.mouse_type == MouseType::IntelliMouseExplorer {
                            mouse_state.cycle += 1;
                        } else {
                            // Enqueue the packet
                            while self.mouse.buffer.1.try_enqueue(mouse_state.packet).is_err() {
                                if self.mouse.buffer.0.try_dequeue().is_err() {
                                    panic!("Mouse: Failed to store received packet in buffer!");
                                }
                            }

                            mouse_state.cycle = 0;
                        }
                    }

                    3 => {
                        // Read fourth byte (IntelliMouse / IntelliMouse Explorer)
                        mouse_state.packet |= (data as u32) << 24;

                        // Discard ign extension, so it doesn't mess with button4/5 (IntelliMouse)
                        if self.mouse.mouse_type == MouseType::IntelliMouse {
                            mouse_state.packet &= 0x0F_FF_FF_FF;
                        }

                        // Enqueue the packet
                        while self.mouse.buffer.1.try_enqueue(mouse_state.packet).is_err() {
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
        info!("   Initializing controller");
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
        info!("   Self test result is OK");

        // Check if the controller has reset itself during the self test and if so, write the configuration byte again
        if controller.read_config()? != config {
            controller.write_config(config)?;
        }

        // Check if keyboard is present
        let test_result = controller.test_keyboard();
        if test_result.is_ok() {
            // Enable keyboard
            info!("   First port detected");
            controller.enable_keyboard()?;
            config.set(ControllerConfigFlags::DISABLE_KEYBOARD, false);
            config.set(ControllerConfigFlags::ENABLE_KEYBOARD_INTERRUPT, true);
            controller.write_config(config)?;
            info!("   First port enabled");
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
        info!("   Initializing keyboard");
        let mut controller = self.controller.lock();

        // Perform self test on keyboard
        controller.keyboard().reset_and_self_test()?;
        info!("   Keyboard has been reset and self test result is OK");

        // Enable keyboard translation if needed
        controller.keyboard().disable_scanning()?;
        let kb_type = controller.keyboard().get_keyboard_type()?;
        info!("   Detected keyboard type [{kb_type:?}]");

        match kb_type {
            KeyboardType::ATWithTranslation | KeyboardType::MF2WithTranslation | KeyboardType::ThinkPadWithTranslation => {
                info!("   Enabling keyboard translation");
                let mut config = controller.read_config()?;
                config.set(ControllerConfigFlags::ENABLE_TRANSLATE, true);
                controller.write_config(config)?;
            }
            _ => info!("   Keyboard does not need translation"),
        }
        let scancode_set = controller.keyboard().get_scancode_set()?;
        info!("   Keyboard uses scancode set {scancode_set}");

        // Setup keyboard
        info!("   Enabling keyboard");
        debug!("     Setting defaults");
        controller.keyboard().set_defaults()?;
        debug!("     Setting rate and delay");
        controller.keyboard().set_typematic_rate_and_delay(0)?;
        debug!("     Setting leds");
        controller.keyboard().set_leds(KeyboardLedFlags::empty())?;
        debug!("     Enabling scanning");
        controller.keyboard().enable_scanning()?;
        
        let keyboard = Keyboard::new(
            Arc::clone(&self.controller),
            KEYBOARD_BUFFER_CAPACITY,
            scancode_set,
        )?;
        self.keyboard.call_once(|| Arc::new(keyboard));

        Ok(())
    }

    fn enable_scroll_wheel(mouse: &mut ps2::Mouse) -> Result<MouseType, MouseError> {
        mouse.set_sample_rate(200)?;
        mouse.set_sample_rate(100)?;
        mouse.set_sample_rate(80)?;

        // Retrieve mouse type
        mouse.disable_data_reporting()?;
        let mouse_type = mouse.get_mouse_type()?;
        Ok(mouse_type)
    }

    fn enable_extra_buttons(mouse: &mut ps2::Mouse) -> Result<MouseType, MouseError> {
        mouse.set_sample_rate(200)?;
        mouse.set_sample_rate(200)?;
        mouse.set_sample_rate(80)?;

        // Retrieve mouse type
        mouse.disable_data_reporting()?;
        let mouse_type = mouse.get_mouse_type()?;
        Ok(mouse_type)
    }

    pub fn init_mouse(&mut self) -> Result<(), MouseError> {
        info!("Initializing Mouse");
        let mut controller = self.controller.lock();

        // Perform self test on mouse
        controller.mouse().reset_and_self_test()?;
        info!("Mouse has been reset and self test result is OK");
        
        // Try to enable scroll wheel
        let mut mouse_type = Self::enable_scroll_wheel(&mut controller.mouse())?;
        
        // Try to enable extra buttons
        if let Ok(new_type) = Self::enable_extra_buttons(&mut controller.mouse()) {
            if new_type == MouseType::IntelliMouseExplorer {
                mouse_type = new_type;
            }
        }

        info!("Detected mouse type [{:?}]", mouse_type);

        // Setup mouse
        info!("Enabling mouse");
        controller.mouse().set_defaults()?;
        //controller.mouse().set_resolution(2)?;
        //controller.mouse().set_sample_rate(10)?;
        //controller.mouse().set_scaling_one_to_one()?;
        //controller.mouse().set_stream_mode()?;
        controller.mouse().enable_data_reporting()?;

        self.mouse.call_once(|| {
            Arc::new(Mouse::new(Arc::clone(&self.controller), MOUSE_BUFFER_CAPACITY, mouse_type))
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
