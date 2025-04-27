use core::ptr::addr_of;

use crate::{eflags, kpanic, mem::Buffer, ptr_to_seg_off, seg_off_to_ptr, video::Video};

#[repr(C, packed)]
pub struct BiosInterruptResult {
    pub eax: usize,
    pub ebx: usize,
    pub ecx: usize,
    pub edx: usize,
    pub esi: usize,
    pub edi: usize,
    pub eflags: usize,
}

impl BiosInterruptResult {
    pub fn print(&self) {
        unsafe {
            let video = Video::get();
            video.write_string(b"BiosInterruptResult {\n");
            video.write_string(b"  eax: 0x");
            video.write_hex_u32(self.eax as u32);
            video.write_char(b'\n');
            video.write_string(b"  ebx: 0x");
            video.write_hex_u32(self.ebx as u32);
            video.write_char(b'\n');
            video.write_string(b"  ecx: 0x");
            video.write_hex_u32(self.ecx as u32);
            video.write_char(b'\n');
            video.write_string(b"  edx: 0x");
            video.write_hex_u32(self.edx as u32);
            video.write_char(b'\n');
            video.write_string(b"  esi: 0x");
            video.write_hex_u32(self.esi as u32);
            video.write_char(b'\n');
            video.write_string(b"  edi: 0x");
            video.write_hex_u32(self.edi as u32);
            video.write_char(b'\n');
            video.write_string(b"  eflags: 0x");
            video.write_hex_u32(self.eflags as u32);
            video.write_char(b'\n');
            video.write_string(b"}\n");
        }
    }
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct DiskParamsRaw {
    size: u16,
    info: u16,
    cylinders: u32,
    heads: u32,
    sectors_per_track: u32,
    sectors_lo: u32,
    sectors_hi: u32,
    bytes_per_sector: u16,
    ptr: u32,
}

#[repr(C, packed)]
pub struct DiskAccessPacket {
    pub size: u8,
    pub null: u8,
    pub sector_count: u16,
    pub offset: u16,
    pub segment: u16,
    pub lba: u64,
}

unsafe extern "cdecl" {
    pub unsafe fn unsafe_call_bios_interrupt(
        bios_idt: usize,
        interrupt: usize,
        eax: usize,
        ebx: usize,
        ecx: usize,
        edx: usize,
        esi: usize,
        edi: usize,
        ds: usize,
        es: usize,
        fs: usize,
        gs: usize,
    ) -> usize;
}

static mut DAP: DiskAccessPacket = DiskAccessPacket {
    size: 0x10,
    null: 0,
    sector_count: 0,
    offset: 0,
    segment: 0,
    lba: 0,
};
static mut PARAMS: DiskParamsRaw = DiskParamsRaw {
    size: 0x1E,
    info: 0,
    cylinders: 0,
    heads: 0,
    sectors_per_track: 0,
    sectors_hi: 0,
    sectors_lo: 0,
    bytes_per_sector: 0,
    ptr: 0,
};
static mut BUFF: [u8; 4096] = [0; 4096];

#[derive(Clone, Copy)]
pub struct DiskParams {
    pub info: u16,
    pub cylinders: u32,
    pub heads: u32,
    pub sectors_per_track: u32,
    pub sectors: u64,
    pub bytes_per_sector: u16,
}

pub enum DiskError {
    OutputBufferTooSmall,
    InvalidDiskParameters,
    FailedMemAlloc(usize),
    ReadError(usize),
    ReadParametersError(usize),
}

impl DiskError {
    pub fn panic(&self) -> ! {
        unsafe {
            let video = Video::get();
            video.write_string(b"Disk error: ");
            match self {
                DiskError::ReadError(c) => {
                    video.write_string(b"read error 0x");
                    video.write_hex_u32(*c as u32);
                }
                DiskError::ReadParametersError(c) => {
                    video.write_string(b"read parameters error 0x");
                    video.write_hex_u32(*c as u32);
                }
                DiskError::OutputBufferTooSmall => {
                    video.write_string(b"output buffer too small");
                }
                DiskError::InvalidDiskParameters => {
                    video.write_string(b"invalid disk parameters");
                }
                DiskError::FailedMemAlloc(size) => {
                    video.write_string(b"failed to allocate memory: 0x");
                    video.write_hex_u32(*size as u32);
                }
            }
            video.write_char(b'\n');
        }
        kpanic();
    }
}

#[derive(Clone)]
pub struct ExtendedDisk {
    disk: u8,
    bios_idt: usize,
    params: Option<DiskParams>,
}

impl ExtendedDisk {
    pub fn new(disk: u8, bios_idt: usize) -> Self {
        Self {
            disk,
            bios_idt,
            params: None,
        }
    }

