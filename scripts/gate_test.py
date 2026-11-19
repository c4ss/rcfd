#!/usr/bin/env python3
"""Release test stage of the gate (ADR 0127): the same tests as `cargo test --workspace --release --no-fail-fast`, run as one
process per test from a single pool, so tests of different binaries overlap instead of waiting for the slowest test of each binary.

Usage (from the workspace root):
    python scripts/gate_test.py [--jobs N] [--order prev.tsv] [--tsv out.tsv] [cargo package selection, default --workspace]
--jobs     concurrent test processes (default RUST_TEST_THREADS, else 3): the same number of tests at once as one cargo test binary.
--order    a TSV written by an earlier run: start its slowest tests first. Ordering only; every test still runs.
--tsv      write binary, test, outcome, seconds, peak working set (MB) for every test.
--heartbeat S  every S seconds (default 300; 0 off) write a progress line to stderr: tests done of total, failures so far,
           elapsed, estimated time left, and the longest-running tests still going.
--shard K/N  run only shard K of N (1 <= K <= N). The tests are dealt to the N shards by their seconds in the --order TSV so that
           the shards carry about equal recorded time; with the same --order file the N runs together run every test once. The
           ignored tests and the doc-tests are reported by shard 1 only, so the sums over the N logs equal those of one run. The
           split does not depend on the order of the binaries, and a line `gate_test: shard K/N runs m of M tests, split DIGEST`
           lets scripts/gate_shards_check.py show that the N runs dealt from one list (ADR 0653 Amendment 3; gate.yml input `shards`).
--no-doc   skip the closing doc-tests (ADR 0681: a run restricted with `--test NAME`, where `cargo test --doc` is an error). Cargo options
           in the selection come after `--`: gate_test.py --no-doc -- -p CRATE --features F --test NAME.
--timeout-factor F, --timeout-min S, --timeout-max S (ADR 0129; defaults 5, 600, 3600): a test's time limit is
           clamp(F * its seconds in the --order TSV, min, max), max if it has no record; --timeout-max 0 turns the limit off. On expiry
           the test's process tree (never cargo) is killed and the test fails with outcome TIMEOUT; the other tests go on.
--limits FILE (ADR 0740; off by default; the gate passes scripts/gate_limits.tsv): a table with the columns binary, test, limit_s, adr.
           A test named by a row has the limit of its row, whatever the three options above and the --order record give; every other
           test keeps the limit it has without the table. --timeout-max 0 still turns every limit off.
--skip-pattern REGEX (off by default; docs/process/documentation-process-review.md I2): every test runs with `--show-output`, and
           the TSV gains a last column `skipped` with the output lines of a PASSED test that match REGEX, joined by " | ". The closing
           `gate_test:` line then also gives the number of passed tests that reported a skip. Suggested REGEX: SKIP_SUGGESTED below.
           It matches 24 of the 26 print lines of the test sources that contain "skip" (13 files, 2026-11-18); some of those lines
           report a skipped part or a skipped input, not a skipped test, so the count is an upper bound until an item fixes the pattern.
           The child-process entry points that pass without doing anything print nothing and are not found this way.
Progress goes to stderr: phase markers, one `[k/N outcome seconds MB elapsed] binary::test` line per finished test and the
heartbeat lines. Output keeps the libtest lines (`Running`, `test X ... ok`, `test result:`), so the gate's summing awk works unchanged. Afterwards
`cargo test --doc` runs the doc-tests. Exit status 0 only if the build, every test and the doc-tests pass.
"""
import argparse
import hashlib
import concurrent.futures as cf
import json
import os
import re
import signal
import subprocess
import sys
import threading
import time

RESULT = re.compile(r"^test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored", re.M)


# "skipped" as a word, not after the count 0 ("0 skipped" is a tally, not a skip).
SKIP_SUGGESTED = r"(?i)(?<!\b0 )\bskipped\b"


def skip_lines(out, pattern):
    """The lines of a test's output that match the compiled pattern, stripped; tabs become spaces so they fit a TSV cell."""
    return [ln.strip().replace("\t", " ") for ln in out.splitlines() if pattern.search(ln)]


