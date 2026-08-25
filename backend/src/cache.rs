use std::fs;
use std::path::PathBuf;

use zeroize::Zeroizing;

use crate::crypto;
use crate::storage::{self, Publish};
use crate::models::PluginOverviewResponse;

const CACHE_FILE_NAME: &str = "overview_cache.enc";
/// The plaintext cache written by earlier versions, in the directory those
/// versions used. Never read - only deleted, because it is a readable copy of
/// the user's budget that must not outlive the upgrade.
const LEGACY_CACHE_DIR_NAME: &str = "omarchy/ynab-glance";
const LEGACY_CACHE_FILE_NAME: &str = "overview_cache.json";
/// The cache is our own writing; a file larger than this is a sign of tampering
/// or corruption, and reading it unbounded would let a local attacker exhaust
/// memory by pointing the path at something huge.
const MAX_CACHE_BYTES: u64 = 8 * 1024 * 1024;

fn get_cache_dir() -> Option<PathBuf> {
    storage::xdg_dir("XDG_CACHE_HOME", ".cache")
}

fn legacy_cache_dir() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let root = match std::env::var("XDG_CACHE_HOME") {
        Ok(r) if !r.is_empty() => PathBuf::from(r),
        _ => PathBuf::from(home).join(".cache"),
    };
    Some(root.join(LEGACY_CACHE_DIR_NAME))
}

/// Deletes the pre-move cache, and the directory it sat in if nothing else is
/// using it.
///
/// The old file is a readable copy of the user's budget. Leaving it behind
/// would quietly break the promise that revoking the token erases the cached
/// data, since `auth clear` would only ever look at the new path.
fn remove_legacy_cache() {
    let dir = match legacy_cache_dir() {
        Some(d) => d,
        None => return,
    };
    // One stat in the steady state instead of an unlink that returns ENOENT on
    // every fetch forever: this migration matters exactly once.
    if !dir.exists() {
        return;
    }
    let _ = fs::remove_file(dir.join(LEGACY_CACHE_FILE_NAME));
    // Only succeeds while the directory is empty, which is what we want: this
    // must never remove another plugin's cache.
    let _ = fs::remove_dir(&dir);
}

/// Reads the cached overview payload if it is present and trustworthy.
///
/// The cache holds the user's financial data, so a file that is not a regular
/// file (a symlink or FIFO planted by another local process), is group- or
/// world-accessible, or is implausibly large is discarded rather than read.
pub fn read_cache() -> Option<PluginOverviewResponse> {
    let cache_file = get_cache_dir()?.join(CACHE_FILE_NAME);

    // Refuses a symlink or FIFO planted in place of the cache, and checks the
    // file through its descriptor rather than its path. See `open_private_file`.
    let sealed = storage::read_private_file(&cache_file, MAX_CACHE_BYTES).ok()?;

    // A cache we cannot open is a cache we will never open again: the key is
    // gone, or another process wrote something else here. Either way it is
    // dead weight holding budget data, so delete it and refetch.
    let opened = match crypto::open(&sealed) {
        Ok(plaintext) => plaintext,
        Err(_) => {
            let _ = fs::remove_file(&cache_file);
            return None;
        }
    };

    serde_json::from_str(&opened).ok()
}

/// Securely writes overview payload with strict 0600 permissions and atomic rename
pub fn write_cache(payload: &PluginOverviewResponse) -> Result<(), std::io::Error> {
    let cache_dir = get_cache_dir().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Could not determine cache directory",
        )
    })?;
    storage::ensure_private_dir(&cache_dir)?;

    let serialized = Zeroizing::new(
        serde_json::to_string(payload)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?,
    );

    // Sealed with the same per-install key as the token. The panel's data is
    // as personal as the credential that fetches it, and AEAD also means a
    // cache another process rewrote fails the tag instead of being displayed
    // as though it came from YNAB.
    let sealed = crypto::seal(&serialized)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::PermissionDenied, e.to_string()))?;

    // Refuse to write what `read_cache` would refuse to read. The entity caps
    // in the aggregator allow a payload larger than this ceiling, and without
    // this check that case writes a file that every later run silently
    // discards - an offline cache that is permanently dead with nothing
    // anywhere saying so.
    if sealed.len() as u64 > MAX_CACHE_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "sealed payload is {} bytes, over the {} byte cache limit",
                sealed.len(),
                MAX_CACHE_BYTES
            ),
        ));
    }

    storage::write_private_file(&cache_dir.join(CACHE_FILE_NAME), &sealed, Publish::Replace)?;

    // The new cache is in place, so a pre-move copy is now just an orphaned
    // readable snapshot of the budget. Sweep it on the way past.
    remove_legacy_cache();
    Ok(())
}

