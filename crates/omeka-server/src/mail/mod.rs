//! The mailer seam: one transport behind it, and the two messages upstream
//! sends.
//!
//! `Stdlib\Mailer` builds an activation or a reset message, hands it to the
//! configured Laminas transport, and gets a `PasswordCreation` row from the
//! entity manager on the way. The port splits that: the key is
//! `omeka_api::login`'s job, this module builds the message, and a [`Transport`]
//! sends it. The route returns the link to the caller either way, so an
//! installation with no SMTP relay stays usable.
//!
//! **Two transports.** [`Log`] is the default: it writes the whole message to
//! the request log, which is what a container deployment with no relay gets and
//! what a test asserts on. [`Smtp`](smtp::Smtp) hands the message to a relay
//! over `lettre`. [`Mailer::from_config`] picks one from the `ROMEKA_SMTP_*`
//! settings: a host means SMTP, no host means the log. A host whose TLS settings
//! cannot be built logs the error and falls back to the log transport, so a
//! misconfigured relay does not stop the server from starting.
//!
//! **Blocking sends.** `lettre`'s SMTP transport is synchronous, and an SMTP
//! conversation can take seconds, so [`Mailer::deliver`] runs the transport on
//! `tokio`'s blocking pool and the handler awaits it.

pub mod smtp;

use std::sync::Arc;

use omeka_api::jsonld::BaseUrl;
use omeka_db::rows::UserRow;
use thiserror::Error;

use crate::config::MailConfig;

/// The SPA route a set-password link points at, after the base URL.
///
/// The admin application is a hash-routed single page
/// (`web/admin/src/app/router.ts`), and the route the SPA renders is
/// `#/password/<key>`, so the link is `<base>/#/password/<key>`. The key is 32
/// characters of `[A-Za-z0-9]`, so no escaping is needed. This is the one piece
/// of the mail contract the server and the SPA share; the brief freezes the
/// routes, not the link.
pub const SET_PASSWORD_PATH: &str = "/#/password/";

/// A message ready for a transport.
///
/// The envelope sender travels with the message, so a transport that records or
/// forwards it has everything one message needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub from: String,
    pub to: String,
    pub name: String,
    pub subject: String,
    pub body: String,
}

/// What a transport did with a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delivery {
    /// An SMTP relay accepted it.
    Sent,
    /// The log transport wrote it; nothing left the process.
    Logged,
}

/// A transport that could not send.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MailError {
    #[error("the smtp relay refused the message: {0}")]
    Smtp(String),
    #[error("the message has no valid recipient: {0}")]
    Recipient(String),
}

/// Which transport a mailer holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportKind {
    Log,
    Smtp,
}

/// The seam: something that can accept a message.
pub trait Transport: Send + Sync + 'static {
    /// Which transport this is, for a caller that reports it.
    fn kind(&self) -> TransportKind;
    /// Hand the message over.
    ///
    /// # Errors
    /// A [`MailError`] when the message could not be handed over.
    fn send(&self, message: &Message) -> Result<Delivery, MailError>;
}

/// The default transport: the message goes to the log, and nowhere else.
///
/// Upstream's `file` transport writes to a file and its `smtp` transport needs
/// a relay. A container with no relay has neither, and a silently dropped
/// invitation is worse than a loud log line, so the port's fallback names the
/// recipient, the subject and the body, link included.
#[derive(Debug, Clone, Copy, Default)]
pub struct Log;

impl Transport for Log {
    fn kind(&self) -> TransportKind {
        TransportKind::Log
    }

    fn send(&self, message: &Message) -> Result<Delivery, MailError> {
        tracing::info!(
            target: "romeka::mail",
            from = %message.from,
            to = %message.to,
            subject = %message.subject,
            body = %message.body,
            "outgoing mail"
        );
        Ok(Delivery::Logged)
    }
}

/// The mailer: a transport, the envelope sender, and the base URL the links use.
#[derive(Clone)]
pub struct Mailer {
    transport: Arc<dyn Transport>,
    from: String,
    base: BaseUrl,
}

impl Mailer {
    /// The transport the configuration selects.
    ///
    /// A host means SMTP; no host means [`Log`]. `Smtp` is built once, here, so
    /// a request never pays for it.
    #[must_use]
    pub fn from_config(config: &MailConfig, base: BaseUrl) -> Self {
        let from = config.from.clone();
        match &config.host {
            None => Self::with_transport(Arc::new(Log), from, base),
            Some(host) => match smtp::Smtp::from_config(config, host) {
                Ok(transport) => Self::with_transport(Arc::new(transport), from, base),
                Err(error) => {
                    tracing::error!(
                        target: "romeka::mail",
                        %error,
                        "the smtp transport cannot be built; mail goes to the log"
                    );
                    Self::with_transport(Arc::new(Log), from, base)
                }
            },
        }
    }

