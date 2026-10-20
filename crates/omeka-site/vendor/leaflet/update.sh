#!/usr/bin/env bash
# Refresh the vendored Leaflet from npm at the pinned version.
# Run from anywhere; it rewrites this directory's leaflet.js, leaflet.css and LICENSE.
# A version change also needs the NOTICE entry.
set -euo pipefail
LEAFLET=1.9.4
here=$(cd "$(dirname "$0")" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
(cd "$work" && npm pack --silent "leaflet@$LEAFLET" >/dev/null)
tar xzf "$work/leaflet-$LEAFLET.tgz" -C "$work"
cp "$work/package/dist/leaflet.js" "$work/package/dist/leaflet.css" "$work/package/LICENSE" "$here/"
# The bundle ends with a sourceMappingURL comment; the map is not vendored.
sed -i '/^\/\/# sourceMappingURL=/d' "$here/leaflet.js"
echo "leaflet $LEAFLET"
