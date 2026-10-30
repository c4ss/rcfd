"""ADR 0159 (job 48Y): NACA 0012 angle-of-attack sweep up to and through stall, run on demand.

    python scripts/w48y_aoa_sweep.py mesh <rcfd> <work> [M1|M2 ...] [--far C]   build the aerofoil O-grids into
                                                                         <work>/meshes (--far: far field at C chords,
                                                                         first cell kept, named <mesh>-far<C>; ADR 0167)
    python scripts/w48y_aoa_sweep.py cases <work>                        write every case of the registry to <work>/cases
    python scripts/w48y_aoa_sweep.py run <rcfd> <work> <id> ... [--parallel K]   run cases (one thread each)
    python scripts/w48y_aoa_sweep.py plan <work>                         the DDES and extension ids the registered rules ask
                                                                         for, from the steady runs collected so far
    python scripts/w48y_aoa_sweep.py collect <work> <outdir> [--plots]   results.json, results.csv, histories.csv (and the
                                                                         PNG plots with --plots; needs matplotlib)
    python scripts/w48y_aoa_sweep.py list                                every run id
    python scripts/w48y_aoa_sweep.py selftest                            checks of the statistics and the rules

Nothing is written outside <work> and <outdir>; an existing run folder is never overwritten. The runs are not kept in the
repository (ADR 0159 D6): the committed results (docs/investigations/job48y-aoa-sweep/) are rebuilt by running the ids of
ADR 0159 in a fresh <work> folder.

The mesh (ADR 0159 D2) is the repository's NACA 0012 O-grid route (ADR 0092, `scripts/make_example_projects.py`, recipe
`meshes/naca0012-ogrid.recipe.toml`): the `cylinder_ogrid` generator written as an OpenFOAM polyMesh by `rcfd convert`,
every point moved onto the aerofoil. Two changes against the example's straight blend: the grid lines leave the wall
along the surface normal and turn into the straight line to the far-field point over the first `blend` of the radial
coordinate (so the wall cells are orthogonal), and near the sharp trailing edge the normals are turned towards +x over
an angular width `te_fan` (so neighbouring lines do not jump by the trailing-edge angle). Far field: a circle of radius
`far` chords about mid-chord.

Angles of attack are set by rotating the mesh (ADR 0155): `[mesh.transform] rotate_z_deg = -alpha` about the quarter
chord, the flow along +x, lift along +y, drag along +x.
"""
import csv
import hashlib
import json
import math
import os
import shutil
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from make_example_projects import naca_thickness  # noqa: E402  (ADR 0092 closed-trailing-edge thickness, t = 0.12)

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
LADSON = os.path.join(REPO, "validation-data", "reference", "w31c_naca0012_ladson1988_80grit_cl_cd.csv")

# ---------------------------------------------------------------------------------------------------- registry (ADR 0159)

# mesh name -> parameters (D2). M2 (1.5 x M1 in each direction) has the first radial step of M1 (grid check G1).
# M1 and M2 are the registered meshes; M1 diverged at its trailing-edge cells in every smoke run (amendment 1), so the
# counted runs use M1w and M2w: the same grids with a wall cell for SA's wall law (grading 1e4, wall y+ about 40).
MESHES = {
    "M1": {"nx": 256, "ny": 128, "grading": 3.0e5, "far": 50.0, "blend": 0.1, "te_fan": 0.05},
    "M2": {"nx": 384, "ny": 192, "grading": "match:M1", "far": 50.0, "blend": 0.1, "te_fan": 0.05},
    "M1w": {"nx": 256, "ny": 128, "grading": 1.0e4, "far": 50.0, "blend": 0.1, "te_fan": 0.05},
    "M2w": {"nx": 384, "ny": 192, "grading": "match:M1w", "far": 50.0, "blend": 0.1, "te_fan": 0.05},
}
SWEEP_MESH, GRID_MESH = "M1w", "M2w"  # amendment 1

ALPHAS = [0, 2, 4, 6, 8, 10, 12, 13, 14, 15, 16, 17, 18, 20]  # D1
EXTENSION = [22, 24]  # D8: only when the base sweep shows no stall
SLOPE_ALPHAS = [0, 2, 4, 6, 8]  # L2
NU = 1.6666666666666667e-7  # Re = 6e6 (U = 1, c = 1), the example's value

# steady arms (D3): iteration cap and the window of the statistics
STEADY_ITERATIONS = 8000  # amendment 1 may change it after the disclosed smoke, before the counted runs
STEADY_WINDOW = 1000
SST_ITERATIONS = 4000  # amendment 2
SST_ALPHAS = [0, 4, 8, 12, 14, 16, 18, 20]  # amendment 2
STEADY_CL_RANGE = 0.005  # D4: steady iff max - min over the window <= these
STEADY_CD_RANGE = 0.0005
# transient arm (D5)
DDES_DT = 0.01  # amendment 1 may change it after the smoke; the fallback ladder halves it at most twice (D5)
DDES_END = 40.0
STALL_DROP = 0.10  # L3: stall = some cl after the peak <= (1 - 0.10) cl_max
SLOPE_BAND = (0.90, 1.05)  # L2: slope / (2 pi per rad)
CL0_LIMIT = 0.005  # L4

ARMS = {
    "SA": "steady Spalart-Allmaras (simplec) (primary steady arm)",
    "SST": "steady k-omega SST (simplec) (report only; the example projects' model)",
    "DD": "transient SA-DDES (piso, adaptive correctors)",
    "SA2": "steady Spalart-Allmaras (simplec) on the finer mesh (grid check, report only)",
}

WALL_CAP = {"SA": 4 * 3600, "SST": 4 * 3600, "SA2": 8 * 3600, "DD": 12 * 3600}


def rid(arm, alpha, dt_level=0, smoke=False):
    s = f"{arm}-a{alpha:02d}"
    if dt_level:
        s += f"-dt{dt_level}"
    return ("smoke-" + s) if smoke else s


