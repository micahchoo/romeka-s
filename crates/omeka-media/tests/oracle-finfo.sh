#!/usr/bin/env bash
# Print upstream's media type for every file in a folder, as Rust tuples for
# `tests/media_type.rs`. Upstream is `TempFile::getMediaType`: PHP `finfo` on
# the file in the oracle container, then `MEDIA_TYPE_ALIASES` from the
# oracle's source. It reads a path, not a buffer, because libmagic answers a
# buffer differently: a zip read as a buffer is `application/octet-stream`.
set -euo pipefail
dir=${1:?usage: oracle-finfo.sh <fixture dir> [container]}
container=${2:-omeka-oracle-app-1}
for file in "$dir"/*; do
  name=$(basename "$file")
  type=$(docker exec -i "$container" sh -c '
    probe=$(mktemp)
    cat >"$probe"
    php -r "
      require \"/var/www/html/vendor/autoload.php\";
      \$type = (new finfo(FILEINFO_MIME_TYPE))->file(\$argv[1]);
      echo Omeka\\File\\TempFile::MEDIA_TYPE_ALIASES[\$type] ?? \$type;
    " "$probe"
    rm -f "$probe"
  ' <"$file")
  printf '    ("%s", "%s"),\n' "$name" "$type"
done
