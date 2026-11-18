#!/usr/bin/env python3
"""The lead's checks between a local merge into main and the push (docs/test-process.md 7.4), and the ledger of what was pushed.

Run on the main checkout, with the merge of one branch at HEAD:
    python scripts/push_main.py                 dry run: print the plan (commands, what the budget leaves out), run nothing
    python scripts/push_main.py --run           run the plan; on success regenerate the generated files, append the ledger line and commit
    python scripts/push_main.py --run --push    the same, then `git push origin HEAD:main`
    python scripts/push_main.py --run --left-out-run   after the commit, start the left-out tests detached; a `leftout` line follows
    python scripts/push_main.py --gate-line --commit SHA --passed N --failed N --ignored N [--attribution gate_attribute.json]
    python scripts/push_main.py --since-verified

The plan, in order:
 1. always (never cut by the budget): cargo fmt and clippy of the touched packages; the tests the index (scripts/adr_test_index.py)
    maps to the merge's ADRs; the unit tests (--lib) of the touched crates; baseline_hashes; docs_site when docs/ is touched;
    `python scripts/process_report.py --check`.
 2. diff-selected, within --budget-minutes (default 30) of recorded run time: the integration-test binaries of the touched crates,
    then those of the crates that depend on them. Inside each group: binaries with a test that failed in a recent gate or left-out
    line of the ledger first, then by recorded seconds, shortest first. Recorded seconds come from --times, the TSV that
    scripts/gate_test.py --tsv writes (binary, test, outcome, seconds, MB), summed over the non-ignored tests of a binary; a binary
    with no record counts --default-seconds (60). Selection stops at the first binary that does not fit; the rest is left out,
    counted in the ledger line, and runs in CI on main. No test is switched off.
 3. mutation patches: every `*.mutation.patch` the merge adds or changes, and the one beside each test file of the merge's ADRs
    (crates/<crate>/tests/<test>.mutation.patch for <test>.rs). Its first line is `# expect-fail: <command>`. The patch is applied
    with git apply, the command must FAIL, and the patch is removed again whatever happens.
 4. `npm test` in apps/studio-web when the merge touches it; Playwright's skipped count goes in the ledger line.
On the first failing command the script stops, prints the line, writes nothing and leaves the merge unpushed.

The ledger, docs/process/ledger.tsv, is changed only on main. Columns: date, kind (merge, gate, leftout, ci), commit, branch,
items, adrs, result, minutes, left_out, left_out_minutes, detail. A gate line's detail is
`passed=N;failed=N;ignored=N;fails=binary::test=ADR/F-item,...`. A green gate line moves the local tag `verified`.
Python 3.9 standard library only.
"""
import argparse
import datetime
import json
import os
import re
import subprocess
import sys
import time

LEDGER = "docs/process/ledger.tsv"
COLUMNS = ["date", "kind", "commit", "branch", "items", "adrs", "result", "minutes", "left_out", "left_out_minutes", "detail"]
LAST_GREEN = "49fbf1a3"  # the last green full gate before the ledger began (2026-11-12)
EXPECT = re.compile(r"^#\s*expect-fail:\s*(\S.*?)\s*$")
# Committed after a merge, with whatever `process_report.py --list-generated` names (the ADR index and the pages with a generated block).
GENERATED = ["docs/roadmap-progress.svg", "docs/roadmap-categories.svg", LEDGER]


# ---------------------------------------------------------------- pure parts


def parse_subject(subject):
    """Branch, items and ADR numbers named by a merge subject."""
    m = re.search(r"Merge (?:remote-tracking )?branch '([^']+)'", subject)
    items = sorted({f"F-{int(n):03d}" for n in re.findall(r"\bF-(\d{1,4})\b", subject)})
    adrs = sorted({f"{int(n):04d}" for n in re.findall(r"\bADR[ -]?(\d{1,4})\b", subject)})
    return (m.group(1) if m else ""), items, adrs


