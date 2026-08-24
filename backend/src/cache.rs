use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::PathBuf;

use crate::models::PluginOverviewResponse;

const CACHE_DIR_NAME: &str = "omarchy/ynab-glance";
const CACHE_FILE_NAME: &str = "overview_cache.json";

fn get_cache_dir() -> Option<PathBuf> {
    if let Ok(xdg_cache) = std::env::var("XDG_CACHE_HOME") {
        if !xdg_cache.is_empty() {
            return Some(PathBuf::from(xdg_cache).join(CACHE_DIR_NAME));
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        return Some(PathBuf::from(home).join(".cache").join(CACHE_DIR_NAME));
    }
    None
}

/// Securely reads cached overview payload if available
pub fn read_cache() -> Option<PluginOverviewResponse> {
    let cache_dir = get_cache_dir()?;
    let cache_file = cache_dir.join(CACHE_FILE_NAME);

    if !cache_file.exists() {
        return None;
    }

    let mut file = File::open(&cache_file).ok()?;
    let mut contents = String::new();
    file.read_to_string(&mut contents).ok()?;

    serde_json::from_str(&contents).ok()
}

/// Securely writes overview payload with strict 0600 permissions and atomic rename
pub fn write_cache(payload: &PluginOverviewResponse) -> Result<(), std::io::Error> {
    let cache_dir = match get_cache_dir() {
        Some(d) => d,
        None => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Could not determine cache directory",
            ))
        }
    };

    // Ensure directory exists with 0700 permissions
    fs::create_dir_all(&cache_dir)?;
    let mut dir_perms = fs::metadata(&cache_dir)?.permissions();
    dir_perms.set_mode(0o700);
    let _ = fs::set_permissions(&cache_dir, dir_perms);

    let serialized = serde_json::to_string_pretty(payload)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    let tmp_file_path = cache_dir.join(format!("{}.tmp.{}", CACHE_FILE_NAME, std::process::id()));
    let target_file_path = cache_dir.join(CACHE_FILE_NAME);

    {
        let mut tmp_file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp_file_path)?;

        tmp_file.write_all(serialized.as_bytes())?;
        tmp_file.sync_all()?;
    }

    fs::rename(tmp_file_path, target_file_path)?;

    Ok(())
}
