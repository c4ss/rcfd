#!/usr/bin/env python3
"""Failure attribution of the full gate (ADR 0653 D5): for each failed or timed-out test of a gate run, whether it is flaky,
which ADRs and roadmap items it belongs to, and which first-parent commits (merges `Merge F-...`) since the last green gate
could have broken it.

Usage (from the workspace root, after scripts/gate_test.py):
    python scripts/gate_attribute.py attribute --tsv gate-out/times.tsv --log gate-out/test.log --index gate-out/adr_test_index.json
        [--feature-binary BINARY=FEATURE] [--base SHA] [--head SHA] [--rerun] [--rerun-timeout S] [--bisect] [--bisect-minutes M] [--probe-cmd CMD]
        [--max-commits N] [--work DIR] [--json gate-out/attribution.json] [--summary FILE]
    python scripts/gate_attribute.py issues --json gate-out/attribution.json [--dry-run] [--include-flaky]
attribute  --base: the SHA of the last green gate (empty or unknown: the last --max-commits first-parent commits).
           --rerun: run each failed test once more the way gate_test.py runs it; a pass marks it flaky.
           --bisect: when more than one candidate commit remains, find the first candidate at which the test fails (assumes
           it passes at base and fails at head); each probe builds only that test's target in a git worktree under --work and
           runs the one test, or runs --probe-cmd there (exit 0 = pass). Stops when --bisect-minutes are used up.
--tsv, --log  one or more files (ADR 0681: the gate's second step writes times_hooks.tsv and test_hooks.log); a missing file is skipped.
--feature-binary  repeatable; the bisect probe of that test binary builds with `--features FEATURE` (a feature-gated test file).
issues     one GitHub issue per non-flaky failure with `gh issue create`, skipped when an open issue has the same title.
"""
import argparse
import json
import os
import re
import shlex
import subprocess
import sys
import time

RUNNING = re.compile(r"^\s+Running (?:unittests )?(\S+) \((.+)\)\s*$")
DOC_FAILED = re.compile(r"^test (.+ - .+ \(line \d+\)) \.\.\. FAILED\s*$")
ITEM = re.compile(r"\bF-(\d{2,4})\b")
ADR = re.compile(r"\bADRs? (\d{4})\b")


def stem_of(exe):
    # Either separator: the log of a Windows gate is also read on Linux (the scripts job of ci.yml).
    return re.split(r"[\\/]", exe)[-1].rsplit("-", 1)[0].replace("-", "_")


