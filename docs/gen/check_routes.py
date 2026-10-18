#!/usr/bin/env python3
"""Compare the generated handler column with live responses from the port.

Reads the route table out of `api.md`, sends every row's request to a running
instance with a global-admin key, and checks the rows that make an exact claim:

  501 rows must answer 501, 500 rows must answer 500, served rows must not.

It creates rows (a create with an empty body mostly succeeds), so point it at a
throwaway database and a port nobody else is using:

  omeka install -d /tmp/proof/omeka.db --email a@b.c --name A --password x
  omeka key add -d /tmp/proof/omeka.db --owner a@b.c
  omeka serve -d /tmp/proof/omeka.db --port 18081 &
  ./check_routes.py http://127.0.0.1:18081 <identity> <credential>

The two root rows of the table are skipped: they are not per-resource.
"""

from __future__ import annotations

import re
import sys
import urllib.error
import urllib.request
from collections import Counter
from pathlib import Path

TABLE_ROW = re.compile(r"^\| ([a-z_()]+) \| (GET|POST|PUT|PATCH|DELETE) \| (/[^ ]*) \| ([^|]+) \| ([^|]+) \|$")


def request(base: str, method: str, path: str, key: tuple[str, str]) -> int:
    url = f"{base}{path}?key_identity={key[0]}&key_credential={key[1]}"
    body = b"{}" if method in ("POST", "PUT", "PATCH") else None
    req = urllib.request.Request(url, data=body, method=method)
    req.add_header("Content-Type", "application/json")
    try:
        with urllib.request.urlopen(req, timeout=10) as answer:
            return answer.status
    except urllib.error.HTTPError as error:
        return error.code
    except urllib.error.URLError as error:
        raise SystemExit(f"cannot reach {url}: {error}")


def main() -> int:
    base, identity, credential = sys.argv[1], sys.argv[2], sys.argv[3]
    rows = []
    for line in (Path(__file__).resolve().parent.parent / "api.md").read_text().splitlines():
        match = TABLE_ROW.match(line)
        if match:
            resource, method, path, operation, handler = match.groups()
            rows.append((resource, method, path, operation.strip(), handler.strip()))

    counts: Counter[str] = Counter()
    wrong = []
    for resource, method, path, _operation, handler in rows:
        if resource == "(root)":
            continue
        target = path.replace("{resource}", "items" if resource == "(root)" else resource)
        target = target.replace("{id}", "1")
        status = request(base, method, target, (identity, credential))
        counts[str(status)] += 1
        if handler.startswith("501") and status != 501:
            wrong.append((resource, method, target, handler, status))
        elif handler.startswith("500") and status != 500:
            wrong.append((resource, method, target, handler, status))
        elif not handler.startswith(("500", "501")) and status in (500, 501):
            wrong.append((resource, method, target, handler, status))

    print(f"rows checked: {len(rows) - 2}")
    print("status distribution:", dict(sorted(counts.items())))
    print(f"claims contradicted: {len(wrong)}")
    for resource, method, target, handler, status in wrong:
        print(f"  {method} {target} expected [{handler}] got {status}")
    return 1 if wrong else 0


if __name__ == "__main__":
    raise SystemExit(main())
