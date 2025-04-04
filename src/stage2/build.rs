use std::process::Command;

fn main() {
    // Assemble the assembly file
    Command::new("nasm")
        .args(["-f", "elf32", "-o", "main.o", "main.asm"])
        .status()
        .expect("Failed to assemble main.asm");

    // Link the object file with Rust's output
    println!("cargo:rustc-link-arg=main.o");
    println!("cargo:rerun-if-changed=main.asm");
}
