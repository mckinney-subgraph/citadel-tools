use std::fs;
use std::io::{self, Write};
use std::path::{PathBuf, Path};
use std::process::Command;

use sodiumoxide::randombytes::randombytes;

use crate::{Result, RealmFS, FileLock, ImageHeader, LoopDevice, ResizeSize, util};
use crate::realm::BridgeAllocator;
use crate::util::is_euid_root;
use crate::terminal::TerminalRestorer;
use crate::verity::Verity;

const BLOCK_SIZE: usize  = 4096;

// The maximum number of backup copies the rotate() method will create
const NUM_BACKUPS: usize = 2;

const E2FSCK: &str = "e2fsck";
const RESIZE2FS: &str = "resize2fs";

/// Manages the process of updating or resizing a `RealmFS` image file.
///
pub struct Update<'a> {
    realmfs: &'a RealmFS,    // RealmFS being updated
    name: String,            // name for nspawn instance
    target: PathBuf,         // Path to the update copy of realmfs image
    mountpath: PathBuf,      // Path at which update copy is mounted
    _lock: FileLock,
    resize: Option<ResizeSize>,   // If the image needs to be resized, the resize size is stored here
    network_allocated: bool,
}

impl <'a> Update<'a> {
    fn new(realmfs: &'a RealmFS, lock: FileLock) -> Self {

        let metainfo = realmfs.metainfo();
        let tag = metainfo.verity_tag();
        let mountpath = Path::new(RealmFS::RUN_DIRECTORY)
            .join(format!("realmfs-{}-{}.update", realmfs.name(), tag));

        Update {
            realmfs,
            name: format!("{}-{}-update", realmfs.name(), tag),
            target: realmfs.path().with_extension("update"),
            mountpath,
            _lock: lock,
            resize: ResizeSize::auto_resize_size(realmfs),
            network_allocated: false,
        }
    }

