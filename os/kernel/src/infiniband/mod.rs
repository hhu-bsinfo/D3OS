use log::{info, trace};

pub mod ib_core;
pub mod ibverbs_sys;
pub mod ibverbs;

use crate::pci_bus;

#[cfg(feature = "infiniband_mlx4")]
use crate::device::mlx4::ConnectX3Nic;

// add new card by specifying corresponding init with feature

#[cfg(feature = "infiniband_mlx4")]
fn _init() {
    use crate::device::mlx4;

    let devices = pci_bus()
        .search_by_ids(
            mlx4::MLX_VEND,
            mlx4::CONNECTX3_DEV);
    if !devices.is_empty() {
        let device = devices[0];

        info!("Found ConnectX-3 card !");

        let _ = ConnectX3Nic::init(device);
    }

    else { trace!("No ConnectX-3 card found !"); }
}

#[cfg(feature = "infiniband_mlx5")]
fn _init() {}


#[cfg(not(any(feature = "infiniband_mlx4", feature = "infiniband_mlx5")))]
fn _init() {
    warn!("Init routine for hw-device won't be triggered. No binding for specfied feature or missing to specify one.")
}

pub fn init() {
   _init();
}