    /// A mailer around a transport a caller supplies, which is how a test
    /// records the messages instead of sending them.
    #[must_use]
    pub fn with_transport(transport: Arc<dyn Transport>, from: String, base: BaseUrl) -> Self {
        Self {
            transport,
            from,
            base,
        }
    }

    /// Which transport this mailer holds.
    #[must_use]
    pub fn kind(&self) -> TransportKind {
        self.transport.kind()
    }

    /// The set-password link for one key.
    #[must_use]
    pub fn link(&self, key: &str) -> String {
        format!("{}{SET_PASSWORD_PATH}{key}", self.base.as_str())
    }

    /// `Mailer::sendResetPassword` as a message. `expires` is the string the
    /// body writes; see `omeka_api::login::IssuedKey::expires_display`.
    #[must_use]
    pub fn reset_message(
        &self,
        user: &UserRow,
        key: &str,
        installation_title: &str,
        expires: &str,
    ) -> Message {
        let body = format!(
            "Greetings, {}!\n\n\
             It seems you have forgotten your password for {installation_title} at {site}\n\n\
             To reset your password, click this link:\n{link}\n\n\
             Your reset link will expire on {expires}.",
            user.name,
            site = self.base.as_str(),
            link = self.link(key),
            expires = expires,
        );
        self.message(
            user,
            format!("Reset your password for {installation_title}"),
            body,
        )
    }

    /// `Mailer::sendUserActivation` as a message.
    #[must_use]
    pub fn activation_message(
        &self,
        user: &UserRow,
        key: &str,
        installation_title: &str,
        expires: &str,
    ) -> Message {
        let body = format!(
            "Greetings!\n\n\
             A user has been created for you on {installation_title} at {site}\n\n\
             Your username is your email: {email}\n\n\
             Click this link to set a password and begin using Romeka S:\n{link}\n\n\
             Your activation link will expire on {expires}. If you have not completed the user \
             activation process by the time the link expires, you will need to request another \
             activation email from your site administrator.",
            site = self.base.as_str(),
            email = user.email,
            link = self.link(key),
            expires = expires,
        );
        self.message(
            user,
            format!("User activation for {installation_title}"),
            body,
        )
    }

    /// Hand a message to the transport, off the async worker.
    ///
    /// # Errors
    /// A [`MailError`] when the transport refuses the message, and no error when
    /// the blocking task itself could not be spawned: the message is logged as
    /// lost instead, because a caller cannot act on either.
    pub async fn deliver(&self, message: Message) -> Result<Delivery, MailError> {
        let transport = Arc::clone(&self.transport);
        let outcome = tokio::task::spawn_blocking(move || transport.send(&message)).await;
        match outcome {
            Ok(result) => result,
            Err(error) => {
                tracing::error!(target: "romeka::mail", %error, "the mail task did not run");
                Err(MailError::Smtp(error.to_string()))
            }
        }
    }

    /// Build and deliver the reset message.
    ///
    /// # Errors
    /// A [`MailError`] when the transport refuses it.
    pub async fn send_reset_password(
        &self,
        user: &UserRow,
        key: &str,
        installation_title: &str,
        expires: &str,
    ) -> Result<Delivery, MailError> {
        let message = self.reset_message(user, key, installation_title, expires);
        self.deliver(message).await
    }

    /// Build and deliver the activation message.
    ///
    /// # Errors
    /// A [`MailError`] when the transport refuses it.
    pub async fn send_user_activation(
        &self,
        user: &UserRow,
        key: &str,
        installation_title: &str,
        expires: &str,
    ) -> Result<Delivery, MailError> {
        let message = self.activation_message(user, key, installation_title, expires);
        self.deliver(message).await
    }

