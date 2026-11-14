"""Puts big files into the R2 bucket through the temporary upload Worker, a part at a time.

    python upload.py LOCAL_FILE=KEY_IN_BUCKET[:CONTENT_TYPE] ...

The key is read from `.key` beside this script and is never printed.
"""
import json
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

HERE = Path(__file__).resolve().parent
BASE = "https://upload.rust4x4.com"
KEY = (HERE / ".key").read_text().strip()
PART = 64 * 1024 * 1024


def call(path, query, data=None, tries=6):
    url = f"{BASE}{path}?{urllib.parse.urlencode(query)}"
    for attempt in range(tries):
        request = urllib.request.Request(
            url,
            data=data if data is not None else b"",
            method="POST",
            headers={"x-upload-key": KEY, "User-Agent": "rust4x4-uploader/1", "Content-Type": "application/octet-stream"},
        )
        try:
            with urllib.request.urlopen(request, timeout=600) as response:
                return json.loads(response.read())
        except urllib.error.HTTPError as error:
            said = error.read()[:300].decode("utf-8", "replace")
            print(f"    {path} failed ({error.code}): {said}", flush=True)
        except Exception as error:  # noqa: BLE001
            print(f"    {path} failed: {error}", flush=True)
        time.sleep(3 * (attempt + 1))
    sys.exit(f"gave up on {path}")


def upload(local, key, kind):
    size = local.stat().st_size
    began = time.monotonic()
    print(f"{local.name} -> {key} ({size / 1e6:.0f} MB)", flush=True)
    upload_id = call("/create", {"key": key, "type": kind})["uploadId"]
    parts = []
    with open(local, "rb") as file:
        number = 1
        while True:
            chunk = file.read(PART)
            if not chunk:
                break
            part = call("/part", {"key": key, "uploadId": upload_id, "n": number}, chunk)
            parts.append(part)
            done = min(number * PART, size)
            print(f"    part {number}: {done / size * 100:.0f}%", flush=True)
            number += 1
    done = call("/complete", {"key": key, "uploadId": upload_id}, json.dumps(parts).encode())
    assert done["size"] == size, (done, size)
    print(f"    done in {time.monotonic() - began:.0f} s", flush=True)


for spec in sys.argv[1:]:
    local, _, rest = spec.partition("=")
    key, _, kind = rest.partition(":")
    upload(Path(local), key, kind or "application/octet-stream")
