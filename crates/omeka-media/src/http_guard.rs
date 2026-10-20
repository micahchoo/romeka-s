//! The guard every outbound fetch passes through (docs/import-v2.md
//! "Arrival", "URL sources").
//!
//! Upstream fetches any URL a user names, so a media create or an import sheet
//! can make the server read its own network: a loopback admin port, a LAN
//! host, the cloud metadata address `169.254.169.254`. The port refuses those
//! targets. [`crate::http_client::HttpClient`] asks the guard to admit each
//! hop, redirects included, and connects only to the addresses the guard
//! returns, so a name that resolves to a public address for the check and a
//! private one a moment later (DNS rebinding) never gets a second lookup.
//!
//! The rules:
//!
//! - `http` and `https` only, and no credentials in the URL.
//! - The host resolves once, through [`Resolve`], and every address it
//!   resolves to must fall outside [`RefusedRange`]. An address literal is
//!   checked without a lookup.
//! - [`AddressPolicy::AllowPrivate`], the operator's
//!   `import_allow_private_urls`, lifts the address check and nothing else.

use std::fmt;
use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::Duration;

use crate::http::{FetchError, FetchErrorKind};

/// Turns a host name into the addresses a fetch may connect to.
///
/// The seam exists so a test can answer a name with a chosen address and
/// needs no DNS. Production uses [`SystemResolver`].
pub trait Resolve: Send + Sync + 'static {
    /// Every address `host` resolves to. An empty answer means the host does
    /// not resolve.
    ///
    /// # Errors
    /// The lookup failure.
    fn resolve(&self, host: &str) -> io::Result<Vec<IpAddr>>;
}

/// The operating system's resolver.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemResolver;

impl Resolve for SystemResolver {
    fn resolve(&self, host: &str) -> io::Result<Vec<IpAddr>> {
        // The port is a placeholder: `ToSocketAddrs` needs one, and the guard
        // pairs each address with the URL's own port afterwards.
        Ok((host, 0)
            .to_socket_addrs()?
            .map(|address| address.ip())
            .collect())
    }
}

/// Whether the address check applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AddressPolicy {
    /// Refuse every address in a [`RefusedRange`]. The default.
    #[default]
    PublicOnly,
    /// Admit private addresses too: the operator set
    /// `import_allow_private_urls`, for an install that fetches from its own
    /// network on purpose.
    AllowPrivate,
}

/// A range of addresses a fetch may not reach.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefusedRange {
    /// `127.0.0.0/8` and `::1`.
    Loopback,
    /// `169.254.0.0/16`, the cloud metadata address among them, and
    /// `fe80::/10`.
    LinkLocal,
    /// The RFC 1918 ranges `10/8`, `172.16/12` and `192.168/16`.
    Private,
    /// The carrier-grade NAT range `100.64.0.0/10` (RFC 6598).
    SharedAddressSpace,
    /// The IPv6 unique local range `fc00::/7`.
    UniqueLocal,
    /// `0.0.0.0/8` and `::`, which Linux connects to the local host.
    Unspecified,
    /// `224.0.0.0/4` and `ff00::/8`.
    Multicast,
    /// `255.255.255.255`.
    Broadcast,
}

impl fmt::Display for RefusedRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Loopback => "loopback",
            Self::LinkLocal => "link-local",
            Self::Private => "private",
            Self::SharedAddressSpace => "shared (CGNAT)",
            Self::UniqueLocal => "unique local",
            Self::Unspecified => "unspecified",
            Self::Multicast => "multicast",
            Self::Broadcast => "broadcast",
        })
    }
}

/// The refused range `ip` falls in, or `None` for an address a fetch may
/// reach. An IPv4-mapped (`::ffff:a.b.c.d`) or IPv4-compatible (`::a.b.c.d`)
/// address is judged as the IPv4 address it carries.
#[must_use]
pub fn refused_range(ip: IpAddr) -> Option<RefusedRange> {
    match ip {
        IpAddr::V4(v4) => refused_v4(v4),
        IpAddr::V6(v6) => refused_v6(v6),
    }
}

fn refused_v4(ip: Ipv4Addr) -> Option<RefusedRange> {
    let [a, b, ..] = ip.octets();
    if ip.is_loopback() {
        Some(RefusedRange::Loopback)
    } else if ip.is_link_local() {
        Some(RefusedRange::LinkLocal)
    } else if ip.is_private() {
        Some(RefusedRange::Private)
    } else if a == 100 && (b & 0b1100_0000) == 64 {
        Some(RefusedRange::SharedAddressSpace)
    } else if a == 0 {
        Some(RefusedRange::Unspecified)
    } else if ip.is_multicast() {
        Some(RefusedRange::Multicast)
    } else if ip.is_broadcast() {
        Some(RefusedRange::Broadcast)
    } else {
        None
    }
}

fn refused_v6(ip: Ipv6Addr) -> Option<RefusedRange> {
    let first = ip.segments()[0];
    if ip.is_loopback() {
        Some(RefusedRange::Loopback)
    } else if ip.is_unspecified() {
        Some(RefusedRange::Unspecified)
    } else if let Some(v4) = ip.to_ipv4() {
        // `to_ipv4` reads both the mapped and the compatible form; `::1` and
        // `::` are judged above, before it would read them as `0.0.0.x`.
        refused_v4(v4)
    } else if (first & 0xffc0) == 0xfe80 {
        Some(RefusedRange::LinkLocal)
    } else if (first & 0xfe00) == 0xfc00 {
        Some(RefusedRange::UniqueLocal)
    } else if ip.is_multicast() {
        Some(RefusedRange::Multicast)
    } else {
        None
    }
}

