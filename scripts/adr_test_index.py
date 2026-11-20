#!/usr/bin/env python3
"""Test-to-ADR index (ADR 0653 D4): which ADRs and roadmap items each test of the workspace belongs to.

Usage (from the workspace root):
    python scripts/adr_test_index.py [--out index.json] [--report] [--adr NNNN ...] [--metadata meta.json] [--root DIR]
--out       write the index as JSON
--report    print the coverage and the unmapped binaries
--primary   primary-ADR rule: git (default, ADR 0690), registered (ADR 0684) or p0679
--adr       print the tests of these ADRs as cargo commands (the per-merge "ADR's own registered tests")
--metadata  a saved `cargo metadata --no-deps --format-version 1` (default: run cargo)

Tests are found without a build: each test target's root file is read, `mod x;` is followed, and `#[test]` functions get
their libtest names from the `mod` nesting. Evidence per test, each with its kind:
  E1 item number in the file name (`_f123_`, `f123_x`, `x_f123`), E2 an ADR cites the file, E3 an ADR cites the test function
  (distinctive names only), E4 a roadmap note cites the file or function, E5 (unit tests) an ADR cites the module path,
  E0 (ADR 0679 amendment 1) the first `ADR NNNN` of the file's leading comment block, E6 (ADR 0679) the wave tag of the file or binary (`w34a`, `wave17a`) equals the tag of exactly one ADR named `NNNN-wave34a-...`
  (a tag with several ADRs or only a prefix match is ambiguous and listed, not mapped), E7 (ADR 0679, integration tests) the binary
  name occurs as a word in at most 3 ADRs.
Items map to ADRs through ADR file names `NNNN-fMMM-...` and `ADR NNNN` in the item's roadmap note.
Each test with an ADR also gets `primary_adr` (ADR 0679 rule P): the first tier of E0 > E1 > E6 > E2 > E3 > E7 > E5 > E4 that has an ADR;
within the tier an ADR file named `registration` or `-wave` first, then the lowest number.
`--primary registered` (the default of the command line, ADR 0684 rule R) takes the primary from E0 > E1 > E6 > E2 only: E3, E4, E5
and E7 stay evidence (`adrs`) but give no primary (`null`), and in E2 the lowest ADR number that cites the file wins, the listing
ADRs 0653, 0679 and 0684 left out. The library call `build()` keeps rule P unless `primary_rule="registered"` or `"git"` is given.
`--primary git` (the default of the command line, ADR 0690 rule G) is rule R, except that a test whose primary evidence is E2 takes
its primary from the first git commit that adds its file (renames followed): the first `ADR dddd` of the commit subject, else the
registration ADR of the first `F-ddd` of the subject, and only when that ADR is one of the test's `adrs`; else no primary. On a
shallow clone, outside a git repository or when git fails, those tests have no primary; `coverage.primary.git` says which.
`--primary git` also applies rule H (ADR 0701): H0, a leading comment block with no `ADR dddd` but a `docs/adr/dddd-....md` path gives E0;
H1 to H3, a file-citation-only test that rule G left without a primary takes the first ADR (tier, then number) with a registering unit
that cites its file: H1 a unit in a `## Test files (added <date>, ADR dddd)` section, H2 a unit that says `without an ADR`, `ADR-less`
or `no ADR describes ...`. Rule H3 of ADR 0701 (a table row with the file in one cell and a limit in another) is removed (ADR 0703: it read
1 of 5, audit and test-list rows) and its code is deleted (ADR 0712). `coverage.primary.retro` keeps the key `h3`, always 0.
`--primary git` also applies rule E8 (ADR 0720; `--no-e8` gives the output of ADR 0690, the library default of `build()` is `e8=False`):
on a full history the ADRs named in the subject of the commit that first adds a test's file (every `ADR dddd` that exists, then the
registration ADR of every `F-ddd` that has one) are evidence of kind E8 of its tests (`adrs`, `items`, `evidence` extended, no
coherence step), and the first of them is the primary in the tier order E0 > E1 > E6 > E8 > E2.
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
WAVE_IN_STEM = re.compile(r"(?:^|_)(?:w|wave)(\d{2})([a-z]\d?)?(?=_|$)")
WAVE_ADR = re.compile(r"^(\d{4})-wave(\d{2})([a-z]\d?)?-")
NOT_EVIDENCE = {"0653", "0679"}  # ADRs that list test binaries: they are no evidence of which ADR registered one
E7_MAX_ADRS, E7_MIN_LEN = 3, 10
TIERS = ("E0", "E1", "E6", "E2", "E3", "E7", "E5", "E4")  # rule P of ADR 0679 (`--primary p0679`)
REGISTERED_TIERS = ("E0", "E1", "E6", "E2")  # rule R of ADR 0684: the evidence that registers or first describes a file
LISTING_ADRS = {"0653", "0679", "0684"}  # ADRs that list binaries or sample files: no E2 evidence for the primary (ADR 0684)
RETRO_EXCLUDED = LISTING_ADRS | {"0690", "0701"}  # ADRs whose tables quote sample files: no source of rule H (ADR 0701)
RETRO_SECTION = re.compile(r"^#{2,3}\s+Test files \(added \d{4}-\d{2}-\d{2}, ADR \d{4}\)\s*$")
RETRO_PHRASE = re.compile(r"without an ADR|ADR-less|had no ADR|no ADR (?:describes|registers|registered|states|names|covers)", re.I)
WORD = re.compile(r"\b[A-Za-z_]\w*\b")
ADR_IN_SUBJECT = re.compile(r"\bADRs? (\d{4})\b")
ITEM_IN_SUBJECT = re.compile(r"\bF-(\d{2,4})\b")
E8_TIERS = ("E0", "E1", "E6", "E8", "E2")  # ADR 0720: the tier order of rule G with E8 (the ADR the adding commit names)
CRATE_IN_PATH = re.compile(r"(rcfd-[a-z0-9-]+)/((?:src|tests|benches|examples)/.+\.rs)$")


def item_id(n):
    return f"F-{int(n):03d}"


HEADER_ADR_PATH = re.compile(r"\bdocs/adr/(\d{4})-[\w.-]+\.md")


def header_adr(path, known, cache={}, by_path=False):
    """E0 (ADR 0679 amendment 1): the first `ADR NNNN` of the leading comment block of a Rust file (lines before the first line of
    code), when that ADR exists and is not one of the ADRs that list test binaries. With `by_path` (ADR 0701 rule H0), a block without
    an `ADR NNNN` falls back to the first `docs/adr/NNNN-....md` path of the block."""
    if (path, by_path) not in cache:
        found = via_path = None
        try:
            with open(path, encoding="utf-8", errors="replace") as fh:
                for ln in fh:
                    st = ln.strip()
                    if st and not st.startswith("//"):
                        break
                    m = re.search(r"\bADR (\d{4})\b", st)
                    if m:
                        found = m.group(1)
                        break
                    if by_path and via_path is None:
                        m = HEADER_ADR_PATH.search(st)
                        if m:
                            via_path = m.group(1)
        except OSError:
            pass
        cache[(path, by_path)] = found or via_path
    n = cache[(path, by_path)]
    return n if n in known and n not in NOT_EVIDENCE else None


def waves_in_stem(stem):
    """Wave tags of a file or binary stem: `w34a` and `wave34a` give `34a`, `w22` gives `22`."""
    return sorted({n + (c or "") for n, c in WAVE_IN_STEM.findall(stem.lower())})


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


def retro_units(text):
    """Rule H of ADR 0701: the registering units of one ADR text as (tier, keys): tier 1 a unit inside a `Test files (added ..., ADR dddd)`
    section, 2 a paragraph or table row with a `without an ADR` phrase (rule H3 is removed: ADRs 0703 and 0712). keys are the .rs file
    keys the unit cites (rs_keys). A paragraph is the lines up to a blank line, a heading or a table row."""
    out, para, in_section = [], [], False

    def flush():
        if para:
            unit = " ".join(para)
            keys = rs_keys(unit)
            if keys:
                if in_section:
                    out.append((1, keys))
                if RETRO_PHRASE.search(unit):
                    out.append((2, keys))
            para.clear()

    for ln in text.splitlines():
        st = ln.strip()
        if st.startswith("#"):
            flush()
            if re.match(r"#{1,2}\s", st):
                in_section = False
            if RETRO_SECTION.match(st):
                in_section = True
        elif st.startswith("|"):
            flush()
            keys = rs_keys(st)
            if keys:
                if in_section:
                    out.append((1, keys))
                if RETRO_PHRASE.search(st):
                    out.append((2, keys))
        elif not st:
            flush()
        else:
            para.append(st)
    flush()
    return out


def load_docs(root):
    adr_dir = os.path.join(root, "docs", "adr")
    adrs = {}
    for fn in sorted(os.listdir(adr_dir)) if os.path.isdir(adr_dir) else []:
        m = ADR_FILE.match(fn)
        if not m or not fn.endswith(".md"):
            continue
        text = open(os.path.join(adr_dir, fn), encoding="utf-8", errors="replace").read()
        items = sorted({item_id(x) for x in re.findall(r"f(\d+)", m.group(2))})
        adrs.setdefault(m.group(1), {"files": [], "items": set(), "text": [], "waves": set(), "retro": []})
        a = adrs[m.group(1)]
        wm = WAVE_ADR.match(fn)
        if wm:
            a["waves"].add(wm.group(2) + (wm.group(3) or ""))
        a["files"].append("docs/adr/" + fn)
        a["items"].update(items)
        a["text"].append(text)
        if m.group(1) not in RETRO_EXCLUDED:
            a["retro"].extend(retro_units(text))
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


def wave_table(adrs):
    """Tag -> sorted ADR numbers, from the ADR file names."""
    tab = {}
    for n, a in adrs.items():
        for tag in a["waves"]:
            tab.setdefault(tag, set()).add(n)
    return {k: sorted(v) for k, v in tab.items()}


def wave_resolve(tag, table):
    """('map', [adr]) for exactly one ADR of the tag, ('ambiguous', candidates) for several ADRs of the tag or only a proper
    prefix match (`48g` against `48g1`..), ('none', [])."""
    if tag in table:
        return ("map", table[tag]) if len(table[tag]) == 1 else ("ambiguous", table[tag])
    cands = sorted({n for t, ns in table.items() if t.startswith(tag) and t != tag for n in ns})
    return ("ambiguous", cands) if cands else ("none", [])


def primary_adr(ev, item_adrs, adr_files, rule="p0679", e8=False):
    """(ADR, evidence kind) of a test. Rule p0679: the first tier of TIERS with an ADR; in the tier an ADR file named registration
    or -wave first, then the lowest number. Rule registered (ADR 0684): only the tiers of REGISTERED_TIERS; in E2 the lowest
    number outside LISTING_ADRS; none (None, None) when no such tier has an ADR. With `e8` (rule git, ADR 0720) the tiers are E8_TIERS and
    the primary of tier E8 is the first E8 entry."""
    registered = rule in ("registered", "git")
    for kind in E8_TIERS if registered and e8 else REGISTERED_TIERS if registered else TIERS:
        if kind == "E8":
            for e in ev:
                if e["kind"] == "E8":
                    return e["adr"], "E8"
            continue
        cands = set()
        for e in ev:
            if e["kind"] != kind:
                continue
            if "adr" in e:
                if not (registered and kind == "E2" and e["adr"] in LISTING_ADRS):
                    cands.add(e["adr"])
            elif kind in ("E1", "E4"):
                cands |= item_adrs.get(e["item"], set())
        if cands:
            if registered and kind == "E2":
                return min(cands), kind

            def key(n):
                reg = any("registration" in f or "-wave" in f for f in adr_files.get(n, []))
                return (0 if reg else 1, n)
            return min(cands, key=key), kind
    return None, None


def git_history(root):
    """(state, origin, calls) for rule G (ADR 0690). state is `full` (the workspace root is the top of a git repository that is not
    shallow), `shallow` (anything but `false` from --is-shallow-repository) or `none` (no repository at the root, git missing or
    failing). origin maps a path to (commit, subject) of the first commit that adds it, renames followed; empty unless `full`.
    At most 2 git processes: one rev-parse, one log over the whole history."""
    calls = 0
    try:
        calls += 1
        r = subprocess.run(["git", "rev-parse", "--show-toplevel", "--is-shallow-repository"], cwd=root, stdout=subprocess.PIPE,
                           stderr=subprocess.PIPE)
        if r.returncode != 0:
            return "none", {}, calls
        lines = r.stdout.decode("utf-8", "replace").split("\n")
        norm = lambda p: os.path.normcase(os.path.realpath(p))  # noqa: E731
        if norm(lines[0].strip()) != norm(root):
            return "none", {}, calls
        if (lines[1].strip() if len(lines) > 1 else "") != "false":
            return "shallow", {}, calls
        calls += 1
        r = subprocess.run(["git", "-c", "core.quotepath=off", "log", "--reverse", "-M", "--diff-filter=AR", "--name-status",
                            "--format=%x01%H%x09%s", "--relative"], cwd=root, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        if r.returncode != 0:
            return "none", {}, calls
    except OSError:
        return "none", {}, calls
    origin, cur = {}, None
    for ln in r.stdout.decode("utf-8", "replace").splitlines():
        if ln.startswith("\x01"):
            h, _, subj = ln[1:].partition("\t")
            cur = (h, subj)
        elif "\t" in ln and cur:
            parts = ln.split("\t")
            if parts[0] == "A" and len(parts) == 2:
                origin.setdefault(parts[1], cur)
            elif parts[0].startswith("R") and len(parts) == 3:
                origin.setdefault(parts[2], origin.get(parts[1], cur))
    return "full", origin, calls


def commit_adr(subject, adrs, reg):
    """The ADR a commit subject names: the first `ADR dddd` that exists, else the registration ADR of the first `F-ddd` that has one."""
    for m in ADR_IN_SUBJECT.finditer(subject):
        if m.group(1) in adrs:
            return m.group(1)
    for m in ITEM_IN_SUBJECT.finditer(subject):
        if item_id(m.group(1)) in reg:
            return reg[item_id(m.group(1))]
    return None


def e8_adrs(subject, adrs, reg):
    """Rule E8 (ADR 0720): [(ADR, via)] named by a commit subject: every `ADR dddd` that exists, in order, then the registration ADR of
    every `F-ddd` that has one, without repeats."""
    out = []
    for m in ADR_IN_SUBJECT.finditer(subject):
        if m.group(1) in adrs and m.group(1) not in [n for n, _ in out]:
            out.append((m.group(1), "subject"))
    for m in ITEM_IN_SUBJECT.finditer(subject):
        it = item_id(m.group(1))
        if it in reg and reg[it] not in [n for n, _ in out]:
            out.append((reg[it], it))
    return out


def registration_adrs(adrs):
    """Item -> its registration ADR: the ADR whose file name carries `-fddd-` (a file named `registration` first, then the lowest number)."""
    reg = {}
    for n, a in sorted(adrs.items()):
        isreg = any("registration" in f for f in a["files"])
        for it in a["items"]:
            if it not in reg or (isreg and not reg[it][0]):
                reg[it] = (isreg, n)
    return {it: v[1] for it, v in reg.items()}


def metadata(root, path=None):
    if path:
        return json.load(open(path, encoding="utf-8"))
    out = subprocess.run(["cargo", "metadata", "--no-deps", "--format-version", "1"], cwd=root, stdout=subprocess.PIPE,
                         check=True).stdout
    return json.loads(out)


def rel(root, p):
    return os.path.relpath(p, root).replace("\\", "/")


def build(root, meta, primary_rule="p0679", retro=None, e8=False):
    """`retro` (ADR 0701): the header path as E0 (H0) and the retro-registration units (H1 to H3). Default on for rule `git`, off
    otherwise; it has no effect on another rule. `e8` (ADR 0720): evidence of kind E8, the ADRs named by the commit that adds a test's
    file; default off here (the output of ADR 0690), on in the command line; it has no effect on another rule than `git`."""
    retro = (primary_rule == "git") if retro is None else (retro and primary_rule == "git")
    e8 = bool(e8) and primary_rule == "git"
    state, origin, gcalls = git_history(root) if primary_rule == "git" else (None, {}, 0)
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

    reg = registration_adrs(adrs) if primary_rule == "git" else {}
    e8stat = {"history": state, "tests": 0, "files": set(), "links": 0, "links_new": 0, "tests_new": 0}
    wtab = wave_table(adrs)
    adr_files = {n: a["files"] for n, a in adrs.items()}
    wave_map, wave_amb, wave_none, e7_common = {}, {}, set(), set()
    for t in tests:
        ev = []
        fn = t["name"].rsplit("::", 1)[-1]
        stem = os.path.splitext(os.path.basename(t["file"]))[0]
        for s in {stem, os.path.splitext(os.path.basename(binaries[t["binary"]][0]["file"]))[0]}:
            for it in items_in_stem(s):
                ev.append({"kind": "E1", "item": it, "from": s})
        h0 = header_adr(os.path.join(ws_root, t["file"]), adrs, by_path=retro)
        if h0:
            ev.append({"kind": "E0", "adr": h0})
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
        tags = sorted({g for s_ in {stem, t["binary"]} for g in waves_in_stem(s_)})
        t["wave"] = tags
        for g in tags:
            how, cands = wave_resolve(g, wtab)
            if how == "map":
                wave_map[g] = cands[0]
                ev.append({"kind": "E6", "adr": cands[0], "tag": g})
            elif how == "ambiguous":
                wave_amb[g] = cands
            else:
                wave_none.add(g)
        b_stem = t["binary"]
        if t["kind"] == "test" and "_" in b_stem and len(b_stem) >= E7_MIN_LEN and stem_count.get(b_stem, 0) == 1:
            hit = sorted(n for n, a in adrs.items() if n not in NOT_EVIDENCE and b_stem in a["cite"].words)
            if len(hit) > E7_MAX_ADRS:
                e7_common.add(b_stem)
            else:
                ev.extend({"kind": "E7", "adr": n, "binary": b_stem} for n in hit)
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
        if e8 and state == "full" and t["file"] in origin:
            h, subj = origin[t["file"]]
            named = e8_adrs(subj, adrs, reg)
            if named:
                new = [n for n, _ in named if n not in found]
                e8stat["tests"] += 1
                e8stat["files"].add(t["file"])
                e8stat["links"] += len(named)
                e8stat["links_new"] += len(new)
                e8stat["tests_new"] += 1 if new else 0
                ev.extend({"kind": "E8", "adr": n, "via": via, "commit": h[:10]} for n, via in named)
                found |= {n for n, _ in named}
        t["adrs"] = sorted(found)
        t["items"] = sorted(set(items) | {i for n in found for i in adr_items.get(n, ())})
        t["evidence"] = ev
        t["primary_adr"], t["primary_evidence"] = primary_adr(ev, item_adrs, adr_files, primary_rule, e8)

    gstat = None
    if primary_rule == "git":
        calls = gcalls
        gstat = {"history": state, "e2_tests": 0, "from_git": 0, "no_commit_rule": 0, "not_in_adrs": 0, "no_origin": 0,
                 "git_calls": calls}
        e2_null = []
        for t in tests:
            if e8 and t["primary_evidence"] == "E8":
                gstat["e2_tests"] += 1
                gstat["from_git"] += 1
                continue
            if t["primary_evidence"] != "E2":
                continue
            gstat["e2_tests"] += 1
            cand, why = None, "no_origin"
            if e8:  # tier E8 took every test whose commit names an ADR or an item: the others have no primary (ADR 0720)
                if state == "full" and t["file"] in origin:
                    why = "no_commit_rule"
            elif state == "full" and t["file"] in origin:
                cand = commit_adr(origin[t["file"]][1], adrs, reg)
                why = "no_commit_rule" if cand is None else ("not_in_adrs" if cand not in t["adrs"] else "from_git")
            if why == "from_git":
                t["primary_adr"] = cand
            else:
                t["primary_adr"], t["primary_evidence"] = None, None
                e2_null.append(t)
            gstat[why] += 1
        if retro:
            rstat = retro_primaries(adrs, e2_null, file_keys)

    by_adr, by_primary = {}, {}
    for t in tests:
        for n in t["adrs"]:
            by_adr.setdefault(n, []).append(f"{t['crate']}/{t['binary']}::{t['name']}")
        if t["primary_adr"]:
            by_primary.setdefault(t["primary_adr"], []).append(f"{t['crate']}/{t['binary']}::{t['name']}")
    idx = {
        "about": "ADR 0653 test-to-ADR index; tests keyed by crate, binary (times.tsv stem) and libtest name",
        "binaries": binaries,
        "crates": {c: {"dir": crate_dir[c], "deps": deps[c]} for c in sorted(names)},
        "tests": tests,
        "adrs": {n: {"files": a["files"], "items": sorted(adr_items.get(n, set()) | a["items"]),
                     "tests": sorted(by_adr.get(n, [])), "primary_tests": sorted(by_primary.get(n, []))}
                 for n, a in sorted(adrs.items())},
        "items": {it: sorted(ns) for it, ns in sorted(item_adrs.items())},
    }
    idx["coverage"] = coverage(idx)
    idx["coverage"]["wave"] = {
        "tagged_tests": sum(1 for t in tests if t["wave"]),
        "mapped_tags": dict(sorted(wave_map.items())),
        "ambiguous_tags": dict(sorted(wave_amb.items())),
        "no_wave_adr_tags": sorted(wave_none),
        "e7_too_common": sorted(e7_common),
    }
    pe = {}
    for t in tests:
        if t["primary_adr"]:
            pe[t["primary_evidence"]] = pe.get(t["primary_evidence"], 0) + 1
    idx["coverage"]["primary"] = {"with_primary": sum(pe.values()), "by_evidence": dict(sorted(pe.items()))}
    if gstat:
        idx["coverage"]["primary"]["git"] = gstat
        if e8:
            e8stat["files"] = len(e8stat["files"])
            e8stat["primary_tests"] = sum(1 for t in tests if t["primary_evidence"] == "E8")
            e8stat["primary_files"] = len({t["file"] for t in tests if t["primary_evidence"] == "E8"})
            idx["coverage"]["primary"]["e8"] = e8stat
        if retro:
            idx["coverage"]["primary"]["retro"] = rstat
    return idx


def retro_primaries(adrs, tests, file_keys):
    """Rule H1 to H3 of ADR 0701 for the E2-only tests that rule G left without a primary: the first ADR, by tier then by number, with a
    registering unit that cites the test's file. Sets `primary_adr` (evidence stays E2); returns the counts."""
    by_key = {}
    for n, a in adrs.items():
        for tier, keys in a["retro"]:
            for k in keys:
                by_key.setdefault(k, set()).add((tier, n))
    stat = {"h1": 0, "h2": 0, "h3": 0, "none": 0, "files": {}}
    found = {}
    for t in tests:
        if t["file"] not in found:
            cands = set()
            for k in file_keys(t):
                cands |= by_key.get(k, set())
            found[t["file"]] = min(cands) if cands else None
        best = found[t["file"]]
        if best is None or best[1] not in t["adrs"]:
            stat["none"] += 1
            continue
        t["primary_adr"], t["primary_evidence"] = best[1], "E2"
        stat["h%d" % best[0]] += 1
        stat["files"][t["file"]] = [best[0], best[1]]
    return stat


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
    """cargo commands that run the tests the index maps to one ADR, one command per binary.

    Ignored tests are left out, as the full gate and a command for a whole binary leave them out. An ignored test is a
    record (a known miss of rule 2, a retired test, a defective set-up kept beside its correction) that fails by design,
    or a heavy run its own registration runs; until 2026-11-20 they were run one by one with --include-ignored, which
    failed the push of ADR 0748 twice on records the merge had not touched."""
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
                if not t["ignored"]:
                    cmds.append(f"cargo test -p {crate} --release {sel} -- --exact {t['name']}")
    return cmds


