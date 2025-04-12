use core::ptr;

use crate::{
    bios::{DiskError, ExtendedDisk},
    gpt::DiskRange,
    kpanic,
    mem::{Box, Buffer, RefIterVec, Vec},
    printf,
    video::Video,
};

#[repr(C, packed)]
pub struct Ext2SuperBlock {
    pub inodes_count: u32,
    pub blocks_count: u32,
    pub su_reserved: u32,
    pub unallocated_blocks: u32,
    pub unallocated_inodes: u32,
    pub superblock_block: u32,
    pub log_block_size: u32,
    pub log_fragment_size: u32,
    pub blocks_per_group: u32,
    pub fragments_per_group: u32,
    pub inodes_per_group: u32,
    pub last_mount_time: u32,
    pub last_write_time: u32,
    pub mount_count_since_fsck: u16,
    pub max_mount_count_before_fsck: u16,
    pub signature: u16,
    pub fs_state: u16,
    pub on_error_behavior: u16,
    pub minor_version_level: u16,
    pub last_fsck_time: u32,
    pub fsck_interval: u32,
    pub os_id: u32,
    pub major_version_level: u32,
    pub user_id_reserved_blocks: u16,
    pub group_id_reserved_blocks: u16,

    // Extended Superblock
    pub first_non_reserved_inode: u32,
    pub inode_struct_size: u16,
    pub this_block_group: u16,
    pub optional_features: u32,
    pub required_features: u32,
    pub readonly_or_support_features: u32,
    pub fs_id: [u8; 16],
    pub volume_name: [u8; 16],
    pub last_mount_path: [u8; 64],
    pub compression_algorithm: u32,
    pub file_block_preallocate_count: u8,
    pub directory_block_preallocate_count: u8,
    pub unused: [u8; 2],
    pub journal_id: [u8; 16],
    pub journal_inode: u32,
    pub journal_device: u32,
    pub head_of_orphan_inode_list: u32,
}

pub const EXT2_SUPERBLOCK_SIGNATURE: u16 = 0xEF53;

pub const FS_STATE_CLEAN: u16 = 1;
pub const FS_STATE_ERROR: u16 = 2;

pub const ON_ERROR_BEHAVIOR_CONTINUE: u16 = 1;
pub const ON_ERROR_BEHAVIOR_RO: u16 = 2;
pub const ON_ERROR_BEHAVIOR_PANIC: u16 = 3;

pub const OS_ID_LINUX: u32 = 0;
pub const OS_ID_GNU_HURD: u32 = 1;
pub const OS_ID_MASIX: u32 = 2;
pub const OS_ID_FREEBSD: u32 = 3;
pub const OS_ID_LITES: u32 = 4;

pub const OPTIONAL_FEATURE_PREALLOCATE_BLOCKS: u32 = 0x1;
pub const OPTIONAL_FEATURE_AFS_SERVER_INODES: u32 = 0x2;
pub const OPTIONAL_FEATURE_FS_JOURNAL: u32 = 0x4;
pub const OPTIONAL_FEATURE_EXTENDED_INODE_ATTRIBUTES: u32 = 0x8;
pub const OPTIONAL_FEATURE_FS_RESIZE_SELF_LARGER: u32 = 0x10;
pub const OPTIONAL_FEATURE_DIRECTORIES_USE_HASH_INDEX: u32 = 0x20;

pub const REQUIRED_FEATURE_COMPRESSION: u32 = 0x1;
pub const REQUIRED_FEATURE_DIRECTORY_ENTRIES_HAVE_TYPE_FIELD: u32 = 0x2;
pub const REQUIRED_FEATURE_FS_NEEDS_TO_REPLAY_JOURNAL: u32 = 0x4;
pub const REQUIRED_FEATURE_FS_USES_JOURNAL_DEVICE: u32 = 0x8;

pub const RO_FEATURE_SPARSE_DESCRIPTOR_TABLES: u32 = 0x1;
pub const RO_FEATURE_64BIT_FILE_SIZE: u32 = 0x2;
pub const RO_FEATURE_DIRECTORY_CONTENT_IN_BINARY_TREE: u32 = 0x4;

