use core::cell::SyncUnsafeCell;

use crate::asm::{inb, outb};

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct Character {
    pub character: u8,
    pub color: u8,
}

#[repr(u8)]
pub enum Color {
    Black = 0x00,
    Blue = 0x01,
    Green = 0x02,
    Cyan = 0x03,
    Red = 0x04,
    Purple = 0x05,
    Brown = 0x06,
    Gray = 0x07,
    DarkGray = 0x08,
    LightBlue = 0x09,
    LightGreen = 0x0A,
    LightCyan = 0x0B,
    LightRed = 0x0C,
    LightPurple = 0x0D,
    Yellow = 0x0E,
    White = 0x0F,
}

impl Color {
    pub const fn fg(color: Color) -> u8 {
        color as u8
    }

    pub const fn bg(color: Color) -> u8 {
        (color as u8) << 4
    }

    pub const fn color(fg: Color, bg: Color) -> u8 {
        Self::fg(fg) | Self::bg(bg)
    }
}

pub const VGA_WIDTH: usize = 80;
pub const VGA_HEIGHT: usize = 25;
pub const VGA_START_ADDRESS: usize = 0xB8000;
pub const VGA_SIZE: usize = VGA_WIDTH * VGA_HEIGHT;
pub const VGA_END_ADDRESS: usize = VGA_START_ADDRESS + size_of::<Character>() * VGA_SIZE;
pub struct Cursor {}

impl Cursor {
    pub fn enable_cursor(start: u8, end: u8) {
        unsafe {
            outb(0x3D4, 0x0A);
            outb(0x3D5, (inb(0x3D5) & 0xC0) | start);

            outb(0x3D4, 0x0B);
            outb(0x3D5, (inb(0x3D5) & 0xE0) | end);
        }
    }

    pub fn disable_cursor() {
        unsafe {
            outb(0x3D4, 0x0A);
            outb(0x3D5, 0x20);
        }
    }

    pub fn update_cursor(x: usize, y: usize) {
        let pos = y * VGA_WIDTH + x;
        unsafe {
            outb(0x3D4, 0x0F);
            outb(0x3D5, (pos & 0xFF) as u8);
            outb(0x3D4, 0x0E);
            outb(0x3D5, ((pos >> 8) & 0xFF) as u8);
        }
    }

    pub fn get_cursor_position() -> u16 {
        let mut pos: u16 = 0;
        unsafe {
            outb(0x3D4, 0x0F);
            pos |= inb(0x3D5) as u16;
            outb(0x3D4, 0x0E);
            pos |= (inb(0x3D5) as u16) << 8;
        }
        pos
    }
}

macro_rules! video_memory {
    [$idx: expr] => {{
        let video_memory = VGA_START_ADDRESS as *mut $crate::video::Character;
        &mut *video_memory.add($idx)
    }};
}

fn get_hex_digit(value: u8) -> u8 {
    if value < 10 {
        b'0' + value
    } else {
        b'A' - 10 + value
    }
}

static VIDEO: SyncUnsafeCell<Video> = SyncUnsafeCell::new(Video::new());

pub struct Video {
    current_x: u16,
    current_y: u16,
    current_color: u8,
}

