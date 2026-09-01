use crate::interrupt::interrupt_handler::InterruptHandler;
use super::{virtio_gpu, GPU_QUEUE_PENDING, GPU_CONFIG_PENDING, virtio_input, virtio_rng, virtio_sound};
use log::{debug};
use virtio::transport::InterruptStatus;
use core::sync::atomic::Ordering;

pub struct VirtioInterruptHandler;

impl InterruptHandler for VirtioInterruptHandler {
    fn trigger(&self) {
        //GPU Handler
        if let Some(gpu) = virtio_gpu() {
            if let Some(mut g) = gpu.try_lock() {

                let st = g.ack_interrupt();

                if st.contains(InterruptStatus::QUEUE_INTERRUPT) {
                    GPU_QUEUE_PENDING.store(true, Ordering::Release);
                }
                if st.contains(InterruptStatus::DEVICE_CONFIGURATION_INTERRUPT) {
                    GPU_CONFIG_PENDING.store(true, Ordering::Release);
                }

                if !st.is_empty() {
                    let _q = st.contains(InterruptStatus::QUEUE_INTERRUPT);
                    let _c = st.contains(InterruptStatus::DEVICE_CONFIGURATION_INTERRUPT);
                    //info!("virtio-gpu irq: queue={}, config={}", q, c);
                }
            }
        }
        // Input Handler
        if let Some(input_dev) = virtio_input() {
            if let Some(mut driver) = input_dev.try_lock() {
                if !driver.ack_interrupt().is_empty() {
                    //VIRTIO_INPUT_PENDING.store(true, Ordering::Release);
                    debug!("virtio-input irq acknowledged.");
                    // while let Some(event) = driver.pop_pending_event() {
                    //     info!("VirtIO Input Event: type={}, code={}, value={}", event.event_type, event.code, event.value);
                    // }
                }
            }
        }
        // RNG Handler
        if let Some(rng_dev) = virtio_rng() {
            if let Some(mut r) = rng_dev.try_lock() {
                let st = r.ack_interrupt();
                if !st.is_empty() {
                    debug!("virtio-rng irq acknowledged.");
                }
            }
        }
        // Sound Handler
        if let Some(sound_dev) = virtio_sound() {
            if let Some(mut sound_driver) = sound_dev.try_lock() {
                let _status = sound_driver.ack_interrupt();
                // if !status.is_empty() {
                //     info!("virtio-sound irq acknowledged.");
                // }
            }
        }
        // Socket Handler - nutzt polling
    }
}
