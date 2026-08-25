//! Envelope encryption for the Personal Access Token and the offline cache.
//!
//! The keyring never holds the token itself and the cache is never readable
//! JSON; both are sealed with a key this plugin owns. What that does and does
//! not defend against - it stops an untargeted keyring scraper, not an
//! attacker looking for this plugin - is set out in SECURITY.md under "What
//! the encryption layer is and is not". That argument belongs in one place.
//!
//! Construction: XChaCha20-Poly1305 with a 32-byte key and a random 192-bit
//! nonce per seal. The nonce is large enough that random generation needs no
//! counter and no reuse tracking. The magic bytes are authenticated as
//! associated data, so an envelope cannot be replayed as a different version,
//! and any bit flip fails the tag rather than decrypting to garbage that gets
//! sent to YNAB as a bearer token.

use std::path::PathBuf;

use chacha20poly1305::{
    aead::{Aead, KeyInit, Payload},
    Key, XChaCha20Poly1305, XNonce,
};
use zeroize::Zeroizing;

use crate::storage::{self, Publish};

/// The plugin id, not `omarchy/ynab-glance`: Omarchy symlinks
/// `~/.local/share/omarchy` to its root-owned system install, so a key
/// directory nested under that name resolves into `/usr/share` and cannot be
/// created. The id is unique to this plugin and cannot collide with it.
const KEY_FILE_NAME: &str = "token.key";

const KEY_BYTES: usize = 32;
const NONCE_BYTES: usize = 24;

/// Envelope prefix. Also the associated data, so the version cannot be
/// stripped or downgraded without failing the tag.
const MAGIC: &[u8; 4] = b"YNP1";

#[derive(Debug)]
pub enum CryptoError {
    /// The key file is missing, unreadable, or not the right size.
    KeyUnavailable(String),
    /// The key file could not be created.
    KeyNotWritable(String),
    /// The envelope failed authentication: wrong key, or tampered ciphertext.
    Undecryptable,
    /// Sealing failed. Distinct from `Undecryptable` because telling a user to
    /// re-enter their token is the wrong advice when nothing was stored yet.
    NotSealable,
    /// The system CSPRNG failed.
    RandomFailure,
}

impl std::fmt::Display for CryptoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CryptoError::KeyUnavailable(msg) => write!(
                f,
                "Token encryption key is unavailable ({}). Re-enter your Personal Access Token to generate a new one.",
                msg
            ),
            CryptoError::KeyNotWritable(msg) => {
                write!(f, "Could not create the token encryption key: {}", msg)
            }
            CryptoError::Undecryptable => write!(
                f,
                "The stored token could not be decrypted with this machine's key. Re-enter your Personal Access Token."
            ),
            CryptoError::NotSealable => write!(f, "The token could not be encrypted for storage"),
            CryptoError::RandomFailure => {
                write!(f, "The system random number generator is unavailable")
            }
        }
    }
}

impl std::error::Error for CryptoError {}

fn key_dir() -> Option<PathBuf> {
    storage::xdg_dir("XDG_DATA_HOME", ".local/share")
}

fn key_path() -> Option<PathBuf> {
    Some(key_dir()?.join(KEY_FILE_NAME))
}

/// Reads the key, refusing anything that is not a private regular file.
///
/// Opened `O_NOFOLLOW | O_NONBLOCK` and then inspected through the descriptor,
/// so a symlink planted in its place is refused by the kernel, a FIFO fails
/// instead of blocking, and there is no window between the check and the read.
fn load_key() -> Result<Zeroizing<Vec<u8>>, CryptoError> {
    let path = key_path().ok_or_else(|| CryptoError::KeyUnavailable("no HOME".to_string()))?;

    // One byte over the key size, so a longer file is refused rather than
    // silently truncated to something that looks like a valid key.
    let bytes = Zeroizing::new(
        storage::read_private_file(&path, (KEY_BYTES + 1) as u64)
            .map_err(|e| CryptoError::KeyUnavailable(e.to_string()))?,
    );

    if bytes.len() != KEY_BYTES {
        return Err(CryptoError::KeyUnavailable(format!(
            "expected {} key bytes, found {}",
            KEY_BYTES,
            bytes.len()
        )));
    }
    Ok(bytes)
}

