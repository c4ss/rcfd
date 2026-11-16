#!/usr/bin/env python3
"""Test-to-ADR index (ADR 0653 D4): which ADRs and roadmap items each test of the workspace belongs to.

Usage (from the workspace root):
    python scripts/adr_test_index.py [--out index.json] [--report] [--adr NNNN ...] [--metadata meta.json] [--root DIR]
--out       write the index as JSON
--report    print the coverage and the unmapped binaries
--adr       print the tests of these ADRs as cargo commands (the per-merge "ADR's own registered tests")
--metadata  a saved `cargo metadata --no-deps --format-version 1` (default: run cargo)

Tests are found without a build: each test target's root file is read, `mod x;` is followed, and `#[test]` functions get
their libtest names from the `mod` nesting. Evidence per test, each with its kind:
  E1 item number in the file name (`_f123_`, `f123_x`, `x_f123`), E2 an ADR cites the file, E3 an ADR cites the test function
  (distinctive names only), E4 a roadmap note cites the file or function, E5 (unit tests) an ADR cites the module path.
Items map to ADRs through ADR file names `NNNN-fMMM-...` and `ADR NNNN` in the item's roadmap note.
"""
import argparse
import json
import os
import re
import subprocess
import sys

ITEM_IN_STEM = re.compile(r"(?:^|_)f(\d{2,4})(?=_|$)")
ADR_FILE = re.compile(r"^(\d{4})((?:-f\d+)*)-")
ADR_REF = re.compile(r"\bADRs? (\d{4})(?:\s*(?:-|to|–)\s*(\d{4}))?")
RS_TOKEN = re.compile(r"[\w./\\-]*\w\.rs\b")
MOD_PATH = re.compile(r"\brcfd_\w+(?:::\w+)+")
WORD = re.compile(r"\b[A-Za-z_]\w*\b")
CRATE_IN_PATH = re.compile(r"(rcfd-[a-z0-9-]+)/((?:src|tests|benches|examples)/.+\.rs)$")


def item_id(n):
    return f"F-{int(n):03d}"


def items_in_stem(stem):
    return sorted({item_id(m) for m in ITEM_IN_STEM.findall(stem.lower())})


# ---------------------------------------------------------------- Rust source scan


RAW = re.compile(r"(?:br|r)(#*)\"")


def blank(src):
    """Comments, string and char literals replaced by spaces (newlines kept), so offsets stay and braces are code braces."""
    out = list(src)
    i, n = 0, len(src)

    def wipe(a, b):
        for k in range(a, b):
            if out[k] != "\n":
                out[k] = " "

    while i < n:
        c = src[i]
        if src.startswith("//", i):
            j = src.find("\n", i)
            j = n if j < 0 else j
            wipe(i, j)
            i = j
        elif src.startswith("/*", i):
            depth, j = 1, i + 2
            while j < n and depth:
                if src.startswith("/*", j):
                    depth, j = depth + 1, j + 2
                elif src.startswith("*/", j):
                    depth, j = depth - 1, j + 2
                else:
                    j += 1
            wipe(i, j)
            i = j
        elif c in "rb" and (i == 0 or not (src[i - 1].isalnum() or src[i - 1] == "_")) and RAW.match(src, i):
            m = RAW.match(src, i)
            close = '"' + m.group(1)
            j = src.find(close, m.end())
            j = n if j < 0 else j + len(close)
            wipe(i, j)
            i = j
        elif c == '"' or (c == "b" and src.startswith('b"', i) and (i == 0 or not (src[i - 1].isalnum() or src[i - 1] == "_"))):
            j = i + (2 if c == "b" else 1)
            while j < n and src[j] != '"':
                j += 2 if src[j] == "\\" else 1
            j = min(j + 1, n)
            wipe(i, j)
            i = j
        elif c == "'":
            m = re.match(r"'(?:\\(?:x[0-9a-fA-F]{2}|u\{[0-9a-fA-F]+\}|.)|[^\\'\n])'", src[i:i + 16])
            if m:
                wipe(i, i + m.end())
                i += m.end()
            else:
                i += 1  # a lifetime or a label
        else:
            i += 1
    return "".join(out)


TOKEN = re.compile(
    r"(?P<open>\{)|(?P<close>\})|(?P<semi>;)"
    r"|(?P<pathattr>#\[\s*path\s*=)"
    r"|(?P<test>#\[\s*(?:\w+::)?test\s*\])"
    r"|(?P<ignore>#\[\s*ignore\b)"
    r"|\bmod\s+(?P<moddecl>\w+)\s*;"
    r"|\bmod\s+(?P<modopen>\w+)\s*\{"
    r"|\bfn\s+(?P<fn>\w+)"
)


