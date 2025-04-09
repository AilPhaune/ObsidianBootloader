use core::{ptr, slice};

use crate::{bios::{DiskError, ExtendedDisk}, kpanic, mem::{free, malloc, memcpy, Box, Vec}};

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

pub enum Ext2Error {
    DiskError(DiskError),
    BadDiskSectorSize(u16),
    BadBlockSize(usize, u16),
    BadBlockGroupDescriptorTableEntrySize(usize, usize),
    NullBlockSize,
    BadSuperblock,
    FailedMemAlloc,
}

pub struct Ext2FileSystem {
    disk: ExtendedDisk,
    superblock: Box<Ext2SuperBlock>,
    pub block_groups: Vec<Ext2BlockGroupDescriptor>,
    sectors_per_block: usize,
}

impl Ext2FileSystem {
    pub fn mount_ro(disk: ExtendedDisk) -> Result<Self, Ext2Error> {
        let mut ext2 = Self {
            disk,
            superblock: unsafe { Box::from_raw(ptr::null_mut()) },
            block_groups: Vec::default(),
            sectors_per_block: 0
        };
        ext2.read_superblock()?;
        ext2.read_block_group_descriptor_table()?;
        Ok(ext2)
    }

    fn read_superblock(&mut self) -> Result<(), Ext2Error> {
        let params = self.disk.get_params().map_err(Ext2Error::DiskError)?;
        if params.bytes_per_sector != 512 && params.bytes_per_sector != 4096 {
            return Err(Ext2Error::BadDiskSectorSize(params.bytes_per_sector));
        }
        let superblock_buffer = malloc::<u8>(1024).ok_or(Ext2Error::FailedMemAlloc)?;
        let buffer = malloc::<u8>(4096).ok_or(Ext2Error::FailedMemAlloc)?;

        let start_lba = 1024 / (params.bytes_per_sector as usize);
        let buf_idx = 1024 % (params.bytes_per_sector as usize);

        unsafe {
            self.disk.read_to_buffer(start_lba as u64, slice::from_raw_parts_mut(buffer, 4096)).map_err(Ext2Error::DiskError)?;
            memcpy(superblock_buffer, buffer.add(buf_idx), 1024);
            self.superblock = Box::from_raw(superblock_buffer as *mut Ext2SuperBlock);
            free(buffer);
        }

        if (self.block_size() % (params.bytes_per_sector as usize)) != 0 {
            // A block isn't a whole amount of logical sectors
            return Err(Ext2Error::BadBlockSize(self.block_size(), params.bytes_per_sector));
        }
        self.sectors_per_block = self.block_size() / (params.bytes_per_sector as usize);

        Ok(())
    }

    fn read_block_group_descriptor_table(&mut self) -> Result<(), Ext2Error> {
        let entry_count = self.count_block_groups()?;
        let table_size = entry_count * BLOCK_GROUP_DESCRIPTOR_SIZE;
        let bs = self.block_size();
        if bs == 0 {
            return Err(Ext2Error::NullBlockSize);
        }
        let buffer = malloc::<u8>(table_size).ok_or(Ext2Error::FailedMemAlloc)?;
        let block_buffer = malloc::<u8>(bs).ok_or(Ext2Error::FailedMemAlloc)?;

        let mut read = 0;
        let mut disk_byte = 2048;

        while read < table_size {
            let disk_block = disk_byte / bs;
            let block_offset = disk_byte % bs;
            let to_copy = (table_size - read).min(bs - block_offset);
            unsafe {
                self.read_block(disk_block as u64, block_buffer)?;
                memcpy(buffer.add(read), block_buffer.add(block_offset), to_copy);
            }
            read += to_copy;
            disk_byte += to_copy;
        }

        self.block_groups.ensure_capacity(entry_count);
        for i in 0..entry_count {
            let offset = i * BLOCK_GROUP_DESCRIPTOR_SIZE;
            let block_group = unsafe { &*(buffer.add(offset) as *const Ext2BlockGroupDescriptor) };
            self.block_groups.push(*block_group);
        }

        unsafe {
            free(buffer);
            free(block_buffer);
        }

        Ok(())
    }

    unsafe fn read_block(&mut self, block: u64, buffer: *mut u8) -> Result<(), Ext2Error> {
        let begin_lba: u64 = block * self.sectors_per_block as u64;
        for i in 0..self.sectors_per_block {
            self.disk
                .unsafe_read_sector_to_buffer(begin_lba + i as u64, buffer.add(i * self.block_size()))
                .map_err(Ext2Error::DiskError)?;
        }
        Ok(())
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

    fn block_size(&self) -> usize {
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
}