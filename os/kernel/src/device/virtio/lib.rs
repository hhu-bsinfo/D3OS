use crate::memory::PAGE_SIZE;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum VirtioGpuError {
    /// Insufficient descriptors available in the virtqueue. Try again later.
    #[error("Virtqueue lacks available descriptors")]
    QueueExhausted,
    /// Device is not in a ready state.
    #[error("Device is not initialized or ready")]
    Unavailable,
    /// Mismatch between expected and received descriptor chains.
    #[error("Unexpected descriptor chain used by device")]
    DescriptorMismatch,
    /// The queue is currently occupied.
    #[error("Virtqueue is already occupied")]
    QueueOccupied,
    /// Provided parameter is invalid.
    #[error("Invalid input parameter")]
    BadParameter,
    /// DMA memory allocation failure.
    #[error("Unable to allocate DMA memory")]
    DmaFailure,
    /// General I/O operation failure.
    #[error("I/O operation failed")]
    IoFailure,
    /// The device does not support this request.
    #[error("Operation not supported by device")]
    NotSupported,
    /// Device configuration space is smaller than expected.
    #[error("Advertised config space is too small")]
    InsufficientConfigSpace,
    /// The device lacks configuration space, but it was expected.
    #[error("Expected configuration space is missing")]
    MissingConfigSpace,
}

fn align_up(size: usize) -> usize {
    (size + PAGE_SIZE) & !(PAGE_SIZE - 1)
}

fn pages(size: usize) -> usize {
    size.div_ceil(PAGE_SIZE)
}