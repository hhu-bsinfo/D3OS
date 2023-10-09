use ps2::{Controller, KeyboardType};
use ps2::flags::{ControllerConfigFlags, KeyboardLedFlags};
use spin::Mutex;

pub static CONTROLLER: Mutex<Controller> = Mutex::new(unsafe { Controller::new() });

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