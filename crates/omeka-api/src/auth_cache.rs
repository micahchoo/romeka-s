//! A per-process memo of API key credential checks.
//!
//! `Authentication\Adapter\KeyAdapter::authenticate` runs `password_verify`,
//! which is bcrypt, on every request that carries `key_identity` and
//! `key_credential`. bcrypt is deliberately slow, so the port pays that cost on
//! every authenticated request. [`AuthCache`] remembers the triples that already
//! verified so the second and later requests with the same key skip the check.
//!
//! The cache stores one 128-bit fingerprint per verified triple, computed with
//! the process's random hasher seed over the identity, the presented credential
//! and the stored credential hash. It never holds the credential and never holds
//! the stored hash, and the fingerprint is only comparable inside the process
//! that computed it, so it is not a verifier an attacker can test against.
//!
//! **Bound.** [`DEFAULT_CAPACITY`] entries per process, evicted in insertion
//! order. The entries are attacker-independent: only a credential that verified
//! is remembered, so an unknown identity cannot grow the cache.
//!
//! **Staleness.** None for the credential check. The stored hash is part of the
//! fingerprint, so a rotated `credential_hash` misses on the next request and
//! bcrypt runs again. A deleted key row or a deactivated owner stops a request
//! at once, because the key row and the owner row are read on every request and
//! the cache only replaces the bcrypt call.
//!
//! **Cost.** One mutex lock and one fingerprint per authenticated request,
//! against one bcrypt. The lock is held for a set lookup only.
//!
//! **Tradeoff.** The check is as strong as the fingerprint: a credential the
//! cache accepts without bcrypt is a forged fingerprint. A blind forgery needs a
//! 128-bit match, and each attempt pays the bcrypt it is trying to avoid.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hash, Hasher};
use std::sync::{Arc, Mutex, MutexGuard};

use indexmap::IndexSet;

/// Entries one process remembers before the oldest is evicted.
pub const DEFAULT_CAPACITY: usize = 256;

/// Whether one presented credential verifies against one stored hash.
///
/// The identity is passed because the verdict belongs to a key row: the cache
/// keys on it, [`Bcrypt`] ignores it.
pub trait CredentialVerifier: Send + Sync {
    fn verify(&self, identity: &str, credential: &str, credential_hash: &str) -> bool;
}

/// The production check: `password_verify`'s bcrypt, as
/// `Entity\ApiKey::verifyCredential` runs it.
#[derive(Debug, Clone, Copy, Default)]
pub struct Bcrypt;

impl CredentialVerifier for Bcrypt {
    fn verify(&self, _identity: &str, credential: &str, credential_hash: &str) -> bool {
        bcrypt::verify(credential, credential_hash).unwrap_or(false)
    }
}

/// Two keyed 64-bit halves of one fingerprint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Fingerprint(u64, u64);

/// An [`AuthCache`]'s entries, evicted in insertion order.
#[derive(Debug)]
struct Entries {
    set: IndexSet<Fingerprint, RandomState>,
    capacity: usize,
}

/// A bounded, process-local memo in front of a [`CredentialVerifier`].
///
/// Cloning shares the entries: the server holds one and every request clones it.
#[derive(Debug, Clone)]
pub struct AuthCache<V = Bcrypt> {
    verifier: V,
    seed: RandomState,
    entries: Arc<Mutex<Entries>>,
}

impl AuthCache<Bcrypt> {
    /// A cache with [`DEFAULT_CAPACITY`] entries in front of bcrypt.
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY)
    }

    /// A cache with room for `capacity` entries in front of bcrypt.
    pub fn with_capacity(capacity: usize) -> Self {
        Self::with_verifier(Bcrypt, capacity)
    }
}

impl Default for AuthCache {
    fn default() -> Self {
        Self::new()
    }
}

impl<V: CredentialVerifier> AuthCache<V> {
    /// A cache in front of `verifier` with room for `capacity` entries.
    ///
    /// A capacity of zero would never remember anything, so it is raised to one.
    pub fn with_verifier(verifier: V, capacity: usize) -> Self {
        Self {
            verifier,
            seed: RandomState::new(),
            entries: Arc::new(Mutex::new(Entries {
                set: IndexSet::default(),
                capacity: capacity.max(1),
            })),
        }
    }

    /// Entries this cache can hold.
    pub fn capacity(&self) -> usize {
        self.lock().capacity
    }

    /// Entries this cache holds now.
    pub fn len(&self) -> usize {
        self.lock().set.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn lock(&self) -> MutexGuard<'_, Entries> {
        // A panic while the lock is held cannot leave the set half-updated: the
        // lookup and the insert are single IndexSet calls.
        self.entries
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }

    fn fingerprint(&self, identity: &str, credential: &str, credential_hash: &str) -> Fingerprint {
        let mut low = self.seed.build_hasher();
        let mut high = self.seed.build_hasher();
        low.write_u8(0);
        high.write_u8(1);
        for hasher in [&mut low, &mut high] {
            identity.hash(hasher);
            credential.hash(hasher);
            credential_hash.hash(hasher);
        }
        Fingerprint(low.finish(), high.finish())
    }

    fn remember(&self, fingerprint: Fingerprint) {
        let mut entries = self.lock();
        if entries.set.insert(fingerprint) && entries.set.len() > entries.capacity {
            entries.set.shift_remove_index(0);
        }
    }
}

impl<V: CredentialVerifier> CredentialVerifier for AuthCache<V> {
    /// True when the triple verified before, otherwise the wrapped verdict,
    /// which is remembered when it is true.
    fn verify(&self, identity: &str, credential: &str, credential_hash: &str) -> bool {
        let fingerprint = self.fingerprint(identity, credential, credential_hash);
        if self.lock().set.contains(&fingerprint) {
            return true;
        }
        if !self.verifier.verify(identity, credential, credential_hash) {
            return false;
        }
        self.remember(fingerprint);
        true
    }
}
