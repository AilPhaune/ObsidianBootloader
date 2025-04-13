use core::{
    ops::{Deref, DerefMut},
    ptr,
};

use crate::{
    bios::{unsafe_call_bios_interrupt, BiosInterruptResult},
    eflags, kpanic, ptr_to_seg_off,
    video::Video,
};

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct SystemMemoryMap {
    base_addr_lo: u32,
    base_addr_hi: u32,
    len_lo: u32,
    len_hi: u32,
    range_type: u32,
}

impl SystemMemoryMap {
    pub fn base_addr(&self) -> u64 {
        (self.base_addr_hi as u64) << 32 | self.base_addr_lo as u64
    }
    pub fn len(&self) -> u64 {
        (self.len_hi as u64) << 32 | self.len_lo as u64
    }
}

pub const RANGE_TYPE_AVAILABLE: u32 = 0x1;
pub const RANGE_TYPE_RESERVED: u32 = 0x2;
pub const RANGE_TYPE_ACPI_RECLAIM: u32 = 0x3;
pub const RANGE_TYPE_ACPI_NVS: u32 = 0x4;

static mut SYSTEM_MEMORY_MAP: [SystemMemoryMap; 64] = [SystemMemoryMap {
    base_addr_lo: 0,
    base_addr_hi: 0,
    len_lo: 0,
    len_hi: 0,
    range_type: 0,
}; 64];
static mut USED_MAP: usize = 0;

const SMAP: usize = 0x534D4150;

pub fn detect_system_memory(bios_idt: usize) -> Result<(), u8> {
    unsafe {
        let video = Video::get();
        video.write_string(b"Detecting system memory...\n");

        let mut index = 0;
        let mut start_addr = 0;

        loop {
            if index >= 64 {
                break;
            }
            let map = &mut SYSTEM_MEMORY_MAP[index];
            let (seg, off) = ptr_to_seg_off(map as *const SystemMemoryMap as usize);

            let result = unsafe_call_bios_interrupt(
                bios_idt,
                0x15,
                0xe820,
                start_addr,
                20,
                SMAP,
                0,
                off as usize,
                seg as usize,
                seg as usize,
                0,
                0,
            ) as *const BiosInterruptResult;

            if ((*result).eflags & eflags::CF) != 0 {
                return Err((((*result).eax & 0xFF00) >> 8) as u8);
            }

            if map.base_addr() >= 1024 * 1024
                && map.base_addr_hi == 0
                && map.range_type == RANGE_TYPE_AVAILABLE
            {
                let max_available = (u32::MAX as u64) - map.len();
                let available = max_available.min(map.len());

                if USED_MAP < 64 && available > SYSTEM_MEMORY_MAP[USED_MAP].len() {
                    USED_MAP = index;
                }
            } else {
                video.write_string(b"Skipped 0x");
                video.write_hex_u32(map.base_addr_hi);
                video.write_hex_u32(map.base_addr_lo);
                video.write_string(b" | Length 0x");
                video.write_hex_u32(map.len_hi);
                video.write_hex_u32(map.len_lo);
                video.write_string(b" | Type 0x");
                video.write_hex_u32(map.range_type);
                video.write_char(b'\n');
            }

            start_addr = (*result).ebx;
            if start_addr == 0 {
                break;
            }

            index += 1;
        }

        if USED_MAP < 64 {
            let map = &mut SYSTEM_MEMORY_MAP[USED_MAP];
            video.write_string(b"Using 0x");
            video.write_hex_u32(map.len_hi);
            video.write_hex_u32(map.len_lo);
            video.write_string(b" bytes of contiguous memory at 0x");
            video.write_hex_u32(map.base_addr_lo);
            video.write_char(b'\n');

            let header = get_first_header();
            // Aligned to 4Kb
            let max_addr = (u32::MAX as u64).min(map.base_addr() + map.len()) as usize;

            *header = MemoryBlock {
                size: max_addr - (header as usize) - size_of::<MemoryBlock>(),
                free: 1,
                prev: ptr::null_mut(),
                next: ptr::null_mut(),
            };
        }

        Ok(())
    }
}

