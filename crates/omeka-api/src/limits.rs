//! Bounded attempt counters for the routes an anonymous caller can reach.
//!
//! Upstream has no rate limit anywhere. `LoginController::loginAction` runs
//! bcrypt against whatever address and whatever password arrives, and
//! `forgotPasswordAction` mints a row and sends a mail for every request that
//! names a matching address. Both are open to a password guesser and to a mail
//! amplifier, so the port adds counters in front of them.
//!
//! **Shape.** One entry per counted subject holds the instants of its attempts
//! inside the window. An attempt past the window is dropped on the next look, so
//! the window slides and no timer runs. An entry holds at most the subject's
//! limit, which bounds memory per entry, and the map holds at most
//! [`Limiter::capacity`] entries, evicting the least recently inserted one the
//! way [`crate::auth_cache::AuthCache`] does.
//!
//! **Window and bounds.** Five minutes. Five logins per email, a hundred per
//! client address, three forgot-password requests per email, thirty per
//! address. The email counters are the tight ones: five guesses per account per
//! window is what protects an account, and a login that succeeds clears its
//! email counter, so a person who mistypes twice and then gets it right starts
//! clean. The address counters are a throughput cap for one host, not an
//! account guard, so they are generous: a shared office address or a test
//! suite on loopback must not lock itself out, while a scripted guesser is held
//! to a hundred bcrypt checks per window per host.
//!
//! **Failure mode.** The counters live in the process, so a restart forgets
//! them and two processes count separately. That is deliberate: the alternative
//! is a shared table and a write on every attempt, which is what the counters
//! exist to avoid. A restart is not the attack.
//!
//! **Address.** A caller behind a proxy is counted by the address the proxy
//! reports; see `omeka-server`'s `client_address`. When no address can be
//! established the address counter is skipped rather than shared by every
//! caller, so an unproxied deployment keeps the email counters only.

use std::collections::VecDeque;
use std::net::IpAddr;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use indexmap::IndexMap;

/// The sliding window every shipped policy uses.
pub const WINDOW: Duration = Duration::from_secs(300);
/// Logins allowed per email address in [`WINDOW`].
pub const LOGIN_PER_EMAIL: u32 = 5;
/// Logins allowed per client address in [`WINDOW`].
pub const LOGIN_PER_ADDRESS: u32 = 100;
/// Forgot-password requests allowed per email in [`WINDOW`].
pub const FORGOT_PER_EMAIL: u32 = 3;
/// Forgot-password requests allowed per client address in [`WINDOW`].
pub const FORGOT_PER_ADDRESS: u32 = 30;
/// Subjects one limiter remembers before it evicts the oldest.
pub const CAPACITY: usize = 4096;

/// What one subject may do inside one window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Policy {
    /// Attempts allowed in the window. Zero refuses every attempt.
    pub limit: u32,
    pub window: Duration,
}

impl Policy {
    /// A policy of `limit` attempts per [`WINDOW`].
    #[must_use]
    pub const fn per_window(limit: u32) -> Self {
        Self {
            limit,
            window: WINDOW,
        }
    }
}

/// The shipped limits, one policy per counted subject.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateLimits {
    pub login_email: Policy,
    pub login_address: Policy,
    pub forgot_email: Policy,
    pub forgot_address: Policy,
}

impl Default for RateLimits {
    fn default() -> Self {
        Self {
            login_email: Policy::per_window(LOGIN_PER_EMAIL),
            login_address: Policy::per_window(LOGIN_PER_ADDRESS),
            forgot_email: Policy::per_window(FORGOT_PER_EMAIL),
            forgot_address: Policy::per_window(FORGOT_PER_ADDRESS),
        }
    }
}

impl RateLimits {
    fn policy(&self, subject: &Subject) -> Policy {
        match subject {
            Subject::LoginEmail(_) => self.login_email,
            Subject::LoginAddress(_) => self.login_address,
            Subject::ForgotEmail(_) => self.forgot_email,
            Subject::ForgotAddress(_) => self.forgot_address,
        }
    }
}

/// The thing an attempt is counted against.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Subject {
    LoginEmail(String),
    LoginAddress(IpAddr),
    ForgotEmail(String),
    ForgotAddress(IpAddr),
}

/// What one attempt may do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Allowed,
    Refused { retry_after_secs: u64 },
}

/// The counters for one process.
///
/// Cloning shares the counters, so the server state holds one and a test can
/// hold another view of it.
#[derive(Debug, Clone)]
pub struct Limiter {
    limits: RateLimits,
    capacity: usize,
    entries: Arc<Mutex<IndexMap<Subject, VecDeque<Instant>>>>,
}

impl Limiter {
    /// A limiter with the shipped limits and [`CAPACITY`] subjects.
    #[must_use]
    pub fn new(limits: RateLimits) -> Self {
        Self::with_capacity(limits, CAPACITY)
    }

