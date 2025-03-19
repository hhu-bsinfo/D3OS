pub mod queue;
pub mod command;


bitflags::bitflags! {
    /// Virtual I/O Device (VIRTIO) Version 1.3, section 2.1: Device Status Field
    /// https://docs.oasis-open.org/virtio/virtio/v1.1/virtio-v1.1.pdf#page=15
    #[derive(Debug, Copy, Clone, PartialEq)]
    #[repr(transparent)]
    pub struct DeviceStatusFlags: u8 {
        /// Indicates that the guest OS has found the device and recognized it as a valid virtio device.
        const ACKNOWLEDGE = 1;
        /// Indicates that the guest OS knows how to drive the device. Note: There could be a significant (or infinite) delay before setting this bit.
        /// For example, under Linux, drivers can be loadable modules.
        const DRIVER = 2;
        /// Indicates that something went wrong in the guest, and it has given up on the device.
        /// This could be an internal error, or the driver didn’t like the device for some reason, or even a fatal error during device operation.
        const FAILED = 128;
        /// Indicates that the driver has acknowledged all the features it understands, and feature negotiation is complete.
        const FEATURES_OK = 8;
        /// Indicates that the driver is set up and ready to drive the device.
        const DRIVER_OK = 4;
        /// Indicates that the device has experienced an error from which it can’t recover.
        const DEVICE_NEEDS_RESET = 64;
    }
}