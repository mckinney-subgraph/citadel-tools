use std::fs::File;
use std::io::{Read,Seek,SeekFrom};
use std::path::Path;

use byteorder::{ByteOrder,LittleEndian};

use crate::{RealmFS,Result};

const BLOCK_SIZE: usize  = 4096;
const BLOCKS_PER_MEG: usize = (1024 * 1024) / BLOCK_SIZE;
const BLOCKS_PER_GIG: usize = 1024 * BLOCKS_PER_MEG;

// If less than 1gb remaining space
const AUTO_RESIZE_MINIMUM_FREE: ResizeSize = ResizeSize(BLOCKS_PER_GIG);
// ... add 4gb to size of image
const AUTO_RESIZE_INCREASE_SIZE: ResizeSize = ResizeSize(4 * BLOCKS_PER_GIG);


#[derive(Copy,Clone)]
pub struct ResizeSize(usize);

impl ResizeSize {

    pub fn gigs(n: usize) -> Self {
        ResizeSize(BLOCKS_PER_GIG * n)

    }
    pub fn megs(n: usize) -> Self {
        ResizeSize(BLOCKS_PER_MEG * n)
    }

    pub fn blocks(n: usize) -> Self {
        ResizeSize(n)
    }

    pub fn nblocks(&self) -> usize {
        self.0
    }

    pub fn size_in_gb(&self) -> usize {
        self.0 / BLOCKS_PER_GIG
    }

    pub fn size_in_mb(&self) -> usize {
        self.0 / BLOCKS_PER_MEG
    }

    /// If the RealmFS needs to be resized to a larger size, returns the
    /// recommended size.
    pub fn auto_resize_size(realmfs: &RealmFS) -> Option<ResizeSize> {
        let sb = match Superblock::load(realmfs.path(), 4096) {
            Ok(sb) => sb,
            Err(e) => {
                warn!("Error reading superblock from {}: {}", realmfs.path().display(), e);
                return None;
            },
        };

        sb.free_block_count();
        let free_blocks = sb.free_block_count() as usize;
        if free_blocks < AUTO_RESIZE_MINIMUM_FREE.nblocks() {
            let metainfo_nblocks = realmfs.metainfo().nblocks() + 1;
            let increase_multiple = metainfo_nblocks / AUTO_RESIZE_INCREASE_SIZE.nblocks();
            let grow_size = (increase_multiple + 1) * AUTO_RESIZE_INCREASE_SIZE.nblocks();
            let mask = grow_size - 1;
            let grow_blocks = (free_blocks + mask) & !mask;
            Some(ResizeSize::blocks(grow_blocks))
        } else {
            None
        }
    }
}

const SUPERBLOCK_SIZE: usize = 1024;
pub struct Superblock([u8; SUPERBLOCK_SIZE]);

impl Superblock {
    fn new() -> Self {
        Superblock([0u8; SUPERBLOCK_SIZE])
    }

    pub fn load(path: impl AsRef<Path>, offset: u64) -> Result<Self> {
        let path = path.as_ref();
        let mut sb = Self::new();
        let mut file = File::open(path)
            .map_err(context!("failed to open image file {:?}", path))?;
        file.seek(SeekFrom::Start(1024 + offset))
            .map_err(context!("failed to seek to offset {} of image file {:?}", 1024 + offset, path))?;
        file.read_exact(&mut sb.0)
            .map_err(context!("error reading superblock from image file {:?}", path))?;
        Ok(sb)
    }

    pub fn free_block_count(&self) -> u64 {
        self.split_u64(0x0C, 0x158)
    }

    fn u32(&self, offset: usize) -> u32 {
        LittleEndian::read_u32(self.at(offset))
    }

    fn split_u64(&self, offset_lo: usize, offset_hi: usize) -> u64 {
        let lo = u64::from(self.u32(offset_lo));
        let hi = u64::from(self.u32(offset_hi));
        (hi << 32) | lo
    }

    fn at(&self, offset: usize) -> &[u8] {
        &self.0[offset..]
    }
}
