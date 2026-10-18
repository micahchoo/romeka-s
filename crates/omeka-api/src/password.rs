//! `User::setPassword` and `User::verifyPassword`, and the password rules.
//!
//! Upstream stores `password_hash($password, PASSWORD_DEFAULT)`
//! (`Entity/User.php:156`), which PHP fills with bcrypt at cost 10 and the
//! `$2y$` prefix. [`hash`] writes that exact shape with the `bcrypt` crate, and
//! [`verify`] is `password_verify`: it accepts every bcrypt version prefix
//! (`$2a$`, `$2b$`, `$2x$`, `$2y$`), so the hashes a migrated Omeka S install
//! carries keep working, and so do the ones the port writes.
//!
//! The column holds 60 characters upstream (`varchar(60)`), which is exactly
//! what bcrypt writes.
//!
//! The rules are `Config['password']` (`application/config/module.config.php:5`),
//! enforced by `Form\Element\PasswordConfirm::passwordIsValid`. Upstream
//! measures the length in bytes (`strlen`) and the character classes by
//! case-folding the whole string, so the port counts bytes for the length and
//! Unicode characters for the classes; the shipped config sets only
//! `min_length`, so the other four rules are inert until an operator sets them.

use bcrypt::Version;
use thiserror::Error;

use crate::errors::ApiError;

/// The bcrypt cost. Upstream's `password_hash` runs PHP's default, 10.
pub const BCRYPT_COST: u32 = 10;

/// The message `PasswordConfirm::passwordIsValid` reports through its callback.
pub const INVALID_PASSWORD: &str = "The password is invalid";

/// `Config['password']['symbol_list']`.
pub const SYMBOL_LIST: &str = "`~!@#$%^&*()-=_+[]\\{}|;:\",./<>?'";

/// A failure while hashing: no salt, or the crate refused the input.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum HashError {
    #[error("cannot hash the password: {0}")]
    Bcrypt(String),
}

impl From<HashError> for ApiError {
    fn from(error: HashError) -> Self {
        Self::internal(error)
    }
}

/// `User::setPassword`: the stored hash for one password.
///
/// # Errors
/// A [`HashError`] when the operating system has no randomness for the salt.
pub fn hash(password: &str) -> Result<String, HashError> {
    let parts = bcrypt::hash_with_result(password, BCRYPT_COST)
        .map_err(|error| HashError::Bcrypt(error.to_string()))?;
    Ok(parts.format_for_version(Version::TwoY))
}

/// `User::verifyPassword`: `password_verify` against the stored hash.
///
/// The hash is `Option` because `user.password_hash` is nullable and upstream
/// refuses every password when it is null. A hash this build cannot parse is
/// false, not an error, which is what `password_verify` does.
#[must_use]
pub fn verify(password: &str, stored: Option<&str>) -> bool {
    stored.is_some_and(|hash| bcrypt::verify(password, hash).unwrap_or(false))
}

/// `Config['password']`: the rules a new password must satisfy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasswordRules {
    pub min_length: Option<usize>,
    pub min_lowercase: Option<usize>,
    pub min_uppercase: Option<usize>,
    pub min_number: Option<usize>,
    pub min_symbol: Option<usize>,
    pub symbol_list: String,
}

impl Default for PasswordRules {
    /// The shipped configuration: six characters and no character-class rule.
    fn default() -> Self {
        Self {
            min_length: Some(6),
            min_lowercase: None,
            min_uppercase: None,
            min_number: None,
            min_symbol: None,
            symbol_list: SYMBOL_LIST.to_string(),
        }
    }
}

impl PasswordRules {
    /// `PasswordConfirm::passwordIsValid`.
    #[must_use]
    pub fn accepts(&self, password: &str) -> bool {
        if self
            .min_length
            .is_some_and(|minimum| password.len() < minimum)
        {
            return false;
        }
        if self
            .min_lowercase
            .is_some_and(|minimum| count(password, char::is_lowercase) < minimum)
        {
            return false;
        }
        if self
            .min_uppercase
            .is_some_and(|minimum| count(password, char::is_uppercase) < minimum)
        {
            return false;
        }
        if self
            .min_number
            .is_some_and(|minimum| count(password, |c| c.is_ascii_digit()) < minimum)
        {
            return false;
        }
        if let (Some(minimum), true) = (self.min_symbol, !self.symbol_list.is_empty()) {
            let symbols = self
                .symbol_list
                .chars()
                .map(|symbol| password.matches(symbol).count())
                .sum::<usize>();
            if symbols < minimum {
                return false;
            }
        }
        true
    }
}

fn count(password: &str, matches: impl Fn(char) -> bool) -> usize {
    password.chars().filter(|c| matches(*c)).count()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn user_set_password() {
        let stored = hash("hunter2").unwrap();
        assert_eq!(stored.len(), 60, "varchar(60) holds exactly this: {stored}");
        assert!(stored.starts_with("$2y$10$"), "{stored}");
        assert!(verify("hunter2", Some(&stored)));
        assert!(!verify("hunter3", Some(&stored)));
    }

    #[test]
    fn user_verify_password() {
        // `User::verifyPassword` refuses every password when no hash is set.
        assert!(!verify("anything", None));
        // A hash written by another bcrypt implementation verifies. This one
        // comes from Python's bcrypt at cost 4.
        assert!(verify(
            "correctbatteryhorsestapler",
            Some("$2b$04$EGdrhbKUv8Oc9vGiXX0HQOxSg445d458Muh7DAHskb6QbtCvdxcie")
        ));
        // A `$2y$` hash, the prefix PHP's `password_hash` writes.
        let php_shaped = bcrypt::hash_with_result("hunter2", 4)
            .unwrap()
            .format_for_version(Version::TwoY);
        assert!(php_shaped.starts_with("$2y$04$"), "{php_shaped}");
        assert!(verify("hunter2", Some(&php_shaped)));
        // A stored value that is not a bcrypt hash fails closed.
        assert!(!verify("hunter2", Some("not-a-hash")));
        assert!(!verify("hunter2", Some("$pbkdf2-sha256$100000$aa$bb")));
    }

    #[test]
    fn the_shipped_rules_require_six_bytes() {
        let rules = PasswordRules::default();
        assert!(!rules.accepts("short"));
        assert!(!rules.accepts("five5"));
        assert!(rules.accepts("sixchr"));
        assert_eq!(rules.min_length, Some(6));
        assert_eq!(rules.symbol_list, SYMBOL_LIST);
    }

    #[test]
    fn a_configured_character_class_rule_is_enforced() {
        let rules = PasswordRules {
            min_lowercase: Some(1),
            min_uppercase: Some(1),
            min_number: Some(1),
            min_symbol: Some(1),
            ..PasswordRules::default()
        };
        assert!(rules.accepts("aB1!cd"));
        assert!(!rules.accepts("AB1!CD"));
        assert!(!rules.accepts("ab1!cd"));
        assert!(!rules.accepts("aBcd!e"));
        assert!(!rules.accepts("aB1cde"));
    }

    #[test]
    fn an_empty_symbol_list_disables_the_symbol_rule() {
        let rules = PasswordRules {
            min_symbol: Some(1),
            symbol_list: String::new(),
            ..PasswordRules::default()
        };
        assert!(rules.accepts("plain1"));
    }
}
