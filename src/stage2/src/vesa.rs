use core::ptr::addr_of;

use crate::{
    bios::{unsafe_call_bios_interrupt, BiosInterruptResult},
    e9::write_char,
    kpanic,
    mem::{memset, Buffer},
    obsiboot::{ObsiBootConfig, ObsiBootConfigVbeMode},
    printf, ptr_to_seg_off, seg_off_to_ptr,
    video::Video,
};

#[repr(C, packed)]
pub struct VbeInfoBlock {
    pub signature: [u8; 4],
    pub version: u16,
    pub oem_string_ptr: [u16; 2],
    pub capabilities: [u8; 4],
    pub video_mode_ptr: [u16; 2],
    pub total_memory: u16,
    pub reserved: [u8; 492],
}

#[repr(C, packed)]
#[derive(Clone)]
pub struct VesaModeInfoStructure {
    pub attributes: u16,
    pub window_a: u8,
    pub window_b: u8,
    pub granularity: u16,
    pub window_size: u16,
    pub segment_a: u16,
    pub segment_b: u16,
    pub win_func_ptr: u32,
    pub pitch: u16,
    pub width: u16,
    pub height: u16,
    pub w_char: u8,
    pub y_char: u8,
    pub planes: u8,
    pub bpp: u8,
    pub banks: u8,
    pub memory_model: u8,
    pub bank_size: u8,
    pub image_pages: u8,
    pub reserved0: u8,
    pub red_mask: u8,
    pub red_position: u8,
    pub green_mask: u8,
    pub green_position: u8,
    pub blue_mask: u8,
    pub blue_position: u8,
    pub reserved_mask: u8,
    pub reserved_position: u8,
    pub direct_color_attributes: u8,
    pub framebuffer: u32,
    pub offscreen_mem_off: u32,
    pub offscreen_mem_size: u16,
    pub reserved1: [u8; 206],
}

#[repr(align(512))]
struct VesaContainer([u8; 512]);
#[repr(align(256))]
struct VesaContainerSmall([u8; 256]);

struct BestMode {
    mode: u16,
    width: usize,
    height: usize,
    bpp: u8,
    framebuffer: u32,
}

static mut VESA_INFO: VesaContainer = VesaContainer([0; 512]);
static mut VESA_MODE_INFO: VesaContainerSmall = VesaContainerSmall([0; 256]);

static mut MODES_BUFFER: Buffer = Buffer::null();
static mut BESTMODE: BestMode = BestMode {
    mode: 0,
    width: 0,
    height: 0,
    bpp: 0,
    framebuffer: 0,
};

const MESSAGE: &[u8] = b"Failed to switch to graphics mode !\r\n";