    /// A limiter that remembers at most `capacity` subjects.
    ///
    /// A capacity of zero would forget every attempt immediately, so it is
    /// raised to one.
    #[must_use]
    pub fn with_capacity(limits: RateLimits, capacity: usize) -> Self {
        Self {
            limits,
            capacity: capacity.max(1),
            entries: Arc::new(Mutex::new(IndexMap::new())),
        }
    }

    /// The policies this limiter enforces.
    #[must_use]
    pub fn limits(&self) -> &RateLimits {
        &self.limits
    }

    /// Subjects this limiter remembers now.
    #[must_use]
    pub fn len(&self) -> usize {
        self.lock().len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Count one attempt against every subject, or refuse it.
    ///
    /// A refusal counts nothing, so a caller that keeps knocking is answered
    /// from the attempts that are already there and the window keeps its
    /// original end.
    pub fn admit(&self, subjects: &[Subject], now: Instant) -> Verdict {
        let mut entries = self.lock();
        let mut retry_after_secs = 0;
        for subject in subjects {
            let policy = self.limits.policy(subject);
            let mut attempts = entries.get_mut(subject);
            let live = attempts
                .as_mut()
                .map(|queue| {
                    prune(queue, policy, now);
                    queue.len()
                })
                .unwrap_or(0);
            if policy.limit == 0 || live >= policy.limit as usize {
                let waited = attempts
                    .and_then(|queue| queue.front())
                    .map_or(Duration::ZERO, |first| {
                        now.saturating_duration_since(*first)
                    });
                retry_after_secs = retry_after_secs.max(retry_after(policy.window, waited));
            }
        }
        if retry_after_secs > 0 {
            return Verdict::Refused { retry_after_secs };
        }
        for subject in subjects {
            entries.entry(subject.clone()).or_default().push_back(now);
        }
        while entries.len() > self.capacity {
            entries.shift_remove_index(0);
        }
        Verdict::Allowed
    }

    /// Forget one subject's attempts, as a successful login does.
    pub fn reset(&self, subject: &Subject) {
        self.lock().shift_remove(subject);
    }

    fn lock(&self) -> MutexGuard<'_, IndexMap<Subject, VecDeque<Instant>>> {
        // An entry is only ever pushed to or pruned, and both are one call, so
        // a panic while the lock is held cannot leave a half-updated entry.
        self.entries
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }
}

/// Drop the attempts that have left the window. They are in order, so the front
/// is the only one that can have.
fn prune(attempts: &mut VecDeque<Instant>, policy: Policy, now: Instant) {
    while attempts
        .front()
        .is_some_and(|first| now.saturating_duration_since(*first) >= policy.window)
    {
        attempts.pop_front();
    }
}

