use std::ffi::OsStr;
use std::fmt;
use std::fs::{self, DirEntry};
use std::path::{PathBuf, Path};

use crate::{Result, RealmFS, CommandLine, ImageHeader};
use crate::verity::Verity;


/// Represents the path at which a RealmFS is mounted and manages RealmFS activation and
/// deactivation.
///
/// Activation of a RealmFS involves:
///
/// 1. create mountpoint directory
/// 2. create loop and dm-verity device for image file
/// 3. Mount dm-verity device at mountpoint directory
///
/// Deactivation reverses these steps.
///
#[derive(Clone,Eq,PartialEq,Hash,Debug)]
pub struct Mountpoint(PathBuf);

impl Mountpoint {
    const MOUNT: &'static str = "/usr/bin/mount";
    const UMOUNT: &'static str = "/usr/bin/umount";

    /// Read `RealmFS::RUN_DIRECTORY` to collect all current mountpoints
    /// and return them.
    pub fn all_mountpoints() -> Result<Vec<Mountpoint>> {
        let all = fs::read_dir(RealmFS::RUN_DIRECTORY)?
            .flat_map(|e| e.ok())
            .map(Into::into)
            .filter(Mountpoint::is_valid)
            .collect();
        Ok(all)
    }

    /// Build a new `Mountpoint` from the provided realmfs `name` and `tag`.
    ///
    /// The directory name of the mountpoint will have the structure:
    ///
    ///     realmfs-$name-$tag.mountpoint
    ///
    pub fn new(name: &str, tag: &str) -> Self {
        let filename = format!("realmfs-{}-{}.mountpoint", name, tag);
        Mountpoint(Path::new(RealmFS::RUN_DIRECTORY).join(filename))
    }

    pub fn exists(&self) -> bool {
        self.0.exists()
    }

    fn create_dir(&self) -> Result<()> {
        fs::create_dir_all(self.path())?;
        Ok(())
    }

    pub fn is_mounted(&self) -> bool {
        // test for an arbitrary expected directory
        self.path().join("etc").exists()
    }

    fn mount<P: AsRef<Path>>(&self, source: P) -> Result<()> {
        cmd!(Self::MOUNT, "-oro {} {}",
            source.as_ref().display(),
            self.path().display()
        )
    }

    pub fn activate(&self, realmfs: &RealmFS) -> Result<()> {
        if self.is_mounted() {
            return Ok(())
        }

        if !self.exists() {
            self.create_dir()?;
        }
        let verity_path = self.verity_device_path();
        if verity_path.exists() {
            warn!("dm-verity device {:?} already exists which was not expected", verity_path);
        } else if let Err(err) = self.setup_verity(realmfs) {
            let _ = fs::remove_dir(self.path());
            return Err(err);
        }

        if let Err(err) = self.mount(verity_path) {
            self.deactivate();
            Err(err)
        } else {
            Ok(())
        }
    }

    fn setup_verity(&self, realmfs: &RealmFS) -> Result<()> {
        if !CommandLine::nosignatures() {
            realmfs.verify_signature()?;
        }
        if !realmfs.header().has_flag(ImageHeader::FLAG_HASH_TREE) {
            self.generate_verity(realmfs)?;
        }
        let verity = Verity::new(realmfs.path())?;
        verity.setup()?;
        Ok(())
    }

    fn generate_verity(&self, realmfs: &RealmFS) -> Result<()> {
        info!("Generating verity hash tree");
        let verity = Verity::new(realmfs.path())?;
        verity.generate_image_hashtree()?;
        realmfs.header().set_flag(ImageHeader::FLAG_HASH_TREE);
        realmfs.header().write_header_to(self.path())?;
        info!("Done generating verity hash tree");
        Ok(())
    }

    /// Deactivate this mountpoint by unmounting it and removing the directory.
    pub fn deactivate(&self) {
        if !self.exists() {
            return;
        }
        info!("Unmounting {} and removing directory", self);

        // 1. Unmount directory
        if self.is_mounted() {
            if let Err(err) = cmd!(Self::UMOUNT, "{}", self) {
                warn!("Failed to unmount directory {}: {}", self, err);
            }
        }

        // 2. Remove dm-verity device
        let verity = self.verity_device_path();
        if verity.exists() {
            if let Err(err) = Verity::close_device(self.verity_device().as_str()) {
                warn!("Failed to remove dm-verity device {:?}: {}", verity, err);
            }
        }

        // 3. Remove directory
        if let Err(err) = fs::remove_dir(self.path()) {
            warn!("Failed to remove mountpoint directory {}: {}", self, err);
        }

    }

    fn verity_device_path(&self) -> PathBuf {
        Path::new("/dev/mapper")
            .join(self.verity_device())
    }

    // Return the name of the dm-verity device associated with this mountpoint
    pub fn verity_device(&self) -> String {
        format!("verity-realmfs-{}-{}", self.realmfs(), self.tag())
    }

    /// Full `&Path` of mountpoint.
    pub fn path(&self) -> &Path {
        self.0.as_path()
    }

    /// Name of RealmFS extracted from structure of directory filename.
    pub fn realmfs(&self) -> &str {
        self.field(1)
    }

    /// Tag field extracted from structure of directory filename.
    pub fn tag(&self) -> &str {
        self.field(2)
    }

    fn field(&self, n: usize) -> &str {
        Self::filename_fields(self.path())
            .and_then(|mut fields| fields.nth(n))
            .unwrap_or_else(|| panic!("Failed to access field {} of mountpoint {}", n, self))
    }

    /// Return `true` if this instance is a `&Path` in `RealmFS::RUN_DIRECTORY` and
    /// the filename has the expected structure.
    pub fn is_valid(&self) -> bool {
        self.path().starts_with(RealmFS::RUN_DIRECTORY) && self.has_valid_extention() &&
            Self::filename_fields(self.path()).map(|it| it.count() == 3).unwrap_or(false)
    }

    fn has_valid_extention(&self) -> bool {
        self.path().extension().map_or(false, |e| e == "mountpoint")
    }

    fn filename_fields(path: &Path) -> Option<impl Iterator<Item=&str>> {
        Self::filename(path).map(|name| name.split('-'))
    }

    fn filename(path: &Path) -> Option<&str> {
        path.file_name()
            .and_then(OsStr::to_str)
            .map(|s| s.trim_end_matches(".mountpoint"))
    }
}

impl fmt::Display for Mountpoint {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0.to_str().unwrap())
    }
}

impl From<&Path> for Mountpoint {
    fn from(p: &Path) -> Self {
        Mountpoint(p.to_path_buf())
    }
}

impl From<PathBuf> for Mountpoint {
    fn from(p: PathBuf) -> Self {
        Mountpoint(p)
    }
}

impl From<DirEntry> for Mountpoint {
    fn from(entry: DirEntry) -> Self {
        Mountpoint(entry.path())
    }
}
