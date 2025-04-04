extern "cdecl" {
    pub fn outb(port: u16, value: u8);
    pub fn outw(port: u16, value: u16);
    pub fn outl(port: u16, value: u32);
    pub fn inb(port: u16) -> u8;
    pub fn inw(port: u16) -> u16;
    pub fn inl(port: u16) -> u32;
}

const UNUSED_PORT: u16 = 0x80;
pub fn iowait() {
    unsafe { outb(UNUSED_PORT, 0) };
}