def registry():
    """Every run id the ADR names: id -> spec. DD ids exist for every angle; which of them run is the plan's (D5)."""
    out = {}
    for a in ALPHAS + EXTENSION:
        out[rid("SA", a)] = {"arm": "SA", "alpha": a, "mesh": SWEEP_MESH}
        # amendment 2: the report-only SST arm runs 4000 iterations, at SST_ALPHAS (and 22, 24 by its own curve)
        out[rid("SST", a)] = {"arm": "SST", "alpha": a, "mesh": SWEEP_MESH, "iterations": SST_ITERATIONS}
        for lvl in (0, 1, 2):
            out[rid("DD", a, lvl)] = {"arm": "DD", "alpha": a, "mesh": SWEEP_MESH, "dt": DDES_DT / 2 ** lvl}
    for a in (6, 14):
        out[rid("SA2", a)] = {"arm": "SA2", "alpha": a, "mesh": GRID_MESH}
    # disclosed smoke runs (not counted): cost, corrector counts, the transient time-step
    out[rid("SA", 6, smoke=True)] = {"arm": "SA", "alpha": 6, "mesh": "M1", "iterations": 3000, "smoke": True}
    out[rid("SA", 16, smoke=True)] = {"arm": "SA", "alpha": 16, "mesh": "M1", "iterations": 3000, "smoke": True}
    out[rid("DD", 16, smoke=True)] = {"arm": "DD", "alpha": 16, "mesh": "M1", "dt": DDES_DT, "end": 2.0, "smoke": True}
    return out


# ---------------------------------------------------------------------------------------------------------------- mesh


def geometric_first(grading, n):
    """First of n cells of a geometric series with last/first ratio `grading`, as a fraction of the total."""
    if abs(grading - 1.0) < 1e-12:
        return 1.0 / n
    r = grading ** (1.0 / (n - 1))
    return (r - 1.0) / (r ** n - 1.0)


def grading_for_first(first, n):
    lo, hi = 1.0, 1e12
    for _ in range(200):
        mid = math.sqrt(lo * hi)
        if geometric_first(mid, n) > first:
            lo = mid
        else:
            hi = mid
    return math.sqrt(lo * hi)


def mesh_params(name):
    p = dict(MESHES[name])
    if isinstance(p["grading"], str):
        ref = MESHES[p["grading"].split(":")[1]]
        p["grading"] = grading_for_first(geometric_first(ref["grading"], ref["ny"]), p["ny"])
    return p


def far_field_variant(name, far):
    """ADR 0167 (job 49D): the mesh `name` with its far-field circle at `far` chords instead of the registered one, and
    the grading changed so that the first radial step (about the first cell at mid-chord) stays the same. Registers the
    variant in MESHES as `<name>-far<far>` and returns that name; `far` equal to the registered value returns `name`
    itself, so the registered meshes (and ADR 0159's results) are unchanged. The grading expression is ADR 0164's for its
    F500w (M1w at 500 chords), in the same order of operations, so that the same points are written."""
    p = MESHES[name]
    if float(far) == float(p["far"]):
        return name
    if isinstance(p["grading"], str):
        raise SystemExit(f"{name}: a far-field variant needs a numeric grading")
    new = f"{name}-far{fmt(far)}"
    MESHES[new] = {**p, "far": float(far),
                   "grading": grading_for_first(geometric_first(p["grading"], p["ny"]) * p["far"] / float(far), p["ny"])}
    return new


def surface(th):
    xa = 0.5 * (1.0 + math.cos(th))
    if th == 0.0:
        return xa, 0.0
    return xa, max(naca_thickness(xa), 0.0) * math.copysign(1.0, math.sin(th))


def wall_direction(th, te_fan):
    h = 1e-6
    x1, y1 = surface(th - h)
    x2, y2 = surface(th + h)
    tx, ty = x2 - x1, y2 - y1
    n = math.hypot(tx, ty)
    nx, ny = ty / n, -tx / n
    px, py = surface(th)
    if nx * (px - 0.5) + ny * py < 0:
        nx, ny = -nx, -ny
    g = math.exp(-((th / te_fan) ** 2)) if te_fan > 0 else 0.0
    ex, ey = (1.0 - g) * nx + g, (1.0 - g) * ny
    m = math.hypot(ex, ey)
    return ex / m, ey / m


def smoothstep(t):
    t = min(max(t, 0.0), 1.0)
    return t * t * (3.0 - 2.0 * t)


def warp(p, far, blend, te_fan, r_in=1.0, r_out=2.0):
    """A point of the cylinder O-grid (r in [r_in, r_out]) to the aerofoil grid (module docstring)."""
    x, y, z = p
    th = math.atan2(y, x)
    if abs(th) < 1e-14:
        th = 0.0
    s = min(max((math.hypot(x, y) - r_in) / (r_out - r_in), 0.0), 1.0)
    px, py = surface(th)
    fx, fy = 0.5 + far * math.cos(th), far * math.sin(th)
    length = math.hypot(fx - px, fy - py)
    erx, ery = (fx - px) / length, (fy - py) / length
    nx, ny = wall_direction(th, te_fan)
    w = smoothstep(s / blend)
    dx, dy = (1.0 - w) * nx + w * erx, (1.0 - w) * ny + w * ery
    m = math.hypot(dx, dy)
    d = s * length
    return px + d * dx / m, py + d * dy / m, z, s, th


def fmt(v):
    r = repr(float(v))
    return r[:-2] if r.endswith(".0") else r


