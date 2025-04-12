use kformat_default_nostd::Writeable;

use crate::{
    io::{inb, outb},
    mem::Buffer,
    video::get_hex_digit,
};

pub fn write_string(string: &[u8]) {
    for c in string.iter() {
        write_char(*c);
    }
}

#[no_mangle]
pub fn write_char(character: u8) {
    unsafe {
        // BOCHS
        outb(0xE9, character);

        // QEMU
        while inb(0x379) & 0b01000000 == 0 {}
        outb(0x378, character);
        outb(0x37A, inb(0x37A) | 1);
        while inb(0x379) & 0b00100000 != 0 {}
        outb(0x37A, inb(0x37A) & 0b11111110);
    }
}

pub fn write_hex_u8(value: u8) {
    write_char(get_hex_digit((value >> 4) & 0xF));
    write_char(get_hex_digit(value & 0xF));
}

pub fn write_hex_u16(value: u16) {
    for i in (0..4).rev() {
        write_char(get_hex_digit(((value >> (i * 4)) & 0xF) as u8));
    }
}

pub fn write_hex_u32(value: u32) {
    for i in (0..8).rev() {
        write_char(get_hex_digit(((value >> (i * 4)) & 0xF) as u8));
    }
}

pub fn write_buffer_slice_as_string(buffer: &Buffer, start: usize, end: usize) {
    for i in start..end {
        write_char(buffer.get(i).unwrap_or(b'?'));
    }
}

pub fn write_buffer_as_string(buffer: &Buffer) {
    write_buffer_slice_as_string(buffer, 0, buffer.len());
}

pub struct E9 {}

impl Writeable for E9 {
    fn write(&mut self, data: char) -> Result<(), usize> {
        write_char(data as u8);
        Ok(())
    }
}

#[macro_export]
macro_rules! e9kprint {
    ($fmt: literal, $($args:expr),*) => {{
        use $crate::e9::E9;
        use kformat_default_nostd::kwrite;
        let mut e9 = E9 {};
        kwrite!(e9, $fmt, $($args),*)
    }};
}

#[macro_export]
macro_rules! printf {
    ($fmt:expr) => {{
        use $crate::e9::write_string;
        write_string($fmt);
    }};
    ($fmt:literal $(,$arg:expr)*) => {{
        use $crate::e9::{write_char, write_hex_u8, write_hex_u32};
        let mut iter = $fmt.iter();
        let args = [$($arg),*];
        let mut args_iter = args.iter();
        while let Some(byte) = iter.next() {
            if *byte == b'%' {
                match iter.next() {
                    Some(b'd') => {
                        if let Some(arg) = args_iter.next() {
                            write_hex_u32(*arg as u32);
                        }
                    }
                    Some(b'x') => {
                        if let Some(arg) = args_iter.next() {
                            write_hex_u32(*arg as u32);
                        }
                    }
                    Some(b'b') => {
                        if let Some(arg) = args_iter.next() {
                            write_hex_u8(*arg as u8);
                        }
                    }
                    _ => {}
                }
            } else {
                write_char(*byte);
            }
        }
    }};
}
