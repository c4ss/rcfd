#!/usr/bin/env python3
"""Read-only scanner for tests that cannot fail (ADR 0129 D4). Python 3.9 standard library only; never edits a file.

Usage (from the workspace root): python scripts/limit_audit.py [root, default .]
Scans crates/**/tests/*.rs and crates/**/src/**/*tests*.rs and prints one line per hit: `file:line: (class) line text`.
Classes:
  (i)   an assertion whose operands are only literals and constants (ALL_CAPS or a local set from literals), or a sleep/elapsed value
        compared in a test that sleeps
  (ii)  a #[test] with no assert*!, panic!, unreachable!, unwrap/expect, no `?` in a test returning a value, and no should_panic
  (iii) a test whose name has the word control(s) or negative with no assertion that expects a failure (no !, <, >, Err, None, false, fail, reject,
        assert_ne!, should_panic)
  (iv)  an early `return` before the first assertion (a silent skip); `[skip message]` is appended when the three lines before it
        mention skip, adapter or unavailable
  (v)   assert!(true), or assert_eq!/assert_ne!/assert!(a == a) of a value with itself
It is a heuristic: every hit needs reading (docs/test-process.md section 6 has the triage)."""
import glob
import os
import re
import sys

ASSERT = re.compile(r"\b(assert|assert_eq|assert_ne|debug_assert|debug_assert_eq|debug_assert_ne|assert_matches)!\s*\(")
CHECKS = re.compile(r"\b(assert\w*|panic|unreachable)!|\.(unwrap|expect|unwrap_err|expect_err)\(")
PURE = {"Duration", "from_millis", "from_secs", "from_micros", "from_nanos", "from_secs_f64", "f64", "f32", "u8", "u16", "u32",
        "u64", "usize", "i8", "i16", "i32", "i64", "isize", "std", "core", "time", "consts", "as", "abs", "sqrt", "powi", "powf",
        "true", "false", "mul", "div", "exp", "ln", "PI", "E"}
SKIP_MSG = re.compile(r"skip|adapter|unavailable|not available", re.I)


def mask(s):
    """Same length as s; comments and the contents of string and char literals replaced by spaces (newlines kept)."""
    out, i, n = list(s), 0, len(s)

    def blank(a, b):
        for k in range(a, b):
            if out[k] != "\n":
                out[k] = " "

    while i < n:
        c = s[i]
        if s.startswith("//", i):
            j = s.find("\n", i)
            j = n if j < 0 else j
            blank(i, j)
            i = j
        elif s.startswith("/*", i):
            depth, j = 1, i + 2
            while j < n and depth:
                if s.startswith("/*", j):
                    depth, j = depth + 1, j + 2
                elif s.startswith("*/", j):
                    depth, j = depth - 1, j + 2
                else:
                    j += 1
            blank(i, j)
            i = j
        elif c == "r" and re.match(r'r#*"', s[i:]) and (i == 0 or not (s[i - 1].isalnum() or s[i - 1] == "_") or s[i - 1] == "b"):
            h = re.match(r'r(#*)"', s[i:]).group(1)
            start = i + 2 + len(h)
            j = s.find('"' + h, start)
            j = n if j < 0 else j
            blank(start, j)
            i = j + 1 + len(h)
        elif c == '"':
            j = i + 1
            while j < n and s[j] != '"':
                j += 2 if s[j] == "\\" else 1
            blank(i + 1, j)
            i = j + 1
        elif c == "'":
            if i + 1 < n and s[i + 1] == "\\":
                j = s.find("'", i + 2)
                blank(i + 1, j)
                i = j + 1
            elif i + 2 < n and s[i + 2] == "'":
                blank(i + 1, i + 2)
                i += 3
            else:
                i += 1  # lifetime or label
        else:
            i += 1
    return "".join(out)


def close(m, i, op="(", cl=")"):
    """Index just past the bracket matching m[i] == op."""
    d = 0
    for j in range(i, len(m)):
        d += (m[j] == op) - (m[j] == cl)
        if d == 0:
            return j + 1
    return len(m)


def split_args(text):
    """Top-level comma split."""
    parts, d, cur = [], 0, ""
    for ch in text:
        if ch in "([{":
            d += 1
        elif ch in ")]}":
            d -= 1
        if ch == "," and d == 0:
            parts.append(cur)
            cur = ""
        else:
            cur += ch
    return parts + [cur]


def idents(expr):
    return re.findall(r"\b[A-Za-z_][A-Za-z0-9_]*\b", re.sub(r"\b\d[\d_]*(\.\d[\d_]*)?([eE][+-]?\d+)?(_?[a-z]\d+)?\b", " ", expr))


def literal_only(expr, consts):
    ids = idents(expr)
    return all(x in PURE or x in consts or re.fullmatch(r"[A-Z][A-Z0-9_]*", x) for x in ids)


def tests_in(src):
    """(name, attrs text, signature text, body start, body end) of every #[test] fn, positions in the masked text."""
    m = mask(src)
    for t in re.finditer(r"#\[test\]", m):
        f = re.compile(r"\bfn\s+(\w+)").search(m, t.end())
        if not f:
            continue
        a = m.rfind("\n\n", 0, t.start())
        attrs = m[a if a >= 0 else 0: f.start()]
        b = m.find("{", f.end())
        yield f.group(1), attrs, m[f.end(): b], b, close(m, b, "{", "}"), m