def adrs_in_files(files):
    """ADR numbers and items of docs/adr files (not briefs) in a file list."""
    adrs, items = set(), set()
    for f in files:
        m = re.match(r"docs/adr/(\d{4})-([^/]+)\.md$", f)
        if m:
            adrs.add(m.group(1))
            items.update(f"F-{int(n):03d}" for n in re.findall(r"(?:^|-)f(\d{1,4})(?=-)", m.group(2)))
    return sorted(adrs), sorted(items)


def touched_crates(files, crates):
    """Crates (index `crates`: name -> {dir, deps}) owning a changed file, by the longest directory prefix."""
    out = set()
    for f in files:
        hits = [c for c, info in crates.items() if f.startswith(info["dir"].rstrip("/") + "/")]
        if hits:
            out.add(max(hits, key=lambda c: len(crates[c]["dir"])))
    return sorted(out)


def dependents(crates, touched):
    """Crates that depend, directly or not, on a touched crate (the touched ones excluded)."""
    def closure(c):
        seen, todo = set(), [c]
        while todo:
            for d in crates.get(todo.pop(), {}).get("deps", []):
                if d not in seen:
                    seen.add(d)
                    todo.append(d)
        return seen
    t = set(touched)
    return sorted(c for c in crates if c not in t and closure(c) & t)


def read_times(path):
    """(binary, test) -> seconds from a gate_test.py TSV."""
    out = {}
    if path and os.path.isfile(path):
        for ln in open(path, encoding="utf-8").read().splitlines()[1:]:
            f = ln.split("\t")
            if len(f) >= 4 and f[3]:
                try:
                    out[(f[0], f[1])] = float(f[3])
                except ValueError:
                    pass
    return out


def binary_units(idx, crates_wanted, times, default_s):
    """One unit per integration-test binary of the wanted crates: crate, target, binary, seconds (None if no record), cost."""
    per = {}
    for t in idx["tests"]:
        if t["kind"] == "test" and t["crate"] in crates_wanted and not t["ignored"]:
            per.setdefault((t["crate"], t["target"], t["binary"]), []).append(t["name"])
    units = []
    for (crate, target, binary), names in sorted(per.items()):
        rec = [times[(binary, n)] for n in names if (binary, n) in times]
        secs = sum(rec) if rec else None
        units.append({"crate": crate, "target": target, "binary": binary, "seconds": secs,
                      "cost": secs if secs is not None else float(default_s),
                      "cmd": f"cargo test -p {crate} --release --test {target}"})
    return units


def order_group(units, failed_binaries):
    """Recently failed first, then by cost ascending, then by name."""
    return sorted(units, key=lambda u: (u["binary"] not in failed_binaries, u["cost"], u["crate"], u["target"]))


def cut(groups, budget_s, skip_cmds=()):
    """Take units group by group until one does not fit; everything from there on is left out. Units whose command already
    runs in the always-group are dropped without cost."""
    taken, left, used, full = [], [], 0.0, False
    for g in groups:
        for u in g:
            if u["cmd"] in skip_cmds:
                continue
            if not full and used + u["cost"] <= budget_s:
                taken.append(u)
                used += u["cost"]
            else:
                full = True
                left.append(u)
    return taken, left, used


def read_ledger(path):
    if not os.path.isfile(path):
        return []
    rows = []
    for ln in open(path, encoding="utf-8").read().splitlines()[1:]:
        if ln.strip():
            f = ln.split("\t")
            rows.append(dict(zip(COLUMNS, f + [""] * (len(COLUMNS) - len(f)))))
    return rows


def ledger_line(**kw):
    unknown = set(kw) - set(COLUMNS)
    if unknown:
        raise ValueError(f"not a ledger column: {sorted(unknown)}")
    return "\t".join(re.sub(r"[\t\r\n]+", " ", str(kw.get(c, ""))) for c in COLUMNS)


def append_ledger(path, line):
    new = not os.path.isfile(path)
    os.makedirs(os.path.dirname(os.path.abspath(path)), exist_ok=True)
    with open(path, "a", encoding="utf-8", newline="\n") as f:
        if new:
            f.write("\t".join(COLUMNS) + "\n")
        f.write(line + "\n")