/// Whole seconds until the window moves past `elapsed`, never zero.
fn retry_after(window: Duration, elapsed: Duration) -> u64 {
    let remaining = window.saturating_sub(elapsed);
    if remaining.subsec_nanos() > 0 {
        remaining.as_secs() + 1
    } else {
        remaining.as_secs().max(1)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    fn address(last: u8) -> Subject {
        Subject::LoginAddress(IpAddr::from([127, 0, 0, last]))
    }

    fn email(address: &str) -> Subject {
        Subject::LoginEmail(address.to_string())
    }

    /// The subject list of one counted subject.
    fn one(subject: &Subject) -> &[Subject] {
        std::slice::from_ref(subject)
    }

    fn short(limit: u32) -> Policy {
        Policy {
            limit,
            window: Duration::from_millis(50),
        }
    }

    #[test]
    fn the_shipped_windows_and_bounds_are_the_documented_ones() {
        let limits = RateLimits::default();
        assert_eq!(limits.login_email, Policy::per_window(5));
        assert_eq!(limits.login_address, Policy::per_window(100));
        assert_eq!(limits.forgot_email, Policy::per_window(3));
        assert_eq!(limits.forgot_address, Policy::per_window(30));
        assert_eq!(WINDOW, Duration::from_secs(300));
        assert_eq!(CAPACITY, 4096);
    }

    #[test]
    fn the_login_limit_trips_then_releases() {
        let limiter = Limiter::new(RateLimits {
            login_email: short(3),
            ..RateLimits::default()
        });
        let start = Instant::now();
        let subject = email("person@example.org");
        for attempt in 0..3 {
            assert_eq!(
                limiter.admit(one(&subject), start),
                Verdict::Allowed,
                "attempt {attempt} is inside the limit"
            );
        }
        match limiter.admit(one(&subject), start) {
            Verdict::Refused { retry_after_secs } => assert_eq!(retry_after_secs, 1),
            other => panic!("the fourth attempt should be refused, got {other:?}"),
        }
        // A refused attempt is not recorded, so re-asking is refused with the
        // same answer and the window still ends where it did.
        assert!(matches!(
            limiter.admit(one(&subject), start + Duration::from_millis(40)),
            Verdict::Refused {
                retry_after_secs: 1
            }
        ));
        assert_eq!(
            limiter.admit(&[subject], start + Duration::from_millis(50)),
            Verdict::Allowed,
            "the window has moved past the first attempt"
        );
    }

    #[test]
    fn a_successful_login_clears_the_email_counter() {
        let limiter = Limiter::new(RateLimits {
            login_email: short(1),
            ..RateLimits::default()
        });
        let start = Instant::now();
        let subject = email("person@example.org");
        assert_eq!(limiter.admit(one(&subject), start), Verdict::Allowed);
        assert!(matches!(
            limiter.admit(one(&subject), start),
            Verdict::Refused { .. }
        ));
        limiter.reset(&subject);
        assert_eq!(limiter.admit(&[subject], start), Verdict::Allowed);
    }

    #[test]
    fn each_subject_carries_its_own_count() {
        let limiter = Limiter::new(RateLimits {
            login_email: short(1),
            login_address: short(2),
            ..RateLimits::default()
        });
        let start = Instant::now();
        let first = email("one@example.org");
        let second = email("two@example.org");
        let client = address(9);

        assert_eq!(
            limiter.admit(&[first.clone(), client.clone()], start),
            Verdict::Allowed
        );
        assert_eq!(
            limiter.admit(&[second.clone(), client.clone()], start),
            Verdict::Allowed,
            "another email has an empty counter of its own"
        );
        assert!(
            matches!(
                limiter.admit(&[first, client.clone()], start),
                Verdict::Refused { .. }
            ),
            "the first email is over its limit though the address is not"
        );
        assert!(
            matches!(
                limiter.admit(&[second, client], start),
                Verdict::Refused { .. }
            ),
            "the address has now reached its own limit"
        );
    }

    #[tokio::test]
    async fn a_refused_subject_stops_the_whole_attempt_from_counting() {
        let limiter = Limiter::new(RateLimits {
            login_email: short(1),
            login_address: short(3),
            ..RateLimits::default()
        });
        let start = Instant::now();
        let client = address(9);
        assert_eq!(
            limiter.admit(&[email("one@example.org"), client.clone()], start),
            Verdict::Allowed
        );
        assert!(matches!(
            limiter.admit(&[email("one@example.org"), client.clone()], start),
            Verdict::Refused { .. }
        ));
        // The address counted nothing for the refused attempt, so two of its
        // three attempts are still there.
        for attempt in 0..2 {
            assert_eq!(
                limiter.admit(
                    &[email(&format!("more{attempt}@example.org")), client.clone()],
                    start
                ),
                Verdict::Allowed,
                "attempt {attempt} is inside the address limit"
            );
        }
        assert!(
            matches!(
                limiter.admit(&[email("fourth@example.org"), client], start),
                Verdict::Refused { .. }
            ),
            "the address has now reached its own limit"
        );
    }

    #[test]
    fn the_limiter_evicts_the_oldest_subject_past_its_capacity() {
        let limiter = Limiter::with_capacity(
            RateLimits {
                login_email: short(1),
                ..RateLimits::default()
            },
            2,
        );
        let start = Instant::now();
        for (index, name) in ["one", "two", "three"].iter().enumerate() {
            assert_eq!(
                limiter.admit(&[email(name)], start + Duration::from_millis(index as u64)),
                Verdict::Allowed
            );
        }
        assert_eq!(limiter.len(), 2, "the map stays bounded");
        assert_eq!(
            limiter.admit(&[email("one")], start + Duration::from_millis(3)),
            Verdict::Allowed,
            "the oldest subject was evicted, so it starts over"
        );
    }

    #[test]
    fn a_zero_limit_refuses_every_attempt() {
        let limiter = Limiter::new(RateLimits {
            forgot_email: short(0),
            ..RateLimits::default()
        });
        assert!(matches!(
            limiter.admit(
                &[Subject::ForgotEmail("person@example.org".to_string())],
                Instant::now()
            ),
            Verdict::Refused { .. }
        ));
    }

    #[test]
    fn attempts_older_than_the_window_are_forgotten() {
        let limiter = Limiter::new(RateLimits {
            login_email: short(2),
            ..RateLimits::default()
        });
        let start = Instant::now();
        let subject = email("person@example.org");
        assert_eq!(limiter.admit(one(&subject), start), Verdict::Allowed);
        assert_eq!(
            limiter.admit(one(&subject), start + Duration::from_millis(50)),
            Verdict::Allowed,
            "the first attempt left the window before the second arrived"
        );
    }
}