def checking_fns(srcs):
    """Names of the functions and macros (name + "!") in srcs that check something, directly or through another such one."""
    called = {}
    for src in srcs:
        m = mask(src)
        for f in re.finditer(r"\b(?:fn\s+(\w+)|macro_rules!\s*(\w+))[^{;]*\{", m):
            b = f.end() - 1
            body = m[b: close(m, b, "{", "}")]
            name = f.group(1) or f.group(2) + "!"
            c = called.setdefault(name, set())
            c |= {x + "!" if bang else x for x, bang in re.findall(r"\b(\w+)\s*(?:::<[^>]*>)?\s*(!)?\s*\(", body)}
            if CHECKS.search(body):
                c.add("assert!")
    good = {"assert!"}
    while True:
        more = {n for n, c in called.items() if n not in good and c & good}
        if not more:
            return good
        good |= more


def calls_any(body, helpers):
    return any((x + "!" if bang else x) in helpers for x, bang in re.findall(r"\b(\w+)\s*(?:::<[^>]*>)?\s*(!)?\s*\(", body))


def scan(path, src, helpers=frozenset()):
    lines = src.splitlines()
    hits = []

    def hit(pos, cls, m, extra=""):
        ln = m.count("\n", 0, pos)
        hits.append((path, ln + 1, cls, lines[ln].strip() + extra))

    for name, attrs, sig, b, e, m in tests_in(src):
        body = m[b:e]
        asserts = []
        for a in ASSERT.finditer(body):
            p = a.end() - 1
            asserts.append((b + a.start(), a.group(1), split_args(body[p + 1: close(body, p) - 1])))
        should_panic = "should_panic" in attrs
        # (ii)
        if not CHECKS.search(body) and not should_panic and not ("->" in sig and "?" in body) and not calls_any(body, helpers):
            hit(b, "ii", m, f"  [fn {name}]")
        # locals set only from literals; locals holding an elapsed time
        consts, elapsed = set(), set()
        changed = set(re.findall(r"(?<!let )(?<!mut )\b(\w+)\s*[-+*/]?=(?![=>])", body)) | set(re.findall(r"&mut\s+(\w+)", body))
        for d in re.finditer(r"\blet\s+(mut\s+)?(\w+)\s*(?::[^=;]+)?=([^;]+);", body):
            if not d.group(1) and literal_only(d.group(3), consts):
                consts.add(d.group(2))
            if "elapsed" in d.group(3):
                elapsed.add(d.group(2))
        consts -= changed
        sleeps = "sleep(" in body
        for pos, kind, args in asserts:
            ops = args[:1] if kind in ("assert", "debug_assert") else args[:2]
            joined = " ".join(ops)
            # (v)
            if (kind in ("assert", "debug_assert") and joined.strip() == "true") or (
                    len(ops) == 2 and ops[0].strip() == ops[1].strip()):
                hit(pos, "v", m)
                continue
            same = re.fullmatch(r"\s*(.+?)\s*==\s*(.+?)\s*", joined) if kind == "assert" else None
            if same and same.group(1) == same.group(2):
                hit(pos, "v", m)
                continue
            # (i)
            if idents(joined) and literal_only(joined, consts):
                hit(pos, "i", m)
            elif sleeps and ("elapsed" in joined or set(idents(joined)) & elapsed):
                hit(pos, "i", m, "  [sleep/elapsed]")
        # (iii)
        if re.search(r"(^|_)(negative|controls?)(_|$)", name) and not should_panic:
            exp = re.compile(r"!|<|>|Err|err|None|none|false|fail|reject")
            if not any(kind.endswith("_ne") or exp.search(" ".join(args)) for _, kind, args in asserts):
                hit(b, "iii", m, f"  [fn {name}]")
        # (iv)
        first = asserts[0][0] if asserts else e
        for r in re.finditer(r"\breturn\b", m[b:first]):
            pos = b + r.start()
            ln = m.count("\n", 0, pos)
            ctx = "\n".join(lines[max(0, ln - 3): ln + 1])
            hit(pos, "iv", m, "  [skip message]" if SKIP_MSG.search(ctx) else "")
    return hits


def files(root):
    fs = glob.glob(os.path.join(root, "crates", "**", "tests", "*.rs"), recursive=True)
    fs += glob.glob(os.path.join(root, "crates", "**", "src", "**", "*tests*.rs"), recursive=True)
    return sorted(set(os.path.normpath(f) for f in fs))


def main():
    root = sys.argv[1] if len(sys.argv) > 1 else "."
    hits, srcs = [], {}
    for f in files(root):
        with open(f, encoding="utf-8") as fh:
            srcs[f] = fh.read()
    for f, src in srcs.items():
        # helpers: the file's own fns and macros, and those of the modules in subfolders of its tests folder (tests/common/...)
        near = [src]
        d = os.path.dirname(f)
        if os.path.basename(d) == "tests":
            for g in glob.glob(os.path.join(d, "*", "**", "*.rs"), recursive=True):
                with open(g, encoding="utf-8") as fh:
                    near.append(fh.read())
        hits += scan(os.path.relpath(f, root).replace(os.sep, "/"), src, checking_fns(near))
    for path, ln, cls, text in sorted(hits):
        print(f"{path}:{ln}: ({cls}) {text}")
    counts = {c: sum(h[2] == c for h in hits) for c in ("i", "ii", "iii", "iv", "v")}
    print(f"limit_audit: {len(hits)} hits " + " ".join(f"({c}) {n}" for c, n in counts.items()), file=sys.stderr)


if __name__ == "__main__":
    main()