const BLOCK_GROUP_DESCRIPTOR_SIZE: usize = 32;

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct Ext2BlockGroupDescriptor {
    pub block_usage_bitmap: u32,
    pub inode_usage_bitmap: u32,
    pub inode_table_block: u32,
    pub free_blocks_count: u16,
    pub free_inodes_count: u16,
    pub directory_count: u16,
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct Ext2Inode {
    pub type_and_permissions: u16,
    pub uid: u16,
    pub size_lo: u32,
    pub atime: u32,
    pub ctime: u32,
    pub mtime: u32,
    pub dtime: u32,
    pub gid: u16,
    pub links_count: u16,
    pub sectors_count: u32,
    pub flags: u32,
    pub ossv1: u32,
    pub direct_block_pointers: [u32; 12],
    pub single_indirect_block_pointer: u32,
    pub double_indirect_block_pointer: u32,
    pub triple_indirect_block_pointer: u32,
    pub generation_number: u32,
    pub extended_attribute_block: u32,
    pub size_hi_or_dir_acl: u32,
    pub fragment_block: u32,
    pub ossv2: [u8; 12],
}

pub const INODE_TYPE_FIFO: u16 = 0x1000;
pub const INODE_TYPE_CHAR_DEVICE: u16 = 0x2000;
pub const INODE_TYPE_DIRECTORY: u16 = 0x4000;
pub const INODE_TYPE_BLOCK_DEVICE: u16 = 0x6000;
pub const INODE_TYPE_REGULAR_FILE: u16 = 0x8000;
pub const INODE_TYPE_SYMLINK: u16 = 0xA000;
pub const INODE_TYPE_UNIX_SOCKET: u16 = 0xC000;

pub const INODE_PERMISSION_OTHER_EXECUTE: u16 = 0x1;
pub const INODE_PERMISSION_OTHER_WRITE: u16 = 0x2;
pub const INODE_PERMISSION_OTHER_READ: u16 = 0x4;
pub const INODE_PERMISSION_GROUP_EXECUTE: u16 = 0x8;
pub const INODE_PERMISSION_GROUP_WRITE: u16 = 0x10;
pub const INODE_PERMISSION_GROUP_READ: u16 = 0x20;
pub const INODE_PERMISSION_OWNER_EXECUTE: u16 = 0x40;
pub const INODE_PERMISSION_OWNER_WRITE: u16 = 0x80;
pub const INODE_PERMISSION_OWNER_READ: u16 = 0x100;
pub const INODE_PERMISSION_STICKYBIT: u16 = 0x200;
pub const INODE_PERMISSION_SETGID: u16 = 0x400;
pub const INODE_PERMISSION_SETUID: u16 = 0x800;

pub const INODE_FLAG_SECURE_DELETION: u32 = 0x1;
pub const INODE_FLAG_KEEP_COPY_OF_DATA_WHEN_DELETED: u32 = 0x2;
pub const INODE_FLAG_FILE_COMPRESSION: u32 = 0x4;
pub const INODE_FLAG_SYNCHRONOUS: u32 = 0x8;
pub const INODE_FLAG_IMMUTABLE: u32 = 0x10;
pub const INODE_FLAG_APPEND_ONLY: u32 = 0x20;
pub const INODE_FLAG_HIDDEN_IN_DUMP: u32 = 0x40;
pub const INODE_FLAG_NO_UPDATE_ATIME: u32 = 0x80;
pub const INODE_FLAG_HASH_INDEXED_DIRECTORY: u32 = 0x10000;
pub const INODE_FLAG_AFS_DIRECTORY: u32 = 0x20000;
pub const INODE_FLAG_JOURNAL_FILE_DATA: u32 = 0x40000;

pub enum Ext2Error {
    DiskError(DiskError),
    BadDiskSectorSize(u16),
    BadBlockSize(usize, u16),
    BadBlockGroupDescriptorTableEntrySize(usize, usize),
    BadInodeIndex(usize),
    BufferTooSmall(usize, usize),
    UnsupportedInodeType(u16),
    DirectoryParseFailed,
    NullBlockSize,
    NullPointer,
    BadSuperblock,
    FailedMemAlloc,
}

impl Ext2Error {
    pub fn panic(&self) -> ! {
        unsafe {
            let video = Video::get();
            match self {
                Ext2Error::FailedMemAlloc => {
                    video.write_string(b"Failed to allocate memory\n");
                }
                Ext2Error::BadDiskSectorSize(s) => {
                    video.write_string(b"Bad disk sector size: 0x");
                    video.write_hex_u16(*s);
                    video.write_char(b'\n');
                }
                Ext2Error::BadBlockSize(bs, ss) => {
                    video.write_string(b"Bad block size: 0x");
                    video.write_hex_u32(*bs as u32);
                    video.write_string(b" is not an integer multiple of the disk sector size 0x");
                    video.write_hex_u16(*ss);
                    video.write_char(b'\n');
                }
                Ext2Error::BadBlockGroupDescriptorTableEntrySize(a, b) => {
                    video.write_string(b"Bad block group descriptor table entry size: 0x");
                    video.write_hex_u32(*a as u32);
                    video.write_string(b" != 0x");
                    video.write_hex_u32(*b as u32);
                    video.write_char(b'\n');
                }
                Ext2Error::BufferTooSmall(a, b) => {
                    video.write_string(b"Buffer too small: 0x");
                    video.write_hex_u32(*a as u32);
                    video.write_string(b" < 0x");
                    video.write_hex_u32(*b as u32);
                    video.write_char(b'\n');
                }
                Ext2Error::NullBlockSize => {
                    video.write_string(b"Null block size\n");
                }
                Ext2Error::NullPointer => {
                    video.write_string(b"Tried following null ext2 pointer\n");
                }
                Ext2Error::BadSuperblock => {
                    video.write_string(b"Bad superblock\n");
                }
                Ext2Error::BadInodeIndex(i) => {
                    video.write_string(b"Bad inode index: 0x");
                    video.write_hex_u32(*i as u32);
                    video.write_char(b'\n');
                }
                Ext2Error::DiskError(e) => {
                    video.write_string(b"Ext2 file system error caused by:\n");
                    e.panic();
                }
                Ext2Error::UnsupportedInodeType(t) => {
                    video.write_string(b"Unsupported inode type: 0x");
                    video.write_hex_u16(*t);
                    video.write_char(b'\n');
                }
                Ext2Error::DirectoryParseFailed => {
                    video.write_string(b"Failed to parse directory\n");
                }
            }
        }
        kpanic();
    }
}

pub enum InodeReadingLocationInfo {
    Direct(usize),
    Single(usize),
    Double(usize, usize),
    Triple(usize, usize, usize),
}

pub struct InodeReadingLocation {
    location: InodeReadingLocationInfo,
    table_size: usize,
}

impl InodeReadingLocation {
    pub fn new(table_size: usize, block_idx: usize) -> Option<Self> {
        if table_size == 0 {
            return None;
        }
        let table_size2 = table_size * table_size;
        if table_size2 == 0 {
            return None;
        }

        let location = if block_idx < 12 {
            InodeReadingLocationInfo::Direct(block_idx)
        } else {
            let idx = block_idx - 12;
            if idx < table_size {
                InodeReadingLocationInfo::Single(idx)
            } else {
                let idx = idx - table_size;
                if idx < table_size2 {
                    let idx1 = idx / table_size;
                    let idx2 = idx % table_size;

                    InodeReadingLocationInfo::Double(idx1, idx2)
                } else {
                    let idx = idx - table_size2;
                    let idx1 = idx / table_size2;
                    let idx2 = (idx % table_size2) / table_size;
                    let idx3 = idx % table_size;

                    InodeReadingLocationInfo::Triple(idx1, idx2, idx3)
                }
            }
        };

        Some(Self {
            table_size,
            location,
        })
    }

    pub fn advance(&mut self) -> bool {
        match self.location {
            InodeReadingLocationInfo::Direct(direct) => {
                if direct == 11 {
                    self.location = InodeReadingLocationInfo::Single(0);
                } else {
                    self.location = InodeReadingLocationInfo::Direct(direct + 1);
                }
            }
            InodeReadingLocationInfo::Single(single) => {
                if single == self.table_size - 1 {
                    self.location = InodeReadingLocationInfo::Double(0, 0);
                } else {
                    self.location = InodeReadingLocationInfo::Single(single + 1);
                }
            }
            InodeReadingLocationInfo::Double(double1, double2) => {
                if double1 == self.table_size - 1 && double2 == self.table_size - 1 {
                    self.location = InodeReadingLocationInfo::Triple(0, 0, 0);
                } else if double2 == self.table_size - 1 {
                    self.location = InodeReadingLocationInfo::Double(double1 + 1, 0);
                } else {
                    self.location = InodeReadingLocationInfo::Double(double1, double2 + 1);
                }
            }
            InodeReadingLocationInfo::Triple(triple1, triple2, triple3) => {
                if triple1 == self.table_size - 1
                    && triple2 == self.table_size - 1
                    && triple3 == self.table_size - 1
                {
                    return false;
                } else if triple2 == self.table_size - 1 && triple3 == self.table_size - 1 {
                    self.location = InodeReadingLocationInfo::Triple(triple1 + 1, 0, 0);
                } else if triple3 == self.table_size - 1 {
                    self.location = InodeReadingLocationInfo::Triple(triple1, triple2 + 1, 0);
                } else {
                    self.location = InodeReadingLocationInfo::Triple(triple1, triple2, triple3 + 1);
                }
            }
        }
        true
    }
}

pub struct CachedInodeReadingLocation {
    location: InodeReadingLocation,
    inode: Ext2Inode,
    max_block: usize,

    table1: Buffer,
    table1_addr: usize,

    table2: Buffer,
    table2_addr: usize,

    table3: Buffer,
    table3_addr: usize,
}

impl CachedInodeReadingLocation {
    pub fn new(ext2: &Ext2FileSystem, inode: Ext2Inode) -> Result<Self, Ext2Error> {
        let size = ext2.block_size();
        if size == 0 {
            return Err(Ext2Error::NullBlockSize);
        }
        let location =
            InodeReadingLocation::new(ext2.block_size() / 4, 0).ok_or(Ext2Error::NullBlockSize)?;
        let table1 = Buffer::new(size).ok_or(Ext2Error::FailedMemAlloc)?;
        let table2 = Buffer::new(size).ok_or(Ext2Error::FailedMemAlloc)?;
        let table3 = Buffer::new(size).ok_or(Ext2Error::FailedMemAlloc)?;

        let max_block = ((inode.size_lo as usize) / size) - 1;

        Ok(Self {
            location,
            inode,
            max_block,
            table1_addr: 0,
            table2_addr: 0,
            table3_addr: 0,
            table1,
            table2,
            table3,
        })
    }

    fn check_table1(&mut self, ext2: &mut Ext2FileSystem) -> Result<(), Ext2Error> {
        let addr = match self.location.location {
            InodeReadingLocationInfo::Direct(_) => 0,
            InodeReadingLocationInfo::Single(_) => self.inode.single_indirect_block_pointer,
            InodeReadingLocationInfo::Double(_, _) => self.inode.double_indirect_block_pointer,
            InodeReadingLocationInfo::Triple(_, _, _) => self.inode.triple_indirect_block_pointer,
        } as usize;
        if addr == 0 {
            self.table1_addr = 0;
            return Ok(());
        }

        if self.table1_addr != addr {
            match ext2.read_block(addr as u64, &mut self.table1) {
                Ok(_) => {
                    self.table1_addr = addr;
                }
                Err(e) => {
                    self.table1_addr = 0;
                    return Err(e);
                }
            }
        }
        Ok(())
    }

    fn follow1(&self, idx: usize) -> Result<usize, Ext2Error> {
        if idx * 4 < self.table1.len() {
            let entry = unsafe { *(self.table1.get_ptr().add(idx * 4) as *const u32) };
            Ok(entry as usize)
        } else {
            Err(Ext2Error::NullPointer)
        }
    }

    fn check_table2(&mut self, ext2: &mut Ext2FileSystem) -> Result<(), Ext2Error> {
        let addr = match self.location.location {
            InodeReadingLocationInfo::Direct(_) => 0,
            InodeReadingLocationInfo::Single(_) => 0,
            InodeReadingLocationInfo::Double(p1, _)
            | InodeReadingLocationInfo::Triple(p1, _, _) => self.follow1(p1)?,
        };
        if addr == 0 {
            self.table2_addr = 0;
            return Ok(());
        }

        if self.table2_addr != addr {
            match ext2.read_block(addr as u64, &mut self.table2) {
                Ok(_) => {
                    self.table2_addr = addr;
                }
                Err(e) => {
                    self.table2_addr = 0;
                    return Err(e);
                }
            }
        }
        Ok(())
    }

    fn follow2(&self, idx: usize) -> Result<usize, Ext2Error> {
        if idx * 4 < self.table2.len() {
            let entry = unsafe { *(self.table2.get_ptr().add(idx * 4) as *const u32) };
            Ok(entry as usize)
        } else {
            Err(Ext2Error::NullPointer)
        }
    }

    fn check_table3(&mut self, ext2: &mut Ext2FileSystem) -> Result<(), Ext2Error> {
        let addr = match self.location.location {
            InodeReadingLocationInfo::Direct(_) => 0,
            InodeReadingLocationInfo::Single(_) => 0,
            InodeReadingLocationInfo::Double(_, p2)
            | InodeReadingLocationInfo::Triple(_, p2, _) => self.follow2(p2)?,
        };
        if addr == 0 {
            self.table3_addr = 0;
            return Ok(());
        }

        if self.table3_addr != addr {
            match ext2.read_block(addr as u64, &mut self.table3) {
                Ok(_) => {
                    self.table3_addr = addr;
                }
                Err(e) => {
                    self.table3_addr = 0;
                    return Err(e);
                }
            }
        }
        Ok(())
    }

    fn follow3(&self, idx: usize) -> Result<usize, Ext2Error> {
        if idx * 4 < self.table3.len() {
            let entry = unsafe { *(self.table3.get_ptr().add(idx * 4) as *const u32) };
            Ok(entry as usize)
        } else {
            Err(Ext2Error::NullPointer)
        }
    }

    pub fn seek(&mut self, ext2: &mut Ext2FileSystem, block: usize) -> Result<(), Ext2Error> {
        self.location = InodeReadingLocation::new(ext2.block_size() / 4, block)
            .ok_or(Ext2Error::NullBlockSize)?;
        self.check_table1(ext2)?;
        self.check_table2(ext2)?;
        self.check_table3(ext2)?;
        Ok(())
    }

    pub fn get_next_block(&self) -> Result<usize, Ext2Error> {
        Ok(match self.location.location {
            InodeReadingLocationInfo::Direct(direct) => {
                if direct >= 12 {
                    return Err(Ext2Error::NullPointer);
                }
                self.inode.direct_block_pointers[direct] as usize
            }
            InodeReadingLocationInfo::Single(single) => self.follow1(single)?,
            InodeReadingLocationInfo::Double(_, double) => self.follow2(double)?,
            InodeReadingLocationInfo::Triple(_, _, triple) => self.follow3(triple)?,
        })
    }

    pub fn read_block(
        &mut self,
        ext2: &mut Ext2FileSystem,
        buffer: &mut Buffer,
    ) -> Result<usize, Ext2Error> {
        let bs = ext2.block_size();
        if bs == 0 {
            return Err(Ext2Error::NullBlockSize);
        }
        if buffer.len() < bs {
            return Err(Ext2Error::BufferTooSmall(buffer.len(), bs));
        }
        let block = self.get_next_block()?;
        ext2.read_block(block as u64, buffer)?;
        if block < self.max_block {
            Ok(bs)
        } else {
            let read = (self.inode.size_lo as usize) % bs;
            Ok(if read == 0 { bs } else { read })
        }
    }

    pub fn advance(&mut self) -> bool {
        match self.get_next_block() {
            Ok(block) => {
                if block >= self.max_block {
                    false
                } else {
                    self.location.advance();
                    true
                }
            }
            Err(_) => false,
        }
    }
}

pub struct Ext2File<'a> {
    ext2: &'a mut Ext2FileSystem,
    fd: CachedInodeReadingLocation,
}

