#!/usr/bin/env bash
# Start Romeka S: install the database when it is absent, serve it on loopback,
# and run Caddy in front for TLS, the admin SPA and the media bytes.
#
# The same script is the container entrypoint, so every setting it reads is an
# environment variable the deployment sets (see deploy/.env.example).
set -euo pipefail

DB="${ROMEEKA_DB:-/db/romeka.db}"
STORE="${ROMEEKA_STORE:-/data/files}"
DIST="${ROMEEKA_DIST:-/srv/dist}"
CADDYFILE="${ROMEEKA_CADDYFILE:-/etc/caddy/Caddyfile}"
HOST="${ROMEEKA_API_HOST:-127.0.0.1}"
PORT="${ROMEEKA_API_PORT:-8123}"
BASE_URL="${ROMEEKA_BASE_URL:-}"
DOMAIN="${ROMEEKA_DOMAIN:-}"

# A container that cannot say where it is cannot serve cookies or JSON-LD links.
if [ -z "$BASE_URL" ]; then
    echo "romeka: ROMEEKA_BASE_URL is not set" >&2
    exit 1
fi
if [ -z "$DOMAIN" ]; then
    echo "romeka: ROMEEKA_DOMAIN is not set" >&2
    exit 1
fi
# Caddy's `email` option needs a value to parse, so an empty contact is a
# configuration error rather than a silent default addressed to a stranger.
if [ -z "${ROMEEKA_ACME_EMAIL:-}" ]; then
    echo "romeka: ROMEEKA_ACME_EMAIL is not set" >&2
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
for name in ROMEEKA_LOG ROMEEKA_MAX_BODY_BYTES ROMEEKA_REQUEST_TIMEOUT_SECS \
    ROMEEKA_SMTP_HOST ROMEEKA_SMTP_PORT ROMEEKA_SMTP_USER ROMEEKA_SMTP_PASSWORD \
    ROMEEKA_SMTP_TLS ROMEEKA_MAIL_FROM; do
    if [ -z "${!name:-}" ]; then
        unset "$name"
    fi
done

mkdir -p "$(dirname "$DB")" "$STORE"
# The server reads and writes the store as `files` relative to its working
# directory, so it starts in the store's parent and that is the path it serves.
cd "$(dirname "$STORE")"

# `romeka install` creates the schema, seeds the default vocabularies and the
# Base Resource template, and writes the first global_admin. It also rewrites the
# settings rows it owns, installation_title among them, so it must NOT run on
# every start: a container restart would undo every settings change an
# administrator made. An absent database is the signal; DEPLOY.md's upgrade
# procedure runs install explicitly.
if [ ! -s "$DB" ]; then
    for name in ROMEEKA_ADMIN_EMAIL ROMEEKA_ADMIN_NAME ROMEEKA_ADMIN_PASSWORD; do
        if [ -z "${!name:-}" ]; then
            echo "romeka: $name is not set and $DB is empty, so there is no administrator to create" >&2
            exit 1
        fi
    done
    echo "romeka: $DB is empty, installing" >&2
    romeka install -d "$DB" \
        --email "$ROMEEKA_ADMIN_EMAIL" \
        --name "$ROMEEKA_ADMIN_NAME" \
        --password "$ROMEEKA_ADMIN_PASSWORD" \
        --title "${ROMEEKA_TITLE:-Romeka S}" \
        --time-zone "${ROMEEKA_TIME_ZONE:-UTC}" \
        --locale "${ROMEEKA_LOCALE:-en}"
fi

echo "romeka: serving $HOST:$PORT with base URL $BASE_URL, the origin on :80 and :443" >&2
romeka serve -d "$DB" --host "$HOST" --port "$PORT" --base-url "$BASE_URL" &
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