def tsv_row(stem, test, outcome, seconds, mb, skipped=None):
    """One TSV row; `skipped` (a list of lines, or None when --skip-pattern is off) adds the last column."""
    row = f"{stem}\t{test}\t{outcome}\t{seconds}\t{mb}"
    return row if skipped is None else row + "\t" + " | ".join(skipped)


_PROGRESS = threading.Lock()


def hms(sec):
    sec = int(max(sec, 0))
    return f"{sec // 3600:d}:{sec // 60 % 60:02d}:{sec % 60:02d}"


def progress(msg):
    """A progress line on stderr, flushed at once (the gate shows stderr live in the Actions log)."""
    with _PROGRESS:  # one write per line, so the pool's threads never interleave within a line
        sys.stderr.write(msg + "\n")
        sys.stderr.flush()


def shard_of(seconds, n, default=1.0):
    """The shard (0 to n-1) of each test, given the recorded seconds of the tests in the order they would start (None where
    there is no record, counted as `default`). Each test goes to the shard with the least seconds so far, the lowest-numbered
    on a tie, so the same order and records always give the same split and the shards carry about equal recorded time."""
    load = [0.0] * n
    out = []
    for s in seconds:
        k = min(range(n), key=lambda i: (load[i], i))
        load[k] += default if s is None else s
        out.append(k)
    return out


def shard_split(keys, seconds, n):
    """The split of ADR 0653 Amendment 3: `keys` are (binary, manifest path, test) and `seconds` the recorded seconds (None
    without a record) of all tests, in any order. Returns (order, part, digest): the indices in the order the tests are
    dealt (longest first, then by key, so the order does not depend on the order of the binaries), the shard (0 to n-1)
    of each test in that order, and the SHA-256 of the ordered list, which shards with the same input share."""
    order = sorted(range(len(keys)), key=lambda k: (-(seconds[k] or 0.0), keys[k]))
    secs = [seconds[k] for k in order]
    text = "".join(f"{keys[k][0]}\t{keys[k][1]}\t{keys[k][2]}\t{seconds[k]}\n" for k in order)
    return order, shard_of(secs, n), hashlib.sha256(text.encode("utf-8")).hexdigest()


def parse_shard(text):
    """"K/N" as (K, N) with 1 <= K <= N."""
    m = re.fullmatch(r"(\d+)/(\d+)", text or "")
    if not m or not 1 <= int(m.group(1)) <= int(m.group(2)):
        raise SystemExit(f"gate_test: --shard takes K/N with 1 <= K <= N, not {text!r}")
    return int(m.group(1)), int(m.group(2))


def limit_s(rec, factor, lo, hi):
    """Time limit of one test (ADR 0129): clamp(factor * recorded seconds, lo, hi); hi without a record; None if hi <= 0 (off)."""
    if hi <= 0:
        return None
    if rec is None:
        return hi
    return min(max(factor * rec, lo), hi)


def read_limits(path):
    """The table of --limits (ADR 0740): {(binary, test): seconds}. Lines that are empty or start with `#` are skipped and the
    first other line is the header. A row with fewer than four cells, a limit that is not a positive number or a test named
    twice ends the run with a message that names the line."""
    table, header = {}, False
    with open(path, encoding="utf-8") as fh:
        for n, raw in enumerate(fh.read().splitlines(), 1):
            if not raw.strip() or raw.lstrip().startswith("#"):
                continue
            f = [c.strip() for c in raw.split("\t")]
            if not header:
                header = True
                if f[:4] != ["binary", "test", "limit_s", "adr"]:
                    raise SystemExit(f"gate_test: {path} line {n}: the header is not binary, test, limit_s, adr")
                continue
            if len(f) < 4 or not f[0] or not f[1]:
                raise SystemExit(f"gate_test: {path} line {n}: a row needs binary, test, limit_s, adr")
            try:
                sec = float(f[2])
            except ValueError:
                sec = 0.0
            if not 0.0 < sec < float("inf"):
                raise SystemExit(f"gate_test: {path} line {n}: the limit {f[2]!r} is not a positive number of seconds")
            if (f[0], f[1]) in table:
                raise SystemExit(f"gate_test: {path} line {n}: {f[0]}::{f[1]} is named twice")
            table[(f[0], f[1])] = sec
    return table