def build_mesh(rcfd, work, name):
    p = mesh_params(name)
    mdir = os.path.join(work, "meshes")
    os.makedirs(mdir, exist_ok=True)
    recipe = os.path.join(mdir, name + ".recipe.toml")
    with open(recipe, "w", encoding="utf-8", newline="\n") as f:
        f.write(
            f'title = "ADR 0159 mesh {name} (cylinder O-grid, warped by scripts/w48y_aoa_sweep.py)"\n\n[mesh]\n'
            f'generator = "cylinder_ogrid"\nnx = {p["nx"]}\nny = {p["ny"]}\nr_in = 1.0\nr_out = 2.0\n'
            f'grading = {fmt(p["grading"])}\ndepth = 0.1\n'
        )
    dst = os.path.join(mdir, name)
    if os.path.exists(dst):
        raise SystemExit(f"{dst} exists; refusing to overwrite a mesh")
    with tempfile.TemporaryDirectory(prefix="w48y_mesh_") as tmp:
        r = subprocess.run([rcfd, "convert", recipe, tmp], capture_output=True, text=True)
        if r.returncode != 0:
            raise SystemExit(f"rcfd convert failed: {r.stderr}")
        src = os.path.join(tmp, "polyMesh")
        os.makedirs(dst)
        for f in ("faces", "owner", "neighbour"):
            shutil.copyfile(os.path.join(src, f), os.path.join(dst, f))
        with open(os.path.join(src, "boundary"), encoding="utf-8") as f:
            text = f.read()
        if text.count("    cylinder\n") != 1:
            raise SystemExit("boundary of the O-grid has no single 'cylinder' patch")
        with open(os.path.join(dst, "boundary"), "w", encoding="utf-8", newline="\n") as f:
            f.write(text.replace("    cylinder\n", "    airfoil\n"))
        with open(os.path.join(src, "points"), encoding="utf-8") as f:
            lines = f.read().split("\n")
    i, out = 0, []
    while not lines[i].strip().isdigit():
        out.append(lines[i])
        i += 1
    n = int(lines[i])
    if lines[i + 1] != "(":
        raise SystemExit("unexpected points file layout")
    out += [lines[i], lines[i + 1]]
    s_min, h_mid, best = 1.0, None, None
    pts = []
    for k in range(n):
        q = tuple(float(a) for a in lines[i + 2 + k].strip("()").split())
        x, y, z, s, th = warp(q, p["far"], p["blend"], p["te_fan"])
        pts.append((x, y, z, s, th))
        out.append("(" + " ".join(fmt(c) for c in (x, y, z)) + ")")
        if s > 1e-15:
            s_min = min(s_min, s)
    # first cell height at mid-chord: the wall point and the first ring point nearest theta = pi/2, z = 0
    wall = [t for t in pts if t[3] == 0.0 and t[2] == 0.0 and t[4] > 0]
    ring = [t for t in pts if abs(t[3] - s_min) < 1e-12 * max(1.0, s_min) + 1e-18 and t[2] == 0.0 and t[4] > 0]
    if wall and ring:
        w0 = min(wall, key=lambda t: abs(t[4] - math.pi / 2))
        r0 = min(ring, key=lambda t: abs(t[4] - w0[4]))
        h_mid = math.hypot(r0[0] - w0[0], r0[1] - w0[1])
        best = w0[0]
    out += lines[i + 2 + n:]
    with open(os.path.join(dst, "points"), "w", encoding="utf-8", newline="\n") as f:
        f.write("\n".join(out))
    sha = hashlib.sha256()
    for f in ("points", "faces", "owner", "neighbour", "boundary"):
        with open(os.path.join(dst, f), "rb") as fh:
            sha.update(fh.read())
    q = subprocess.run([rcfd, "check-mesh", dst, "--json"], capture_output=True, text=True)
    try:
        quality = json.loads(q.stdout)
        quality.pop("poor_list", None)
    except json.JSONDecodeError:
        quality = {"raw": q.stdout, "stderr": q.stderr}
    # flat-plate estimate of the wall y+ of the first cell centre at mid-chord (White: Cf = 0.455 / ln^2(0.06 Re_x))
    re_x = 0.5 / NU
    cf = 0.455 / math.log(0.06 * re_x) ** 2
    y_plus = (h_mid or 0.0) / 2.0 * math.sqrt(cf / 2.0) / NU
    info = {
        "name": name, "params": p, "points": n, "sha256": sha.hexdigest(), "first_s": s_min,
        "first_cell_mid_chord": h_mid, "mid_chord_x": best, "y_plus_centre_estimate": y_plus,
        "check_mesh_exit": q.returncode, "quality": quality,
    }
    with open(os.path.join(mdir, name + ".json"), "w", encoding="utf-8") as f:
        json.dump(info, f, indent=1)
    return info


# --------------------------------------------------------------------------------------------------------------- cases

COMMON = """[mesh]
source = "file"
file = "../meshes/{mesh}"

[mesh.transform]
rotate_z_deg = {rot}   # angle of attack {alpha} deg: the aerofoil turns by -alpha about the quarter chord (ADR 0155)
rotate_origin = [0.25, 0.0, 0.0]

[physics]
nu = 1.6666666666666667e-7   # Re = U c / nu = 6e6

[initial]
velocity = [1.0, 0.0, 0.0]

[boundaries.airfoil]
velocity = "fixed"

[boundaries.farfield]
velocity = "freestream"
velocity_value = [1.0, 0.0, 0.0]
pressure = "freestream"

[reports.cl]
type = "force_coefficient"
patch = "airfoil"
direction = [0.0, 1.0, 0.0]
a_ref = 0.1
u_ref = 1.0

[reports.cd]
type = "force_coefficient"
patch = "airfoil"
direction = [1.0, 0.0, 0.0]
a_ref = 0.1
u_ref = 1.0
"""

STEADY = """
[turbulence]
model = "{model}"

[solver]
algorithm = "simplec"
convection = "linear_upwind"
max_iterations = {n}
min_iterations = {nmin}   # amendment 1: no stop at the residual targets before the cap (the forces still drift there)

[output]
store_interval = {n}   # the last snapshot only
store_precision = "f32"
store_fields = ["ux", "uy", "p", "nut"]
"""

DDES = """
[turbulence]
model = "spalart_allmaras"

[turbulence.sa]
des = "ddes"

[solver]
algorithm = "piso"
corrector_control = "adaptive"   # ADR 0156
convection = "linear_upwind"
dt = {dt}
end_time = {end}

[output]
store_time_interval = {end}   # the last snapshot only
store_precision = "f32"
store_fields = ["ux", "uy", "p", "nut"]
"""


def case_text(spec):
    a = spec["alpha"]
    head = f'title = "NACA 0012, alpha {a} deg, {ARMS[spec["arm"]]}, mesh {spec["mesh"]} (ADR 0159, job 48Y)"\n\n'
    rot = "0.0" if a == 0 else fmt(-float(a))
    body = COMMON.format(mesh=spec["mesh"], rot=rot, alpha=a)
    if spec["arm"] == "DD":
        tail = DDES.format(dt=fmt(spec["dt"]), end=fmt(spec.get("end", DDES_END)))
    else:
        model = "k_omega_sst" if spec["arm"] == "SST" else "spalart_allmaras"
        n = spec.get("iterations", STEADY_ITERATIONS)
        if spec.get("smoke"):
            # the registered smoke cases (S0) had no min_iterations line
            tail = STEADY.format(model=model, n=n, nmin=0).replace(
                "min_iterations = 0   # amendment 1: no stop at the residual targets before the cap (the forces still "
                "drift there)\n", "")
        else:
            tail = STEADY.format(model=model, n=n, nmin=n)
    return head + body + tail