def recent_failed_binaries(rows, last=5):
    """Binaries named in the `fails=` field of the newest gate and left-out lines."""
    out = set()
    for r in [r for r in rows if r["kind"] in ("gate", "leftout")][-last:]:
        m = re.search(r"fails=([^;]*)", r["detail"])
        if m:
            out.update(x.split("::", 1)[0] for x in m.group(1).split(",") if "::" in x)
    return out


def newest_green(rows):
    for r in reversed(rows):
        if r["kind"] == "gate" and r["result"] == "green":
            return r["commit"]
    return LAST_GREEN


def expect_fail(patch_text):
    """The command of a mutation patch: its first line, `# expect-fail: <command>`. None if absent."""
    first = patch_text.splitlines()[0] if patch_text else ""
    m = EXPECT.match(first)
    return m.group(1) if m else None


def patch_paths(files, test_files, exists=os.path.isfile):
    """Mutation patches to verify: those in the merge's file list, and the one beside each registered test file."""
    out = [f for f in files if f.endswith(".mutation.patch")]
    for t in test_files:
        p = re.sub(r"\.(rs|ts|tsx)$", "", t) + ".mutation.patch"
        if p not in out and exists(p):
            out.append(p)
    return sorted(set(out))


def gate_detail(passed, failed, ignored, attribution=None, note=""):
    d = f"passed={passed};failed={failed};ignored={ignored}"
    if attribution:
        fails = []
        for f in attribution.get("failures", []):
            adr = f.get("primary_adr") or (f.get("adrs") or ["none"])[0]
            item = (f.get("items") or ["none"])[0]
            fails.append(f"{f['binary']}::{f['test']}={adr}/{item}")
        fails += [f"doc::{x['name']}=none/none" for x in attribution.get("doc_failures", [])]
        if fails:
            d += ";fails=" + ",".join(fails)
    return d + (";" + note if note else "")


def skipped_count(text):
    m = re.findall(r"(\d+) skipped", text)
    return int(m[-1]) if m else None


# ---------------------------------------------------------------- git and processes


def git(root, *args, check=True):
    p = subprocess.run(["git", *args], cwd=root, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, encoding="utf-8")
    if check and p.returncode:
        raise SystemExit(f"push_main: git {' '.join(args)}: {p.stderr.strip()}")
    return p.stdout


def merge_info(root, rev):
    parents = git(root, "rev-list", "--parents", "-n", "1", rev).split()
    if len(parents) != 3:
        raise SystemExit(f"push_main: {rev} is not a merge of one branch (parents: {len(parents) - 1})")
    sha, first, second = parents
    subject = git(root, "log", "-1", "--format=%s", sha).strip()
    files = [f for f in git(root, "diff", "--name-only", first, sha).splitlines() if f]
    added = [f for f in git(root, "diff", "--name-only", "--diff-filter=A", first, sha).splitlines() if f]
    branch, items, adrs = parse_subject(subject)
    a2, i2 = adrs_in_files(added)
    return {"sha": sha, "first": first, "second": second, "subject": subject, "files": files, "branch": branch,
            "items": sorted(set(items) | set(i2)), "adrs": sorted(set(adrs) | set(a2))}


def since_verified(root, rows, head="HEAD"):
    base = newest_green(rows)
    n = len(git(root, "rev-list", "--first-parent", "--merges", f"{base}..{head}").split())
    return base, n


def sh(cmd, cwd, capture=False):
    """Run one shell command; returns (exit code, minutes, output or '')."""
    t0 = time.time()
    if capture:
        p = subprocess.run(cmd, cwd=cwd, shell=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
                           encoding="utf-8", errors="replace")
        sys.stdout.write(p.stdout)
        return p.returncode, (time.time() - t0) / 60.0, p.stdout
    return subprocess.run(cmd, cwd=cwd, shell=True).returncode, (time.time() - t0) / 60.0, ""


