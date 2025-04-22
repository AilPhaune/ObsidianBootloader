use crate::{
    fs::{Ext2Error, Ext2File},
    kpanic,
    mem::{Buffer, Vec},
    video::Video,
};

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct ElfHeader32 {
    pub magic: [u8; 4],
    pub bits: u8,
    pub endianness: u8,
    pub header_version: u8,
    pub os_abi: u8,
    pub padding: [u8; 8],
    pub elf_type: u16,
    pub instruction_set: u16,
    pub elf_version: u32,
    pub entry_offset: u32,
    pub program_header_table_offset: u32,
    pub section_header_table_offset: u32,
    pub flags: u32,
    pub header_size: u16,
    pub program_header_entry_size: u16,
    pub program_header_entry_count: u16,
    pub section_header_entry_size: u16,
    pub section_header_entry_count: u16,
    pub index_of_section_header_string_table: u16,
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct ElfHeader64 {
    pub magic: [u8; 4],
    pub bits: u8,
    pub endianness: u8,
    pub header_version: u8,
    pub os_abi: u8,
    pub padding: [u8; 8],
    pub elf_type: u16,
    pub instruction_set: u16,
    pub elf_version: u32,
    pub entry_offset: u64,
    pub program_header_table_offset: u64,
    pub section_header_table_offset: u64,
    pub flags: u32,
    pub header_size: u16,
    pub program_header_entry_size: u16,
    pub program_header_entry_count: u16,
    pub section_header_entry_size: u16,
    pub section_header_entry_count: u16,
    pub index_of_section_header_string_table: u16,
}

union ElfHeader {
    elf32: ElfHeader32,
    elf64: ElfHeader64,
}

pub const INSTRUCTION_SET_X86: u8 = 0x03;
pub const INSTRUCTION_SET_X86_64: u8 = 0x3E;

pub const ENDIANNESS_LITTLE: u8 = 1;
pub const ENDIANNESS_BIG: u8 = 2;

pub const ELF_TYPE_RELOCATABLE: u8 = 1;
pub const ELF_TYPE_EXECUTABLE: u8 = 2;
pub const ELF_TYPE_SHARED_OBJECT: u8 = 3;
pub const ELF_TYPE_CORE: u8 = 4;

pub enum ElfHeaderFlavour {
    Elf32(ElfHeader32),
    Elf64(ElfHeader64),
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct ElfProgramHeader32 {
    pub segment_type: u32,
    pub p_offset: u32,
    pub p_vaddr: u32,
    pub p_paddr: u32,
    pub p_filesz: u32,
    pub p_memsz: u32,
    pub flags: u32,
    pub align: u32,
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct ElfProgramHeader64 {
    pub segment_type: u32,
    pub flags: u32,
    pub p_offset: u64,
    pub p_vaddr: u64,
    pub p_paddr: u64,
    pub p_filesz: u64,
    pub p_memsz: u64,
    pub align: u64,
}

pub const SEGMENT_TYPE_NULL: u32 = 0;
pub const SEGMENT_TYPE_LOAD: u32 = 1;
pub const SEGMENT_TYPE_DYNAMIC: u32 = 2;
pub const SEGMENT_TYPE_INTERP: u32 = 3;
pub const SEGMENT_TYPE_NOTE: u32 = 4;

pub const FLAG_EXECUTABLE: u32 = 1;
pub const FLAG_WRITABLE: u32 = 2;
pub const FLAG_READABLE: u32 = 4;

pub enum ElfError {
    UnsupportedEndianness,
    Ext2Error(Ext2Error),
    FailedMemAlloc,
    InvalidMagic,
}

impl ElfError {
    pub fn panic(&self) -> ! {
        unsafe {
            let video = Video::get();
            match self {
                ElfError::UnsupportedEndianness => {
                    video.write_string(b"Unsupported endianness\n");
                }
                ElfError::FailedMemAlloc => {
                    video.write_string(b"Failed to allocate memory\n");
                }
                ElfError::InvalidMagic => {
                    video.write_string(b"Invalid ELF magic\n");
                }
                ElfError::Ext2Error(e) => e.panic(),
            }
            kpanic()
        }
    }
}

fn parse_elf_header(file: &mut Ext2File) -> Result<ElfHeaderFlavour, ElfError> {
    let mut elf_header = Buffer::new(size_of::<ElfHeader>()).ok_or(ElfError::FailedMemAlloc)?;
    file.seek(0).map_err(ElfError::Ext2Error)?;
    file.read(&mut elf_header, size_of::<ElfHeader>())
        .map_err(ElfError::Ext2Error)?;

    let elf_header: ElfHeader = elf_header.boxed::<ElfHeader>().unbox();
    unsafe {
        if &elf_header.elf32.magic != b"\x7fELF" {
            return Err(ElfError::InvalidMagic);
        }
        if elf_header.elf32.bits == 0x01 {
            let elf_header = elf_header.elf32;
            if elf_header.endianness != ENDIANNESS_LITTLE {
                return Err(ElfError::UnsupportedEndianness);
            }
            Ok(ElfHeaderFlavour::Elf32(elf_header))
        } else {
            let elf_header = elf_header.elf64;
            if elf_header.endianness != ENDIANNESS_LITTLE {
                return Err(ElfError::UnsupportedEndianness);
            }
            Ok(ElfHeaderFlavour::Elf64(elf_header))
        }
    }
}

pub struct ElfFile32<'a> {
    file: Ext2File<'a>,
    header: ElfHeader32,
    ph: Vec<ElfProgramHeader32>,
}

macro_rules! impl_load_ph {
    ($elfph: ident, $utype: ident) => {
        fn load_ph(&mut self, i: $utype) -> Result<(), ElfError> {
            let offset = self.header.program_header_table_offset
                + (i * self.header.program_header_entry_size as $utype);

            self.file
                .seek(offset as usize)
                .map_err(ElfError::Ext2Error)?;

            let mut buf =
                Buffer::new(core::mem::size_of::<$elfph>()).ok_or(ElfError::FailedMemAlloc)?;

            self.file
                .read(&mut buf, core::mem::size_of::<$elfph>())
                .map_err(ElfError::Ext2Error)?;

            let ph: $elfph = buf.boxed::<$elfph>().unbox();

            self.ph.push(ph);

            Ok(())
        }

        pub fn load_program_headers(&mut self) -> Result<&Vec<$elfph>, ElfError> {
            if !self.ph.is_empty() {
                return Ok(&self.ph);
            }
            self.ph
                .ensure_capacity(self.header.program_header_entry_count as usize);

            for i in 0..self.header.program_header_entry_count {
                self.load_ph(i as $utype)?;
            }

            Ok(&self.ph)
        }
    };
}

impl<'a> ElfFile32<'a> {
    pub fn new(file: Ext2File<'a>, elf_header: ElfHeader32) -> Result<ElfFile32<'a>, ElfError> {
        Ok(ElfFile32 {
            file,
            header: elf_header,
            ph: Vec::default(),
        })
    }

    impl_load_ph!(ElfProgramHeader32, u32);

    pub fn entry_point(&self) -> u32 {
        self.header.entry_offset
    }

    pub fn get_file(&self) -> &Ext2File {
        &self.file
    }

    pub fn get_file_mut(&mut self) -> &'a mut Ext2File {
        &mut self.file
    }
}

pub struct ElfFile64<'a> {
    file: Ext2File<'a>,
    header: ElfHeader64,
    ph: Vec<ElfProgramHeader64>,
}

impl<'a> ElfFile64<'a> {
    pub fn new(file: Ext2File<'a>, elf_header: ElfHeader64) -> Result<ElfFile64<'a>, ElfError> {
        Ok(ElfFile64 {
            file,
            header: elf_header,
            ph: Vec::default(),
        })
    }

    impl_load_ph!(ElfProgramHeader64, u64);

    pub fn entry_point(&self) -> u64 {
        self.header.entry_offset
    }

    pub fn get_file(&self) -> &Ext2File {
        &self.file
    }

    pub fn get_file_mut(&mut self) -> &'a mut Ext2File {
        &mut self.file
    }
}

pub enum ElfFileFlavour<'f> {
    Elf32(ElfFile32<'f>),
    Elf64(ElfFile64<'f>),
}

pub fn load_elf<'f>(mut file: Ext2File<'f>) -> Result<ElfFileFlavour<'f>, ElfError> {
    let elf_header = parse_elf_header(&mut file)?;
    match elf_header {
        ElfHeaderFlavour::Elf32(elf_header) => {
            let elf_file = ElfFile32::new(file, elf_header)?;
            Ok(ElfFileFlavour::Elf32(elf_file))
        }
        ElfHeaderFlavour::Elf64(elf_header) => {
            let elf_file = ElfFile64::new(file, elf_header)?;
            Ok(ElfFileFlavour::Elf64(elf_file))
        }
    }
}
