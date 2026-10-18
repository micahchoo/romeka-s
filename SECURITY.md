# Security

Audience: anyone who has found a security problem in Romeka S, and anyone who needs to know what the
session and the API key protect.

## Reporting a vulnerability

Report privately through the repository's **Security** tab, using GitHub's private vulnerability
reporting form. Do not open a public issue for a security problem.

A report is useful with the version (`romeka --version`), the exact request or command, what you
expected, what happened, and anything you have that shows the impact. Do not include a live
credential or a person's data; a key you minted for the test is enough, and it can be revoked with
`romeka key revoke`.

If the problem is in upstream Omeka S rather than in this fork, report it upstream. If it is in a
third-party service or in your own proxy and TLS configuration, it is yours to fix.

## What the credentials protect

**The API key.** A key is a pair: `key_identity` and `key_credential`, both 32 alphanumeric
characters. Callers send them as query parameters, which is what upstream expects, so a key minted
here works against an Omeka S server and the reverse. The credential is stored as a bcrypt hash
(`crates/omeka-cli/src/commands/key.rs`), is printed once when minted, and is in no later output.
`romeka key list` prints identities and never credentials. Possession of the pair is the whole
authority: treat it as a password, and revoke and re-mint it if it leaks.

**The session.** Signing in with a password or a one-time link creates a server-side session row
behind an `HttpOnly` cookie (`romeka_session`). The cookie is `SameSite=Lax` with `Path=/`, and
`Secure` is added when the configured base URL is https, so a deployment behind plain HTTP does not
mark it `Secure` (`crates/omeka-api/src/session.rs`). `SameSite` is not the CSRF defence on its own:
a state-changing request that carries the cookie must come from the configured base URL's origin,
checked on `Origin` and, when there is none, on `Referer`, and anything else is a 403
(`crates/omeka-api/src/origin.rs`). This is why `--base-url` has to name the origin a browser
actually uses: a mismatched base URL refuses the write rather than accepting it from anywhere.

**One-time links.** An invite or a password-reset link carries a key in its URL and expires two weeks
after it is minted. Only the last link minted for an account works. A link that reaches a log or a
chat window is a credential; with no SMTP relay configured the link is written to the server log by
design, so the log is as sensitive as the mail would have been.

**Rate limits.** Login attempts are counted by email and by client address, and the forgot-password
route is limited too (`crates/omeka-server/src/accounts.rs`). Behind a proxy, the client address
comes from `X-Real-IP` and `X-Forwarded-For`, so a proxy that does not set them gives every caller
one counter.

## What the process holds

The database is one file, and the media store is a directory tree beside it. Both are the data: file
permissions, volume permissions and backups are the operator's to protect, and
[`deploy/DEPLOY.md`](deploy/DEPLOY.md) covers backup and restore. There is no at-rest encryption, and
none is planned. Nothing is sent anywhere: with no SMTP relay configured, mail is written to the log
and never leaves the process.

## What is not in scope

The reimplementation is in scope: the crates under `crates/`, the admin under `web/admin`, the image
under `packaging/` and the deployment under `deploy/`. Upstream's PHP application is not, and neither
is the PHP module and theme ecosystem it carries. The surface this fork has not built answers 501 and
is listed in [`docs/api.md`](docs/api.md); a 501 is not a vulnerability. A deployment's own TLS
termination, reverse proxy and firewall are the operator's.