def verify_patch(root, path, run=sh, apply=None):
    """Apply a mutation patch, require its command to fail, remove the patch. Returns (ok, message)."""
    with open(os.path.join(root, path), encoding="utf-8") as f:
        cmd = expect_fail(f.read())
    if not cmd:
        return False, f"{path}: first line is not `# expect-fail: <command>`"
    apply = apply or (lambda *a: subprocess.run(["git", "apply", *a, path], cwd=root).returncode)
    if apply():
        return False, f"{path}: git apply failed"
    try:
        code = run(cmd, root)[0]
    finally:
        back = apply("-R")
    if back:
        return False, f"{path}: could not remove the patch (git apply -R); the tree is dirty"
    if code == 0:
        return False, f"{path}: `{cmd}` passed with the mutation applied"
    return True, f"{path}: `{cmd}` failed as expected"


# ---------------------------------------------------------------- plan


def make_plan(root, info, idx, times, rows, budget_min, default_s):
    crates = idx["crates"]
    touched = touched_crates(info["files"], crates)
    always = []
    for c in touched:
        always.append(f"cargo fmt -p {c} -- --check")
    if touched:
        always.append("cargo clippy " + " ".join(f"-p {c}" for c in touched) + " --all-targets -- -D warnings")
    import adr_test_index
    for a in info["adrs"]:
        for cmd in adr_test_index.adr_commands(idx, a):
            if cmd not in always:
                always.append(cmd)
    has_lib = {t["crate"] for t in idx["tests"] if t["kind"] == "lib"}
    always += [f"cargo test -p {c} --release --lib" for c in touched if c in has_lib]
    always.append("cargo test -p rcfd-api --release --test baseline_hashes")
    if any(f.startswith("docs/") for f in info["files"]):
        always.append("cargo test -p rcfd-cli --release --test docs_site")
    always.append("python scripts/process_report.py --check")
    always = list(dict.fromkeys(always))
    failed = recent_failed_binaries(rows)
    g1 = order_group(binary_units(idx, set(touched), times, default_s), failed)
    g2 = order_group(binary_units(idx, set(dependents(crates, touched)), times, default_s), failed)
    taken, left, used = cut([g1, g2], budget_min * 60.0, set(always))
    adr_tests = sorted({t["file"] for t in idx["tests"] if set(t["adrs"]) & set(info["adrs"]) and t["kind"] == "test"})
    patches = patch_paths(info["files"], adr_tests, lambda p: os.path.isfile(os.path.join(root, p)))
    studio = any(f.startswith("apps/studio-web/") for f in info["files"])
    return {"touched": touched, "always": always, "selected": taken, "left": left, "selected_minutes": used / 60.0,
            "left_minutes": sum(u["cost"] for u in left) / 60.0, "patches": patches, "studio": studio}


def print_plan(info, plan, base, n):
    print(f"merge {info['sha'][:8]} of '{info['branch']}': items {','.join(info['items']) or '-'}; ADRs {','.join(info['adrs']) or '-'}; "
          f"{len(info['files'])} files; crates {','.join(plan['touched']) or '-'}")
    print(f"merges on main since the newest green full run ({base[:8]}): {n}")
    print("always:")
    for c in plan["always"]:
        print("  " + c)
    print(f"diff-selected ({len(plan['selected'])} binaries, {plan['selected_minutes']:.1f} recorded minutes):")
    for u in plan["selected"]:
        print(f"  {u['cmd']}    # {u['seconds']:.0f} s" if u["seconds"] is not None else f"  {u['cmd']}    # no record")
    print(f"left out for CI on main: {len(plan['left'])} binaries, {plan['left_minutes']:.1f} recorded minutes")
    for p in plan["patches"]:
        print(f"mutation patch: {p}")
    if plan["studio"]:
        print("studio: npm test in apps/studio-web")


def left_out_file(root, info, plan):
    d = os.path.join(root, "target", "push_main")
    os.makedirs(d, exist_ok=True)
    path = os.path.join(d, f"leftout-{info['sha'][:8]}.json")
    json.dump({"commit": info["sha"], "branch": info["branch"], "items": info["items"], "adrs": info["adrs"],
               "cmds": [u["cmd"] for u in plan["left"]]}, open(path, "w", encoding="utf-8"), indent=1)
    return path


# ---------------------------------------------------------------- modes


