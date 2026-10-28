"""Build and check the finished example projects in example-projects/ (ADR 0142).

    python scripts/make_example_projects.py                  rebuild every project: mesh recipe, runs, ledger, run table
    python scripts/make_example_projects.py --project NAME   rebuild one project (wind-tunnel or quick-start)
    python scripts/make_example_projects.py --check          check the committed examples (no solver run, no binary needed)
    python scripts/make_example_projects.py --rerun-check    rebuild every project in a temporary copy and compare the
                                                             files that are byte-reproducible with the committed ones

A rebuild runs the real solver through the real run service: it starts `rcfd-runner serve --project <dir>` itself (output
to a file, never a pipe), queues each case with `rcfd-runner start <case> --no-runner`, waits for each run, shuts the
runner down and removes the runtime state (`.rcfd/` except its `.gitignore`) and the bulky solver files that the folder
contract does not list (`solver-status.jsonl`, `output/solution.vtu`, checkpoints). It needs `rcfd` and `rcfd-runner`
from `cargo build --release -p rcfd-cli -p rcfd-runner` (or `--bin-dir`). Case files, notes and the mesh recipe are
inputs and are never rewritten; the manifest `rcfd-project.json` is written once and kept.

Byte-reproducible on the same machine and binaries (checked by --rerun-check): case.snapshot, residuals.jsonl,
fields/store.rts, fields/index.json, output/monitors.json and the polyMesh of the aerofoil mesh. Not reproducible: run
ids, every time stamp and pid (run.json, status.json, events.jsonl, metrics.json `solver_finish`, the ledger's
`recorded_at_ms` and `wall_s`), the absolute paths that run.json and the solver events record, log.txt and report.html.
"""

import argparse
import json
import math
import os
import re
import shutil
import struct
import subprocess
import sys
import tempfile
import time

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
EXAMPLES = os.path.join(REPO, "example-projects")
EXE = ".exe" if os.name == "nt" else ""

# Size caps registered in ADR 0142 (bytes, every file of the project folder counted, `.rcfd/` included).
CAP_TOTAL = 40_000_000
CAP_PROJECT = {"wind-tunnel": 35_000_000, "quick-start": 6_000_000}
MAX_SNAPSHOTS = 100

# Files of a run folder that a rebuild removes (not in the folder contract; bulky or duplicated).
PRUNE_RUN_FILES = ["solver-status.jsonl", "pause", "pause.ack", "cancel"]
PRUNE_OUTPUT_SUFFIXES = (".vtu", ".rcfd")

# Files of a run folder that are byte-reproducible on the same machine and binaries.
REPRODUCIBLE = ["case.snapshot", "residuals.jsonl", "fields/store.rts", "fields/index.json", "output/monitors.json"]

RUNS_BEGIN = "<!-- generated: runs (scripts/make_example_projects.py) -->"
RUNS_END = "<!-- end of generated runs -->"

# ------------------------------------------------------------------------------------------------ project specs


def _cyl(re_, case):
    return {"case": case, "study": "cylinder-reynolds", "params": [["Re", float(re_), "1"]], "kind": "shedding"}


def _naca(alpha, case):
    return {"case": case, "study": "naca0012-angle-of-attack", "params": [["alpha", float(alpha), "deg"]],
            "kind": "steady", "alpha": float(alpha)}


def _step(re_, case):
    return {"case": case, "study": "step-reynolds", "params": [["Re", float(re_), "1"]], "kind": "step",
            "box": {"nx": 200, "ny": 40, "lx": 15.0, "ly": 1.0}}


PROJECTS = {
    "wind-tunnel": {
        "name": "Wind tunnel: cylinder, NACA 0012 aerofoil and backward-facing step",
        "meshes": ["naca0012-ogrid"],
        "runs": [
            _cyl(100, "cases/cylinder-re100.toml"),
            _cyl(200, "cases/cylinder-re200.toml"),
            _naca(4, "cases/naca0012-aoa4.toml"),
            _naca(2, "cases/naca0012-aoa2.toml"),
            _step(800, "cases/step-re800.toml"),
            _step(400, "cases/step-re400.toml"),
        ],
    },
    "quick-start": {
        "name": "Quick start: lid-driven cavity and heated cavity",
        "meshes": [],
        "runs": [
            {"case": "cases/cavity-re100.toml", "study": "cavity-reynolds", "params": [["Re", 100.0, "1"]],
             "kind": "steady"},
            {"case": "cases/cavity-re400.toml", "study": "cavity-reynolds", "params": [["Re", 400.0, "1"]],
             "kind": "steady"},
            {"case": "cases/heated-cavity-ra1e3.toml", "study": "heated-cavity-rayleigh",
             "params": [["Ra", 1000.0, "1"]], "kind": "steady"},
            {"case": "cases/heated-cavity-ra1e4.toml", "study": "heated-cavity-rayleigh",
             "params": [["Ra", 10000.0, "1"]], "kind": "steady"},
        ],
    },
}

