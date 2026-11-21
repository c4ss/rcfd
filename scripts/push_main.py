#!/usr/bin/env python3
"""The lead's checks between a local merge into main and the push (docs/test-process.md 7.4), and the ledger of what was pushed.

Run on the main checkout, with the merge of one branch at HEAD:
    python scripts/push_main.py                 dry run: print the plan (commands, what the budget leaves out), run nothing
    python scripts/push_main.py --run           run the plan; on success regenerate the generated files, append the ledger line and commit
    python scripts/push_main.py --run --push    the same, then `git push origin HEAD:main`
    python scripts/push_main.py --run --left-out-run   after the commit, start the left-out tests detached; a `leftout` line follows
    python scripts/push_main.py --run --base REV    several merges at once: the diff REV..HEAD is one joint diff (an octopus merge needs no --base)
    python scripts/push_main.py --run --fresh       do not resume: repeat the commands that passed on this merge commit in a stopped run
    python scripts/push_main.py --run [--reported TIME] [--review-minutes N] [--proposed STATUS] [--written STATUS]
                                the process measures of this merge, as the lead states them (ADR 0754; below)
    python scripts/push_main.py --gate-line --commit SHA --passed N --failed N --ignored N [--skipped N] [--attribution gate_attribute.json]
                                [--gate-times PATH]   PATH (a times TSV or a folder of them) becomes the default of --times
                                [--escaped COMMIT=BINARY::TEST ...]   the commit that brought a failed test of a red gate in
    python scripts/push_main.py --mutants [--merge REV] [--mutants-timeout S] [--run]   generated mutants on the lines the merge changed
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
Before the plan runs, the generated files are regenerated (process_report.py --write, roadmap_progress.py), so that the process
check and docs_site see the pages the push will carry (ADR 0752, finding 9 of F-851).
A command that fails is run RETRIES (2) further times (ADR 0752). If it then passes, the tests that failed are flaky: they are
printed and named in the ledger line (`flaky=binary::test,...`) and the run goes on. A command that fails every time stops the
script: it prints the line, writes nothing to the ledger and leaves the merge unpushed. Nothing is left out of a further run.
A stopped run is resumed by the same command line: what passed on the same merge commit is not repeated (the record is
target/push_main/run-<commit>.json; --fresh repeats everything; the ledger line then has `resumed=N`). The tree must be clean,
except for lines appended to the ledger (a left-out line, which the run commits) and the generated files.
The script writes no roadmap status: the lead writes it on main.
When the merge changes scripts/, the plan begins with every scripts/test_*.py. An ADR of the merge that has registered tests and
no committed mutation patch is named in the output. The budget counts BUILD_SECONDS (1.8; --build-seconds) per selected binary
beside its recorded seconds. Without --times the recorded seconds come from the newest gate line of the ledger that names a
times path that exists (`times=PATH` in its detail, written by --gate-times).
The item field of a merge line comes from the brief of the branch (its `Items:` line) or, without a brief, from the head of the
branch name (`f855-edit-lock-f2-lockbusy` is F-855). Several merges checked as one joint diff (--base, or an octopus merge) give
one line: the branches joined by `,`, the items and ADRs of all, and `joint=<the other merge commits>` in the detail.

The ledger, docs/process/ledger.tsv, is changed only on main. Columns: date, kind (merge, gate, leftout, mutants, ci), commit,
branch, items, adrs, result, minutes, left_out, left_out_minutes, detail. A gate line's detail is
`passed=N;failed=N;ignored=N[;skipped=N];fails=binary::test=ADR/F-item,...[;times=PATH][;escaped=...]`; `skipped` is the number of passed tests that
reported a skip (the closing line of `scripts/gate_test.py --skip-pattern`). A green gate line moves the local tag `verified`.

Process measures (ADR 0754, F-863). Four options of --run put entries into the detail of the merge line, after `joint=` and
before `cmds=`; an option that is not given writes no entry, and a run without them writes the line it wrote before.
--reported TIME (ISO, `2026-11-21T14:05`, with an optional offset; without one it is local time) writes `reported=TIME` and
`queue_minutes=N`, the minutes from that time to the committer time of the merge commit. --review-minutes N writes
`review_minutes=N`, the lead's minutes on the agent's report. --proposed STATUS and --written STATUS write the status the agent
proposed and the status the lead writes: `done`, `partial` or `todo`, or for several items `F-863=done,F-864=partial`. A value
that cannot be read stops the run before any check. With --gate-line on a red gate, --escaped COMMIT=BINARY::TEST (several
times for several tests) writes `escaped=<commit>=<binary::test>,...` after `times=`: the commit that brought the failure in;
`--escaped none` says that the lead looked and no commit did. scripts/process_report.py --weekly reads the entries.

--mutants (docs/process/documentation-process-review.md, "Mutations the author did not write"): the diff of the merge against
its first parent, Rust files under crates/ only, is written to a temporary file and `cargo mutants --in-diff` runs on it for the
touched packages. Without --run the command is printed. With --run a ledger line of kind `mutants` follows, with
`caught=N;missed=N;timeout=N;unviable=N` and the first missed mutants in the detail; result is green when none was missed.
It reports and blocks nothing. NOT RUN HERE: cargo-mutants was not installed where this was written, its run time on this
workspace is unmeasured, and the layout of `mutants.out/outcomes.json` (counts at the top level, one `summary` per outcome,
`missed.txt` beside it) is taken from the tool's documentation as remembered, not from a run. cargo-mutants is a development
tool (`cargo install cargo-mutants`), not a dependency of any crate.
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
RETRIES = 2  # further runs of a command that failed (ADR 0752: a test that fails 15 % of its runs then stops one push in 290)
BUILD_SECONDS = 1.8  # build seconds counted per selected test binary (ADR 0752: 1592.1 s over 891 commands of the lead's runs)
BRIEFS = "docs/adr/briefs"
REGENERATE = ("python scripts/process_report.py --write", "python scripts/roadmap_progress.py")
# the process measures of ADR 0754 (F-863)
STATUSES = ("done", "partial", "todo")
STATUS_LIST = re.compile(r"F-\d{1,4}=(?:done|partial|todo)(?:,F-\d{1,4}=(?:done|partial|todo))*")
REPORT_TIME = re.compile(r"\d{4}-\d\d-\d\d[T ]\d\d:\d\d")
ESCAPED = re.compile(r"([^=;,\s]+)=([^=;,\s:]+::[^=;,\s]+)")
MEASURE_OPTIONS = (("reported", "--reported"), ("review_minutes", "--review-minutes"), ("proposed", "--proposed"),
                   ("written", "--written"))


# ---------------------------------------------------------------- pure parts


def parse_subject(subject):
    """Branch, items and ADR numbers named by a merge subject."""
    m = re.search(r"Merge (?:remote-tracking )?branch '([^']+)'", subject)
    items = sorted({f"F-{int(n):03d}" for n in re.findall(r"\bF-(\d{1,4})\b", subject)})
    adrs = sorted({f"{int(n):04d}" for n in re.findall(r"\bADR[ -]?(\d{1,4})\b", subject)})
    return (m.group(1) if m else ""), items, adrs


def subject_branches(subject):
    """Every branch a merge subject names: `Merge branch 'a'`, `Merge branches 'a', 'b' and 'c'`."""
    m = re.search(r"Merge (?:remote-tracking )?branch(?:es)? ", subject)
    out, rest = [], subject[m.end():] if m else ""
    while True:
        q = re.match(r"'([^']+)'", rest)
        if not q:
            return out
        out.append(q.group(1))
        sep = re.match(r", and |, | and ", rest[q.end():])
        if not sep:
            return out
        rest = rest[q.end() + sep.end():]


def branch_items(branch):
    """The items the head of a branch name gives: its leading run of `f<digits>-` segments. `f855-edit-lock-f2-lockbusy` is
    F-855 alone (finding 18 of F-851: the `-f2-` inside a name is not an item)."""
    m = re.match(r"(?:f\d{1,4}-)+", branch)
    return sorted({f"F-{int(n):03d}" for n in re.findall(r"f(\d+)-", m.group(0))}) if m else []


def brief_items(text):
    """The items of a brief's `Items:` line (`none` gives an empty list); None if the text has no such line."""
    m = re.search(r"^Items:[ \t]*(.*)$", text or "", re.M)
    if not m:
        return None
    return sorted({f"F-{int(n):03d}" for n in re.findall(r"\bF-(\d{1,4})\b", m.group(1))})