def today():
    return datetime.date.today().isoformat()


def do_run(root, a, info, plan):
    ledger = os.path.join(root, LEDGER)
    if git(root, "rev-parse", "HEAD").strip() != info["sha"]:
        raise SystemExit("push_main: --run needs the merge at HEAD")
    if git(root, "status", "--porcelain", "--untracked-files=no").strip():
        raise SystemExit("push_main: the working tree has uncommitted changes")
    cmds, minutes, studio_skipped = [], 0.0, None
    for cmd in plan["always"] + [u["cmd"] for u in plan["selected"]]:
        print(f"push_main: {cmd}", flush=True)
        code, m, _ = sh(cmd, root)
        minutes += m
        cmds.append(cmd)
        if code:
            print(f"push_main: FAILED (exit {code}): {cmd}\npush_main: nothing written; the merge is not pushed")
            return 1
    for p in plan["patches"]:
        t0 = time.time()
        ok, msg = verify_patch(root, p)
        minutes += (time.time() - t0) / 60.0
        print("push_main: mutation " + msg)
        if not ok:
            print("push_main: FAILED; nothing written; the merge is not pushed")
            return 1
    if plan["studio"]:
        code, m, out = sh("npm test", os.path.join(root, "apps", "studio-web"), capture=True)
        minutes += m
        cmds.append("npm test (apps/studio-web)")
        if code:
            print(f"push_main: FAILED (exit {code}): npm test in apps/studio-web\npush_main: nothing written; the merge is not pushed")
            return 1
        studio_skipped = skipped_count(out)
    for cmd in ("python scripts/process_report.py --write", "python scripts/roadmap_progress.py"):
        code = sh(cmd, root)[0]
        if code:
            print(f"push_main: FAILED (exit {code}): {cmd}\npush_main: nothing written to the ledger; the merge is not pushed")
            return 1
    detail = "exit=0;mutations=%d" % len(plan["patches"])
    if studio_skipped is not None:
        detail += f";studio_skipped={studio_skipped}"
    detail += ";cmds=" + " | ".join(cmds)
    append_ledger(ledger, ledger_line(date=today(), kind="merge", commit=info["sha"][:10], branch=info["branch"],
                                      items=",".join(info["items"]), adrs=",".join(info["adrs"]), result="pass",
                                      minutes=f"{minutes:.1f}", left_out=len(plan["left"]),
                                      left_out_minutes=f"{plan['left_minutes']:.1f}", detail=detail))
    code, _, listed = sh("python scripts/process_report.py --list-generated", root, capture=True)
    paths = [p for p in GENERATED + ([] if code else (listed or "").split()) if os.path.exists(os.path.join(root, p))]
    git(root, "add", "--", *paths)
    if git(root, "diff", "--cached", "--name-only").strip():
        what = ", ".join(info["items"]) or info["branch"] or info["sha"][:8]
        git(root, "commit", "-m", f"Ledger and generated pages after the merge of {what}")
    lo = left_out_file(root, info, plan) if plan["left"] else None
    if a.push:
        if sh("git push origin HEAD:main", root)[0]:
            print("push_main: FAILED: git push; the ledger commit is local")
            return 1
    if lo:
        cmd = f'python scripts/push_main.py --run-left-out "{lo}"'
        if a.left_out_run:
            flags = getattr(subprocess, "DETACHED_PROCESS", 0) | getattr(subprocess, "CREATE_NEW_PROCESS_GROUP", 0)
            log = open(lo[:-5] + ".log", "w", encoding="utf-8")
            subprocess.Popen([sys.executable, os.path.abspath(__file__), "--root", root, "--run-left-out", lo], cwd=root,
                             stdout=log, stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL, creationflags=flags,
                             start_new_session=(os.name != "nt"))
            print(f"push_main: left-out tests started in the background, log {lo[:-5]}.log")
        else:
            print(f"push_main: to run the {len(plan['left'])} left-out binaries for this merge: {cmd}")
    return 0


