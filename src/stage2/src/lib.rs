#![no_std]
#![no_main]
#![feature(sync_unsafe_cell)]
#![feature(optimize_attribute)]

pub mod bios;
pub mod e9;
pub mod fs;
pub mod gpt;
pub mod io;
pub mod mem;
pub mod video;

pub mod eflags {
    /// Carry Flag
    pub const CF: usize = 0b00000000000000000000000000000001;
    /// Parity Flag
    pub const PF: usize = 0b00000000000000000000000000000010;
    /// Auxiliary Carry Flag
    pub const AF: usize = 0b00000000000000000000000000000100;
    /// Zero Flag
    pub const ZF: usize = 0b00000000000000000000000000001000;
    /// Sign Flag
    pub const SF: usize = 0b00000000000000000000000000010000;
    /// Trap Flag
    pub const TF: usize = 0b00000000000000000000000000100000;
    /// Interrupt Enable Flag
    pub const IF: usize = 0b00000000000000000000000001000000;
    /// Direction Flag
    pub const DF: usize = 0b00000000000000000000000010000000;
    /// Overflow Flag
    pub const OF: usize = 0b00000000000000000000000100000000;

    /// I/O Privilege Level (IOPL)
    pub const IOPL: usize = 0b00000000000000000001100000000000;
    /// Nested Task Flag
    pub const NT: usize = 0b00000000000000000100000000000000;
    /// Resume Flag
    pub const RF: usize = 0b00000000000000001000000000000000;
    /// Virtual 8086 Mode Flag
    pub const VM: usize = 0b00000000000000100000000000000000;
    /// Alignment Check Flag
    pub const AC: usize = 0b00000000000001000000000000000000;
    /// Virtual Interrupt Flag
    pub const VIF: usize = 0b00000000000010000000000000000000;
    /// Virtual Interrupt Pending Flag
    pub const VIP: usize = 0b00000000000100000000000000000000;
}

use bios::ExtendedDisk;
use e9::write_buffer_as_string;
use fs::{Ext2Error, Ext2FileSystem, Ext2FileType};
use gpt::GUIDPartitionTable;
use mem::{detect_system_memory, get_mem_free, get_mem_total, get_mem_used, Buffer};

use crate::video::{Color, Video};

#[macro_export]
macro_rules! integer_enum_impl {
    ($enum_name: ident, $int_type: ident) => {
        impl BitOr<$enum_name> for $enum_name {
            type Output = $int_type;
            fn bitor(self, rhs: $enum_name) -> Self::Output {
                (self as $int_type) | (rhs as $int_type)
            }
        }

        impl BitOr<$enum_name> for $int_type {
            type Output = $int_type;
            fn bitor(self, rhs: $enum_name) -> Self::Output {
                self | (rhs as $int_type)
            }
        }

        impl BitOr<$int_type> for $enum_name {
            type Output = $int_type;
            fn bitor(self, rhs: $int_type) -> Self::Output {
                (self as $int_type) | rhs
            }
        }

        impl BitOrAssign<$enum_name> for $int_type {
            fn bitor_assign(&mut self, rhs: $enum_name) {
                *self = (*self) | rhs
            }
        }
    };
}

extern "cdecl" {
    pub fn stage3_entry();
}

pub fn ptr_to_seg_off(ptr: usize) -> (u16, u16) {
    ((ptr >> 4) as u16, (ptr & 0xF) as u16)
}

pub fn seg_off_to_ptr(seg: u16, off: u16) -> usize {
    ((seg as usize) << 4) + (off as usize)
}

#[panic_handler]
pub fn panic(_info: &core::panic::PanicInfo) -> ! {
    kpanic();
}

pub fn kpanic() -> ! {
    unsafe {
        let video = Video::get();
        video.set_color(Color::Black, Color::Red);
        video.write_string(b"\r\nPANIC\r\n");
    }

    #[allow(clippy::empty_loop)]
    loop {}
}