def brief_path(branch):
    return f"{BRIEFS}/{branch.replace('/', '-')}.md"


def merge_items(branches, brief_text):
    """The ledger's item field: for each branch the items of its brief (`brief_text(branch)` is the brief's text or None),
    else those of the head of its name. Nothing else gives an item (ADR 0752)."""
    items = set()
    for b in branches:
        got = brief_items(brief_text(b))
        items.update(branch_items(b) if got is None else got)
    return sorted(items)


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
    """(binary, test) -> seconds from a gate_test.py TSV, or from every `times*.tsv` under a folder (one per shard of a gate;
    where two files have the same test the larger seconds are kept)."""
    out = {}
    if path and os.path.isdir(path):
        for d, _, names in sorted(os.walk(path)):
            for n in sorted(names):
                if n.startswith("times") and n.endswith(".tsv"):
                    for k, v in read_times(os.path.join(d, n)).items():
                        out[k] = max(v, out.get(k, v))
    elif path and os.path.isfile(path):
        for ln in open(path, encoding="utf-8").read().splitlines()[1:]:
            f = ln.split("\t")
            if len(f) >= 4 and f[3]:
                try:
                    out[(f[0], f[1])] = float(f[3])
                except ValueError:
                    pass
    return out


def default_times(rows, root, exists=os.path.exists):
    """(path, commit) of the newest gate line whose detail names a times path that exists (`times=PATH`; a relative path is
    taken from the repository root); (None, None) if no line does."""
    for r in reversed(rows):
        m = re.search(r"(?:^|;)times=([^;]+)", r["detail"]) if r["kind"] == "gate" else None
        if m:
            p = m.group(1).strip()
            p = p if os.path.isabs(p) else os.path.join(root, p)
            if exists(p):
                return p, r["commit"]
    return None, None


def times_source(given, rows, root, default_s, exists=os.path.exists):
    """(path or None, the line the plan prints) of the recorded seconds: --times when given, else the ledger's default."""
    if given:
        return given, f"recorded seconds: {given} (--times)"
    path, gate = default_times(rows, root, exists)
    if path:
        return path, f"recorded seconds: {path} (named by the gate line of {gate})"
    return None, (f"recorded seconds: none (no gate line of the ledger names a times path that exists; every binary counts "
                  f"{default_s:g} s)")


