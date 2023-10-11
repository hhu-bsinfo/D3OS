use pic8259::ChainedPics;
use spin::Mutex;
use crate::kernel::int_disp::InterruptVector;

const PIC_OFFSET: u8 = 0x20;

static PICS: Mutex<ChainedPics> = Mutex::new(unsafe { ChainedPics::new(PIC_OFFSET, PIC_OFFSET + 8) });

pub fn init() {
    let mut pics = PICS.lock();
    unsafe {
        pics.initialize();
        pics.write_masks(0xff, 0xff);
    }
}

pub fn allow(vector: InterruptVector) {
    let mut pics = PICS.lock();

    unsafe {
        let masks = pics.read_masks();
        let vector_masks = gen_masks(vector);

        pics.write_masks(masks[0] & !vector_masks[0], masks[1] & !vector_masks[1]);
    }
}

pub fn send_eoi(vector: InterruptVector) {
    unsafe {
        PICS.force_unlock();
        PICS.lock().notify_end_of_interrupt(vector as u8);
    }
}

fn gen_masks(vector: InterruptVector) -> [u8; 2] {
    let int = vector as u8 - PIC_OFFSET;
    let mut masks = [0, 0];

    if vector < InterruptVector::Rtc {
        masks[0] = 1 << int;
    } else {
        masks[1] = 1 << (int - 8);
    }

    return masks;
}