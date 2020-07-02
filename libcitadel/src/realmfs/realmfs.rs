use std::ffi::OsStr;
use std::fs;
use std::io::Write;
use std::os::unix::fs::MetadataExt;
use std::path::{Path,PathBuf};
use std::sync::{Arc, Weak, RwLock};

use crate::{ImageHeader, MetaInfo, Result, KeyRing, KeyPair, util, RealmManager, PublicKey, ResizeSize};
use crate::realmfs::resizer::Superblock;
use crate::realmfs::update::Update;
use super::mountpoint::Mountpoint;

// Maximum length of a RealmFS name
const MAX_REALMFS_NAME_LEN: usize = 40;

///
/// Representation of a RealmFS disk image file.
///
/// RealmFS images contain the root filesystem for one or more realms. A single RealmFS
/// image may be shared by multiple running realm instances.
///
/// A RealmFS image header includes metadata needed to mount the image with dm-verity to securely
/// securely enforce read-only access to the image. This header is signed with either regular
/// channel keys or with a user-controlled key generated upon installation and stored in the kernel
/// keyring.
///
/// RealmFS images are normally stored in the directory `BASE_PATH` (/storage/realms/realmfs-images),
/// and images stored in this directory can be loaded by name rather than needing the exact path
/// to the image.
///
/// RealmFS image files in this directory are named $NAME-realmfs.img so the full path to a RealmFS
/// image with name 'main' would be:
///
///     /storage/realms/realmfs-images/main-realmfs.img
///
#[derive(Clone)]
pub struct RealmFS {
    // RealmFS name
    name: Arc<String>,
    // path to RealmFS image file
    path: Arc<PathBuf>,
    // current RealmFS image file header
    header: Arc<ImageHeader>,
    // mountpoint of the path this realmfs is mounted at when activated
    mountpoint: Arc<RwLock<Mountpoint>>,

    manager: Weak<RealmManager>,
}

impl RealmFS {
    // Directory where RealmFS images are stored
    pub const BASE_PATH: &'static str = "/storage/realms/realmfs-images";

    // Directory where RealmFS mountpoints are created
    pub const RUN_DIRECTORY: &'static str = "/run/citadel/realmfs";

    // Name used to retrieve key by 'description' from kernel key storage
    pub const USER_KEYNAME: &'static str = "realmfs-user";

    /// Locate a RealmFS image by name in the default location using the standard name convention
    pub fn load_by_name(name: &str) -> Result<Self> {
        Self::validate_name(name)?;
        let path = Self::image_path(name);
        if !path.exists() {
            bail!("No image found at {}", path.display());
        }

        Self::load_from_path(path)
    }

    /// Load RealmFS image from an exact path.
    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self> {
        let header = Self::load_realmfs_header(path.as_ref())?;
        let metainfo = header.metainfo();

        let name = metainfo.realmfs_name()
            .expect("RealmFS does not have a name");

        let mountpoint = Mountpoint::new(name, metainfo.verity_tag());

        Ok(RealmFS::new(name, path.as_ref(), header, mountpoint))
    }

    fn new(name: &str, path: &Path, header: ImageHeader, mountpoint: Mountpoint) -> Self {
        RealmFS {
            name: Arc::new(name.to_owned()),
            path: Arc::new(path.to_owned()),
            header: Arc::new(header),
            mountpoint: Arc::new(RwLock::new(mountpoint)),
            manager: Weak::new(),
        }
    }

    pub(super) fn set_manager(&mut self, manager: Arc<RealmManager>) {
        self.manager = Arc::downgrade(&manager);
    }

    pub fn manager(&self) -> Arc<RealmManager> {
        self.manager.upgrade()
            .expect(&format!("No manager set on realmfs {}", self.name))
    }

    fn with_manager<F>(&self, f: F)
        where F: FnOnce(Arc<RealmManager>)
    {
        if let Some(manager) = self.manager.upgrade() {
            f(manager);
        }
    }

    pub fn is_valid_realmfs_image(path: impl AsRef<Path>) -> bool {
        Self::load_realmfs_header(path.as_ref()).is_ok()
    }

    fn load_realmfs_header(path: &Path) -> Result<ImageHeader> {
        let header = ImageHeader::from_file(path)?;
        if !header.is_magic_valid() {
            bail!("Image file {} does not have a valid header", path.display());
        }
        let metainfo = header.metainfo();
        if metainfo.image_type()  != "realmfs" {
            bail!("Image file {} is not a realmfs image", path.display());
        }
        match metainfo.realmfs_name() {
            Some(name) => Self::validate_name(name)?,
            None => bail!("RealmFS image file {} does not have a 'realmfs-name' field", path.display()),
        };
        Ok(header)
    }

    /// Return an Error result if name is not valid.
    fn validate_name(name: &str) -> Result<()> {
        if !Self::is_valid_name(name) {
            bail!("Invalid realm name '{}'", name);
        }
        Ok(())
    }