def git(repo, *args, check=True):
    r = subprocess.run(["git", *args], cwd=repo, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if check and r.returncode != 0:
        raise RuntimeError(f"git {' '.join(args)}: {r.stderr.decode('utf-8', 'replace').strip()}")
    return r.stdout.decode("utf-8", "replace")


# ---------------------------------------------------------------- reading the gate output


def as_list(x):
    """A path, a list of paths or None as a list (ADR 0681: several tsv and log files)."""
    return [] if not x else [x] if isinstance(x, str) else list(x)


def read_failures(tsv, log):
    """Failed and timed-out tests from times.tsv; executables, a build failure and failed doc-tests from test.log.
    Each argument is one path or a list of paths; a build failure in several logs is reported joined by "; "."""
    fails = []
    for path in as_list(tsv):
        if not os.path.isfile(path):
            continue
        for ln in open(path, encoding="utf-8").read().splitlines()[1:]:
            f = ln.split("\t")
            if len(f) >= 3 and f[2] in ("FAILED", "TIMEOUT"):
                fails.append({"binary": f[0], "test": f[1], "outcome": f[2],
                              "seconds": float(f[3]) if len(f) > 3 and f[3] else None})
    exes, builds, docs = {}, [], []
    for path in as_list(log):
        if not os.path.isfile(path):
            continue
        for ln in open(path, encoding="utf-8", errors="replace").read().splitlines():
            m = RUNNING.match(ln)
            if m:
                exes.setdefault(stem_of(m.group(2)), []).append({"src": m.group(1), "exe": m.group(2)})
            elif ln.startswith("gate_test: build failed"):
                builds.append(ln.strip())
            else:
                d = DOC_FAILED.match(ln)
                if d:
                    docs.append(d.group(1))
    return fails, exes, "; ".join(builds) or None, docs


# ---------------------------------------------------------------- index lookup


def lookup(idx, binary, test):
    """The index entry of one test, and the binary record; the test name disambiguates a stem shared by two crates."""
    hits = [t for t in idx["tests"] if t["binary"] == binary and t["name"] == test]
    bins = idx["binaries"].get(binary, [])
    if hits:
        t = hits[0]
        b = next((x for x in bins if x["crate"] == t["crate"] and x["target"] == t["target"]), bins[0] if bins else None)
        return t, b
    return None, (bins[0] if len(bins) == 1 else None)


def doc_crate(idx, doc):
    """Crate of a failed doc-test from its `path - item (line N)` name (the path is relative to the workspace root)."""
    path = doc.split(" - ", 1)[0].replace("\\", "/")
    hits = [c for c, info in idx["crates"].items() if path.startswith(info["dir"].rstrip("/") + "/")]
    return max(hits, key=lambda c: len(idx["crates"][c]["dir"])) if hits else None


# ---------------------------------------------------------------- candidates


def dep_closure(idx, crate):
    seen, todo = set(), [crate]
    while todo:
        c = todo.pop()
        for d in idx["crates"].get(c, {}).get("deps", []):
            if d not in seen:
                seen.add(d)
                todo.append(d)
    seen.discard(crate)
    return seen


def first_parent_range(repo, base, head, max_commits):
    """First-parent commits after base up to head, oldest first, and whether base was usable."""
    base_ok = False
    if base:
        r = subprocess.run(["git", "merge-base", "--is-ancestor", base, head], cwd=repo, stdout=subprocess.DEVNULL,
                           stderr=subprocess.DEVNULL)
        base_ok = r.returncode == 0
    if base_ok:
        out = git(repo, "rev-list", "--first-parent", "--reverse", f"{base}..{head}")
    else:
        out = git(repo, "rev-list", "--first-parent", f"--max-count={max_commits}", head)
        out = "\n".join(reversed(out.split()))
    return [s for s in out.split() if s], base_ok


def commit_info(repo, sha):
    subject = git(repo, "log", "-1", "--format=%s", sha).strip()
    parents = git(repo, "log", "-1", "--format=%P", sha).split()
    if parents:
        files = git(repo, "diff", "--name-only", parents[0], sha).split("\n")
    else:
        files = git(repo, "show", "--name-only", "--format=", sha).split("\n")
    return subject, [f.strip() for f in files if f.strip()], len(parents) > 1


def relation(files, crate_dirs, test_file, dep_dirs):
    """direct (the test's file or crate), dependency (a workspace crate it depends on, or the root manifest/lock), or None."""
    hit = []
    for f in files:
        if f == test_file or any(f.startswith(d + "/") for d in crate_dirs):
            return "direct", [f]
    for f in files:
        # a dependency's own tests, benches and examples are not compiled into the test's binary
        if f in ("Cargo.toml", "Cargo.lock") or any(f.startswith(d + "/") and not f.startswith(
                (d + "/tests/", d + "/benches/", d + "/examples/")) for d in dep_dirs):
            hit.append(f)
    return ("dependency", hit[:5]) if hit else (None, [])


def candidates(repo, idx, commits, crate, test_file, cache):
    """The commits of the range that touch the test's crate, its file or a dependency, oldest first."""
    if crate is None:
        crate_dirs, dep_dirs = [], []
    else:
        crate_dirs = [idx["crates"][crate]["dir"]]
        dep_dirs = [idx["crates"][d]["dir"] for d in dep_closure(idx, crate)]
    out = []
    for sha in commits:
        if sha not in cache:
            cache[sha] = commit_info(repo, sha)
        subject, files, merge = cache[sha]
        rel, why = relation(files, crate_dirs, test_file, dep_dirs) if crate else ("unknown crate", files[:5])
        if rel is None:
            continue
        items = sorted({f"F-{int(x):03d}" for x in ITEM.findall(subject)})
        adrs = set(ADR.findall(subject))
        for it in items:
            adrs.update(idx.get("items", {}).get(it, []))
        out.append({"sha": sha, "subject": subject, "merge": merge, "relation": rel, "files": why, "items": items,
                    "adrs": sorted(adrs)})
    return out


# ---------------------------------------------------------------- re-run and bisect


def run_test(exe, test, cwd, timeout):
    """One test the way gate_test.py runs it; returns ok / FAILED / TIMEOUT and seconds."""
    t = time.time()
    try:
        r = subprocess.run([exe, test, "--exact", "--test-threads=1"], cwd=cwd, env=dict(os.environ, CARGO_MANIFEST_DIR=cwd),
                           stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=timeout)
    except subprocess.TimeoutExpired:
        return "TIMEOUT", time.time() - t
    except OSError as e:
        return f"not run ({e})", 0.0
    out = r.stdout.decode("utf-8", "replace")
    ok = r.returncode == 0 and re.search(r"test result: ok\. 1 passed; 0 failed; 0 ignored", out) is not None
    return ("ok" if ok else "FAILED"), time.time() - t


def cargo_probe(entry, binary_rec, features=None):
    """Shell command that builds only the test's target and runs the one test (cwd: the worktree).
    features: the feature of a feature-gated test binary (ADR 0681), added as `--features`."""
    kind, target, crate = binary_rec["kind"], binary_rec["target"], binary_rec["crate"]
    sel = "--lib" if kind == "lib" else f"--bin {target}" if kind == "bin" else f"--test {target}"
    feat = f" --features {features}" if features else ""
    return f"cargo test -p {crate}{feat} --release {sel} -- --exact {shlex.quote(entry['test'])} --test-threads=1"


def bisect(cands, probe, minutes):
    """First candidate (oldest first) at which probe fails, assuming a pass before the first and a fail at the last.
    probe(sha) -> True for a pass. Returns a dict with the result, the probes made and whether the budget ran out."""
    lo, hi = -1, len(cands) - 1  # pass known at lo (base), fail known at hi (head side)
    probes, t0 = [], time.time()
    while hi - lo > 1:
        if (time.time() - t0) / 60.0 >= minutes:
            return {"first_failing": None, "range": [cands[lo + 1]["sha"], cands[hi]["sha"]], "probes": probes,
                    "budget_exhausted": True}
        mid = (lo + hi) // 2
        ok = probe(cands[mid]["sha"])
        probes.append({"sha": cands[mid]["sha"], "pass": ok})
        if ok:
            lo = mid
        else:
            hi = mid
    return {"first_failing": cands[hi]["sha"], "range": [cands[hi]["sha"], cands[hi]["sha"]], "probes": probes,
            "budget_exhausted": False}


def worktree_probe(repo, work, cmd, timeout):
    def probe(sha):
        d = os.path.join(work, "bisect-" + sha[:12])
        if not os.path.isdir(d):
            git(repo, "worktree", "add", "--detach", d, sha)
        try:
            r = subprocess.run(cmd, cwd=d, shell=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=timeout)
            return r.returncode == 0
        except subprocess.TimeoutExpired:
            return False
        finally:
            git(repo, "worktree", "remove", "--force", d, check=False)
    return probe


# ---------------------------------------------------------------- output


def adr_cell(f):
    """ADR cell of the table: the primary ADR (ADR 0679) in bold first when the test has one, then the others."""
    p = f.get("primary_adr")
    if not p:
        return ", ".join(f["adrs"]) or "unmapped"
    return ", ".join([f"**{p}**"] + [a for a in f["adrs"] if a != p])


def summary_md(res):
    head, base = res["head"], res["base"]
    lines = [f"### Failure attribution at {head[:12]} (ADR 0653)", ""]
    lines.append(f"Last green gate: {base[:12] if base else 'none'}" +
                 ("" if res["base_found"] else f" (not usable: the last {res['max_commits']} first-parent commits are used)") +
                 f"; {len(res['commits'])} first-parent commits in the range.")
    if res["build_failed"]:
        lines += ["", f"**Build failed**: `{res['build_failed']}`"]
    if not res["failures"] and not res["doc_failures"] and not res["build_failed"]:
        lines += ["", "No failed test."]
    if res["failures"]:
        lines += ["", "| test | outcome | re-run | ADRs | items | candidates (oldest first) |", "|---|---|---|---|---|---|"]
        for f in res["failures"]:
            rr = f["rerun"]["outcome"] + (" (flaky)" if f["flaky"] else "") if f["rerun"] else "not run"
            cands = "<br>".join(f"`{c['sha'][:10]}` {c['subject'][:60]} ({c['relation']})" for c in f["candidates"][:8])
            if len(f["candidates"]) > 8:
                cands += f"<br>... {len(f['candidates']) - 8} more"
            if f.get("bisect") and f["bisect"].get("first_failing"):
                cands = f"**bisect: `{f['bisect']['first_failing'][:10]}`**<br>" + cands
            lines.append(f"| `{f['crate'] or '?'}/{f['binary']}::{f['test']}` | {f['outcome']} | {rr} | "
                         f"{adr_cell(f)} | {', '.join(f['items']) or '-'} | {cands or 'none'} |")
    for d in res["doc_failures"]:
        lines.append(f"- doc-test failed: `{d['name']}` (crate {d['crate'] or '?'}, {len(d['candidates'])} candidates)")
    return "\n".join(lines) + "\n"


def attribute(a):
    idx = json.load(open(a.index, encoding="utf-8"))
    repo = os.path.abspath(a.repo)
    head = git(repo, "rev-parse", a.head).strip()
    base = (a.base or "").strip()
    commits, base_ok = first_parent_range(repo, base, head, a.max_commits)
    fails, exes, build_failed, docs = read_failures(a.tsv, a.log)
    features = dict(x.split("=", 1) for x in a.feature_binary)
    cache, res = {}, {"head": head, "base": base, "base_found": base_ok, "max_commits": a.max_commits, "commits": commits,
                      "build_failed": build_failed, "failures": [], "doc_failures": []}
    t_start = time.time()
    for f in fails:
        entry, b = lookup(idx, f["binary"], f["test"])
        crate = entry["crate"] if entry else b["crate"] if b else None
        test_file = entry["file"] if entry else b["file"] if b else None
        rec = {**f, "crate": crate, "file": test_file, "adrs": entry["adrs"] if entry else [],
               "primary_adr": entry.get("primary_adr") if entry else None,
               "items": entry["items"] if entry else [], "evidence": entry["evidence"] if entry else [],
               "in_index": entry is not None, "rerun": None, "flaky": False, "bisect": None}
        if a.rerun:
            ex = exes.get(f["binary"], [])
            if ex and crate:
                cwd = os.path.join(repo, idx["crates"][crate]["dir"])
                outcome, secs = run_test(ex[0]["exe"], f["test"], cwd, a.rerun_timeout)
                rec["rerun"] = {"outcome": outcome, "seconds": round(secs, 2)}
                rec["flaky"] = outcome == "ok"
            else:
                rec["rerun"] = {"outcome": "not run (no executable or crate)", "seconds": 0.0}
        rec["candidates"] = candidates(repo, idx, commits, crate, test_file, cache)
        if a.bisect and not rec["flaky"] and len(rec["candidates"]) > 1 and (crate or a.probe_cmd):
            left = a.bisect_minutes - (time.time() - t_start) / 60.0
            if left <= 0:
                rec["bisect"] = {"skipped": "budget used up"}
            else:
                cmd = a.probe_cmd or cargo_probe(rec, b, features.get(f["binary"]))
                os.makedirs(a.work, exist_ok=True)
                rec["bisect"] = bisect(rec["candidates"], worktree_probe(repo, a.work, cmd, a.probe_timeout), left)
                rec["bisect"]["command"] = cmd
        res["failures"].append(rec)
    for d in docs:
        crate = doc_crate(idx, d)
        res["doc_failures"].append({"name": d, "crate": crate, "candidates": candidates(repo, idx, commits, crate, None, cache)})
    os.makedirs(os.path.dirname(os.path.abspath(a.json)), exist_ok=True)
    with open(a.json, "w", encoding="utf-8") as fh:
        json.dump(res, fh, indent=1)
    md = summary_md(res)
    if a.summary:
        with open(a.summary, "a", encoding="utf-8") as fh:
            fh.write(md)
    else:
        sys.stdout.write(md)
    return 0


def issue_title(f):
    return f"Gate failure: {f['crate'] or '?'}/{f['binary']}::{f['test']}"


def issues(a, run=subprocess.run):
    res = json.load(open(a.json, encoding="utf-8"))
    made = []
    for f in res["failures"]:
        if f["flaky"] and not a.include_flaky:
            continue
        title = issue_title(f)
        body = [f"The full gate at {res['head']} reports `{f['outcome']}` for `{f['binary']}::{f['test']}` "
                f"(crate {f['crate'] or '?'}, file `{f['file'] or '?'}`).", "",
                f"- re-run: {f['rerun']['outcome'] if f['rerun'] else 'not run'}",
                f"- ADRs: {', '.join(f['adrs']) or 'none in the index'}; items: {', '.join(f['items']) or 'none'}",
                f"- last green gate: {res['base'] or 'none'}", "- candidate commits (oldest first):"]
        body += [f"  - {c['sha'][:10]} {c['subject']} ({c['relation']})" for c in f["candidates"]] or ["  - none"]
        if f.get("bisect") and f["bisect"].get("first_failing"):
            body.append(f"- bisect: first failing {f['bisect']['first_failing']}")
        body.append("\nWritten by scripts/gate_attribute.py (ADR 0653).")
        if a.dry_run:
            print(f"would open: {title}")
            made.append(title)
            continue
        q = run(["gh", "issue", "list", "--state", "open", "--search", f"\"{title}\" in:title", "--json", "title"],
                stdout=subprocess.PIPE, check=True)
        if any(i.get("title") == title for i in json.loads(q.stdout.decode("utf-8") or "[]")):
            print(f"open issue exists: {title}")
            continue
        run(["gh", "issue", "create", "--title", title, "--body", "\n".join(body)], check=True)
        made.append(title)
    return made


def main(argv=None):
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    p = sub.add_parser("attribute")
    p.add_argument("--tsv", required=True, nargs="+")
    p.add_argument("--log", required=True, nargs="+")
    p.add_argument("--feature-binary", action="append", default=[], metavar="BINARY=FEATURE")
    p.add_argument("--index", required=True)
    p.add_argument("--repo", default=".")
    p.add_argument("--base", default=os.environ.get("GATE_LAST_GREEN", ""))
    p.add_argument("--head", default="HEAD")
    p.add_argument("--max-commits", type=int, default=30)
    p.add_argument("--rerun", action="store_true")
    p.add_argument("--rerun-timeout", type=float, default=3600.0)
    p.add_argument("--bisect", action="store_true")
    p.add_argument("--bisect-minutes", type=float, default=30.0)
    p.add_argument("--probe-cmd")
    p.add_argument("--probe-timeout", type=float, default=3600.0)
    p.add_argument("--work", default=os.path.join("gate-out", "bisect"))
    p.add_argument("--json", default=os.path.join("gate-out", "attribution.json"))
    p.add_argument("--summary")
    q = sub.add_parser("issues")
    q.add_argument("--json", default=os.path.join("gate-out", "attribution.json"))
    q.add_argument("--dry-run", action="store_true")
    q.add_argument("--include-flaky", action="store_true")
    a = ap.parse_args(argv)
    if a.cmd == "attribute":
        return attribute(a)
    issues(a)
    return 0


if __name__ == "__main__":
    sys.exit(main())