def scan_file(path, mod_path, child_dir, seen, out):
    """Append (module path list, fn name, ignored, file) for every #[test] fn in path and the module files it declares."""
    rp = os.path.normcase(os.path.abspath(path))
    if rp in seen or not os.path.isfile(path):
        return
    seen.add(rp)
    src = open(path, encoding="utf-8", errors="replace").read()
    b = blank(src)
    stack = []  # one entry per open brace: a module name or None
    attrs = set()
    pending_path = None
    for m in TOKEN.finditer(b):
        g = m.lastgroup
        if g == "open":
            stack.append(None)
            attrs.clear()
        elif g == "close":
            if stack:
                stack.pop()
            attrs.clear()
        elif g == "semi":
            attrs.clear()
        elif g == "pathattr":
            q = re.match(r'\s*"([^"]*)"', src[m.end():])
            pending_path = q.group(1) if q else None
        elif g == "test":
            attrs.add("test")
        elif g == "ignore":
            attrs.add("ignore")
        elif g == "moddecl":
            name = m.group("moddecl")
            inline = [s for s in stack if s is not None]
            if pending_path is not None:
                base = os.path.join(os.path.dirname(path), *inline) if inline else os.path.dirname(path)
                f = os.path.normpath(os.path.join(base, pending_path))
                sub_dir = os.path.dirname(f)
            else:
                d = os.path.join(child_dir, *inline)
                f = os.path.join(d, name + ".rs")
                if os.path.isfile(f):
                    sub_dir = os.path.join(d, name)
                else:
                    f = os.path.join(d, name, "mod.rs")
                    sub_dir = os.path.join(d, name)
            pending_path = None
            attrs.clear()
            scan_file(f, mod_path + inline + [name], sub_dir, seen, out)
        elif g == "modopen":
            stack.append(m.group("modopen"))
            attrs.clear()
            pending_path = None
        elif g == "fn":
            name = m.group("fn")
            if "test" in attrs:
                inline = [s for s in stack if s is not None]
                out.append((mod_path + inline, name, "ignore" in attrs, path))
            attrs.clear()


def target_tests(src_path, kind):
    """Tests of one target: list of (libtest name, ignored, file)."""
    out = []
    # a target's root file is a crate root: the files of its `mod x;` are next to it
    scan_file(src_path, [], os.path.dirname(src_path), set(), out)
    return [("::".join(mp + [fn]), ig, f) for mp, fn, ig, f in out]


# ---------------------------------------------------------------- citations


def rs_keys(text):
    """Keys of the .rs files a text cites: ('stem', stem) and ('path', crate, path in crate)."""
    keys = set()
    for tok in RS_TOKEN.findall(text):
        t = tok.replace("\\", "/")
        keys.add(("stem", os.path.splitext(os.path.basename(t))[0]))
        m = CRATE_IN_PATH.search(t)
        if m:
            keys.add(("path", m.group(1), m.group(2)))
    return keys


def adr_refs(text):
    out = set()
    for a, b in ADR_REF.findall(text):
        if b and 0 < int(b) - int(a) <= 20:
            out.update(f"{k:04d}" for k in range(int(a), int(b) + 1))
        else:
            out.add(a)
    return out


class Text:
    def __init__(self, text):
        self.rs = rs_keys(text)
        # a cited path `rcfd_x::a::f` also cites its parents `rcfd_x::a`
        self.mods = {"::".join(p.split("::")[:k]) for p in MOD_PATH.findall(text) for k in range(2, p.count("::") + 2)}
        self.words = set(WORD.findall(text))


def load_docs(root):
    adr_dir = os.path.join(root, "docs", "adr")
    adrs = {}
    for fn in sorted(os.listdir(adr_dir)) if os.path.isdir(adr_dir) else []:
        m = ADR_FILE.match(fn)
        if not m or not fn.endswith(".md"):
            continue
        text = open(os.path.join(adr_dir, fn), encoding="utf-8", errors="replace").read()
        items = sorted({item_id(x) for x in re.findall(r"f(\d+)", m.group(2))})
        adrs.setdefault(m.group(1), {"files": [], "items": set(), "text": []})
        a = adrs[m.group(1)]
        a["files"].append("docs/adr/" + fn)
        a["items"].update(items)
        a["text"].append(text)
    for a in adrs.values():
        a["cite"] = Text("\n".join(a.pop("text")))
    roadmap = {}
    rp = os.path.join(root, "docs", "roadmap.json")
    if os.path.isfile(rp):
        data = json.load(open(rp, encoding="utf-8"))
        for it in data if isinstance(data, list) else data.get("items", []):
            roadmap[it["id"]] = {"cite": Text(it.get("note") or ""), "adrs": adr_refs(it.get("note") or "")}
    item_adrs = {}
    for n, a in adrs.items():
        for it in a["items"]:
            item_adrs.setdefault(it, set()).add(n)
    for it, r in roadmap.items():
        for n in r["adrs"]:
            if n in adrs:
                item_adrs.setdefault(it, set()).add(n)
    adr_items = {}
    for it, ns in item_adrs.items():
        for n in ns:
            adr_items.setdefault(n, set()).add(it)
    return adrs, roadmap, item_adrs, adr_items


