use core::ptr;

use crate::{
    kpanic,
    mem::{self, Box, Buffer, SystemMemoryMap, Vec, RANGE_TYPE_AVAILABLE, SYSTEM_MEMORY_MAP},
    printf,
};

pub const PAGE_SIZE_2MB: u64 = 2 * 1024 * 1024;
pub const PAGE_PRESENT: u64 = 1;
pub const PAGE_WRITE: u64 = 1 << 1;
pub const PAGE_LARGE: u64 = 1 << 7;
pub const PAGE_USER: u64 = 1 << 2;

#[repr(align(4096))]
#[derive(Copy, Clone)]
pub struct PageTable([u64; 512]);

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

static mut PML4: Box<PageTable> = unsafe { Box::null_const() };
static mut PDPT: Box<PageTable> = unsafe { Box::null_const() };
static mut PD: Box<PageTable> = unsafe { Box::null_const() };

unsafe fn init() {
    PML4 = Buffer::new(size_of::<PageTable>())
        .unwrap_or_else(|| kpanic())
        .boxed();

    PDPT = Buffer::new(size_of::<PageTable>())
        .unwrap_or_else(|| kpanic())
        .boxed();

    PD = Buffer::new(size_of::<PageTable>())
        .unwrap_or_else(|| kpanic())
        .boxed();
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

    loop {
        let (new_layout, had_overlap) = overlapping_pass(layout);
        if !had_overlap {
            break new_layout;
        }
        layout = new_layout;
    }
}

pub fn enable_paging() {
    unsafe {
        let layout = parse_memory_layout();

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

        init();
    }
}
