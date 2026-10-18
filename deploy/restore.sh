#!/usr/bin/env bash
# Replace the database and the media store from an archive backup.sh wrote.
#
#     cd deploy && ./restore.sh backup-20261017T120000Z.tar
#
# The service is stopped for the restore: it replaces the database file and
# mirrors the store, and a running server holds the database open. Everything
# the archive holds is checked against its manifest before anything is written,
# and the store is mirrored, so a file the archive does not hold is removed.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"

archive="${1:-}"
if [ -z "$archive" ] || [ ! -f "$archive" ]; then
    echo "restore: usage: ./restore.sh ARCHIVE" >&2
    exit 1
fi

# The archive is read into the database volume over stdin. A bind mount of the
# host directory would need that directory and the file in it to be readable by
# the container's user, which an operator's own backup directory often is not.
# The name is hidden and removed again below, and a rerun overwrites it.
staged='$(dirname "$ROMEEKA_DB")/.restore-archive.tar'
echo "restore: reading $archive into the container"
docker compose run --rm --no-deps --entrypoint /bin/sh -T romeka \
    -c "cat > $staged" < "$archive"

echo "restore: stopping the service"
docker compose stop romeka

# The database and store paths come from the container's own environment, which
# is what .env sets, so this script and the service cannot disagree about them.
docker compose run --rm --no-deps --entrypoint /bin/sh -T romeka \
    -c "exec romeka restore --database \"\$ROMEEKA_DB\" --store \"\$ROMEEKA_STORE\" --in $staged --force"

docker compose run --rm --no-deps --entrypoint /bin/sh -T romeka \
    -c "rm -f $staged"

echo "restore: starting the service"
docker compose up --detach

echo "restore: restored $archive; watch the log with docker compose logs --follow romeka"
