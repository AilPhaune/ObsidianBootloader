use core::slice;

use crate::{bios::{DiskError, ExtendedDisk}, mem::{free, malloc, memcpy, Box}};

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

#[repr(C, packed)]
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
    FailedMemAlloc,
}

pub struct Ext2FileSystem {
    disk: ExtendedDisk,
    superblock: Option<Box<Ext2SuperBlock>>,
}

impl Ext2FileSystem {
    pub fn new(disk: ExtendedDisk) -> Result<Self, Ext2Error> {
        let mut ext2 = Self { disk, superblock: None };
        ext2.read_superblock()?;
        Ok(ext2)
    }

    #[no_mangle]
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
            self.superblock = Some(Box::from_raw(superblock_buffer as *mut Ext2SuperBlock));
            free(buffer);
        }

        Ok(())
    }
}