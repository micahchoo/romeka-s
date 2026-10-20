//! API key credentials as they arrive in the query string.
//!
//! `MvcListeners::authenticateApiKey` reads two query parameters,
//! `key_identity` and `key_credential`, on every request to the `/api` route.
//! When both are present it runs the authentication service; a failed attempt
//! is silent, the request continues as an anonymous user. That decision is why
//! nothing here returns an HTTP error: this module only names and validates
//! what arrived, and wave 2 turns it into an authentication attempt.
//!
//! An identity that is not 32 `[A-Za-z0-9]` characters can never match
//! `ApiKey::STRING_LENGTH` / `STRING_CHARLIST`, so [`identity_is_well_formed`]
//! lets wave 2 skip a database round trip without changing the observable
//! result. The credential is *not* shape-checked: upstream passes it to
//! `password_verify`, which accepts any string.
//!
//! [`authenticate`] runs bcrypt on every attempt, which is what upstream does.
//! [`authenticate_with`] takes a [`CredentialVerifier`], and the server passes
//! an [`AuthCache`](crate::auth_cache::AuthCache), so a key that verified once
//! in this process skips the bcrypt call on its later requests. See
//! `auth_cache` for the bound, the lifetime and the tradeoff.

/// `MvcListeners::authenticateApiKey` query parameter names.
pub const KEY_IDENTITY_PARAM: &str = "key_identity";
pub const KEY_CREDENTIAL_PARAM: &str = "key_credential";

/// `ApiKey::STRING_LENGTH`.
pub const API_KEY_LENGTH: usize = 32;

/// The credential/identity alphabet from `ApiKey::STRING_CHARLIST`
/// (`a-zA-Z0-9`; note there is no `0`-`9` ordering subtlety, the list simply
/// spells out alphanumerics).
pub fn is_api_key_char(c: char) -> bool {
    c.is_ascii_alphanumeric()
}

/// True when a string could be a stored key identity.
pub fn identity_is_well_formed(identity: &str) -> bool {
    identity.len() == API_KEY_LENGTH && identity.chars().all(is_api_key_char)
}

/// The pair of query parameters upstream treats as an authentication attempt.
///
/// Construction does not validate the credential; use
/// [`KeyCredentials::identity_is_well_formed`] before a lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyCredentials {
    identity: String,
    credential: String,
}

impl KeyCredentials {
    /// Build credentials from raw values.
    pub fn new(identity: impl Into<String>, credential: impl Into<String>) -> Self {
        Self {
            identity: identity.into(),
            credential: credential.into(),
        }
    }

    /// Read credentials from decoded query pairs.
    ///
    /// Returns `None` when either parameter is absent, mirroring upstream's
    /// `is_null($identity) || is_null($credential)` early return. On duplicate
    /// names the last pair wins, which is what PHP `parse_str` does.
    pub fn from_pairs<'a, I>(pairs: I) -> Option<Self>
    where
        I: IntoIterator<Item = (&'a str, &'a str)>,
    {
        let mut identity = None;
        let mut credential = None;
        for (name, value) in pairs {
            match name {
                KEY_IDENTITY_PARAM => identity = Some(value.to_string()),
                KEY_CREDENTIAL_PARAM => credential = Some(value.to_string()),
                _ => {}
            }
        }
        Some(Self {
            identity: identity?,
            credential: credential?,
        })
    }

    /// Read credentials from a raw query string (`a=1&b=2`).
    ///
    /// Percent escapes and `+` are decoded, matching what PHP put into
    /// `$_GET`.
    pub fn from_query(query: &str) -> Option<Self> {
        let pairs: Vec<(String, String)> = query
            .split('&')
            .filter(|part| !part.is_empty())
            .map(|part| {
                let (name, value) = part.split_once('=').unwrap_or((part, ""));
                (decode(name), decode(value))
            })
            .collect();
        Self::from_pairs(
            pairs
                .iter()
                .map(|(name, value)| (name.as_str(), value.as_str())),
        )
    }

