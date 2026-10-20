#!/usr/bin/env bash
# Start Romeka S: install the database when it is absent, serve it on loopback,
# and run Caddy in front for TLS, the admin SPA and the media bytes.
#
# ROMEKA_MODE=install runs the install and the schema migrations against an
# existing database and stops, which is what DEPLOY.md's upgrade procedure uses.
#
# The same script is the container entrypoint, so every setting it reads is an
# environment variable the deployment sets (see deploy/.env.example).
set -euo pipefail

DB="${ROMEKA_DB:-/db/romeka.db}"
STORE="${ROMEKA_STORE:-/data/files}"
DIST="${ROMEKA_DIST:-/srv/dist}"
CADDYFILE="${ROMEKA_CADDYFILE:-/etc/caddy/Caddyfile}"
HOST="${ROMEKA_API_HOST:-127.0.0.1}"
PORT="${ROMEKA_API_PORT:-8123}"
BASE_URL="${ROMEKA_BASE_URL:-}"
DOMAIN="${ROMEKA_DOMAIN:-}"

# A container that cannot say where it is cannot serve cookies or JSON-LD links.
if [ -z "$BASE_URL" ]; then
    echo "romeka: ROMEKA_BASE_URL is not set" >&2
    exit 1
fi
if [ -z "$DOMAIN" ]; then
    echo "romeka: ROMEKA_DOMAIN is not set" >&2
    exit 1
fi
# Caddy's `email` option needs a value to parse, so an empty contact is a
# configuration error rather than a silent default addressed to a stranger.
if [ -z "${ROMEKA_ACME_EMAIL:-}" ]; then
    echo "romeka: ROMEKA_ACME_EMAIL is not set" >&2
    exit 1
fi
if [ ! -d "$DIST" ]; then
    echo "romeka: missing the admin bundle at $DIST" >&2
    exit 1
fi
if [ ! -f "$CADDYFILE" ]; then
    echo "romeka: missing the origin config at $CADDYFILE" >&2
    exit 1
fi

# An empty value in the environment means "not configured", but the server reads
# any value present, empty or not: an empty ROMEKA_SMTP_HOST would build a relay
# at "" and log an error on every start. Drop the empty ones so absence reaches
# the server as absence.
for name in ROMEKA_LOG ROMEKA_MAX_BODY_BYTES ROMEKA_REQUEST_TIMEOUT_SECS \
    ROMEKA_SMTP_HOST ROMEKA_SMTP_PORT ROMEKA_SMTP_USER ROMEKA_SMTP_PASSWORD \
    ROMEKA_SMTP_TLS ROMEKA_MAIL_FROM ROMEKA_ARCHIVE_ROOT; do
    if [ -z "${!name:-}" ]; then
        unset "$name"
    fi
done

mkdir -p "$(dirname "$DB")" "$STORE"
# The server reads and writes the store as `files` relative to its working
# directory, so it starts in the store's parent and that is the path it serves.
cd "$(dirname "$STORE")"

# `romeka install` creates the schema, seeds the default vocabularies and the
# Base Resource template, writes the first global_admin, and runs the schema
# migrations, which is what an upgrade is. It also rewrites the settings rows it
# owns, installation_title among them, so it is not run on every start: a restart
# would undo every settings change an administrator made.
install() {
    for name in ROMEKA_ADMIN_EMAIL ROMEKA_ADMIN_NAME ROMEKA_ADMIN_PASSWORD; do
        if [ -z "${!name:-}" ]; then
            echo "romeka: $name is not set, so there is no administrator to create" >&2
            exit 1
        fi
    done
    romeka install -d "$DB" \
        --email "$ROMEKA_ADMIN_EMAIL" \
        --name "$ROMEKA_ADMIN_NAME" \
        --password "$ROMEKA_ADMIN_PASSWORD" \
        --title "${ROMEKA_TITLE:-Romeka S}" \
        --time-zone "${ROMEKA_TIME_ZONE:-UTC}" \
        --locale "${ROMEKA_LOCALE:-en}"
}

case "${ROMEKA_MODE:-serve}" in
    serve)
        # An absent or empty database is the first start.
        if [ ! -s "$DB" ]; then
            echo "romeka: $DB is empty, installing" >&2
            install
        fi
        ;;
    install)
        # DEPLOY.md's upgrade runs this against an existing database, in a
        # one-off container, with the service stopped.
        echo "romeka: installing into $DB" >&2
        install
        exit 0
        ;;
    *)
        echo "romeka: ROMEKA_MODE must be serve or install, not ${ROMEKA_MODE:-}" >&2
        exit 1
        ;;
esac

echo "romeka: serving $HOST:$PORT with base URL $BASE_URL, the origin on :80 and :443" >&2
# `--store` names the store the import and undo jobs write too: the server
# starts each job as `romeka job run` with this store, the database and
# ROMEKA_ARCHIVE_ROOT, so the job and the server read the same files.
romeka serve -d "$DB" --host "$HOST" --port "$PORT" --base-url "$BASE_URL" --store "$STORE" &
serve_pid=$!

caddy run --config "$CADDYFILE" --adapter caddyfile &
caddy_pid=$!

shutdown() {
    kill "$serve_pid" "$caddy_pid" 2>/dev/null || true
    wait "$serve_pid" "$caddy_pid" 2>/dev/null || true
}
trap shutdown EXIT TERM INT

# Exit when either process dies, so the restart policy sees a stop rather than a
# half-dead service answering 502.
wait -n "$serve_pid" "$caddy_pid"
