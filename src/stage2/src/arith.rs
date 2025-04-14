#[no_mangle]
pub extern "C" fn __udivdi3(mut n: u64, d: u64) -> u64 {
    if d == 0 {
        // Optional: halt, panic, or return max
        return u64::MAX;
    }

    let mut q = 0u64;
    let mut r = 0u64;

    for _ in 0..64 {
        r = (r << 1) | (n >> 63);
        n <<= 1;
        q <<= 1;

        if r >= d {
            r -= d;
            q |= 1;
        }
    }

    q
}
