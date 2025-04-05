#![no_std]
#![no_main]
#![feature(sync_unsafe_cell)]

pub mod bios;
pub mod fs;
pub mod mem;
pub mod io;
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

use bios::{DiskError, ExtendedDisk};
use fs::{Ext2Error, Ext2FileSystem};
use mem::{detect_system_memory, mem_free, mem_total, mem_used};

use crate::video::{ Video, Color };

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
    
        let extended_disk = ExtendedDisk::new(boot_drive as u8, bios_idt);
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
                video.write_hex_u32(mem_free() as u32);
                video.write_string(b" / 0x");
                video.write_hex_u32(mem_used() as u32);
                video.write_string(b" / 0x");
                video.write_hex_u32(mem_total() as u32);
                video.write_char(b'\n');
            };
        }

        let ext2 = Ext2FileSystem::new(extended_disk).unwrap_or_else(|e| {
            match e {
                Ext2Error::FailedMemAlloc => {
                    video.write_string(b"Failed to allocate memory\n");
                },
                Ext2Error::BadDiskSectorSize(s) => {
                    video.write_string(b"Bad disk sector size: 0x");
                    video.write_hex_u16(s);
                    video.write_char(b'\n');
                }
                Ext2Error::DiskError(e) => {
                    video.write_string(b"Disk error: ");
                    match e {
                        DiskError::ReadError(c) => {
                            video.write_string(b"read error 0x");
                            video.write_hex_u32(c as u32);
                        },
                        DiskError::ReadParametersError(c) => {
                            video.write_string(b"read parameters error 0x");
                            video.write_hex_u32(c as u32);
                        },
                        DiskError::OutputBufferTooSmall => {
                            video.write_string(b"output buffer too small");
                        },
                        DiskError::InvalidDiskParameters => {
                            video.write_string(b"invalid disk parameters");
                        },
                    }
                    video.write_char(b'\n');
                }
            }
            kpanic();
        });
        show_mem!();
        
    }

    #[allow(clippy::empty_loop)]
    loop {}
}