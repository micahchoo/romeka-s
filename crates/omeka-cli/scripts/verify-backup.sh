#!/usr/bin/env bash
# End-to-end check of `omeka backup` and `omeka restore`.
#
#   scripts/verify-backup.sh [--database FILE --store DIR] [--out DIR]
#
# With no arguments the script builds a small database and store, archives them,
# lists the archive with the system `tar`, restores into a fresh directory and
# compares the counts, the bytes and the refusal. With `--database` and `--store`
# it archives that pair instead, which is how the acceptance store is measured.
#
# Exit 0 when every check holds. Nothing is written outside the output directory,
# and a source store is only read.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/mnt/Ghar/2TA/DevStuff/notebook/omeka-research/cargo-s2/backup}"
export CARGO_TARGET_DIR
OMEKA="$CARGO_TARGET_DIR/debug/omeka"

DATABASE=""
STORE=""
OUT=""
while [ $# -gt 0 ]; do
    case "$1" in
        --database) DATABASE="$2"; shift 2 ;;
        --store) STORE="$2"; shift 2 ;;
        --out) OUT="$2"; shift 2 ;;
        -h|--help) sed -n '2,12p' "$0"; exit 0 ;;
        *) echo "unknown argument: $1" >&2; exit 2 ;;
    esac
done
[ -n "$OUT" ] || OUT="$(mktemp -d /tmp/opencode/verify-backup-XXXXXX)"
mkdir -p "$OUT"

check() { # check NAME ACTUAL EXPECTED
    if [ "$2" != "$3" ]; then
        echo "FAIL: $1: got \"$2\", expected \"$3\"" >&2
        exit 1
    fi
    echo "ok: $1"
}

echo "verify-backup"
echo "  root  $ROOT"
echo "  out   $OUT"

cargo build --manifest-path "$ROOT/Cargo.toml" -p omeka-cli >/dev/null

if [ -z "$DATABASE" ]; then
    echo "[1/6] build a database and a store"
    DATABASE="$OUT/omeka.db"
    STORE="$OUT/files"
    "$OMEKA" install -d "$DATABASE" --email verify@example.org --name Verify --password verify-password-1 >/dev/null
    STORAGE_ID="$(printf 'a%.0s' {1..40})"
    printf 'original bytes\n' > "$OUT/original.bin"
    SHA="$(sha256sum "$OUT/original.bin" | cut -d' ' -f1)"
    SIZE="$(stat -c %s "$OUT/original.bin")"
    mkdir -p "$STORE/original"
    cp "$OUT/original.bin" "$STORE/original/$STORAGE_ID.jpg"
    sqlite3 "$DATABASE" <<SQL
INSERT INTO resource (title, is_public, created, modified, resource_type)
    VALUES ('Verify set', 1, datetime('now'), datetime('now'), 'Omeka\\Entity\\ItemSet');
INSERT INTO item_set (id, is_open) VALUES (last_insert_rowid(), 0);
INSERT INTO resource (title, is_public, created, modified, resource_type)
    VALUES ('Round Trip', 1, datetime('now'), datetime('now'), 'Omeka\\Entity\\Item');
INSERT INTO item (id) VALUES (last_insert_rowid());
INSERT INTO resource (title, is_public, created, modified, resource_type)
    VALUES ('original.bin', 1, datetime('now'), datetime('now'), 'Omeka\\Entity\\Media');
INSERT INTO media (id, item_id, ingester, renderer, source, media_type, storage_id, extension, sha256, size, has_original, has_thumbnails)
    VALUES (last_insert_rowid(), (SELECT id FROM item), 'upload', 'file', 'original.bin', 'image/jpeg', '$STORAGE_ID', 'jpg', '$SHA', $SIZE, 1, 0);
SQL
else
    echo "[1/6] use the database and store the caller named"
fi

echo "[2/6] back up"
ARCHIVE="$OUT/backup.tar"
"$OMEKA" backup -d "$DATABASE" --store "$STORE" --out "$ARCHIVE"

echo "[3/6] read the archive with the system tar"
LISTING="$(tar tf "$ARCHIVE")"
grep -qx omeka.db <<<"$LISTING"
grep -qx manifest.json <<<"$LISTING"
grep -q '^store/' <<<"$LISTING"
mkdir -p "$OUT/manifest"
tar xf "$ARCHIVE" -C "$OUT/manifest" manifest.json
check "the manifest names the format" "$(jq -r .format "$OUT/manifest/manifest.json")" "omeka-backup"
check "the manifest names the commit" "$(jq -r '.commit | length > 0' "$OUT/manifest/manifest.json")" "true"
check "the manifest carries the store checksum" "$(jq -r '.store.checksum | length' "$OUT/manifest/manifest.json")" "64"
COUNTED="$(jq -r '.counts.item' "$OUT/manifest/manifest.json")"
check "the counts come from the archived copy" "$COUNTED" "$(sqlite3 "$DATABASE" 'SELECT COUNT(*) FROM item;')"

echo "[4/6] extract the database copy and compare it"
mkdir -p "$OUT/extracted"
tar xf "$ARCHIVE" -C "$OUT/extracted" omeka.db
check "the database copy matches its manifest" \
    "$(sha256sum "$OUT/extracted/omeka.db" | cut -d' ' -f1)" \
    "$(jq -r .database.sha256 "$OUT/manifest/manifest.json")"

echo "[5/6] restore into a fresh directory"
RESTORED="$OUT/restored"
"$OMEKA" restore -d "$RESTORED/omeka.db" --store "$RESTORED/files" --in "$ARCHIVE"
check "the restored item count" "$(sqlite3 "$RESTORED/omeka.db" 'SELECT COUNT(*) FROM item;')" "$COUNTED"
SOURCE_FILES="$(find "$STORE" -type f | wc -l)"
check "the restored store file count" "$(find "$RESTORED/files" -type f | wc -l)" "$SOURCE_FILES"
while IFS= read -r file; do
    relative="${file#"$STORE"/}"
    check "store/$relative" \
        "$(sha256sum "$RESTORED/files/$relative" | cut -d' ' -f1)" \
        "$(sha256sum "$file" | cut -d' ' -f1)"
done < <(find "$STORE" -type f)

echo "[6/6] a second restore without --force is refused, with --force it lands"
if "$OMEKA" restore -d "$RESTORED/omeka.db" --store "$RESTORED/files" --in "$ARCHIVE" 2>"$OUT/refused.log"; then
    echo "FAIL: the second restore was not refused" >&2
    exit 1
fi
grep -q -- "--force" "$OUT/refused.log"
echo "ok: the refusal names --force"
"$OMEKA" restore -d "$RESTORED/omeka.db" --store "$RESTORED/files" --in "$ARCHIVE" --force >/dev/null
check "the forced restore keeps the item count" "$(sqlite3 "$RESTORED/omeka.db" 'SELECT COUNT(*) FROM item;')" "$COUNTED"

echo "archive $(stat -c %s "$ARCHIVE") bytes at $ARCHIVE"
echo "VERIFIED"
