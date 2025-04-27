use core::arch::{asm, x86::__cpuid};

pub struct ExtensionsStatus {
    pub fpu: bool,
    pub sse: bool,
    pub sse2: bool,
}

unsafe fn check_and_enable_fpu() -> bool {
    let cr0: u32;
    asm!("mov {}, cr0", out(reg) cr0);
    // Clear CR0.EM, set CR0.MP
    let cr0 = (cr0 & !(1 << 2)) | (1 << 1);
    asm!("mov cr0, {}", in(reg) cr0);

    asm!("fninit");

    true
}

unsafe fn check_and_enable_sse() -> bool {
    let result = __cpuid(1);

    if result.edx & (1 << 25) == 0 {
        return false;
    }

    let cr0: u32;
    asm!("mov {}, cr0", out(reg) cr0);
    // Clear CR0.EM, set CR0.MP
    let cr0 = (cr0 & !(1 << 2)) | (1 << 1);
    asm!("mov cr0, {}", in(reg) cr0);

    let cr4: u32;
    asm!("mov {}, cr4", out(reg) cr4);
    let cr4 = cr4 | (0b11 << 9);
    asm!("mov cr4, {}", in(reg) cr4);
    true
}

pub fn check_and_enable_cpu_extensions() -> ExtensionsStatus {
    let mut status = ExtensionsStatus {
        fpu: false,
        sse: false,
        sse2: false,
    };

    unsafe {
        status.fpu = check_and_enable_fpu();
        status.sse = check_and_enable_sse();
    }

    status
}
