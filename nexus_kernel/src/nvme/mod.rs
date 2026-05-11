//! NVMe Controller Driver — Admin queue initialisation + Identify Controller.
//!
//! DMA buffers are allocated as raw physical frames and accessed via the
//! bootloader's direct physical-memory map (phys + PHYS_MEM_OFFSET).
//! This avoids the need for page-table walking to find physical addresses.

use core::ptr::{read_volatile, write_volatile};
use core::sync::atomic::{fence, Ordering};
use x86_64::structures::paging::FrameAllocator;

// ── BAR0 register offsets ────────────────────────────────────────────────────
const OFF_CC: usize = 0x14;
const OFF_CSTS: usize = 0x1C;
const OFF_AQA: usize = 0x24;
const OFF_ASQ: usize = 0x28;
const OFF_ACQ: usize = 0x30;

// ── CC bits ───────────────────────────────────────────────────────────────────
const CC_EN: u32 = 1 << 0;
const CC_IOSQES: u32 = 6 << 16; // SQE = 2^6 = 64 bytes
const CC_IOCQES: u32 = 4 << 20; // CQE = 2^4 = 16 bytes

// ── CSTS bits ────────────────────────────────────────────────────────────────
const CSTS_RDY: u32 = 1 << 0;
const CSTS_CFS: u32 = 1 << 1;

const QUEUE_DEPTH: usize = 4;

// ── MMIO helpers ─────────────────────────────────────────────────────────────
unsafe fn rd32(base: u64, off: usize) -> u32 {
    read_volatile((base as usize + off) as *const u32)
}

unsafe fn wr32(base: u64, off: usize, val: u32) {
    write_volatile((base as usize + off) as *mut u32, val);
}

unsafe fn wr64(base: u64, off: usize, val: u64) {
    write_volatile((base as usize + off) as *mut u64, val);
}

// ── DMA frame allocation ──────────────────────────────────────────────────────

struct DmaBuffers {
    sq_phys: u64,
    cq_phys: u64,
    id_phys: u64,
}

/// Allocate three 4 KiB physical frames for DMA and zero them via the direct map.
fn alloc_dma() -> Option<DmaBuffers> {
    let phys_off = crate::PHYS_MEM_OFFSET.load(Ordering::Relaxed);
    let mut guard = crate::FRAME_ALLOCATOR.lock();
    let fa = guard.as_mut()?;

    let sq_phys = fa.allocate_frame()?.start_address().as_u64();
    let cq_phys = fa.allocate_frame()?.start_address().as_u64();
    let id_phys = fa.allocate_frame()?.start_address().as_u64();

    // Zero via direct-mapped virtual addresses
    unsafe {
        core::ptr::write_bytes((sq_phys + phys_off) as *mut u8, 0, 4096);
        core::ptr::write_bytes((cq_phys + phys_off) as *mut u8, 0, 4096);
        core::ptr::write_bytes((id_phys + phys_off) as *mut u8, 0, 4096);
    }

    Some(DmaBuffers {
        sq_phys,
        cq_phys,
        id_phys,
    })
}

// ── Public API ────────────────────────────────────────────────────────────────

pub struct ControllerInfo {
    pub vendor_id: u16,
    pub serial: [u8; 20],
    pub model: [u8; 40],
    pub firmware: [u8; 8],
}

impl ControllerInfo {
    pub fn serial_str(&self) -> &str {
        core::str::from_utf8(&self.serial).unwrap_or("?").trim_end()
    }
    pub fn model_str(&self) -> &str {
        core::str::from_utf8(&self.model).unwrap_or("?").trim_end()
    }
    pub fn firmware_str(&self) -> &str {
        core::str::from_utf8(&self.firmware)
            .unwrap_or("?")
            .trim_end()
    }
}

/// Find the first NVMe controller on the PCI bus, initialise it, and return
/// controller info from the Identify response.
pub fn find_and_init() -> Option<ControllerInfo> {
    let devices = crate::pci::enumerate();
    let dev = devices
        .iter()
        .find(|d| d.class == 0x01 && d.subclass == 0x08)?;

    if dev.bar0 == 0 {
        return None;
    }

    unsafe { init(dev.bar0, dev.vendor_id) }
}

// ── Initialisation sequence ───────────────────────────────────────────────────