impl<'a> Ext2File<'a> {
    pub fn new(fd: CachedInodeReadingLocation, ext2: &'a mut Ext2FileSystem) -> Self {
        Self { fd, ext2 }
    }
}

struct Ext2DirectoryEntryrRaw {
    pub inode: u32,
    pub entry_size: u16,
    pub len_lo: u8,
    pub type_or_len_hi: u8,
}

pub struct Ext2DirectoryEntry {
    inode: u32,
    name: Buffer,
}

impl Ext2DirectoryEntry {
    pub fn has_name(&self, name: &[u8]) -> bool {
        if self.name.len() != name.len() {
            false
        } else {
            for i in 0..self.name.len() {
                match (self.name.get(i), name.get(i)) {
                    (Some(a), Some(&b)) => {
                        if a != b {
                            return false;
                        }
                    }
                    _ => return false,
                }
            }
            true
        }
    }

    pub fn get_name(&self) -> &Buffer {
        &self.name
    }

    pub fn get_inode(&self) -> u32 {
        self.inode
    }
}

pub struct Ext2Directory<'a> {
    ext2: &'a mut Ext2FileSystem,
    fd: CachedInodeReadingLocation,
    entries: Vec<Ext2DirectoryEntry>,
    self_entry: usize,
    parent_entry: usize,
}