    pub fn identity(&self) -> &str {
        &self.identity
    }

    pub fn credential(&self) -> &str {
        &self.credential
    }

    /// See [`identity_is_well_formed`].
    pub fn identity_is_well_formed(&self) -> bool {
        identity_is_well_formed(&self.identity)
    }
}

use omeka_db::DbPool;
use omeka_db::ids::UserId;
use omeka_db::repos::{ApiKeyRepo, UserRepo};
use omeka_db::with_db;

use crate::auth_cache::{Bcrypt, CredentialVerifier};
use crate::errors::ApiError;

/// The authenticated caller of one API request.
///
/// Upstream `MvcListeners::authenticateApiKey` runs the key adapter silently: a
/// missing, malformed, unknown or unverifiable key leaves the request
/// anonymous. `Omeka\Permissions\Acl` then grants an unauthenticated role
/// `search` and `read` on most adapters and nothing on `users`, `jobs`,
/// `modules` and `value_annotations` (`Service\AclFactory::addRulesForAllRoles`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Actor {
    user: Option<UserId>,
    role: Option<String>,
}

impl Actor {
    /// The unauthenticated caller.
    pub fn anonymous() -> Self {
        Self::default()
    }

    pub fn new(user: UserId, role: impl Into<String>) -> Self {
        Self {
            user: Some(user),
            role: Some(role.into()),
        }
    }

    pub fn user(&self) -> Option<UserId> {
        self.user
    }

    pub fn role(&self) -> Option<&str> {
        self.role.as_deref()
    }

    /// Roles the ACL grants `view-all` on `Omeka\Entity\Resource`: the two
    /// admin roles (which are allowed everything) plus editor and reviewer
    /// (`AclFactory::addRulesForReviewer` / `addRulesForEditor`).
    pub fn is_privileged(&self) -> bool {
        matches!(
            self.role(),
            Some("global_admin" | "site_admin" | "editor" | "reviewer")
        )
    }

    /// Whether the actor holds one of the two admin roles, which upstream's
    /// `AclFactory` allows everything (`$acl->allow('site_admin')` /
    /// `$acl->allow('global_admin')`).
    pub fn is_admin(&self) -> bool {
        matches!(self.role(), Some("global_admin" | "site_admin"))
    }

    /// Whether a resource is visible: public, owned by the caller, or the
    /// caller holds `view-all`.
    pub fn sees_resource(&self, is_public: bool, owner: Option<UserId>) -> bool {
        is_public
            || self.is_privileged()
            || match (self.user, owner) {
                (Some(user), Some(owner)) => user == owner,
                _ => false,
            }
    }

    /// Whether a site is visible: public, owned by the caller, or the caller
    /// holds a site permission. That is the `read` rule upstream grants on
    /// `Omeka\Entity\Site` (`AclFactory::addRulesForAllRoles`), whose assertion
    /// is `SiteIsPublicAssertion | OwnsEntityAssertion | HasSitePermissionAssertion('viewer')`;
    /// the admin roles are allowed everything separately. The caller reads
    /// `has_permission` from `site_permission`.
    pub fn sees_site(&self, is_public: bool, owner: Option<UserId>, has_permission: bool) -> bool {
        self.is_admin() || is_public || self.owns(owner) || has_permission
    }

    /// Whether a site page is visible. Upstream's `SitePageVisibilityFilter`
    /// keeps a page when its site and the page are both public, or the caller
    /// owns the site, or the caller has a site permission; `view-all` on
    /// `Omeka\Entity\SitePage` (the admin roles) bypasses the filter. A page
    /// has no owner of its own, so the owner test reads the site's owner.
    pub fn sees_site_page(
        &self,
        page_is_public: bool,
        site_is_public: bool,
        site_owner: Option<UserId>,
        has_permission: bool,
    ) -> bool {
        self.is_admin()
            || (page_is_public && site_is_public)
            || self.owns(site_owner)
            || has_permission
    }

