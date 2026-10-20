"""Drive a fresh `romeka` against one MySQL or MariaDB database through its real routes.

Usage: python3 crates/omeka-cli/scripts/mysql-smoke.py ROMEKA_BINARY DATABASE_URL PORT WORKDIR

DATABASE_URL names an empty database (`mysql://root:secret@127.0.0.1:13306/romeka_smoke`, or
`mariadb://...`); PORT is a free port; WORKDIR an empty folder for the archive, the store and the
server log.

install, then serve; admin login; an item with an uploaded PNG in a public site; search finds
it; the public item page shows it; an import v2 run of the S1 folder (the job runs in a child
`romeka job run` process, which reaches the database through ROMEKA_DATABASE_URL) and its undo;
backup refuses. Prints one line per step and exits non-zero on the first failure.
"""
import base64
import http.cookiejar
import json
import os
import subprocess
import sys
import time
import urllib.error
import urllib.request

ROMEKA, URL, PORT, WORK = sys.argv[1], sys.argv[2], int(sys.argv[3]), sys.argv[4]
BASE = f"http://127.0.0.1:{PORT}"
EMAIL, PASSWORD = "admin@example.org", "correct horse battery"
PNG = base64.b64decode(
    "iVBORw0KGgoAAAANSUhEUgAAAAgAAAAICAYAAADED76LAAAAEklEQVR4nGM4ISf3Hx9mGBkKAFNGgMHUn6b2AAAAAElFTkSuQmCC"
)
JPEG = b"\xFF\xD8\xFF\xE0\x00\x10JFIF\x00\x01\x01\x00\x00\x01\x00\x01\x00\x00"
TIFF = b"II*\x00\x08\x00\x00\x00\x00\x00"
PATTERN = r"^(?<set>[^/]+)/(?:[^/]+/)*(?<identifier>BHC_006_[A-Z]{2}_\d{3})(?:_(?<part>[a-z0-9]+))?\.(?:jpg|tif)$"

jar = http.cookiejar.CookieJar()
opener = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(jar))


def step(name, ok, detail=""):
    print(f"{'ok  ' if ok else 'FAIL'} {name} {detail}".rstrip(), flush=True)
    if not ok:
        sys.exit(1)


def call(method, path, body=None, raw=None, content_type=None):
    data = raw if raw is not None else (json.dumps(body).encode() if body is not None else None)
    request = urllib.request.Request(f"{BASE}{path}", data=data, method=method)
    request.add_header("Accept", "application/json")
    request.add_header("Origin", BASE)
    if data is not None:
        request.add_header("Content-Type", content_type or "application/json")
    try:
        with opener.open(request, timeout=60) as response:
            return response.status, response.read().decode()
    except urllib.error.HTTPError as error:
        return error.code, error.read().decode()


def as_json(text):
    try:
        return json.loads(text)
    except ValueError:
        return None


def write_objects(root):
    files = []
    for n in range(1, 5):
        files.append((f"Artifacts/BHC_006_AF_00{n}_front.jpg", JPEG))
        files.append((f"Artifacts/BHC_006_AF_00{n}_back.jpg", JPEG))
        files.append((f"Photographs/1950s/BHC_006_PH_10{n}.tif", TIFF))
    for page in ["p1", "p2", "p10"]:
        files.append((f"Scans/BHC_006_SC_001_{page}.tif", TIFF))
    for path, data in files:
        full = os.path.join(root, path)
        os.makedirs(os.path.dirname(full), exist_ok=True)
        with open(full, "wb") as out:
            out.write(data + path.encode())
    os.makedirs(os.path.join(root, "_scratch"), exist_ok=True)
    with open(os.path.join(root, "_scratch/notes.txt"), "wb") as out:
        out.write(b"call the donor about AF_003\n")


def wait_import(import_id, wanted, seconds=120):
    deadline = time.time() + seconds
    row = None
    while time.time() < deadline:
        status, text = call("GET", f"/api/romeka/imports/{import_id}")
        row = as_json(text) or {}
        if row.get("status") in wanted:
            return row
        time.sleep(0.5)
    return row


archive = os.path.join(WORK, "archive")
files = os.path.join(WORK, "files")
os.makedirs(archive, exist_ok=True)
write_objects(archive)
env = dict(os.environ, ROMEKA_DATABASE_URL=URL, ROMEKA_ARCHIVE_ROOT=archive, ROMEKA_LOG="warn")

installed = subprocess.run(
    [ROMEKA, "install", "--email", EMAIL, "--name", "Admin", "--password", PASSWORD],
    env=env, capture_output=True, text=True,
)
step("install", installed.returncode == 0, installed.stdout.splitlines()[0] if installed.stdout else installed.stderr)
again = subprocess.run(
    [ROMEKA, "install", "--email", EMAIL, "--name", "Admin", "--password", PASSWORD],
    env=env, capture_output=True, text=True,
)
step("install again is a no-op", again.returncode == 0 and "already present" in again.stdout)

