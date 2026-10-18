#!/usr/bin/env bash
# Start the port and its static origin as one service.
#
# The same script runs as the container entrypoint and as the systemd unit's
# ExecStart, so the container and a bare-metal install resolve the same paths
# and render the same origin config.
#
# It converges on a running service from any prior state: `omeka install`
# creates an absent database and seeds it, and is a no-op on one already seeded.
# The database and the store live on paths the caller mounts, never inside the
# installed tree.
set -euo pipefail

DB="${OMEKA_DB:-/data/omeka.db}"
STORE="${OMEKA_STORE:-/data/files}"
DIST="${OMEKA_DIST:-/app/dist}"
API_HOST="${OMEKA_API_HOST:-127.0.0.1}"
API_PORT="${OMEKA_API_PORT:-8123}"
PUBLIC_PORT="${OMEKA_PUBLIC_PORT:-8080}"
BASE_URL="${OMEKA_BASE_URL:-http://localhost:${PUBLIC_PORT}}"
TEMPLATE="${OMEKA_NGINX_TEMPLATE:-/etc/omeka/nginx.conf.template}"
CONF="${OMEKA_NGINX_CONF:-/tmp/nginx.conf}"

if [ ! -f "$TEMPLATE" ]; then
    echo "omeka: missing nginx template $TEMPLATE" >&2
    exit 1
fi
if [ ! -d "$DIST" ]; then
    echo "omeka: missing admin bundle $DIST" >&2
    exit 1
fi

mkdir -p "$(dirname "$DB")" "$STORE"

# Idempotent: a second run against a seeded database inserts nothing.
if [ "${OMEKA_SKIP_INSTALL:-0}" != "1" ]; then
    omeka install -d "$DB" \
        --email "${OMEKA_ADMIN_EMAIL:-admin@example.com}" \
        --name "${OMEKA_ADMIN_NAME:-Admin}" \
        --password "${OMEKA_ADMIN_PASSWORD:-changeme}" \
        --title "${OMEKA_TITLE:-Omeka S}"
fi

# A path holding sed's metacharacters stays literal in the render.
escape() {
    printf '%s' "$1" | sed -e 's/[&\\|]/\\&/g'
}

# Render before anything starts, so a bad path fails the start on its own.
sed \
    -e "s|@PUBLIC_PORT@|$(escape "$PUBLIC_PORT")|g" \
    -e "s|@DIST@|$(escape "$DIST")|g" \
    -e "s|@STORE@|$(escape "$STORE")|g" \
    -e "s|@BACKEND@|$(escape "${API_HOST}:${API_PORT}")|g" \
    "$TEMPLATE" > "$CONF"

echo "omeka: serving the port on ${API_HOST}:${API_PORT}, the origin on :${PUBLIC_PORT}" >&2
omeka serve -d "$DB" --host "$API_HOST" --port "$API_PORT" --base-url "$BASE_URL" &
serve_pid=$!

nginx -c "$CONF" -g 'daemon off;' &
nginx_pid=$!

shutdown() {
    kill "$serve_pid" "$nginx_pid" 2>/dev/null || true
    wait "$serve_pid" "$nginx_pid" 2>/dev/null || true
}
trap shutdown EXIT TERM INT

# Exit when either process dies so the restart policy sees a stop, not a
# half-dead service answering with 502.
wait -n "$serve_pid" "$nginx_pid"
