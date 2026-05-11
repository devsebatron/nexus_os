use alloc::string::String;
use alloc::vec::Vec;
use futures::stream::StreamExt;

use crate::task::keyboard::ScancodeStream;

pub async fn run() {
    let mut scancodes = ScancodeStream::new();
    let mut buf: Vec<u8> = Vec::new();

    crate::println!("\nNexusOS shell ready. Type 'help' for commands.");
    print_prompt();

    while let Some(sc) = scancodes.next().await {
        match sc {
            _ if sc >= 0x80 => {} // key-release event, ignore
            0x0E => {
                // Backspace
                if !buf.is_empty() {
                    buf.pop();
                    crate::_backspace();
                }
            }
            0x1C => {
                // Enter
                crate::println!();
                let line = String::from_utf8_lossy(&buf);
                dispatch(line.trim());
                buf.clear();
                print_prompt();
            }
            sc => {
                if let Some(c) = crate::task::keyboard::decode_scancode(sc) {
                    buf.push(c as u8);
                    crate::print!("{}", c);
                }
            }
        }
    }
}

fn print_prompt() {
    crate::print!("nexus> ");
}

fn dispatch(line: &str) {
    if line.is_empty() {
        return;
    }
    let mut parts = line.splitn(2, ' ');
    let cmd = parts.next().unwrap_or("");
    let args = parts.next().unwrap_or("").trim();

    match cmd {
        "help" => cmd_help(),
        "clear" => cmd_clear(),
        "echo" => crate::println!("{}", args),
        "version" => crate::println!("NexusOS v0.1.0 (cortex_integration)"),
        "cortex" => cmd_cortex(args),
        "meminfo" => cmd_meminfo(),
        "pci" => cmd_pci(),
        "nvme" => cmd_nvme(),
        _ => crate::println!("unknown command: {}  (try 'help')", cmd),
    }
}

fn cmd_help() {
    crate::println!("Commands:");
    crate::println!("  help            show this message");
    crate::println!("  clear           clear the screen");
    crate::println!("  echo <text>     print text");
    crate::println!("  version         show OS version");
    crate::println!("  cortex <text>   run Cortex AI inference on input");
    crate::println!("  meminfo         show heap memory info");
    crate::println!("  pci             list PCI devices");
    crate::println!("  nvme            show NVMe controller info");
}

fn cmd_clear() {
    crate::_clear();
    print_prompt();
}

fn cmd_cortex(args: &str) {
    let text = if args.is_empty() { "hello" } else { args };
    let input: Vec<f32> = text
        .as_bytes()
        .iter()
        .take(8)
        .map(|&b| b as f32 / 128.0)
        .collect();
    let result = crate::cortex::CortexEngine::new().infer(&input);
    crate::println!("{}", result);
}

fn cmd_nvme() {
    crate::println!("Initialising NVMe controller...");
    match crate::nvme::find_and_init() {
        Some(info) => {
            crate::println!("  Vendor:   0x{:04x}", info.vendor_id);
            crate::println!("  Model:    {}", info.model_str());
            crate::println!("  Serial:   {}", info.serial_str());
            crate::println!("  Firmware: {}", info.firmware_str());
        }
        None => crate::println!("No NVMe controller found or init failed."),
    }
}

fn cmd_meminfo() {
    crate::println!("Heap base: 0x4444_4444_0000");
    crate::println!("Heap size: 1 MiB");
}

fn cmd_pci() {
    crate::println!("Scanning PCI bus...");
    let devices = crate::pci::enumerate();
    if devices.is_empty() {
        crate::println!("No PCI devices found.");
        return;
    }
    crate::println!("{} device(s) found:", devices.len());
    for d in &devices {
        crate::println!(
            "  {:02x}:{:02x}.{} [{:04x}:{:04x}] {}",
            d.bus,
            d.device,
            d.function,
            d.vendor_id,
            d.device_id,
            crate::pci::class_name(d.class, d.subclass)
        );
    }
}
