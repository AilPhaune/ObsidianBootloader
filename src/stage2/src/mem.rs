use core::ops::{Deref, DerefMut};

use crate::{bios::{unsafe_call_bios_interrupt, BiosInterruptResult}, eflags, kpanic, ptr_to_seg_off, video::Video};

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

static mut SYSTEM_MEMORY_MAP: [SystemMemoryMap; 64] = [SystemMemoryMap { base_addr_lo: 0, base_addr_hi: 0, len_lo: 0, len_hi: 0, range_type: 0 }; 64];
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
                bios_idt, 0x15,
                0xe820, start_addr, 20, SMAP, 0, off as usize, seg as usize, seg as usize, 0, 0
            ) as *const BiosInterruptResult;

            if ((*result).eflags & eflags::CF) != 0 {
                return Err((((*result).eax & 0xFF00) >> 8) as u8);
            }
            
            if map.base_addr() >= 1024*1024 && map.base_addr_hi == 0 && map.range_type == RANGE_TYPE_AVAILABLE {
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

            let base_block = (map.base_addr() as usize) as *mut MemoryBlock;
            *base_block = MemoryBlock {
                size: map.len() as usize - size_of::<MemoryBlock>(),
                flags: BLOCK_FLAG_FREE,
                prev: core::ptr::null_mut(),
                next: core::ptr::null_mut(),
            };
        }

        Ok(())
    }
}


#[repr(C, packed)]
#[derive(Clone, Copy)]
struct MemoryBlock {
    size: usize,
    flags: u8,
    prev: *mut MemoryBlock,
    next: *mut MemoryBlock,
}

const BLOCK_FLAG_FREE: u8 = 0x1;
const BLOCK_FLAG_HAS_NEXT: u8 = 0x2;
const BLOCK_FLAG_HAS_PREV: u8 = 0x4;

impl MemoryBlock {
    pub fn is_free(&self) -> bool {
        (self.flags & BLOCK_FLAG_FREE) != 0
    }

    pub fn set_free(&mut self) {
        self.flags |= BLOCK_FLAG_FREE;
    }

    pub fn set_used(&mut self) {
        self.flags &= !BLOCK_FLAG_FREE;
    }

    pub fn has_prev(&self) -> bool {
        (self.flags & BLOCK_FLAG_HAS_PREV) != 0
    }

