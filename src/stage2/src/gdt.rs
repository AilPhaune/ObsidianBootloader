use core::{arch::x86::__cpuid, ptr::addr_of};

use dc_access::{ACCESSED, CODE_READ, CODE_SEGMENT, DATA_SEGMENT, DATA_WRITE, PRESENT, RING0};
use flags::{GRANULARITY_4KB, IS_32BIT, LONG_MODE};

use crate::{e9::write_u8_decimal, printf};

extern "cdecl" {
    fn check_cpuid_supported() -> usize;
}

pub fn is_cpuid_supported() -> bool {
    unsafe { check_cpuid_supported() != 0 }
}

pub fn is_long_mode_supported() -> bool {
    let cpuid = unsafe { __cpuid(0x80000001) };
    (cpuid.edx & (1 << 29)) != 0
}

pub mod dc_access {
    pub const PRESENT: u8 = 1 << 7;
    pub const RING0: u8 = 0 << 5;
    pub const RING1: u8 = 1 << 5;
    pub const RING2: u8 = 2 << 5;
    pub const RING3: u8 = 3 << 5;
    pub const CODE_SEGMENT: u8 = 0b0001_1000;
    pub const DATA_SEGMENT: u8 = 0b0001_0000;
    pub const DATA_DIRECTION_DOWN: u8 = 1 << 2;
    pub const CODE_DPL: u8 = 1 << 2;
    pub const CODE_READ: u8 = 1 << 1;
    pub const DATA_WRITE: u8 = 1 << 1;
    pub const ACCESSED: u8 = 1 << 0;
}

pub mod flags {
    pub const GRANULARITY_4KB: u8 = 0b1000;
    pub const IS_32BIT: u8 = 0b0100;
    pub const LONG_MODE: u8 = 0b0010;
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct GdtDescriptor {
    limit: u16,
    base: u64,
}

#[derive(Clone, Copy)]
struct GdtEntry {
    limit_low: u16,
    base_low: u16,
    base_mid: u8,
    access: u8,
    flags_limit_high: u8,
    base_high: u8,
}

impl GdtEntry {
    const fn new(base: u32, limit: u32, access: u8, flags: u8) -> GdtEntry {
        GdtEntry {
            limit_low: (limit & 0xFFFF) as u16,
            base_low: (base & 0xFFFF) as u16,
            base_mid: ((base >> 16) & 0xFF) as u8,
            access,
            flags_limit_high: (((limit >> 16) & 0x0F) as u8) | (flags << 4),
            base_high: ((base >> 24) & 0xFF) as u8,
        }
    }

    const fn into(self) -> u64 {
        self.limit_low as u64
            | (self.base_low as u64) << 16
            | (self.base_mid as u64) << 24
            | (self.access as u64) << 40
            | (self.flags_limit_high as u64) << 48
            | (self.base_high as u64) << 56
    }
}

#[repr(align(8))]
struct GdtAligned([u64; 7]);

static mut GDT: GdtAligned = GdtAligned([
    GdtEntry::new(0, 0, 0, 0).into(), // Null descriptor
    GdtEntry::new(
        0,
        u32::MAX,
        PRESENT | RING0 | CODE_SEGMENT | CODE_READ | ACCESSED,
        GRANULARITY_4KB | IS_32BIT,
    )
    .into(), // 32-bit Code
    GdtEntry::new(
        0,
        u32::MAX,
        PRESENT | RING0 | DATA_SEGMENT | DATA_WRITE | ACCESSED,
        GRANULARITY_4KB | IS_32BIT,
    )
    .into(), // 32-bit Data
    GdtEntry::new(
        0,
        u32::MAX,
        PRESENT | RING0 | CODE_SEGMENT | CODE_READ | ACCESSED,
        0,
    )
    .into(), // 16-bit Code
    GdtEntry::new(
        0,
        u32::MAX,
        PRESENT | RING0 | DATA_SEGMENT | DATA_WRITE | ACCESSED,
        0,
    )
    .into(), // 16-bit Data
    GdtEntry::new(
        0,
        u32::MAX,
        PRESENT | RING0 | CODE_SEGMENT | CODE_READ | ACCESSED,
        GRANULARITY_4KB | LONG_MODE,
    )
    .into(), // 64-bit Code
    GdtEntry::new(
        0,
        u32::MAX,
        PRESENT | RING0 | DATA_SEGMENT | DATA_WRITE | ACCESSED,
        GRANULARITY_4KB | LONG_MODE,
    )
    .into(), // 64-bit Data
]);

pub const CODE16_SELECTOR: usize = 0x18;
pub const CODE32_SELECTOR: usize = 0x08;
pub const CODE64_SELECTOR: usize = 0x28;

pub const DATA16_SELECTOR: usize = 0x20;
pub const DATA32_SELECTOR: usize = 0x10;
pub const DATA64_SELECTOR: usize = 0x30;

#[no_mangle]
pub static mut GDTR: GdtDescriptor = GdtDescriptor { limit: 0, base: 0 };

#[allow(static_mut_refs)]
pub(crate) unsafe fn init_gdtr() {
    GDTR = GdtDescriptor {
        limit: size_of::<[GdtEntry; 7]>() as u16 - 1,
        base: GDT.0.as_ptr() as u64,
    };

    printf!(b"GDT at 0x%x\r\n", GDTR.base as usize);
    for i in 0..7 {
        printf!(b"  Descriptor ");
        write_u8_decimal(i as u8);
        printf!(b": 0x%x%x\r\n", (GDT.0[i] >> 32) as u32, GDT.0[i] as u32);
    }
    printf!(b"GDTR at 0x%x\r\n", addr_of!(GDTR) as usize);
}
