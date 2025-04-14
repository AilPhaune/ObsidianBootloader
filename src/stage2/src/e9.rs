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

macro_rules! impl_display_dec {
    ($value: ident, $buffer: ident, $i: ident) => {{
        let mut v = $value;
        while v > 0 {
            let (q, r) = (v / 10, v % 10);
            v = q;
            $i -= 1;
            if let Some(p) = $buffer.get_mut($i) {
                *p = b'0' + (r as u8);
            }
        }
        while $i < $buffer.len() {
            if let Some(c) = $buffer.get($i) {
                write_char(*c);
            }
            $i += 1;
        }
    }};
}

pub fn write_u8_decimal(value: u8) {
    if value == 0 {
        write_char(b'0');
        return;
    }
    let mut buffer = [b' '; 4];
    let mut i = 4;
    impl_display_dec!(value, buffer, i);
}

pub fn write_u16_decimal(value: u16) {
    if value == 0 {
        write_char(b'0');
        return;
    }
    let mut buffer = [b' '; 6];
    let mut i = 6;
    impl_display_dec!(value, buffer, i);
}

pub fn write_u32_decimal(value: u32) {
    if value == 0 {
        write_char(b'0');
        return;
    }
    let mut buffer = [b' '; 10];
    let mut i = 10;
    impl_display_dec!(value, buffer, i);
}

pub fn write_u64_decimal(value: u64) {
    if value == 0 {
        write_char(b'0');
        return;
    }
    let mut buffer = [b' '; 21];
    let mut i = 21;
    impl_display_dec!(value, buffer, i);
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

pub fn write_guid(guid: [u8; 16]) {
    printf!(
        b"%b%b%b%b-%b%b-%b%b-%b%b-%b%b%b%b%b%b",
        guid[3],
        guid[2],
        guid[1],
        guid[0],
        guid[5],
        guid[4],
        guid[7],
        guid[6],
        guid[8],
        guid[9],
        guid[10],
        guid[11],
        guid[12],
        guid[13],
        guid[14],
        guid[15]
    );
}