def write_cases(work):
    cdir = os.path.join(work, "cases")
    os.makedirs(cdir, exist_ok=True)
    for i, spec in registry().items():
        with open(os.path.join(cdir, i + ".toml"), "w", encoding="utf-8", newline="\n") as f:
            f.write(case_text(spec))
    return cdir


# ----------------------------------------------------------------------------------------------------------------- run


def command(rcfd, work, i):
    spec = registry()[i]
    cases = os.path.join(work, "cases")
    run = os.path.join(work, "runs", i)
    if os.path.exists(run):
        raise SystemExit(f"{run} exists; refusing to overwrite a run")
    os.makedirs(os.path.join(run, "output"))
    shutil.copyfile(os.path.join(cases, i + ".toml"), os.path.join(run, "case.snapshot"))
    cmd = [
        rcfd, "run", os.path.join(run, "case.snapshot"), "--case-dir", cases,
        "--out", os.path.join(run, "output"),
        "--status-file", os.path.join(run, "solver-status.jsonl"),
        "--status-every", "1", "--quiet", "--threads", "1",
        "--store-dir", os.path.join(run, "fields"),
        "--status-monitors",
        "--max-elapsed", str(WALL_CAP[spec["arm"]]),
    ]
    with open(os.path.join(run, "cmd.txt"), "w", encoding="utf-8") as f:
        f.write(json.dumps(cmd, indent=1) + "\n")
    return run, cmd


def run_many(rcfd, work, ids, parallel):
    reg = registry()
    unknown = [i for i in ids if i not in reg]
    if unknown:
        raise SystemExit(f"unknown ids {unknown}")
    queue, live = list(ids), []
    while queue or live:
        while queue and len(live) < parallel:
            i = queue.pop(0)
            run, cmd = command(rcfd, work, i)
            log = open(os.path.join(run, "log.txt"), "ab")
            t0 = time.time()
            p = subprocess.Popen(cmd, cwd=run, stdin=subprocess.DEVNULL, stdout=log, stderr=log)
            live.append((i, run, p, log, t0))
            print(f"started {i} pid {p.pid} at {time.strftime('%H:%M:%S')}", flush=True)
        time.sleep(2)
        for item in list(live):
            i, run, p, log, t0 = item
            if p.poll() is not None:
                log.close()
                live.remove(item)
                wall = time.time() - t0
                with open(os.path.join(run, "wall.json"), "w", encoding="utf-8") as f:
                    json.dump({"id": i, "exit": p.returncode, "process_wall_s": wall, "started": t0}, f)
                print(f"finished {i} exit {p.returncode} after {wall:.1f} s", flush=True)


# ------------------------------------------------------------------------------------------------------------ analysis


def read_stream(path):
    by, fin, warnings = {}, None, []
    with open(path, encoding="utf-8") as f:
        for line in f:
            try:
                e = json.loads(line)
            except json.JSONDecodeError:
                continue
            ev = e.get("event")
            if ev == "iteration":
                by[e["index"]] = e
            elif ev == "finish":
                fin = e
            elif ev == "warning":
                warnings.append(e)
    return [by[k] for k in sorted(by)], fin, warnings


def stats(values):
    n = len(values)
    if n == 0:
        return None
    m = sum(values) / n
    return {"mean": m, "min": min(values), "max": max(values), "n": n}


def steady_summary(its, window=STEADY_WINDOW):
    """D4: statistics of cl and cd over the last `window` iterations; steady iff both ranges are within the limits."""
    w = [e for e in its if "cl" in e.get("monitors", {})][-window:]
    cl = stats([e["monitors"]["cl"] for e in w])
    cd = stats([e["monitors"]["cd"] for e in w])
    if cl is None:
        return {"steady": False, "reason": "no monitor values"}
    full = len(w) == window
    steady = full and cl["max"] - cl["min"] <= STEADY_CL_RANGE and cd["max"] - cd["min"] <= STEADY_CD_RANGE
    return {
        "steady": steady, "window_iterations": len(w), "cl": cl, "cd": cd,
        "cl_final": w[-1]["monitors"]["cl"], "cd_final": w[-1]["monitors"]["cd"],
        "iterations": its[-1]["index"] if its else 0,
    }


def crossings(ts, ys, mean):
    ups = []
    for (ta, a), (tb, b) in zip(zip(ts, ys), zip(ts[1:], ys[1:])):
        a, b = a - mean, b - mean
        if a < 0 <= b:
            ups.append(ta + (tb - ta) * (-a) / (b - a))
    return ups


def transient_summary(its, end):
    """D5: time statistics over the second half [end/2, end]; half-window drift; Strouhal number from cl."""
    t0 = end / 2.0
    w = [e for e in its if e.get("time", 0.0) >= t0 - 1e-12 and "cl" in e.get("monitors", {})]
    if len(w) < 3:
        return {"reached": False}
    ts = [e["time"] for e in w]
    cl = [e["monitors"]["cl"] for e in w]
    cd = [e["monitors"]["cd"] for e in w]

    def tmean(ys, lo, hi):
        acc, span = 0.0, 0.0
        for (ta, a), (tb, b) in zip(zip(ts, ys), zip(ts[1:], ys[1:])):
            a0, b0 = max(ta, lo), min(tb, hi)
            if b0 <= a0:
                continue
            fa = a + (b - a) * (a0 - ta) / (tb - ta)
            fb = a + (b - a) * (b0 - ta) / (tb - ta)
            acc += 0.5 * (fa + fb) * (b0 - a0)
            span += b0 - a0
        return acc / span if span > 0 else None

    t_end = ts[-1]
    mid = 0.5 * (t0 + t_end)
    out = {
        "reached": t_end >= end - 1e-9 * max(1.0, end),
        "window": [t0, t_end],
        "cl": {"mean": tmean(cl, t0, t_end), "min": min(cl), "max": max(cl)},
        "cd": {"mean": tmean(cd, t0, t_end), "min": min(cd), "max": max(cd)},
        "cl_half_drift": tmean(cl, mid, t_end) - tmean(cl, t0, mid),
        "cd_half_drift": tmean(cd, mid, t_end) - tmean(cd, t0, mid),
        "steps": its[-1]["index"],
    }
    ups = crossings(ts, cl, out["cl"]["mean"])
    if len(ups) >= 4 and out["cl"]["max"] - out["cl"]["min"] > 1e-3:
        period = (ups[-1] - ups[0]) / (len(ups) - 1)
        out["St_chord"] = 1.0 / period
        out["periods"] = len(ups) - 1
    cors = [e["correctors"] for e in its if "correctors" in e]
    if cors:
        out["correctors"] = {"mean": sum(cors) / len(cors), "max": max(cors), "min": min(cors)}
    caps = [e.get("corrector_cap", 0) for e in its]
    out["corrector_cap"] = max(caps) if caps else 0
    return out