impl Video {
    /// # Safety
    /// This function is safe to call as long as the video memory is mapped at 0xB8000 and the VGA size is 80x25
    pub unsafe fn get() -> &'static mut Video {
        &mut *VIDEO.get()
    }

    pub fn println(string: &[u8], foreground: Color, background: Color) {
        unsafe {
            let video = Self::get();
            let color = video.current_color;
            video.set_color(foreground, background);
            video.write_string(string);
            video.write_char(b'\n');
            video.current_color = color;
        }
    }

    /// # Safety
    /// This function reads memory from the given pointer until it encounters a null byte. Make absolutely sure your string is null terminated !
    pub unsafe fn print_c_str(c_str: *const u8, foreground: Color, background: Color) {
        let video = Self::get();
        let color = video.current_color;
        video.set_color(foreground, background);
        video.write_c_string(c_str);
        video.current_color = color;
    }

    const fn new() -> Video {
        Video {
            current_x: 0,
            current_y: 0,
            current_color: Color::color(Color::White, Color::Black),
        }
    }

    pub fn update_cursor(&mut self) {
        Cursor::update_cursor(self.current_x as usize, self.current_y as usize);
    }

    pub fn current_writing_position(&mut self) -> (u16, u16) {
        (self.current_x, self.current_y)
    }

    /// Doesn't update the cursor
    pub fn set_writing_position(&mut self, x: i16, y: i16) {
        self.set_writing_column(x);
        self.set_writing_row(y);
    }

    /// Doesn't update the cursor
    pub fn set_writing_column(&mut self, x: i16) {
        let x = x % (VGA_WIDTH as i16);
        self.current_x = (((VGA_WIDTH as i16) + x) as u16) % (VGA_WIDTH as u16);
    }

    /// Doesn't update the cursor
    pub fn set_writing_row(&mut self, y: i16) {
        let y = y % (VGA_HEIGHT as i16);
        self.current_y = (((VGA_HEIGHT as i16) + y) as u16) % (VGA_HEIGHT as u16);
    }

    /// Doesn't update the cursor
    pub fn carriage_return(&mut self) {
        self.current_x = 0;
    }

    /// Doesn't update the cursor
    pub fn line_feed(&mut self) {
        self.current_y += 1;
        if self.current_y as usize == VGA_HEIGHT {
            self.scroll(1);
        }
    }

    pub fn clear(&mut self) {
        unsafe {
            for i in 0..(VGA_WIDTH * VGA_HEIGHT) {
                video_memory![i].character = 0;
                video_memory![i].color = self.current_color;
            }
        }
        self.current_x = 0;
        self.current_y = 0;
        self.update_cursor();
    }

    pub fn write_char(&mut self, character: u8) {
        self.write_char0(character);
        self.update_cursor();
    }

    pub fn scroll(&mut self, amount: u16) {
        if amount == 0 {
            return;
        }
        if amount >= (VGA_HEIGHT as u16) {
            unsafe {
                for i in 0..(VGA_WIDTH * VGA_HEIGHT) {
                    video_memory![i].character = 0;
                    video_memory![i].color = self.current_color;
                }
            }
            self.current_y = 0;
            return;
        }
        let remaining_lines = (VGA_HEIGHT as u16) - amount;
        let remaining_chars = remaining_lines * (VGA_WIDTH as u16);
        unsafe {
            for i in 0..(remaining_chars as usize) {
                *video_memory![i] = *video_memory![VGA_SIZE - (remaining_chars as usize) + i];
            }
            for i in (remaining_chars as usize)..VGA_SIZE {
                video_memory![i].character = 0;
                video_memory![i].color = self.current_color;
            }
        }
        self.current_y -= amount;
    }

    pub fn current_position(&self) -> u16 {
        self.current_y * (VGA_WIDTH as u16) + self.current_x
    }

    fn write_char0(&mut self, character: u8) {
        if character == b'\r' {
            self.current_x = 0;
        } else if character == b'\n' {
            if self.current_y == (VGA_HEIGHT - 1) as u16 {
                self.scroll(1);
            }
            self.current_y += 1;
            self.current_x = 0;
        } else {
            if self.current_x == VGA_WIDTH as u16 {
                self.current_x = 0;
                if self.current_y == (VGA_HEIGHT - 1) as u16 {
                    self.scroll(1);
                }
                self.current_y += 1;
            }
            unsafe {
                let pos = self.current_position() as usize;
                video_memory![pos].character = character;
                video_memory![pos].color = self.current_color;
            }
            self.current_x += 1;
        }
    }

    /// # Safety
    /// This function reads memory from the given pointer until it encounters a null byte. Make absolutely sure your string is null terminated !
    pub unsafe fn write_c_string(&mut self, mut string: *const u8) {
        while *string > 0 {
            self.write_char0(*string);
            string = string.add(1);
        }
        self.update_cursor();
    }

    pub fn write_string(&mut self, string: &[u8]) {
        for c in string.iter() {
            self.write_char0(*c);
        }
        self.update_cursor();
    }

    pub fn write_centered(&mut self, string: &[u8]) {
        if string.len() > VGA_WIDTH {
            self.write_string(string);
            return;
        }
        self.current_x = ((VGA_WIDTH - string.len()) >> 1) as u16;
        for c in string.iter() {
            self.write_char0(*c);
        }
        self.update_cursor();
    }

    pub fn clear_line(&mut self, line: u16) {
        unsafe {
            for i in 0..VGA_WIDTH {
                video_memory![i + line as usize * VGA_WIDTH].character = 0;
                video_memory![i + line as usize * VGA_WIDTH].color = self.current_color;
            }
        }
    }

    pub fn clear_current_line(&mut self) {
        self.clear_line(self.current_y);
    }

    pub fn write_centered_line(&mut self, string: &[u8]) {
        self.clear_current_line();
        self.write_centered(string);
        self.line_feed();
    }

    pub fn write_hex_u8(&mut self, value: u8) {
        self.write_char0(get_hex_digit((value >> 4) & 0xF));
        self.write_char0(get_hex_digit(value & 0xF));
        self.update_cursor();
    }

    pub fn write_string_bounded(&mut self, string: &[u8], index: usize, length: usize) {
        for c in string.iter().skip(index).take(length) {
            self.write_char0(*c);
        }
        self.update_cursor();
    }

    pub fn set_foreground_color(&mut self, color: Color) {
        self.current_color = (self.current_color & 0xF0) | Color::fg(color);
    }

    pub fn set_background_color(&mut self, color: Color) {
        self.current_color = (self.current_color & 0x0F) | Color::bg(color);
    }

    pub fn set_color(&mut self, foreground: Color, background: Color) {
        self.current_color = Color::color(foreground, background);
    }

    pub fn set_color_u8(&mut self, color: u8) {
        self.current_color = color;
    }
}
