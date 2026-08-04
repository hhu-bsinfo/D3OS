## Fix error when running multiple benchmarks
```shell
[nix-shell:~/rust-perftest]$ cargo run ib2
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.36s
Running `target/debug/rust-perftest ib2`

RDMA_Read BW Test
Connecting to ib2:18515

#bytes   #iterations      BW avg[Gb/sec]   MsgRate[Mpps]
65536          1000                1.10        0.002099

[nix-shell:~/rust-perftest]$ cargo run ib2
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.04s
Running `target/debug/rust-perftest ib2`

RDMA_Read BW Test
Connecting to ib2:18515

#bytes   #iterations      BW avg[Gb/sec]   MsgRate[Mpps]
Error: WC error: IBV_WC_RETRY_EXC_ERR vendor_err=129
```