/// Returns the existing key, or generates one on first use.
///
/// The key is written 0600 into a 0700 directory, created with `O_EXCL` and
/// renamed into place, so two helpers racing on first run cannot end up with
/// one of them reading a half-written key.
fn load_or_create_key() -> Result<Zeroizing<Vec<u8>>, CryptoError> {
    match load_key() {
        Ok(key) => return Ok(key),
        Err(CryptoError::KeyUnavailable(_)) => {}
        Err(e) => return Err(e),
    }

    let dir = key_dir().ok_or_else(|| CryptoError::KeyNotWritable("no HOME".to_string()))?;
    storage::ensure_private_dir(&dir)
        .map_err(|e| CryptoError::KeyNotWritable(format!("{}: {}", dir.display(), e)))?;

    let mut key = Zeroizing::new(vec![0u8; KEY_BYTES]);
    getrandom::fill(&mut key).map_err(|_| CryptoError::RandomFailure)?;

    let target = dir.join(KEY_FILE_NAME);
    // `KeepExisting` is what makes this first-writer-wins: exactly one
    // concurrent starter creates the key and every other one reads what the
    // winner wrote. A last-writer-wins publish would instead let both
    // "succeed", and the loser would seal the token under a key that no longer
    // exists on disk - an envelope nothing can ever open, discovered only at
    // the next read.
    match storage::write_private_file(&target, &key, Publish::KeepExisting) {
        Ok(()) => Ok(key),
        Err(ref e) if e.kind() == std::io::ErrorKind::AlreadyExists => load_key(),
        Err(e) => load_key()
            .map_err(|_| CryptoError::KeyNotWritable(format!("{}: {}", target.display(), e))),
    }
}

