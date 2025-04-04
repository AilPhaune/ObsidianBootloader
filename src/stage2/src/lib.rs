#![no_std]
#![no_main]
#![feature(sync_unsafe_cell)]

pub mod asm;
pub mod video;

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
pub extern "cdecl" fn rust_entry(boot_drive: usize) -> ! {
    unsafe {
        let video = Video::get();
        video.clear();
        video.write_string(b"Booting from drive 0x");
        video.write_hex_u8(boot_drive as u8);
        video.write_char(b'\n');
    }
    
    #[allow(clippy::empty_loop)]
    loop {}
}