impl<'a> Ext2Directory<'a> {
    fn new(
        fd: CachedInodeReadingLocation,
        ext2: &'a mut Ext2FileSystem,
    ) -> Result<Self, Ext2Error> {
        let mut dir = Ext2Directory {
            ext2,
            fd,
            entries: Vec::default(),
            self_entry: 0,
            parent_entry: 0,
        };
        // Allocate buffers
        let mut buffer =
            Buffer::new(dir.fd.inode.size_lo as usize).ok_or(Ext2Error::FailedMemAlloc)?;
        let mut block_buffer =
            Buffer::new(dir.ext2.block_size()).ok_or(Ext2Error::FailedMemAlloc)?;

        // Read content
        let mut idx = 0;
        loop {
            let read = dir.fd.read_block(dir.ext2, &mut block_buffer)?;
            block_buffer.copy_to(0, &mut buffer, idx, read);
            idx += read;
            if !dir.fd.advance() {
                break;
            }
        }

        // Parse directory entries
        idx = 0;
        while idx < dir.fd.inode.size_lo as usize {
            let entry_raw = unsafe {
                (buffer.get_ptr().add(idx) as *const Ext2DirectoryEntryrRaw).read_unaligned()
            };
            let name_entry_len = if (dir.ext2.superblock.required_features
                & REQUIRED_FEATURE_DIRECTORY_ENTRIES_HAVE_TYPE_FIELD)
                == REQUIRED_FEATURE_DIRECTORY_ENTRIES_HAVE_TYPE_FIELD
            {
                entry_raw.len_lo as usize
            } else {
                ((entry_raw.type_or_len_hi as usize) << 8) + (entry_raw.len_lo as usize)
            };

            let mut entry = Ext2DirectoryEntry {
                inode: entry_raw.inode,
                name: Buffer::new(name_entry_len).ok_or(Ext2Error::FailedMemAlloc)?,
            };
            if !buffer.copy_to(
                idx + size_of::<Ext2DirectoryEntryrRaw>(),
                &mut entry.name,
                0,
                name_entry_len,
            ) {
                return Err(Ext2Error::DirectoryParseFailed);
            }

            if entry.has_name(b".") {
                dir.self_entry = dir.entries.len();
            }
            if entry.has_name(b"..") {
                dir.parent_entry = dir.entries.len();
            }

            dir.entries.push(entry);

            idx += entry_raw.entry_size as usize;
        }

        Ok(dir)
    }