# Units of the report values the ledger copies from metrics.json (reports not listed are copied with unit "1").
REPORT_UNITS = {"lid_drag": "N", "q_hot": "W"}

# ------------------------------------------------------------------------------------------------ small helpers


def fnv64(data):
    h = 0xCBF29CE484222325
    for b in data:
        h ^= b
        h = (h * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return h


def hash_bytes(data):
    return "fnv64:%016x" % fnv64(data)


def hash_matches(recorded, data):
    want = hash_bytes(data)
    if recorded.startswith("fnv1a64:"):
        return want[len("fnv64:"):] == recorded[len("fnv1a64:"):]
    return recorded == want


def read(path):
    with open(path, "rb") as f:
        return f.read()


def read_json(path):
    with open(path, "rb") as f:
        return json.loads(f.read().decode("utf-8"))


def write_text(path, text):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        f.write(text)


def jsonl(path):
    out = []
    if not os.path.exists(path):
        return out
    for line in read(path).decode("utf-8").split("\n"):
        if line.strip():
            out.append(json.loads(line))
    return out


def tree_size(root):
    total = 0
    for d, _, files in os.walk(root):
        for f in files:
            total += os.path.getsize(os.path.join(d, f))
    return total


def rel(root, path):
    return os.path.relpath(path, root).replace("\\", "/")


def rfc3339_ms(t):
    """'2026-10-28T14:00:00.123456789Z' (any fraction) -> milliseconds since the Unix epoch."""
    m = re.match(r"(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})(?:\.(\d+))?Z$", t)
    if not m:
        raise ValueError("not an RFC 3339 UTC time: %r" % t)
    y, mo, d, h, mi, s = (int(m.group(i)) for i in range(1, 7))
    frac = (m.group(7) or "0")[:9].ljust(9, "0")
    # days from civil (Howard Hinnant's algorithm)
    y2 = y - (1 if mo <= 2 else 0)
    era = (y2 if y2 >= 0 else y2 - 399) // 400
    yoe = y2 - era * 400
    doy = (153 * (mo + (-3 if mo > 2 else 9)) + 2) // 5 + d - 1
    doe = yoe * 365 + yoe // 4 - yoe // 100 + doy
    days = era * 146097 + doe - 719468
    return ((days * 86400 + h * 3600 + mi * 60 + s) * 1000) + int(frac) // 1_000_000


def event_kind(ev):
    return ev.get("kind") or ev.get("event")


# ------------------------------------------------------------------------------------------------ mesh recipe


def naca_thickness(x):
    """Closed-trailing-edge NACA 4-digit thickness, t = 0.12 (ADR 0092, cases/naca0012/reference/naca4_thickness.txt)."""
    return 0.6 * (0.2969 * math.sqrt(x) - 0.1260 * x - 0.3516 * x * x + 0.2843 * x ** 3 - 0.1036 * x ** 4)


def naca_warp(p, r_in=1.0, r_out=2.0, far=20.0):
    """ADR 0092 (crates/rcfd-api/tests/registry_setups_w39a.rs, naca0012_mesh): the inner ring of the cylinder O-grid goes
    onto the aerofoil at x = (1 + cos theta) / 2, the outer ring onto a circle of radius `far` about mid-chord, points in
    between are blended linearly in the generator's graded radial coordinate."""
    x, y, z = p
    th = math.atan2(y, x)
    s = min(max((math.hypot(x, y) - r_in) / (r_out - r_in), 0.0), 1.0)
    xa = 0.5 * (1.0 + math.cos(th))
    ya = max(naca_thickness(xa), 0.0) * math.copysign(1.0, math.sin(th))
    xo, yo = 0.5 + far * math.cos(th), far * math.sin(th)
    return ((1.0 - s) * xa + s * xo, (1.0 - s) * ya + s * yo, z)


def fmt_float(v):
    r = repr(float(v))
    return r[:-2] if r.endswith(".0") else r


def warp_polymesh(src, dst):
    """Copy the polyMesh in `src` to `dst` with every point warped and the patch `cylinder` renamed `airfoil`."""
    os.makedirs(dst, exist_ok=True)
    for f in ["faces", "owner", "neighbour"]:
        shutil.copyfile(os.path.join(src, f), os.path.join(dst, f))
    text = read(os.path.join(src, "boundary")).decode("utf-8")
    if text.count("    cylinder\n") != 1:
        raise RuntimeError("boundary of the O-grid recipe has no single 'cylinder' patch")
    write_text(os.path.join(dst, "boundary"), text.replace("    cylinder\n", "    airfoil\n"))
    lines = read(os.path.join(src, "points")).decode("utf-8").split("\n")
    i = 0
    out = []
    while not lines[i].strip().isdigit():
        out.append(lines[i])
        i += 1
    n = int(lines[i])
    out += [lines[i], lines[i + 1]]
    if lines[i + 1] != "(":
        raise RuntimeError("unexpected points file layout")
    i += 2
    for k in range(n):
        q = naca_warp(tuple(float(a) for a in lines[i + k].strip("()").split()))
        out.append("(" + " ".join(fmt_float(c) for c in q) + ")")
    out += lines[i + n:]
    write_text(os.path.join(dst, "points"), "\n".join(out))
    return n


def build_mesh_recipes(project_dir, bins, dest_root=None):
    """Rebuild every mesh of the project from its recipe in meshes/<name>.recipe.toml."""
    dest_root = dest_root or project_dir
    built = []
    for name in PROJECTS[os.path.basename(project_dir)]["meshes"]:
        recipe = os.path.join(project_dir, "meshes", name + ".recipe.toml")
        with tempfile.TemporaryDirectory(prefix="rcfd_w48h_mesh_") as tmp:
            r = subprocess.run([bins["rcfd"], "convert", recipe, tmp], capture_output=True, text=True)
            if r.returncode != 0:
                raise RuntimeError("rcfd convert %s failed: %s" % (recipe, r.stderr))
            dst = os.path.join(dest_root, "meshes", name)
            if os.path.isdir(dst):
                shutil.rmtree(dst)
            n = warp_polymesh(os.path.join(tmp, "polyMesh"), dst)
        built.append((name, n))
    return built


# ------------------------------------------------------------------------------------------------ field store (minimal)


def store_snapshots(store_path):
    """(mesh n_cells, [(step, time, {field: (precision, codec, n, block_values, [offsets])})]) of a finished store, read
    from its footer (rcfd-tsstore format, crates/rcfd-tsstore/src/lib.rs). Checksums are left to the Rust reader."""
    b = read(store_path)
    if b[:8] != b"RCFDTSS\0":
        raise RuntimeError("%s: not a field store" % store_path)
    mesh_len = struct.unpack_from("<Q", b, 20 + 4)[0]
    n_cells = struct.unpack_from("<Q", b, 20 + 28 + 8)[0]
    foot_off = struct.unpack_from("<Q", b, len(b) - 20)[0]
    if b[len(b) - 12:len(b) - 8] != b"RTSE" or b[foot_off:foot_off + 4] != b"FOOT":
        raise RuntimeError("%s: no footer (an unfinished store)" % store_path)
    flen = struct.unpack_from("<Q", b, foot_off + 4)[0]
    foot = json.loads(b[foot_off + 28:foot_off + 28 + flen].decode("utf-8"))
    snaps = []
    for step, t, off, _end in foot["snapshots"]:
        slen = struct.unpack_from("<Q", b, off + 4)[0]
        d = json.loads(b[off + 28:off + 28 + slen].decode("utf-8"))
        snaps.append((step, t, {f["name"]: f for f in d["fields"]}))
    del mesh_len
    return b, n_cells, snaps


def store_field(store_path, snapshot, name):
    b, _, snaps = store_snapshots(store_path)
    f = snaps[snapshot][2][name]
    if f["codec"] != "raw":
        raise RuntimeError("only raw blocks are read here")
    width, code = (4, "<%df") if f["precision"] == "f32" else (8, "<%dd")
    out = []
    left = f["n"]
    for off in f["blocks"]:
        count = min(f["block_values"], left)
        out.extend(struct.unpack_from(code % count, b, off + 28))
        left -= count
    return out


def sign_changes(xs, us):
    """x of every sign change of u along a row (linear interpolation), as the repository's laminar step test reads it
    (crates/rcfd-fv/tests/step_laminar.rs `reattachment`)."""
    out = []
    for i in range(1, len(xs)):
        a, b = us[i - 1], us[i]
        if (a < 0.0 <= b) or (a >= 0.0 > b):
            w = -a / (b - a)
            out.append(xs[i - 1] + w * (xs[i] - xs[i - 1]))
    return out


def step_reattachment(run_dir, box):
    """Floor reattachment and upper-wall bubble of a box_2d step run from the last stored snapshot of ux (cell c is
    (i, j) = (c mod nx, c div nx), the ordering of the box_2d generator; the Rust staleness test checks it against the
    stored mesh's cell centres)."""
    store = os.path.join(run_dir, "fields", "store.rts")
    _, n_cells, snaps = store_snapshots(store)
    nx, ny, lx = box["nx"], box["ny"], box["lx"]
    if n_cells != nx * ny:
        raise RuntimeError("store has %d cells, the step box %d" % (n_cells, nx * ny))
    ux = store_field(store, len(snaps) - 1, "ux")
    dx = lx / nx
    xs = [(i + 0.5) * dx for i in range(nx)]
    floor = sign_changes(xs, ux[:nx])
    top = sign_changes(xs, ux[(ny - 1) * nx:])
    return floor, top


# ------------------------------------------------------------------------------------------------ monitors


def shedding(monitors_path):
    """Strouhal number (D = U = 1), mean C_D and the C_L amplitude over all whole periods of C_L between the first and
    the last upward crossing of C_L through its mean in the second half of the run (ADR 0142 amendment 1)."""
    m = read_json(monitors_path)["series"]
    t, cl, cd = m["cl"]["x"], m["cl"]["y"], m["cd"]["y"]
    half = t[-1] / 2.0
    idx = [i for i in range(len(t)) if t[i] >= half]
    mean = sum(cl[i] for i in idx) / len(idx)
    cross = []
    for a, b in zip(idx, idx[1:]):
        if cl[a] - mean < 0.0 <= cl[b] - mean:
            w = (mean - cl[a]) / (cl[b] - cl[a])
            cross.append(t[a] + w * (t[b] - t[a]))
    if len(cross) < 2:
        raise RuntimeError("%s: %d upward crossings in the second half, no whole period" % (monitors_path, len(cross)))
    periods = len(cross) - 1
    ta, tb = cross[0], cross[-1]
    win = [i for i in idx if ta <= t[i] <= tb]
    num = 0.0
    for a, b in zip(win, win[1:]):
        num += 0.5 * (cd[a] + cd[b]) * (t[b] - t[a])
    return {
        "strouhal": periods / (tb - ta),
        "cd_mean": num / (t[win[-1]] - t[win[0]]),
        "cl_amplitude": 0.5 * (max(cl[i] for i in win) - min(cl[i] for i in win)),
        "periods": periods,
        "window_start": ta,
        "window_end": tb,
    }


# ------------------------------------------------------------------------------------------------ the run service


class Runner:
    def __init__(self, project_dir, bins, threads):
        self.dir = os.path.abspath(project_dir)
        self.bins = bins
        self.threads = threads
        self.proc = None
        self.log = None

    def _cli(self, *args, check=True):
        env = dict(os.environ, RCFD_BIN=self.bins["rcfd"])
        r = subprocess.run([self.bins["rcfd-runner"], *args, "--project", self.dir], capture_output=True, text=True,
                           env=env, stdin=subprocess.DEVNULL)
        if check and r.returncode != 0:
            raise RuntimeError("rcfd-runner %s: exit %d: %s %s" % (" ".join(args), r.returncode, r.stdout, r.stderr))
        return r

    def start(self):
        dot = os.path.join(self.dir, ".rcfd")
        os.makedirs(dot, exist_ok=True)
        cfg = {"max_runs": 1, "cpu_threads": self.threads, "poll_ms": 100}
        write_text(os.path.join(dot, "runner-config.json"), json.dumps(cfg, indent=2) + "\n")
        self.log = open(os.path.join(dot, "serve-output.txt"), "wb")
        env = dict(os.environ, RCFD_BIN=self.bins["rcfd"])
        self.proc = subprocess.Popen([self.bins["rcfd-runner"], "serve", "--project", self.dir], stdout=self.log,
                                     stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL, env=env)
        t0 = time.time()
        while not os.path.exists(os.path.join(dot, "runner.json")):
            if self.proc.poll() is not None:
                raise RuntimeError("rcfd-runner serve exited with %s" % self.proc.returncode)
            if time.time() - t0 > 30:
                raise RuntimeError("rcfd-runner serve did not start within 30 s")
            time.sleep(0.05)

    def submit(self, case_abs):
        r = self._cli("start", case_abs, "--threads", str(self.threads), "--no-runner")
        return r.stdout.strip().splitlines()[-1]

    def wait(self, run_id, timeout):
        r = self._cli("wait", run_id, "--timeout", str(timeout), check=False)
        return r.returncode, r.stdout

    def stop(self):
        if self.proc is not None:
            self._cli("shutdown", check=False)
            try:
                self.proc.wait(timeout=60)
            except subprocess.TimeoutExpired:
                self.proc.kill()
                self.proc.wait()
            self.proc = None
        if self.log is not None:
            self.log.close()
            self.log = None


def clean_dot(project_dir):
    dot = os.path.join(project_dir, ".rcfd")
    os.makedirs(dot, exist_ok=True)
    for name in os.listdir(dot):
        if name == ".gitignore":
            continue
        p = os.path.join(dot, name)
        shutil.rmtree(p) if os.path.isdir(p) else os.remove(p)
    write_text(os.path.join(dot, ".gitignore"),
               "# Runtime state of the run service and the Studio (lock, logs, requests): never versioned.\n*\n"
               "!.gitignore\n")


def prune_run(run_dir):
    removed = []
    for f in PRUNE_RUN_FILES:
        p = os.path.join(run_dir, f)
        if os.path.exists(p):
            removed.append((f, os.path.getsize(p)))
            os.remove(p)
    out = os.path.join(run_dir, "output")
    if os.path.isdir(out):
        for f in sorted(os.listdir(out)):
            if f.endswith(PRUNE_OUTPUT_SUFFIXES):
                p = os.path.join(out, f)
                removed.append(("output/" + f, os.path.getsize(p)))
                os.remove(p)
    return removed


def run_wall_s(run_dir):
    """Wall time of a run from its own events: the launched event to the final state event."""
    evs = jsonl(os.path.join(run_dir, "events.jsonl"))
    t0 = next((rfc3339_ms(e["t"]) for e in evs if event_kind(e) == "launched"), None)
    t1 = next((rfc3339_ms(e["t"]) for e in reversed(evs) if event_kind(e) == "state"
               and (e.get("to") or e.get("state")) in ("finished", "failed", "cancelled")), None)
    return None if t0 is None or t1 is None else (t1 - t0) / 1000.0, t1


def ensure_project(project_dir, name, bins):
    for d in ["cases", "meshes", "runs", "designs", "notes", ".rcfd"]:
        os.makedirs(os.path.join(project_dir, d), exist_ok=True)
    man = os.path.join(project_dir, "rcfd-project.json")
    if not os.path.exists(man):
        version = subprocess.run([bins["rcfd"], "--version"], capture_output=True, text=True).stdout.strip()
        now = time.strftime("%Y-%m-%dT%H:%M:%S", time.gmtime()) + ".000Z"
        write_text(man, json.dumps({"schema_version": 1, "name": name, "created": now,
                                    "solver_version_created": version}, indent=2) + "\n")


def build_project(pname, bins, threads, root=EXAMPLES):
    spec = PROJECTS[pname]
    pdir = os.path.join(root, pname)
    ensure_project(pdir, spec["name"], bins)
    meshes = build_mesh_recipes(pdir, bins)
    runs_dir = os.path.join(pdir, "runs")
    for name in os.listdir(runs_dir):
        shutil.rmtree(os.path.join(runs_dir, name))
    clean_dot(pdir)
    runner = Runner(pdir, bins, threads)
    runner.start()
    ids = []
    try:
        for r in spec["runs"]:
            ids.append(runner.submit(os.path.join(pdir, r["case"].replace("/", os.sep))))
        results = []
        for r, rid in zip(spec["runs"], ids):
            code, out = runner.wait(rid, 3600)
            st = read_json(os.path.join(runs_dir, rid, "status.json"))
            print("  %s %s: %s, step %d, %s" % (rid, r["case"], st["state"], st["step"], st.get("reason", "")))
            results.append((rid, code))
    finally:
        runner.stop()
    clean_dot(pdir)
    pruned = {rid: prune_run(os.path.join(runs_dir, rid)) for rid in ids}
    ledger_lines, table = summarise(pdir, spec, ids)
    write_text(os.path.join(pdir, "designs", "variants.ledger.jsonl"), "".join(l + "\n" for l in ledger_lines))
    update_run_table(os.path.join(pdir, "notes", "README.md"), table)
    return {"project": pname, "runs": ids, "meshes": meshes, "pruned": pruned}


def summarise_existing(pname, root=EXAMPLES):
    """Write the ledger and the run table of a project from its existing runs (no solver run; ADR 0142 amendment 1)."""
    spec = PROJECTS[pname]
    pdir = os.path.join(root, pname)
    ids = []
    for r in spec["runs"]:
        rid = run_of_case(pdir, r["case"])
        if rid is None:
            raise RuntimeError("%s: no run of %s to summarise" % (pname, r["case"]))
        ids.append(rid)
    ledger_lines, table = summarise(pdir, spec, ids)
    write_text(os.path.join(pdir, "designs", "variants.ledger.jsonl"), "".join(l + "\n" for l in ledger_lines))
    update_run_table(os.path.join(pdir, "notes", "README.md"), table)
    return ids


# ------------------------------------------------------------------------------------------------ ledger and run table


def metric_list(spec, run_dir):
    """Metrics of one run for the ledger: the final report values of metrics.json, plus the derived values of the
    problem (shedding statistics, reattachment)."""
    met = read_json(os.path.join(run_dir, "metrics.json"))
    out = []
    for k in sorted(met.get("reports", {})):
        v = met["reports"][k]
        out.append([k, v, REPORT_UNITS.get(k, "1")])
    if spec["kind"] == "shedding":
        s = shedding(os.path.join(run_dir, "output", "monitors.json"))
        out += [["strouhal", s["strouhal"], "1"], ["cd_mean", s["cd_mean"], "1"],
                ["cl_amplitude", s["cl_amplitude"], "1"], ["periods", float(s["periods"]), "1"]]
    if spec["kind"] == "step":
        floor, top = step_reattachment(run_dir, spec["box"])
        out.append(["x_reattach", floor[-1], "m"])
        if len(top) >= 2:
            out += [["x_top_separation", top[0], "m"], ["x_top_reattach", top[1], "m"]]
    return out


def summarise(pdir, spec, ids):
    lines = []
    rows = []
    for r, rid in zip(spec["runs"], ids):
        run_dir = os.path.join(pdir, "runs", rid)
        rec = read_json(os.path.join(run_dir, "run.json"))
        st = read_json(os.path.join(run_dir, "status.json"))
        met = read_json(os.path.join(run_dir, "metrics.json"))
        idx = read_json(os.path.join(run_dir, "fields", "index.json"))
        wall, t_end = run_wall_s(run_dir)
        metrics = metric_list(r, run_dir)
        fin = (met.get("solver_finish") or {}).get("state", "")
        entry = {
            "format": "rcfd-ledger-1", "kind": "evaluation", "study": r["study"],
            "variant": os.path.splitext(os.path.basename(r["case"]))[0],
            "params": [{"name": n, "value": v, "unit": u} for n, v, u in r["params"]],
            "evaluator": "rcfd-runner + rcfd run", "evaluator_version": rec["solver_version"],
            "solver_version": rec["solver_version"], "geometry_hash": "fnv64:" + idx["mesh_hash"],
            "settings": {"case": r["case"], "case_snapshot_hash": rec["case_snapshot_hash"], "run": rid,
                         "run_settings": rec["settings"]},
            "metrics": [{"name": n, "value": v, "unit": u} for n, v, u in metrics],
            "verdict": "ok", "reason": "", "fidelity": "cfd", "recorded_at_ms": t_end, "wall_s": wall, "seed": None,
            "provenance": {"driver": "scripts/make_example_projects.py", "adr": "0142", "project": os.path.basename(pdir),
                           "run": rid, "state": st["state"], "solver_state": fin, "reason": st.get("reason", "")},
        }
        lines.append(json.dumps(entry, sort_keys=True, separators=(",", ":"), ensure_ascii=False))
        rows.append((r, rid, st, wall, len(idx["snapshots"]), metrics, fin))
    return lines, rows


def fmt_metric(v):
    return "%.4g" % v


def update_run_table(readme, rows):
    head = ["| Case | Run | State | Steps | Wall time | Snapshots | Results (final values) |",
            "|---|---|---|---|---|---|---|"]
    body = []
    for r, rid, st, wall, nsnap, metrics, fin in rows:
        res = ", ".join("%s %s" % (n, fmt_metric(v)) for n, v, _ in metrics)
        body.append("| `%s` | `runs/%s` | %s (%s) | %d | %.1f s | %d | %s |" % (
            r["case"], rid, st["state"], fin or "-", st["step"], wall or float("nan"), nsnap, res))
    block = "\n".join([RUNS_BEGIN, ""] + head + body + ["", RUNS_END])
    text = read(readme).decode("utf-8")
    a, b = text.find(RUNS_BEGIN), text.find(RUNS_END)
    if a < 0 or b < 0:
        raise RuntimeError("%s has no generated run table markers" % readme)
    write_text(readme, text[:a] + block + text[b + len(RUNS_END):])


# ------------------------------------------------------------------------------------------------ manifest


def write_manifest(bins, root=EXAMPLES):
    version = subprocess.run([bins["rcfd"], "--version"], capture_output=True, text=True).stdout.strip()
    projects = {}
    for pname, spec in PROJECTS.items():
        pdir = os.path.join(root, pname)
        runs = []
        for r in spec["runs"]:
            rid = run_of_case(pdir, r["case"])
            runs.append({"case": r["case"], "run": rid})
        projects[pname] = {"runs": runs, "bytes": tree_size(pdir)}
    man = {"format": "rcfd-example-projects-1", "adr": "0142", "solver_version": version,
           "script": "scripts/make_example_projects.py", "projects": projects,
           "caps": {"total": CAP_TOTAL, "projects": CAP_PROJECT}}
    write_text(os.path.join(root, "manifest.json"), json.dumps(man, indent=2) + "\n")


def run_of_case(pdir, case):
    hits = []
    runs = os.path.join(pdir, "runs")
    for rid in sorted(os.listdir(runs)) if os.path.isdir(runs) else []:
        p = os.path.join(runs, rid, "run.json")
        if os.path.exists(p) and read_json(p).get("case") == case:
            hits.append(rid)
    return hits[-1] if hits else None


# ------------------------------------------------------------------------------------------------ check


BACKTICK = re.compile(r"`([^`\s]+)`")
PATH_PREFIXES = ("cases/", "meshes/", "runs/", "designs/", "notes/", "../../")


def readme_paths(text):
    """Project-relative paths that a notes file names in backticks (`cases/a.toml`, `runs/<id>/fields`)."""
    out = []
    for m in BACKTICK.finditer(text):
        p = m.group(1).rstrip(".,;:")
        if p.startswith(PATH_PREFIXES) and "<" not in p and "*" not in p:
            out.append(p)
    return out


def check(root=EXAMPLES, problems=None):
    """Problems of the committed examples (an empty list means fresh)."""
    problems = [] if problems is None else problems
    add = problems.append
    man_path = os.path.join(root, "manifest.json")
    if not os.path.exists(man_path):
        add("manifest.json: missing")
        return problems
    man = read_json(man_path)
    total = 0
    for pname, spec in PROJECTS.items():
        pdir = os.path.join(root, pname)
        if not os.path.isdir(pdir):
            add("%s: missing" % pname)
            continue
        listed = man["projects"].get(pname, {}).get("runs", [])
        for r in listed:
            if not r.get("run"):
                add("%s: manifest.json has no run of %s" % (pname, r.get("case")))
        expected = {r["run"]: r["case"] for r in listed if r.get("run")}
        try:
            pm = read_json(os.path.join(pdir, "rcfd-project.json"))
            if pm.get("schema_version") != 1:
                add("%s/rcfd-project.json: schema_version %r, expected 1" % (pname, pm.get("schema_version")))
        except Exception as e:  # noqa: BLE001 (reported, not raised)
            add("%s/rcfd-project.json: %s" % (pname, e))
        for d in ["cases", "meshes", "runs", "designs", "notes", ".rcfd"]:
            if not os.path.isdir(os.path.join(pdir, d)):
                add("%s/%s: missing directory" % (pname, d))
        dot = os.path.join(pdir, ".rcfd")
        if os.path.isdir(dot):
            extra = [n for n in os.listdir(dot) if n != ".gitignore"]
            if extra:
                add("%s/.rcfd: runtime files left: %s" % (pname, extra))
        cases = {r["case"] for r in spec["runs"]}
        if set(expected.values()) != cases or len(expected) != len(cases):
            add("%s: manifest lists runs of %s, the spec %s" % (pname, sorted(expected.values()), sorted(cases)))
        runs_dir = os.path.join(pdir, "runs")
        present = sorted(os.listdir(runs_dir)) if os.path.isdir(runs_dir) else []
        for rid in present:
            if rid not in expected:
                print("note: %s/runs/%s is not one of the committed example runs (ignored)" % (pname, rid))
        for rid, case in expected.items():
            check_run(pdir, pname, rid, case, add)
        size = sum(tree_size(os.path.join(pdir, d)) for d in os.listdir(pdir)
                   if os.path.isdir(os.path.join(pdir, d)) and d != "runs")
        size += sum(os.path.getsize(os.path.join(pdir, f)) for f in os.listdir(pdir)
                    if os.path.isfile(os.path.join(pdir, f)))
        size += sum(tree_size(os.path.join(runs_dir, rid)) for rid in expected if os.path.isdir(os.path.join(runs_dir, rid)))
        total += size
        if size > CAP_PROJECT[pname]:
            add("%s: %d bytes, above the cap %d" % (pname, size, CAP_PROJECT[pname]))
        for dirpath, _, files in os.walk(os.path.join(pdir, "notes")):
            for f in files:
                if not f.endswith(".md"):
                    continue
                text = read(os.path.join(dirpath, f)).decode("utf-8")
                for p in readme_paths(text):
                    if not os.path.exists(os.path.join(pdir, p)):
                        add("%s/notes/%s names `%s`, which does not exist" % (pname, f, p))
        check_ledger(pdir, pname, expected, add)
    total += sum(os.path.getsize(os.path.join(root, f)) for f in os.listdir(root) if os.path.isfile(os.path.join(root, f)))
    if total > CAP_TOTAL:
        add("example-projects: %d bytes, above the cap %d" % (total, CAP_TOTAL))
    return problems


def check_run(pdir, pname, rid, case, add):
    rd = os.path.join(pdir, "runs", rid)
    where = "%s/runs/%s" % (pname, rid)
    if not os.path.isdir(rd):
        add("%s: missing" % where)
        return
    for f in ["run.json", "case.snapshot", "status.json", "events.jsonl", "log.txt", "residuals.jsonl",
              "metrics.json", "fields/store.rts", "fields/index.json"]:
        if not os.path.exists(os.path.join(rd, f)):
            add("%s/%s: missing" % (where, f))
    try:
        rec = read_json(os.path.join(rd, "run.json"))
        snap = read(os.path.join(rd, "case.snapshot"))
        if not hash_matches(rec["case_snapshot_hash"], snap):
            add("%s/case.snapshot: hash %s does not match run.json %s" % (where, hash_bytes(snap),
                                                                          rec["case_snapshot_hash"]))
        if rec.get("case") != case:
            add("%s/run.json: case %r, expected %r" % (where, rec.get("case"), case))
        cur = os.path.join(pdir, case)
        if not os.path.exists(cur):
            add("%s: the case %s is gone" % (where, case))
        elif read(cur) != snap:
            add("%s: %s changed since the run (stale run: rebuild with scripts/make_example_projects.py)" % (where, case))
        st = read_json(os.path.join(rd, "status.json"))
        if st.get("state") != "finished":
            add("%s/status.json: state %s, expected finished" % (where, st.get("state")))
        res = jsonl(os.path.join(rd, "residuals.jsonl"))
        steps = [r["step"] for r in res]
        if steps != list(range(1, len(steps) + 1)) or (steps and steps[-1] != st.get("step")):
            add("%s/residuals.jsonl: steps are not 1..%s" % (where, st.get("step")))
        idx = read_json(os.path.join(rd, "fields", "index.json"))
        n = len(idx["snapshots"])
        if not 1 <= n <= MAX_SNAPSHOTS:
            add("%s/fields: %d snapshots (limit 1..%d)" % (where, n, MAX_SNAPSHOTS))
        _, n_cells, snaps = store_snapshots(os.path.join(rd, "fields", "store.rts"))
        if n_cells != idx["n_cells"] or len(snaps) != n:
            add("%s/fields: store and index.json disagree (%d/%d cells, %d/%d snapshots)" % (
                where, n_cells, idx["n_cells"], len(snaps), n))
        for step, _, fields in snaps:
            for fname, f in fields.items():
                if f["precision"] != "f32":
                    add("%s/fields: step %d field %s is %s, expected f32" % (where, step, fname, f["precision"]))
    except Exception as e:  # noqa: BLE001
        add("%s: %s" % (where, e))


def check_ledger(pdir, pname, expected, add):
    path = os.path.join(pdir, "designs", "variants.ledger.jsonl")
    if not os.path.exists(path):
        add("%s/designs/variants.ledger.jsonl: missing" % pname)
        return
    entries = jsonl(path)
    runs = {e["settings"]["run"]: e for e in entries}
    if set(runs) != set(expected):
        add("%s/designs: ledger runs %s, expected %s" % (pname, sorted(runs), sorted(expected)))
        return
    for rid, e in runs.items():
        met = read_json(os.path.join(pdir, "runs", rid, "metrics.json"))
        for q in e["metrics"]:
            if q["name"] in met.get("reports", {}) and met["reports"][q["name"]] != q["value"]:
                add("%s/designs: %s %s = %r, metrics.json %r" % (pname, rid, q["name"], q["value"],
                                                                 met["reports"][q["name"]]))


# ------------------------------------------------------------------------------------------------ rerun check


def rerun_check(bins, threads, root=EXAMPLES, projects=None):
    problems = []
    with tempfile.TemporaryDirectory(prefix="rcfd_w48h_rerun_") as tmp:
        for pname in projects or list(PROJECTS):
            src = os.path.join(root, pname)
            dst = os.path.join(tmp, pname)
            shutil.copytree(src, dst, ignore=shutil.ignore_patterns("runs"))
            os.makedirs(os.path.join(dst, "runs"))
            print("rebuilding a copy of %s in %s" % (pname, dst))
            build_project(pname, bins, threads, root=tmp)
            for name in PROJECTS[pname]["meshes"]:
                for f in sorted(os.listdir(os.path.join(src, "meshes", name))):
                    if read(os.path.join(src, "meshes", name, f)) != read(os.path.join(dst, "meshes", name, f)):
                        problems.append("%s/meshes/%s/%s differs from the rebuilt copy" % (pname, name, f))
            for r in PROJECTS[pname]["runs"]:
                a, b = run_of_case(src, r["case"]), run_of_case(dst, r["case"])
                for f in REPRODUCIBLE:
                    pa = os.path.join(src, "runs", a, f)
                    pb = os.path.join(dst, "runs", b, f)
                    same = os.path.exists(pa) and os.path.exists(pb) and read(pa) == read(pb)
                    print("  %s %s: %s" % (r["case"], f, "byte-equal" if same else "DIFFERS"))
                    if not same:
                        problems.append("%s %s %s differs from the rebuilt copy" % (pname, r["case"], f))
    return problems


# ------------------------------------------------------------------------------------------------ main


def binaries(bin_dir):
    bins = {n: os.path.join(bin_dir, n + EXE) for n in ["rcfd", "rcfd-runner"]}
    for n, p in bins.items():
        if not os.path.exists(p):
            sys.exit("%s not found (cargo build --release -p rcfd-cli -p rcfd-runner, or --bin-dir)" % p)
    return bins


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--project", action="append", choices=sorted(PROJECTS), help="rebuild only this project")
    ap.add_argument("--check", action="store_true", help="check the committed examples (no solver run)")
    ap.add_argument("--rerun-check", action="store_true", help="rebuild in a temporary copy and compare")
    ap.add_argument("--bin-dir", default=os.path.join(REPO, "target", "release"))
    ap.add_argument("--threads", type=int, default=1, help="solver threads per run (results do not depend on it)")
    ap.add_argument("--root", default=EXAMPLES, help="folder holding the projects (default: example-projects)")
    ap.add_argument("--summarise-only", action="store_true",
                    help="write the ledger and run tables of the existing runs (no solver run), then the manifest")
    a = ap.parse_args(argv)
    root = os.path.abspath(a.root)
    if a.check:
        problems = check(root)
        for p in problems:
            print("PROBLEM:", p)
        print("check: %d problem(s)" % len(problems))
        return 1 if problems else 0
    bins = binaries(os.path.abspath(a.bin_dir))
    if a.rerun_check:
        problems = rerun_check(bins, a.threads, root, a.project)
        for p in problems:
            print("PROBLEM:", p)
        print("rerun check: %d problem(s)" % len(problems))
        return 1 if problems else 0
    for pname in a.project or list(PROJECTS):
        if a.summarise_only:
            print("summarising %s: %s" % (pname, summarise_existing(pname, root)))
            continue
        print("building %s" % pname)
        t0 = time.time()
        res = build_project(pname, bins, a.threads, root)
        print("  built in %.0f s; pruned %s" % (time.time() - t0, {k: [f for f, _ in v] for k, v in res["pruned"].items()}))
    write_manifest(bins, root)
    problems = check(root)
    for p in problems:
        print("PROBLEM:", p)
    for pname in PROJECTS:
        print("%s: %d bytes" % (pname, tree_size(os.path.join(root, pname))))
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