def test_limit(key, rec, factor, lo, hi, table=None):
    """Time limit of the test `key` = (binary, test): its row of the --limits table if it has one (ADR 0740), else `limit_s`;
    None if hi <= 0 (every limit off)."""
    if hi > 0 and table and key in table:
        return table[key]
    return limit_s(rec, factor, lo, hi)


def peak_mb(proc, field="PeakWorkingSetSize"):
    """Peak (or current, with field="WorkingSetSize") working set of a child (Windows only; 0 elsewhere)."""
    if os.name != "nt":
        return 0
    import ctypes
    from ctypes import wintypes

    class PMC(ctypes.Structure):
        _fields_ = [("cb", wintypes.DWORD), ("PageFaultCount", wintypes.DWORD)] + [
            (n, ctypes.c_size_t)
            for n in ("PeakWorkingSetSize", "WorkingSetSize", "QuotaPeakPagedPoolUsage", "QuotaPagedPoolUsage",
                      "QuotaPeakNonPagedPoolUsage", "QuotaNonPagedPoolUsage", "PagefileUsage", "PeakPagefileUsage")
        ]

    c = PMC()
    c.cb = ctypes.sizeof(c)
    # proc._handle: the process handle stays open after wait() until the Popen object is dropped.
    ok = ctypes.windll.kernel32.K32GetProcessMemoryInfo(wintypes.HANDLE(int(proc._handle)), ctypes.byref(c), c.cb)
    return getattr(c, field) / 2**20 if ok else 0


def win_descendants(root):
    """Pids of root and its descendants, leaving out cargo.exe and its subtree (Windows; Toolhelp snapshot)."""
    import ctypes
    from ctypes import wintypes

    k = ctypes.windll.kernel32
    k.CreateToolhelp32Snapshot.restype = wintypes.HANDLE
    k.OpenProcess.restype = wintypes.HANDLE

    class PE(ctypes.Structure):
        _fields_ = [("dwSize", wintypes.DWORD), ("cntUsage", wintypes.DWORD), ("pid", wintypes.DWORD),
                    ("heap", ctypes.c_size_t), ("module", wintypes.DWORD), ("threads", wintypes.DWORD),
                    ("ppid", wintypes.DWORD), ("prio", wintypes.LONG), ("flags", wintypes.DWORD),
                    ("exe", wintypes.WCHAR * 260)]

    def created(pid):
        h = k.OpenProcess(0x1000, False, pid)  # PROCESS_QUERY_LIMITED_INFORMATION
        if not h:
            return None
        t = [wintypes.FILETIME() for _ in range(4)]
        ok = k.GetProcessTimes(wintypes.HANDLE(h), *[ctypes.byref(x) for x in t])
        k.CloseHandle(wintypes.HANDLE(h))
        return (t[0].dwHighDateTime << 32 | t[0].dwLowDateTime) if ok else None

    snap = k.CreateToolhelp32Snapshot(2, 0)  # TH32CS_SNAPPROCESS
    kids = {}
    e = PE()
    e.dwSize = ctypes.sizeof(e)
    more = k.Process32FirstW(wintypes.HANDLE(snap), ctypes.byref(e))
    while more:
        kids.setdefault(e.ppid, []).append((e.pid, e.exe))
        more = k.Process32NextW(wintypes.HANDLE(snap), ctypes.byref(e))
    k.CloseHandle(wintypes.HANDLE(snap))
    pids, todo = [root], [root]
    while todo:
        q = todo.pop()
        tq = created(q)
        for c, exe in kids.get(q, []):
            tc = created(c)
            # a parent pid can be stale and reused: a real child was created after its parent
            if exe.lower() != "cargo.exe" and c not in pids and tq is not None and tc is not None and tc >= tq:
                pids.append(c)
                todo.append(c)
    return pids


