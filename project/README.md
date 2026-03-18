<p align="center">
  <a href="https://www.uni-duesseldorf.de/home/en/home.html"><img src="media/d3os.png" width=460></a>
</p>

**A new distributed operating system for data centers, developed by the [operating systems group](https://www.cs.hhu.de/en/research-groups/operating-systems.html) of the department of computer science at [Heinrich Heine University Düsseldorf](https://www.hhu.de)**

<p align="center">
  <a href="https://www.uni-duesseldorf.de/home/en/home.html"><img src="media/hhu.svg" width=300></a>
</p>

<p align="center">
  <a href="https://github.com/hhu-bsinfo/D3OS/actions/workflows/build.yml"><img src="https://github.com/hhu-bsinfo/D3OS/actions/workflows/build.yml/badge.svg"></a>
  <img src="https://img.shields.io/badge/Rust-2024-blue.svg">
  <img src="https://img.shields.io/badge/license-GPLv3-orange.svg">
</p>

## Requirements

For building D3OS, the following packages for Debian/Ubuntu based systems (or their equivalent packages on other distributions) need to be installed:
```bash
apt install rustup build-essential nasm dosfstools wget qemu-system-x86
```

This has been tested on Ubuntu 24.04.

For macOS, the same can be achieved with:
```bash
xcode-select --install
brew install rustup dosfstools nasm x86_64-elf-gcc gnu-tar wget qemu
brew link --force rustup
```

This has been tested on macOS 14.

[rustup](https://rustup.rs/) will download a _rust nightly_ toolchain on the first compile.

To run the build, the commands _cargo-make_ and _cargo-license_ are required. Install them with:
```bash
cargo install --no-default-features cargo-make cargo-license
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

## Debugging 

### In a terminal with gdb

Open a terminal and compile and start D3OS in `qemu` halted by `gdb` with the following commands:
```bash
cargo make --no-workspace clean
cargo make --no-workspace debug
```

Open another terminal and start `gdb` with:
```bash
cargo make --no-workspace gdb
```
This will fire booting D3OS and stop in `boot.rs::start`.

Setting a breakpoint in `gdb`:
```bash
break kernel::naming::api::init
```
For further commands check [GDB Quick Reference](docs/gdb-commands.pdf).

## Creating a bootable USB stick

### Using towboot
D3OS uses [towboot](https://github.com/hhuOS/towboot) which is already installed after you have successfully compiled D3OS. 

Use following command (in the D3OS directory) to create a bootable media for the device referenced by `/mnt/external`

`$ towbootctl install /mnt/external --removable -- -config towboot.toml`

### Using balenaEtcher
Write the file `d3os.img` using [balenaEtcher](https://etcher.balena.io) to your USB stick.

## Passing an existing PCI device to the VM

To use a real device with QEMU, change the Makefile so that it uses `${CARGO_MAKE_WORKSPACE_WORKING_DIRECTORY}/qemu-pci.sh` instead of `qemu-system-x86_64`.
Also take a look at that script and fill in the constants at the top.

If you want to run D3OS on a different device, build with `cargo make --no-workspace image` and copy over `qemu-pci.sh`, `RELEASEX64_OVMF.fd` and `d3os.img`.
Run it with `./qemu-pci.sh -bios RELEASEX64_OVMF.fd -hda d3os.img`.
