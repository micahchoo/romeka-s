#!/usr/bin/env python3
"""Cross-check the generated wire member lists against the captured fixtures.

`docs/api.md` names each document's members by parsing `crates/omeka-api/src/wire/`.
The golden fixtures under `fixtures/api/` are documents derived from upstream, so
they are independent evidence for the same lists. This script asserts every
top-level key a fixture carries is one the parsed document declares (a flattened
value map may carry any property term).

  ./gen/check_wire.py [DIR]          check DIR (default fixtures/api), one line per fixture
  ./gen/check_wire.py --quiet [DIR]  print only failures

An empty fixture set is a failure: the cross-check proves nothing when it read no
fixture, so it exits non-zero rather than reporting a vacuous pass.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))

from paths import REPO  # noqa: E402
from wire import wire_documents  # noqa: E402


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "directory",
        nargs="?",
        default=None,
        help="fixture directory (default: fixtures/api)",
    )
    parser.add_argument("--quiet", action="store_true")
    args = parser.parse_args()

    documents, _ = wire_documents()
    document_by_stem = {}
    for doc in documents:
        if not doc["resource"]:
            continue
        members = [member.key for item in doc["items"] for member in item.members]
        document_by_stem[Path(doc["file"]).stem] = (
            set(members),
            "(flattened)" in members,
            doc["file"],
        )

    fixtures = Path(args.directory) if args.directory is not None else REPO / "fixtures" / "api"
    paths = sorted(fixtures.glob("*.json"))
    if not paths:
        print(f"no fixtures found under {fixtures}")
        return 1

    failures = 0
    checked = 0
    for path in paths:
        entry = document_by_stem.get(path.stem)
        if entry is None:
            if not args.quiet:
                print(f"skip  {path.name}: no wire module for it")
            continue
        keys, flattened, wire_file = entry
        body = json.loads(path.read_text(encoding="utf-8"))
        if not isinstance(body, dict):
            print(f"fail  {path.name}: fixture is not an object")
            failures += 1
            continue
        unknown = sorted(
            key
            for key in body
            if key not in keys and not (flattened and ":" in key)
        )
        checked += 1
        if unknown:
            failures += 1
            print(f"fail  {path.name}: keys not in {wire_file}: {', '.join(unknown)}")
        elif not args.quiet:
            print(f"ok    {path.name}: {len(body)} keys, all declared by {wire_file}")

    if checked == 0:
        print(f"no fixture matched a wire module under {fixtures}")
        return 1

    print(f"\n{checked} fixtures checked against the parsed wire modules, {failures} with unknown keys")
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
