<p align="left">
  <a href="https://www.uni-duesseldorf.de/home/en/home.html"><img src="media/d3os.png" width=300></a>
</p>

A new distributed operating system for data centers, developed by the [operating systems group](https://www.cs.hhu.de/en/research-groups/operating-systems.html) of the department of computer science at [Heinrich Heine University Düsseldorf](https://www.hhu.de)

<p align="center">
  <a href="https://www.uni-duesseldorf.de/home/en/home.html"><img src="media/hhu.svg" width=200></a>
</p>

<p align="center">
  <a href="https://github.com/hhu-bsinfo/D3OS/actions/workflows/build.yml"><img src="https://github.com/hhu-bsinfo/D3OS/actions/workflows/build.yml/badge.svg"></a>
  <img src="https://img.shields.io/badge/Rust-2024-blue.svg">
  <img src="https://img.shields.io/badge/license-GPLv3-orange.svg">
</p>

## Requirements

For building D3OS, a _rust nightly_ toolchain is needed. To install _rust_ use [rustup](https://rustup.rs/):
```bash
rustup toolchain install nightly
rustup override set nightly
```

The toolchain `nightly-2024-12-02` is confirmed to work. If you are having problems with new versions, try:
```bash
rustup toolchain install nightly-2024-12-02
rustup override set nightly-2024-12-02
```

To run the build commands _cargo-make_ is required. Install it with:
```bash
cargo install --no-default-features cargo-make
```

Furthermore, the following packages for Debian/Ubuntu based systems (or their equivalent packages on other distributions) need to be installed:
```bash
apt install build-essential nasm dosfstools wget qemu-system-x86_64
```

## Build and Run

To build D3OS and run it in QEMU, just execute:
```bash
cargo make --no-workspace
```

To build a release version of D3OS (much faster) and run it in QEMU, just execute:
```bash
cargo make --no-workspace --profile production
```


To only build the bootable image _d3os.img_, run:
```bash
cargo make --no-workspace image
```
