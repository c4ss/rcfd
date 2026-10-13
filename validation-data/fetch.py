#!/usr/bin/env python3
"""Fetch and verify the public validation data of rcfd (F-125).

Only open upstream sources listed in manifest.json are used. Nothing is read from any
proprietary install. CI and the test suite never run the network commands.

Commands (standard library only):
  list                      datasets, licences and status
  fetch [id ...]            download missing files into data/<id>/ and verify SHA-256
  verify [id ...]           check local files (data/ and in-repo reference tables) offline
  pin [id ...]              maintainer tool: (re)compute sha256/bytes of the URLs and local
                            files and write them into manifest.json
Options: --data DIR (default: <this dir>/data), --manifest FILE.
Exit status: 0 ok, 1 checksum mismatch or missing file, 2 usage error.
"""
import hashlib
import json
import os
import sys
import urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)


def sha256_of(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def load(path):
    with open(path, encoding="utf-8") as f:
        return json.load(f)


def download(url, dest):
    req = urllib.request.Request(url, headers={"User-Agent": "rcfd-fetch/1"})
    os.makedirs(os.path.dirname(dest), exist_ok=True)
    tmp = dest + ".part"
    with urllib.request.urlopen(req, timeout=120) as r, open(tmp, "wb") as f:
        while True:
            b = r.read(1 << 20)
            if not b:
                break
            f.write(b)
    os.replace(tmp, dest)


def select(man, ids):
    ds = man["datasets"]
    if ids:
        known = {d["id"] for d in ds}
        bad = [i for i in ids if i not in known]
        if bad:
            sys.exit("unknown dataset(s): %s (known: %s)" % (", ".join(bad), ", ".join(sorted(known))))
        ds = [d for d in ds if d["id"] in ids]
    return ds


def local_path(data_dir, ds, f):
    if "local" in f:
        return os.path.join(REPO, f["local"])
    return os.path.join(data_dir, ds["id"], f["path"])


def main(argv):
    args = list(argv)
    data_dir = os.path.join(HERE, "data")
    manifest = os.path.join(HERE, "manifest.json")
    for opt in ("--data", "--manifest"):
        if opt in args:
            i = args.index(opt)
            if i + 1 >= len(args):
                print("%s needs a value" % opt, file=sys.stderr)
                return 2
            val = args[i + 1]
            del args[i : i + 2]
            if opt == "--data":
                data_dir = val
            else:
                manifest = val
    if not args:
        print(__doc__)
        return 2
    cmd, ids = args[0], args[1:]
    man = load(manifest)
    ds = select(man, ids)
    rc = 0
    if cmd == "list":
        for d in ds:
            print("%-32s %-12s %-14s %s" % (d["id"], d["status"], d["licence"]["spdx"], d["title"]))
        return 0
    for d in ds:
        if d["status"] != "available":
            print("[%s] %s: %s" % (d["id"], d["status"], d.get("unresolved_reason", "")))
            continue
        for f in d["files"]:
            p = local_path(data_dir, d, f)
            name = f.get("local") or f["path"]
            if cmd == "fetch" and "url" in f and not os.path.exists(p):
                print("[%s] downloading %s" % (d["id"], f["url"]))
                download(f["url"], p)
            if cmd == "pin":
                if "url" in f and not os.path.exists(p):
                    download(f["url"], p)
                if not os.path.exists(p):
                    print("[%s] missing %s" % (d["id"], p))
                    rc = 1
                    continue
                f["sha256"] = sha256_of(p)
                f["bytes"] = os.path.getsize(p)
                print("[%s] pinned %s %s" % (d["id"], name, f["sha256"][:12]))
                continue
            if cmd in ("fetch", "verify"):
                if not os.path.exists(p):
                    print("[%s] MISSING %s (run: fetch.py fetch %s)" % (d["id"], name, d["id"]))
                    if "local" in f or cmd == "fetch":
                        rc = 1
                    continue
                got = sha256_of(p)
                if got != f["sha256"]:
                    print("[%s] CHECKSUM MISMATCH %s: expected %s got %s" % (d["id"], name, f["sha256"], got))
                    rc = 1
                else:
                    print("[%s] ok %s" % (d["id"], name))
    if cmd == "pin":
        with open(manifest, "w", encoding="utf-8", newline="\n") as fh:
            json.dump(man, fh, indent=2, ensure_ascii=False)
            fh.write("\n")
    elif cmd not in ("fetch", "verify"):
        print("unknown command %s" % cmd, file=sys.stderr)
        return 2
    return rc


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
