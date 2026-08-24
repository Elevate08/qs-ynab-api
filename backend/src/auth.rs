use keyring::Entry;
use std::fmt;

const KEYRING_SERVICE: &str = "io.github.elevate08.ynab-glance";
const KEYRING_USER: &str = "pat";

#[derive(Debug)]
pub enum AuthError {
    KeyringError(String),
    InvalidTokenFormat,
    NotFound,
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthError::KeyringError(msg) => write!(f, "Secret Service Keyring error: {}", msg),
            AuthError::InvalidTokenFormat => write!(f, "Token format is invalid or empty"),
            AuthError::NotFound => write!(f, "No personal access token found in keyring"),
        }
    }
}

impl std::error::Error for AuthError {}

/// Securely retrieves the YNAB Personal Access Token from Linux Secret Service
pub fn get_token() -> Result<String, AuthError> {
    let entry = Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .map_err(|e| AuthError::KeyringError(e.to_string()))?;

    match entry.get_password() {
        Ok(token) => {
            let trimmed = token.trim().to_string();
            if trimmed.is_empty() {
                Err(AuthError::NotFound)
            } else {
                Ok(trimmed)
            }
        }
        Err(keyring::Error::NoEntry) => Err(AuthError::NotFound),
        Err(e) => Err(AuthError::KeyringError(e.to_string())),
    }
}

/// Securely stores the YNAB Personal Access Token in Linux Secret Service
pub fn set_token(token: &str) -> Result<(), AuthError> {
    let trimmed = token.trim();
    if trimmed.is_empty() || trimmed.len() < 10 {
        return Err(AuthError::InvalidTokenFormat);
    }

    let entry = Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .map_err(|e| AuthError::KeyringError(e.to_string()))?;

    entry
        .set_password(trimmed)
        .map_err(|e| AuthError::KeyringError(e.to_string()))?;

    Ok(())
}

/// Securely deletes the token from Secret Service
pub fn clear_token() -> Result<(), AuthError> {
    let entry = Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .map_err(|e| AuthError::KeyringError(e.to_string()))?;

    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(AuthError::KeyringError(e.to_string())),
    }
}

/// Checks if a token is present in the keyring without exposing it
pub fn has_token() -> bool {
    get_token().is_ok()
}