/// Removes all locally cached financial data.
///
/// Revoking the token must also erase what was already fetched with it,
/// otherwise "disconnect" leaves a readable copy of the budget on disk.
pub fn purge_cache() -> Result<(), std::io::Error> {
    remove_legacy_cache();

    let cache_dir = match get_cache_dir() {
        Some(d) => d,
        None => return Ok(()),
    };

    match fs::remove_file(cache_dir.join(CACHE_FILE_NAME)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{symlink, PermissionsExt};

    /// Points both XDG directories at one temporary tree, so a test gets its
    /// own cache *and* its own encryption key. Holds the shared environment
    /// lock for the duration: both variables are process-wide.
    fn with_temp_dirs<T>(name: &str, body: impl FnOnce() -> T) -> T {
        let _guard = crate::test_env::lock();

        let base = std::env::temp_dir().join(format!("ynab-cache-test-{}-{}", std::process::id(), name));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        std::env::set_var("XDG_CACHE_HOME", base.join("cache"));
        std::env::set_var("XDG_DATA_HOME", base.join("data"));
        let out = body();
        std::env::remove_var("XDG_CACHE_HOME");
        std::env::remove_var("XDG_DATA_HOME");
        let _ = fs::remove_dir_all(&base);
        out
    }

    /// Built by deserialization rather than a struct literal: every field
    /// already carries `#[serde(default)]` for the cache read path, so this
    /// stays valid as the payload grows.
    fn sample_payload() -> PluginOverviewResponse {
        serde_json::from_str(
            r#"{"ok":true,"authenticated":true,"active_budget_name":"Household"}"#,
        )
        .unwrap()
    }

    /// The whole point of the change: what lands on disk must not be the
    /// user's budget in readable form.
    #[test]
    fn the_cache_on_disk_is_sealed() {
        with_temp_dirs("sealed", || {
            write_cache(&sample_payload()).unwrap();

            let path = get_cache_dir().unwrap().join(CACHE_FILE_NAME);
            let raw = fs::read(&path).unwrap();
            assert!(crypto::is_sealed(&raw), "cache is not an envelope");
            assert!(
                !raw.windows(9).any(|w| w == b"Household"),
                "budget data is readable in the cache file"
            );
            assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);

            let read_back = read_cache().expect("sealed cache should round-trip");
            assert_eq!(read_back.active_budget_name.as_deref(), Some("Household"));
        });
    }

    /// A cache another process rewrote fails the tag rather than being shown
    /// in the panel as though YNAB had sent it.
    #[test]
    fn a_tampered_cache_is_discarded_not_displayed() {
        with_temp_dirs("tampered", || {
            write_cache(&sample_payload()).unwrap();

            let path = get_cache_dir().unwrap().join(CACHE_FILE_NAME);
            let mut raw = fs::read(&path).unwrap();
            let last = raw.len() - 1;
            raw[last] ^= 0x01;
            fs::write(&path, &raw).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();

            assert!(read_cache().is_none(), "a forged cache was accepted");
            assert!(!path.exists(), "an unopenable cache should be deleted");
        });
    }

    /// An upgrade must not leave the old readable JSON behind: it is a copy of
    /// the user's budget that `auth clear` would no longer know to look for.
    #[test]
    fn the_plaintext_cache_is_swept_on_write_and_on_purge() {
        with_temp_dirs("plaintext", || {
            let old_dir = legacy_cache_dir().unwrap();
            let old_plain = old_dir.join(LEGACY_CACHE_FILE_NAME);

            fs::create_dir_all(&old_dir).unwrap();
            fs::write(&old_plain, "{}").unwrap();
            write_cache(&sample_payload()).unwrap();
            assert!(!old_plain.exists(), "writing the new cache left the old one");
            assert!(!old_dir.exists(), "the emptied legacy directory was left behind");

            fs::create_dir_all(&old_dir).unwrap();
            fs::write(&old_plain, "{}").unwrap();
            purge_cache().unwrap();
            assert!(!old_plain.exists(), "purge left a plaintext cache behind");
        });
    }

    /// A local process that plants a symlink where the cache belongs must not
    /// get the helper to read the target on its behalf.
    #[test]
    fn a_symlinked_cache_is_refused() {
        with_temp_dirs("symlink", || {
            let cache_dir = get_cache_dir().unwrap();
            fs::create_dir_all(&cache_dir).unwrap();

            let elsewhere = cache_dir.join("elsewhere.bin");
            fs::write(&elsewhere, "{}").unwrap();
            fs::set_permissions(&elsewhere, fs::Permissions::from_mode(0o600)).unwrap();
            symlink(&elsewhere, cache_dir.join(CACHE_FILE_NAME)).unwrap();

            assert!(read_cache().is_none(), "read_cache followed a symlink");
        });
    }
}
