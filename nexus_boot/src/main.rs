use std::{env, process::Command};

fn main() {
    let current_dir = env::current_dir().unwrap();
    let kernel_path = if current_dir.join("nexus_kernel").exists() {
        current_dir.join("nexus_kernel")
    } else if current_dir.ends_with("nexus_kernel") {
        current_dir
    } else {
        panic!(
            "Cannot find nexus_kernel directory. Please run from workspace root or kernel directory."
        );
    };

    // Build kernel
    let status = Command::new("cargo")
        .current_dir(&kernel_path)
        .args(["build", "--target", "x86_64-unknown-none"])
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
    let boot = bootloader::BiosBoot::new(&kernel_binary);
    boot.create_disk_image(&image_path).unwrap();

    // Run QEMU
    let mut cmd = Command::new("qemu-system-x86_64");
    cmd.arg("-drive");
    cmd.arg(format!("format=raw,file={}", image_path.display()));

    let mut child = cmd.spawn().unwrap();
    child.wait().unwrap();
}