    /// Return `true` if `name` is a valid name for a RealmFS.
    ///
    /// Valid names:
    ///   * Are 40 characters or less in length
    ///   * Have an alphabetic ascii letter as first character
    ///   * Contain only alphanumeric ascii characters or '-' (dash)
    ///
    pub fn is_valid_name(name: &str) -> bool {
        util::is_valid_name(name, MAX_REALMFS_NAME_LEN)
    }

    pub fn named_image_exists(name: &str) -> bool {
        if !util::is_valid_name(name, MAX_REALMFS_NAME_LEN) {
            return false;
        }
        Self::is_valid_realmfs_image(Self::image_path(name))
    }

    fn image_path(name: &str) -> PathBuf {
        Path::new(Self::BASE_PATH).join(format!("{}-realmfs.img", name))
    }

    /// Return the `Path` to this RealmFS image file.
    pub fn path(&self) -> &Path {
        self.path.as_ref()
    }

    pub fn mountpoint(&self) -> Mountpoint {
        let lock = self.mountpoint.read().unwrap();
        lock.clone()
    }

    pub fn delete(&self) -> Result<()> {
        self.manager().delete_realmfs(self)
    }

    /// Return a new `PathBuf` based on the path of the current image by appending
    /// the string `ext` as an extension to the filename. If the current filename
    /// ends with '.img' then the specified extension is appended to this as '.img.ext'
    /// otherwise it replaces any existing extension.
    fn path_with_extension(&self, ext: &str) -> PathBuf {
        if self.path.extension() == Some(OsStr::new("img")) {
            self.path.with_extension(format!("img.{}", ext))
        } else {
            self.path.with_extension(ext)
        }
    }

    /// Return a new `PathBuf` based on the path of the current image by replacing
    /// the image filename with the specified name.
    pub fn path_with_filename(&self, filename: impl AsRef<str>) -> PathBuf {
        let mut path = (*self.path).clone();
        path.pop();
        path.push(filename.as_ref());
        path
    }

    /// Return the 'realmfs-name' metainfo field of this image.
    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    pub fn notes(&self) -> Option<String> {
        let path = self.path_with_extension("notes");
        if path.exists() {
            return fs::read_to_string(path).ok();
        }
        None
    }

    pub fn save_notes(&self, notes: impl AsRef<str>) -> Result<()> {
        let path = self.path_with_extension("notes");
        let notes = notes.as_ref();
        if path.exists() && notes.is_empty() {
            util::remove_file(&path)
        } else {
            util::write_file(&path, notes)
        }
    }

    /// Return `MetaInfo` from image header of this RealmFS.
    pub fn metainfo(&self) -> Arc<MetaInfo> {
        self.header().metainfo()
    }

    // Each time RealmFS header is accessed, verify that the header on disk has not changed.
    // If the header changes generate a new mountpoint instance because the verity tag may
    // have changed.
    fn check_stale_header(&self) -> Result<()> {
        if self.header.reload_if_stale(self.path())? {
            let mut lock = self.mountpoint.write().unwrap();
            *lock = Mountpoint::new(self.name(), self.header.metainfo().verity_tag());
        }
        Ok(())
    }

    pub fn header(&self) -> &ImageHeader {
        if let Err(err) = self.check_stale_header() {
            warn!("error reloading stale image header: {}", err);
        }
        &self.header
    }

    /// Return true if this RealmFS is sealed with user signing keys.
    pub fn is_user_realmfs(&self) -> bool {
        self.metainfo().channel() == Self::USER_KEYNAME
    }

    /// Return `true` if RealmFS is activated and some Realm is currently using
    /// it. A RealmFS which is in use cannot be deactivated.
    pub fn is_in_use(&self) -> bool {
        self.manager().realmfs_mountpoint_in_use(&self.mountpoint())
    }

    /// Deactivate this RealmFS image if currently activated, but not in use.
    /// Return `true` if deactivation occurs.
    pub fn deactivate(&self) {
        if !self.is_in_use() {
            self.mountpoint().deactivate();
        }
    }

    pub fn interactive_update(&self, scheme: Option<&str>) -> Result<()> {
        let mut update = Update::create(self)?;
        update.run_interactive_update(scheme)
    }

    // Return the public key for verifying the signature on this image
    fn public_key(&self) -> Result<PublicKey> {
        let pubkey = if self.metainfo().channel() == RealmFS::USER_KEYNAME {
            self.sealing_keys()?.public_key()
        } else {
            match self.header().public_key()? {
                Some(pubkey) => pubkey,
                None => bail!("No public key available for channel {}", self.metainfo().channel()),
            }
        };
        Ok(pubkey)
    }

    pub(super) fn verify_signature(&self) -> Result<()> {
        let pubkey = self.public_key()?;
        if !self.header().verify_signature(pubkey) {
            bail!("header signature verification failed on realmfs image '{}'", self.name());
        }
        info!("header signature verified on realmfs image '{}'", self.name());
        Ok(())
    }