def binary_units(idx, crates_wanted, times, default_s, build_s=0.0):
    """One unit per integration-test binary of the wanted crates: crate, target, binary, seconds (None if no record), cost.
    The cost is the recorded seconds (or the default) plus `build_s`, the build seconds counted per binary."""
    per = {}
    for t in idx["tests"]:
        if t["kind"] == "test" and t["crate"] in crates_wanted and not t["ignored"]:
            per.setdefault((t["crate"], t["target"], t["binary"]), []).append(t["name"])
    units = []
    for (crate, target, binary), names in sorted(per.items()):
        rec = [times[(binary, n)] for n in names if (binary, n) in times]
        secs = sum(rec) if rec else None
        units.append({"crate": crate, "target": target, "binary": binary, "seconds": secs,
                      "cost": (secs if secs is not None else float(default_s)) + float(build_s),
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


def unpatched_adrs(tests, adrs, patches):
    """[(ADR, number of its registered tests)] for each ADR of the merge that has tests in the index and no mutation patch:
    no patch of `patches` lies beside one of its test files, and none lies beside no ADR's test file (such a patch, of a
    script test for example, counts for every ADR of the merge)."""
    def beside(f):
        return re.sub(r"\.(rs|ts|tsx)$", "", f) + ".mutation.patch"
    files = {a: {t["file"] for t in tests if a in t["adrs"] and t["kind"] == "test"} for a in adrs}
    count = {a: sum(1 for t in tests if a in t["adrs"]) for a in adrs}
    owned = {beside(f) for a in adrs for f in files[a]}
    if any(p not in owned for p in patches):
        return []
    return [(a, count[a]) for a in adrs if count[a] and not any(beside(f) in patches for f in files[a])]


def script_test_commands(files, names):
    """The script tests of scripts/ (`names`: the file names of that folder) when the merge changes a file under scripts/."""
    if not any(f.startswith("scripts/") for f in files):
        return []
    return [f"python scripts/{n}" for n in sorted(names) if re.match(r"test_.*\.py$", n)]


def command_binary(cmd):
    """The test binary a cargo command runs: the target of --test, or the crate (with `_`) for --lib; None otherwise."""
    m = re.search(r"--test (\S+)", cmd)
    if m:
        return m.group(1)
    p = re.search(r"-p (\S+)", cmd)
    return p.group(1).replace("-", "_") if p and "--lib" in cmd.split() else None


def failed_names(cmd, out):
    """The tests a failed command names, as `binary::test`: cargo's `test <name> ... FAILED` lines and its last `failures:`
    list. A failure with no test name (a build error, a script) is `binary::*`, or the command itself."""
    def clean(x):
        return re.sub(r"[,;\t\r\n]+", " ", x)
    names = re.findall(r"^test (.+?) \.\.\. FAILED\s*$", out or "", re.M)
    lines = (out or "").splitlines()
    heads = [n for n, ln in enumerate(lines) if ln.strip() == "failures:"]
    for ln in lines[heads[-1] + 1:] if heads else []:
        m = re.match(r"^    (\S.*?)\s*$", ln)
        if not m:
            break
        names.append(m.group(1))
    b = command_binary(cmd)
    if not names:
        return [f"{b}::*" if b else clean(cmd)]
    return [f"{b or clean(cmd)}::{clean(n)}" for n in dict.fromkeys(names)]


def run_with_retries(cmd, cwd, run, retries=RETRIES):
    """Run a command; after a failure run it again, up to `retries` further times, and stop at the first run that passes.
    Returns {ok, runs, code, minutes, failed, out}: `out` is the last run's output, `failed` maps each name that failed (failed_names) to the first run it failed
    in; ok with a non-empty `failed` means flaky. A further run is the same command: nothing is filtered out of it."""
    failed, minutes, code, out = {}, 0.0, 0, ""
    for k in range(1, retries + 2):
        code, m, out = run(cmd, cwd, True)
        minutes += m
        if code == 0:
            return {"ok": True, "runs": k, "code": 0, "minutes": minutes, "failed": failed, "out": out}
        for n in failed_names(cmd, out):
            failed.setdefault(n, k)
        if k <= retries:
            print(f"push_main: failed (exit {code}) in run {k} of at most {retries + 1}; running it again: {cmd}", flush=True)
    return {"ok": False, "runs": retries + 1, "code": code, "minutes": minutes, "failed": failed, "out": out}


def ledger_appended(old, new):
    """True when the working-tree ledger `new` is the committed one `old` with whole lines appended (or unchanged)."""
    return new == old or (new.startswith(old) and (not old or old.endswith("\n")))


def blocking_changes(changed, generated, ledger_ok):
    """The changed tracked files that stop a run: all but the generated files (the run rewrites them) and the ledger when
    lines were only appended to it (a left-out line; the run commits it)."""
    return [p for p in changed if not (p in generated and p != LEDGER) and not (p == LEDGER and ledger_ok)]


def load_state(path, sha, base):
    """({command: {minutes, flaky, ...}}, note) of a stopped run on the same merge commit and base; ({}, why) otherwise."""
    if not os.path.isfile(path):
        return {}, ""
    try:
        with open(path, encoding="utf-8") as fh:
            st = json.load(fh)
    except (OSError, ValueError):
        return {}, f"the record of an earlier run, {path}, cannot be read; not used"
    if st.get("commit") != sha or st.get("base") != base:
        return {}, f"the record of an earlier run, {path}, is of another merge commit or base; not used"
    return dict(st.get("passed") or {}), ""


def save_state(path, sha, base, passed):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path + ".tmp", "w", encoding="utf-8") as fh:
        json.dump({"commit": sha, "base": base, "passed": passed}, fh, indent=1)
    os.replace(path + ".tmp", path)


def report_time(text):
    """The time of --reported as a datetime with an offset: ISO, `YYYY-MM-DDTHH:MM`, with optional seconds and an optional
    offset; a time without an offset is the local time of the machine."""
    try:
        if not REPORT_TIME.match(str(text).strip()):
            raise ValueError(text)
        t = datetime.datetime.fromisoformat(str(text).strip())
    except ValueError:
        raise SystemExit(f"push_main: --reported: {text!r} is not a time like 2026-11-21T14:05 or 2026-11-21T14:05+01:00")
    return t if t.tzinfo else t.astimezone()


def queue_minutes(reported, merge_epoch):
    """Whole minutes, rounded to the nearest, from the agent's report to the merge commit (its committer time in seconds)."""
    return int((float(merge_epoch) - reported.timestamp()) / 60.0 + 0.5)


def status_value(text, option):
    """A status as --proposed and --written take it: one of STATUSES, or `F-NNN=status` for each of several items."""
    v = str(text).strip()
    if v not in STATUSES and not STATUS_LIST.fullmatch(v):
        raise SystemExit(f"push_main: {option}: {text!r} is not done, partial or todo, nor a list like F-863=done,F-864=partial")
    return v


def measure_entries(reported=None, review_minutes=None, proposed=None, written=None, merge_epoch=None):
    """The entries the options of --run add to the detail of a merge line (ADR 0754), in their fixed order; an option that
    is not given (None) adds none. A value that cannot be read raises SystemExit."""
    out = []
    if reported is not None:
        t = report_time(reported)
        if merge_epoch is None or float(merge_epoch) < t.timestamp():
            raise SystemExit(f"push_main: --reported: {reported} is after the merge commit")
        out += [f"reported={t.isoformat(timespec='minutes')}", f"queue_minutes={queue_minutes(t, merge_epoch)}"]
    if review_minutes is not None:
        try:
            m = float(review_minutes)
        except (TypeError, ValueError):
            m = -1.0
        if not 0.0 <= m < float("inf"):
            raise SystemExit(f"push_main: --review-minutes: {review_minutes!r} is not a number of minutes, zero or more")
        out.append(f"review_minutes={m:g}")
    p = None if proposed is None else status_value(proposed, "--proposed")
    w = None if written is None else status_value(written, "--written")
    if p is not None and w is not None:
        if (p in STATUSES) != (w in STATUSES):
            raise SystemExit("push_main: --written and --proposed: one is a status and the other a list of items")
        if p not in STATUSES and sorted(x.split("=")[0] for x in p.split(",")) != sorted(x.split("=")[0] for x in w.split(",")):
            raise SystemExit("push_main: --written and --proposed name different items")
    out += [] if p is None else [f"proposed={p}"]
    out += [] if w is None else [f"written={w}"]
    return out


def escaped_entry(values, resolve):
    """The entry `escaped=...` of a gate line from the values of --escaped, or None when there is none. Each value is
    `COMMIT=BINARY::TEST`; `resolve(COMMIT)` gives the full commit or None. `none` alone gives `escaped=none`."""
    if not values:
        return None
    if "none" in values:
        if len(values) > 1:
            raise SystemExit("push_main: --escaped: none stands alone")
        return "escaped=none"
    out = []
    for v in values:
        m = ESCAPED.fullmatch(v)
        if not m:
            raise SystemExit(f"push_main: --escaped: {v!r} is not COMMIT=BINARY::TEST (no `;`, `,` or white space)")
        sha = resolve(m.group(1))
        if not sha:
            raise SystemExit(f"push_main: --escaped: {m.group(1)} is not a commit of this repository")
        entry = f"{sha[:10]}={m.group(2)}"
        if entry not in out:
            out.append(entry)
    return "escaped=" + ",".join(out)


def gate_detail(passed, failed, ignored, attribution=None, note="", skipped=None, times=None, escaped=None):
    d = f"passed={passed};failed={failed};ignored={ignored}" + ("" if skipped is None else f";skipped={skipped}")
    if attribution:
        fails = []
        for f in attribution.get("failures", []):
            adr = f.get("primary_adr") or (f.get("adrs") or ["none"])[0]
            item = (f.get("items") or ["none"])[0]
            fails.append(f"{f['binary']}::{f['test']}={adr}/{item}")
        fails += [f"doc::{x['name']}=none/none" for x in attribution.get("doc_failures", [])]
        if fails:
            d += ";fails=" + ",".join(fails)
    if times:
        d += ";times=" + times
    if escaped:
        d += ";" + escaped
    return d + (";" + note if note else "")


def mutants_command(packages, diff_file, out_dir, timeout=None):
    """The cargo-mutants command line for one merge: only the lines of the diff, only the touched packages."""
    cmd = ["cargo", "mutants", "--in-diff", diff_file, "--output", out_dir]
    for p in packages:
        cmd += ["--package", p]
    if timeout:
        cmd += ["--timeout", str(int(timeout))]
    return cmd


def mutants_counts(outcomes, missed_text=""):
    """(counts, missed names) from cargo-mutants' outcomes.json (a dict) and missed.txt. The top-level counts are used when
    present; otherwise the `summary` of each outcome is counted. The unmutated baseline run (`Success`) is not a mutant."""
    keys = {"caught": "CaughtMutant", "missed": "MissedMutant", "timeout": "Timeout", "unviable": "Unviable"}
    counts = {}
    for k, summary in keys.items():
        if isinstance(outcomes.get(k), int):
            counts[k] = outcomes[k]
        else:
            counts[k] = sum(1 for o in outcomes.get("outcomes", []) if o.get("summary") == summary)
    missed = [ln.strip() for ln in missed_text.splitlines() if ln.strip()]
    return counts, missed


def mutants_detail(counts, missed, shown=5):
    d = ";".join(f"{k}={counts[k]}" for k in ("caught", "missed", "timeout", "unviable"))
    if missed:
        d += ";missed_list=" + " | ".join(missed[:shown]) + (f" | and {len(missed) - shown} more" if len(missed) > shown else "")
    return d


def skipped_count(text):
    m = re.findall(r"(\d+) skipped", text)
    return int(m[-1]) if m else None


# ---------------------------------------------------------------- git and processes


def git(root, *args, check=True):
    p = subprocess.run(["git", *args], cwd=root, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, encoding="utf-8")
    if check and p.returncode:
        raise SystemExit(f"push_main: git {' '.join(args)}: {p.stderr.strip()}")
    return p.stdout


def git_try(root, *args):
    """(exit code, stdout) of a git command that may fail."""
    p = subprocess.run(["git", *args], cwd=root, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, encoding="utf-8")
    return p.returncode, p.stdout


def merge_info(root, rev, base=None):
    """What is checked as one diff. Without `base`: the merge `rev` against its first parent (two parents or more). With
    `base`: every merge on the first-parent line of base..rev, as the one diff base..rev; `rev` itself is a merge.
    Branches and ADRs are those of all the merges; the items come from the briefs or the heads of the branch names."""
    if base is None:
        parents = git(root, "rev-list", "--parents", "-n", "1", rev).split()
        if len(parents) < 3:
            raise SystemExit(f"push_main: {rev} is not a merge (parents: {len(parents) - 1})")
        sha, first, second = parents[0], parents[1], parents[2]
        merges = [sha]
    else:
        first = git(root, "rev-parse", f"{base}^{{commit}}").strip()
        line = [ln.split() for ln in git(root, "rev-list", "--first-parent", "--parents", f"{first}..{rev}").splitlines() if ln]
        if not line or line[-1][1:2] != [first]:
            raise SystemExit(f"push_main: --base {base} is not on the first-parent line below {rev}")
        if len(line[0]) < 3:
            raise SystemExit(f"push_main: {rev}, the last commit of {base}..{rev}, is not a merge")
        sha, second = line[0][0], line[0][2]
        merges = [ln[0] for ln in reversed(line) if len(ln) >= 3]
    subjects = [git(root, "log", "-1", "--format=%s", m).strip() for m in merges]
    files = [f for f in git(root, "diff", "--name-only", first, sha).splitlines() if f]
    added = [f for f in git(root, "diff", "--name-only", "--diff-filter=A", first, sha).splitlines() if f]
    branches = list(dict.fromkeys(b for s in subjects for b in subject_branches(s)))
    adrs = {a for s in subjects for a in parse_subject(s)[2]} | set(adrs_in_files(added)[0])

    def brief_text(branch):
        code, text = git_try(root, "show", f"{sha}:{brief_path(branch)}")
        return None if code else text
    return {"sha": sha, "first": first, "second": second, "subject": subjects[-1], "files": files, "branch": ",".join(branches),
            "branches": branches, "merges": merges, "items": merge_items(branches, brief_text), "adrs": sorted(adrs)}


def since_verified(root, rows, head="HEAD"):
    base = newest_green(rows)
    n = len(git(root, "rev-list", "--first-parent", "--merges", f"{base}..{head}").split())
    return base, n


def sh(cmd, cwd, capture=False):
    """Run one shell command; returns (exit code, minutes, output or ''). With `capture` the output (stderr with it) is
    printed line by line as it comes and also returned."""
    t0 = time.time()
    if capture:
        sys.stdout.flush()
        raw = getattr(sys.stdout, "buffer", None)
        p = subprocess.Popen(cmd, cwd=cwd, shell=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        kept = []
        for chunk in iter(p.stdout.readline, b""):
            kept.append(chunk)
            if raw is not None:  # the bytes as they are: the console's encoding may not hold every character
                raw.write(chunk)
                raw.flush()
            else:
                sys.stdout.write(chunk.decode("utf-8", errors="replace"))
        p.stdout.close()
        return p.wait(), (time.time() - t0) / 60.0, b"".join(kept).decode("utf-8", errors="replace").replace("\r\n", "\n")
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


def make_plan(root, info, idx, times, rows, budget_min, default_s, build_s=BUILD_SECONDS):
    crates = idx["crates"]
    touched = touched_crates(info["files"], crates)
    sdir = os.path.join(root, "scripts")
    always = script_test_commands(info["files"], os.listdir(sdir) if os.path.isdir(sdir) else [])
    for c in touched:
        always.append(f"python scripts/fmt_check.py {c}")  # cargo fmt -p, in batches (os error 206 on Windows for rcfd-fem)
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
    g1 = order_group(binary_units(idx, set(touched), times, default_s, build_s), failed)
    g2 = order_group(binary_units(idx, set(dependents(crates, touched)), times, default_s, build_s), failed)
    taken, left, used = cut([g1, g2], budget_min * 60.0, set(always))
    adr_tests = sorted({t["file"] for t in idx["tests"] if set(t["adrs"]) & set(info["adrs"]) and t["kind"] == "test"})
    patches = patch_paths(info["files"], adr_tests, lambda p: os.path.isfile(os.path.join(root, p)))
    studio = any(f.startswith("apps/studio-web/") for f in info["files"])
    return {"touched": touched, "always": always, "selected": taken, "left": left, "selected_minutes": used / 60.0,
            "left_minutes": sum(u["cost"] for u in left) / 60.0, "patches": patches, "studio": studio, "build_s": float(build_s),
            "no_patch": unpatched_adrs(idx["tests"], info["adrs"], patches)}


def print_plan(info, plan, base, n, times_note=""):
    print(f"merge {info['sha'][:8]} of '{info['branch']}': items {','.join(info['items']) or '-'}; ADRs {','.join(info['adrs']) or '-'}; "
          f"{len(info['files'])} files; crates {','.join(plan['touched']) or '-'}")
    if len(info.get("merges", [])) > 1:
        print(f"joint diff of {len(info['merges'])} merges, {info['first'][:8]}..{info['sha'][:8]}: "
              + ", ".join(m[:8] for m in info["merges"]))
    print(f"merges on main since the newest green full run ({base[:8]}): {n}")
    if times_note:
        print(times_note)
    print("always:")
    for c in plan["always"]:
        print("  " + c)
    build = f", with {plan['build_s']:g} s of build counted per binary" if plan.get("build_s") else ""
    print(f"diff-selected ({len(plan['selected'])} binaries, {plan['selected_minutes']:.1f} recorded minutes{build}):")
    for u in plan["selected"]:
        print(f"  {u['cmd']}    # {u['seconds']:.0f} s" if u["seconds"] is not None else f"  {u['cmd']}    # no record")
    print(f"left out for CI on main: {len(plan['left'])} binaries, {plan['left_minutes']:.1f} recorded minutes")
    for p in plan["patches"]:
        print(f"mutation patch: {p}")
    for adr, count in plan.get("no_patch", []):
        print(f"no mutation patch: ADR {adr} has {count} registered tests and no committed mutation patch (agent rules, rule 16)")
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


def tree_blockers(root, run):
    """The changed tracked files that stop a run (blocking_changes): the list of generated files is asked for only when a
    file is changed, and the committed ledger is compared with the working-tree one only when the ledger is."""
    changed = [f for f in git(root, "diff", "--name-only", "HEAD").splitlines() if f]
    if not changed:
        return []
    code, _, listed = run("python scripts/process_report.py --list-generated", root, True)
    ledger_ok = False
    if LEDGER in changed and os.path.isfile(os.path.join(root, LEDGER)):
        code2, old = git_try(root, "show", f"HEAD:{LEDGER}")
        with open(os.path.join(root, LEDGER), encoding="utf-8") as fh:
            ledger_ok = ledger_appended("" if code2 else old, fh.read())
    return blocking_changes(changed, set(GENERATED) | set([] if code else (listed or "").split()), ledger_ok)


def do_run(root, a, info, plan, run=sh):
    """Run the plan and write the ledger line. `run(cmd, cwd, capture)` runs one command (sh; the tests give a recorder)."""
    ledger = os.path.join(root, LEDGER)
    if git(root, "rev-parse", "HEAD").strip() != info["sha"]:
        raise SystemExit("push_main: --run needs the merge at HEAD")
    # the process measures the lead states (ADR 0754): read before any check, so that a value that cannot be read stops here
    given = {name: getattr(a, name, None) for name, _ in MEASURE_OPTIONS}
    epoch = int(git(root, "log", "-1", "--format=%ct", info["sha"]).strip()) if given["reported"] is not None else None
    measures = measure_entries(merge_epoch=epoch, **given)
    blockers = tree_blockers(root, run)
    if blockers:
        raise SystemExit("push_main: the working tree has uncommitted changes: " + ", ".join(blockers[:8]))
    state = os.path.join(root, "target", "push_main", f"run-{info['sha'][:8]}.json")
    earlier, note = ({}, "") if getattr(a, "fresh", False) else load_state(state, info["sha"], info["first"])
    if note:
        print("push_main: " + note)
    passed, cmds, flaky, minutes, resumed, studio_skipped = dict(earlier), [], [], 0.0, 0, None

    def stopped(what, code=None):
        print(f"push_main: FAILED{'' if code is None else f' (exit {code})'}: {what}\n"
              "push_main: nothing written to the ledger; the merge is not pushed")
        return 1

    def repeated(key):
        """True, with the earlier run's minutes and flaky names counted, when `key` passed on this merge commit before."""
        nonlocal minutes, resumed
        if key not in earlier:
            return False
        print(f"push_main: passed earlier on this merge commit, not repeated: {key}", flush=True)
        minutes += float(earlier[key].get("minutes") or 0.0)
        flaky.extend(n for n in earlier[key].get("flaky") or [] if n not in flaky)
        resumed += 1
        return True

    def record(key, m, names=(), **more):
        passed[key] = dict(minutes=round(m, 3), flaky=list(names), **more)
        save_state(state, info["sha"], info["first"], passed)

    def checked(cmd, cwd, key):
        """Run one command with its further runs; True if it passed. Flaky names are printed and kept."""
        nonlocal minutes
        r = run_with_retries(cmd, cwd, run)
        minutes += r["minutes"]
        if not r["ok"]:
            return False, r
        for name, k in r["failed"].items():
            print(f"push_main: FLAKY: {name} failed in run {k} of {r['runs']} of {key}", flush=True)
            if name not in flaky:
                flaky.append(name)
        return True, r

    # the generated files first: the process check and docs_site then see the pages the push will carry (finding 9 of F-851)
    for cmd in REGENERATE:
        code = run(cmd, root, True)[0]
        if code:
            return stopped(cmd, code)
    for cmd in plan["always"] + [u["cmd"] for u in plan["selected"]]:
        cmds.append(cmd)
        if repeated(cmd):
            continue
        print(f"push_main: {cmd}", flush=True)
        ok, r = checked(cmd, root, cmd)
        if not ok:
            return stopped(f"{cmd} (in each of {r['runs']} runs: {', '.join(r['failed'])})", r["code"])
        record(cmd, r["minutes"], r["failed"])
    for p in plan["patches"]:
        if repeated(f"mutation {p}"):
            continue
        t0 = time.time()
        ok, msg = verify_patch(root, p, run=run)
        minutes += (time.time() - t0) / 60.0
        print("push_main: mutation " + msg)
        if not ok:
            return stopped("mutation " + msg)
        record(f"mutation {p}", (time.time() - t0) / 60.0)
    if plan["studio"]:
        key = "npm test (apps/studio-web)"
        cmds.append(key)
        if repeated(key):
            studio_skipped = earlier[key].get("skipped")
        else:
            ok, r = checked("npm test", os.path.join(root, "apps", "studio-web"), key)
            if not ok:
                return stopped(f"npm test in apps/studio-web (in each of {r['runs']} runs)", r["code"])
            studio_skipped = skipped_count(r["out"])
            record(key, r["minutes"], r["failed"], skipped=studio_skipped)
    detail = "exit=0;mutations=%d" % len(plan["patches"])
    if studio_skipped is not None:
        detail += f";studio_skipped={studio_skipped}"
    if flaky:
        print("push_main: flaky in this run (failed, then passed in a further run): " + ", ".join(flaky))
        detail += ";flaky=" + ",".join(flaky)
    if resumed:
        detail += f";resumed={resumed}"
    if len(info.get("merges", [])) > 1:
        detail += ";joint=" + ",".join(m[:10] for m in info["merges"] if m != info["sha"])
    detail += "".join(";" + e for e in measures)
    detail += ";cmds=" + " | ".join(cmds)
    append_ledger(ledger, ledger_line(date=today(), kind="merge", commit=info["sha"][:10], branch=info["branch"],
                                      items=",".join(info["items"]), adrs=",".join(info["adrs"]), result="pass",
                                      minutes=f"{minutes:.1f}", left_out=len(plan["left"]),
                                      left_out_minutes=f"{plan['left_minutes']:.1f}", detail=detail))
    if os.path.isfile(state):
        os.remove(state)
    code, _, listed = run("python scripts/process_report.py --list-generated", root, True)
    paths = [p for p in GENERATED + ([] if code else (listed or "").split()) if os.path.exists(os.path.join(root, p))]
    git(root, "add", "--", *paths)
    if git(root, "diff", "--cached", "--name-only").strip():
        what = ", ".join(info["items"]) or info["branch"] or info["sha"][:8]
        git(root, "commit", "-m", f"Ledger and generated pages after the merge of {what}")
    lo = left_out_file(root, info, plan) if plan["left"] else None
    if a.push:
        if run("git push origin HEAD:main", root, False)[0]:
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


def do_mutants(root, a, info, touched):
    import shutil
    import tempfile
    diff = subprocess.run(["git", "-C", root, "diff", info["first"], info["sha"], "--", "crates/*.rs"], stdout=subprocess.PIPE,
                          check=True).stdout
    if not diff.strip() or not touched:
        print("push_main: the merge changes no Rust file under crates/; no mutants")
        return 0
    work = tempfile.mkdtemp(prefix="mutants-")
    diff_file = os.path.join(work, "merge.diff")
    with open(diff_file, "wb") as fh:
        fh.write(diff)
    cmd = mutants_command(touched, diff_file, work, a.mutants_timeout)
    print(" ".join(cmd))
    if not a.run:
        print("dry run: nothing was run (use --run)")
        shutil.rmtree(work, ignore_errors=True)
        return 0
    if git(root, "rev-parse", "HEAD").strip() != info["sha"]:
        raise SystemExit("push_main: --mutants --run needs the merge at HEAD")
    try:
        ok = subprocess.run(["cargo", "mutants", "--version"], cwd=root, stdout=subprocess.PIPE, stderr=subprocess.STDOUT).returncode == 0
    except OSError:
        ok = False
    if not ok:
        raise SystemExit("push_main: cargo-mutants is not installed (cargo install cargo-mutants); nothing was run")
    t0 = time.time()
    subprocess.run(cmd, cwd=root)  # its exit code is non-zero when a mutant was missed; the counts below are the result
    out = os.path.join(work, "mutants.out")
    try:
        outcomes = json.load(open(os.path.join(out, "outcomes.json"), encoding="utf-8"))
    except (OSError, ValueError) as e:
        raise SystemExit(f"push_main: no readable {out}/outcomes.json ({e}); nothing written to the ledger")
    mp = os.path.join(out, "missed.txt")
    counts, missed = mutants_counts(outcomes, open(mp, encoding="utf-8").read() if os.path.isfile(mp) else "")
    append_ledger(os.path.join(root, LEDGER), ledger_line(
        date=today(), kind="mutants", commit=info["sha"][:10], branch=info["branch"], items=",".join(info["items"]),
        adrs=",".join(info["adrs"]), result="red" if counts["missed"] else "green", minutes=f"{(time.time() - t0) / 60.0:.1f}",
        detail=mutants_detail(counts, missed)))
    print(f"push_main: mutants {mutants_detail(counts, missed)}; reports in {out}")
    return 0


def gate_times_field(path, root):
    """The path of --gate-times as the ledger holds it: forward slashes; it must exist and hold no `;`, tab or line break."""
    p = path.replace(chr(92), "/")
    if any(c in p for c in (";", chr(9), chr(10), chr(13))):
        raise SystemExit("push_main: --gate-times: the path holds a `;`, a tab or a line break")
    if not os.path.exists(p if os.path.isabs(p) else os.path.join(root, p)):
        raise SystemExit(f"push_main: --gate-times: {p} does not exist")
    return p


def do_gate_line(root, a):
    if not a.commit or a.passed is None or a.failed is None or a.ignored is None:
        raise SystemExit("push_main: --gate-line needs --commit, --passed, --failed and --ignored")
    att = json.load(open(a.attribution, encoding="utf-8")) if a.attribution else None
    green = a.failed == 0 and not (att and (att.get("build_failed") or att.get("doc_failures")))
    sha = git(root, "rev-parse", a.commit).strip()
    times = gate_times_field(a.gate_times, root) if getattr(a, "gate_times", None) else None
    values = getattr(a, "escaped", None)
    if values and green:
        raise SystemExit("push_main: --escaped is for a red gate; this one is green")

    def resolve(rev):
        code, out = git_try(root, "rev-parse", "--verify", "--quiet", rev + "^{commit}")
        return None if code else out.strip()
    escaped = escaped_entry(values, resolve)
    append_ledger(os.path.join(root, LEDGER), ledger_line(
        date=a.date or today(), kind="gate", commit=sha[:10], result="green" if green else "red",
        minutes=a.minutes or "", detail=gate_detail(a.passed, a.failed, a.ignored, att, a.note or "", a.skipped, times, escaped)))
    if green:
        git(root, "tag", "-f", "verified", sha)
        print(f"push_main: tag verified moved to {sha[:10]}")
    return 0


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--root", default=".")
    ap.add_argument("--merge", default="HEAD", help="the merge commit (dry run only; --run needs HEAD)")
    ap.add_argument("--base", metavar="REV", help="several merges as one joint diff: the commit on main before the first of them")
    ap.add_argument("--fresh", action="store_true", help="with --run: repeat what passed on this merge commit in a stopped run")
    ap.add_argument("--build-seconds", type=float, default=BUILD_SECONDS,
                    help="build seconds counted per selected test binary (default: the figure of ADR 0752)")
    ap.add_argument("--gate-times", metavar="PATH",
                    help="with --gate-line: the times TSV of this gate, or the folder of its shards; it becomes the default of --times")
    ap.add_argument("--run", action="store_true")
    ap.add_argument("--push", action="store_true")
    ap.add_argument("--left-out-run", action="store_true")
    ap.add_argument("--run-left-out", metavar="JSON", help="internal: run the left-out commands of one merge")
    ap.add_argument("--times", help="TSV of an earlier gate_test.py --tsv run (recorded seconds per test), or a folder of "
                    "them; default: the newest times path a gate line of the ledger names")
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
    ap.add_argument("--skipped", type=int, help="passed tests that reported a skip (gate_test.py --skip-pattern)")
    ap.add_argument("--mutants", action="store_true", help="generated mutants on the lines the merge changed (cargo-mutants)")
    ap.add_argument("--mutants-timeout", type=float, help="seconds per mutant, passed to cargo mutants --timeout")
    ap.add_argument("--minutes")
    ap.add_argument("--note")
    ap.add_argument("--attribution", help="JSON written by scripts/gate_attribute.py")
    ap.add_argument("--reported", metavar="TIME", help="with --run: the time of the agent's report (2026-11-21T14:05, local "
                    "time without an offset); the merge line gets reported= and queue_minutes=")
    ap.add_argument("--review-minutes", metavar="N", help="with --run: the lead's minutes on the agent's report")
    ap.add_argument("--proposed", metavar="STATUS", help="with --run: the status the agent proposed (done, partial, todo, or "
                    "F-863=done,F-864=partial for several items)")
    ap.add_argument("--written", metavar="STATUS", help="with --run: the status the lead writes, in the same form")
    ap.add_argument("--escaped", metavar="COMMIT=BINARY::TEST", action="append",
                    help="with --gate-line on a red gate: the commit that brought the failure of that test in (repeat for "
                    "several); `none` when no commit did")
    a = ap.parse_args(argv)
    for name, flag in MEASURE_OPTIONS:
        if getattr(a, name) is not None and not a.run:
            raise SystemExit(f"push_main: {flag} needs --run")
    if a.escaped and not a.gate_line:
        raise SystemExit("push_main: --escaped needs --gate-line")
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
    info = merge_info(root, a.merge, a.base)
    idx = adr_test_index.build(root, adr_test_index.metadata(root, a.metadata), "git", e8=True)
    if a.mutants:
        return do_mutants(root, a, info, touched_crates(info["files"], idx["crates"]))
    times, times_note = times_source(a.times, rows, root, a.default_seconds)
    plan = make_plan(root, info, idx, read_times(times), rows, a.budget_minutes, a.default_seconds, a.build_seconds)
    base, n = since_verified(root, rows, info["sha"])
    print_plan(info, plan, base, n, times_note)
    if not a.run:
        print("dry run: nothing was run (use --run)")
        return 0
    return do_run(root, a, info, plan)


if __name__ == "__main__":
    sys.exit(main())
