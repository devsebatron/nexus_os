#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

extern crate alloc;

use alloc::{boxed::Box, vec, vec::Vec};
use bootloader_api::config::Mapping;
use bootloader_api::{entry_point, BootInfo};
use core::panic::PanicInfo;
use lazy_static::lazy_static;
use spin::Mutex;
use x86_64::VirtAddr;

use logger::FrameBufferWriter;

mod allocator;
mod cortex;
mod interrupts;
mod logger;
mod memory;
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

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset.into_option().unwrap());
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

    let heap_value = Box::new(41);
    println!("heap_value at {:p}", heap_value);

    let mut vec = Vec::new();
    for i in 0..500 {
        vec.push(i);
    }
    println!("vec at {:p}", vec.as_slice());

    println!("Heap verification successful!");

    println!("Heap verification successful!");

    init(); // Initialize IDT and PICs

    // Cortex AI Layer Initialization
    println!("Initializing Cortex AI Layer...");
    let cortex = cortex::CortexEngine::new();
    let input = vec![0.5, -0.5, 1.0, 0.0];
    let result = cortex.infer(&input);
    println!("{}", result);
    println!("Cortex Engine: AVX registers used successfully without fault.");

    let mut executor = task::simple_executor::SimpleExecutor::new();
    executor.spawn(task::Task::new(task::keyboard::print_keypresses()));
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
