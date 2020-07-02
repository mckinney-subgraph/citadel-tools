use std::fs::{self,File,OpenOptions};
use std::io::{Error,ErrorKind};
use std::os::unix::io::AsRawFd;
use std::path::{Path,PathBuf};

use crate::{Result, util};

///
/// Create a lockfile and acquire an exclusive lock with flock(2)
///
/// The lock can either be acquired by blocking until available or
/// by failing immediately if the lock is already held.
///
/// The lock is released and the lockfile is removed when `FileLock`
/// instance is dropped.
///
pub struct FileLock {
    file: File,
    path: PathBuf,
}

impl FileLock {

    pub fn nonblocking_acquire<P: AsRef<Path>>(path: P) -> Result<Option<Self>> {
        let file = Self::open_lockfile(path.as_ref())?;
        let flock = FileLock {
            file,
            path: path.as_ref().into(),
        };

        if flock.lock(false)? {
            Ok(Some(flock))
        } else {
            Ok(None)
        }
    }

    pub fn acquire<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let file = Self::open_lockfile(&path)?;
        let flock = FileLock { file, path };
        flock.lock(true)?;
        Ok(flock)
    }

    fn open_lockfile(path: &Path) -> Result<File> {
        if let Some(parent) = path.parent() {
            util::create_dir(parent)?;
        }

        // Make a few attempts just in case we try to open lockfile
        // at exact moment another process is releasing and deleting
        // file.
        for _ in 0..3 {
            if let Some(file) = Self::try_create_lockfile(path)? {
                return Ok(file);
            }
            if let Some(file) = Self::try_open_lockfile(path)? {
                return Ok(file);
            }
        }
        bail!("unable to open lockfile {:?}", path)
    }

    fn try_create_lockfile(path: &Path) -> Result<Option<File>> {
        match OpenOptions::new().write(true).create_new(true).open(path) {
            Ok(file) => Ok(Some(file)),
            Err(ref e) if e.kind() == ErrorKind::AlreadyExists => Ok(None),
            Err(e) => bail!("failed to create lockfile {:?}: {}", path, e),
        }
    }

    fn try_open_lockfile(path: &Path) -> Result<Option<File>> {
        match File::open(path) {
            Ok(file) => Ok(Some(file)),
            Err(ref e) if e.kind() == ErrorKind::NotFound => Ok(None),
            Err(e) => bail!("failed to open lockfile {:?}: {}", path, e),
        }
    }

    fn unlock(&self) -> Result<()> {
        self.flock(libc::LOCK_UN, true)?;
        Ok(())
    }

    fn lock(&self, block: bool) -> Result<bool> {
        if block {
            self.flock(libc::LOCK_EX, true)
        } else {
            self.flock(libc::LOCK_EX | libc::LOCK_NB, false)
        }
    }

    fn flock(&self, flag: libc::c_int, block: bool) -> Result<bool> {
        if unsafe { libc::flock(self.file.as_raw_fd(), flag) } < 0 {
            let errno = Self::last_errno();
            if !block && errno == libc::EWOULDBLOCK {
                return Ok(false);
            }
            bail!("error calling flock(): {}", Error::from_raw_os_error(errno));
        }
        Ok(true)
    }

    pub fn last_errno() -> i32 {
        unsafe { *libc::__errno_location() }
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
        let _ = self.unlock();
    }
}