/// Removes the key file. Called when the token is revoked: the envelope in the
/// keyring must not stay openable after the credential it protects is gone.
pub fn destroy_key() -> Result<(), std::io::Error> {
    use std::fs;
    let path = match key_path() {
        Some(p) => p,
        None => return Ok(()),
    };
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// True if the bytes carry our envelope header.
///
/// Used to tell a sealed token from a plaintext one written by an older
/// version, so an upgrade does not strand the user's credential.
pub fn is_sealed(bytes: &[u8]) -> bool {
    bytes.len() > MAGIC.len() + NONCE_BYTES && bytes.starts_with(MAGIC)
}

/// Seals a token for storage, generating the key on first use.
///
/// Layout: `MAGIC(4) || nonce(24) || ciphertext+tag`.
pub fn seal(plaintext: &str) -> Result<Vec<u8>, CryptoError> {
    let key = load_or_create_key()?;
    let cipher = XChaCha20Poly1305::new(Key::from_slice(&key));

    let mut nonce_bytes = [0u8; NONCE_BYTES];
    getrandom::fill(&mut nonce_bytes).map_err(|_| CryptoError::RandomFailure)?;
    let nonce = XNonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(
            nonce,
            Payload {
                msg: plaintext.as_bytes(),
                aad: MAGIC,
            },
        )
        .map_err(|_| CryptoError::NotSealable)?;

    let mut out = Vec::with_capacity(MAGIC.len() + NONCE_BYTES + ciphertext.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Opens a sealed token. Never generates a key: if the key is gone, the
/// envelope is unreadable and the user has to re-enter the token.
pub fn open(envelope: &[u8]) -> Result<Zeroizing<String>, CryptoError> {
    if !is_sealed(envelope) {
        return Err(CryptoError::Undecryptable);
    }
    let key = load_key()?;
    let cipher = XChaCha20Poly1305::new(Key::from_slice(&key));

    let nonce = XNonce::from_slice(&envelope[MAGIC.len()..MAGIC.len() + NONCE_BYTES]);
    let body = &envelope[MAGIC.len() + NONCE_BYTES..];

    let plaintext = Zeroizing::new(
        cipher
            .decrypt(nonce, Payload { msg: body, aad: MAGIC })
            .map_err(|_| CryptoError::Undecryptable)?,
    );

    let text = String::from_utf8(plaintext.to_vec()).map_err(|_| CryptoError::Undecryptable)?;
    Ok(Zeroizing::new(text))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    /// Each test gets its own key directory: the key is per-install state, and
    /// tests that shared one would race on creating and destroying it.
    fn with_temp_key_dir<T>(name: &str, body: impl FnOnce() -> T) -> T {
        let _guard = crate::test_env::lock();

        let base = std::env::temp_dir().join(format!("ynab-key-test-{}-{}", std::process::id(), name));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        std::env::set_var("XDG_DATA_HOME", &base);
        let out = body();
        std::env::remove_var("XDG_DATA_HOME");
        let _ = fs::remove_dir_all(&base);
        out
    }

    #[test]
    fn a_sealed_token_round_trips() {
        with_temp_key_dir("roundtrip", || {
            let token = "a1b2c3d4".repeat(8);
            let sealed = seal(&token).unwrap();
            assert!(is_sealed(&sealed));
            // The point of the exercise: the token is not in the stored bytes.
            assert!(!sealed.windows(token.len()).any(|w| w == token.as_bytes()));
            assert_eq!(open(&sealed).unwrap().as_str(), token);
        });
    }

    #[test]
    fn each_seal_uses_a_fresh_nonce() {
        with_temp_key_dir("nonce", || {
            let a = seal("token-aaaaaaaaaaaa").unwrap();
            let b = seal("token-aaaaaaaaaaaa").unwrap();
            assert_ne!(a, b, "identical ciphertexts mean a reused nonce");
        });
    }

    #[test]
    fn a_tampered_envelope_is_refused() {
        with_temp_key_dir("tamper", || {
            let mut sealed = seal("token-aaaaaaaaaaaa").unwrap();
            let last = sealed.len() - 1;
            sealed[last] ^= 0x01;
            assert!(matches!(open(&sealed), Err(CryptoError::Undecryptable)));
        });
    }

    #[test]
    fn a_different_key_cannot_open_the_envelope() {
        let sealed = with_temp_key_dir("key-a", || seal("token-aaaaaaaaaaaa").unwrap());
        // A fresh directory means a freshly generated key, which is exactly
        // the position a keyring scraper on another machine is in.
        with_temp_key_dir("key-b", || {
            let _ = seal("something-else-here").unwrap();
            assert!(matches!(open(&sealed), Err(CryptoError::Undecryptable)));
        });
    }

    /// `rename` overwrites, so concurrent first-run helpers both write a key
    /// and only one survives. Whatever each caller gets back has to be the one
    /// on disk, or it seals data under a key that no longer exists.
    #[test]
    fn concurrent_first_use_agrees_on_the_key_that_is_on_disk() {
        with_temp_key_dir("race", || {
            let mut handles = Vec::new();
            for _ in 0..8 {
                handles.push(std::thread::spawn(|| load_or_create_key().unwrap().to_vec()));
            }
            let returned: Vec<Vec<u8>> = handles.into_iter().map(|h| h.join().unwrap()).collect();

            let on_disk = fs::read(key_path().unwrap()).unwrap();
            for (i, key) in returned.iter().enumerate() {
                assert_eq!(key, &on_disk, "thread {} returned a key that is not on disk", i);
            }
        });
    }

    #[test]
    fn a_missing_key_is_reported_not_regenerated() {
        with_temp_key_dir("missing", || {
            let sealed = seal("token-aaaaaaaaaaaa").unwrap();
            destroy_key().unwrap();
            assert!(matches!(open(&sealed), Err(CryptoError::KeyUnavailable(_))));
        });
    }

    #[test]
    fn plaintext_is_not_mistaken_for_an_envelope() {
        assert!(!is_sealed(b"a1b2c3d4a1b2c3d4a1b2c3d4a1b2c3d4"));
        assert!(!is_sealed(b""));
        assert!(!is_sealed(MAGIC));
    }

    /// Regression: `~/.local/share/omarchy` is a symlink to the root-owned
    /// system install on Omarchy, so a key directory reached through a symlink
    /// must fail by name instead of surfacing a bare "Permission denied".
    #[test]
    fn a_symlinked_key_directory_is_refused_by_name() {
        with_temp_key_dir("symlink-dir", || {
            let dir = key_dir().unwrap();
            let elsewhere = dir.parent().unwrap().join("elsewhere");
            fs::create_dir_all(&elsewhere).unwrap();
            std::os::unix::fs::symlink(&elsewhere, &dir).unwrap();

            match seal("token-aaaaaaaaaaaa") {
                Err(CryptoError::KeyNotWritable(msg)) => {
                    assert!(msg.contains("symlink"), "unhelpful message: {}", msg)
                }
                other => panic!("expected a named refusal, got {:?}", other),
            }
        });
    }

    #[test]
    fn a_world_readable_key_is_refused() {
        with_temp_key_dir("perms", || {
            let sealed = seal("token-aaaaaaaaaaaa").unwrap();
            let path = key_path().unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
            assert!(matches!(open(&sealed), Err(CryptoError::KeyUnavailable(_))));
        });
    }
}
