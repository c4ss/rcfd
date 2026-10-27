#!/usr/bin/env python3
"""Release test stage of the gate (ADR 0127): the same tests as `cargo test --workspace --release --no-fail-fast`, run as one
process per test from a single pool, so tests of different binaries overlap instead of waiting for the slowest test of each binary.

Usage (from the workspace root):
    python scripts/gate_test.py [--jobs N] [--order prev.tsv] [--tsv out.tsv] [cargo package selection, default --workspace]
--jobs     concurrent test processes (default RUST_TEST_THREADS, else 3): the same number of tests at once as one cargo test binary.
--order    a TSV written by an earlier run: start its slowest tests first. Ordering only; every test still runs.
--tsv      write binary, test, outcome, seconds, peak working set (MB) for every test.
--timeout-factor F, --timeout-min S, --timeout-max S (ADR 0129; defaults 5, 600, 3600): a test's time limit is
           clamp(F * its seconds in the --order TSV, min, max), max if it has no record; --timeout-max 0 turns the limit off. On expiry
           the test's process tree (never cargo) is killed and the test fails with outcome TIMEOUT; the other tests go on.
Output keeps the libtest lines (`Running`, `test X ... ok`, `test result:`), so the gate's summing awk works unchanged. Afterwards
`cargo test --doc` runs the doc-tests. Exit status 0 only if the build, every test and the doc-tests pass.
"""
import argparse
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


def limit_s(rec, factor, lo, hi):
    """Time limit of one test (ADR 0129): clamp(factor * recorded seconds, lo, hi); hi without a record; None if hi <= 0 (off)."""
    if hi <= 0:
        return None
    if rec is None:
        return hi
    return min(max(factor * rec, lo), hi)


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
    ap.add_argument("--timeout-factor", type=float, default=5.0)
    ap.add_argument("--timeout-min", type=float, default=600.0)
    ap.add_argument("--timeout-max", type=float, default=3600.0)
    ap.add_argument("select", nargs="*")
    a = ap.parse_args()
    select = a.select or ["--workspace"]

    build = subprocess.run(
        ["cargo", "test", *select, "--release", "--no-fail-fast", "--no-run", "--message-format=json-render-diagnostics"],
        stdout=subprocess.PIPE,
    )
    bins = []
    for line in build.stdout.decode("utf-8", "replace").splitlines():
        m = json.loads(line) if line.startswith("{") else {}
        if m.get("reason") == "compiler-artifact" and m["profile"]["test"] and m.get("executable"):
            bins.append(m)
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

    def one(job):
        i, t = job
        b = bins[i]
        cwd = os.path.dirname(b["manifest_path"])
        lim = limit_s(prev.get((stem[i], t)), a.timeout_factor, a.timeout_min, a.timeout_max)
        code, out, dt, mb, to = run([b["executable"], t, "--exact", "--test-threads=1"], cwd, dict(env, CARGO_MANIFEST_DIR=cwd),
                                    lim)
        r = RESULT.search(out)
        ok = "TIMEOUT" if to else "ok" if code == 0 and r is not None and r.groups() == ("1", "0", "0") else "FAILED"
        print(f"[{ok} {dt:7.1f}s {mb:6.0f}MB] {stem[i]}::{t}", file=sys.stderr, flush=True)
        return job, ok, out, dt, mb, lim

    t0 = time.time()
    stop, mem = threading.Event(), [0.0]
    threading.Thread(target=sample_memory, args=(stop, mem), daemon=True).start()
    with cf.ThreadPoolExecutor(a.jobs) as ex:
        res = {j: (ok, out, dt, mb, lim) for j, ok, out, dt, mb, lim in ex.map(one, jobs)}
    stop.set()
    wall = time.time() - t0
    ratio = max([dt / lim for _, _, dt, _, lim in res.values() if lim] or [0.0])

    fails = 0
    rows = ["binary\ttest\toutcome\tseconds\tpeak_mb"]
    for i, b in enumerate(bins):
        rel = os.path.relpath(b["target"]["src_path"], os.path.dirname(b["manifest_path"]))
        kind = "unittests " if b["target"]["kind"] != ["test"] else ""
        print(f"     Running {kind}{rel} ({b['executable']})\n\nrunning {len(alls[i])} tests")
        p = f = 0
        secs = 0.0
        for t in alls[i]:
            if t in ign[i]:
                print(f"test {t} ... ignored")
                rows.append(f"{stem[i]}\t{t}\tignored\t0\t0")
                continue
            outcome, out, dt, mb, lim = res[(i, t)]
            ok = outcome == "ok"
            secs += dt
            print(f"test {t} ... {'ok' if ok else 'FAILED'}" + (f" (timeout after {lim:.0f} s)" if outcome == "TIMEOUT" else ""))
            rows.append(f"{stem[i]}\t{t}\t{outcome}\t{dt:.2f}\t{mb:.0f}")
            p, f = p + ok, f + (not ok)
            if not ok:
                # indented so the gate's `^test result` sum does not count the inner summary a second time
                print(f"---- {t} output ----\n" + "".join("    " + ln + "\n" for ln in out.splitlines()))
        fails += f
        status = "ok" if f == 0 else "FAILED"
        print(f"\ntest result: {status}. {p} passed; {f} failed; {len(ign[i])} ignored; 0 measured; 0 filtered out; "
              f"finished in {secs:.2f}s (process seconds)\n")
    print(f"gate_test: {len(jobs)} test processes, {len(bins)} binaries, jobs={a.jobs}, wall {wall:.1f}s, "
          f"peak summed working set {mem[0]:.0f} MB, max time/limit {ratio:.3f}")
    if a.tsv:
        open(a.tsv, "w", encoding="utf-8").write("\n".join(rows) + "\n")

    sys.stdout.flush()
    doc = subprocess.run(["cargo", "test", *select, "--release", "--no-fail-fast", "--doc"])
    return 101 if fails or doc.returncode else 0


if __name__ == "__main__":
    sys.exit(main())
