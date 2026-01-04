# Contributing to NexusOS

## Tech Stack Enforcement

To maintain the integrity and vision of NexusOS, contributions must adhere to the following stack:

- **Kernel Core**: **Rust** (Stable or Nightly as specified in `rust-toolchain.toml`).
- **AI/ML**: **Candle** (Hugging Face) or **Burn**. No Python-based ML libraries.
- **Storage**: **Qdrant** (Rust client).

## Strict Constraints

### 1. #![no_std] is Law
The core kernel and "LibOS" must strictly adhere to `#![no_std]`.
- **Reason**: We are running on bare metal. There is no Operating System underneath us.
- **Consequence**: You cannot use `std::fs`, `std::net`, or `std::thread`. You must use `core` and `alloc`, or our own `nexus_std` crate.

### 2. No libc
We do not link against `libc`. Any dependency that requires `libc` to compile is **banned** from the kernel layer.

## Development Environment

### Mandated OS
All development and compilation must be done on **Linux**.
- **Windows Users**: You **MUST** use **WSL2** running **Ubuntu 24.04** (or equivalent).
- **Mac Users**: You must use a Linux VM (e.g., via UTM or QEMU) to compile the kernel, as macOS Mach-O binary formats and linker behaviors differ significantly from the ELF targets we require.

### Setup
1. **Install Rust**: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
2. **Install Target**: `rustup target add x86_64-unknown-none`
3. **Install QEMU**: `sudo apt install qemu-system-x86`

---

*By contributing, you agree that your code becomes part of the NexusOS Hyper-Unikernel and may be compiled into a single static binary.*
