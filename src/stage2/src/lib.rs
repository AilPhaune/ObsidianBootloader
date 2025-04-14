#![no_std]
#![no_main]
#![feature(sync_unsafe_cell)]
#![feature(optimize_attribute)]
#![feature(naked_functions)]

pub mod arith;
pub mod bios;
pub mod e9;
pub mod fs;
pub mod gdt;
pub mod gpt;
pub mod io;
pub mod mem;
pub mod paging;
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
use e9::{write_buffer_as_string, write_guid, write_u64_decimal};
use fs::{Ext2FileSystem, Ext2FileType};
use gdt::{is_cpuid_supported, is_long_mode_supported};
use gpt::{GUIDPartitionTable, PARTITION_GUID_TYPE_LINUX_FS};
use mem::{detect_system_memory, get_mem_free, get_mem_total, get_mem_used, Buffer};
use paging::enable_paging;

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
        printf!(b"Bios IDT located at: 0x%x\r\n", bios_idt);

        video.write_string(b"Booting from drive 0x");
        video.write_hex_u8(boot_drive as u8);
        video.write_char(b'\n');
        printf!(b"Booting from BIOS drive #%bh\r\n", boot_drive);

        if !is_cpuid_supported() {
            video.write_string(b"Failed to boot: CPUID not supported !\n");
            kpanic();
        }
        printf!(b"CPU supports cpuid\r\n");

        let mut extended_disk = ExtendedDisk::new(boot_drive as u8, bios_idt);
        if !extended_disk.check_present() {
            kpanic();
        }
        printf!(b"Extended BIOS disk functions present\r\n");
        let disk_params = extended_disk.get_params().unwrap_or_else(|e| e.panic());

        match detect_system_memory(bios_idt) {
            Ok(_) => {
                printf!(b"Successfully detected system memory from BIOS\r\n");
            }
            Err(e) => {
                printf!(b"Failed to detect system memory from BIOS: 0x%b\r\n", e);
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
        printf!(b"\r\nFound GUID Partition Table on boot drive\r\nList partitions:\r\n");
        for partition in gpt.get_partitions().iter() {
            if partition.name.is_empty() || !partition.name.iter().any(|c| c != 0) {
                printf!(b"> NO NAME");
            } else {
                printf!(b"> \"");
                write_buffer_as_string(&partition.name);
                printf!(b"\"");
            }
            printf!(
                b"\r\n|--- Begin LBA: HEX %x%x / DEC ",
                (partition.first_lba >> 32) as u32,
                partition.first_lba as u32
            );
            write_u64_decimal(partition.first_lba);
            printf!(
                b"\r\n|--- End LBA: HEX %x%x / DEC ",
                (partition.last_lba >> 32) as u32,
                partition.last_lba as u32
            );
            write_u64_decimal(partition.last_lba);
            printf!(b"\r\n|--- Size: ");
            let size = partition.last_lba - partition.first_lba + 1;
            write_u64_decimal(size);
            printf!(b" sectors => ");
            write_u64_decimal(size * (disk_params.bytes_per_sector as u64));
            printf!(b" bytes\r\n|--- Type: ");
            write_guid(partition.type_guid);
            printf!(b"\r\n|--- Unique id: ");
            write_guid(partition.unique_guid);
            printf!(
                b"\r\n+--- Flags: %x %x\r\n",
                (partition.flags >> 32) as u32,
                partition.flags as u32
            );
        }
        printf!(b"\n");

        let (part_i, mut ext2) = {
            let mut part = None;
            for (i, partition) in gpt.get_partitions().iter().enumerate() {
                if partition.type_guid == PARTITION_GUID_TYPE_LINUX_FS {
                    match Ext2FileSystem::mount_ro(extended_disk.clone(), partition.as_disk_range())
                    {
                        Ok(ext2) => {
                            part = Some((i, ext2));
                            break;
                        }
                        Err(e) => {
                            printf!(b"Failed to mount partition 0x%b as ext2: ", i);
                            e.printf();
                        }
                    }
                }
            }
            if let Some(part) = part {
                part
            } else {
                printf!(b"Couldn't find an ext2-formatted linux type filesystem partition.\r\n");
                video.write_string(b"No ext2 partition !\n");
                kpanic();
            }
        };
        video.write_string(b"Mounted ext2 partition 0x");
        video.write_hex_u8(part_i as u8);
        video.write_string(b".\n");
        printf!(b"Mounted partition 0x%b as ext2.\r\n\n", part_i);

        show_mem!();

        let Ext2FileType::Directory(root) = ext2.open(2).unwrap_or_else(|e| e.panic()) else {
            printf!(b"Inode 2 is not a directory !\r\n");
            video.write_string(b"Root is not a directory !\n");
            kpanic();
        };

        printf!(b"Listing files of root directory (inode 2):\r\n");
        for entry in root.listdir() {
            printf!(b"    /");
            write_buffer_as_string(entry.get_name());
            printf!(b"\r\n");
        }
        printf!(b"Done.\r\n\n");

        if !is_long_mode_supported() {
            printf!(b"Long mode not supported\r\n");
            video.write_string(b"Failed to boot: Long mode not supported !\n");
            kpanic();
        }
        printf!(b"CPU supports long mode\r\n\n");

        enable_paging(temp64 as usize);

        #[allow(clippy::empty_loop)]
        loop {}
    }
}

#[naked]
#[no_mangle]
pub extern "C" fn temp64() -> ! {
    unsafe {
        core::arch::naked_asm!(".code64", "cli", "2:", "hlt", "jmp 2b");
    }
}