log = open(os.path.join(WORK, "serve.log"), "w")
server = subprocess.Popen(
    [ROMEKA, "serve", "--port", str(PORT), "--store", files], env=env, stdout=log, stderr=log, cwd=WORK
)
try:
    for _ in range(100):
        try:
            status, _ = call("GET", "/ready")
            if status == 200:
                break
        except urllib.error.URLError:
            pass
        time.sleep(0.2)
    step("ready", status == 200)

    status, text = call("POST", "/api/login", {"email": EMAIL, "password": PASSWORD})
    step("admin login", status == 200, f"{status}")
    status, text = call("GET", "/api/me")
    step("session reads the admin", status == 200 and EMAIL in text)

    status, text = call("GET", "/api/properties?term=dcterms:title")
    title = as_json(text)[0]["o:id"]
    status, text = call("POST", "/api/sites", {
        "o:slug": "smoke", "o:title": "Smoke site", "o:theme": "default", "o:is_public": True,
    })
    site = as_json(text)
    step("site create", status == 200 and site and site.get("o:id"), f"{status} {text[:200] if status != 200 else ''}")

    data = {
        "o:is_public": True,
        "dcterms:title": [{"type": "literal", "property_id": title, "@value": "Amphora from the smoke run"}],
        "o:site": [{"o:id": site["o:id"]}],
        "o:media": [{"o:ingester": "upload", "file_index": 0, "o:is_public": True}],
    }
    boundary = "smokeboundary"
    raw = (
        f"--{boundary}\r\nContent-Disposition: form-data; name=\"data\"\r\n\r\n{json.dumps(data)}\r\n"
        f"--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"sample.png\"\r\n"
        f"Content-Type: image/png\r\n\r\n"
    ).encode() + PNG + f"\r\n--{boundary}--\r\n".encode()
    status, text = call("POST", "/api/items", raw=raw, content_type=f"multipart/form-data; boundary={boundary}")
    item = as_json(text)
    step("item create with media", status == 200 and item and len(item.get("o:media", [])) == 1,
         f"{status} {text[:300] if status != 200 else ''}")
    item_id = item["o:id"]
    status, text = call("GET", f"/api/media/{item['o:media'][0]['o:id']}")
    media = as_json(text)
    step("media stored", status == 200 and media.get("o:media_type") == "image/png")
    stored = os.path.join(files, "original", media["o:filename"])
    step("original on disk", os.path.isfile(stored), stored)

    status, text = call("GET", "/api/items?fulltext_search=amphora")
    found = [row["o:id"] for row in (as_json(text) or [])]
    step("search finds the item", status == 200 and item_id in found, f"{found}")
    status, text = call("GET", "/api/items?search=amphora")
    found = [row["o:id"] for row in (as_json(text) or [])]
    step("keyword search finds the item", status == 200 and item_id in found, f"{found}")

    status, text = call("GET", f"/s/smoke/item/{item_id}")
    step("public item page", status == 200 and "Amphora from the smoke run" in text, f"{status}")
    status, text = call("GET", "/s/smoke/item?fulltext_search=amphora")
    step("public site search", status == 200 and "Amphora from the smoke run" in text, f"{status}")

    status, text = call("POST", "/api/romeka/imports", {
        "sources": [{"kind": "folder", "id": "s1", "root": "archive", "path": ""}]
    })
    draft = as_json(text)
    step("import draft", status == 201, f"{status} {text[:200] if status != 201 else ''}")
    plan = draft["plan"]
    plan["grouping"]["pattern"] = PATTERN
    plan["slots"]["item"]["sets"] = {"sources": [{"from": "group", "name": "set"}], "combine": "first"}
    status, text = call("PUT", f"/api/romeka/imports/{draft['id']}", {"plan": plan, "revision": draft["revision"]})
    step("import plan save", status == 200, f"{status} {text[:200] if status != 200 else ''}")
    status, text = call("POST", f"/api/romeka/imports/{draft['id']}/scan")
    step("import scan", status == 200 and as_json(text)["sources"][0]["files"] == 16, text[:200])
    status, text = call("POST", f"/api/romeka/imports/{draft['id']}/preview", {"include": ["check"]})
    check = (as_json(text) or {}).get("check", {})
    creates = sum(row["count"] for row in check.get("counts", []) if row["kind"] == "item" and row["does"] == "create")
    step("import check", status == 200 and check.get("canRun") and creates == 9, f"{creates} item creates")
    status, text = call("POST", f"/api/romeka/imports/{draft['id']}/run", {"digest": check["digest"]})
    step("import run starts", status == 200, f"{status} {text[:200]}")
    row = wait_import(draft["id"], {"done", "run_failed"})
    step("import run done", row.get("status") == "done", json.dumps(row.get("summary", row))[:300])
    status, text = call("GET", "/api/items?fulltext_search=BHC_006_AF_001")
    status, text = call("GET", "/api/item_sets?per_page=50")
    sets = sorted(s["o:title"] for s in as_json(text) if s.get("o:title") in ("Artifacts", "Photographs", "Scans"))
    step("import made the sets", sets == ["Artifacts", "Photographs", "Scans"], f"{sets}")
    status, text = call("GET", "/api/items?per_page=100")
    after_run = len(as_json(text))
    step("import made nine items", after_run == 10, f"{after_run} items")

    status, text = call("GET", f"/api/romeka/imports/{draft['id']}/undo/preview")
    step("undo preview", status == 200, text[:200])
    status, text = call("POST", f"/api/romeka/imports/{draft['id']}/undo", {})
    step("undo starts", status == 200, f"{status} {text[:200]}")
    row = wait_import(draft["id"], {"undone", "undo_failed"})
    step("undo done", row.get("status") == "undone", json.dumps(row.get("summary", row))[:300])
    status, text = call("GET", "/api/items?per_page=100")
    left = [row["o:id"] for row in as_json(text)]
    step("undo left only the hand-made item", left == [item_id], f"{left}")

    backup = subprocess.run(
        [ROMEKA, "backup", "--store", files, "--out", os.path.join(WORK, "backup.tar")],
        env=env, capture_output=True, text=True,
    )
    step("backup refuses", backup.returncode != 0 and "mysqldump --single-transaction" in backup.stderr
         and not os.path.exists(os.path.join(WORK, "backup.tar")), backup.stderr.strip())
finally:
    server.terminate()
    server.wait(timeout=10)
    log.close()
print("all steps passed")
