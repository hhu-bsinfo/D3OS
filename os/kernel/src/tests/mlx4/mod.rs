mod ibstat;
mod rdma;
mod transfer;
#[cfg(kernel_bench)]
mod bench;

use ibstat::invoke as ibstat_invoke;
use crate::tests::mlx4::rdma::rdma_read::invoke as rdma_read_invoke;
use crate::tests::mlx4::rdma::rdma_write::invoke as rdma_write_invoke;
use super::test_runner::TestPlugin;
use crate::scheduler;
use crate::process::thread::Thread;

pub(super) struct Mlx4Plugin {}

impl TestPlugin for Mlx4Plugin {
    fn run(&self) -> Self::Output {
        #[cfg(stat)]
        scheduler().ready(Thread::new_kernel_thread(|| {
            ibstat_invoke()
        }, "ibstat"));

        #[cfg(read)]
        scheduler().ready(Thread::new_kernel_thread(|| {
            rdma_read_invoke()
        }, "rdma_read_test"));

        #[cfg(write)]
        scheduler().ready(Thread::new_kernel_thread(|| {
            rdma_write_invoke()
        }, "rdma_write_test"));
    }
}