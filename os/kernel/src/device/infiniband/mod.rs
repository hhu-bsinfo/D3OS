#[cfg(feature = "infiniband_mlx4")]
pub mod mlx4;
#[cfg(feature = "infiniband_mlx4")]
pub mod uverbs;
#[cfg(feature = "infiniband_mlx4")]
pub mod uverbs_cmd;

// add new card by specifying corresponding init with feature

#[cfg(feature = "infiniband_mlx4")]
fn init_mlx4() {
    use crate::pci_bus;
    use log::{info, trace};

    let devices = pci_bus().search_by_ids(mlx4::MLX_VEND, mlx4::CONNECTX3_DEV);
    for (i, dev) in devices.iter().enumerate() {
        info!("Found ConnectX-3 card ! - dev : {}", i);

        let minor = mlx4::Mlx4Device::init(dev).expect("error in x3 init");
        info!("Initialized mlx4 driver, associated dev {} with minor => {}", i, minor);
    }

    if devices.is_empty() {
        trace!("No ConnectX-3 card found !");
    }
}

pub fn init() {
    #[cfg(feature = "infiniband_mlx4")]
    init_mlx4()
}