    pub fn get_inode(&self) -> u32 {
        self.entries
            .get(self.self_entry)
            .unwrap_or_else(|| kpanic())
            .inode
    }

    pub fn get_parent_inode(&self) -> u32 {
        self.entries
            .get(self.parent_entry)
            .unwrap_or_else(|| kpanic())
            .inode
    }

    pub fn listdir(&self) -> RefIterVec<Ext2DirectoryEntry> {
        self.entries.iter()
    }
}

pub enum Ext2FileType<'a> {
    File(Ext2File<'a>),
    Directory(Ext2Directory<'a>),
}

pub struct Ext2FileSystem {
    disk: ExtendedDisk,
    partition: DiskRange,
    superblock: Box<Ext2SuperBlock>,
    block_groups: Vec<Ext2BlockGroupDescriptor>,
    sectors_per_block: usize,
    sector_size: usize,
}

impl Ext2FileSystem {
    pub fn mount_ro(disk: ExtendedDisk, partition: DiskRange) -> Result<Self, Ext2Error> {
        let mut ext2 = Self {
            disk,
            partition,
            superblock: unsafe { Box::from_raw(ptr::null_mut()) },
            block_groups: Vec::default(),
            sectors_per_block: 0,
            sector_size: 0,
        };
        ext2.read_superblock()?;
        ext2.read_block_group_descriptor_table()?;
        Ok(ext2)
    }

    fn read_superblock(&mut self) -> Result<(), Ext2Error> {
        let params = self.disk.get_params().map_err(Ext2Error::DiskError)?;
        let bps = params.bytes_per_sector as usize;
        if bps != 512 && bps != 4096 {
            return Err(Ext2Error::BadDiskSectorSize(params.bytes_per_sector));
        }
        self.sector_size = bps;

        let mut superblock_buffer = Buffer::new(1024).ok_or(Ext2Error::FailedMemAlloc)?;
        let mut buffer = Buffer::new(4096).ok_or(Ext2Error::FailedMemAlloc)?;

        // For dev profile, low optimization doesn't recognize that bps is not 0 from the first !=512 && !=4096 check
        // Gets optimized out on release profile, and removes undefined panick symbols related to division by 0 on dev profile
        // Weak compiler bruh
        if bps == 0 {
            return Err(Ext2Error::BadDiskSectorSize(params.bytes_per_sector));
        }

        let start_lba = 1024 / bps;
        let buf_idx = 1024 % bps;

        self.disk
            .read_to_buffer(start_lba as u64 + self.partition.start_lba, &mut buffer)
            .map_err(Ext2Error::DiskError)?;
        buffer.copy_to(buf_idx, &mut superblock_buffer, 0, 1024);
        self.superblock = superblock_buffer.boxed::<Ext2SuperBlock>();

        if (self.block_size() % bps) != 0 {
            // A block isn't a whole amount of logical sectors
            return Err(Ext2Error::BadBlockSize(
                self.block_size(),
                params.bytes_per_sector,
            ));
        }
        self.sectors_per_block = self.block_size() / bps;

        Ok(())
    }

    fn read_block_group_descriptor_table(&mut self) -> Result<(), Ext2Error> {
        let entry_count = self.count_block_groups()?;
        let table_size = entry_count * BLOCK_GROUP_DESCRIPTOR_SIZE;
        let bs = self.block_size();
        if bs == 0 {
            return Err(Ext2Error::NullBlockSize);
        }
        let mut buffer = Buffer::new(table_size).ok_or(Ext2Error::FailedMemAlloc)?;
        let mut block_buffer = Buffer::new(bs).ok_or(Ext2Error::FailedMemAlloc)?;

        let mut read = 0;
        let mut disk_byte = 2048;

        while read < table_size {
            let disk_block = disk_byte / bs;
            let block_offset = disk_byte % bs;
            let to_copy = (table_size - read).min(bs - block_offset);

            self.read_block(disk_block as u64, &mut block_buffer)?;
            if !block_buffer.copy_to(block_offset, &mut buffer, read, to_copy) {
                return Err(Ext2Error::FailedMemAlloc);
            }

            read += to_copy;
            disk_byte += to_copy;
        }

        self.block_groups.ensure_capacity(entry_count);
        for i in 0..entry_count {
            let offset = i * BLOCK_GROUP_DESCRIPTOR_SIZE;
            let block_group =
                unsafe { &*(buffer.get_ptr().add(offset) as *const Ext2BlockGroupDescriptor) };
            self.block_groups.push(*block_group);
        }

        Ok(())
    }

    unsafe fn unsafe_read_block(&mut self, block: u64, buffer: *mut u8) -> Result<(), Ext2Error> {
        let begin_lba: u64 = block * self.sectors_per_block as u64 + self.partition.start_lba;
        for i in 0..self.sectors_per_block {
            let lba = begin_lba + i as u64;
            let output_addr = buffer.add(i * self.sector_size);

            self.disk
                .unsafe_read_sector_to_buffer(lba, output_addr)
                .map_err(Ext2Error::DiskError)?;
        }
        Ok(())
    }

    fn read_block(&mut self, block: u64, buffer: &mut Buffer) -> Result<(), Ext2Error> {
        if buffer.len() < self.block_size() {
            return Err(Ext2Error::BufferTooSmall(buffer.len(), self.block_size()));
        }
        unsafe { self.unsafe_read_block(block, buffer.get_ptr()) }
    }

    fn count_block_groups(&self) -> Result<usize, Ext2Error> {
        let bpg = self.superblock.blocks_per_group;
        let ipg = self.superblock.inodes_per_group;
        if bpg == 0 || ipg == 0 {
            return Err(Ext2Error::BadSuperblock);
        }
        let r1 = self.superblock.blocks_count.div_ceil(bpg) as usize;
        let r2 = self.superblock.inodes_count.div_ceil(ipg) as usize;
        if r1 != r2 {
            Err(Ext2Error::BadBlockGroupDescriptorTableEntrySize(r1, r2))
        } else {
            Ok(r1)
        }
    }

    pub fn block_size(&self) -> usize {
        1024 << (self.superblock.log_block_size as usize)
    }

    fn get_inode_group(&self, inode: usize) -> usize {
        if self.superblock.inodes_per_group == 0 {
            kpanic();
        }
        (inode - 1) / (self.superblock.inodes_per_group as usize)
    }

    fn get_inode_index_in_group(&self, inode: usize) -> usize {
        if self.superblock.inodes_per_group == 0 {
            kpanic();
        }
        (inode - 1) % (self.superblock.inodes_per_group as usize)
    }

    fn inode_size(&self) -> usize {
        if self.superblock.major_version_level >= 1 {
            self.superblock.inode_struct_size as usize
        } else {
            128
        }
    }

    fn get_inode(&mut self, inode: usize) -> Result<Ext2Inode, Ext2Error> {
        if inode == 0 || inode > self.superblock.inodes_count as usize {
            return Err(Ext2Error::BadInodeIndex(inode));
        }

        let group = self.get_inode_group(inode);
        let index = self.get_inode_index_in_group(inode);

        let block_size = self.block_size();
        if block_size == 0 {
            return Err(Ext2Error::NullBlockSize);
        }

        let inode_size = self.inode_size();
        if inode_size == 0 {
            printf!(b"NULL INODE SIZE ??\r\n");
            return Err(Ext2Error::NullBlockSize);
        }

        let block = self
            .block_groups
            .get(group)
            .ok_or(Ext2Error::BadSuperblock)?
            .inode_table_block as u64;

        let offset = index * inode_size;
        let mut block_buffer = Buffer::new(self.block_size()).ok_or(Ext2Error::FailedMemAlloc)?;
        let mut buffer = Buffer::new(inode_size).ok_or(Ext2Error::FailedMemAlloc)?;

        unsafe {
            self.read_block(block, &mut block_buffer)?;
            if !block_buffer.copy_to(offset, &mut buffer, 0, inode_size) {
                printf!(
                    b"\r\n\r\nWHAT THE FUCK ???\r\nblock_buffer %x (len %x) | buffer %x (len %x)\r\nTried copy: source offset %x | dest offset %x | amount %x\r\n",
                    block_buffer.get_ptr() as u32,
                    block_buffer.len() as u32,
                    buffer.get_ptr() as u32,
                    buffer.len() as u32,
                    offset as u32,
                    0,
                    inode_size as u32
                );

                kpanic();
            }

            let inode = (buffer.get_ptr() as *mut Ext2Inode).read_unaligned();
            Ok(inode)
        }
    }

    fn open_inode(&mut self, inode: usize) -> Result<CachedInodeReadingLocation, Ext2Error> {
        let inode = self.get_inode(inode)?;
        CachedInodeReadingLocation::new(self, inode)
    }

    pub fn open<'a>(&'a mut self, inode: usize) -> Result<Ext2FileType<'a>, Ext2Error> {
        let fd = self.open_inode(inode)?;
        if (fd.inode.type_and_permissions & INODE_TYPE_DIRECTORY) == INODE_TYPE_DIRECTORY {
            Ok(Ext2FileType::Directory(Ext2Directory::new(fd, self)?))
        } else if (fd.inode.type_and_permissions & INODE_TYPE_REGULAR_FILE)
            == INODE_TYPE_REGULAR_FILE
        {
            Ok(Ext2FileType::File(Ext2File::new(fd, self)))
        } else {
            Err(Ext2Error::UnsupportedInodeType(
                fd.inode.type_and_permissions,
            ))
        }
    }
}