fn get_mem_map() -> SystemMemoryMap {
    unsafe {
        if USED_MAP < 64 {
            SYSTEM_MEMORY_MAP[USED_MAP]
        } else {
            kpanic()
        }
    }
}

static mut MEM_USED: usize = 0;

pub fn get_mem_used() -> usize {
    unsafe { MEM_USED }
}

pub fn get_mem_total() -> usize {
    let base_addr = get_mem_map().base_addr();
    let end_addr = base_addr + get_mem_map().len();
    let end_addr_effective = end_addr.min(usize::MAX as u64);

    if end_addr_effective < base_addr {
        0
    } else {
        (end_addr_effective - base_addr) as usize
    }
}

pub fn get_mem_free() -> usize {
    get_mem_total() - get_mem_used()
}

#[no_mangle]
#[inline(never)]
/// # Safety
/// Copies `size` bytes from `src` to `dst`
pub unsafe fn memcpy(dst: usize, src: usize, size: usize) {
    mem_cpy(dst as *mut u8, src as *const u8, size);
}

#[no_mangle]
#[inline(never)]
/// # Safety
/// Fills `count` bytes into `dst` with the given `value`
pub unsafe fn memset(dst: usize, value: u8, count: usize) {
    for i in 0..count {
        *((dst + i) as *mut u8) = value;
    }
}

#[no_mangle]
#[inline(never)]
/// # Safety
/// Fills `count` bytes into `dst` with the given `value`
pub unsafe fn memcmp(a: usize, b: usize, count: usize) -> isize {
    let mut p = a as *const u8;
    let mut q = b as *const u8;
    let mut c = count;
    loop {
        if c == 0 {
            break 0;
        }
        let va = *p;
        let vb = *q;
        if *p != *q {
            break if va < vb { -1 } else { 1 };
        }
        p = p.add(1);
        q = q.add(1);
        c -= 1;
    }
}

/// # Safety
/// Copies `size` bytes from `src` to `dst`
pub unsafe fn mem_cpy<A, B>(dst: *mut A, src: *const B, size: usize) {
    let dst = dst as *mut u8;
    let src = src as *const u8;
    for i in 0..size {
        *dst.add(i) = *src.add(i);
    }
}

