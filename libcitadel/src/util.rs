use std::path::{Path,PathBuf};
use std::process::{Command,Stdio};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::os::unix::fs as unixfs;
use std::env;
use std::fs::{self, File, DirEntry};
use std::ffi::CString;
use std::io::{self, Seek, Read, BufReader, SeekFrom};

use walkdir::WalkDir;
use libc;

use crate::{Result, util};

pub fn is_valid_name(name: &str, maxsize: usize) -> bool {
    name.len() <= maxsize &&
        // Also false on empty string
        is_first_char_alphabetic(name) &&
        name.chars().all(is_alphanum_or_dash)
}

fn is_alphanum_or_dash(c: char) -> bool {
    is_ascii(c) && (c.is_alphanumeric() || c == '-')
}

fn is_ascii(c: char) -> bool {
    c as u32 <= 0x7F
}

pub fn is_first_char_alphabetic(s: &str) -> bool {
    if let Some(c) = s.chars().next() {
        return is_ascii(c) && c.is_alphabetic()
    }
    false
}

fn search_path(filename: &str) -> Result<PathBuf> {
    let path_var = env::var("PATH").unwrap_or("".into());
    for mut path in env::split_paths(&path_var) {
        path.push(filename);
        if path.exists() {
            return Ok(path);
        }
    }
    bail!("could not find {} in $PATH", filename)
}

pub fn ensure_command_exists(cmd: &str) -> Result<()> {
    let path = Path::new(cmd);
    if !path.is_absolute() {
        search_path(cmd)?;
        return Ok(())
    } else if path.exists() {
        return Ok(())
    }
    bail!("cannot execute '{}': command does not exist", cmd)
}

pub fn sha256<P: AsRef<Path>>(path: P) -> Result<String> {
    let path = path.as_ref();
    let output = cmd_with_output!("/usr/bin/sha256sum", "{}", path.display())
        .map_err(context!("failed to calculate sha256 on {:?}", path))?;

    let v: Vec<&str> = output.split_whitespace().collect();
    Ok(v[0].trim().to_owned())
}

#[derive(Copy,Clone)]
pub enum FileRange {
    All,
    Offset(usize),
    Range{offset: usize, len: usize},
}

fn ranged_reader<P: AsRef<Path>>(path: P, range: FileRange) -> Result<Box<dyn Read>> {
    let path = path.as_ref();
    let mut f = File::open(path)
        .map_err(context!("error opening input file {:?}", path))?;
    let offset = match range {
        FileRange::All => 0,
        FileRange::Offset(n) => n,
        FileRange::Range {offset, .. } => offset,
    };
    if offset > 0 {
        f.seek(SeekFrom::Start(offset as u64))
            .map_err(context!("error seeking to offset {} in input file {:?}", offset, path))?;
    }
    let r = BufReader::new(f);
    if let FileRange::Range {len, ..} = range {
        Ok(Box::new(r.take(len as u64)))
    } else {
        Ok(Box::new(r))
    }
}

///
/// Execute a command, pipe the contents of a file to stdin, return the output as a `String`
///
pub fn exec_cmdline_pipe_input<S,P>(cmd_path: &str, args: S, input: P, range: FileRange) -> Result<String>
    where S: AsRef<str>, P: AsRef<Path>
{
    let mut r = ranged_reader(input.as_ref(), range)?;
    ensure_command_exists(cmd_path)?;
    let args: Vec<&str> = args.as_ref().split_whitespace().collect::<Vec<_>>();
    let mut child = Command::new(cmd_path)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(context!("unable to execute {}", cmd_path))?;

    let stdin = child.stdin.as_mut().unwrap();
    io::copy(&mut r, stdin)
        .map_err(context!("error copying input to stdin"))?;
    let output = child.wait_with_output()
        .map_err(context!("error waiting for command {} to exit", cmd_path))?;
    Ok(String::from_utf8(output.stdout).unwrap().trim().to_owned())
}

pub fn xz_compress<P: AsRef<Path>>(path: P) -> Result<()> {
    let path = path.as_ref();
    cmd!("/usr/bin/xz", "-T0 {}", path.display())
        .map_err(context!("failed to compress {:?}", path))
}

pub fn xz_decompress<P: AsRef<Path>>(path: P) -> Result<()> {
    let path = path.as_ref();
    cmd!("/usr/bin/xz", "-d {}", path.display())
        .map_err(context!("failed to decompress {:?}", path))
}

pub fn mount<P: AsRef<Path>>(source: impl AsRef<str>, target: P, options: Option<&str>) -> Result<()> {
    let source = source.as_ref();
    let target = target.as_ref();
    if let Some(options) = options {
        cmd!("/usr/bin/mount", "{} {} {}", options, source, target.display())
    } else {
        cmd!("/usr/bin/mount", "{} {}", source, target.display())
    }.map_err(context!("failed to mount {} to {:?}", source, target))
}

pub fn umount<P: AsRef<Path>>(path: P) -> Result<()> {
    let path = path.as_ref();
    cmd!("/usr/bin/umount", "{}", path.display())
        .map_err(context!("failed to unmount {:?}", path))
}

pub fn chown_user<P: AsRef<Path>>(path: P) -> Result<()> {
    chown(path.as_ref(), 1000, 1000)
}

pub fn chown(path: &Path, uid: u32, gid: u32) -> Result<()> {
    let cstr = CString::new(path.as_os_str().as_bytes())
        .expect("path contains null byte");
    unsafe {
        if libc::chown(cstr.as_ptr(), uid, gid) == -1 {
            let err = io::Error::last_os_error();
            bail!("failed to chown({},{}) {:?}: {}", uid, gid, path, err);
        }
    }
    Ok(())
}