def slope_fit(points):
    """Least squares cl = a + b alpha (alpha in rad) through (alpha_deg, cl); returns b, a."""
    xs = [math.radians(a) for a, _ in points]
    ys = [c for _, c in points]
    n = len(xs)
    mx, my = sum(xs) / n, sum(ys) / n
    sxx = sum((x - mx) ** 2 for x in xs)
    b = sum((x - mx) * (y - my) for x, y in zip(xs, ys)) / sxx
    return b, my - b * mx


def stall(curve, drop=STALL_DROP):
    """L3 on a curve [(alpha, cl)] sorted by alpha: the peak, and whether some later cl <= (1 - drop) cl_max."""
    if not curve:
        return {"detected": False, "reason": "empty"}
    k = max(range(len(curve)), key=lambda j: curve[j][1])
    a_peak, cl_max = curve[k]
    after = curve[k + 1:]
    low = min(after, key=lambda t: t[1]) if after else None
    det = bool(after) and low[1] <= (1.0 - drop) * cl_max
    first = next((t for t in after if t[1] <= (1.0 - drop) * cl_max), None)
    return {
        "detected": det, "alpha_peak": a_peak, "cl_max": cl_max,
        "lowest_after": list(low) if low else None, "first_alpha_below": first[0] if first else None,
        "drop_fraction": (cl_max - low[1]) / cl_max if low else 0.0,
    }


def ddes_plan(sa_steady):
    """D5: the DDES angles from {alpha: steady?} of the SA arm: every unsteady angle, and the two highest steady angles
    below the lowest unsteady one (overlap); with no unsteady angle, 18 and 20 (the deepest base angles)."""
    unsteady = sorted(a for a, s in sa_steady.items() if not s)
    if not unsteady:
        return sorted(a for a in (18, 20) if a in sa_steady)
    below = sorted(a for a, s in sa_steady.items() if s and a < unsteady[0])
    return sorted(set(unsteady) | set(below[-2:]))


def read_runs(work):
    out = {}
    rdir = os.path.join(work, "runs")
    if not os.path.isdir(rdir):
        return out
    reg = registry()
    for i in sorted(os.listdir(rdir)):
        if i not in reg:
            continue
        spec = reg[i]
        stream = os.path.join(rdir, i, "solver-status.jsonl")
        wallf = os.path.join(rdir, i, "wall.json")
        if not os.path.exists(stream) or not os.path.exists(wallf):
            continue
        its, fin, warnings = read_stream(stream)
        with open(wallf, encoding="utf-8") as f:
            wall = json.load(f)
        r = {"id": i, **spec, "exit": wall["exit"], "wall_s": wall["process_wall_s"],
             "state": (fin or {}).get("state"), "solver_state": (fin or {}).get("solver_state"),
             "warnings": [w.get("kind") or w.get("rule") or w.get("message", "")[:60] for w in warnings]}
        if spec["arm"] == "DD":
            r["summary"] = transient_summary(its, spec.get("end", DDES_END))
            r["complete"] = wall["exit"] == 0 and r["summary"].get("reached", False)
        else:
            r["summary"] = steady_summary(its)
            r["complete"] = wall["exit"] in (0, 5)
        r["history"] = [(e["index"], e.get("time"), e["monitors"].get("cl"), e["monitors"].get("cd"))
                        for e in its if "cl" in e.get("monitors", {})]
        out[i] = r
    return out


def ladson():
    with open(LADSON, encoding="utf-8") as f:
        return [(float(r["alpha_deg"]), float(r["cl"]), float(r["cd"])) for r in csv.DictReader(f)]


def interp(table, a, col):
    for (a0, *v0), (a1, *v1) in zip(table, table[1:]):
        if a0 <= a <= a1:
            return v0[col] + (v1[col] - v0[col]) * (a - a0) / (a1 - a0)
    return None


