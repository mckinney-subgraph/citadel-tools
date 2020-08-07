use std::ffi::OsStr;
use std::fmt;
use std::fs::{self, DirEntry};
use std::path::{PathBuf, Path};

use crate::{Result, RealmFS, CommandLine, ImageHeader, util};
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

    fn path_is_mountpoint(path: &Path) -> bool {
        // path begins with /run/citadel/realmfs
        path.starts_with(RealmFS::RUN_DIRECTORY) &&
            // path has a filename with extension 'mountpoint'
            path.extension().map_or(false, |ext| ext == "mountpoint") &&
            // path has a filename and that name starts with "realmfs-"
            path.file_name().and_then(OsStr::to_str).map_or(false, |name| name.starts_with("realmfs-")) &&
            // path filename can be parsed correctly
            Self::parse_filename(path).is_some()
    }

    /// Read `RealmFS::RUN_DIRECTORY` to collect all current mountpoints
    /// and return them.
    pub fn all_mountpoints() -> Result<Vec<Mountpoint>> {
        let mut v = Vec::new();
        util::read_directory(RealmFS::RUN_DIRECTORY, |dent| {
            let path = dent.path();
            if Self::path_is_mountpoint(&path) {
                v.push(Mountpoint(path));
            }
            Ok(())
        })?;
        Ok(v)
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

    pub fn is_mounted(&self) -> bool {
        // test for an arbitrary expected directory
        self.path().join("etc").exists()
    }

    fn mount<P: AsRef<Path>>(&self, source: P) -> Result<()> {
        let source = source.as_ref();
        cmd!(Self::MOUNT, "-oro {} {}", source.display(), self.path().display())
            .map_err(context!("failed to mount {:?} to {:?}", source, self.path()))
    }

    pub fn activate(&self, realmfs: &RealmFS) -> Result<()> {
        if self.is_mounted() {
            return Ok(())
        }

        util::create_dir(self.path())?;

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
        realmfs.header().write_header_to(realmfs.path())?;
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
        info!("realmfs: {} tag: {}", self.realmfs(), self.tag());
        format!("verity-realmfs-{}-{}", self.realmfs(), self.tag())
    }

    /// Full `&Path` of mountpoint.
    pub fn path(&self) -> &Path {
        self.0.as_path()
    }

    /// Name of RealmFS extracted from structure of directory filename.
    pub fn realmfs(&self) -> &str {
        self.filename_fields().0
    }

    /// Tag field extracted from structure of directory filename.
    pub fn tag(&self) -> &str {
        self.filename_fields().1
    }

    /// Return `true` if this instance is a `&Path` in `RealmFS::RUN_DIRECTORY` and
    /// the filename has the expected structure.
    pub fn is_valid(&self) -> bool {
        Self::path_is_mountpoint(self.path())
    }

    fn filename_fields(&self) -> (&str, &str) {
        Self::parse_filename(self.path())
            .expect(&format!("failed to parse mountpoint filename {:?} into fields", self.path()))
    }

    fn parse_filename(path: &Path) -> Option<(&str,&str)> {
        let fname = path.file_name()
            .and_then(OsStr::to_str)
            .map(|s|
                s.trim_end_matches(".mountpoint")
                    .trim_start_matches("realmfs-")
            )?;
        let idx = fname.rfind("-")?;
        let (first,last) = fname.split_at(idx);
        Some((first, &last[1..]))
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