    fn message(&self, user: &UserRow, subject: String, body: String) -> Message {
        Message {
            from: self.from.clone(),
            to: user.email.clone(),
            name: user.name.clone(),
            subject,
            body,
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use std::sync::Mutex;

    use crate::config::MailTls;
    use omeka_db::repos::UserRepo;
    use omeka_db::rows::NewUser;
    use omeka_db::{migrate, open_memory};

    fn base() -> BaseUrl {
        BaseUrl::parse("https://museum.example").unwrap()
    }

    /// One stored user row, read back through the repository.
    async fn user() -> UserRow {
        let pool = open_memory().await.unwrap();
        migrate(&pool).await.unwrap();
        UserRepo::new(&pool)
            .create(&NewUser {
                email: "person@example.org".to_string(),
                name: "A Person".to_string(),
                password_hash: None,
                role: "editor".to_string(),
                is_active: true,
                created: None,
            })
            .await
            .unwrap()
    }

    /// The display form `IssuedKey::expires_display` writes.
    const EXPIRES: &str = "2026-10-31 12:00 UTC";

    /// A transport that keeps what it was handed.
    #[derive(Default)]
    struct Recording {
        sent: Mutex<Vec<Message>>,
    }

    impl Transport for Recording {
        fn kind(&self) -> TransportKind {
            TransportKind::Smtp
        }

        fn send(&self, message: &Message) -> Result<Delivery, MailError> {
            self.sent.lock().unwrap().push(message.clone());
            Ok(Delivery::Sent)
        }
    }

    fn mailer_with(transport: Arc<dyn Transport>) -> Mailer {
        Mailer::with_transport(transport, "no-reply@romeka.invalid".to_string(), base())
    }

    #[tokio::test]
    async fn no_host_configured_selects_the_log_transport() {
        let mailer = Mailer::from_config(&MailConfig::default(), base());
        assert_eq!(mailer.kind(), TransportKind::Log);
        let message = mailer.reset_message(&user().await, "KEY", "Romeka S", EXPIRES);
        assert_eq!(message.from, "no-reply@romeka.invalid");
    }

    #[test]
    fn a_configured_host_selects_the_smtp_transport() {
        for tls in [MailTls::Starttls, MailTls::Tls, MailTls::None] {
            let config = MailConfig {
                host: Some("smtp.example.org".to_string()),
                tls,
                ..MailConfig::default()
            };
            // Building the transport reads the system certificate store and
            // dials nothing; the send is what would reach the network.
            let mailer = Mailer::from_config(&config, base());
            assert_eq!(
                mailer.kind(),
                TransportKind::Smtp,
                "a host with {tls} selects SMTP"
            );
        }
    }

    #[test]
    fn the_link_points_at_the_set_password_route() {
        let mailer = mailer_with(Arc::new(Log));
        assert_eq!(
            mailer.link("ABC123"),
            "https://museum.example/#/password/ABC123"
        );
    }

    #[tokio::test]
    async fn the_reset_message_carries_the_link_and_the_expiry() {
        let mailer = mailer_with(Arc::new(Log));
        let message = mailer.reset_message(&user().await, "KEY123", "Test Museum", EXPIRES);
        assert_eq!(message.to, "person@example.org");
        assert_eq!(message.subject, "Reset your password for Test Museum");
        assert!(
            message
                .body
                .contains("https://museum.example/#/password/KEY123"),
            "{}",
            message.body
        );
        assert!(
            message.body.contains("2026-10-31 12:00 UTC"),
            "{}",
            message.body
        );
        assert!(message.body.contains("A Person"), "{}", message.body);
    }

    #[tokio::test]
    async fn the_activation_message_carries_the_email_and_the_link() {
        let mailer = mailer_with(Arc::new(Log));
        let message = mailer.activation_message(&user().await, "KEY123", "Test Museum", EXPIRES);
        assert_eq!(message.subject, "User activation for Test Museum");
        assert!(
            message.body.contains("person@example.org"),
            "{}",
            message.body
        );
        assert!(
            message
                .body
                .contains("https://museum.example/#/password/KEY123"),
            "{}",
            message.body
        );
    }

    #[tokio::test]
    async fn delivery_hands_the_built_message_to_the_transport() {
        let recording = Arc::new(Recording::default());
        let mailer = mailer_with(recording.clone());
        let delivery = mailer
            .send_reset_password(&user().await, "KEY123", "Test Museum", EXPIRES)
            .await
            .unwrap();
        assert_eq!(
            delivery,
            Delivery::Sent,
            "the fake transport reports a send"
        );
        let sent = recording.sent.lock().unwrap();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].from, "no-reply@romeka.invalid");
        assert!(sent[0].body.contains("/#/password/KEY123"));
    }

    #[tokio::test]
    async fn the_log_transport_writes_the_link_to_the_log() {
        // A writer the test owns, installed as the thread's subscriber.
        #[derive(Clone, Default)]
        struct Sink(Arc<Mutex<Vec<u8>>>);

        impl std::io::Write for Sink {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(bytes);
                Ok(bytes.len())
            }

            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let written = Arc::new(Mutex::new(Vec::new()));
        let sink = Sink(Arc::clone(&written));
        let subscriber = tracing_subscriber::fmt()
            .with_ansi(false)
            .with_max_level(tracing::Level::TRACE)
            .with_writer(move || sink.clone())
            .finish();
        let mailer = mailer_with(Arc::new(Log));
        let message = mailer.reset_message(&user().await, "KEY123", "Test Museum", EXPIRES);
        let delivery = tracing::subscriber::with_default(subscriber, || Log.send(&message))
            .expect("the log transport accepts everything");

        assert_eq!(delivery, Delivery::Logged);
        let logged = String::from_utf8(written.lock().unwrap().clone()).unwrap();
        assert!(logged.contains("outgoing mail"), "{logged}");
        assert!(logged.contains("person@example.org"), "{logged}");
        assert!(
            logged.contains("https://museum.example/#/password/KEY123"),
            "{logged}"
        );
    }
}
