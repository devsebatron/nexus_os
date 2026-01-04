# NexusOS Roadmap

## Phase 1: Foundation (Current)
**Objective**: Booting a minimal Unikernel on bare metal.
- [ ] **Hypervisor Layer**: Implement a minimal Rust-based Type-1 Hypervisor.
- [ ] **Memory Allocator**: Write a custom allocator suitable for Unikernel environments.
- [ ] **"Hello World"**: Successfully boot a single-task Unikernel that prints to the serial port.

## Phase 2: Cortex
**Objective**: Integration of the AI Engine.
- [ ] **Port BitNet b1.58**: Adapt the 1-bit LLM inference engine for `no_std` Rust.
- [ ] **AVX-512 Optimization**: Hand-tune the matrix addition kernels for specific CPU support.
- [ ] **Freestanding LLM**: Run a port of `llama.cpp` directly on the kernel without a Linux host.

## Phase 3: MemexFS
**Objective**: Semantic Storage Layer.
- [ ] **NVMe Driver**: Implement a polled-mode NVMe driver in Rust for high-throughput I/O.
- [ ] **Qdrant Integration**: Embed the Qdrant vector search logic directly into the storage driver.
- [ ] **Semantic Write**: Implement the flow where writes trigger vector embedding generation.

## Phase 4: Compatibility
**Objective**: Running Legacy Applications.
- [ ] **Syscall Shim**: Create a translation layer that intercepts Linux syscalls from standard ELF binaries.
- [ ] **Runtime Support**: Enable running `node` and `python` binaries by re-routing their OS requests to NexusOS functions.
