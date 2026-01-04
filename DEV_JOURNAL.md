# Developer Journal - NexusOS

## Milestone 1: The Awakening

This journal chronicles the engineering decisions, toolchain setup, and code implementation for NexusOS, a specialized "Hyper-Unikernel" designed for high-performance AI integration.

### 1. The Vision
NexusOS is not a Linux distribution. It is a unikernel written from scratch in Rust, designed to run directly on hardware (bare metal) to eliminate the overhead of general-purpose operating systems.

Our architecture consists of three pillars:
- **NexusOS**: The bare-metal kernel.
- **nexus_cortex**: The AI integration layer.
- **nexus_memex**: The semantic storage engine.

We are building this without the Rust Standard Library (`no_std`), meaning we have no access to `std::cout`, threads, or heap allocation until we build them ourselves.

### 2. Environment Setup (The "Antigravity" Stack)
To reproduce this environment, you need a Linux system (or WSL2) and the Rust nightly toolchain.

**Tools & Requirements**
- **WSL2 (Ubuntu 24.04)**: Our build host.
- **Rust Nightly**: Required for experimental bare-metal features (e.g., `abi_x86_interrupt`, `naked_functions`).
- **QEMU**: The machine emulator used to run our kernel.

**Crate Dependencies:**
- `bootloader_api` (v0.11): Handles the transition from BIOS/UEFI to our Rust code.
- `noto-sans-mono-bitmap`: Provides fonts for the graphical framebuffer.

### 3. Repository Governance
We follow a Trunk-Based Development strategy to ensure high velocity.
- **Main Branch**: Always deployable.
- **CI Pipeline**: Configured in `.github/workflows/ci.yml`. Runs `cargo build` and `cargo test` on every push to verify the code compiles on the nightly toolchain.

### 4. Engineering Milestone 1 (The Awakening)
**The `no_std` Decision**
Operating systems cannot rely on an operating system. We disabled the standard library in `nexus_kernel/src/main.rs`:
```rust
#![no_std]  // Don't link the Rust Standard Library
#![no_main] // Disable all Rust-level entry points
```

**The Panic Handler**
In `no_std`, we must define how the kernel reacts to crashes. Our handler prints the error and freezes the machine:
```rust
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info); // Uses our global logger
    loop {}
}
```

**Evolution: From VGA to Framebuffer**
Initially, we used the legacy VGA Text Buffer at address `0xb8000`. However, modern bootloaders (Bootloader v0.11) initialize the system in a high-resolution Graphical Framebuffer mode.
To support this, we built a custom logging architecture:
- `nexus_kernel/src/logger.rs`: A driver that writes pixels directly to the video memory.
- **Bitmap Fonts**: integrated `noto-sans-mono-bitmap` to render text pixel-by-pixel.
- **Global Interface**: Restored `print!` and `println!` macros to transparently use this graphical writer.

**Boot Configuration**
We utilize a Workspace Structure to separate concerns:
- `nexus_kernel`: The kernel binary (runs on QEMU).
- `nexus_boot`: The host builder (runs on Linux).
The `nexus_boot` crate acts as our build system. It uses the `bootloader` crate to package the kernel into a bootable BIOS/UEFI disk image.

### 5. How to Run (The "Magic" Command)
We consolidated the build and run steps into a single command using our runner crate:
```bash
cargo run --package nexus_boot
```

---

## 2026-01-04: The Cortex (M4) & The Build System War

**Milestone:** M4 (The Cortex Integration)
**Status:** Success

### The Mission
To implement "The Cortex," a kernel-level AI inference engine for NexusOS. The goal was to prove that we could run safe, AVX-accelerated code in Ring-0 without crashing the system, simulating a "Revolutionary" 1.58-bit BitNet architecture.

### Technical Architecture: BitNet b1.58
We simulated a 1.58-bit quantized model using `i8` weights with values `{-1, 0, 1}`.
- **Why?** To demonstrate the architectural intent of extremely low-precision, high-efficiency inference.
- **Mechanism:** A static array `WEIGHTS` holds the quantized values. The `infer` function (in `mod cortex`) performs on-the-fly dequantization (integer to float conversion) and computes the dot product.
- **Acceleration:** The compute function is marked with `#[target_feature(enable = "avx")]`, allowing the compiler to emit AVX instructions for the loop.

### The Challenge: Ring-0 SIMD Safety
**The Risk:** In x86_64, the standard calling convention does not preserve extended vector registers (YMM/ZMM) across interrupts. If an ISR fires while the Cortex Engine is calculating a dot product, the registers could be clobbered, leading to silent data corruption or undefined behavior upon return.
**The Solution:** We implemented `cortex::CortexEngine::infer` inside a `x86_64::instructions::interrupts::without_interrupts` closure. This creates a critical section, effectively disabling hardware interrupts while the AVX unit is active. This is a valid strategy for the MVP, though future iterations will need a full XSAVE/XRSTOR implementation in the scheduler.

### The War Story: Dependency Hell & Build Isolation
The implementation faced significant hurdles with the Rust build system in a mixed environment (Host Bootloader + Bare Metal Kernel).

1.  **The `.cargo/config.toml` Trap:**
    - *Initial State:* A single config at the workspace root forced `x86_64-unknown-none` on EVERYTHING.
    - *Result:* `nexus_boot` (which needs `std`) tried to compile against the bare-metal target and failed spectacularly with missing `std` symbols.
    - *Fix:* I moved the config to `nexus_kernel/.cargo/config.toml`. Ideally, this isolates the configuration. However, running `cargo run` from the workspace root *still* ignored the subdirectory config or applied it incorrectly in some contexts. The key realization was that CLI args override config files.

2.  **The `serde` Regression:**
    - *Issue:* `serde` v1.0.228 introduced a change that broke `no_std` compilation (missing `stringify!` macro imports).
    - *Fix:* We pinned `serde` to v1.0.217 and `serde_json` to v1.0.133 in `Cargo.lock` (via `cargo update`) to bypass the broken nightly build interaction.

3.  **Linker Errors:**
    - *Issue:* The `rust-lld` linker complained about `-nostartfiles` being an unknown argument.
    - *Fix:* Removed the flag from the kernel config. The bare-metal target spec already handles this.

### Outcome
Both the kernel and the bootloader now compile cleanly. The system boots, initializes the Cortex layer, and successfully outputs a "BitNet Activation" score, proving that the AVX instructions executed correctly in the critical section.