    pub fn check_present(&self) -> bool {
        unsafe {
            let result = unsafe_call_bios_interrupt(
                self.bios_idt,
                0x13,
                0x4100,
                0x55AA,
                0,
                self.disk as usize,
                0,
                0,
                0,
                0,
                0,
                0,
            ) as *const BiosInterruptResult;

            ((*result).eflags & eflags::CF) == 0
                && ((*result).ebx & 0xFFFF) == 0xAA55
                && ((*result).ecx & 0b101) == 0b101
        }
    }

    pub fn get_params(&mut self) -> Result<DiskParams, DiskError> {
        if let Some(params) = self.params {
            return Ok(params);
        }
        unsafe {
            let (seg, off) = ptr_to_seg_off(addr_of!(PARAMS) as usize);

            let result = unsafe_call_bios_interrupt(
                self.bios_idt,
                0x13,
                0x4800,
                0,
                0,
                self.disk as usize,
                off as usize,
                0,
                seg as usize,
                seg as usize,
                0,
                0,
            ) as *const BiosInterruptResult;

            if ((*result).eflags & eflags::CF) != 0 {
                Err(DiskError::ReadParametersError((*result).eax as usize))
            } else {
                let params = DiskParams {
                    info: PARAMS.info,
                    cylinders: PARAMS.cylinders,
                    heads: PARAMS.heads,
                    sectors_per_track: PARAMS.sectors_per_track,
                    sectors: ((PARAMS.sectors_hi as u64) << 32) | (PARAMS.sectors_lo as u64),
                    bytes_per_sector: PARAMS.bytes_per_sector,
                };
                self.params = Some(params);
                Ok(params)
            }
        }
    }

    pub fn read_sector(&mut self, lba: u64, buffer: &mut Buffer) -> Result<(), DiskError> {
        let bps = self.get_params()?.bytes_per_sector as usize;
        if buffer.len() < bps {
            return Err(DiskError::OutputBufferTooSmall);
        }

        let (segment, offset) = ptr_to_seg_off(addr_of!(BUFF) as usize);

        unsafe {
            let (dap_seg, dap_off) = ptr_to_seg_off(addr_of!(DAP) as usize);
            DAP = DiskAccessPacket {
                size: 0x10,
                null: 0,
                sector_count: 1,
                offset,
                segment,
                lba,
            };

            let result = unsafe_call_bios_interrupt(
                self.bios_idt,
                0x13,
                0x4200,
                0,
                0,
                self.disk as usize,
                dap_off as usize,
                0,
                dap_seg as usize,
                dap_seg as usize,
                0,
                0,
            ) as *const BiosInterruptResult;

            if ((*result).eflags & eflags::CF) != 0 {
                return Err(DiskError::ReadError(((*result).eax & 0xFFFF) >> 8));
            }

            let output_buf = seg_off_to_ptr(segment, offset) as *const u8;
            for (i, item) in buffer.iter_mut().enumerate().take(bps) {
                *item = *output_buf.add(i);
            }
        }
        Ok(())
    }

    /// # Safety
    /// Passed buffer must be at least `bytes_per_sector` long
    pub unsafe fn unsafe_read_sector_to_buffer(
        &mut self,
        lba: u64,
        buffer: *mut u8,
    ) -> Result<(), DiskError> {
        let bps = self.get_params()?.bytes_per_sector as usize;
        let (segment, offset) = ptr_to_seg_off(addr_of!(BUFF) as usize);
        unsafe {
            let (dap_seg, dap_off) = ptr_to_seg_off(addr_of!(DAP) as usize);
            DAP = DiskAccessPacket {
                size: 0x10,
                null: 0,
                sector_count: 1,
                offset,
                segment,
                lba,
            };

            let result = unsafe_call_bios_interrupt(
                self.bios_idt,
                0x13,
                0x4200,
                0,
                0,
                self.disk as usize,
                dap_off as usize,
                0,
                dap_seg as usize,
                dap_seg as usize,
                0,
                0,
            ) as *const BiosInterruptResult;

            if ((*result).eflags & eflags::CF) != 0 {
                return Err(DiskError::ReadError(((*result).eax & 0xFFFF) >> 8));
            }

            let output_buf = seg_off_to_ptr(segment, offset) as *const u8;
            for i in 0..bps {
                *buffer.add(i) = *output_buf.add(i);
            }
        }
        Ok(())
    }

    pub fn read_to_buffer(&mut self, lba: u64, buffer: &mut Buffer) -> Result<(), DiskError> {
        let bps = self.get_params()?.bytes_per_sector as usize;
        if bps == 0 {
            return Err(DiskError::InvalidDiskParameters);
        }
        let sector_count = buffer.len() / bps;
        let mut sector_buffer = Buffer::new(bps).ok_or(DiskError::FailedMemAlloc(bps))?;
        for i in 0..sector_count {
            let begin = i * bps;
            let end = (i + 1) * bps;
            if begin >= buffer.len() || end >= buffer.len() || end <= begin {
                break;
            }
            self.read_sector(lba + i as u64, &mut sector_buffer)?;
            sector_buffer.copy_to(0, buffer, begin, bps);
        }
        Ok(())
    }
}