    /// Whether the actor is the row's owner.
    fn owns(&self, owner: Option<UserId>) -> bool {
        matches!((self.user, owner), (Some(user), Some(owner)) if user == owner)
    }
}

/// Authenticate the caller from the `key_identity` / `key_credential` query
/// parameters against the `api_key` table, running the credential check every
/// time.
///
/// This is the uncached path, for a caller that authenticates once.
/// [`authenticate_with`] is the server's entry point.
pub async fn authenticate<'a, I>(pool: &DbPool, pairs: I) -> Result<Actor, ApiError>
where
    I: IntoIterator<Item = (&'a str, &'a str)>,
{
    authenticate_with(pool, &Bcrypt, pairs).await
}

/// Authenticate the caller, asking `verifier` about the credential.
///
/// Mirrors `Authentication\Adapter\KeyAdapter::authenticate`: the key row must
/// exist, its owner must be active, and the credential must verify against the
/// stored bcrypt hash. Any failure returns the anonymous actor, because
/// `authenticateApiKey` discards the authentication result.
///
/// The key row and the owner row are read on every call. A verifier that
/// remembers past verdicts therefore cannot outlive a deleted key or a
/// deactivated owner; it only replaces the credential check.
pub async fn authenticate_with<'a, I>(
    pool: &DbPool,
    verifier: &impl CredentialVerifier,
    pairs: I,
) -> Result<Actor, ApiError>
where
    I: IntoIterator<Item = (&'a str, &'a str)>,
{
    let Some(credentials) = KeyCredentials::from_pairs(pairs) else {
        return Ok(Actor::anonymous());
    };
    if !credentials.identity_is_well_formed() {
        return Ok(Actor::anonymous());
    }
    let Some(key) = with_db!(pool, |db| ApiKeyRepo::new(db)
        .get(credentials.identity())
        .await)
    .map_err(ApiError::internal)?
    else {
        return Ok(Actor::anonymous());
    };
    let Some(owner) = with_db!(pool, |db| UserRepo::new(db).get(key.owner_id).await)
        .map_err(ApiError::internal)?
    else {
        return Ok(Actor::anonymous());
    };
    if !owner.is_active {
        return Ok(Actor::anonymous());
    }
    if !verifier.verify(
        credentials.identity(),
        credentials.credential(),
        &key.credential_hash,
    ) {
        return Ok(Actor::anonymous());
    }
    // `KeyAdapter` records the use; the caller's address is not threaded
    // through this layer, so only the timestamp moves.
    let _ = with_db!(pool, |db| ApiKeyRepo::new(db)
        .touch(credentials.identity(), None, None)
        .await);
    Ok(Actor::new(owner.id, owner.role))
}

