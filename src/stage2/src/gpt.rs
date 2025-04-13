use crate::{
    bios::{DiskError, ExtendedDisk},
    kpanic,
    mem::{Buffer, Vec},
    video::Video,
};

#[repr(C, packed)]
struct MBRPartition {
    pub bootable: u8,
    pub start_chs: [u8; 3],
    pub os_type: u8,
    pub end_chs: [u8; 3],
    pub start_lba: u32,
    pub end_lba: u32,
}

impl MBRPartition {
    pub fn is_null(&self) -> bool {
        self.bootable == 0
            && self.start_chs == [0, 0, 0]
            && self.os_type == 0
            && self.end_chs == [0, 0, 0]
            && self.start_lba == 0
            && self.end_lba == 0
    }
}

#[repr(C, packed)]
struct MasterBootRecord {
    pub boot_code: [u8; 446],
    pub mbr_partitions: [MBRPartition; 4],
    pub signature: [u8; 2],
}

#[repr(C, packed)]
pub struct GPTHeader {
    pub signature: [u8; 8],
    pub revision: u32,
    pub header_size: u32,
    pub header_crc32: u32,
    pub reserved: u32,
    pub current_lba: u64,
    pub backup_lba: u64,
    pub first_usable_lba: u64,
    pub last_usable_lba: u64,
    pub disk_guid: [u8; 16],
    pub partition_table_lba: u64,
    pub partition_entry_count: u32,
    pub partition_entry_size: u32,
    pub partition_entries_crc32: u32,
}

#[repr(C, packed)]
struct GUIDPartitionTableEntryRaw {
    pub type_guid: [u8; 16],
    pub unique_guid: [u8; 16],
    pub first_lba: u64,
    pub last_lba: u64,
    pub flags: u64,
}

pub struct GUIDPartitionTableEntry {
    pub type_guid: [u8; 16],
    pub unique_guid: [u8; 16],
    pub first_lba: u64,
    pub last_lba: u64,
    pub flags: u64,
    pub name: Buffer,
}

impl GUIDPartitionTableEntry {
    pub fn as_disk_range(&self) -> DiskRange {
        DiskRange {
            start_lba: self.first_lba,
            end_lba: self.last_lba,
        }
    }
}

pub struct GUIDPartitionTable {
    header: GPTHeader,
    partitions: Vec<GUIDPartitionTableEntry>,
}

impl GUIDPartitionTable {
    pub fn get_partitions(&self) -> &Vec<GUIDPartitionTableEntry> {
        &self.partitions
    }

    pub fn get_header(&self) -> &GPTHeader {
        &self.header
    }

    pub fn as_disk_range(&self) -> DiskRange {
        DiskRange {
            start_lba: self.header.first_usable_lba,
            end_lba: self.header.last_usable_lba,
        }
    }
}

pub struct DiskRange {
    pub start_lba: u64,
    pub end_lba: u64,
}

pub enum GPTError {
    FailedMemAlloc,
    BadSectorSize,
    BadMasterBootRecord,
    NotGPT,
    UnsupportedTableLBA,
    DiskError(DiskError),
}

impl GPTError {
    pub fn panic(&self) -> ! {
        unsafe {
            let video = Video::get();
            match self {
                GPTError::DiskError(e) => {
                    video.write_string(b"GUID Partition Table reading error caused by:\n");
                    e.panic();
                }
                GPTError::FailedMemAlloc => {
                    video.write_string(b"Failed to allocate memory\n");
                }
                GPTError::BadSectorSize => {
                    video.write_string(b"Bad disk sector size\n");
                }
                GPTError::BadMasterBootRecord => {
                    video.write_string(b"Bad Master Boot Record\n");
                }
                GPTError::NotGPT => {
                    video.write_string(b"Disk is not GPT formatted\n");
                }
                GPTError::UnsupportedTableLBA => {
                    video.write_string(b"Unsupported parition table LBA\n");
                }
            }
        }
        kpanic();
    }
}

impl GUIDPartitionTable {
    pub fn read(disk: &mut ExtendedDisk) -> Result<GUIDPartitionTable, GPTError> {
        let disk_params = disk.get_params().map_err(GPTError::DiskError)?;

        let sector_size = disk_params.bytes_per_sector as usize;
        if sector_size != 512 {
            return Err(GPTError::BadSectorSize);
        }

        let max_lba = disk_params.sectors - 1;

        let mut buffer = Buffer::new(34 * 512).ok_or(GPTError::FailedMemAlloc)?; // 34 logical 512-byte sectors
        let mut sector_buffer = Buffer::new(sector_size).ok_or(GPTError::FailedMemAlloc)?; // 1 physiqual sector

        let mut read = 0;
        let mut lba = 0;
        while read < 34 * 512 {
            disk.read_sector(lba, &mut sector_buffer)
                .map_err(GPTError::DiskError)?;

            let to_copy = (34 * 512 - read).min(sector_size);
            sector_buffer.copy_to(0, &mut buffer, read, to_copy);

            read += sector_size;
            lba += 1;
        }

        let mbr = unsafe { (buffer.get_ptr() as *const MasterBootRecord).read_unaligned() };
        if mbr.signature[0] != 0x55 || mbr.signature[1] != 0xAA {
            return Err(GPTError::BadMasterBootRecord);
        }

        if mbr.mbr_partitions[0].bootable != 0
            || mbr.mbr_partitions[0].os_type != 0xEE
            || mbr.mbr_partitions[0].start_chs[0] != 0
            || mbr.mbr_partitions[0].start_chs[1] != 2
            || mbr.mbr_partitions[0].start_chs[2] != 0
            || mbr.mbr_partitions[0].start_lba != 1
            || (if max_lba > u32::MAX as u64 {
                mbr.mbr_partitions[0].end_lba != u32::MAX
            } else {
                mbr.mbr_partitions[0].end_lba != max_lba as u32
            })
        {
            return Err(GPTError::NotGPT);
        }

        for i in 1..4 {
            if !mbr.mbr_partitions[i].is_null() {
                return Err(GPTError::NotGPT);
            }
        }

        let header = unsafe { (buffer.get_ptr().add(512) as *const GPTHeader).read_unaligned() };

        if &header.signature != b"EFI PART" || header.header_size != 0x5C {
            return Err(GPTError::NotGPT);
        }

        if header.partition_table_lba != 2 {
            return Err(GPTError::UnsupportedTableLBA);
        }

        let entry_size = header.partition_entry_size as usize;
        let part_count = header.partition_entry_count as usize;
        let name_size = header.partition_entry_size as usize - 0x38;

        let mut table = GUIDPartitionTable {
            header,
            partitions: Vec::new(part_count),
        };

        for i in 0..part_count {
            let (entry, name) = unsafe {
                let addr = buffer.get_ptr().add(1024 + entry_size * i);
                let entry = (addr as *const GUIDPartitionTableEntryRaw).read_unaligned();

                if entry.type_guid == [0; 16] {
                    continue;
                }

                let name = Buffer::new(name_size).ok_or(GPTError::FailedMemAlloc)?;
                (entry, name)
            };

            let part = GUIDPartitionTableEntry {
                type_guid: entry.type_guid,
                unique_guid: entry.unique_guid,
                first_lba: entry.first_lba,
                last_lba: entry.last_lba,
                flags: entry.flags,
                name,
            };

            table.partitions.push(part);
        }

        Ok(table)
    }
}
