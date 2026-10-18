#!/usr/bin/env bash
# Build the image, run it as the service on a private port with fresh volumes,
# prove the origin answers, prove the volumes carry the state across a restart,
# then tear everything down.
#
#     cd packaging && ./smoke.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
IMAGE="${OMEKA_SMOKE_IMAGE:-omeka-s-packaging:smoke}"
BASE_HOST="127.0.0.1"
CONTAINER="omeka-smoke-$$"
DATA_VOLUME="omeka-smoke-data-$$"
STORE_VOLUME="omeka-smoke-store-$$"

cleanup() {
    docker rm --force "$CONTAINER" >/dev/null 2>&1 || true
    docker volume rm "$DATA_VOLUME" "$STORE_VOLUME" >/dev/null 2>&1 || true
}
trap cleanup EXIT

# A random host port is reassigned when the container restarts, so resolve it
# again after every start.
resolve_base() {
    printf 'http://%s:%s' "$BASE_HOST" \
        "$(docker port "$CONTAINER" 8080/tcp | head -n 1 | sed 's/.*://')"
}

wait_healthy() {
    local base="$1"
    for _ in $(seq 1 60); do
        if curl -fsS --max-time 2 "$base/health" >/dev/null 2>&1; then
            return 0
        fi
        sleep 1
    done
    return 1
}

fail() {
    echo "$1" >&2
    docker logs "$CONTAINER" >&2 || true
    exit 1
}

# The run must not touch the repository; compare its state at both ends.
before="$(git -C "$ROOT" status --porcelain)"

echo "== building $IMAGE"
docker build --file "$ROOT/packaging/Dockerfile" --tag "$IMAGE" "$ROOT"

echo "== starting $CONTAINER on fresh volumes"
docker volume create "$DATA_VOLUME" >/dev/null
docker volume create "$STORE_VOLUME" >/dev/null
docker run --detach --name "$CONTAINER" \
    --publish "${BASE_HOST}::8080" \
    --volume "$DATA_VOLUME":/data \
    --volume "$STORE_VOLUME":/data/files \
    --env OMEKA_ADMIN_EMAIL=admin@example.com \
    --env OMEKA_ADMIN_NAME=Admin \
    --env OMEKA_ADMIN_PASSWORD=smoke-password \
    --env OMEKA_TITLE="Omeka S" \
    --env OMEKA_BASE_URL="http://${BASE_HOST}:8124" \
    "$IMAGE" >/dev/null

mounts="$(docker inspect --format '{{range .Mounts}}{{.Type}}:{{.Destination}} {{end}}' "$CONTAINER")"
case "$mounts" in
    *"volume:/data "*) ;;
    *) fail "the data path is not a volume: $mounts" ;;
esac
case "$mounts" in
    *"volume:/data/files"*) ;;
    *) fail "the store path is not a volume: $mounts" ;;
esac
echo "volumes: ${mounts% }"

base="$(resolve_base)"
echo "== waiting for ${base}/health"
wait_healthy "$base" || fail "the container never became healthy"

echo "health:  $(curl -fsS "$base/health")"
echo "page:    $(curl -s -o /dev/null -w '%{http_code}' "$base/")"
echo "api:     $(curl -fsS "$base/api" | cut -c1-120)"

# A row written before the restart is the evidence that the database survived.
identity="$(docker exec "$CONTAINER" omeka key add -d /data/omeka.db \
    --owner admin@example.com | sed -n 's/^  key_identity=//p')"
echo "== minted key ${identity}"

echo "== restarting, to show the volumes keep the state"
docker restart "$CONTAINER" >/dev/null
base="$(resolve_base)"
wait_healthy "$base" || fail "the container never became healthy after a restart"

if ! docker exec "$CONTAINER" omeka key list -d /data/omeka.db | cut -f1 | grep -qx "$identity"; then
    fail "the key minted before the restart is gone"
fi
echo "health after restart: $(curl -fsS "$base/health")"
echo "key survives restart: ${identity}"

after="$(git -C "$ROOT" status --porcelain)"
if [ "$before" != "$after" ]; then
    echo "the repository changed during the smoke run" >&2
    printf '%s\n' "$after" >&2
    exit 1
fi
echo "== ok: the origin answers, the volumes hold the state, the repository is untouched"
