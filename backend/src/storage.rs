//! Where this plugin keeps its state, and how it touches those files.
//!
//! The token key and the offline cache have the same requirements - a private
//! directory under an XDG root, files no other user can read, and reads that
//! cannot be redirected by something planted in the path - so those rules live
//! here once rather than in each module that stores something.
//!
//! They were written twice before, and the copies had already drifted: the key
//! directory refused a symlink and checked ownership, the cache directory did
//! neither, which is exactly the failure that made the key unwritable in the
//! first place. A third store added later inherits the checks by construction.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

/// The plugin id, used for the keyring service name and for both state
/// directories. Deliberately not `omarchy/...`: Omarchy symlinks
/// `~/.local/share/omarchy` to its root-owned system install, so a directory
/// nested under that name resolves into `/usr/share` and cannot be created.
pub const PLUGIN_ID: &str = "io.github.elevate08.ynab-glance";

/// Resolves an XDG directory, falling back to the conventional path under
/// `HOME` when the variable is unset or empty.
pub fn xdg_dir(var: &str, home_fallback: &str) -> Option<PathBuf> {
    if let Ok(root) = std::env::var(var) {
        if !root.is_empty() {
            return Some(PathBuf::from(root).join(PLUGIN_ID));
        }
    }
    let home = std::env::var("HOME").ok()?;
    Some(PathBuf::from(home).join(home_fallback).join(PLUGIN_ID))
}

/// Creates a directory only this user can enter, refusing anything suspicious
/// about the path it is asked to create.
///
/// A symlink is refused by name rather than followed: it may point at a
/// root-owned system path (the Omarchy case) or at somewhere another user can
/// read. A directory owned by someone else is refused outright.
pub fn ensure_private_dir(dir: &Path) -> std::io::Result<()> {
    if let Ok(meta) = fs::symlink_metadata(dir) {
        if meta.file_type().is_symlink() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("{} is a symlink; refusing to write through it", dir.display()),
            ));
        }
    }

    fs::create_dir_all(dir)?;

    let meta = fs::metadata(dir)?;
    // Safety: getuid() always succeeds and touches no memory.
    if meta.uid() != unsafe { libc::getuid() } {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            format!("{} is owned by uid {}, not by you", dir.display(), meta.uid()),
        ));
    }

    let mut perms = meta.permissions();
    perms.set_mode(0o700);
    let _ = fs::set_permissions(dir, perms);
    Ok(())
}

/// Opens a file that must be ours alone, refusing anything else.
///
/// `O_NOFOLLOW` makes the kernel reject a symlink planted in place of the
/// file, and `O_NONBLOCK` makes a planted FIFO fail instead of blocking the
/// widget on an open that waits forever for a writer. The metadata is then
/// taken from the descriptor rather than the path, so there is no window
/// between the check and the read in which another process could substitute a
/// different file.
pub fn open_private_file(path: &Path, max_bytes: u64) -> std::io::Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;

    let meta = file.metadata()?;
    if !meta.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "not a regular file",
        ));
    }
    if meta.len() > max_bytes {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("{} bytes exceeds the {} byte limit", meta.len(), max_bytes),
        ));
    }
    if meta.permissions().mode() & 0o077 != 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "file is group- or world-accessible",
        ));
    }
    Ok(file)
}

