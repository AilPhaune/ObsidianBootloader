use std::process::Command;
use std::fs;
use std::path::Path;

fn visit_dirs(dir: &Path, cb: &dyn Fn(&fs::DirEntry)) -> std::io::Result<()> {
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                visit_dirs(&path, cb)?;
            } else {
                cb(&entry);
            }
        }
    }
    Ok(())
}

fn find_asm_recursive() {
    let current_dir = std::env::current_dir().unwrap();
    visit_dirs(&current_dir, &|entry| {
        let path = entry.path();
        if path.extension() != Some("asm".as_ref()) {
            return;
        }
        println!("cargo:rerun-if-changed={}", path.display());
    }).unwrap();
}

fn main() {
    // Assemble the assembly file
    Command::new("nasm")
        .args(["-f", "elf32", "-o", "main.o", "main.asm"])
        .status()
        .expect("Failed to assemble main.asm");

    // Link the object file with Rust's output
    println!("cargo:rustc-link-arg=main.o");
    println!("cargo:rerun-if-changed=main.asm");
    println!("cargo:rerun-if-changed=build.rs");

    find_asm_recursive();
}