/// # Safety
/// Fills `count` amount of A, into `dst` with the given A `value`]
pub unsafe fn mem_set<A>(dst: *mut A, value: A, count: usize)
where
    A: Copy,
{
    for i in 0..count {
        *dst.add(i) = value;
    }
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct MemoryBlock {
    size: usize,
    free: u8,
    prev: *mut MemoryBlock,
    next: *mut MemoryBlock,
}

fn get_first_header() -> *mut MemoryBlock {
    let mem = get_mem_map();
    let base_addr = mem.base_addr() as usize;
    // Find first 4Kb aligned address
    let aligned_addr = (base_addr & !(0x1000 - 1)) + 0x1000;
    let header_size = size_of::<MemoryBlock>();
    let first_header = if aligned_addr - header_size > base_addr {
        aligned_addr - header_size
    } else {
        (aligned_addr + 0x1000) - header_size
    };
    first_header as *mut MemoryBlock
}

fn mem_alloc<T>(size: usize) -> Option<*mut T> {
    let header_size = size_of::<MemoryBlock>();
    let mut header = get_first_header();
    loop {
        let mut header_v = unsafe { header.read_unaligned() };
        if header_v.free != 0 && header_v.size >= size {
            header_v.free = 0;
            unsafe {
                header.write_unaligned(header_v);
            }
            // Split the header
            let header_end = (header as usize) + header_v.size;
            let desired_end = (header as usize) + size;
            let mut next_header = (desired_end & !(0x1000 - 1)) + 0x1000 - header_size;
            while next_header <= desired_end {
                next_header += 0x1000;
            }
            // Have a valid header address now
            if next_header + header_size < header_end {
                // Split
                header_v.size = next_header - (header as usize) - header_size;
                let next2_addr = header_v.next;
                let new_header = MemoryBlock {
                    free: 1,
                    prev: header,
                    next: next2_addr,
                    size: header_end - next_header - header_size,
                };
                unsafe {
                    (next_header as *mut MemoryBlock).write_unaligned(new_header);

                    if !next2_addr.is_null() {
                        let mut next2 = next2_addr.read_unaligned();
                        next2.prev = next_header as *mut MemoryBlock;
                        next2_addr.write_unaligned(next2);
                    }

                    header_v.next = next_header as *mut MemoryBlock;
                    header.write_unaligned(header_v);
                }
            }
            // Else no split
            unsafe {
                MEM_USED += header_v.size + header_size;
            }
            let ptr = ((header as usize) + header_size) as *mut T;
            return Some(ptr);
        }
        if header_v.next.is_null() {
            return None;
        }
        header = header_v.next;
    }
}

fn mem_free<T>(ptr: *mut T) {
    if ptr.is_null() {
        return;
    }
    let header_size = size_of::<MemoryBlock>();
    let header = ((ptr as usize) - header_size) as *mut MemoryBlock;

    let mut header_v = unsafe { header.read_unaligned() };
    header_v.free = 1;

    unsafe {
        MEM_USED -= header_v.size + header_size;
        header.write_unaligned(header_v);
    };

    // Merge with next block if free
    if !header_v.next.is_null() {
        let next_header = header_v.next;
        let next_header_v = unsafe { next_header.read_unaligned() };
        if next_header_v.free != 0 {
            // Update size
            header_v.size += next_header_v.size + header_size;
            header_v.next = next_header_v.next;
            // If there is a block after the one we've merged, make it's prev pointer point to us
            if !header_v.next.is_null() {
                let mut next_v = unsafe { header_v.next.read_unaligned() };
                next_v.prev = header;
                // Save the data to the pointer
                unsafe { header_v.next.write_unaligned(next_v) };
            }
            // Save the data to the pointer
            unsafe { header.write_unaligned(header_v) };
        }
    }

    // Merge with previous block if free
    if !header_v.prev.is_null() {
        let prev_header = header_v.prev;
        let mut prev_header_v = unsafe { prev_header.read_unaligned() };
        if prev_header_v.free != 0 {
            // Update prev's size, as we get deleted
            prev_header_v.size += header_v.size + header_size;
            prev_header_v.next = header_v.next;
            // If there's a block after us, make it's prev pointer point to the merged block
            if !header_v.next.is_null() {
                let mut next_v = unsafe { header_v.next.read_unaligned() };
                next_v.prev = prev_header;
                // Save the data to the pointer
                unsafe { header_v.next.write_unaligned(next_v) };
            }
            // Save the data to the pointer
            unsafe { prev_header.write_unaligned(prev_header_v) };
        }
    }
}

/// # Safety
/// ptr must be a pointer returned by malloc
unsafe fn mem_realloc<T>(ptr: *mut T, size: usize) -> Result<*mut T, *mut T> {
    let header_size = size_of::<MemoryBlock>();
    let header = ((ptr as usize) - header_size) as *mut MemoryBlock;

    let mut header_v = unsafe { header.read_unaligned() };

    // Case 1: The current block is already large enough to fit the requested size.
    if header_v.size >= size {
        return Ok(ptr);
    }

    // Case 2: Try to merge with the next free block if possible.
    if !header_v.next.is_null() {
        let next_header = header_v.next;
        let next_header_v = unsafe { next_header.read_unaligned() };
        if next_header_v.free != 0 {
            header_v.size += next_header_v.size + header_size;
            header_v.next = next_header_v.next;
            if !header_v.next.is_null() {
                let mut next_v = unsafe { header_v.next.read_unaligned() };
                next_v.prev = header;
                unsafe { header_v.next.write_unaligned(next_v) };
            }
            unsafe { header.write_unaligned(header_v) };
        }
    }

    // Case 3: The block is now large enough to fit the requested size.
    if header_v.size >= size {
        return Ok(ptr);
    }

    // Case 4: Allocate new memory for the requested size.
    let new_memory = mem_alloc::<T>(size).ok_or(ptr)?;
    // Copy data from the old memory to the new memory.
    mem_cpy(new_memory, ptr, header_v.size);
    // Free the old memory.
    mem_free(ptr);

    Ok(new_memory)
}

pub struct Box<T>
where
    T: Sized,
{
    ptr: *mut T,
}

impl<T> Box<T>
where
    T: Sized,
{
    pub fn new(value: T) -> Option<Self> {
        unsafe {
            let ptr = mem_alloc::<T>(size_of::<T>())?;
            *ptr = value;
            Some(Self { ptr })
        }
    }

    /// # Safety
    /// ptr must be a pointer returned by malloc and point to a valid and initialized T
    /// ptr is invalidated when this Box is dropped
    pub unsafe fn from_raw(ptr: *mut T) -> Self {
        if !ptr.is_aligned() {
            unsafe {
                Video::get().write_string(b"Unaligned pointer.\r\n");
            }
            kpanic();
        }
        Self { ptr }
    }
}

impl<T> Box<T>
where
    T: Sized + Clone,
{
    pub fn try_clone(&self) -> Option<Self> {
        Self::new(self.deref().clone())
    }
}

impl<T> Drop for Box<T>
where
    T: Sized,
{
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            mem_free(self.ptr);
        }
    }
}

