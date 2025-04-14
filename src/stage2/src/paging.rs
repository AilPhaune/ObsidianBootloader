use crate::{
    gdt::{init_gdtr, CODE64_SELECTOR, DATA64_SELECTOR},
    kpanic,
    mem::{Vec, RANGE_TYPE_AVAILABLE, SYSTEM_MEMORY_MAP, USED_MAP},
    printf,
};

extern "cdecl" {
    fn enable_paging_and_jump64(
        pml4: usize,
        data_selector: usize,
        code_selector: usize,
        entry64: usize,
    );
}

#[derive(Copy, Clone)]
pub struct MemoryRegion {
    start: u64,
    end: u64,
    kind: MemoryRegionType,
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
    start: usize,
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
            start,
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

const PAGE_SIZE: usize = 4096;
const PAGE_SIZE_2MB: usize = 2 * 1024 * 1024;

// Page Table Entry Flags
const PAGE_PRESENT: u64 = 1 << 0;
const PAGE_RW: u64 = 1 << 1;
const PAGE_USER: u64 = 1 << 2;
const PAGE_WRITE_THROUGH: u64 = 1 << 3;
const PAGE_CACHE_DISABLE: u64 = 1 << 4;
const PAGE_ACCESSED: u64 = 1 << 5;
const PAGE_DIRTY: u64 = 1 << 6;
const PAGE_HUGE: u64 = 1 << 7;
const PAGE_GLOBAL: u64 = 1 << 8;
const PAGE_NO_EXECUTE: u64 = 1 << 63;

const KB4: usize = 4 * 1024;
const MB2: usize = 2 * 1024 * 1024;

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

fn align_up(addr: u64, align: u64) -> u64 {
    (addr + align - 1) & !(align - 1)
}

unsafe fn map_page_4k(virt: u64, phys: u64, flags: u64, allocator: &mut SimpleArenaAllocator) {
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

unsafe fn translate_virtual(virt: u64) -> Option<u64> {
    let (pml4_idx, pdpt_idx, pd_idx, pt_idx) = split_virt_addr(virt);

    let pml4_entry = *PML4.add(pml4_idx);
    if pml4_entry & PAGE_PRESENT == 0 {
        return None;
    }
    let pdpt = (pml4_entry & 0x000F_FFFF_FFFF_F000) as *const u64;

    let pdpt_entry = *pdpt.add(pdpt_idx);
    if pdpt_entry & PAGE_PRESENT == 0 {
        return None;
    }
    let pd = (pdpt_entry & 0x000F_FFFF_FFFF_F000) as *const u64;

    let pd_entry = *pd.add(pd_idx);
    if pd_entry & PAGE_PRESENT == 0 {
        return None;
    }

    if pd_entry & PAGE_HUGE != 0 {
        let base = pd_entry & 0x000F_FFFF_FFE0_0000;
        let offset = virt & 0x1F_FFFF;
        return Some(base + offset);
    }

    let pt = (pd_entry & 0x000F_FFFF_FFFF_F000) as *const u64;
    let pt_entry = *pt.add(pt_idx);
    if pt_entry & PAGE_PRESENT == 0 {
        return None;
    }

    let base = pt_entry & 0x000F_FFFF_FFFF_F000;
    let offset = virt & 0xFFF;
    Some(base + offset)
}

pub fn enable_paging(entry64: usize) {
    unsafe {
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

        for region in layout.iter() {
            if region.kind != MemoryRegionType::Usable {
                continue;
            }

            if region.start < 0x100_000 {
                // Map using 4Kb pages
                let aligned_start = (region.start + 0xFFF) & !0xFFF; // align up
                let aligned_end = region.end & !0xFFF; // align down

                printf!(
                    b"Mapping (4KiB pages) 0x%x to 0x%x\r\n",
                    aligned_start,
                    aligned_end
                );

                let mut addr = aligned_start;
                while addr < aligned_end {
                    map_page_4k(addr, addr, PAGE_RW, &mut allocator);

                    addr += 4 * 1024;
                }
                continue;
            }

            let aligned_start = (region.start + 0x1F_FFFF) & !0x1F_FFFF; // align up
            let aligned_end = region.end & !0x1F_FFFF; // align down

            printf!(
                b"Mapping (2MiB pages) 0x%x to 0x%x\r\n",
                aligned_start,
                aligned_end
            );

            let mut addr = aligned_start;
            while addr < aligned_end {
                map_page_2mb(addr, addr, PAGE_RW, &mut allocator);

                addr += 2 * 1024 * 1024;
            }
        }

        printf!(
            b"Paging tables built at 0x%x%x\r\n",
            (PML4 as u64 >> 32) as u32,
            PML4 as u32
        );

        init_gdtr();
        enable_paging_and_jump64(PML4 as usize, DATA64_SELECTOR, CODE64_SELECTOR, entry64);
    }
}