def analyse(runs):
    """The registered evaluation (ADR 0159 L1-L4, D4-D8) of the counted runs (smoke runs left out)."""
    counted = {i: r for i, r in runs.items() if not r.get("smoke")}
    sa = {r["alpha"]: r for r in counted.values() if r["arm"] == "SA"}
    sst = {r["alpha"]: r for r in counted.values() if r["arm"] == "SST"}
    # DD: per angle the run with the lowest dt level that completed, else the last one tried
    dd = {}
    for r in sorted((r for r in counted.values() if r["arm"] == "DD"), key=lambda r: r["id"]):
        cur = dd.get(r["alpha"])
        if cur is None or (not cur["complete"]):
            dd[r["alpha"]] = r
    sa_steady = {a: r["summary"].get("steady", False) for a, r in sa.items()}
    curve, rows = [], []
    for a in sorted(sa):
        r = sa[a]
        s = r["summary"]
        if s.get("steady"):
            curve.append({"alpha": a, "source": "SA steady", "cl": s["cl"]["mean"], "cd": s["cd"]["mean"],
                          "cl_min": s["cl"]["min"], "cl_max": s["cl"]["max"], "cd_min": s["cd"]["min"],
                          "cd_max": s["cd"]["max"], "flag": ""})
        elif a in dd and dd[a]["complete"]:
            t = dd[a]["summary"]
            curve.append({"alpha": a, "source": "DD time mean", "cl": t["cl"]["mean"], "cd": t["cd"]["mean"],
                          "cl_min": t["cl"]["min"], "cl_max": t["cl"]["max"], "cd_min": t["cd"]["min"],
                          "cd_max": t["cd"]["max"], "flag": ""})
        elif "cl" in s:
            curve.append({"alpha": a, "source": "SA iteration mean", "cl": s["cl"]["mean"], "cd": s["cd"]["mean"],
                          "cl_min": s["cl"]["min"], "cl_max": s["cl"]["max"], "cd_min": s["cd"]["min"],
                          "cd_max": s["cd"]["max"], "flag": "steady SA not converged and no completed DDES run"})
    res = {"curve": curve, "sa_steady": {str(a): v for a, v in sorted(sa_steady.items())}}
    res["ddes_plan"] = ddes_plan(sa_steady) if sa_steady else []
    # L1 runs complete
    res["L1"] = {"incomplete": sorted(i for i, r in counted.items() if not r["complete"]),
                 "counted": len(counted)}
    # L2 slope
    pts = []
    for a in SLOPE_ALPHAS:
        c = next((p for p in curve if p["alpha"] == a), None)
        if c:
            pts.append((a, c["cl"]))
    if len(pts) == len(SLOPE_ALPHAS):
        b, icpt = slope_fit(pts)
        res["L2"] = {"slope_per_rad": b, "slope_per_deg": b * math.pi / 180.0, "factor": b / (2 * math.pi),
                     "intercept": icpt, "band": list(SLOPE_BAND),
                     "met": SLOPE_BAND[0] <= b / (2 * math.pi) <= SLOPE_BAND[1]}
    # L3 stall on the curve; base and with the extension
    base = [(p["alpha"], p["cl"]) for p in curve if p["alpha"] in ALPHAS]
    full = [(p["alpha"], p["cl"]) for p in curve]
    res["L3_base"] = stall(base)
    res["L3"] = stall(full)
    res["extension_needed"] = bool(base) and not res["L3_base"]["detected"]
    # L4 symmetry
    c0 = next((p for p in curve if p["alpha"] == 0), None)
    if c0:
        res["L4"] = {"cl0": c0["cl"], "limit": CL0_LIMIT, "met": abs(c0["cl"]) <= CL0_LIMIT}
    # report-only: SST curve, grid check, Ladson
    sst_curve = []
    for a in sorted(sst):
        s = sst[a]["summary"]
        if "cl" in s:
            sst_curve.append({"alpha": a, "cl": s["cl"]["mean"], "cd": s["cd"]["mean"], "cl_min": s["cl"]["min"],
                              "cl_max": s["cl"]["max"], "steady": s["steady"]})
    res["sst_curve"] = sst_curve
    if sst_curve:
        res["sst_stall"] = stall([(p["alpha"], p["cl"]) for p in sst_curve])
        spts = [(p["alpha"], p["cl"]) for p in sst_curve if p["alpha"] in SLOPE_ALPHAS]
        if len(spts) >= 3:  # amendment 2: 0, 4 and 8 degrees
            b, _ = slope_fit(spts)
            res["sst_slope_factor"] = b / (2 * math.pi)
            res["sst_slope_alphas"] = [a for a, _ in spts]
    res["ddes"] = {str(a): {"id": r["id"], "dt": r["dt"], "complete": r["complete"], **{
        k: r["summary"].get(k) for k in ("cl", "cd", "cl_half_drift", "cd_half_drift", "St_chord", "periods",
                                         "correctors", "corrector_cap", "steps", "reached")}}
                   for a, r in sorted(dd.items())}
    grid = {}
    for r in counted.values():
        if r["arm"] == "SA2" and r["alpha"] in sa and "cl" in r["summary"] and "cl" in sa[r["alpha"]]["summary"]:
            f, c = sa[r["alpha"]]["summary"], r["summary"]
            grid[str(r["alpha"])] = {"M2_cl": c["cl"]["mean"], "M1_cl": f["cl"]["mean"], "M2_cd": c["cd"]["mean"],
                                     "M1_cd": f["cd"]["mean"], "M2_steady": c["steady"], "M1_steady": f["steady"]}
    res["grid_check"] = grid
    lad = ladson()
    lad_curve = [(a, c) for a, c, _ in lad]
    res["ladson_80grit"] = {"cl_max": max(c for _, c in lad_curve),
                            "alpha_cl_max": max(lad_curve, key=lambda t: t[1])[0],
                            "slope_factor": slope_fit([(a, c) for a, c in lad_curve if -4.1 <= a <= 8.31])[0] / (2 * math.pi),
                            "cd_near_0": next(cd for a, c, cd in lad if abs(a) < 0.1)}
    res["vs_ladson"] = [{"alpha": p["alpha"], "cl": p["cl"], "cl_ladson": interp(lad, p["alpha"], 0),
                         "cd": p["cd"], "cd_ladson": interp(lad, p["alpha"], 1)} for p in curve]
    return res


def write_outputs(runs, res, outdir, plots):
    os.makedirs(outdir, exist_ok=True)
    slim = {i: {k: v for k, v in r.items() if k != "history"} for i, r in runs.items()}
    with open(os.path.join(outdir, "results.json"), "w", encoding="utf-8", newline="\n") as f:
        json.dump({"adr": "0159", "runs": slim, "evaluation": res}, f, indent=1, sort_keys=True)
        f.write("\n")
    with open(os.path.join(outdir, "results.csv"), "w", encoding="utf-8", newline="") as f:
        w = csv.writer(f, lineterminator="\n")
        w.writerow(["id", "arm", "alpha_deg", "dt", "exit", "complete", "steady_or_reached", "cl_mean", "cl_min",
                    "cl_max", "cd_mean", "cd_min", "cd_max", "iterations_or_steps", "St_chord", "correctors_mean",
                    "correctors_max", "wall_s"])
        for i, r in sorted(runs.items()):
            s = r["summary"]
            cl, cd = s.get("cl") or {}, s.get("cd") or {}
            w.writerow([i, r["arm"], r["alpha"], r.get("dt", ""), r["exit"], r["complete"],
                        s.get("steady", s.get("reached")), *(f"{v:.6g}" if isinstance(v, float) else v for v in (
                            cl.get("mean"), cl.get("min"), cl.get("max"), cd.get("mean"), cd.get("min"),
                            cd.get("max"))),
                        s.get("iterations", s.get("steps")), f"{s['St_chord']:.4g}" if "St_chord" in s else "",
                        f"{s['correctors']['mean']:.3g}" if "correctors" in s else "",
                        s["correctors"]["max"] if "correctors" in s else "", f"{r['wall_s']:.0f}"])
    with open(os.path.join(outdir, "histories.csv"), "w", encoding="utf-8", newline="") as f:
        w = csv.writer(f, lineterminator="\n")
        w.writerow(["id", "index", "time", "cl", "cd"])
        for i, r in sorted(runs.items()):
            h = r["history"]
            stride = max(1, math.ceil(len(h) / 100))  # at most 101 samples (D6: at most 201)
            sel = h[::stride]
            if h and sel[-1] is not h[-1]:
                sel.append(h[-1])
            for idx, t, cl, cd in sel:
                w.writerow([i, idx, "" if t is None else f"{t:.6g}", f"{cl:.6g}", f"{cd:.6g}"])
    if plots:
        make_plots(runs, res, outdir)