pub fn fnv1a64(data: &Buffer) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in data.iter() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[no_mangle]
pub extern "cdecl" fn rust_entry(bios_idt: usize, boot_drive: usize) -> ! {
    unsafe {
        let video = Video::get();
        video.clear();

        video.write_string(b"Bios IDT: 0x");
        video.write_hex_u8((bios_idt >> 24) as u8);
        video.write_hex_u8((bios_idt >> 16) as u8);
        video.write_hex_u8((bios_idt >> 8) as u8);
        video.write_hex_u8(bios_idt as u8);
        video.write_char(b'\n');

        video.write_string(b"Booting from drive 0x");
        video.write_hex_u8(boot_drive as u8);
        video.write_char(b'\n');

        let mut extended_disk = ExtendedDisk::new(boot_drive as u8, bios_idt);
        if !extended_disk.check_present() {
            kpanic();
        }

        match detect_system_memory(bios_idt) {
            Ok(_) => {}
            Err(e) => {
                video.write_string(b"Memory detection failed: 0x");
                video.write_hex_u8(e);
                video.write_char(b'\n');
                kpanic();
            }
        }

        macro_rules! show_mem {
            () => {
                video.write_string(b"Free/Used/Total: 0x");
                video.write_hex_u32(get_mem_free() as u32);
                video.write_string(b" / 0x");
                video.write_hex_u32(get_mem_used() as u32);
                video.write_string(b" / 0x");
                video.write_hex_u32(get_mem_total() as u32);
                video.write_char(b'\n');
            };
        }

        let gpt = GUIDPartitionTable::read(&mut extended_disk).unwrap_or_else(|e| e.panic());
        let Some(part0) = gpt.get_partitions().get(0) else {
            video.write_string(b"No partitions !\n");
            kpanic();
        };
        let mut ext2 = Ext2FileSystem::mount_ro(extended_disk, part0.as_disk_range())
            .unwrap_or_else(|e| e.panic());
        video.write_string(b"Mounted ext2\n");
        show_mem!();

        let Ext2FileType::Directory(root) = ext2.open(2).unwrap_or_else(|e| e.panic()) else {
            video.write_string(b"Root is not a directory !\n");
            kpanic();
        };

        let mut hellotxt = None;
        let mut randombin = None;

        for entry in root.listdir() {
            if entry.has_name(b"hello.txt") {
                printf!(b"Found /hello.txt\r\n");
                hellotxt = Some(entry.get_inode());
            }

            if entry.has_name(b"random.bin") {
                printf!(b"Found /random.bin\r\n");
                randombin = Some(entry.get_inode());
            }
        }

        if let Some(inode) = randombin {
            let mut file = match ext2.open(inode as usize) {
                Ok(Ext2FileType::File(file)) => file,
                Err(e) => e.panic(),
                _ => {
                    video.write_string(b"/random.bin is not a file !\n");
                    kpanic()
                }
            };

            let contents = file.read_all().unwrap_or_else(|e| e.panic());
            printf!(b"Fetched /random.bin contents\r\n");
            let hash = fnv1a64(&contents);

            printf!(
                b"/random.bin (size = 0x%x), contents hash: %x%x\r\n",
                file.get_size() as u32,
                (hash >> 32) as u32,
                hash as u32
            );
        }

        if let Some(inode) = hellotxt {
            let mut file = match ext2.open(inode as usize) {
                Ok(Ext2FileType::File(file)) => file,
                Err(e) => e.panic(),
                _ => {
                    video.write_string(b"/hello.txt is not a file !\n");
                    kpanic()
                }
            };

            let contents = file.read_all().unwrap_or_else(|e| e.panic());

            printf!(b"/hello.txt (size = 0x%x), contents:\r\n", file.get_size());
            write_buffer_as_string(&contents);
        }

        #[allow(clippy::empty_loop)]
        loop {}
    }
}
