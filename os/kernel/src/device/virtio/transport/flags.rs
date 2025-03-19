/// Virtual I/O Device (VIRTIO) Version 1.3, section 2.1: Device Status Field
/// https://docs.oasis-open.org/virtio/virtio/v1.3/virtio-v1.3.pdf#page=15
bitflags::bitflags! {
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

/// Virtual I/O Device (VIRTIO) Version 1.3, section 6: Reserved Feature Bits
/// https://docs.oasis-open.org/virtio/virtio/v1.3/virtio-v1.3.pdf#page=253
bitflags::bitflags! {
    #[derive(Debug, Copy, Clone, PartialEq)]
    #[repr(transparent)]
    /// VirtIO feature bits
    pub struct VirtioFeatures: u64 {
        /// Negotiating this feature indicates that the driver can use descriptors
        /// with the VIRTQ_DESC_F_INDIRECT flag set as described in 2.7.5.3 Indirect
        /// Descriptors and 2.8.7 Indirect Flag: Scatter-Gather Support.
        const VIRTIO_F_INDIRECT_DESC = 1 << 28;

        /// This feature enables the used_event and the avail_event fields as
        /// described in 2.7.7, 2.7.8 and 2.8.10.
        const VIRTIO_F_EVENT_IDX = 1 << 29;

        /// This indicates compliance with this specification, giving a simple way
        /// to detect legacy devices or drivers.
        const VIRTIO_F_VERSION_1 = 1 << 32;

        /// This feature indicates that the device can be used on a platform where device
        /// access to data in memory is limited and/or translated. E.g. this is the case
        /// if the device can be located behind an IOMMU that translates bus addresses
        /// from the device into physical addresses in memory, if the device can be limited
        /// to only access certain memory addresses or if special commands such as a cache
        /// flush can be needed to synchronise data in memory with the device. Whether
        /// accesses are actually limited or translated is described by platform-specific
        /// means. If this feature bit is set to 0, then the device has same access to
        /// memory addresses supplied to it as the driver has. In particular, the device
        /// will always use physical addresses matching addresses used by the driver
        /// (typically meaning physical addresses used by the CPU) and not translated
        /// further, and can access any address supplied to it by the driver. When clear,
        /// this overrides any platform-specific description of whether device access is
        /// limited or translated in any way, e.g. whether an IOMMU may be present.
        const VIRTIO_F_ACCESS_PLATFORM = 1 << 33;

        /// This feature indicates support for the packed virtqueue layout as described
        /// in 2.8 Packed Virtqueues.
        const VIRTIO_F_RING_PACKED = 1 << 34;

        /// This feature indicates that all buffers are used by the device in the same order
        /// in which they have been made available.
        const VIRTIO_F_IN_ORDER = 1 << 35;

        /// This feature indicates that memory accesses by the driver and the device are
        /// ordered in a way described by the platform.
        /// If this feature bit is negotiated, the ordering in effect for any memory
        /// accesses by the driver that need to be ordered in a specific way with respect
        /// to accesses by the device is the one suitable for devices described by the
        /// platform. This implies that the driver needs to use memory barriers suitable
        /// for devices described by the platform; e.g. for the PCI transport in the case
        /// of hardware PCI devices.
        ///
        /// If this feature bit is not negotiated, then the device and driver are assumed
        /// to be implemented in software, that is they can be assumed to run on identical
        /// CPUs in an SMP configuration. Thus a weaker form of memory barriers is sufficient
        /// to yield better performance.
        const VIRTIO_F_ORDER_PLATFORM = 1 << 36;

        /// This feature indicates that the device supports Single Root I/O Virtualization.
        /// Currently only PCI devices support this feature.
        const VIRTIO_F_SR_IOV = 1 << 37;

        /// This feature indicates that the driver passes extra data (besides identifying
        /// the virtqueue) in its device notifications. See 2.9 Driver Notifications.
        const VIRTIO_F_NOTIFICATION_DATA = 1 << 38;

        /// This feature indicates that the driver uses the data provided by the device as
        /// a virtqueue identifier in available buffer notifications. As mentioned in section
        /// 2.9, when the driver is required to send an available buffer notification to the
        /// device, it sends the virtqueue number to be notified. The method of delivering
        /// notifications is transport specific. With the PCI transport, the device can
        /// optionally provide a per-virtqueue value for the driver to use in driver
        /// notifications, instead of the virtqueue number. Some devices may benefit from this
        /// flexibility by providing, for example, an internal virtqueue identifier, or an
        /// internal offset related to the virtqueue number.
        ///
        /// This feature indicates the availability of such value. The definition of the data
        /// to be provided in driver notification and the delivery method is transport
        /// specific. For more details about driver notifications over PCI see 4.1.5.2.
        const VIRTIO_F_NOTIF_CONFIG_DATA = 1 << 39;

        /// This feature indicates that the driver can reset a queue individually. See 2.6.1.
        const VIRTIO_F_RING_RESET = 1 << 40;
    }
}