/// Rename or move file at `from` to file path `to`
///
/// A wrapper around `fs::rename()` which on failure returns an error indicating the source and
/// destination paths.
///
pub fn rename(from: impl AsRef<Path>, to: impl AsRef<Path>) -> Result<()> {
    let from = from.as_ref();
    let to = to.as_ref();
    fs::rename(from, to)
        .map_err(context!("error renaming {:?} to {:?}", from, to))
}

/// Create a symlink at path `dst` which points to `src`
///
/// A wrapper around `fs::symlink()` which on failure returns an error indicating the source and
/// destination paths.
///
pub fn symlink(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> Result<()> {
    let src = src.as_ref();
    let dst = dst.as_ref();
    unixfs::symlink(src, dst)
        .map_err(context!("failed to create symlink {:?} to {:?}", dst, src))
}

/// Read directory `dir` and call closure `f` on each `DirEntry`
pub fn read_directory<F>(dir: impl AsRef<Path>, mut f: F) -> Result<()>
where
    F: FnMut(&DirEntry) -> Result<()>
{
    let dir = dir.as_ref();
    let entries = fs::read_dir(dir)
        .map_err(context!("failed to read directory {:?}", dir))?;
    for dent in entries {
        let dent = dent.map_err(context!("error reading entry from directory {:?}", dir))?;
        f(&dent)?;
    }
    Ok(())
}

/// Remove file at `path` if it exists.
///
/// A wrapper around `fs::remove_file()` which on failure returns an error indicating the path of
/// the file which failed to be removed.
///
pub fn remove_file(path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();
    if path.exists() {
        fs::remove_file(path)
            .map_err(context!("failed to remove file {:?}", path))?;
    }
    Ok(())
}

/// Create directory `path` if it does not already exist.
///
/// A wrapper around `fs::create_dir_all()` which on failure returns an error indicating the path
/// of the directory which failed to be created.
///
pub fn create_dir(path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();
    if !path.exists() {
        fs::create_dir_all(path)
            .map_err(context!("failed to create directory {:?}", path))?;
    }
    Ok(())
}

/// Write `contents` to file `path`
///
/// A wrapper around `fs::write()` which on failure returns an error indicating the path
/// of the file which failed to be written.
///
pub fn write_file(path: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> Result<()> {
    let path = path.as_ref();
    fs::write(path, contents)
        .map_err(context!("failed to write to file {:?}", path))
}

/// Read content of file `path` into a `String`
///
/// A wrapper around `fs::read_to_string()` which on failure returns an error indicating the path
/// of the file which failed to be read.
///
pub fn read_to_string(path: impl AsRef<Path>) -> Result<String> {
    let path = path.as_ref();
    fs::read_to_string(path)
        .map_err(context!("failed to read file {:?}", path))
}

/// Copy file at path `from` to a new file at path `to`
///
/// A wrapper around `fs::copy()` which on failure returns an error indicating the source and
/// destination paths.
///
pub fn copy_file(from: impl AsRef<Path>, to: impl AsRef<Path>) -> Result<()> {
    let from = from.as_ref();
    let to = to.as_ref();
    fs::copy(from, to)
        .map_err(context!("failed to copy file {:?} to {:?}", from, to))?;
    Ok(())
}

fn copy_path(from: &Path, to: &Path, chown_to: Option<(u32,u32)>) -> Result<()> {
    if to.exists() {
        bail!("destination path {} already exists which is not expected", to.display());
    }

    let meta = from.metadata()
        .map_err(context!("failed to read metadata from source file {:?}", from))?;

    if from.is_dir() {
        util::create_dir(to)?;
    } else {
        util::copy_file(&from, &to)?;
    }

    if let Some((uid,gid)) = chown_to {
        chown(to, uid, gid)?;
    } else {
        chown(to, meta.uid(), meta.gid())?;
    }
    Ok(())

}

pub fn copy_tree(from_base: &Path, to_base: &Path) -> Result<()> {
    _copy_tree(from_base, to_base, None)
}

pub fn copy_tree_with_chown(from_base: &Path, to_base: &Path, chown_to: (u32,u32)) -> Result<()> {
    _copy_tree(from_base, to_base, Some(chown_to))
}

fn _copy_tree(from_base: &Path, to_base: &Path, chown_to: Option<(u32,u32)>) -> Result<()> {
    for entry in WalkDir::new(from_base) {
        let entry = entry.map_err(|e| format_err!("Error walking directory tree: {}", e))?;
        let path = entry.path();
        let suffix = path.strip_prefix(from_base)
            .map_err(|_| format_err!("Failed to strip prefix from {:?}", path))?;
        let to = to_base.join(suffix);
        if &to != to_base {
            copy_path(path, &to, chown_to)
                .map_err(context!("failed to copy {:?} to {:?}", path, to))?;
        }
    }
    Ok(())
}

pub fn chown_tree(base: &Path, chown_to: (u32,u32), include_base: bool) -> Result<()> {
    for entry in WalkDir::new(base) {
        let entry = entry.map_err(|e| format_err!("Error reading directory entry: {}", e))?;
        if entry.path() != base || include_base {
            chown(entry.path(), chown_to.0, chown_to.1)
                .map_err(context!("failed to chown {:?}", entry.path()))?;
        }
    }
    Ok(())
}

pub fn is_euid_root() -> bool {
    unsafe {
        libc::geteuid() == 0
    }
}
