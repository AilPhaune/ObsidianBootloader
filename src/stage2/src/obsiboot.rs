use crate::{e9::write_string, kpanic, printf};

/// # ObsiBoot Kernel Parameters
/// Contains information about the bootloader and the system
/// Documentation for ObsiBoot struct version 1.
#[repr(C, packed)]
pub struct ObsiBootKernelParameters {
    /// The size of this structure in bytes <br>
    pub obsiboot_struct_size: u32,
    /// The version of this structure <br>
    pub obsiboot_struct_version: u32,
    /// A checksum of this structure <br>
    pub obsiboot_struct_checksum: [u32; 8],

    /*
     *
     *                  BEGIN OBSIBOOT VERSION-DEPENDENT FIELDS
     *
     * */
    /// A pointer to a null terminated string containing the name of the bootloader <br>
    /// Note: This is a physical address <br>
    /// Note: Bootloaders may set this value either to a null pointer or to a pointer to a valid null terminated ASCII only string <br>
    pub bootloader_name_ptr: u32,

    /// The bootloader version, as [major, minor, patch, build] <br>
    pub bootloader_version: [u8; 4],

    /// The BIOS drive number of the boot drive <br>
    pub bios_boot_drive: u32,
    /// The BIOS Interrupt Descriptor Table pointer <br>
    pub bios_idt_ptr: u32,

    /// A pointer to a sanitized memory layout given by the BIOS <br>
    /// Note: This is a physical address <br>
    /// Note: Any region that is marked as usable is fully usable by the kernel except for the one containing the address `usbale_kernel_memory_start`. See `usbale_kernel_memory_start` for more information. <br>
    pub ptr_to_memory_layout: u32,
    /// The number of entries in the memory layout <br>
    pub memory_layout_entry_count: u32,
    /// The size of each memory layout entry in bytes (see `paging::OsMemoryRegion`) <br>
    pub memory_layout_entry_size: u32,

    /// The current address of the arena allocator for page tables <br>
    /// Note: This is a physical address <br>
    /// Note: Bootloaders may not set this value if they either: <br>
    /// 1. Do not setup paging in the event of loading a 32-bit kernel (paging is mandatory for 64-bit kernels)
    /// 2. Do not use an arena allocator for allocating page tables
    /// 3. Decide to not set the value at all
    pub page_tables_page_allocator_current_free_page: u32,
    /// The address of the last page of the arena allocator for page tables <br>
    /// Note: This is a physical address <br>
    /// Note: Bootloaders may not set this value. See `page_tables_page_allocator_current_free_page` for more information. <br>
    pub page_tables_page_allocator_last_usable_page: u32,
    /// The base address of PML4 <br>
    pub pml4_base_address: u32,

    /// The address of the first kernel usable memory. <br>
    /// Note: This is a physical address that may not be aligned to anything <br>
    /// Note: The bootloader guarantees that the kernel can use any memory between `usable_kernel_memory_start` and the end of the memory region containing it <br>
    pub usable_kernel_memory_start: u32,

    /// The address of the VBE info block gathered from the BIOS <br>
    /// Note: This is a physical address <br>
    pub vbe_info_block_ptr: u32,
    /// A pointer to a list of [`VesaModeInfoStructure`]s gathered from the BIOS <br>
    /// Note: This is a physical address <br>
    pub vbe_modes_info_ptr: u32,
    /// The number of entries in the [`VesaModeInfoStructure`]s list <br>
    /// Note: Each entry is 256 bytes <br>
    pub vbe_mode_info_block_entry_count: u32,
    /// The selected VESA mode <br>
    pub vbe_selected_mode: u32,

    /// The initial stack pointer used to load the kernel
    pub kernel_stack_pointer: u64,
}

impl ObsiBootKernelParameters {
    /// Computes the checksum, without modifying the structure. Does not set the checksum field.
    /// ### Uses a custom checksum algorithm:
    /// 1. Start with 8 unsigned 32-bit zeros
    /// 2. For each byte in the structure, update the checksum using a custom update function.
    /// ### Update function:
    /// 1. Compute the xor of all 8 u32 elements of the checksum array
    /// 2. Shift the checksum array: \[1..=7] -> \[0..=6]
    /// 3. result[7] = previously computed xor (step 1.)
    /// 4. result[7] += unsigned multiplication of the byte by 0x01100111 (no specific reason for that number except from spreading the byte to 32-bits)
    pub fn calculate_checksum(&mut self) -> [u32; 8] {
        let prev = self.obsiboot_struct_checksum;
        self.obsiboot_struct_checksum = [0u32; 8];

        let mut result = [0u32; 8];
        fn update(result: &mut [u32; 8], byte: u8) {
            let result0 = result[0];
            let mut xored = result0;
            for i in 0..7 {
                result[i] = result[i + 1];
                xored ^= result[i];
            }
            result[7] = xored.wrapping_add((byte as u32).wrapping_mul(0x01100111));
        }
        unsafe {
            let selfptr = self as *const Self as *const u8;
            for i in 0..self.obsiboot_struct_size {
                update(&mut result, *selfptr.add(i as usize))
            }
        }

        self.obsiboot_struct_checksum = prev;
        result
    }