pub fn switch_to_graphics(bios_idt: usize, config: &ObsiBootConfig) {
    unsafe {
        let info = &*(addr_of!(VESA_INFO.0) as *const VbeInfoBlock);
        let (seg, off) = ptr_to_seg_off(addr_of!(VESA_INFO.0) as usize);

        let res = unsafe_call_bios_interrupt(
            bios_idt,
            0x10,
            0x4f00,
            0,
            0,
            0,
            0,
            off as usize,
            seg as usize,
            seg as usize,
            seg as usize,
            seg as usize,
        ) as *const BiosInterruptResult;

        if ((*res).eax & 0xFFFF) != 0x4F {
            Video::get().write_string(MESSAGE);
            printf!(b"Failed to switch to graphics mode: eax=%x\r\n", (*res).eax);
            kpanic();
        }

        if info.signature != [b'V', b'E', b'S', b'A'] {
            Video::get().write_string(MESSAGE);
            printf!(
                b"Bad VESA signature: %b%b%b%b\r\n",
                info.signature[0] as u32,
                info.signature[1] as u32,
                info.signature[2] as u32,
                info.signature[3] as u32
            );
            kpanic();
        }

        // OEM string
        printf!(
            b"Found VESA info block. OEM ptr=%x:%x, value=",
            info.oem_string_ptr[1],
            info.oem_string_ptr[0]
        );
        let mut ptr = seg_off_to_ptr(info.oem_string_ptr[1], info.oem_string_ptr[0]) as *const u8;
        while *ptr != 0 {
            write_char(*ptr);
            ptr = ptr.add(1);
        }
        printf!(b"\r\n");

        // Video modes
        let mut ptr = seg_off_to_ptr(info.video_mode_ptr[1], info.video_mode_ptr[0]) as *const u16;

        let mut bestmode: BestMode = BestMode {
            mode: 0,
            width: 0,
            height: 0,
            bpp: 0,
            framebuffer: 0,
        };

        let mode_info = &*(addr_of!(VESA_MODE_INFO.0) as *const VesaModeInfoStructure);
        let (seg, off) = ptr_to_seg_off(addr_of!(VESA_MODE_INFO.0) as usize);
        printf!(b"Mode info ptr=%x:%x\r\n", seg, off);

        let mode_count = {
            let mut i = 0;
            while *ptr.add(i) != 0xFFFF {
                i += 1;
            }
            i
        };
        MODES_BUFFER = Buffer::new(mode_count * 256).unwrap_or_else(|| {
            printf!(
                b"Failed to allocate 0x%x bytes of memory for VESA modes buffer\r\n",
                mode_count * 256
            );
            Video::get().write_string(MESSAGE);
            kpanic();
        });

        let mut i = 0;
        loop {
            let mode = *ptr;
            if mode == 0xFFFF {
                break;
            }

            let res = unsafe_call_bios_interrupt(
                bios_idt,
                0x10,
                0x4f01,
                0,
                mode as usize,
                0,
                0,
                off as usize,
                seg as usize,
                seg as usize,
                seg as usize,
                seg as usize,
            ) as *const BiosInterruptResult;
            ptr = ptr.add(1);

            #[allow(static_mut_refs)]
            let mode_ptr = MODES_BUFFER.get_ptr() as *mut VesaModeInfoStructure;
            *mode_ptr.add(i) = mode_info.clone();
            i += 1;

            match config.vbe_mode {
                Some(ObsiBootConfigVbeMode::ModeNumber(m)) => {
                    printf!(b"m=%x, mode=%x\r\n", m, mode);
                    if bestmode.mode == m {
                        // Mode already selected
                        printf!(b"ALREADY SELECTED\r\n");
                        continue;
                    }
                    if mode == m {
                        printf!(b"SELECTING\r\n");
                        bestmode.mode = mode;
                        bestmode.width = mode_info.width as usize;
                        bestmode.height = mode_info.height as usize;
                        bestmode.bpp = mode_info.bpp;
                        bestmode.framebuffer = mode_info.framebuffer;
                        continue;
                    }
                }
                Some(ObsiBootConfigVbeMode::ModeInfo { width, height, bpp }) => {
                    if bestmode.width == width as usize
                        && bestmode.height == height as usize
                        && bestmode.bpp == bpp
                    {
                        // Mode already selected
                        continue;
                    }
                    if mode_info.width == width
                        && mode_info.height == height
                        && mode_info.bpp == bpp
                    {
                        bestmode.mode = mode;
                        bestmode.width = mode_info.width as usize;
                        bestmode.height = mode_info.height as usize;
                        bestmode.bpp = mode_info.bpp;
                        bestmode.framebuffer = mode_info.framebuffer;
                        continue;
                    }
                }
                None => {}
            }

            if ((*res).eax & 0xFFFF) != 0x4F {
                // Error/unsupported mode
                continue;
            }

            if (mode_info.attributes & 0x80) != 0x80 {
                // Mode doesn't have linear framebuffer
                continue;
            }

            if mode_info.memory_model != 0x06 {
                // Mode doesn't have direct color memory model
                continue;
            }

            printf!(
                b"\r\nVESA Mode %x: width=0x%x, height=0x%x, bpp=0x%b, window_a=0x%x, window_b=0x%x, granularity=0x%x, window_size=0x%x, attributes=0x%x, segment_a=0x%x, segment_b=0x%x, win_func_ptr=0x%x, pitch=0x%x, w_char=0x%b, y_char=0x%b, planes=0x%b, bpp=0x%b, banks=0x%b, memory_model=0x%b, bank_size=0x%b, image_pages=0x%b, reserved0=0x%b, red_mask=0x%b, red_position=0x%b, green_mask=0x%b, green_position=0x%b, blue_mask=0x%b, blue_position=0x%b, reserved_mask=0x%b, reserved_position=0x%b, direct_color_attributes=0x%b\r\n",
                mode as u32,
                mode_info.width as u32,
                mode_info.height as u32,
                mode_info.bpp as u32,
                mode_info.window_a as u32,
                mode_info.window_b as u32,
                mode_info.granularity as u32,
                mode_info.window_size as u32,
                mode_info.attributes as u32,
                mode_info.segment_a as u32,
                mode_info.segment_b as u32,
                mode_info.win_func_ptr,
                mode_info.pitch as u32,
                mode_info.w_char as u32,
                mode_info.y_char as u32,
                mode_info.planes as u32,
                mode_info.bpp as u32,
                mode_info.banks as u32,
                mode_info.memory_model as u32,
                mode_info.bank_size as u32,
                mode_info.image_pages as u32,
                mode_info.reserved0 as u32,
                mode_info.red_mask as u32,
                mode_info.red_position as u32,
                mode_info.green_mask as u32,
                mode_info.green_position as u32,
                mode_info.blue_mask as u32,
                mode_info.blue_position as u32,
                mode_info.reserved_mask as u32,
                mode_info.reserved_position as u32,
                mode_info.direct_color_attributes as u32
            );

            let pixelcount = (mode_info.width as usize) * (mode_info.height as usize);
            let best_pixels = bestmode.width * bestmode.height;

            if (pixelcount > best_pixels) && mode_info.bpp >= 24
                || (pixelcount == best_pixels && mode_info.bpp > bestmode.bpp)
            {
                bestmode.mode = mode;
                bestmode.width = mode_info.width as usize;
                bestmode.height = mode_info.height as usize;
                bestmode.bpp = mode_info.bpp;
                bestmode.framebuffer = mode_info.framebuffer;
            }
        }

        printf!(
            b"Best VBE mode: framebuffer=%x, mode=%x, width=%x, height=%x, bpp=%x\r\n",
            bestmode.framebuffer,
            bestmode.mode as u32,
            bestmode.width as u32,
            bestmode.height as u32,
            bestmode.bpp as u32
        );

        let res = unsafe_call_bios_interrupt(
            bios_idt,
            0x10,
            0x4f02,
            bestmode.mode as usize,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ) as *const BiosInterruptResult;

        if ((*res).eax & 0xFFFF) != 0x4F {
            Video::get().write_string(MESSAGE);
            printf!(b"Failed to set graphics mode: eax=%x\r\n", (*res).eax);
            kpanic();
        }

        memset(
            bestmode.framebuffer as usize,
            0,
            bestmode.width * bestmode.height * (bestmode.bpp as usize / 8),
        );

        BESTMODE = bestmode;
    }
}

#[allow(static_mut_refs)]
pub fn get_vbe_boot_info() -> (u32, u32, u32, u32) {
    unsafe {
        let vbe_info_block_ptr = VESA_INFO.0.as_ptr() as u32;
        let vbe_modes_info_ptr = MODES_BUFFER.get_ptr() as u32;
        let vbe_mode_count = MODES_BUFFER.len() as u32 / 256;
        let vbe_selected_mode = BESTMODE.mode as u32;

        (
            vbe_info_block_ptr,
            vbe_modes_info_ptr,
            vbe_mode_count,
            vbe_selected_mode,
        )
    }
}