# ---------------------------------------------------------------- the index


def metadata(root, path=None):
    if path:
        return json.load(open(path, encoding="utf-8"))
    out = subprocess.run(["cargo", "metadata", "--no-deps", "--format-version", "1"], cwd=root, stdout=subprocess.PIPE,
                         check=True).stdout
    return json.loads(out)


def rel(root, p):
    return os.path.relpath(p, root).replace("\\", "/")


def build(root, meta):
    adrs, roadmap, item_adrs, adr_items = load_docs(root)
    ws_root = meta.get("workspace_root", root)
    pkgs = meta["packages"]
    names = {p["name"] for p in pkgs}
    crate_dir = {p["name"]: rel(ws_root, os.path.dirname(p["manifest_path"])) for p in pkgs}
    deps = {p["name"]: sorted({d["name"] for d in p["dependencies"] if d["name"] in names and d.get("path")}) for p in pkgs}

    binaries, tests = {}, []
    for p in pkgs:
        for t in p["targets"]:
            kind = t["kind"][0] if t["kind"] else ""
            if kind not in ("lib", "bin", "test") or not t.get("test", True):
                continue
            ts = target_tests(t["src_path"], "test" if kind == "test" else kind)
            stem = t["name"].replace("-", "_")
            b = {"crate": p["name"], "target": t["name"], "kind": kind, "file": rel(ws_root, t["src_path"]),
                 "crate_dir": crate_dir[p["name"]], "tests": len(ts)}
            binaries.setdefault(stem, []).append(b)
            for name, ig, f in ts:
                tests.append({"crate": p["name"], "binary": stem, "target": t["name"], "kind": kind,
                              "file": rel(ws_root, f), "name": name, "ignored": ig})

    # uniqueness of file stems and test function names (E2, E3)
    stem_count, fn_bins = {}, {}
    for p in pkgs:
        d = os.path.dirname(p["manifest_path"])
        for sub in ("src", "tests"):
            for dp, _, fs in os.walk(os.path.join(d, sub)):
                for f in fs:
                    if f.endswith(".rs"):
                        s = f[:-3]
                        stem_count[s] = stem_count.get(s, 0) + 1
    for t in tests:
        fn_bins.setdefault(t["name"].rsplit("::", 1)[-1], set()).add((t["crate"], t["binary"]))

    def file_keys(t):
        crate_rel = os.path.relpath(os.path.join(ws_root, t["file"]), os.path.join(ws_root, crate_dir[t["crate"]]))
        crate_rel = crate_rel.replace("\\", "/")
        stem = os.path.splitext(os.path.basename(t["file"]))[0]
        keys = {("path", t["crate"], crate_rel)}
        if stem_count.get(stem, 0) == 1 and stem not in ("lib", "main", "mod"):
            keys.add(("stem", stem))
        return keys

    def distinctive(fn, t):
        return "_" in fn and len(fn) >= 12 and len(fn_bins.get(fn, ())) == 1

    for t in tests:
        ev = []
        fn = t["name"].rsplit("::", 1)[-1]
        stem = os.path.splitext(os.path.basename(t["file"]))[0]
        for s in {stem, os.path.splitext(os.path.basename(binaries[t["binary"]][0]["file"]))[0]}:
            for it in items_in_stem(s):
                ev.append({"kind": "E1", "item": it, "from": s})
        fk = file_keys(t)
        dist = distinctive(fn, t)
        for n, a in adrs.items():
            if fk & a["cite"].rs:
                ev.append({"kind": "E2", "adr": n})
            if dist and fn in a["cite"].words:
                ev.append({"kind": "E3", "adr": n})
        for it, r in roadmap.items():
            if fk & r["cite"].rs:
                ev.append({"kind": "E4", "item": it, "from": "file"})
            elif dist and fn in r["cite"].words:
                ev.append({"kind": "E4", "item": it, "from": "function"})
        if t["kind"] != "test":
            mods = t["name"].split("::")[:-1]
            lib = t["target"].replace("-", "_") if t["kind"] == "lib" else t["crate"].replace("-", "_")
            for k in range(len(mods), 0, -1):
                path = lib + "::" + "::".join(mods[:k])
                hit = [n for n, a in adrs.items() if path in a["cite"].mods]
                if hit:
                    ev.extend({"kind": "E5", "adr": n, "module": path} for n in hit)
                    break
        items = sorted({e["item"] for e in ev if "item" in e})
        found = {e["adr"] for e in ev if "adr" in e}
        for it in items:
            found |= item_adrs.get(it, set())
        t["adrs"] = sorted(found)
        t["items"] = sorted(set(items) | {i for n in found for i in adr_items.get(n, ())})
        t["evidence"] = ev

    by_adr = {}
    for t in tests:
        for n in t["adrs"]:
            by_adr.setdefault(n, []).append(f"{t['crate']}/{t['binary']}::{t['name']}")
    idx = {
        "about": "ADR 0653 test-to-ADR index; tests keyed by crate, binary (times.tsv stem) and libtest name",
        "binaries": binaries,
        "crates": {c: {"dir": crate_dir[c], "deps": deps[c]} for c in sorted(names)},
        "tests": tests,
        "adrs": {n: {"files": a["files"], "items": sorted(adr_items.get(n, set()) | a["items"]),
                     "tests": sorted(by_adr.get(n, []))} for n, a in sorted(adrs.items())},
        "items": {it: sorted(ns) for it, ns in sorted(item_adrs.items())},
    }
    idx["coverage"] = coverage(idx)
    return idx


