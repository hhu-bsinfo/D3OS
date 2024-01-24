# hhuTOSr
HHU Teaching Operating System written in Rust.

HhuTOSr is derived from Philipp Oppermann’s [excellent series of blog posts](https://os.phil-opp.com/).

## Requirements

For building hhuTOSr, a _rust_ toolchain is needed. To install _rust_ use [rustup](https://rustup.rs/). 
HhuTOSr requires some features of nightly rust. Use the following command to install it:

`rustup toolchain install nightly`

And activate it for the current folder with:

`rustup override set nightly`

To run the build commands _cargo-make_ is required. Install it with:

`cargo install cargo-make`

Further the following packages for Debian/Ubuntu based systems (or their equivalent packages on other distributions) need to be installed:

`apt install build-essential nasm mtools fdisk zstd `

To run the final OS image _QEMU_ is required:

`apt install qemu-system-x86_64 ovmf`

## Build

For a full build run: 

`cargo make`

This will produce _hhuTOSr.img_.

## Run

To run the image, build it first and then use:

`./run.sh`

This will execute the operating system with _QEMU_.

For more information see `run.sh --help`.