def make_plots(runs, res, outdir):
    import matplotlib  # only here: the rest of the driver needs the standard library only

    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    lad = ladson()
    curve = res["curve"]
    fig, ax = plt.subplots(figsize=(6.4, 4.4), dpi=90)
    ax.plot([a for a, _, _ in lad], [c for _, c, _ in lad], "s", mfc="none", color="0.35", ms=5,
            label="Ladson 1988, Re 6e6, 80 grit (experiment)")
    if res.get("sst_curve"):
        sc = res["sst_curve"]
        ax.errorbar([p["alpha"] for p in sc], [p["cl"] for p in sc],
                    yerr=[[p["cl"] - p["cl_min"] for p in sc], [p["cl_max"] - p["cl"] for p in sc]], fmt="--",
                    color="#b07aa1", lw=1.0, capsize=2, alpha=0.8, label="SST, window mean, min/max (report only)")
        unst = [p for p in sc if not p["steady"]]
        if unst:
            ax.plot([p["alpha"] for p in unst], [p["cl"] for p in unst], "v", mfc="none", color="#b07aa1", ms=6,
                    label="SST not steady (D4)")
    dds = [(int(a), d) for a, d in res.get("ddes", {}).items() if d.get("complete") and d.get("cl")]
    if dds:
        ax.errorbar([a for a, _ in dds], [d["cl"]["mean"] for _, d in dds],
                    yerr=[[d["cl"]["mean"] - d["cl"]["min"] for _, d in dds],
                          [d["cl"]["max"] - d["cl"]["mean"] for _, d in dds]],
                    fmt="D", color="#e15759", ms=5, capsize=3, label="SA-DDES time mean, min/max")
    xs = [p["alpha"] for p in curve]
    ys = [p["cl"] for p in curve]
    err = [[p["cl"] - p["cl_min"] for p in curve], [p["cl_max"] - p["cl"] for p in curve]]
    ax.errorbar(xs, ys, yerr=err, fmt="-", color="#4e79a7", lw=1.6, capsize=3, label="sweep curve (min/max bars)")
    for src, mk, col in (("SA steady", "o", "#4e79a7"), ("DD time mean", "D", "#e15759"),
                         ("SA iteration mean", "x", "#f28e2b")):
        pts = [p for p in curve if p["source"] == src]
        if pts:
            ax.plot([p["alpha"] for p in pts], [p["cl"] for p in pts], mk, color=col, ms=5,
                    label="sweep curve: " + src)
    a = [0, 20]
    ax.plot(a, [2 * math.pi * math.radians(v) for v in a], ":", color="0.6", lw=1, label="2 pi alpha")
    ax.set_xlabel("angle of attack alpha [deg]")
    ax.set_ylabel("lift coefficient cl")
    ax.set_title("NACA 0012, Re 6e6: cl(alpha) (ADR 0159)")
    ax.set_ylim(-0.1, 2.6)
    ax.grid(alpha=0.3)
    ax.legend(fontsize=6, loc="upper left")
    fig.tight_layout()
    fig.savefig(os.path.join(outdir, "cl_alpha.png"))
    plt.close(fig)

    fig, ax = plt.subplots(figsize=(6.4, 4.4), dpi=90)
    ax.plot([cd for _, _, cd in lad], [c for _, c, _ in lad], "s", mfc="none", color="0.35", ms=5,
            label="Ladson 1988, 80 grit")
    ax.plot([p["cd"] for p in curve], [p["cl"] for p in curve], "o-", color="#4e79a7", label="sweep curve (SA)")
    if res.get("sst_curve"):
        sc = res["sst_curve"]
        ax.plot([p["cd"] for p in sc], [p["cl"] for p in sc], "v--", mfc="none", color="#b07aa1", lw=1.0,
                label="SST window mean (report only)")
    for p in curve:
        ax.annotate(str(p["alpha"]), (p["cd"], p["cl"]), fontsize=6, xytext=(3, -3), textcoords="offset points")
    ax.set_xscale("log")
    ax.set_xlabel("drag coefficient cd (log)")
    ax.set_ylabel("lift coefficient cl")
    ax.set_title("NACA 0012, Re 6e6: drag polar (ADR 0159)")
    ax.grid(alpha=0.3, which="both")
    ax.legend(fontsize=7, loc="lower right")
    fig.tight_layout()
    fig.savefig(os.path.join(outdir, "polar.png"))
    plt.close(fig)

    hist = [r for r in runs.values() if not r.get("smoke") and (r["arm"] == "DD" or (
        r["arm"] == "SA" and not r["summary"].get("steady", True)))]
    sst_unsteady = [r for r in runs.values() if not r.get("smoke") and r["arm"] == "SST"
                    and not r["summary"].get("steady", True)]
    if hist or sst_unsteady:
        fig, (ax, ax2) = plt.subplots(2, 1, figsize=(6.4, 5.6), dpi=90)
        for r in sorted(hist, key=lambda r: (r["arm"], r["alpha"])):
            h = r["history"]
            stride = max(1, len(h) // 800)
            if r["arm"] == "DD":
                ax.plot([t for _, t, _, _ in h[::stride]], [c for _, _, c, _ in h[::stride]], lw=0.8,
                        label=f"SA-DDES {r['alpha']} deg")
        ax.axvspan(DDES_END / 2, DDES_END, color="0.9", zorder=0, label="averaging window")
        ax.set_ylim(-0.2, 2.2)
        ax.set_xlabel("time t U / c (the start-up spike is cut off)")
        ax.set_ylabel("cl")
        ax.set_title("SA-DDES lift histories and SST iteration histories (ADR 0159)", fontsize=10)
        ax.grid(alpha=0.3)
        ax.legend(fontsize=6, ncol=3)
        for r in sorted(sst_unsteady, key=lambda r: r["alpha"]):
            h = r["history"]
            stride = max(1, len(h) // 1500)
            hw = [e for e in h if e[0] > SST_ITERATIONS - STEADY_WINDOW]
            ax2.plot([i for i, _, _, _ in hw], [c for _, _, c, _ in hw], lw=0.7,
                     label=f"SST {r['alpha']} deg (not steady)")
        ax2.set_ylim(-0.5, 2.5)
        ax2.set_xlabel("iteration (the window of D4, the last 1000)")
        ax2.set_ylabel("cl")
        ax2.grid(alpha=0.3)
        ax2.legend(fontsize=6, ncol=4)
        fig.tight_layout()
        fig.savefig(os.path.join(outdir, "histories.png"))
        plt.close(fig)


# ------------------------------------------------------------------------------------------------------------ selftest


def selftest():
    # geometric first cell and its inverse
    g = grading_for_first(geometric_first(3.0e5, 128), 192)
    assert abs(geometric_first(g, 192) / geometric_first(3.0e5, 128) - 1) < 1e-9
    # the warp keeps the wall on the aerofoil and the outer ring on the far-field circle
    for th in (0.3, 1.0, 2.5, -0.7, -2.9, math.pi):
        c, s = math.cos(th), math.sin(th)
        x, y, *_ = warp((c, s, 0.0), 50.0, 0.1, 0.05)
        xa, ya = surface(math.atan2(s, c))
        assert abs(x - xa) < 1e-12 and abs(y - ya) < 1e-12
        x, y, *_ = warp((2 * c, 2 * s, 0.0), 50.0, 0.1, 0.05)
        assert abs(math.hypot(x - 0.5, y) - 50.0) < 1e-9
    # steady window
    its = [{"index": k, "monitors": {"cl": 1.0 + 1e-4 * (k % 3), "cd": 0.01}} for k in range(1, 3001)]
    assert steady_summary(its)["steady"]
    its = [{"index": k, "monitors": {"cl": 1.0 + 0.01 * math.sin(k), "cd": 0.01}} for k in range(1, 3001)]
    assert not steady_summary(its)["steady"]
    # transient window: a sine of period 2 about 1.2
    its = [{"index": k, "time": 0.01 * k, "monitors": {"cl": 1.2 + 0.1 * math.sin(math.pi * 0.01 * k), "cd": 0.2}}
           for k in range(1, 4001)]
    t = transient_summary(its, 40.0)
    assert t["reached"] and abs(t["cl"]["mean"] - 1.2) < 1e-3 and abs(t["St_chord"] - 0.5) < 1e-3
    # slope and stall
    b, a = slope_fit([(x, 0.1 * x) for x in SLOPE_ALPHAS])
    assert abs(b - 0.1 * 180 / math.pi) < 1e-9 and abs(a) < 1e-12
    assert stall([(10, 1.0), (14, 1.4), (16, 1.5), (18, 1.3)])["detected"]
    assert not stall([(10, 1.0), (14, 1.4), (16, 1.5), (18, 1.4)])["detected"]
    assert not stall([(10, 1.0), (20, 1.5)])["detected"]
    # DDES plan
    p = ddes_plan({a: a < 16 for a in ALPHAS})
    assert p == [14, 15, 16, 17, 18, 20], p
    assert ddes_plan({a: True for a in ALPHAS}) == [18, 20]
    assert ddes_plan({a: a != 13 for a in ALPHAS}) == [10, 12, 13]
    # every registered case is a distinct text
    texts = {case_text(s) for s in registry().values()}
    assert len(texts) == len(registry())
    # ADR 0167: the far-field variant keeps the first radial step and leaves the registered meshes alone
    assert far_field_variant("M1w", 50.0) == "M1w" and MESHES["M1w"]["far"] == 50.0
    v = far_field_variant("M1w", 500.0)
    assert v == "M1w-far500" and MESHES[v]["far"] == 500.0
    assert MESHES[v]["grading"] == grading_for_first(geometric_first(1.0e4, 128) * 50.0 / 500.0, 128)
    del MESHES[v]
    print("selftest: ok")


def main():
    a = sys.argv[1:]
    if not a:
        print(__doc__)
        return
    cmd = a[0]
    if cmd == "selftest":
        selftest()
    elif cmd == "list":
        for i, s in registry().items():
            print(i, json.dumps(s))
    elif cmd == "mesh":
        args = a[1:]
        far = None
        if "--far" in args:  # ADR 0167: far-field circle in chords (default: each mesh's registered value)
            k = args.index("--far")
            far = float(args[k + 1])
            del args[k:k + 2]
        rcfd, work = args[0], args[1]
        names = args[2:] or [SWEEP_MESH, GRID_MESH]
        if far is not None:
            names = [far_field_variant(n, far) for n in names]
        for name in names:
            info = build_mesh(rcfd, work, name)
            print(json.dumps({k: v for k, v in info.items() if k != "quality"}, indent=1))
            q = info["quality"]
            print({k: q.get(k) for k in ("n_cells", "max_non_orthogonality", "max_skewness", "max_aspect_ratio",
                                         "max_volume_ratio", "errors")})
    elif cmd == "cases":
        print(write_cases(a[1]))
    elif cmd == "run":
        args = a[1:]
        par = 4
        if "--parallel" in args:
            k = args.index("--parallel")
            par = int(args[k + 1])
            del args[k:k + 2]
        run_many(args[0], args[1], args[2:], par)
    elif cmd == "plan":
        runs = read_runs(a[1])
        res = analyse(runs)
        print(json.dumps({"sa_steady": res["sa_steady"], "ddes_angles": res["ddes_plan"],
                          "extension_needed": res["extension_needed"], "incomplete": res["L1"]["incomplete"]},
                         indent=1))
    elif cmd == "collect":
        runs = read_runs(a[1])
        res = analyse(runs)
        write_outputs(runs, res, a[2], "--plots" in a)
        print(json.dumps({k: res.get(k) for k in ("L1", "L2", "L3", "L4", "extension_needed", "ddes_plan")},
                         indent=1))
    else:
        raise SystemExit(__doc__)


if __name__ == "__main__":
    main()
