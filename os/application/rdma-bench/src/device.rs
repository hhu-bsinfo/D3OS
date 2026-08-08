use crate::error::{other, Result};
use ibverbs::Context;

/// Opens the first available RDMA device. D3OS's `Device::name()` is not functional yet (it
/// always returns a fixed placeholder), so there's no reliable way to select a device by name —
/// matches the `perftest`/`rdma/mlx4` precedent of always using the first device.
pub fn open() -> Result<Context> {
    let devices = ibverbs::devices()?;
    let device = devices.iter().next().ok_or_else(|| other("no RDMA device available"))?;
    device.open()
}