def do_left_out(root, path):
    """Run every left-out command (no stop on a failure) and append one `leftout` line to the working-tree ledger; the lead's
    next push commits it."""
    job = json.load(open(path, encoding="utf-8"))
    failed, minutes = [], 0.0
    for cmd in job["cmds"]:
        code, m, _ = sh(cmd, root)
        minutes += m
        if code:
            failed.append(cmd)
    detail = f"ran={len(job['cmds'])};failed={len(failed)}"
    if failed:
        bins = [re.search(r"--test (\S+)", c).group(1) + "::*=none/none" for c in failed if "--test " in c]
        detail += ";fails=" + ",".join(bins) + ";cmds=" + " | ".join(failed)
    append_ledger(os.path.join(root, LEDGER), ledger_line(
        date=today(), kind="leftout", commit=job["commit"][:10], branch=job["branch"], items=",".join(job["items"]),
        adrs=",".join(job["adrs"]), result="red" if failed else "green", minutes=f"{minutes:.1f}", detail=detail))
    return 1 if failed else 0


def do_gate_line(root, a):
    if not a.commit or a.passed is None or a.failed is None or a.ignored is None:
        raise SystemExit("push_main: --gate-line needs --commit, --passed, --failed and --ignored")
    att = json.load(open(a.attribution, encoding="utf-8")) if a.attribution else None
    green = a.failed == 0 and not (att and (att.get("build_failed") or att.get("doc_failures")))
    sha = git(root, "rev-parse", a.commit).strip()
    append_ledger(os.path.join(root, LEDGER), ledger_line(
        date=a.date or today(), kind="gate", commit=sha[:10], result="green" if green else "red",
        minutes=a.minutes or "", detail=gate_detail(a.passed, a.failed, a.ignored, att, a.note or "")))
    if green:
        git(root, "tag", "-f", "verified", sha)
        print(f"push_main: tag verified moved to {sha[:10]}")
    return 0


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--root", default=".")
    ap.add_argument("--merge", default="HEAD", help="the merge commit (dry run only; --run needs HEAD)")
    ap.add_argument("--run", action="store_true")
    ap.add_argument("--push", action="store_true")
    ap.add_argument("--left-out-run", action="store_true")
    ap.add_argument("--run-left-out", metavar="JSON", help="internal: run the left-out commands of one merge")
    ap.add_argument("--times", help="TSV of an earlier gate_test.py --tsv run (recorded seconds per test)")
    ap.add_argument("--budget-minutes", type=float, default=30.0)
    ap.add_argument("--default-seconds", type=float, default=60.0)
    ap.add_argument("--metadata", help="cargo metadata JSON (default: run cargo metadata --no-deps)")
    ap.add_argument("--since-verified", action="store_true")
    ap.add_argument("--gate-line", action="store_true")
    ap.add_argument("--commit")
    ap.add_argument("--date")
    ap.add_argument("--passed", type=int)
    ap.add_argument("--failed", type=int)
    ap.add_argument("--ignored", type=int)
    ap.add_argument("--minutes")
    ap.add_argument("--note")
    ap.add_argument("--attribution", help="JSON written by scripts/gate_attribute.py")
    a = ap.parse_args(argv)
    root = os.path.abspath(a.root)
    rows = read_ledger(os.path.join(root, LEDGER))
    if a.run_left_out:
        return do_left_out(root, a.run_left_out)
    if a.gate_line:
        return do_gate_line(root, a)
    if a.since_verified:
        base, n = since_verified(root, rows)
        print(f"{n} merges on main since the newest green full run ({base[:10]})")
        return 0
    if a.push and not a.run:
        raise SystemExit("push_main: --push needs --run")
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import adr_test_index
    info = merge_info(root, a.merge)
    idx = adr_test_index.build(root, adr_test_index.metadata(root, a.metadata), "git", e8=True)
    plan = make_plan(root, info, idx, read_times(a.times), rows, a.budget_minutes, a.default_seconds)
    base, n = since_verified(root, rows, info["sha"])
    print_plan(info, plan, base, n)
    if not a.run:
        print("dry run: nothing was run (use --run)")
        return 0
    return do_run(root, a, info, plan)


if __name__ == "__main__":
    sys.exit(main())
