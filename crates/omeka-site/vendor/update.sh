#!/usr/bin/env bash
# Refresh the vendored IIIF viewers, the 3D viewer and Leaflet from npm at the
# pinned versions. Run from anywhere; it rewrites this directory's mirador/,
# openseadragon/, model-viewer/ and leaflet/.
# A version change also needs the NOTICE entry and the list in
# src/theme/files.rs (the vendor test names any file the list misses).
set -euo pipefail
MIRADOR=4.3.0
OPENSEADRAGON=5.0.1
MODEL_VIEWER=4.4.0
LEAFLET=1.9.4
here=$(cd "$(dirname "$0")" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
(cd "$work" && npm pack --silent "mirador@$MIRADOR" "openseadragon@$OPENSEADRAGON" "model-viewer@$MODEL_VIEWER" "leaflet@$LEAFLET" >/dev/null)
mkdir -p "$work/m" "$work/o" "$work/v" "$work/l"
tar xzf "$work/mirador-$MIRADOR.tgz" -C "$work/m"
tar xzf "$work/openseadragon-$OPENSEADRAGON.tgz" -C "$work/o"
tar xzf "$work/model-viewer-$MODEL_VIEWER.tgz" -C "$work/v"
tar xzf "$work/leaflet-$LEAFLET.tgz" -C "$work/l"
rm -rf "$here/mirador" "$here/openseadragon" "$here/model-viewer" "$here/leaflet"
mkdir -p "$here/mirador" "$here/openseadragon/images" "$here/model-viewer" "$here/leaflet"
cp "$work/m/package/dist/mirador.min.js" "$work/m/package/LICENSE" "$here/mirador/"
cp "$work/o/package/build/openseadragon/openseadragon.min.js" "$here/openseadragon/"
cp "$work/o/package/LICENSE.txt" "$here/openseadragon/LICENSE"
cp "$work/o/package/build/openseadragon/images/"*.png "$here/openseadragon/images/"
# The UMD build registers the custom element and carries three, so the page
# loads it as one classic script that runs under `script-src 'self'`. The two
# module builds need `three` beside them or an import map.
cp "$work/v/package/dist/model-viewer-umd.min.js" "$work/v/package/LICENSE" "$here/model-viewer/"
cp "$work/l/package/dist/leaflet.js" "$work/l/package/dist/leaflet.css" "$work/l/package/LICENSE" "$here/leaflet/"
# The bundles end with a sourceMappingURL comment; the maps are not vendored.
sed -i '/^\/\/# sourceMappingURL=/d' "$here/mirador/mirador.min.js" "$here/openseadragon/openseadragon.min.js" "$here/model-viewer/model-viewer-umd.min.js" "$here/leaflet/leaflet.js"
echo "mirador $MIRADOR, openseadragon $OPENSEADRAGON, model-viewer $MODEL_VIEWER, leaflet $LEAFLET"
