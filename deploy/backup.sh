#!/usr/bin/env bash
# Archive the database and the media store into one file on the host.
#
#     cd deploy && ./backup.sh [archive.tar]
#
# `romeka backup` copies the database through one SQLite read transaction
# (VACUUM INTO), so a running server keeps serving and the archive is a single
# point in time. It runs in a one-off container that mounts the service's
# volumes, so the service does not have to be running.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"

archive="${1:-backup-$(date -u +%Y%m%dT%H%M%SZ).tar}"
if [ -e "$archive" ]; then
    echo "backup: $archive exists, refusing to overwrite it" >&2
    exit 1
fi

# Stage beside the archive, not in /tmp: /tmp can be a RAM disk smaller than the media store.
staging="$(mktemp -d "$(dirname "$archive")/.backup-XXXXXX")"
trap 'rm -rf "$staging"' EXIT
# The container runs as its own system user, which is not the user running this
# script, so the directory the archive is written into has to be open to it. The
# archive is moved out of here immediately, and the name is a fresh random one.
chmod 0777 "$staging"

# The database and store paths come from the container's own environment, which
# is what .env sets, so this script and the service cannot disagree about them.
docker compose run --rm --no-deps --entrypoint /bin/sh \
    --volume "$staging":/out \
    romeka -c 'exec romeka backup --database "$ROMEKA_DB" --store "$ROMEKA_STORE" --out /out/archive.tar'

mv "$staging/archive.tar" "$archive"
echo "backup: wrote $archive ($(du -h "$archive" | cut -f1))"