def coverage(idx):
    tests = idx["tests"]

    def share(ts):
        n = len(ts)
        m = sum(1 for t in ts if t["adrs"])
        io = sum(1 for t in ts if not t["adrs"] and t["items"])
        return {"tests": n, "mapped": m, "item_only": io, "share": round(m / n, 4) if n else 0.0}

    bins = {}
    for t in tests:
        bins.setdefault((t["crate"], t["binary"], t["kind"]), []).append(t)
    for stem, bs in idx["binaries"].items():
        for b in bs:
            bins.setdefault((b["crate"], stem, b["kind"]), [])

    def bshare(kinds):
        ks = [k for k in bins if k[2] in kinds]
        m = [k for k in ks if any(t["adrs"] for t in bins[k])]
        return {"binaries": len(ks), "mapped": len(m), "share": round(len(m) / len(ks), 4) if ks else 0.0}

    unmapped = sorted(f"{k[0]}/{k[1]} ({k[2]}, {len(v)} tests)" for k, v in bins.items() if not any(t["adrs"] for t in v))
    return {
        "integration_binaries": bshare({"test"}),
        "unit_binaries": bshare({"lib", "bin"}),
        "all_tests": share(tests),
        "non_ignored_tests": share([t for t in tests if not t["ignored"]]),
        "integration_tests": share([t for t in tests if t["kind"] == "test"]),
        "unit_tests": share([t for t in tests if t["kind"] != "test"]),
        "unmapped_binaries": unmapped,
    }


def adr_commands(idx, adr):
    """cargo commands that run the tests the index maps to one ADR, one command per binary."""
    adr = f"{int(adr):04d}"
    per = {}
    for t in idx["tests"]:
        if adr in t["adrs"]:
            per.setdefault((t["crate"], t["kind"], t["target"]), []).append(t)
    cmds = []
    for (crate, kind, target), ts in sorted(per.items()):
        sel = "--lib" if kind == "lib" else f"--bin {target}" if kind == "bin" else f"--test {target}"
        if kind == "test" and len(ts) == sum(1 for t in idx["tests"] if t["target"] == target and t["crate"] == crate):
            cmds.append(f"cargo test -p {crate} --release {sel}")
        else:
            for t in ts:
                cmds.append(f"cargo test -p {crate} --release {sel} -- --exact {t['name']}" +
                            (" --include-ignored" if t["ignored"] else ""))
    return cmds


def main(argv=None):
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", default=".")
    ap.add_argument("--metadata")
    ap.add_argument("--out")
    ap.add_argument("--report", action="store_true")
    ap.add_argument("--adr", nargs="*", default=[])
    a = ap.parse_args(argv)
    root = os.path.abspath(a.root)
    idx = build(root, metadata(root, a.metadata))
    if a.out:
        with open(a.out, "w", encoding="utf-8") as f:
            json.dump(idx, f, indent=1)
    if a.report:
        c = idx["coverage"]
        for k in ("integration_binaries", "unit_binaries"):
            print(f"{k}: {c[k]['mapped']} of {c[k]['binaries']} mapped to an ADR ({100 * c[k]['share']:.1f} %)")
        for k in ("all_tests", "non_ignored_tests", "integration_tests", "unit_tests"):
            print(f"{k}: {c[k]['mapped']} of {c[k]['tests']} mapped to an ADR ({100 * c[k]['share']:.1f} %), "
                  f"{c[k]['item_only']} to an item only")
        print(f"unmapped binaries ({len(c['unmapped_binaries'])}):")
        for u in c["unmapped_binaries"]:
            print("  " + u)
    for n in a.adr:
        for cmd in adr_commands(idx, n):
            print(cmd)
    return 0


if __name__ == "__main__":
    sys.exit(main())
