//! NVMe Controller Driver — Admin queue initialisation + Identify Controller.
//!
//! Follows NVMe Base Specification 1.4.
//! Operates in polling mode (no MSI/MSI-X).

use core::ptr::{read_volatile, write_volatile};
use core::sync::atomic::{fence, Ordering};

// ── BAR0 register offsets ────────────────────────────────────────────────────
const OFF_CAP_LO: usize = 0x00; // Controller Capabilities (lo 32 bits)
const OFF_CC: usize = 0x14; // Controller Configuration
const OFF_CSTS: usize = 0x1C; // Controller Status
const OFF_AQA: usize = 0x24; // Admin Queue Attributes
const OFF_ASQ: usize = 0x28; // Admin SQ Base Address (64-bit)
const OFF_ACQ: usize = 0x30; // Admin CQ Base Address (64-bit)

// ── CC bit fields ─────────────────────────────────────────────────────────────
const CC_EN: u32 = 1 << 0;
const CC_IOSQES: u32 = 6 << 16; // SQE = 2^6 = 64 bytes
const CC_IOCQES: u32 = 4 << 20; // CQE = 2^4 = 16 bytes

// ── CSTS bit fields ───────────────────────────────────────────────────────────
const CSTS_RDY: u32 = 1 << 0;
const CSTS_CFS: u32 = 1 << 1;

// ── Admin queue depth (minimum allowed by spec is 2) ─────────────────────────
const QDEPTH: usize = 4;

// ── Identify command constants ────────────────────────────────────────────────
const ADM_IDENTIFY: u32 = 0x06;
const CNS_CONTROLLER: u32 = 0x01;

// ── 4 KiB-aligned static DMA buffers ─────────────────────────────────────────
// Each is a full page so physical contiguity is guaranteed within one frame.
#[repr(C, align(4096))]
struct Page([u8; 4096]);

static mut ADMIN_SQ: Page = Page([0u8; 4096]); // 4 × 64-byte SQEs used
static mut ADMIN_CQ: Page = Page([0u8; 4096]); // 4 × 16-byte CQEs used
static mut IDENTIFY: Page = Page([0u8; 4096]); // Identify Controller output

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

/// Convert a virtual address to its physical address using the stored offset.
fn v2p(virt: *const u8) -> u64 {
    virt as u64 - crate::PHYS_MEM_OFFSET.load(Ordering::Relaxed)
}

