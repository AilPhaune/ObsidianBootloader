use core::arch::x86::__cpuid;

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
