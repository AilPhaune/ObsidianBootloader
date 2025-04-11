# Obsidian bootloader
Obsidian bootloader is a bootloader for the Obsidian operating system.
<br>
It is not multiboot compliant (yet !).

Obsidian leverages the power of the [Rust](https://www.rust-lang.org/) programming language to provide a fast and secure (memory safe) bootloader.

# Goal
This is my honest attempt at learning Operating System development.
<br>
The primary goal of this bootloader is to load the Obsidian kernel.
<br>
As an additional challenge to myself, I wanted to make it multiboot compliant. (It is not yet !)

# Compilation
Must have:
- [Rust (cargo)](https://www.rust-lang.org/)
- [Nasm](https://www.nasm.us/)
- [Make](https://www.gnu.org/software/make/)
- [Scons](https://scons.org/)

### Build binaries: 
- `scons mode=release size=<size in MiB>` OR
- `scons mode=debug size=<size in MiB>`
<br>
Default size if not specified is 32 MiB. Minimum size is 32 MiB.

### Build disk image:
- `sudo ./mkdisk`
<br>
You must have at least one available loopback device (`man losetup`).
<br>
Disk image built at `build/disk.img`