    pub fn create(realmfs: &'a RealmFS) -> Result<Self> {
        let lock = FileLock::nonblocking_acquire(realmfs.path().with_extension("lock"))?
            .ok_or(format_err!("Unable to obtain file lock to update realmfs image: {}", realmfs.name()))?;

        if !realmfs.has_sealing_keys() {
            bail!("Cannot seal realmfs image, no sealing keys available");
        }

        Ok(Update::new(realmfs, lock))
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn target(&self) -> &Path {
        &self.target
    }

    fn create_update_copy(&self) -> Result<()> {
        if self.target.exists() {
            info!("Update file {} already exists, removing it", self.target.display());
            fs::remove_file(&self.target)?;
        }
        self.realmfs.copy_image_file(self.target())?;

        self.truncate_verity()?;
        self.resize_image_file()?;
        Ok(())
    }

    fn setup(&mut self) -> Result<()> {
        self.create_update_copy()?;
        self.truncate_verity()?;
        self.resize_image_file()?;
        self.mount_update_image()?;
        Ok(())
    }

    pub fn resize(&mut self) -> Result<()> {
        if self.resize.is_none() {
            return Ok(())
        }

        self.create_update_copy()?;
        self.truncate_verity()?;
        self.resize_image_file()?;

        LoopDevice::with_loop(self.target(), Some(BLOCK_SIZE), false, |loopdev| {
            self.resize_device(loopdev)
        })
    }

    fn mount_update_image(&mut self) -> Result<()> {
        LoopDevice::with_loop(self.target(), Some(BLOCK_SIZE), false, |loopdev| {
            if self.resize.is_some() {
                self.resize_device(loopdev)?;
            }
            if !self.mountpath.exists() {
                fs::create_dir_all(&self.mountpath)?;
            }
            util::mount(loopdev.device_str(), &self.mountpath, Some("-orw,noatime"))?;
            Ok(())
        })
    }

    // Return size of image file in blocks based on metainfo `nblocks` field.
    // Include header block in count so add one block
    fn metainfo_nblock_size(&self) -> usize {
        self.realmfs.metainfo().nblocks() + 1
    }

    fn unmount_update_image(&mut self) {
        if self.mountpath.exists() {
            if let Err(err) = util::umount(&self.mountpath) {
                warn!("Failed to unmount directory {:?}: {}", self.mountpath, err);

            }
            if let Err(err) = fs::remove_dir(&self.mountpath) {
                warn!("Failed to remove mountpoint directory {:?}: {}", self.mountpath, err);
            }
        }
    }

    fn resize_device(&self, loopdev: &LoopDevice) -> Result<()> {
        info!("Running e2fsck {:?}", loopdev);
        cmd!(E2FSCK,"{} {} {}","-f","-p", loopdev.device().display())?;
        info!("Running resize2fs {:?}", loopdev);
        cmd!(RESIZE2FS, "{}", loopdev.device().display())?;
        Ok(())
    }

    pub fn grow_to(&mut self, size: ResizeSize) {
        let target_nblocks = size.nblocks();
        let current_nblocks = self.metainfo_nblock_size();
        if current_nblocks >= target_nblocks {
            info!("RealmFS image is already larger than requested size, doing nothing");
        } else {
            self.set_resize(target_nblocks);
        }
    }

    pub fn grow_by(&mut self, size: ResizeSize) {
        let nblocks = size.nblocks();
        self.set_resize(self.metainfo_nblock_size() + nblocks);
    }

    fn set_resize(&mut self, nblocks: usize) {
        self.resize = Some(ResizeSize::blocks(nblocks));
    }

    fn set_target_len(&self, nblocks: usize) -> Result<()> {
        let len = (nblocks * BLOCK_SIZE) as u64;
        let f = fs::OpenOptions::new()
            .write(true)
            .open(&self.target)?;
        f.set_len(len)?;
        Ok(())
    }

    // Remove dm-verity hash tree from update copy of image file.
    fn truncate_verity(&self) -> Result<()> {
        let file_nblocks = self.realmfs.file_nblocks()?;
        let metainfo_nblocks = self.metainfo_nblock_size();

        if self.realmfs.header().has_flag(ImageHeader::FLAG_HASH_TREE) {
            self.set_target_len(metainfo_nblocks)?;
        } else if file_nblocks > metainfo_nblocks {
            warn!("RealmFS image size was greater than length indicated by metainfo.nblocks but FLAG_HASH_TREE not set");
        }
        Ok(())
    }

    // If resize was requested, adjust size of update copy of image file.
    fn resize_image_file(&self) -> Result<()> {
        let nblocks = match self.resize {
            Some(rs) => rs.nblocks() + 1,
            None => return Ok(()),
        };

        if nblocks < self.metainfo_nblock_size() {
            bail!("Cannot shrink image")
        }
        // This is an arbitrary restriction which is probably not needed
        if (nblocks - self.metainfo_nblock_size()) > ResizeSize::gigs(8).nblocks() {
            bail!("Can only increase size of RealmFS image by a maximum of 8gb at one time");
        }
        self.set_target_len(nblocks)
    }

    pub fn cleanup(&mut self) {
        if self.mountpath.exists() {
            self.unmount_update_image();
        }

        if self.target().exists() {
            if let Err(err) = fs::remove_file(self.target()) {
                warn!("Failed to remove update image copy {:?}: {}", self.target(), err);
            }
        }

        // If an IP address was allocated, free it
        if self.network_allocated {
            if let Err(err) = BridgeAllocator::default_bridge()
                .and_then(|mut allocator| allocator.free_allocation_for(&self.name())) {

                warn!("Error releasing address allocation for RealmFS ({}) update: {}", self.realmfs.name(), err);
            }
            self.network_allocated = false;
        }
    }

    fn seal(&mut self) -> Result<()> {
        let nblocks = match self.resize {
            Some(rs) => rs.nblocks(),
            None => self.metainfo_nblock_size() - 1,
        };

        let salt = hex::encode(randombytes(32));
        let verity = Verity::new(&self.target)?;
        let output = verity.generate_image_hashtree_with_salt(&salt, nblocks)?;
        // XXX passes metainfo for nblocks
        //let output = Verity::new(&self.target).generate_image_hashtree_with_salt(&self.realmfs.metainfo(), &salt)?;
        let root_hash = output.root_hash()
            .ok_or_else(|| format_err!("no root hash returned from verity format operation"))?;
        info!("root hash is {}", output.root_hash().unwrap());

        /*
        let nblocks = match self.resize {
            Some(rs) => rs.nblocks(),
            None => self.metainfo_nblock_size() - 1,
        };

         */

        info!("Signing new image with user realmfs keys");
        let metainfo_bytes = RealmFS::generate_metainfo(self.realmfs.name(), nblocks, salt.as_str(), root_hash);
        let keys = self.realmfs.sealing_keys().expect("No sealing keys");
        let sig = keys.sign(&metainfo_bytes);
        let header = ImageHeader::new();
        header.set_flag(ImageHeader::FLAG_HASH_TREE);
        header.update_metainfo(&metainfo_bytes, sig.to_bytes(), &self.target)
    }


    fn prompt_user(prompt: &str, default_y: bool) -> Result<bool> {
        let yn = if default_y { "(Y/n)" } else { "(y/N)" };
        print!("{} {} : ", prompt, yn);
        io::stdout().flush()?;
        let mut line = String::new();
        io::stdin().read_line(&mut line)?;

        let yes = match line.trim().chars().next() {
            Some(c) => c == 'Y' || c == 'y',
            None => default_y,
        };
        Ok(yes)
    }

    pub fn run_interactive_update(&mut self, scheme: Option<&str>) -> Result<()> {
        if !is_euid_root() {
            bail!("RealmFS updates must be run as root");
        }
        let mut term = TerminalRestorer::new();
        if let Some(scheme) = scheme {
            term.save_palette();
            term.apply_base16_by_slug(scheme);
        }

        self.setup()?;

        println!();
        println!("Opening update shell for '{}-realmfs.img'", self.realmfs.name());
        println!();
        println!("Exit shell with ctrl-d or 'exit' to return to realm manager");
        println!();

        self.run_update_shell("/usr/libexec/configure-host0.sh && exec /bin/bash")?;

        if Self::prompt_user("Apply changes?", true)? {
            if let Err(err) = self.apply_update() {
                warn!("Failed to apply update changes: {}", err);
            }
        }

        self.cleanup();

        Ok(())
    }

    pub fn run_update_shell(&mut self, command: &str) -> Result<()> {

        let mut alloc = BridgeAllocator::default_bridge()?;
        let addr = alloc.allocate_address_for(&self.name())?;
        let gw = alloc.gateway();
        self.network_allocated = true;
        Command::new("/usr/bin/systemd-nspawn")
            .arg(format!("--setenv=IFCONFIG_IP={}", addr))
            .arg(format!("--setenv=IFCONFIG_GW={}", gw))
            .arg("--quiet")
            .arg(format!("--machine={}", self.name()))
            .arg(format!("--directory={}", &self.mountpath.display()))
            .arg("--network-zone=clear")
            .arg("/bin/bash")
            .arg("-c")
            .arg(command)
            .status()
            .map_err(|e| {
                let _ = self.cleanup();
                e
            })?;
        Ok(())
    }

    fn apply_update(&mut self) -> Result<()> {
        self.unmount_update_image();
        self.seal()?;
        self.rotate()?;
        Ok(())
    }

    fn rotate(&self) -> Result<()> {
        let backup = |n: usize|
            Path::new(RealmFS::BASE_PATH)
                .join(format!("{}-realmfs.img.{}", self.realmfs.name(), n));

        for i in (1..NUM_BACKUPS).rev() {
            let from = backup(i - 1);
            if from.exists() {
                fs::rename(from, backup(i))?;
            }
        }
        fs::rename(self.realmfs.path(), backup(0))?;
        fs::rename(self.target(), self.realmfs.path())?;
        Ok(())
    }
}

impl <'a> Drop for Update<'a> {
    fn drop(&mut self) {
        self.cleanup();
    }
}

