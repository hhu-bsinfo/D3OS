# D3OS
A new research operating system, developed by the [operating systems group](https://www.cs.hhu.de/en/research-groups/operating-systems.html) of the department of computer science at [Heinrich Heine University Düsseldorf](https://www.hhu.de)

<p align="center">
  <a href="https://www.uni-duesseldorf.de/home/en/home.html"><img src="media/hhu.svg" width=300></a>
</p>

<p align="center">
  <a href="https://github.com/hhu-bsinfo/D3OS/actions/workflows/build.yml"><img src="https://github.com/hhu-bsinfo/D3OS/actions/workflows/build.yml/badge.svg"></a>
  <img src="https://img.shields.io/badge/Rust-2021-blue.svg">
  <img src="https://img.shields.io/badge/license-GPLv3-orange.svg">
</p>

## Requirements

For building D3OS, a _rust nightly_ toolchain is needed. To install _rust_ use [rustup](https://rustup.rs/).

```
rustup toolchain install nightly
rustup override set nightly
```

To run the build commands _cargo-make_ is required. Install it with:

```
cargo install --no-default-features cargo-make
```

Further the following packages for Debian/Ubuntu based systems (or their equivalent packages on other distributions) need to be installed:

```
apt install build-essential nasm wget qemu-system-x86_64
```

D3OS depends on the rust-based bootloader [towboot](https://github.com/hhuOS/towboot), which provides the `towbootctl` utility for creating bootable images. Install it with: 

```
rustup target install i686-unknown-uefi x86_64-unknown-uefi
cargo install --git https://github.com/hhuOS/towboot --features=binary -Z bindeps towbootctl
```

## Build and Run

To build D3OS and run it in QEMU, just execute: 

```
cargo make --no-workspace
```

To only build the bootable image _d3os.img_, run:

```
cargo make --no-workspace image
```