/// The check one hop of a fetch passes before it connects.
#[derive(Clone)]
pub(crate) struct UrlGuard {
    resolver: Arc<dyn Resolve>,
    policy: AddressPolicy,
    /// A test's stand-in for the internet: each checked public address is
    /// dialled at its loopback server instead, so a hop can pass the check and
    /// still reach a local socket.
    #[cfg(test)]
    pub(crate) dial_as: Vec<(SocketAddr, SocketAddr)>,
}

impl fmt::Debug for UrlGuard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UrlGuard")
            .field("policy", &self.policy)
            .finish_non_exhaustive()
    }
}

impl Default for UrlGuard {
    fn default() -> Self {
        Self {
            resolver: Arc::new(SystemResolver),
            policy: AddressPolicy::default(),
            #[cfg(test)]
            dial_as: Vec::new(),
        }
    }
}

impl UrlGuard {
    pub(crate) fn with_resolver(mut self, resolver: Arc<dyn Resolve>) -> Self {
        self.resolver = resolver;
        self
    }

    pub(crate) fn with_policy(mut self, policy: AddressPolicy) -> Self {
        self.policy = policy;
        self
    }

    /// Admit one hop: the addresses the client may connect to for `url`.
    ///
    /// The lookup gets at most `timeout`, because a resolver that hangs would
    /// otherwise outlast the fetch's deadline.
    ///
    /// # Errors
    /// [`FetchErrorKind::InvalidUrl`] for a scheme other than http(s) or a URL
    /// with no host, [`FetchErrorKind::Refused`] for credentials or a refused
    /// address, [`FetchErrorKind::Connect`] for a host that does not resolve,
    /// [`FetchErrorKind::Timeout`] for a lookup that passed `timeout`.
    pub(crate) fn admit(
        &self,
        url: &url::Url,
        timeout: Duration,
    ) -> Result<Vec<SocketAddr>, FetchError> {
        if !matches!(url.scheme(), "http" | "https") {
            return Err(FetchError::with_kind(
                FetchErrorKind::InvalidUrl,
                format!("unsupported scheme {} in {url}", url.scheme()),
            ));
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(FetchError::with_kind(
                FetchErrorKind::Refused,
                format!("refused {}: the URL carries credentials", redacted(url)),
            ));
        }
        let (Some(host), Some(port)) = (url.host(), url.port_or_known_default()) else {
            return Err(FetchError::with_kind(
                FetchErrorKind::InvalidUrl,
                format!("no host in {url}"),
            ));
        };
        let addresses = match host {
            url::Host::Ipv4(ip) => vec![IpAddr::V4(ip)],
            url::Host::Ipv6(ip) => vec![IpAddr::V6(ip)],
            url::Host::Domain(name) => self.lookup(name, timeout)?,
        };
        if addresses.is_empty() {
            return Err(FetchError::with_kind(
                FetchErrorKind::Connect,
                format!("{host} does not resolve"),
            ));
        }
        if self.policy == AddressPolicy::PublicOnly {
            for ip in &addresses {
                if let Some(range) = refused_range(*ip) {
                    let target = match host {
                        url::Host::Domain(name) => format!("{name} resolves to {ip}"),
                        _ => ip.to_string(),
                    };
                    return Err(FetchError::with_kind(
                        FetchErrorKind::Refused,
                        format!("refused {url}: {target}, a {range} address"),
                    ));
                }
            }
        }
        Ok(addresses
            .into_iter()
            .map(|ip| self.dial(SocketAddr::new(ip, port)))
            .collect())
    }

    #[cfg(not(test))]
    #[allow(clippy::unused_self)]
    fn dial(&self, address: SocketAddr) -> SocketAddr {
        address
    }

    #[cfg(test)]
    fn dial(&self, address: SocketAddr) -> SocketAddr {
        self.dial_as
            .iter()
            .find(|(public, _)| *public == address)
            .map_or(address, |(_, local)| *local)
    }

    /// Resolve on a thread of its own, so a hung lookup gives the fetch back
    /// at `timeout`. The thread finishes on its own when the lookup does.
    fn lookup(&self, host: &str, timeout: Duration) -> Result<Vec<IpAddr>, FetchError> {
        let (sender, receiver) = mpsc::channel();
        let resolver = Arc::clone(&self.resolver);
        let name = host.to_string();
        thread::spawn(move || {
            let _ = sender.send(resolver.resolve(&name));
        });
        match receiver.recv_timeout(timeout) {
            Ok(Ok(addresses)) => Ok(addresses),
            Ok(Err(error)) => Err(FetchError::with_kind(
                FetchErrorKind::Connect,
                format!("{host} does not resolve: {error}"),
            )),
            Err(_) => Err(FetchError::with_kind(
                FetchErrorKind::Timeout,
                format!("resolving {host} timed out"),
            )),
        }
    }
}

/// The URL without its credentials, for a message that may reach a log.
fn redacted(url: &url::Url) -> url::Url {
    let mut shown = url.clone();
    let _ = shown.set_username("");
    let _ = shown.set_password(None);
    shown
}
