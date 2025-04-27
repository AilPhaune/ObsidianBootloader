use core::ptr::addr_of;

use crate::{
    e9::write_u32_decimal,
    elf::{ElfError, ElfFile64, SEGMENT_TYPE_LOAD},
    gdt::{init_gdtr, CODE64_SELECTOR, DATA64_SELECTOR},
    kpanic,
    mem::{self, Buffer, Vec, RANGE_TYPE_AVAILABLE, SYSTEM_MEMORY_MAP, USED_MAP},
    printf,
    video::Video,
};

extern "cdecl" {
    fn enable_paging_and_jump64(
        pml4: usize,
        data_selector: usize,
        code_selector: usize,
        entry64: u64,
        stack_pointer: u64,
        memory_layout: usize,
        memory_layout_entry_count: usize,
        page_alloc_curr: usize,
        page_alloc_end: usize,
        begin_usable_memory: usize,
    ) -> !;
}

#[derive(Copy, Clone)]
pub struct MemoryRegion {
    start: u64,
    end: u64,
    kind: MemoryRegionType,
}

#[repr(C, packed)]
pub struct OsMemoryRegion {
    start: u64,
    end: u64,
    usable: u64,
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum MemoryRegionType {
    Usable,
    Reserved,
}

impl MemoryRegionType {
    fn strictest(&self, other: &MemoryRegionType) -> MemoryRegionType {
        match (self, other) {
            (MemoryRegionType::Usable, MemoryRegionType::Usable) => MemoryRegionType::Usable,
            _ => MemoryRegionType::Reserved,
        }
    }
}

fn overlapping_pass(layout: Vec<MemoryRegion>) -> (Vec<MemoryRegion>, bool) {
    let mut had_overlap = false;
    let mut fixed_layout: Vec<MemoryRegion> = Vec::new(layout.len());
    for region in layout.iter() {
        let current = *region;
        let mut i = 0;
        while i < fixed_layout.len() {
            let existing = fixed_layout.get(i).copied().unwrap_or_else(|| kpanic());

            if current.end <= existing.start || current.start >= existing.end {
                i += 1;
                continue;
            }

            had_overlap = true;

            // Overlap detected
            let min_start = current.start.min(existing.start);
            let max_end = current.end.max(existing.end);

            // Break into three parts: left, overlap, right
            if min_start < current.start {
                fixed_layout.insert(
                    i,
                    MemoryRegion {
                        start: min_start,
                        end: current.start,
                        kind: existing.kind,
                    },
                );
                i += 1;
            }

            let overlap_start = current.start.max(existing.start);
            let overlap_end = current.end.min(existing.end);
            fixed_layout.insert(
                i,
                MemoryRegion {
                    start: overlap_start,
                    end: overlap_end,
                    kind: current.kind.strictest(&existing.kind), // overlap = reserved wins
                },
            );
            i += 1;

            if current.end < max_end {
                fixed_layout.insert(
                    i,
                    MemoryRegion {
                        start: current.end,
                        end: max_end,
                        kind: existing.kind,
                    },
                );
            }

            break;
        }

        if i == fixed_layout.len() {
            fixed_layout.push(current);
        }
    }

    (fixed_layout, had_overlap)
}

fn parse_memory_layout() -> Vec<MemoryRegion> {
    let mut layout: Vec<MemoryRegion> = unsafe {
        #[allow(static_mut_refs)]
        let mut v = Vec::new(SYSTEM_MEMORY_MAP.len());
        #[allow(static_mut_refs)]
        for map in SYSTEM_MEMORY_MAP.iter() {
            if map.is_null() {
                continue;
            }
            v.push(MemoryRegion {
                start: map.base_addr(),
                end: map.base_addr() + map.len(),
                kind: if map.range_type() == RANGE_TYPE_AVAILABLE {
                    MemoryRegionType::Usable
                } else {
                    MemoryRegionType::Reserved
                },
            });
        }
        // 64 elements is small enough to not bother implementing quicksort (sorry)
        v.bubble_sort(|a, b| {
            if a.start < b.start {
                -1
            } else if a.start > b.start {
                1
            } else {
                0
            }
        });
        v
    };

    let ok_layout = loop {
        let (new_layout, had_overlap) = overlapping_pass(layout);
        if !had_overlap {
            break new_layout;
        }
        layout = new_layout;
    };

    let mut done_layout = Vec::new(16);

    let mut last_region = None;

    for region in ok_layout.iter() {
        match last_region {
            None => {
                last_region = Some(*region);
            }
            Some(mut last) => {
                if last.kind == region.kind && last.end == region.start {
                    last.end = region.end;
                } else {
                    done_layout.push(last);
                    last_region = Some(*region);
                }
            }
        }
    }

    if let Some(last) = last_region {
        done_layout.push(last);
    }
    done_layout
}

struct SimpleArenaAllocator {
    end: usize,
    current: usize,
}

impl SimpleArenaAllocator {
    fn new(start: usize, end: usize) -> SimpleArenaAllocator {
        printf!(
            b"Page tables arena allocator from 0x%x to 0x%x\r\n",
            start,
            end
        );
        SimpleArenaAllocator {
            end,
            current: start,
        }
    }