def kill_tree(p):
    """Kill a timed-out test and the processes it started (never cargo, never anything the runner did not start)."""
    if os.name == "nt":
        pids = win_descendants(p.pid)
        subprocess.run(["taskkill", "/F", *[x for c in pids for x in ("/PID", str(c))]],
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    else:
        # shortcut: the whole process group, a cargo started by the test included; the gate runs on Windows
        os.killpg(p.pid, signal.SIGKILL)
    p.kill()


LIVE = set()  # running test processes, for the memory sampler
LIVE_LOCK = threading.Lock()


def run(cmd, cwd, env, limit=None):
    """Run one process; returns exit code, output, seconds, peak MB, timed out."""
    t = time.time()
    p = subprocess.Popen(cmd, cwd=cwd, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                         start_new_session=os.name != "nt")
    with LIVE_LOCK:
        LIVE.add(p)
    timed_out = False
    try:
        out = p.communicate(timeout=limit)[0]
    except subprocess.TimeoutExpired:
        timed_out = True
        kill_tree(p)
        try:
            out = p.communicate(timeout=30)[0]
        except subprocess.TimeoutExpired:  # a process outside the killed tree holds the pipe
            p.wait()
            out = b"gate_test: output lost, the pipe stayed open after the kill"
    finally:
        with LIVE_LOCK:
            LIVE.discard(p)
    return p.returncode, (out or b"").decode("utf-8", "replace"), time.time() - t, peak_mb(p), timed_out


def sample_memory(stop, peak):
    """Largest summed working set of the running test processes, sampled every 2 s (ADR 0129 D2)."""
    while not stop.wait(2):
        with LIVE_LOCK:
            procs = list(LIVE)
        peak[0] = max(peak[0], sum(peak_mb(q, "WorkingSetSize") for q in procs))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--jobs", type=int, default=int(os.environ.get("RUST_TEST_THREADS", "3")))
    ap.add_argument("--order")
    ap.add_argument("--tsv")
    ap.add_argument("--heartbeat", type=float, default=300.0)
    ap.add_argument("--timeout-factor", type=float, default=5.0)
    ap.add_argument("--timeout-min", type=float, default=600.0)
    ap.add_argument("--timeout-max", type=float, default=3600.0)
    ap.add_argument("--limits")
    ap.add_argument("--no-doc", action="store_true")
    ap.add_argument("--skip-pattern")
    ap.add_argument("--shard")
    ap.add_argument("select", nargs="*")
    a = ap.parse_args()
    select = a.select or ["--workspace"]
    shard = parse_shard(a.shard) if a.shard else None
    first = shard is None or shard[0] == 1  # the ignored tests and the doc-tests are reported by shard 1 only
    a.no_doc = a.no_doc or not first
    skip_re = re.compile(a.skip_pattern) if a.skip_pattern else None
    show = ["--show-output"] if skip_re else []
    table = read_limits(a.limits) if a.limits else None

    t_build = time.time()
    progress(f"gate_test: building test binaries ({' '.join(select)}, release)")
    build = subprocess.run(
        ["cargo", "test", *select, "--release", "--no-fail-fast", "--no-run", "--message-format=json-render-diagnostics"],
        stdout=subprocess.PIPE,
    )
    bins = []
    for line in build.stdout.decode("utf-8", "replace").splitlines():
        m = json.loads(line) if line.startswith("{") else {}
        if m.get("reason") == "compiler-artifact" and m["profile"]["test"] and m.get("executable"):
            bins.append(m)
    progress(f"gate_test: build finished in {hms(time.time() - t_build)}, {len(bins)} test binaries (exit {build.returncode})")
    if build.returncode != 0:
        print(f"gate_test: build failed (exit {build.returncode})")
        return build.returncode or 101

    env = dict(os.environ)
    if bins:
        env["PATH"] = os.path.dirname(bins[0]["executable"]) + os.pathsep + env.get("PATH", "")

    def listing(b, *extra):
        cwd = os.path.dirname(b["manifest_path"])
        code, out, _, _, _ = run([b["executable"], "--list", *extra], cwd, dict(env, CARGO_MANIFEST_DIR=cwd))
        if code != 0:
            raise SystemExit(f"gate_test: listing failed for {b['executable']}:\n{out}")
        return [ln[: -len(": test")] for ln in out.splitlines() if ln.endswith(": test")]

    progress(f"gate_test: listing the tests of {len(bins)} binaries")
    with cf.ThreadPoolExecutor(a.jobs) as ex:
        alls = list(ex.map(listing, bins))
        ign = list(ex.map(lambda b: set(listing(b, "--ignored")), bins))

    prev = {}
    if a.order and os.path.exists(a.order):
        for ln in open(a.order, encoding="utf-8").read().splitlines()[1:]:
            f = ln.split("\t")
            try:
                prev[(f[0], f[1])] = float(f[3])
            except (IndexError, ValueError):
                pass
    stem = [os.path.basename(b["executable"]).rsplit("-", 1)[0] for b in bins]
    jobs = [(i, t) for i, names in enumerate(alls) for t in names if t not in ign[i]]
    jobs.sort(key=lambda j: -prev.get((stem[j[0]], j[1]), 0.0))
    if shard:
        keys = [(stem[i], bins[i]["manifest_path"].replace("\\", "/"), t) for i, t in jobs]
        order, part, digest = shard_split(keys, [prev.get((stem[i], t)) for i, t in jobs], shard[1])
        total = len(jobs)
        jobs = [jobs[k] for k, sh in zip(order, part) if sh == shard[0] - 1]
        line = f"gate_test: shard {shard[0]}/{shard[1]} runs {len(jobs)} of {total} tests, split {digest}"
        progress(line)
        print(line, flush=True)
    if table is not None:
        named = sum((stem[i], t) in table for i, t in jobs)
        line = f"gate_test: limits {a.limits}: {len(table)} rows, {named} name a test of this run"
        progress(line)
        print(line, flush=True)
    n_ign = sum(len(x) for x in ign) if first else 0
    progress(f"gate_test: {len(jobs)} tests to run ({n_ign} ignored) in {len(bins)} binaries, jobs={a.jobs}"
             + (f", order from {a.order}" if prev else ""))
    lock = threading.Lock()
    state = {"done": 0, "failed": 0, "busy": 0.0}
    running = {}

    def one(job):
        i, t = job
        b = bins[i]
        cwd = os.path.dirname(b["manifest_path"])
        lim = test_limit((stem[i], t), prev.get((stem[i], t)), a.timeout_factor, a.timeout_min, a.timeout_max, table)
        with lock:
            running[job] = time.time()
        code, out, dt, mb, to = run([b["executable"], t, "--exact", "--test-threads=1", *show], cwd, dict(env, CARGO_MANIFEST_DIR=cwd),
                                    lim)
        r = (RESULT.findall(out) or [None])[-1] if skip_re else RESULT.search(out)
        r = r if r is None or isinstance(r, tuple) else r.groups()
        ok = "TIMEOUT" if to else "ok" if code == 0 and r == ("1", "0", "0") else "FAILED"
        with lock:
            running.pop(job, None)
            state["done"] += 1
            state["failed"] += ok != "ok"
            state["busy"] += dt
            k = state["done"]
        progress(f"[{k}/{len(jobs)} {ok} {dt:7.1f}s {mb:6.0f}MB {hms(time.time() - t0)}] {stem[i]}::{t}")
        return job, ok, out, dt, mb, lim

    t0 = time.time()
    stop, mem = threading.Event(), [0.0]
    threading.Thread(target=sample_memory, args=(stop, mem), daemon=True).start()

    def heartbeat():
        while not stop.wait(a.heartbeat):
            now = time.time()
            with lock:
                done, failed, busy = state["done"], state["failed"], state["busy"]
                live = sorted(((now - st, f"{stem[j[0]]}::{j[1]}") for j, st in running.items()), reverse=True)
            el = now - t0
            # estimate from the average process seconds per finished test, spread over the pool
            left = (len(jobs) - done) * (busy / done) / a.jobs if done else 0.0
            progress(f"gate_test: heartbeat {done}/{len(jobs)} done ({100.0 * done / max(len(jobs), 1):.1f}%), {failed} failed, "
                     f"elapsed {hms(el)}, about {hms(left)} left, summed working set {mem[0]:.0f} MB peak; running: "
                     + ("; ".join(f"{n} {hms(d)}" for d, n in live[:a.jobs]) or "none"))

    if a.heartbeat > 0:
        threading.Thread(target=heartbeat, daemon=True).start()
    with cf.ThreadPoolExecutor(a.jobs) as ex:
        res = {j: (ok, out, dt, mb, lim) for j, ok, out, dt, mb, lim in ex.map(one, jobs)}
    stop.set()
    wall = time.time() - t0
    ratio = max([dt / lim for _, _, dt, _, lim in res.values() if lim] or [0.0])

    fails = 0
    rows = ["binary\ttest\toutcome\tseconds\tpeak_mb" + ("\tskipped" if skip_re else "")]
    n_skip = 0
    for i, b in enumerate(bins):
        rel = os.path.relpath(b["target"]["src_path"], os.path.dirname(b["manifest_path"]))
        kind = "unittests " if b["target"]["kind"] != ["test"] else ""
        print(f"     Running {kind}{rel} ({b['executable']})\n\nrunning {len(alls[i])} tests")
        p = f = 0
        secs = 0.0
        for t in alls[i]:
            if t in ign[i]:
                if not first:
                    continue
                print(f"test {t} ... ignored")
                rows.append(tsv_row(stem[i], t, "ignored", "0", "0", [] if skip_re else None))
                continue
            if (i, t) not in res:
                continue  # a test of another shard
            outcome, out, dt, mb, lim = res[(i, t)]
            ok = outcome == "ok"
            secs += dt
            print(f"test {t} ... {'ok' if ok else 'FAILED'}" + (f" (timeout after {lim:.0f} s)" if outcome == "TIMEOUT" else ""))
            sk = (skip_lines(out, skip_re) if ok else []) if skip_re else None
            n_skip += bool(sk)
            rows.append(tsv_row(stem[i], t, outcome, f"{dt:.2f}", f"{mb:.0f}", sk))
            p, f = p + ok, f + (not ok)
            if not ok:
                # indented so the gate's `^test result` sum does not count the inner summary a second time
                print(f"---- {t} output ----\n" + "".join("    " + ln + "\n" for ln in out.splitlines()))
        fails += f
        status = "ok" if f == 0 else "FAILED"
        print(f"\ntest result: {status}. {p} passed; {f} failed; {len(ign[i]) if first else 0} ignored; 0 measured; 0 filtered out; "
              f"finished in {secs:.2f}s (process seconds)\n")
    print(f"gate_test: {len(jobs)} test processes, {len(bins)} binaries, jobs={a.jobs}, wall {wall:.1f}s, "
          f"peak summed working set {mem[0]:.0f} MB, max time/limit {ratio:.3f}"
          + (f", {n_skip} passed tests reported a skip" if skip_re else ""))
    if a.tsv:
        open(a.tsv, "w", encoding="utf-8").write("\n".join(rows) + "\n")

    sys.stdout.flush()
    if a.no_doc:
        progress(f"gate_test: tests finished in {hms(wall)}, {fails} failed; doc-tests skipped (--no-doc)")
        return 101 if fails else 0
    progress(f"gate_test: tests finished in {hms(wall)}, {fails} failed; running the doc-tests")
    t_doc = time.time()
    doc = subprocess.run(["cargo", "test", *select, "--release", "--no-fail-fast", "--doc"])
    progress(f"gate_test: doc-tests finished in {hms(time.time() - t_doc)} (exit {doc.returncode})")
    return 101 if fails or doc.returncode else 0


if __name__ == "__main__":
    sys.exit(main())