/// Reads a private file whole, up to `max_bytes`.
pub fn read_private_file(path: &Path, max_bytes: u64) -> std::io::Result<Vec<u8>> {
    let file = open_private_file(path, max_bytes)?;
    // The length is already known from the descriptor, so the buffer is sized
    // once instead of doubling its way up to the payload size.
    let capacity = file.metadata().map(|m| m.len()).unwrap_or(0).min(max_bytes);
    let mut bytes = Vec::with_capacity(capacity as usize);
    file.take(max_bytes).read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// A temp-file suffix unique to this attempt.
///
/// A pid alone is not unique enough. Two threads in one process collide on it
/// outright, and across processes a pid is reused: a stale temp file left by a
/// crash would make `create_new` fail with `EEXIST` forever, wedging writes
/// with no way out but manual deletion. Random bytes make each attempt its own
/// file, so a cleanup only ever removes the writer's own work.
fn temp_suffix() -> String {
    use std::fmt::Write as _;

    let mut bytes = [0u8; 8];
    // Not fatal: the pid still separates processes, the case that matters.
    let _ = getrandom::fill(&mut bytes);

    let mut out = String::with_capacity(24);
    let _ = write!(out, "{}.", std::process::id());
    for b in bytes {
        let _ = write!(out, "{:02x}", b);
    }
    out
}

/// How a staged file should be published.
pub enum Publish {
    /// First writer wins: `link` refuses to overwrite, so exactly one
    /// concurrent writer creates the file and the others find it already
    /// there. Right for a key, where a lost race would mean sealing data under
    /// a key that no longer exists on disk.
    KeepExisting,
    /// Last writer wins: `rename` replaces whatever is there. Right for a
    /// cache, where the newest copy is the wanted one.
    Replace,
}

/// Writes `bytes` to `path` atomically, 0600, never leaving a partial file.
///
/// The content is written to a private temp file in the same directory and
/// fsynced before being published, so the destination is never observed
/// half-written. Returns `AlreadyExists` under `KeepExisting` when another
/// writer got there first - a race the caller usually wants to treat as
/// success after re-reading.
pub fn write_private_file(path: &Path, bytes: &[u8], publish: Publish) -> std::io::Result<()> {
    let dir = path.parent().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "path has no directory")
    })?;
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "path has no name"))?;

    let tmp = dir.join(format!("{}.tmp.{}", name, temp_suffix()));

    let staged = (|| -> std::io::Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()
    })();

    if let Err(e) = staged {
        // Never leave a partial copy of whatever this was behind.
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }

    let published = match publish {
        Publish::KeepExisting => fs::hard_link(&tmp, path),
        Publish::Replace => fs::rename(&tmp, path),
    };

    if matches!(publish, Publish::KeepExisting) || published.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    published
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_base(name: &str) -> PathBuf {
        let base = std::env::temp_dir().join(format!("ynab-storage-{}-{}", std::process::id(), name));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        base
    }

    #[test]
    fn a_written_file_is_private_and_complete() {
        let base = temp_base("write");
        let target = base.join("thing");
        write_private_file(&target, b"payload", Publish::Replace).unwrap();

        assert_eq!(fs::read(&target).unwrap(), b"payload");
        assert_eq!(fs::metadata(&target).unwrap().permissions().mode() & 0o777, 0o600);
        // The temp file is not left lying around.
        assert_eq!(fs::read_dir(&base).unwrap().count(), 1);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn keep_existing_refuses_to_overwrite_and_replace_does_not() {
        let base = temp_base("publish");
        let target = base.join("thing");

        write_private_file(&target, b"first", Publish::KeepExisting).unwrap();
        let second = write_private_file(&target, b"second", Publish::KeepExisting);
        assert_eq!(second.unwrap_err().kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(&target).unwrap(), b"first");
        // A refused publish still cleans up after itself.
        assert_eq!(fs::read_dir(&base).unwrap().count(), 1);

        write_private_file(&target, b"third", Publish::Replace).unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"third");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn a_symlinked_file_is_refused() {
        let base = temp_base("symlink");
        let elsewhere = base.join("elsewhere");
        fs::write(&elsewhere, b"secret").unwrap();
        fs::set_permissions(&elsewhere, fs::Permissions::from_mode(0o600)).unwrap();

        let link = base.join("link");
        std::os::unix::fs::symlink(&elsewhere, &link).unwrap();

        assert!(read_private_file(&link, 1024).is_err());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn a_group_readable_file_is_refused() {
        let base = temp_base("perms");
        let target = base.join("thing");
        fs::write(&target, b"payload").unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o640)).unwrap();

        assert!(read_private_file(&target, 1024).is_err());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn an_oversized_file_is_refused_before_it_is_read() {
        let base = temp_base("size");
        let target = base.join("thing");
        fs::write(&target, vec![0u8; 128]).unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();

        assert!(read_private_file(&target, 64).is_err());
        assert_eq!(read_private_file(&target, 128).unwrap().len(), 128);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn a_symlinked_directory_is_refused_by_name() {
        let base = temp_base("dir");
        let real = base.join("real");
        fs::create_dir_all(&real).unwrap();
        let link = base.join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        let err = ensure_private_dir(&link).unwrap_err();
        assert!(err.to_string().contains("symlink"), "unhelpful: {}", err);

        let fresh = base.join("fresh");
        ensure_private_dir(&fresh).unwrap();
        assert_eq!(fs::metadata(&fresh).unwrap().permissions().mode() & 0o777, 0o700);
        let _ = fs::remove_dir_all(&base);
    }
}