/// Percent-decode one name or value (`%41` -> `A`, `+` -> space).
///
/// A truncated or non-hex escape is kept literal, which is what PHP's decoder
/// does for malformed input in practice.
pub(crate) fn decode(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => match (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                (Some(hi), Some(lo)) => {
                    out.push(hi << 4 | lo);
                    i += 3;
                }
                _ => {
                    out.push(b'%');
                    i += 1;
                }
            },
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn identity_must_be_32_alphanumerics() {
        assert!(identity_is_well_formed(&"a1".repeat(16)));
        assert!(identity_is_well_formed(&"a1".repeat(16).to_uppercase()));
        assert!(!identity_is_well_formed("short"));
        assert!(!identity_is_well_formed(&"a1".repeat(16).replace('a', "!")));
        assert!(!identity_is_well_formed(&"x".repeat(33)));
    }

    #[test]
    fn privileged_roles_see_every_resource() {
        let actor = Actor::new(UserId(1), "editor");
        assert!(actor.is_privileged());
        assert!(actor.sees_resource(false, Some(UserId(2))));
    }

    #[test]
    fn an_anonymous_actor_sees_only_public_resources() {
        let actor = Actor::anonymous();
        assert!(!actor.is_privileged());
        assert!(actor.sees_resource(true, None));
        assert!(!actor.sees_resource(false, None));
    }

    #[test]
    fn an_author_sees_their_own_private_resource() {
        let actor = Actor::new(UserId(7), "author");
        assert!(!actor.is_privileged());
        assert!(actor.sees_resource(false, Some(UserId(7))));
        assert!(!actor.sees_resource(false, Some(UserId(8))));
    }

    #[test]
    fn a_page_needs_a_public_site_and_page_or_a_site_role() {
        let anonymous = Actor::anonymous();
        assert!(anonymous.sees_site_page(true, true, None, false));
        assert!(!anonymous.sees_site_page(true, false, None, false));
        assert!(!anonymous.sees_site_page(false, true, None, false));

        let owner = Actor::new(UserId(7), "author");
        assert!(owner.sees_site_page(false, false, Some(UserId(7)), false));
        assert!(owner.sees_site_page(false, true, Some(UserId(7)), false));
        assert!(!owner.sees_site_page(false, true, Some(UserId(8)), false));
        assert!(owner.sees_site_page(false, false, Some(UserId(8)), true));

        // `editor` holds `view-all` on `Omeka\Entity\Resource`, not on
        // `Omeka\Entity\SitePage`, so it is not privileged here.
        assert!(!Actor::new(UserId(9), "editor").sees_site_page(false, false, None, false));
        assert!(Actor::new(UserId(9), "global_admin").sees_site_page(false, false, None, false));
        assert!(Actor::new(UserId(9), "site_admin").sees_site_page(false, false, None, false));
    }

    #[test]
    fn a_site_needs_public_ownership_or_a_permission() {
        let anonymous = Actor::anonymous();
        assert!(anonymous.sees_site(true, None, false));
        assert!(!anonymous.sees_site(false, Some(UserId(7)), false));

        let author = Actor::new(UserId(7), "author");
        assert!(author.sees_site(false, Some(UserId(7)), false));
        assert!(author.sees_site(false, Some(UserId(8)), true));
        assert!(!author.sees_site(false, Some(UserId(8)), false));

        assert!(!Actor::new(UserId(9), "editor").sees_site(false, None, false));
        assert!(Actor::new(UserId(9), "site_admin").sees_site(false, None, false));
    }

    #[test]
    fn either_parameter_missing_means_no_attempt() {
        assert_eq!(KeyCredentials::from_query("key_identity=abc"), None);
        assert_eq!(KeyCredentials::from_query("key_credential=abc"), None);
        assert_eq!(KeyCredentials::from_query(""), None);
    }

    #[test]
    fn both_parameters_parse_in_any_order() {
        let creds = KeyCredentials::from_query("key_credential=c&key_identity=i").unwrap();
        assert_eq!(creds.identity(), "i");
        assert_eq!(creds.credential(), "c");
    }

    #[test]
    fn last_duplicate_wins_like_parse_str() {
        let creds =
            KeyCredentials::from_query("key_identity=first&key_identity=second&key_credential=c")
                .unwrap();
        assert_eq!(creds.identity(), "second");
    }

    #[test]
    fn percent_escapes_and_plus_decode() {
        let creds = KeyCredentials::from_query("key_identity=%41%42&key_credential=a+b").unwrap();
        assert_eq!(creds.identity(), "AB");
        assert_eq!(creds.credential(), "a b");
    }

    #[test]
    fn malformed_escape_is_kept_literal() {
        let creds = KeyCredentials::from_query("key_identity=a%zz&key_credential=c").unwrap();
        assert_eq!(creds.identity(), "a%zz");
        assert!(!creds.identity_is_well_formed());
    }
}