impl<T> Deref for Box<T>
where
    T: Sized,
{
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.ptr }
    }
}

impl<T> DerefMut for Box<T>
where
    T: Sized,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.ptr }
    }
}

impl<T> Clone for Box<T>
where
    T: Sized + Clone,
{
    fn clone(&self) -> Self {
        self.try_clone().unwrap_or_else(|| kpanic())
    }
}

pub struct Vec<T>
where
    T: Sized,
{
    ptr: *mut T,
    len: usize,
    cap: usize,
}

impl<T> Default for Vec<T>
where
    T: Sized,
{
    fn default() -> Self {
        Self::new(16)
    }
}

impl<T> Vec<T>
where
    T: Sized,
{
    #[inline(always)]
    pub fn get_element_size_bytes() -> usize {
        let raw_size = size_of::<T>();
        let alignment = align_of::<T>();
        if raw_size % alignment == 0 {
            raw_size
        } else {
            raw_size + alignment - (raw_size % alignment)
        }
    }

    pub fn new(capacity: usize) -> Self {
        if capacity == 0 {
            kpanic();
        }
        Self {
            ptr: mem_alloc(capacity * Vec::<T>::get_element_size_bytes())
                .unwrap_or_else(|| kpanic()),
            len: 0,
            cap: capacity,
        }
    }

    pub fn ensure_capacity(&mut self, capacity: usize) {
        if self.cap < capacity {
            unsafe {
                self.ptr = mem_realloc(self.ptr, capacity * Vec::<T>::get_element_size_bytes())
                    .unwrap_or_else(|_| kpanic());
            }
        }
    }

    pub fn grow(&mut self, capacity: usize) {
        if self.cap >= capacity {
            return;
        }
        while self.cap < capacity {
            self.cap *= 2;
        }
        unsafe {
            self.ptr = mem_realloc(self.ptr, self.cap * Vec::<T>::get_element_size_bytes())
                .unwrap_or_else(|_| kpanic());
        }
    }

    #[inline(always)]
    fn get_ptr_for_idx(&self, idx: usize) -> *mut T {
        ((self.ptr as usize) + idx * Vec::<T>::get_element_size_bytes()) as *mut T
    }

    pub fn push(&mut self, value: T) {
        self.grow(self.len + 1);
        unsafe {
            *self.get_ptr_for_idx(self.len) = value;
        }
        self.len += 1;
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn capacity(&self) -> usize {
        self.cap
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len {
            return None;
        }
        unsafe { Some(&*self.get_ptr_for_idx(index)) }
    }

    pub fn pop(&mut self) -> Option<T> {
        if self.is_empty() {
            return None;
        }

        self.len -= 1;

        unsafe {
            let ptr = self.get_ptr_for_idx(self.len);
            let value = ptr.read();
            Some(value)
        }
    }

    pub fn iter<'a>(&'a self) -> RefIterVec<'a, T> {
        RefIterVec { vec: self, idx: 0 }
    }
}