def main(argv=None):
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", default=".")
    ap.add_argument("--metadata")
    ap.add_argument("--out")
    ap.add_argument("--report", action="store_true")
    ap.add_argument("--adr", nargs="*", default=[])
    ap.add_argument("--e8", action=argparse.BooleanOptionalAction, default=True,
                    help="evidence E8, the ADRs named by the commit that adds a test file (ADR 0720; default on, with --primary git)")
    ap.add_argument("--primary", choices=("git", "registered", "p0679"), default="git",
                    help="rule of the primary ADR: git (ADR 0690, default), registered (ADR 0684) or p0679 (ADR 0679)")
    a = ap.parse_args(argv)
    root = os.path.abspath(a.root)
    idx = build(root, metadata(root, a.metadata), a.primary, e8=a.e8)
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
        w = c["wave"]
        print(f"wave tags: {w['tagged_tests']} tagged tests, {len(w['mapped_tags'])} tags mapped, {len(w['ambiguous_tags'])} ambiguous "
              f"{w['ambiguous_tags']}, {len(w['no_wave_adr_tags'])} without a wave ADR; E7 too common: {len(w['e7_too_common'])} binaries")
        print(f"primary ADR: {c['primary']['with_primary']} tests {c['primary']['by_evidence']}")
        if "git" in c["primary"]:
            print(f"primary from git: {c['primary']['git']}")
        if "e8" in c["primary"]:
            print(f"evidence E8 (ADR 0720): {c['primary']['e8']}")
        if "retro" in c["primary"]:
            r = c["primary"]["retro"]
            print(f"primary from retro rule H (ADR 0701, H3 removed by ADR 0703): tests h1 {r['h1']}, h2 {r['h2']}, h3 {r['h3']}, none {r['none']}; "
                  f"{len(r['files'])} files")
        print(f"unmapped binaries ({len(c['unmapped_binaries'])}):")
        for u in c["unmapped_binaries"]:
            print("  " + u)
    for n in a.adr:
        for cmd in adr_commands(idx, n):
            print(cmd)
    return 0


if __name__ == "__main__":
    sys.exit(main())
