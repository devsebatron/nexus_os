#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

extern crate alloc;

use alloc::{boxed::Box, vec::Vec};
use bootloader_api::config::Mapping;
use bootloader_api::{entry_point, BootInfo};
use core::panic::PanicInfo;
use core::sync::atomic::{AtomicU64, Ordering};
use lazy_static::lazy_static;
use spin::Mutex;
use x86_64::VirtAddr;

/// Physical memory offset set at boot; used by drivers for phys↔virt conversion.
pub static PHYS_MEM_OFFSET: AtomicU64 = AtomicU64::new(0);

/// Global frame allocator — available to drivers after heap initialisation.
pub static FRAME_ALLOCATOR: Mutex<Option<memory::BootInfoFrameAllocator>> = Mutex::new(None);

use logger::FrameBufferWriter;

mod allocator;
mod cortex;
mod interrupts;
mod logger;
mod memory;
mod nvme;
mod pci;
mod shell;
mod task;

pub static BOOTLOADER_CONFIG: bootloader_api::BootloaderConfig = {
    let mut config = bootloader_api::BootloaderConfig::new_default();
    config.mappings.physical_memory = Some(Mapping::Dynamic);
    config
};

lazy_static! {
    pub static ref WRITER: Mutex<Option<FrameBufferWriter>> = Mutex::new(None);
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[doc(hidden)]
pub fn _print(args: core::fmt::Arguments) {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;
    interrupts::without_interrupts(|| {
        if let Some(writer) = WRITER.lock().as_mut() {
            writer.write_fmt(args).unwrap();
        }
    });
}

pub fn _backspace() {
    use x86_64::instructions::interrupts;
    interrupts::without_interrupts(|| {
        if let Some(writer) = WRITER.lock().as_mut() {
            writer.backspace();
        }
    });
}

pub fn _clear() {
    use x86_64::instructions::interrupts;
    interrupts::without_interrupts(|| {
        if let Some(writer) = WRITER.lock().as_mut() {
            writer.clear();
        }
    });
}

entry_point!(kernel_main, config = &BOOTLOADER_CONFIG);

fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    if let Some(framebuffer) = boot_info.framebuffer.as_mut() {
        let info = framebuffer.info();
        let buffer = framebuffer.buffer_mut();
        let writer = FrameBufferWriter::new(buffer, info);
        *WRITER.lock() = Some(writer);
    }

    println!("Hello NexusOS!");
    println!("We are back in text mode, but now with PIXELS!");

    let phys_offset_val = boot_info.physical_memory_offset.into_option().unwrap();
    PHYS_MEM_OFFSET.store(phys_offset_val, Ordering::Relaxed);
    let phys_mem_offset = VirtAddr::new(phys_offset_val);
    println!("Physical memory offset: {:?}", phys_mem_offset);

    println!("Initializing mapper...");
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    println!("Mapper initialized.");

    println!("Initializing frame allocator...");
    let mut frame_allocator =
        unsafe { memory::BootInfoFrameAllocator::init(&boot_info.memory_regions) };
    println!("Frame allocator initialized.");

    println!("Initializing heap...");
    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("heap initialization failed");
    println!("Heap initialized.");

    // Move frame allocator into global so drivers (NVMe, etc.) can allocate DMA frames.
    *FRAME_ALLOCATOR.lock() = Some(frame_allocator);

    let heap_value = Box::new(41);
    println!("heap_value at {:p}", heap_value);

    let mut vec = Vec::new();
    for i in 0..500 {
        vec.push(i);
    }
    println!("vec at {:p}", vec.as_slice());

    println!("Heap verification successful!");

    init(); // Initialize IDT and PICs

    // Cortex AI Layer Initialization
    println!("Initializing Cortex AI Layer...");
    let input = alloc::vec![0.5f32, -0.5, 1.0, 0.0];
    println!("{}", cortex::CortexEngine::new().infer(&input));

    let mut executor = task::simple_executor::SimpleExecutor::new();
    executor.spawn(task::Task::new(shell::run()));
    executor.run();

    loop {
        x86_64::instructions::hlt();
    }
}

fn init() {
    interrupts::init_idt();
    unsafe { interrupts::PICS.lock().initialize() };
    x86_64::instructions::interrupts::enable();
}

/// This function is called on panic.
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {
        x86_64::instructions::hlt();
    }
}
