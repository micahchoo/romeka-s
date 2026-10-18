#!/usr/bin/env python3
"""Generate docs/api.md from the omeka-api crate source.

The document is a pure function of these files:

  crates/omeka-api/src/routes.rs       path templates, allowed methods, method -> operation
  crates/omeka-api/src/adapters/*.rs   one AdapterSpec per registered adapter
  crates/omeka-api/src/wire/*.rs       response document structs and enums
  crates/omeka-api/src/read/*.rs       dispatch tables and the query parameters they read
  crates/omeka-api/src/write/*.rs      dispatch tables and the request body members they read

Nothing here is hand-written: every table is parsed from the source, so a member
the source drops disappears from the document on the next run.

Usage:
  ./gen/api_doc.py --out api.md    write the document
  ./gen/api_doc.py --check         fail (exit 1) when docs/api.md is stale
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from model import (
    Doc,
    collect_read_source,
    collect_route_operations,
    parse_adapters,
    parse_const_strs,
    parse_dispatch,
    parse_method_lists,
    route_rows,
)
from paths import DOCS, SRC, read, strip_tests
from render import render_document
from russparse import Function, parse_functions
from wire import wire_documents


def build() -> str:
    routes_text = strip_tests(read(SRC / "routes.rs"))
    route_consts = parse_const_strs(routes_text)
    allow = parse_method_lists(routes_text)
    adapters = parse_adapters(SRC)

    search_dispatch, member_dispatch = collect_read_source()
    served_read = sorted(set(search_dispatch) | set(member_dispatch))
    search_fn = {resource: handler for resource, (_, handler) in search_dispatch.items()}
    if sorted(search_fn) != sorted(served_read):
        missing = sorted(set(served_read) - set(search_fn))
        raise SystemExit(f"read dispatch tables disagree for: {missing}")

    read_functions: list[Function] = []
    for path in sorted((SRC / "read").glob("*.rs")):
        read_functions.extend(parse_functions(read(path), f"crates/omeka-api/src/read/{path.name}"))
    index: dict[str, list[Function]] = {}
    for function in read_functions:
        index.setdefault(function.name, []).append(function)

    write_mod = read(SRC / "write" / "mod.rs")
    write_dispatch = {
        "create": parse_dispatch(write_mod, "create"),
        "update": parse_dispatch(write_mod, "update"),
        "delete": parse_dispatch(write_mod, "delete"),
    }

    documents, wire_by_resource = wire_documents()
    wire_by_file = {doc["file"]: doc for doc in documents}

    operation_order = collect_route_operations(routes_text)
    rows = route_rows(route_consts, allow, adapters, operation_order, served_read, write_dispatch)
    expected = 2 + len(adapters) * 7
    if len(rows) != expected:
        raise SystemExit(f"route table has {len(rows)} rows, expected {expected}")

    return render_document(
        Doc(
            adapters,
            route_consts,
            allow,
            rows,
            index,
            search_fn,
            write_dispatch,
            wire_by_resource,
            wire_by_file,
        )
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", default=str(DOCS / "api.md"))
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--print", dest="to_stdout", action="store_true")
    args = parser.parse_args()

    document = build()
    if args.to_stdout:
        sys.stdout.write(document)
        return 0
    if args.check:
        current = Path(args.out).read_text(encoding="utf-8") if Path(args.out).exists() else ""
        if current == document:
            print(f"{args.out} is up to date ({len(document.splitlines())} lines)")
            return 0
        print(f"{args.out} is stale: run ./gen.sh", file=sys.stderr)
        return 1
    Path(args.out).write_text(document, encoding="utf-8")
    print(f"wrote {args.out} ({len(document.splitlines())} lines)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