    pub fn has_next(&self) -> bool {
        (self.flags & BLOCK_FLAG_HAS_NEXT) != 0
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

pub fn mem_used() -> usize {
    unsafe { MEM_USED }
}

pub fn mem_total() -> usize {
    let base_addr = get_mem_map().base_addr();
    let end_addr = base_addr + get_mem_map().len();
    let end_addr_effective = end_addr.min(usize::MAX as u64);
    
    if end_addr_effective < base_addr {
        0
    } else {
        (end_addr_effective - base_addr) as usize
    }
}

pub fn mem_free() -> usize {
    mem_total() - mem_used()
}

/// # Safety
/// Copies `size` bytes from `src` to `dst`
pub unsafe fn memcpy<A, B>(dst: *mut A, src: *const B, size: usize) {
    let dst = dst as *mut u8;
    let src = src as *const u8;
    for i in 0..size {
        *dst.add(i) = *src.add(i);
    }
}

unsafe fn split_block(block: *mut MemoryBlock, size: usize) {
    if (*block).size > size + size_of::<MemoryBlock>() {
        // Split block
        let new_block = (block as *mut u8).add(size + size_of::<MemoryBlock>()) as *mut MemoryBlock;
        *new_block = MemoryBlock {
            size: (*block).size - size - size_of::<MemoryBlock>(),
            flags: BLOCK_FLAG_FREE | BLOCK_FLAG_HAS_PREV,
            prev: block,
            next: (*block).next,
        };
        if (*block).has_next() {
            (*new_block).flags |= BLOCK_FLAG_HAS_NEXT;
            (*(*new_block).next).prev = new_block;
        }
        (*block).next = new_block;
        (*block).flags |= BLOCK_FLAG_HAS_NEXT;
        (*block).size = size;
    }
}

unsafe fn merge_block_with_next(block: *mut MemoryBlock) {
    let next = (*block).next;
    (*block).size += (*next).size + size_of::<MemoryBlock>();
    (*block).next = (*next).next;
    (*block).flags = ((*block).flags & !BLOCK_FLAG_HAS_NEXT) | ((*next).flags & BLOCK_FLAG_HAS_NEXT);
    if (*next).has_next() {
        (*(*next).next).prev = block;
    }
}

pub fn malloc<T>(size: usize) -> Option<*mut T> {
    let mem_map = get_mem_map();

    unsafe {
        // Search available block
        let mut current_block = mem_map.base_addr() as usize as *mut MemoryBlock;
        while (*current_block).size < size || !(*current_block).is_free() {
            if !(*current_block).has_next() {
                return None;
            }
            current_block = (*current_block).next;
        }

        // Split block
        split_block(current_block, size);

        (*current_block).set_used();
    
        MEM_USED += (*current_block).size + size_of::<MemoryBlock>();
        Some(current_block.offset(1) as *mut T)
    }
}

/// # Safety
/// ptr must be a pointer returned by malloc
pub unsafe fn free<T>(ptr: *mut T) {
    let mut block = (ptr as *mut MemoryBlock).offset(-1);
    (*block).set_free();

    MEM_USED -= (*block).size + size_of::<MemoryBlock>();

    // Merge the block with the previous one if both exist and are free
    if (*block).has_prev() {
        let prev = (*block).prev;
        if (*prev).is_free() {
            merge_block_with_next(prev);
            block = prev;
        }
    }

    // Merge the block with the next one if both exist and are free
    if (*block).has_next() {
        let next = (*block).next;
        if (*next).is_free() {
            merge_block_with_next(block);
        }
    }
}

/// # Safety
/// ptr must be a pointer returned by malloc
pub unsafe fn realloc<T>(ptr: *mut T, size: usize) -> Result<*mut T, *mut T> {
    let mut block = (ptr as *mut MemoryBlock).offset(-1);

    // Case 1: The current block is already large enough to fit the requested size.
    if (*block).size >= size {
        return Ok(ptr);
    }

    // Case 2: Try to merge with the next free block if possible.
    if (*block).has_next() {
        let next = (*block).next;
        if (*next).is_free() {
            merge_block_with_next(block);
            block = next;
        }
    }

    // Case 3: The block is not large enough to fit the requested size.
    if (*block).size >= size {
        return Ok(ptr);
    }

    // Case 4: Allocate new memory for the requested size.
    let new_memory = malloc::<T>(size).ok_or(ptr)?;
    // Copy data from the old memory to the new memory.
    memcpy(new_memory, ptr, (*block).size);
    // Free the old memory.
    free(ptr);

    Ok(new_memory)
}

pub struct Box<T> where T: Sized {
    ptr: *mut T,
}

impl<T> Box<T> where T: Sized {
    pub fn new(value: T) -> Option<Self> {
        unsafe {
            let ptr = malloc::<T>(size_of::<T>())?;
            *ptr = value;
            Some(Self {
                ptr,
            })
        }
    }

    /// # Safety
    /// ptr must be a pointer returned by malloc and point to a valid and initialized T
    pub unsafe fn from_raw(ptr: *mut T) -> Self {
        Self {
            ptr,
        }
    }
}

impl<T> Box<T> where T: Sized + Clone {
    pub fn try_clone(&self) -> Option<Self> {
        Self::new(self.deref().clone())
    }
}

impl<T> Drop for Box<T> where T: Sized {
    fn drop(&mut self) {
        unsafe { free(self.ptr); }
    }
}

impl<T> Deref for Box<T> where T: Sized {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.ptr }
    }
}

impl<T> DerefMut for Box<T> where T: Sized {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.ptr }
    }
}

impl<T> Clone for Box<T> where T: Sized + Clone {
    fn clone(&self) -> Self {
        self.try_clone().unwrap_or_else(|| kpanic())
    }
}

pub struct Vec<T> where T: Sized {
    ptr: *mut T,
    len: usize,
    cap: usize,
}

impl<T> Default for Vec<T> where T: Sized {
    fn default() -> Self {
        Self::new(16)
    }
}

impl<T> Vec<T> where T: Sized {
    pub fn new(capcity: usize) -> Self {
        if capcity == 0 {
            kpanic();
        }
        Self {
            ptr: malloc(size_of::<T>()).unwrap(),
            len: 0,
            cap: capcity,
        }
    }

    pub fn ensure_capacity(&mut self, capacity: usize) {
        if self.cap < capacity {
            unsafe {
                self.ptr = realloc(self.ptr, capacity * size_of::<T>()).unwrap();
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
            self.ptr = realloc(self.ptr, self.cap * size_of::<T>()).unwrap();
        }
    }

    pub fn push(&mut self, value: T) {
        self.grow(self.len + 1);
        unsafe {
            *self.ptr.add(self.len) = value;
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
        unsafe {
            Some(&*self.ptr.add(index))
        }
    }

    pub fn pop(&mut self) -> Option<T> {
        if self.is_empty() {
            return None;
        }

        self.len -= 1;

        unsafe {
            let ptr = self.ptr.add(self.len);
            let value = ptr.read_unaligned();
            Some(value)
        }
    }
}

impl<T> Drop for Vec<T> where T: Sized {
    fn drop(&mut self) {
        unsafe { free(self.ptr); }
    }
}