// ── Public types ──────────────────────────────────────────────────────────────
pub struct ControllerInfo {
    pub serial: [u8; 20],
    pub model: [u8; 40],
    pub firmware: [u8; 8],
    pub vendor_id: u16,
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

// ── Driver entry point ────────────────────────────────────────────────────────

/// Scan PCI for the first NVMe controller, initialise it, run Identify, and
/// return the parsed controller info.  Returns `None` if no NVMe device is
/// found or initialisation fails.
pub fn find_and_init() -> Option<ControllerInfo> {
    // Find NVMe device via PCI (class=0x01, subclass=0x08)
    let devices = crate::pci::enumerate();
    let nvme_dev = devices
        .iter()
        .find(|d| d.class == 0x01 && d.subclass == 0x08)?;

    let bar0_phys = nvme_dev.bar0;
    if bar0_phys == 0 {
        return None;
    }

    unsafe { init(bar0_phys, nvme_dev.vendor_id) }
}

unsafe fn init(bar0_phys: u64, vendor_id: u16) -> Option<ControllerInfo> {
    let phys_off = crate::PHYS_MEM_OFFSET.load(Ordering::Relaxed);
    let base = bar0_phys + phys_off; // virtual address of MMIO

    // ── 1. Read CAP: get doorbell stride ─────────────────────────────────────
    // CAP is a 64-bit register; high 32 bits are at offset 0x04.
    // DSTRD = CAP[35:32] = bits [3:0] of the high dword.
    let cap_hi = rd32(base, OFF_CAP_LO + 4);
    let dstrd = (cap_hi & 0xF) as usize;
    let doorbell_stride = 4usize << dstrd; // bytes between consecutive doorbells

    // ── 2. Disable controller: CC.EN = 0 ─────────────────────────────────────
    let cc = rd32(base, OFF_CC);
    wr32(base, OFF_CC, cc & !CC_EN);
    wait_csts(base, CSTS_RDY, 0)?; // wait RDY → 0

    // ── 3. Zero DMA buffers ───────────────────────────────────────────────────
    core::ptr::write_bytes(core::ptr::addr_of_mut!(ADMIN_SQ) as *mut u8, 0, 4096);
    core::ptr::write_bytes(core::ptr::addr_of_mut!(ADMIN_CQ) as *mut u8, 0, 4096);
    core::ptr::write_bytes(core::ptr::addr_of_mut!(IDENTIFY) as *mut u8, 0, 4096);

    let sq_phys = v2p(core::ptr::addr_of!(ADMIN_SQ) as *const u8);
    let cq_phys = v2p(core::ptr::addr_of!(ADMIN_CQ) as *const u8);

    // ── 4. Configure admin queues ─────────────────────────────────────────────
    // AQA: ACQS[27:16] = ASQS[11:0] = QDEPTH - 1
    let aqa = ((QDEPTH - 1) as u32) | (((QDEPTH - 1) as u32) << 16);
    wr32(base, OFF_AQA, aqa);
    wr64(base, OFF_ASQ, sq_phys);
    wr64(base, OFF_ACQ, cq_phys);

    // ── 5. Enable controller ──────────────────────────────────────────────────
    wr32(base, OFF_CC, CC_EN | CC_IOSQES | CC_IOCQES);
    wait_csts(base, CSTS_RDY | CSTS_CFS, CSTS_RDY)?; // wait RDY=1, CFS=0

    // ── 6. Submit Identify Controller (opcode 0x06, CNS=1) ───────────────────
    let id_phys = v2p(core::ptr::addr_of!(IDENTIFY) as *const u8);
    let cmd_id: u16 = 1;
    submit_admin_cmd(base, cmd_id, ADM_IDENTIFY, 0, id_phys, CNS_CONTROLLER);

    // ── 7. Poll completion queue for phase bit = 1 ────────────────────────────
    if !poll_completion(base, doorbell_stride, cmd_id) {
        return None;
    }

    // ── 8. Parse Identify Controller data ────────────────────────────────────
    // NVMe Identify Controller (CNS=1) data layout:
    //   Bytes  0-1:  VID
    //   Bytes  4-23: SN  (Serial Number,   20 bytes, ASCII space-padded)
    //   Bytes 24-63: MN  (Model Number,    40 bytes, ASCII space-padded)
    //   Bytes 64-71: FR  (Firmware Rev.,    8 bytes, ASCII space-padded)
    let id = &*(core::ptr::addr_of!(IDENTIFY.0) as *const [u8; 4096]);
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

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Write one Admin SQE (64 bytes) and ring the SQ tail doorbell.
unsafe fn submit_admin_cmd(base: u64, cid: u16, opcode: u32, nsid: u32, prp1: u64, cdw10: u32) {
    let sq = core::ptr::addr_of_mut!(ADMIN_SQ.0) as *mut u32;

    // SQE dword layout (NVMe spec Table 20):
    //  DW0  : CID[31:16] | PSDT[15:14] | FUSE[9:8] | OPC[7:0]
    //  DW1  : NSID
    //  DW2-3: reserved
    //  DW4-5: MPTR
    //  DW6-7: PRP1
    //  DW8-9: PRP2
    //  DW10 : command-specific (CNS for Identify)
    //  DW11-15: 0
    sq.add(0).write_volatile(((cid as u32) << 16) | opcode);
    sq.add(1).write_volatile(nsid);
    sq.add(2).write_volatile(0); // reserved
    sq.add(3).write_volatile(0);
    sq.add(4).write_volatile(0); // MPTR lo
    sq.add(5).write_volatile(0); // MPTR hi
    sq.add(6).write_volatile(prp1 as u32); // PRP1 lo
    sq.add(7).write_volatile((prp1 >> 32) as u32); // PRP1 hi
    sq.add(8).write_volatile(0); // PRP2 lo
    sq.add(9).write_volatile(0); // PRP2 hi
    sq.add(10).write_volatile(cdw10);
    for i in 11..16 {
        sq.add(i).write_volatile(0);
    }

    fence(Ordering::Release);

    // Ring Admin SQ tail doorbell (queue 0): BAR0 + 0x1000 + 2*0*dstrd
    let sq_tail_db = base as usize + 0x1000;
    write_volatile(sq_tail_db as *mut u32, 1u32); // new tail = 1
}

/// Poll Admin CQE[0] for the expected phase bit (1 for first wrap).
/// Advances the CQ head doorbell on success.
unsafe fn poll_completion(base: u64, dstrd: usize, expected_cid: u16) -> bool {
    let cq = core::ptr::addr_of!(ADMIN_CQ.0) as *const u8;

    // CQE layout (16 bytes):
    //   [0..4]  DW0: command-specific result
    //   [4..8]  DW1: reserved
    //   [8..10] SQHD
    //   [10..12] SQID
    //   [12..14] CID
    //   [14..16] Status[15:1] | Phase[0]
    let status_ptr = cq.add(14) as *const u16;
    let cid_ptr = cq.add(12) as *const u16;

    for _ in 0..2_000_000u32 {
        let status = read_volatile(status_ptr);
        if status & 1 != 0 {
            // Phase bit set — completion is valid
            let cid = read_volatile(cid_ptr);
            if cid != expected_cid {
                return false; // unexpected command ID
            }
            let sc = (status >> 1) & 0x7FF; // status code
            if sc != 0 {
                return false; // command failed
            }
            // Advance CQ head doorbell: BAR0 + 0x1000 + (2*0+1)*dstrd
            let cq_head_db = base as usize + 0x1000 + dstrd;
            write_volatile(cq_head_db as *mut u32, 1u32); // new head = 1
            return true;
        }
        core::hint::spin_loop();
    }
    false // timeout
}

/// Spin until `(CSTS & mask) == expected`, returning `None` on CFS or timeout.
unsafe fn wait_csts(base: u64, mask: u32, expected: u32) -> Option<()> {
    for _ in 0..2_000_000u32 {
        let csts = rd32(base, OFF_CSTS);
        if csts & CSTS_CFS != 0 {
            return None; // controller fatal status
        }
        if csts & mask == expected {
            return Some(());
        }
        core::hint::spin_loop();
    }
    None // timeout
}
