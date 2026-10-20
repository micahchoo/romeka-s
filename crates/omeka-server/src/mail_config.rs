//! The mail settings: how the SMTP transport protects the connection, and the
//! host, credentials and sender a deployment sets.

/// Envelope sender when nothing overrides it.
pub const DEFAULT_MAIL_FROM: &str = "no-reply@romeka.invalid";

/// How the SMTP transport protects the connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MailTls {
    /// Connect in the clear, then upgrade with the STARTTLS command
    /// (`SmtpTransport::starttls_relay`).
    #[default]
    Starttls,
    /// Connect over TLS from the first byte (`SmtpTransport::relay`).
    Tls,
    /// No TLS at all (`SmtpTransport::builder_dangerous`).
    None,
}

impl MailTls {
    /// The port this mode implies when none is configured: the submission port
    /// for STARTTLS, the submissions port for implicit TLS, and SMTP for none.
    #[must_use]
    pub fn default_port(self) -> u16 {
        match self {
            Self::Starttls => 587,
            Self::Tls => 465,
            Self::None => 25,
        }
    }
}

impl std::fmt::Display for MailTls {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Starttls => "starttls",
            Self::Tls => "tls",
            Self::None => "none",
        })
    }
}

impl std::str::FromStr for MailTls {
    type Err = String;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        match raw.to_ascii_lowercase().as_str() {
            "starttls" => Ok(Self::Starttls),
            "tls" => Ok(Self::Tls),
            "none" => Ok(Self::None),
            other => Err(format!("expected starttls, tls or none, got {other}")),
        }
    }
}

/// The mail settings, from `ROMEKA_SMTP_*` and `ROMEKA_MAIL_FROM`.
///
/// A `host` is what turns mail on: with none, the server uses the log transport
/// and every message goes to the log, which is the default because a deployment
/// with no relay must still run, and the link is returned to the caller either
/// way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MailConfig {
    pub host: Option<String>,
    pub port: Option<u16>,
    pub user: Option<String>,
    pub password: Option<String>,
    pub tls: MailTls,
    /// Envelope sender; [`DEFAULT_MAIL_FROM`] when nothing sets it.
    pub from: String,
}

impl Default for MailConfig {
    fn default() -> Self {
        Self {
            host: None,
            port: None,
            user: None,
            password: None,
            tls: MailTls::Starttls,
            from: DEFAULT_MAIL_FROM.to_string(),
        }
    }
}

impl MailConfig {
    /// The port the transport connects to.
    #[must_use]
    pub fn port(&self) -> u16 {
        self.port.unwrap_or_else(|| self.tls.default_port())
    }
}
