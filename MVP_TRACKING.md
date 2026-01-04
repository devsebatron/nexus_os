# NexusOS MVP Tracking

## 1. Project Dashboard

**Current Status:** `[==........] 20%`

**Active Milestone:** 🟢 **M1: The Awakening**

**Next Big Goal:** Boot into a shell with memory management.

---

## 2. Milestone Breakdown (Granular Tasks)

### M1: The Awakening (Boot & Output)
- [ ] Initialize `no_std` crate structure.
- [ ] Implement VGA Driver (`0xb8000` safe wrapper).
- [ ] Implement Global `WRITER` with `spin::Mutex`.
- [ ] Implement `panic_handler` with line number output.
- [ ] Configure `bootimage` runner in `.cargo/config.toml`.
- [ ] **Verification:** "Hello NexusOS" visible in QEMU.

### M2: The Memory Architect (Allocation)
- [ ] Parse Multiboot2 Memory Map (Identify free RAM).
- [ ] Implement Physical Frame Allocator (4KiB chunks).
- [ ] Implement Virtual Memory (Recursive/Offset Page Tables).
- [ ] Initialize Heap (Range: `0x4444_4444_0000`).
- [ ] Implement `GlobalAlloc` trait (Bump Allocator first).
- [ ] **Verification:** `Box::new(42)` and `Vec::push(1)` do not crash.

### M3: The Hypervisor Core (Multitasking)
- [ ] Create `Task` struct (Future polling logic).
- [ ] Implement Simple Executor (Round-Robin loop).
- [ ] Define `Waker` logic to handle sleeping tasks.
- [ ] Set up IDT (Interrupt Descriptor Table).
- [ ] Enable Hardware Interrupts (PIC/APIC) for Keyboard.
- [ ] **Verification:** Two async print tasks run concurrently.

### M4: The Cortex Integration (AI)
- [ ] Enable CPU Features (SSE/AVX) in `x86_64` config.
- [ ] Port `candle-core` to `no_std` (or create dummy backend).
- [ ] Hardcode a "dummy" quantized model weight set.
- [ ] Implement `infer(input)` function.
- [ ] **Verification:** `kernel_main` calls AI and gets a result string.

### M5: MemexFS Foundation (Storage)
- [ ] Enumerate PCI bus to find NVMe Controller.
- [ ] Write Polling NVMe Driver (Admin Queue init).
- [ ] Implement `read_block` / `write_block`.
- [ ] Port HNSW (Vector Index) logic to heap.

### M6: The Interface (Shell)
- [ ] Switch to Graphics Output Protocol (Framebuffer).
- [ ] Render Text Rendering primitive (Font bitmap).
- [ ] Implement Shell Loop (print "> ", `read_line`, `eval`).

---

## 3. Legend & Workflow

### Status Legend
✅ = Merged to Main
🚧 = In Progress
🛑 = Blocked

### Workflow Reminder
> Check off items in the IDE preview or on GitHub after merging a feature branch.
