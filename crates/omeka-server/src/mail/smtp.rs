//! The SMTP transport: a `lettre` relay, selected by `OMEKA_SMTP_HOST`.
//!
//! Upstream takes one of Laminas' transports from `mail.transport`, and the
//! port's equivalent is the setting set in [`crate::config::MailConfig`]:
//! `OMEKA_SMTP_HOST` with `OMEKA_SMTP_PORT`, `OMEKA_SMTP_USER`,
//! `OMEKA_SMTP_PASSWORD`, `OMEKA_SMTP_TLS` and `OMEKA_MAIL_FROM`.
//!
//! The TLS mode picks the builder. `starttls` connects in the clear on port 587
//! and requires the STARTTLS command before it will send, which is
//! `SmtpTransport::starttls_relay`; `tls` wraps the connection from the first
//! byte on port 465, which is `SmtpTransport::relay`; `none` speaks plain SMTP
//! on port 25, which is `SmtpTransport::builder_dangerous` and is the mode a
//! local relay on the same host takes.
//!
//! Only `from_config` runs at startup. It builds a client and a certificate
//! verifier, and dials nothing: the connection happens on the first send.

use lettre::message::Mailbox;
use lettre::message::header::ContentType;
use lettre::transport::smtp::SmtpTransport;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message as LettreMessage, Transport as LettreTransport};

use crate::config::{MailConfig, MailTls};

use super::{Delivery, MailError, Message, Transport, TransportKind};

/// How long one SMTP command may take before the send fails.
///
/// `lettre`'s default is 60 seconds, which pins a blocking thread for a minute
/// when a relay stops answering. Ten seconds is longer than any relay needs for
/// a command and short enough that a broken relay does not hold a request open.
const COMMAND_TIMEOUT_SECS: u64 = 10;

/// A relay a `lettre` client talks to.
#[derive(Clone)]
pub struct Smtp {
    transport: SmtpTransport,
}

impl Smtp {
    /// The relay `config` names, with its credentials and its TLS mode.
    ///
    /// # Errors
    /// A [`MailError`] when the TLS client cannot be built, which is what a
    /// system with no readable certificate store produces.
    pub fn from_config(config: &MailConfig, host: &str) -> Result<Self, MailError> {
        let builder = match config.tls {
            MailTls::Starttls => SmtpTransport::starttls_relay(host),
            MailTls::Tls => SmtpTransport::relay(host),
            // `builder_dangerous` is safe here because it only means "no TLS
            // configured"; `none` says the operator asked for that.
            MailTls::None => Ok(SmtpTransport::builder_dangerous(host)),
        }
        .map_err(|error| MailError::Smtp(error.to_string()))?;

        let builder = builder
            .port(config.port())
            .timeout(Some(std::time::Duration::from_secs(COMMAND_TIMEOUT_SECS)));
        let builder = match (&config.user, &config.password) {
            (Some(user), Some(password)) => {
                builder.credentials(Credentials::new(user.clone(), password.clone()))
            }
            _ => builder,
        };
        Ok(Self {
            transport: builder.build(),
        })
    }
}

impl Transport for Smtp {
    fn kind(&self) -> TransportKind {
        TransportKind::Smtp
    }

    fn send(&self, message: &Message) -> Result<Delivery, MailError> {
        let sender: Mailbox = message
            .from
            .parse()
            .map_err(|error| MailError::Smtp(format!("invalid from address: {error}")))?;
        let recipient: Mailbox = format!("{} <{}>", message.name, message.to)
            .parse::<Mailbox>()
            .map_err(|error| MailError::Recipient(error.to_string()))?;
        let email = LettreMessage::builder()
            .from(sender)
            .to(recipient)
            .subject(message.subject.clone())
            .header(ContentType::TEXT_PLAIN)
            .body(message.body.clone())
            .map_err(|error| MailError::Smtp(error.to_string()))?;
        self.transport
            .send(&email)
            .map_err(|error| MailError::Smtp(error.to_string()))?;
        Ok(Delivery::Sent)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn each_tls_mode_builds_a_transport_without_dialling() {
        for tls in [MailTls::Starttls, MailTls::Tls, MailTls::None] {
            let config = MailConfig {
                host: Some("smtp.example.org".to_string()),
                tls,
                ..MailConfig::default()
            };
            let smtp = Smtp::from_config(&config, "smtp.example.org")
                .unwrap_or_else(|error| panic!("{tls} builds a client: {error}"));
            assert_eq!(smtp.kind(), TransportKind::Smtp);
        }
    }
}
