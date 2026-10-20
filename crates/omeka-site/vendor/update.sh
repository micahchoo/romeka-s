#!/usr/bin/env bash
# Refresh the vendored IIIF viewers and Leaflet from npm at the pinned versions.
# Run from anywhere; it rewrites this directory's mirador/, openseadragon/ and leaflet/.
# A version change also needs the NOTICE entry and the list in
# src/theme/files.rs (the vendor test names any file the list misses).
set -euo pipefail
MIRADOR=4.3.0
OPENSEADRAGON=5.0.1
LEAFLET=1.9.4
here=$(cd "$(dirname "$0")" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
(cd "$work" && npm pack --silent "mirador@$MIRADOR" "openseadragon@$OPENSEADRAGON" "leaflet@$LEAFLET" >/dev/null)
mkdir -p "$work/m" "$work/o" "$work/l"
tar xzf "$work/mirador-$MIRADOR.tgz" -C "$work/m"
tar xzf "$work/openseadragon-$OPENSEADRAGON.tgz" -C "$work/o"
tar xzf "$work/leaflet-$LEAFLET.tgz" -C "$work/l"
rm -rf "$here/mirador" "$here/openseadragon" "$here/leaflet"
mkdir -p "$here/mirador" "$here/openseadragon/images" "$here/leaflet"
cp "$work/m/package/dist/mirador.min.js" "$work/m/package/LICENSE" "$here/mirador/"
cp "$work/o/package/build/openseadragon/openseadragon.min.js" "$here/openseadragon/"
cp "$work/o/package/LICENSE.txt" "$here/openseadragon/LICENSE"
cp "$work/o/package/build/openseadragon/images/"*.png "$here/openseadragon/images/"
cp "$work/l/package/dist/leaflet.js" "$work/l/package/dist/leaflet.css" "$work/l/package/LICENSE" "$here/leaflet/"
# The bundles end with a sourceMappingURL comment; the maps are not vendored.
sed -i '/^\/\/# sourceMappingURL=/d' "$here/mirador/mirador.min.js" "$here/openseadragon/openseadragon.min.js" "$here/leaflet/leaflet.js"
echo "mirador $MIRADOR, openseadragon $OPENSEADRAGON, leaflet $LEAFLET"