    pub fn fork(&self, new_name: &str) -> Result<Self> {
        Self::validate_name(new_name)?;
        let new_path = self.path_with_filename(format!("{}-realmfs.img", new_name));
        if new_path.exists() {
            bail!("RealmFS image for name {} already exists", new_name);
        }

        let keys = match self.sealing_keys() {
            Ok(keys) => keys,
            Err(err) => bail!("Cannot fork realmfs image, no signing keys available: {}", err),
        };

        info!("forking RealmFS image '{}' to new name '{}'", self.name(), new_name);

        let forked = match self.fork_to_path(new_name, &new_path, keys) {
            Ok(forked) => forked,
            Err(err) => {
                if new_path.exists() {
                    let _ = fs::remove_file(&new_path);
                }
                bail!("Failed to fork RealmFS '{}' to '{}': {}", self.name, new_name, err);
            }
        };

        self.with_manager(|m| m.realmfs_added(&forked));
        Ok(forked)
    }

    // copy source image file to new name and install updated header
    fn fork_to_path(&self, new_name: &str, new_path: &Path, keys: KeyPair) -> Result<Self> {
        self.copy_image_file(new_path)?;
        let metainfo_bytes = self.fork_metainfo(new_name);
        let sig = keys.sign(&metainfo_bytes);
        let forked = Self::load_from_path(new_path)?;
        forked.header().update_metainfo(&metainfo_bytes, sig.to_bytes(), new_path)?;
        Ok(forked)
    }

    pub(super) fn copy_image_file(&self, to: &Path) -> Result<()> {
        if to.exists() {
            bail!("Cannot copy image file to {} because it already exists", to.display());
        }
        cmd!("/usr/bin/cp", "--reflink=auto {} {}", self.path.display(), to.display())?;
        Ok(())
    }

    fn fork_metainfo(&self, new_name: &str) -> Vec<u8> {
        // when creating a realmfs fork, only the name will change
        let metainfo = self.metainfo();
        Self::generate_metainfo(new_name, metainfo.nblocks(), metainfo.verity_salt(), metainfo.verity_root())
    }

    pub(super) fn generate_metainfo(name: &str, nblocks: usize, verity_salt: &str, verity_root: &str) -> Vec<u8> {
        let mut v = Vec::new();
        writeln!(v, "image-type = \"realmfs\"").unwrap();
        writeln!(v, "realmfs-name = \"{}\"", name).unwrap();
        writeln!(v, "nblocks = {}", nblocks).unwrap();
        writeln!(v, "channel = \"{}\"", Self::USER_KEYNAME).unwrap();
        writeln!(v, "verity-salt = \"{}\"", verity_salt).unwrap();
        writeln!(v, "verity-root = \"{}\"", verity_root).unwrap();
        v
    }

    // Return the length in blocks of the actual image file on disk
    pub fn file_nblocks(&self) -> Result<usize> {
        let meta = self.path.metadata()
            .map_err(context!("failed to read metadata from realmfs image file {:?}", self.path))?;
        let len = meta.len() as usize;
        if len % 4096 != 0 {
            bail!("realmfs image file '{}' has size which is not a multiple of block size", self.path.display());
        }
        let nblocks = len / 4096;
        if nblocks < (self.metainfo().nblocks() + 1) {
            bail!("realmfs image file '{}' has shorter length than nblocks field of image header", self.path.display());
        }
        Ok(nblocks)
    }

    pub fn has_sealing_keys(&self) -> bool {
        self.sealing_keys().is_ok()
    }

    pub fn sealing_keys(&self) -> Result<KeyPair> {
        KeyRing::get_kernel_keypair(Self::USER_KEYNAME)
    }

    pub fn auto_resize_size(&self) -> Option<ResizeSize> {
        ResizeSize::auto_resize_size(&self)
    }

    pub fn resize_grow_to(&self, size: ResizeSize) -> Result<()> {
        info!("Resizing to {} blocks", size.nblocks());
        let mut update = Update::create(self)?;
        update.grow_to(size);
        update.resize()
    }

    pub fn resize_grow_by(&self, size: ResizeSize) -> Result<()> {
        info!("Resizing to an increase of {} blocks", size.nblocks());
        let mut update = Update::create(self)?;
        update.grow_by(size);
        update.resize()
    }

    pub fn free_size_blocks(&self) -> Result<usize> {
        let sb = Superblock::load(self.path(), 4096)?;
        Ok(sb.free_block_count() as usize)
    }

    pub fn allocated_size_blocks(&self) -> Result<usize> {
        let meta = self.path().metadata()
            .map_err(context!("failed to read metadata from realmfs image file {:?}", self.path()))?;
        Ok(meta.blocks() as usize / 8)
    }

    /// Activate this RealmFS image if not yet activated.
    pub fn activate(&self) -> Result<()> {
        self.mountpoint().activate(self)
    }

    /// Return `true` if this RealmFS is 'activated'.
    ///
    /// A RealmFS is activated if the device for the image has been created and mounted.
    /// Sealed images create dm-verity devices in /dev/mapper and unsealed images create
    /// /dev/loop devices.
    pub fn is_activated(&self) -> bool {
        self.mountpoint().is_mounted()
    }
}