impl<T> Drop for Vec<T>
where
    T: Sized,
{
    fn drop(&mut self) {
        mem_free(self.ptr);
    }
}

pub struct RefIterVec<'a, T>
where
    T: Sized,
{
    vec: &'a Vec<T>,
    idx: usize,
}

impl<'a, T> Iterator for RefIterVec<'a, T>
where
    T: Sized,
{
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        let res = self.vec.get(self.idx)?;
        self.idx += 1;
        Some(res)
    }
}

pub struct IterVec<T>
where
    T: Sized,
{
    vec: Vec<T>,
    idx: usize,
}

impl<T> Iterator for IterVec<T>
where
    T: Sized,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.idx >= self.vec.len {
            None
        } else {
            self.idx += 1;
            Some(unsafe { self.vec.ptr.add(self.idx - 1).read_unaligned() })
        }
    }
}

impl<T> IntoIterator for Vec<T>
where
    T: Sized,
{
    type Item = T;
    type IntoIter = IterVec<T>;

    fn into_iter(self) -> Self::IntoIter {
        IterVec { vec: self, idx: 0 }
    }
}

pub struct Buffer {
    ptr: *mut u8,
    len: usize,
    owns_data: bool,
}

impl Buffer {
    pub fn new(len: usize) -> Option<Self> {
        let ptr = mem_alloc(len)?;
        Some(Self {
            ptr,
            len,
            owns_data: true,
        })
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn get(&self, index: usize) -> Option<u8> {
        if index >= self.len {
            return None;
        }
        unsafe { Some(*self.ptr.add(index)) }
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut u8> {
        if index >= self.len {
            return None;
        }
        unsafe { Some(&mut *self.ptr.add(index)) }
    }

    /// # Safety
    /// Pointer must be handled safely by the caller
    /// Pointer is invalid after this buffer is dropped
    pub unsafe fn get_ptr(&self) -> *mut u8 {
        self.ptr
    }

    pub fn copy_to(
        &self,
        src_offset: usize,
        dst: &mut Buffer,
        dst_offset: usize,
        count: usize,
    ) -> bool {
        if self.len < src_offset + count || dst.len < dst_offset + count {
            false
        } else {
            unsafe {
                mem_cpy(dst.ptr.add(dst_offset), self.ptr.add(src_offset), count);
            }
            true
        }
    }

    pub fn iter<'b>(&'b self) -> IterBuffer<'b> {
        IterBuffer { vec: self, idx: 0 }
    }

    pub fn iter_mut<'a>(&'a mut self) -> IterBufferMut<'a> {
        IterBufferMut { vec: self, idx: 0 }
    }

    pub fn boxed<T>(mut self) -> Box<T> {
        let ptr = self.ptr;
        self.ptr = ptr::null_mut();
        unsafe { Box::from_raw(ptr as *mut T) }
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        if self.owns_data {
            mem_free(self.ptr);
        }
    }
}

pub struct IterBuffer<'a> {
    vec: &'a Buffer,
    idx: usize,
}

impl<'a> Iterator for IterBuffer<'a> {
    type Item = u8;

    fn next(&mut self) -> Option<Self::Item> {
        let res = self.vec.get(self.idx)?;
        self.idx += 1;
        Some(res)
    }
}

pub struct IterBufferMut<'a> {
    vec: &'a mut Buffer,
    idx: usize,
}

impl<'a> Iterator for IterBufferMut<'a> {
    type Item = &'a mut u8;

    fn next(&mut self) -> Option<Self::Item> {
        if self.idx >= self.vec.len {
            return None;
        }
        let res: &'a mut u8 = unsafe { &mut *self.vec.ptr.add(self.idx) };
        self.idx += 1;
        Some(res)
    }
}
