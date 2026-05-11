use alloc::vec::Vec;
use x86_64::instructions::port::Port;

const CONFIG_ADDRESS: u16 = 0xCF8;
const CONFIG_DATA: u16 = 0xCFC;

/// Read a 32-bit dword from PCI configuration space.
pub(crate) fn config_read(bus: u8, device: u8, func: u8, offset: u8) -> u32 {
    let address: u32 = (1 << 31)
        | ((bus as u32) << 16)
        | ((device as u32) << 11)
        | ((func as u32) << 8)
        | ((offset as u32) & 0xFC);

    unsafe {
        let mut addr: Port<u32> = Port::new(CONFIG_ADDRESS);
        let mut data: Port<u32> = Port::new(CONFIG_DATA);
        addr.write(address);
        data.read()
    }
}

pub struct PciDevice {
    pub bus: u8,
    pub device: u8,
    pub function: u8,
    pub vendor_id: u16,
    pub device_id: u16,
    pub class: u8,
    pub subclass: u8,
    pub prog_if: u8,
    /// Physical base address of BAR0 (MMIO region)
    pub bar0: u64,
}

/// Scan all PCI buses and return every device found.
pub fn enumerate() -> Vec<PciDevice> {
    let mut devices = Vec::new();

    for bus in 0u8..=255 {
        for dev in 0u8..32 {
            scan_device(bus, dev, &mut devices);
        }
    }

    devices
}

fn scan_device(bus: u8, dev: u8, devices: &mut Vec<PciDevice>) {
    let id = config_read(bus, dev, 0, 0x00);
    let vendor_id = (id & 0xFFFF) as u16;
    if vendor_id == 0xFFFF {
        return; // no device
    }

    scan_function(bus, dev, 0, vendor_id, devices);

    // Check multifunction bit (header type bit 7)
    let header_reg = config_read(bus, dev, 0, 0x0C);
    let header_type = ((header_reg >> 16) & 0xFF) as u8;
    if header_type & 0x80 != 0 {
        for func in 1u8..8 {
            let id = config_read(bus, dev, func, 0x00);
            let vendor_id = (id & 0xFFFF) as u16;
            if vendor_id != 0xFFFF {
                scan_function(bus, dev, func, vendor_id, devices);
            }
        }
    }
}

fn scan_function(bus: u8, dev: u8, func: u8, vendor_id: u16, devices: &mut Vec<PciDevice>) {
    let id = config_read(bus, dev, func, 0x00);
    let device_id = ((id >> 16) & 0xFFFF) as u16;

    let class_reg = config_read(bus, dev, func, 0x08);
    let class = ((class_reg >> 24) & 0xFF) as u8;
    let subclass = ((class_reg >> 16) & 0xFF) as u8;
    let prog_if = ((class_reg >> 8) & 0xFF) as u8;

    // Read BAR0; handles both 32-bit and 64-bit memory BARs.
    let bar0_raw = config_read(bus, dev, func, 0x10);
    let bar0 = if bar0_raw & 1 == 0 {
        // Memory BAR
        if (bar0_raw >> 1) & 0x3 == 0x2 {
            // 64-bit BAR: high 32 bits are in BAR1
            let hi = config_read(bus, dev, func, 0x14);
            (bar0_raw as u64 & !0xF) | ((hi as u64) << 32)
        } else {
            // 32-bit BAR
            (bar0_raw as u64) & !0xF
        }
    } else {
        0 // I/O BAR, not used
    };

    devices.push(PciDevice {
        bus,
        device: dev,
        function: func,
        vendor_id,
        device_id,
        class,
        subclass,
        prog_if,
        bar0,
    });

    // If this is a PCI-to-PCI bridge, we would recurse into the secondary bus.
    // (class 0x06, subclass 0x04) — skipped for now; QEMU typically uses bus 0 only.
}

pub fn class_name(class: u8, subclass: u8) -> &'static str {
    match (class, subclass) {
        (0x00, 0x00) => "Non-VGA Unclassified Device",
        (0x00, 0x01) => "VGA-Compatible Unclassified Device",
        (0x01, 0x00) => "SCSI Bus Controller",
        (0x01, 0x01) => "IDE Controller",
        (0x01, 0x06) => "SATA Controller",
        (0x01, 0x08) => "NVMe Controller",
        (0x02, 0x00) => "Ethernet Controller",
        (0x03, 0x00) => "VGA Compatible Controller",
        (0x04, 0x01) => "Multimedia Audio Controller",
        (0x04, 0x03) => "Audio Device",
        (0x06, 0x00) => "Host Bridge",
        (0x06, 0x01) => "ISA Bridge",
        (0x06, 0x04) => "PCI-to-PCI Bridge",
        (0x0C, 0x03) => "USB Controller",
        (0x0C, 0x05) => "SMBus Controller",
        _ => "Unknown Device",
    }
}
