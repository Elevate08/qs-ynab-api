use keyring::Entry;
use std::fmt;
use zeroize::Zeroizing;

use crate::crypto::{self, CryptoError};

const KEYRING_SERVICE: &str = "io.github.elevate08.ynab-glance";
const KEYRING_USER: &str = "pat";

/// Bounds for an accepted Personal Access Token. YNAB issues 64-char hex
/// tokens; the range stays permissive in case that format changes, but caps
/// the length so an oversized paste can never reach the keyring or a header.
const MIN_TOKEN_LEN: usize = 16;
const MAX_TOKEN_LEN: usize = 512;

#[derive(Debug)]
pub enum AuthError {
    KeyringError(String),
    InvalidTokenFormat,
    NotFound,
    /// The keyring entry exists but this machine's key cannot open it.
    Undecryptable(CryptoError),
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthError::KeyringError(msg) => write!(f, "Secret Service Keyring error: {}", msg),
            AuthError::InvalidTokenFormat => write!(
                f,
                "Token format is invalid: expected {}-{} printable, non-whitespace ASCII characters",
                MIN_TOKEN_LEN, MAX_TOKEN_LEN
            ),
            AuthError::NotFound => write!(
                f,
                "No YNAB Personal Access Token configured in Keyring"
            ),
            AuthError::Undecryptable(e) => write!(f, "{}", e),
        }
    }
}

impl std::error::Error for AuthError {}

/// Validates a candidate token without ever echoing it back.
///
/// Rejecting control characters and whitespace matters beyond hygiene: a token
/// containing CR/LF would otherwise be rejected only later, by the HTTP header
/// encoder, and a token with a trailing newline silently authenticates as a
/// different string than the one the user believes they pasted.
fn validate_token(candidate: &str) -> Result<(), AuthError> {
    let len = candidate.len();
    if !(MIN_TOKEN_LEN..=MAX_TOKEN_LEN).contains(&len) {
        return Err(AuthError::InvalidTokenFormat);
    }
    if !candidate
        .chars()
        .all(|c| c.is_ascii_graphic() && c != '\u{7f}')
    {
        return Err(AuthError::InvalidTokenFormat);
    }
    Ok(())
}

/// Securely retrieves the YNAB Personal Access Token from Linux Secret Service.
///
/// The token is returned in a `Zeroizing` wrapper so the plaintext is wiped
/// from process memory when the last holder drops it.
pub fn get_token() -> Result<Zeroizing<String>, AuthError> {
    let entry = Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .map_err(|e| AuthError::KeyringError(e.to_string()))?;

    let stored = match entry.get_secret() {
        Ok(bytes) => Zeroizing::new(bytes),
        Err(keyring::Error::NoEntry) => return Err(AuthError::NotFound),
        Err(e) => return Err(AuthError::KeyringError(e.to_string())),
    };

    if crypto::is_sealed(&stored) {
        let opened = crypto::open(&stored).map_err(AuthError::Undecryptable)?;
        let trimmed = Zeroizing::new(opened.trim().to_string());
        return if trimmed.is_empty() {
            Err(AuthError::NotFound)
        } else {
            Ok(trimmed)
        };
    }

    // A plaintext entry written by an older version. Return it, but seal it on
    // the way past so the keyring stops holding a readable token: an upgrade
    // must not strand a working credential, and must not leave it bare either.
    let legacy = Zeroizing::new(
        String::from_utf8(stored.to_vec())
            .map_err(|_| AuthError::InvalidTokenFormat)?
            .trim()
            .to_string(),
    );
    if legacy.is_empty() {
        return Err(AuthError::NotFound);
    }
    let _ = set_token(&legacy);
    Ok(legacy)
}

/// Securely stores the YNAB Personal Access Token in Linux Secret Service
pub fn set_token(token: &str) -> Result<(), AuthError> {
    let trimmed = token.trim();
    validate_token(trimmed)?;

    // The keyring never sees the token itself, only the sealed envelope.
    let sealed = crypto::seal(trimmed).map_err(AuthError::Undecryptable)?;

    let entry = Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .map_err(|e| AuthError::KeyringError(e.to_string()))?;

    entry
        .set_secret(&sealed)
        .map_err(|e| AuthError::KeyringError(e.to_string()))?;

    Ok(())
}

/// Securely deletes the token from Secret Service
pub fn clear_token() -> Result<(), AuthError> {
    let entry = Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .map_err(|e| AuthError::KeyringError(e.to_string()))?;

    let removed = match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(AuthError::KeyringError(e.to_string())),
    };

    // The key outliving the credential would leave a stale envelope openable
    // if one were ever restored from a keyring backup. Destroy it either way,
    // including when the keyring delete failed: a token that cannot be
    // decrypted is the safer end state.
    let _ = crypto::destroy_key();

    removed
}

/// Reads a token from stdin so it never appears in argv, shell history, or
/// `/proc/<pid>/cmdline` (world-readable to every local process by default).
pub fn read_token_from_stdin() -> Result<Zeroizing<String>, AuthError> {
    use std::io::Read;

    let mut raw = Zeroizing::new(String::new());
    std::io::stdin()
        .take((MAX_TOKEN_LEN + 2) as u64)
        .read_to_string(&mut raw)
        .map_err(|_| AuthError::InvalidTokenFormat)?;

    let trimmed = Zeroizing::new(raw.trim().to_string());
    validate_token(&trimmed)?;
    Ok(trimmed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_short_empty_and_oversized_tokens() {
        assert!(validate_token("").is_err());
        assert!(validate_token("short").is_err());
        assert!(validate_token(&"a".repeat(MAX_TOKEN_LEN + 1)).is_err());
    }

    #[test]
    fn rejects_whitespace_and_control_characters() {
        // A CRLF-bearing token would be a header-injection attempt.
        assert!(validate_token("abcdefghijklmnop\r\nX-Evil: 1").is_err());
        assert!(validate_token("abcdefgh ijklmnopq").is_err());
        assert!(validate_token("abcdefghijklmnop\u{7f}").is_err());
    }

    #[test]
    fn accepts_a_realistic_ynab_token() {
        assert!(validate_token(&"a1b2c3d4".repeat(8)).is_ok());
    }

    /// End-to-end through the real Secret Service, under a throwaway service
    /// name so it can never disturb the user's actual credential. Ignored by
    /// default because it needs a running, unlocked secret service, and it
    /// seals with the real per-install key (or creates one, if this is a fresh
    /// install). Point `XDG_DATA_HOME` elsewhere to keep it off that key:
    ///
    ///     XDG_DATA_HOME=$(mktemp -d) cargo test -- --ignored
    #[test]
    #[ignore]
    fn a_sealed_token_survives_a_real_keyring_round_trip() {
        let service = format!("{}.selftest.{}", KEYRING_SERVICE, std::process::id());
        let entry = Entry::new(&service, KEYRING_USER).expect("keyring entry");
        let token = "a1b2c3d4".repeat(8);

        let sealed = crypto::seal(&token).expect("seal");
        entry.set_secret(&sealed).expect("store");

        let fetched = entry.get_secret().expect("fetch");
        assert!(crypto::is_sealed(&fetched), "keyring holds plaintext");
        assert!(
            !fetched.windows(token.len()).any(|w| w == token.as_bytes()),
            "the token itself is readable in the keyring"
        );
        assert_eq!(crypto::open(&fetched).expect("open").as_str(), token);

        entry.delete_credential().expect("cleanup");
    }
}
