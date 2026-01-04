use std::{
    env,
    process::{self, Command},
};

fn main() {
    let kernel_path = env::current_dir().unwrap().join("nexus_kernel");

    // Build kernel
    let status = Command::new("cargo")
        .current_dir(&kernel_path)
        .args(&["build", "--target", "x86_64-unknown-none"])
        .status()
        .unwrap();

    if !status.success() {
        panic!("Kernel build failed");
    }

    let kernel_binary = kernel_path
        .parent()
        .unwrap()
        .join("target/x86_64-unknown-none/debug/nexus_kernel");

    // Create disk image
    let mut cmd = Command::new("qemu-system-x86_64");
    cmd.arg("-drive").arg(format!(
        "format=raw,file={}",
        env::var("bios_path").unwrap_or_else(|_| ".".into())
    )); // Placeholder

    // Using bootloader::BiosBoot to creating the image
    let image_path = kernel_path
        .parent()
        .unwrap()
        .join("target/x86_64-unknown-none/debug/bootimage.bin");
    let mut boot = bootloader::BiosBoot::new(&kernel_binary);
    boot.create_disk_image(&image_path).unwrap();

    // Run QEMU
    let mut cmd = Command::new("qemu-system-x86_64");
    cmd.arg("-drive");
    cmd.arg(format!("format=raw,file={}", image_path.display()));

    let mut child = cmd.spawn().unwrap();
    child.wait().unwrap();
}