unsafe fn init(bar0_phys: u64, vendor_id: u16) -> Option<ControllerInfo> {
    let phys_off = crate::PHYS_MEM_OFFSET.load(Ordering::Relaxed);
    let base = bar0_phys + phys_off; // virtual address of MMIO

    // ── Doorbell stride from CAP[35:32] (high dword bits [3:0]) ──────────────
    let cap_hi = rd32(base, 0x04);
    let dstrd = (cap_hi & 0xF) as usize;
    let db_stride = 4usize << dstrd;

    // ── Disable controller ────────────────────────────────────────────────────
    wr32(base, OFF_CC, rd32(base, OFF_CC) & !CC_EN);
    wait_csts(base, CSTS_RDY, 0)?;

    // ── Allocate DMA frames ───────────────────────────────────────────────────
    let dma = alloc_dma()?;
    let sq_virt = dma.sq_phys + phys_off;
    let cq_virt = dma.cq_phys + phys_off;
    let id_virt = dma.id_phys + phys_off;

    // ── Configure admin queues ────────────────────────────────────────────────
    let aqa = ((QUEUE_DEPTH - 1) as u32) | (((QUEUE_DEPTH - 1) as u32) << 16);
    wr32(base, OFF_AQA, aqa);
    wr64(base, OFF_ASQ, dma.sq_phys);
    wr64(base, OFF_ACQ, dma.cq_phys);

    // ── Enable controller ─────────────────────────────────────────────────────
    wr32(base, OFF_CC, CC_EN | CC_IOSQES | CC_IOCQES);
    wait_csts(base, CSTS_RDY | CSTS_CFS, CSTS_RDY)?;

    // ── Submit Identify Controller (opcode 0x06, CNS = 1) ────────────────────
    let cid: u16 = 1;
    submit_identify(sq_virt, base, db_stride, cid, dma.id_phys);

    // ── Poll CQE phase bit ────────────────────────────────────────────────────
    if !poll_cq(cq_virt, base, db_stride, cid) {
        return None;
    }

    // ── Parse Identify Controller data ───────────────────────────────────────
    // NVMe 1.4 Figure 274:
    //   Bytes  4–23  SN  (Serial Number,   20 B, ASCII)
    //   Bytes 24–63  MN  (Model Number,    40 B, ASCII)
    //   Bytes 64–71  FR  (Firmware Rev.,    8 B, ASCII)
    let id = core::slice::from_raw_parts(id_virt as *const u8, 4096);
    let mut info = ControllerInfo {
        vendor_id,
        serial: [0u8; 20],
        model: [0u8; 40],
        firmware: [0u8; 8],
    };
    info.serial.copy_from_slice(&id[4..24]);
    info.model.copy_from_slice(&id[24..64]);
    info.firmware.copy_from_slice(&id[64..72]);
    Some(info)
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Write a single Identify SQE into the admin submission queue and ring the
/// tail doorbell (admin queue 0: doorbell at BAR0 + 0x1000).
unsafe fn submit_identify(sq_virt: u64, base: u64, _db_stride: usize, cid: u16, prp1: u64) {
    let sq = sq_virt as *mut u32;
    // SQE dword map (NVMe spec Table 20):
    //   DW0  CID[31:16] | OPC[7:0]
    //   DW1  NSID
    //   DW2–DW3  reserved
    //   DW4–DW5  MPTR
    //   DW6–DW7  PRP1
    //   DW8–DW9  PRP2
    //   DW10 CNS (1 = identify controller)
    sq.add(0).write_volatile(((cid as u32) << 16) | 0x06);
    sq.add(1).write_volatile(0); // NSID = 0
    sq.add(2).write_volatile(0);
    sq.add(3).write_volatile(0);
    sq.add(4).write_volatile(0); // MPTR lo
    sq.add(5).write_volatile(0); // MPTR hi
    sq.add(6).write_volatile(prp1 as u32); // PRP1 lo
    sq.add(7).write_volatile((prp1 >> 32) as u32); // PRP1 hi
    sq.add(8).write_volatile(0);
    sq.add(9).write_volatile(0);
    sq.add(10).write_volatile(1); // CDW10: CNS = 1
    for i in 11..16 {
        sq.add(i).write_volatile(0);
    }

    fence(Ordering::Release);

    // Admin SQ tail doorbell: BAR0 + 0x1000 (queue 0, stride irrelevant for q=0)
    write_volatile((base as usize + 0x1000) as *mut u32, 1u32);
}

/// Poll CQE[0] in the admin completion queue for phase bit = 1.
/// Advances the CQ head doorbell on success.
unsafe fn poll_cq(cq_virt: u64, base: u64, db_stride: usize, expected_cid: u16) -> bool {
    // CQE layout (16 bytes):
    //   [0..4]   DW0 result
    //   [4..8]   DW1 reserved
    //   [8..10]  SQHD
    //   [10..12] SQID
    //   [12..14] CID
    //   [14..16] Phase[0] | Status[15:1]
    let cid_ptr = (cq_virt + 12) as *const u16;
    let stat_ptr = (cq_virt + 14) as *const u16;

    for _ in 0..4_000_000u32 {
        let status = read_volatile(stat_ptr);
        if status & 1 != 0 {
            if read_volatile(cid_ptr) != expected_cid {
                return false;
            }
            if (status >> 1) & 0x7FF != 0 {
                return false; // non-zero status code = error
            }
            // Advance CQ head: doorbell at BAR0 + 0x1000 + db_stride
            write_volatile((base as usize + 0x1000 + db_stride) as *mut u32, 1u32);
            return true;
        }
        core::hint::spin_loop();
    }
    false // timeout
}

/// Spin until `(CSTS & mask) == expected`.  Returns `None` on fatal error or
/// timeout.
unsafe fn wait_csts(base: u64, mask: u32, expected: u32) -> Option<()> {
    for _ in 0..4_000_000u32 {
        let csts = rd32(base, OFF_CSTS);
        if csts & CSTS_CFS != 0 {
            return None;
        }
        if csts & mask == expected {
            return Some(());
        }
        core::hint::spin_loop();
    }
    None
}
