# D3OS
A new research operating system, developed by the [operating systems group](https://www.cs.hhu.de/en/research-groups/operating-systems.html) of the department of computer science at [Heinrich Heine University Düsseldorf](https://www.hhu.de)

## Requirements

For building D3OS, a _rust nightly_ toolchain is needed. To install _rust_ use [rustup](https://rustup.rs/).

`rustup toolchain install nightly`

And activate it for the current folder with:

`rustup override set nightly`

To run the build commands _cargo-make_ is required. Install it with:

`cargo install --no-default-features cargo-make`

Further the following packages for Debian/Ubuntu based systems (or their equivalent packages on other distributions) need to be installed:

`apt install build-essential nasm mtools fdisk zstd`

To run the final OS image _QEMU_ is required:

`apt install qemu-system-x86_64`

## Build

For a full build run: 

`cargo make --no-workspace`

This will produce _d3os.img_.

## Run

To run the image, build it first and then use:

`./run.sh`

This will execute the operating system with _QEMU_.

For more information see `run.sh --help`.
