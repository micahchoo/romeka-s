//! Unguessable ids: the session token and the one-time password key.
//!
//! Upstream draws every character of an API key with `random_int`
//! (`Entity/ApiKey::getString`), and `Entity/PasswordCreation::setId` reuses
//! that generator, so a password key and a key identity have the same shape:
//! [`TOKEN_LENGTH`] characters from [`ALPHABET`].
//!
//! The port reads bytes from the operating system's CSPRNG. `omeka-api` has no
//! random-number dependency, so this module reads `/dev/urandom` directly,
//! which is the same source the `getrandom` crate wraps; `omeka-media` and
//! `omeka-cli` reach it through that crate.
//!
//! Bytes at or above [`UNBIASED_LIMIT`] are redrawn, so `% 62` stays uniform.
//! A redraw is rare (8 of every 256 bytes), and drawing 64 bytes at a time
//! keeps it rare in practice too.

use std::fs::File;
use std::io::Read;

use crate::errors::ApiError;

/// `ApiKey::STRING_LENGTH`, which `PasswordCreation::setId` inherits.
pub const TOKEN_LENGTH: usize = 32;

/// `ApiKey::STRING_CHARLIST`, in upstream's order.
pub const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890";

/// The largest byte value divisible by the alphabet. Above it, `% 62` would
/// favour the first characters, so those bytes are redrawn.
pub const UNBIASED_LIMIT: u8 = 248;

/// The OS random source. A reader per call keeps no lock and no shared offset.
const RANDOM_SOURCE: &str = "/dev/urandom";

/// One token: [`TOKEN_LENGTH`] characters, drawn uniformly.
///
/// # Errors
/// An [`ApiError`] when the OS random source cannot be read.
pub fn token() -> Result<String, ApiError> {
    random_string(TOKEN_LENGTH)
}

/// `length` characters from [`ALPHABET`], each equally likely.
///
/// # Errors
/// An [`ApiError`] when the OS random source cannot be read.
pub fn random_string(length: usize) -> Result<String, ApiError> {
    let mut source = File::open(RANDOM_SOURCE)
        .map_err(|error| ApiError::internal(format!("{RANDOM_SOURCE}: {error}")))?;
    let mut result = String::with_capacity(length);
    let mut bytes = [0u8; 64];
    while result.len() < length {
        source
            .read_exact(&mut bytes)
            .map_err(|error| ApiError::internal(format!("{RANDOM_SOURCE}: {error}")))?;
        for byte in bytes {
            if byte < UNBIASED_LIMIT {
                result.push(ALPHABET[usize::from(byte % (ALPHABET.len() as u8))] as char);
                if result.len() == length {
                    break;
                }
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn the_alphabet_and_the_redraw_limit_match_upstream() {
        assert_eq!(ALPHABET.len(), 62);
        assert_eq!(
            ALPHABET,
            b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890"
        );
        assert_eq!(usize::from(UNBIASED_LIMIT), 256 - 256 % ALPHABET.len());
    }

    #[test]
    fn a_token_has_the_api_key_shape() {
        let value = token().unwrap();
        assert_eq!(value.len(), TOKEN_LENGTH);
        assert!(value.bytes().all(|byte| ALPHABET.contains(&byte)));
        // The shape check the key adapter runs on a presented identity.
        assert!(crate::auth::identity_is_well_formed(&value));
    }

    #[test]
    fn two_tokens_differ() {
        let first = token().unwrap();
        let second = token().unwrap();
        assert_ne!(first, second);
    }

    #[test]
    fn a_requested_length_other_than_the_default_is_honoured() {
        assert_eq!(random_string(7).unwrap().len(), 7);
        assert_eq!(random_string(0).unwrap(), "");
    }
}