    pub fn verify_checksum(&mut self) -> bool {
        let checksum = self.calculate_checksum();
        let expected = self.obsiboot_struct_checksum;
        checksum == expected
    }

    pub const fn empty() -> Self {
        Self {
            obsiboot_struct_size: 0,
            obsiboot_struct_version: 0,
            obsiboot_struct_checksum: [0; 8],
            bootloader_name_ptr: 0,
            bootloader_version: [0; 4],
            bios_boot_drive: 0,
            bios_idt_ptr: 0,
            ptr_to_memory_layout: 0,
            memory_layout_entry_count: 0,
            memory_layout_entry_size: 0,
            page_tables_page_allocator_current_free_page: 0,
            page_tables_page_allocator_last_usable_page: 0,
            pml4_base_address: 0,
            usable_kernel_memory_start: 0,
            vbe_info_block_ptr: 0,
            vbe_modes_info_ptr: 0,
            vbe_mode_info_block_entry_count: 0,
            vbe_selected_mode: 0,
            kernel_stack_pointer: 0,
        }
    }
}

pub enum ObsiBootConfigVbeMode {
    ModeNumber(u16),
    ModeInfo { width: u16, height: u16, bpp: u8 },
}

pub struct ObsiBootConfig {
    pub vbe_mode: Option<ObsiBootConfigVbeMode>,
}

impl ObsiBootConfig {
    pub const fn empty() -> Self {
        Self { vbe_mode: None }
    }

    pub fn parse(data: &[u8]) -> Self {
        let mut config = Self::empty();
        let mut i = 0;
        fn eol(data: &[u8], i: usize) -> usize {
            let Some(slice) = data.get(i..) else {
                return data.len();
            };
            for (j, d) in slice.iter().enumerate() {
                if *d == b'\n' {
                    return i + j;
                }
            }
            data.len()
        }
        fn is_key(data: &[u8], i: usize, key: &[u8]) -> bool {
            if let Some(slice) = data.get(i..(i + key.len())) {
                for (j, d) in slice.iter().enumerate() {
                    if Some(d) != key.get(j) {
                        return false;
                    }
                }
                true
            } else {
                false
            }
        }
        loop {
            if i >= data.len() {
                break;
            }
            if data.get(i) == Some(&b'#') {
                while let Some(c) = data.get(i) {
                    if *c == b'\n' {
                        break;
                    }
                    i += 1;
                }
                continue;
            }
            if data.get(i) == Some(&b'\n') {
                i += 1;
                continue;
            }

            if is_key(data, i, b"vbe_mode=") {
                i += 9;
                let j = eol(data, i);
                let Some(value) = data.get(i..j) else {
                    i = j;
                    continue;
                };
                i = j;
                let Ok(mode_num) = u16::from_ascii(value) else {
                    // Parse as `width`x`height`:`bpp`
                    let Some(idx_x) = value.iter().enumerate().find(|(_, c)| **c == b'x') else {
                        continue;
                    };
                    let Some(idx_c) = value.iter().enumerate().find(|(_, c)| **c == b':') else {
                        continue;
                    };

                    let width_slice = value.get(0..idx_x.0).unwrap_or(b"0");
                    let width = u16::from_ascii(width_slice).unwrap_or(0);

                    let height_slice = value.get(idx_x.0 + 1..idx_c.0).unwrap_or(b"0");
                    let height = u16::from_ascii(height_slice).unwrap_or(0);

                    let bpp_slice = value.get(idx_c.0 + 1..).unwrap_or(b"0");
                    let bpp = u8::from_ascii(bpp_slice).unwrap_or(0);

                    config.vbe_mode = Some(ObsiBootConfigVbeMode::ModeInfo { width, height, bpp });
                    continue;
                };
                config.vbe_mode = Some(ObsiBootConfigVbeMode::ModeNumber(mode_num));
                continue;
            }

            printf!(b"Unknown config line: ");
            write_string(data.get(i..).unwrap_or(b"Error"));
            printf!(b"\r\n");
            kpanic();
        }
        config
    }
}