    fn alloc(&mut self, size: usize) -> Option<usize> {
        if self.current + size > self.end {
            None
        } else {
            let ptr = self.current;
            self.current += size;
            Some(ptr)
        }
    }

    fn alloc_page(&mut self) -> *mut u64 {
        let addr = self.alloc(PAGE_SIZE).unwrap_or_else(|| {
            printf!(b"Failed to alloc page (size = 0x%x)\r\n", PAGE_SIZE);
            kpanic();
        });
        unsafe {
            core::ptr::write_bytes(addr as *mut u8, 0, PAGE_SIZE);
        }
        addr as *mut u64
    }
}

static mut PML4: *mut u64 = core::ptr::null_mut();

pub const PAGE_SIZE: usize = 4096;
pub const PAGE_SIZE_2MB: usize = 2 * 1024 * 1024;

// Page Table Entry Flags
pub const PAGE_PRESENT: u64 = 1 << 0;
pub const PAGE_RW: u64 = 1 << 1;
pub const PAGE_USER: u64 = 1 << 2;
pub const PAGE_WRITE_THROUGH: u64 = 1 << 3;
pub const PAGE_CACHE_DISABLE: u64 = 1 << 4;
pub const PAGE_ACCESSED: u64 = 1 << 5;
pub const PAGE_DIRTY: u64 = 1 << 6;
pub const PAGE_HUGE: u64 = 1 << 7;
pub const PAGE_GLOBAL: u64 = 1 << 8;
pub const PAGE_NO_EXECUTE: u64 = 1 << 63;

pub const KB4: usize = 4 * 1024;
pub const MB2: usize = 2 * 1024 * 1024;

// Helper to extract indices for 4-level paging
fn split_virt_addr(addr: u64) -> (usize, usize, usize, usize) {
    let pml4 = ((addr >> 39) & 0x1FF) as usize;
    let pdpt = ((addr >> 30) & 0x1FF) as usize;
    let pd = ((addr >> 21) & 0x1FF) as usize;
    let pt = ((addr >> 12) & 0x1FF) as usize;
    (pml4, pdpt, pd, pt)
}

// Align address down to nearest 4 KiB or 2 MiB
fn align_down(addr: u64, align: u64) -> u64 {
    addr & !(align - 1)
}

// Align address up to nearest 4 KiB or 2 MiB
fn align_up(addr: u64, align: u64) -> u64 {
    (addr + align - 1) & !(align - 1)
}

unsafe fn map_page_4kb(virt: u64, phys: u64, flags: u64, allocator: &mut SimpleArenaAllocator) {
    let (pml4_idx, pdpt_idx, pd_idx, pt_idx) = split_virt_addr(virt);

    let pml4_entry = &mut *PML4.add(pml4_idx);
    let pdpt_ptr = if *pml4_entry & PAGE_PRESENT != 0 {
        (*pml4_entry & 0x000F_FFFF_FFFF_F000) as *mut u64
    } else {
        let new = allocator.alloc_page();
        *pml4_entry = new as u64 | PAGE_PRESENT | PAGE_RW;
        new
    };

    let pdpt_entry = &mut *pdpt_ptr.add(pdpt_idx);
    let pd_ptr = if *pdpt_entry & PAGE_PRESENT != 0 {
        (*pdpt_entry & 0x000F_FFFF_FFFF_F000) as *mut u64
    } else {
        let new = allocator.alloc_page();
        *pdpt_entry = new as u64 | PAGE_PRESENT | PAGE_RW;
        new
    };

    let pd_entry = &mut *pd_ptr.add(pd_idx);
    let pt_ptr = if *pd_entry & PAGE_PRESENT != 0 && *pd_entry & PAGE_HUGE == 0 {
        (*pd_entry & 0x000F_FFFF_FFFF_F000) as *mut u64
    } else {
        let new = allocator.alloc_page();
        *pd_entry = new as u64 | PAGE_PRESENT | PAGE_RW;
        new
    };

    let pt_entry = &mut *pt_ptr.add(pt_idx);
    *pt_entry = align_down(phys, PAGE_SIZE as u64) | flags | PAGE_PRESENT;
}

unsafe fn map_page_2mb(virt: u64, phys: u64, flags: u64, allocator: &mut SimpleArenaAllocator) {
    let (pml4_idx, pdpt_idx, pd_idx, _) = split_virt_addr(virt);

    let pml4_entry = &mut *PML4.add(pml4_idx);
    let pdpt_ptr = if *pml4_entry & PAGE_PRESENT != 0 {
        (*pml4_entry & 0x000F_FFFF_FFFF_F000) as *mut u64
    } else {
        let new = allocator.alloc_page();
        *pml4_entry = new as u64 | PAGE_PRESENT | PAGE_RW;
        new
    };

    let pdpt_entry = &mut *pdpt_ptr.add(pdpt_idx);
    let pd_ptr = if *pdpt_entry & PAGE_PRESENT != 0 {
        (*pdpt_entry & 0x000F_FFFF_FFFF_F000) as *mut u64
    } else {
        let new = allocator.alloc_page();
        *pdpt_entry = new as u64 | PAGE_PRESENT | PAGE_RW;
        new
    };

    let pd_entry = &mut *pd_ptr.add(pd_idx);
    *pd_entry = align_down(phys, PAGE_SIZE_2MB as u64) | flags | PAGE_PRESENT | PAGE_HUGE;
}

const KERNEL_STACK_SIZE: u64 = 2 * MB2 as u64;

static mut KERNEL_BUFFERS: Option<Vec<Buffer>> = None;
static mut KERNEL_MEMORY_LAYOUT: [OsMemoryRegion; 32] = unsafe { core::mem::zeroed() };

fn load_kernel<'a>(
    kernel_file: &'a mut ElfFile64<'a>,
    allocator: &mut SimpleArenaAllocator,
) -> Result<(u64, u64), ElfError> {
    let phs = kernel_file.load_program_headers()?.clone();
    let file = kernel_file.get_file_mut();

    let mut buffers = Vec::new(phs.len());

    let mut max_addr = 0;

    for ph in phs.iter() {
        if ph.p_vaddr + ph.p_memsz > max_addr {
            max_addr = ph.p_vaddr + ph.p_memsz;
        }

        if ph.segment_type != SEGMENT_TYPE_LOAD {
            continue;
        }

        printf!(
            b"Loading segment: v_addr=0x%x%x, p_memsz=0x%x, p_filesz=0x%x\r\n",
            (ph.p_vaddr >> 32) as u32,
            ph.p_vaddr as u32,
            ph.p_memsz as u32,
            ph.p_filesz as u32
        );
        let mut buf = Buffer::new(ph.p_memsz as usize)
            .ok_or(ElfError::FailedMemAlloc(ph.p_memsz as usize))?;
        unsafe { buf.get_ptr().write_bytes(0, ph.p_memsz as usize) };

        let read = {
            file.seek(ph.p_offset as usize)
                .map_err(ElfError::Ext2Error)?;
            file.read(&mut buf, ph.p_filesz as usize)
                .map_err(ElfError::Ext2Error)?
        };
        printf!(
            b"Read 0x%x bytes of 0x%x bytes\r\n",
            read,
            ph.p_filesz as usize
        );

        if read != ph.p_filesz as usize {
            unsafe {
                Video::get().write_string(b"Failed to boot: Could not read kernel !\n");
            }
            kpanic();
        }

        let buf_ptr = unsafe { buf.get_ptr() as u64 };
        let buf_len = buf.len();
        let buf_num_pages = buf_len.div_ceil(KB4);

        printf!(
            b"Mapping kernel (4KiB pages) vaddr=0x%x%x, paddr=0x%x%x, npages=0x%x\r\n",
            (ph.p_vaddr >> 32) as u32,
            ph.p_vaddr as u32,
            (buf_ptr >> 32) as u32,
            buf_ptr as u32,
            buf_num_pages as u32
        );

        for i in 0..buf_num_pages {
            let offset = (i as u64) * (KB4 as u64);
            let virt = ph.p_vaddr + offset;
            let phys = buf_ptr + offset;

            unsafe {
                map_page_4kb(virt, phys, PAGE_RW, allocator);
            }
        }

        buffers.push(buf);
    }

    if max_addr > 0xFFFF_9000_0000_0000 {
        printf!(
            b"Kernel reserves memory until 0x%x%x > 0xFFFF900000000000 !\r\n",
            (max_addr >> 32) as u32,
            max_addr as u32
        );
        kpanic();
    }

    let begin_stack = 0xFFFF_9000_0000_0000;
    let end_stack = begin_stack + KERNEL_STACK_SIZE;

    let stack_buffer = Buffer::new(KERNEL_STACK_SIZE as usize)
        .ok_or(ElfError::FailedMemAlloc(KERNEL_STACK_SIZE as usize))?;

    unsafe {
        printf!(
            b"Mapping kernel stack vaddr=0x%x%x, paddr=0x%x%x, npages=0x%x\r\n",
            (begin_stack >> 32) as u32,
            begin_stack as u32,
            (stack_buffer.get_ptr() as u64 >> 32) as u32,
            stack_buffer.get_ptr() as u32,
            (end_stack - begin_stack).div_ceil(MB2 as u64) as u32
        );

        for i in 0..(end_stack - begin_stack).div_ceil(MB2 as u64) {
            let offset = i * (MB2 as u64);
            let virt = begin_stack + offset;
            let phys = stack_buffer.get_ptr() as u64 + offset;

            map_page_2mb(virt, phys, PAGE_RW, allocator);
        }
        buffers.push(stack_buffer);

        KERNEL_BUFFERS = Some(buffers);
    }

    Ok((end_stack, end_stack))
}

