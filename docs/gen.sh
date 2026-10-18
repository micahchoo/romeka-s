#!/usr/bin/env bash
# Regenerate docs/api.md from the omeka-api source, or check it for staleness.
#
#   ./gen.sh          write api.md
#   ./gen.sh --check  fail when api.md no longer matches the source
set -euo pipefail
cd "$(dirname "$0")"
exec python3 gen/api_doc.py --out api.md "$@"