pub const DIRECT_MAPPING_OFFSET: u64 = 0xFFFF_A000_0000_0000;

pub fn enable_paging_and_run_kernel<'a>(kernel_file: &'a mut ElfFile64<'a>) {
    unsafe {
        let entry64 = kernel_file.entry_point();
        printf!(
            b"Kernel entry point is 0x%x%x\r\n\n",
            (entry64 >> 32) as u32,
            entry64 as u32
        );
        if entry64 < 0xFFFF_8000_0000_0000 {
            Video::get().write_string(b"Kernel entry point is < 0xFFFF800000000000 !\r\n");
            kpanic();
        }

        let layout = parse_memory_layout();

        printf!(b"=== BEGIN MEMORY LAYOUT DUMP ===\r\n");
        for region in layout.iter() {
            printf!(
                b"REGION: %x%x --> %x%x (usable:",
                (region.start >> 32) as u32,
                (region.start) as u32,
                (region.end >> 32) as u32,
                (region.end) as u32
            );
            if region.kind == MemoryRegionType::Usable {
                printf!(b"yes)\r\n");
            } else {
                printf!(b"no)\r\n");
            }
        }
        printf!(b"===  END MEMORY LAYOUT DUMP  ===\r\n\n");

        // 15MiB is allocated for page tables
        #[allow(static_mut_refs)]
        if USED_MAP >= SYSTEM_MEMORY_MAP.len() {
            // unreachable, check already made when detecting memory layout from BIOS
            kpanic();
        }
        let tables_base_addr = SYSTEM_MEMORY_MAP[USED_MAP].base_addr();
        let tables_end_addr = tables_base_addr + 15 * 1024 * 1024;
        if tables_base_addr > tables_end_addr || tables_end_addr > u32::MAX as u64 {
            printf!(
                b"Invalid memory range for page tables: %x%x --> %x%x\r\n",
                (tables_base_addr >> 32) as u32,
                (tables_base_addr) as u32,
                (tables_end_addr >> 32) as u32,
                (tables_end_addr) as u32
            );
        }
        let mut allocator =
            SimpleArenaAllocator::new(tables_base_addr as usize, tables_end_addr as usize);

        PML4 = allocator.alloc_page();

        printf!(
            b"Mapping (4KiB pages) 0x00000000 to 0x00100000\r\n",
            PML4,
            PML4
        );
        // 256 * 4KiB = 1MiB
        for i in 0..256 {
            let addr = (i * KB4) as u64;
            map_page_4kb(addr, addr, PAGE_RW, &mut allocator);
            map_page_4kb(addr + DIRECT_MAPPING_OFFSET, addr, PAGE_RW, &mut allocator);
        }

        for region in layout.iter() {
            if region.kind != MemoryRegionType::Usable || region.start < (1024 * 1024) {
                continue;
            }

            let aligned_start = align_up(region.start, MB2 as u64);
            let aligned_end = align_down(region.end, MB2 as u64);

            printf!(
                b"Mapping (2MiB pages) 0x%x to 0x%x\r\n",
                aligned_start,
                aligned_end
            );

            let mut addr = aligned_start;
            while addr < aligned_end {
                map_page_2mb(addr, addr, PAGE_RW, &mut allocator);
                map_page_2mb(addr + DIRECT_MAPPING_OFFSET, addr, PAGE_RW, &mut allocator);

                addr += MB2 as u64;
            }

            let kb4_aligned_start = align_up(region.start, KB4 as u64);
            printf!(
                b"> Sub-mapping (4KiB pages) 0x%x to 0x%x\r\n",
                kb4_aligned_start,
                aligned_start
            );
            let mut addr = kb4_aligned_start;
            while addr < aligned_start {
                map_page_4kb(addr, addr, PAGE_RW, &mut allocator);
                map_page_4kb(addr + DIRECT_MAPPING_OFFSET, addr, PAGE_RW, &mut allocator);
                addr += KB4 as u64;
            }

            let kb4_aligned_end = align_down(region.end, KB4 as u64);
            printf!(
                b"> Sub-mapping (4KiB pages) 0x%x to 0x%x\r\n",
                aligned_end,
                kb4_aligned_end
            );
            let mut addr = aligned_end;
            while addr < kb4_aligned_end {
                map_page_4kb(addr, addr, PAGE_RW, &mut allocator);
                map_page_4kb(addr + DIRECT_MAPPING_OFFSET, addr, PAGE_RW, &mut allocator);
                addr += KB4 as u64;
            }
        }

        let num_memory_regions = layout.len();

        #[allow(static_mut_refs)]
        if num_memory_regions > KERNEL_MEMORY_LAYOUT.len() {
            printf!(b"Too many memory regions in layout !\r\n");
            kpanic();
        }
        printf!(
            b"\r\nMemory layout saved at 0x%x (",
            addr_of!(KERNEL_MEMORY_LAYOUT)
        );
        write_u32_decimal(num_memory_regions as u32);
        printf!(b" entries)\r\n\n");
        for (i, reg) in layout.iter().enumerate() {
            #[allow(static_mut_refs)]
            match KERNEL_MEMORY_LAYOUT.get_mut(i) {
                None => {
                    printf!(b"Too many memory regions in layout !\r\n");
                    kpanic();
                }
                Some(region) => {
                    *region = OsMemoryRegion {
                        start: reg.start,
                        end: reg.end,
                        usable: if reg.kind == MemoryRegionType::Usable {
                            1
                        } else {
                            0
                        },
                    }
                }
            }
        }

        let (_, stack_end) = load_kernel(kernel_file, &mut allocator).unwrap_or_else(|e| e.panic());

        printf!(
            b"\r\nPaging tables built at 0x%x%x\r\n",
            (PML4 as u64 >> 32) as u32,
            PML4 as u32
        );

        init_gdtr();
        printf!(b"\r\nJumping to kernel.\r\n\n\n");
        enable_paging_and_jump64(
            PML4 as usize,
            DATA64_SELECTOR,
            CODE64_SELECTOR,
            entry64,
            stack_end,
            addr_of!(KERNEL_MEMORY_LAYOUT) as usize,
            num_memory_regions,
            allocator.current,
            allocator.end,
            mem::get_last_header() as usize,
        );
    }
}
