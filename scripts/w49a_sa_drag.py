"""ADR 0164 (job 49A): why steady Spalart-Allmaras drag of the NACA 0012 is wrong (ADR 0159 question 4), run on demand.

    python scripts/w49a_sa_drag.py mesh <rcfd> <work>                       M1w (ADR 0159) and F500w into <work>/meshes
    python scripts/w49a_sa_drag.py cases <work>                             every case of the registry to <work>/cases
    python scripts/w49a_sa_drag.py run <rcfd> <work> <id> ... [--parallel K] run cases (one thread each)
    python scripts/w49a_sa_drag.py collect <work> <outdir> [--ref48y <w48y-work>] [--plots]
                                                                            results.json, results.csv (and plots)
    python scripts/w49a_sa_drag.py tmr                                      friction part of the TMR SA cd at alpha 0
    python scripts/w49a_sa_drag.py list | selftest

Arms (ADR 0164):
  A00, A10   ADR 0159's steady SA case (M1w, alpha 0 / 10) unchanged, plus extra reports (pressure / viscous parts of cd,
             the opt-in *_effective components, the volume average of nut). Must reproduce ADR 0159's cl/cd bit for bit.
  B00, B10   A with the far-field nuTilda fixed: boundaries.farfield.turbulence = "inlet" (nuTilda = 3 nu there).
  C10        B10 on F500w, the same O-grid with a far field of 500 chords (report only).
  S00        ADR 0159's SST case at alpha 0 with the extra force reports (report only).
  P01, P05, P40   SA flat plate through `rcfd run` (Re 5e6 per m, plate 0..2), wall-cell y+ 1, 5 and 40 at x = 0.97.

Nothing is written outside <work> and <outdir>; an existing run folder is never overwritten.
"""
import csv
import json
import math
import os
import re
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import w48y_aoa_sweep as w48  # noqa: E402  (ADR 0159 driver: mesh builder, case text, stream reader)

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
REF = os.path.join(REPO, "validation-data", "reference")
TMR_CLCD = os.path.join(REF, "w49a_tmr_naca0012_sa_cfl3d_clcd.csv")
TMR_CF0 = os.path.join(REF, "w49a_tmr_naca0012_sa_cfl3d_cf_alpha0_upper.csv")

NU_AF = w48.NU  # 1/6e6
NU_PLATE = 2e-7  # Re 5e6 per m (TMR 2DZP)
TMR_PLATE_CF097 = 2.70565e-3  # TMR CFL3D SA flat plate, Cf(x = 0.97) (ADR 0014)
X_STATION = 0.97

# ---------------------------------------------------------------------------------------------------- registry (ADR 0164)

AF_ITER = 8000  # ADR 0159 D3 / amendment 1 (min_iterations = cap)
SST_ITER = 4000  # ADR 0159 amendment 2
PLATE_ITER = 15000
WINDOW = 1000

# far field of 500 chords with the first cell of M1w at mid-chord (report-only C10)
w48.MESHES["F500w"] = {"nx": 256, "ny": 128, "far": 500.0, "blend": 0.1, "te_fan": 0.05,
                       "grading": w48.grading_for_first(w48.geometric_first(1.0e4, 128) * 50.0 / 500.0, 128)}

PLATES = {  # id -> (target y+ at x = 0.97, wall layers, total ny)
    "P01": (1.0, 50, 70),
    "P05": (5.0, 40, 60),
    "P40": (40.0, 25, 45),
}


def registry():
    out = {
        "A00": {"kind": "af", "arm": "SA", "alpha": 0, "mesh": "M1w", "fix_farfield": False},
        "A10": {"kind": "af", "arm": "SA", "alpha": 10, "mesh": "M1w", "fix_farfield": False},
        "B00": {"kind": "af", "arm": "SA", "alpha": 0, "mesh": "M1w", "fix_farfield": True},
        "B10": {"kind": "af", "arm": "SA", "alpha": 10, "mesh": "M1w", "fix_farfield": True},
        "C10": {"kind": "af", "arm": "SA", "alpha": 10, "mesh": "F500w", "fix_farfield": True},
        "S00": {"kind": "af", "arm": "SST", "alpha": 0, "mesh": "M1w", "fix_farfield": False},
    }
    for k, (yp, layers, ny) in PLATES.items():
        out[k] = {"kind": "plate", "y_plus": yp, "layers": layers, "ny": ny}
    return out


# --------------------------------------------------------------------------------------------------------------- cases

EXTRA_REPORTS = """
# ADR 0164: extra reports (reports do not change the solution)
[reports.cd_p]
type = "force_coefficient"
patch = "airfoil"
direction = [1.0, 0.0, 0.0]
a_ref = 0.1
u_ref = 1.0
component = "pressure"

[reports.cd_v]
type = "force_coefficient"
patch = "airfoil"
direction = [1.0, 0.0, 0.0]
a_ref = 0.1
u_ref = 1.0
component = "viscous"

[reports.cd_eff]
type = "force_coefficient"
patch = "airfoil"
direction = [1.0, 0.0, 0.0]
a_ref = 0.1
u_ref = 1.0
component = "total_effective"

[reports.cd_veff]
type = "force_coefficient"
patch = "airfoil"
direction = [1.0, 0.0, 0.0]
a_ref = 0.1
u_ref = 1.0
component = "viscous_effective"

[reports.cl_eff]
type = "force_coefficient"
patch = "airfoil"
direction = [0.0, 1.0, 0.0]
a_ref = 0.1
u_ref = 1.0
component = "total_effective"
"""

NT_REPORT = """
[reports.nut_avg]
type = "volume_average"
field = "nut"   # nuTilda is not a report field; the eddy viscosity of the far field shows its growth
"""

FARFIELD = 'pressure = "freestream"\n'
FARFIELD_FIXED = ('pressure = "freestream"\nturbulence = "inlet"   # ADR 0164 arm B: nuTilda fixed to '
                  'turbulence.nutilde_ratio * nu = 3 nu on the far field\n')

PLATE = """title = "SA flat plate, wall-cell y+ {yp} at x = 0.97 (ADR 0164, job 49A, arm {id})"

[mesh]
generator = "box_2d"
nx = 200
ny = {ny}
lx = 2.0
ly = 1.0
wall_layers = {layers}
layer_walls = "lower"
layer_growth = 1.15
target_y_plus = {yp}
layer_reference_velocity = 1.0
layer_reference_length = 0.97

[physics]
nu = 2e-7   # Re = 5e6 per m (TMR 2DZP)

[initial]
velocity = [1.0, 0.0, 0.0]

[boundaries.xmin]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]

[boundaries.xmax]
velocity = "zero_gradient"
pressure = "fixed"

[boundaries.ymin]
velocity = "fixed"

[boundaries.ymax]
velocity = "symmetry"

[turbulence]
model = "spalart_allmaras"

[solver]
algorithm = "simplec"
convection = "linear_upwind"
max_iterations = {n}
min_iterations = {n}

[output]
store_interval = {n}
store_precision = "f32"
store_fields = ["ux", "uy", "p", "nut"]

[reports.cf_v]
type = "force_coefficient"
patch = "ymin"
direction = [1.0, 0.0, 0.0]
a_ref = 1.0
u_ref = 1.0
component = "viscous"

[reports.cf_veff]
type = "force_coefficient"
patch = "ymin"
direction = [1.0, 0.0, 0.0]
a_ref = 1.0
u_ref = 1.0
component = "viscous_effective"
"""


def case_text(i, spec):
    if spec["kind"] == "plate":
        return PLATE.format(yp=fmt_num(spec["y_plus"]), id=i, ny=spec["ny"], layers=spec["layers"], n=PLATE_ITER)
    base = {"arm": spec["arm"], "alpha": spec["alpha"], "mesh": spec["mesh"]}
    if spec["arm"] == "SST":
        base["iterations"] = SST_ITER
    text = w48.case_text(base)
    if spec["fix_farfield"]:
        assert text.count(FARFIELD) == 1
        text = text.replace(FARFIELD, FARFIELD_FIXED)
    text += EXTRA_REPORTS
    if spec["arm"] == "SA":
        text += NT_REPORT
    return f"# ADR 0164 (job 49A) arm {i}; the text above the extra reports is ADR 0159's case\n" + text


def fmt_num(v):
    return w48.fmt(v)


def write_cases(work):
    cdir = os.path.join(work, "cases")
    os.makedirs(cdir, exist_ok=True)
    for i, spec in registry().items():
        with open(os.path.join(cdir, i + ".toml"), "w", encoding="utf-8", newline="\n") as f:
            f.write(case_text(i, spec))
    return cdir


# ----------------------------------------------------------------------------------------------------------------- run

WALL_CAP = 6 * 3600


def command(rcfd, work, i):
    cases = os.path.join(work, "cases")
    run = os.path.join(work, "runs", i)
    if os.path.exists(run):
        raise SystemExit(f"{run} exists; refusing to overwrite a run")
    os.makedirs(os.path.join(run, "output"))
    with open(os.path.join(cases, i + ".toml"), encoding="utf-8") as f:
        text = f.read()
    with open(os.path.join(run, "case.snapshot"), "w", encoding="utf-8", newline="\n") as f:
        f.write(text)
    cmd = [
        rcfd, "run", os.path.join(run, "case.snapshot"), "--case-dir", cases,
        "--out", os.path.join(run, "output"),
        "--status-file", os.path.join(run, "solver-status.jsonl"),
        "--status-every", "1", "--quiet", "--threads", "1",
        "--store-dir", os.path.join(run, "fields"),
        "--status-monitors",
        "--max-elapsed", str(WALL_CAP),
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


def read_vtu(path):
    """Arrays of an ASCII .vtu written by rcfd (name -> list of floats or tuples)."""
    with open(path, encoding="utf-8") as f:
        txt = f.read()
    out = {}
    pat = r'<DataArray type="(\w+)" Name="(\w+)" NumberOfComponents="(\d+)" format="ascii">(.*?)</DataArray>'
    for m in re.finditer(pat, txt, re.S):
        typ, name, nc, body = m.group(1), m.group(2), int(m.group(3)), m.group(4).split()
        conv = float if typ.startswith("Float") else int
        vals = [conv(v) for v in body]
        out[name] = [tuple(vals[k:k + nc]) for k in range(0, len(vals), nc)] if nc > 1 else vals
    return out


def cell_points(a):
    con, off = a["connectivity"], a["offsets"]
    start = 0
    for e in off:
        yield sorted(set(con[start:e]))
        start = e


def centre(pts, ids):
    n = len(ids)
    return tuple(sum(pts[i][k] for i in ids) / n for k in range(3))


def freestream_nutilde(vtu, nu):
    """Median nuTilda/nu over the cells 5 to 40 chords from mid-chord, upstream (x < -0.5, |y| > 2)."""
    a = read_vtu(vtu)
    pts = a["Points"]
    vals = []
    for c, ids in enumerate(cell_points(a)):
        x, y, _ = centre(pts, ids)
        r = math.hypot(x - 0.5, y)
        if 5.0 <= r <= 40.0 and x < -0.5 and abs(y) > 2.0:
            vals.append(a["nuTilda"][c] / nu)
    vals.sort()
    if not vals:
        return None
    return {"median": vals[len(vals) // 2], "min": vals[0], "max": vals[-1], "cells": len(vals)}


def plate_wall(vtu):
    """Local Cf along the plate from the wall cells (wall at y = 0): the wall law's 2 u_tau^2 (u_tau = y+ nu / y_P, the
    model's y+ field) and the molecular 2 nu u_P / y_P; and their integrals over x in [0, 2] (per unit width)."""
    a = read_vtu(vtu)
    pts = a["Points"]
    rows = []
    for c, ids in enumerate(cell_points(a)):
        if a["y_plus"][c] <= 0.0:
            continue
        xs = [pts[i][0] for i in ids]
        ys = [pts[i][1] for i in ids]
        if min(ys) > 1e-12:
            continue
        xc, yc, _ = centre(pts, ids)
        dx = max(xs) - min(xs)
        ut = a["y_plus"][c] * NU_PLATE / yc
        ux = a["U"][c][0]
        rows.append((xc, dx, 2.0 * ut * ut, 2.0 * NU_PLATE * ux / yc, a["y_plus"][c]))
    rows.sort()
    out = {"cells": len(rows)}
    for k, name in ((2, "law"), (3, "molecular")):
        out[f"CF_{name}"] = sum(r[1] * r[k] for r in rows) / 2.0  # mean over the plate length 2
        xs = [r[0] for r in rows]
        ys = [r[k] for r in rows]
        out[f"cf097_{name}"] = interp(xs, ys, X_STATION)
    out["y_plus_097"] = interp([r[0] for r in rows], [r[4] for r in rows], X_STATION)
    return out


def interp(xs, ys, x):
    for (xa, ya), (xb, yb) in zip(zip(xs, ys), zip(xs[1:], ys[1:])):
        if xa <= x <= xb:
            return ya + (yb - ya) * (x - xa) / (xb - xa)
    return None


def mean_range(its, key, lo, hi):
    v = [e["monitors"][key] for e in its if lo <= e["index"] <= hi and key in e.get("monitors", {})]
    if not v:
        return None
    return {"mean": sum(v) / len(v), "min": min(v), "max": max(v), "n": len(v)}


def monitors_csv(path):
    """ADR 0159 run's monitors.csv: iteration -> {column: value}."""
    out = {}
    with open(path, encoding="utf-8") as f:
        for row in csv.DictReader(f):
            out[int(row["iteration"])] = row
    return out


def bit_compare(its, ref_csv):
    """Every iteration's cl and cd of this run against the ADR 0159 run's monitors.csv, as printed text (17 digits)."""
    ref = monitors_csv(ref_csv)
    n, diff, first = 0, 0, None
    for e in its:
        r = ref.get(e["index"])
        if r is None:
            continue
        for k in ("cd", "cl"):
            n += 1
            if float(r[k]) != e["monitors"][k]:
                diff += 1
                if first is None:
                    first = (e["index"], k, e["monitors"][k], float(r[k]))
    return {"compared": n, "different": diff, "first_difference": first, "ref_iterations": len(ref),
            "run_iterations": len(its)}


def tmr_friction():
    """Friction part of the TMR CFL3D SA cd at alpha 0: 2 * integral cf dx over the upper surface (symmetric)."""
    pts = []
    with open(TMR_CF0, encoding="utf-8") as f:
        for row in csv.reader(f):
            if row and not row[0].startswith("#") and row[0] != "x":
                pts.append((float(row[0]), float(row[1])))
    pts.sort()
    s = sum(0.5 * (ca + cb) * (xb - xa) for (xa, ca), (xb, cb) in zip(pts, pts[1:]))
    clcd = {}
    with open(TMR_CLCD, encoding="utf-8") as f:
        for row in csv.reader(f):
            if row and not row[0].startswith("#") and row[0] != "alpha_deg":
                clcd[float(row[0])] = (float(row[1]), float(row[2]))
    cd0 = clcd[0.0][1]
    return {"cd_friction_alpha0": 2.0 * s, "cd_alpha0": cd0, "cd_pressure_alpha0": cd0 - 2.0 * s,
            "cl_alpha10": clcd[10.0][0], "cd_alpha10": clcd[10.0][1], "points": len(pts)}


def analyse(work, ref48y):
    reg = registry()
    rdir = os.path.join(work, "runs")
    runs = {}
    for i in sorted(os.listdir(rdir)) if os.path.isdir(rdir) else []:
        if i not in reg:
            continue
        stream = os.path.join(rdir, i, "solver-status.jsonl")
        wallf = os.path.join(rdir, i, "wall.json")
        if not (os.path.exists(stream) and os.path.exists(wallf)):
            continue
        its, fin, warnings = w48.read_stream(stream)
        with open(wallf, encoding="utf-8") as f:
            wall = json.load(f)
        spec = reg[i]
        last = its[-1]["index"] if its else 0
        r = {"id": i, **spec, "exit": wall["exit"], "wall_s": wall["process_wall_s"], "iterations": last,
             "state": (fin or {}).get("state"),
             "warnings": [w.get("kind") or w.get("rule") or w.get("message", "")[:60] for w in warnings]}
        keys = sorted({k for e in its[-1:] for k in e.get("monitors", {})})
        r["window"] = {k: mean_range(its, k, last - WINDOW + 1, last) for k in keys}
        vtu = os.path.join(rdir, i, "output", "solution.vtu")
        if spec["kind"] == "af":
            if "cd" in keys:
                early = mean_range(its, "cd", 1001, 2000)
                mid = mean_range(its, "cd", 3001, 4000)
                late = r["window"]["cd"]
                r["drift_cd_1k2k_to_last"] = (late["mean"] - early["mean"]) / late["mean"] if early else None
                r["drift_cd_3k4k_to_last"] = (late["mean"] - mid["mean"]) / late["mean"] if mid else None
            if spec["arm"] == "SA" and os.path.exists(vtu):
                r["freestream_nutilde"] = freestream_nutilde(vtu, NU_AF)
            ref_id = {"A00": "SA-a00", "A10": "SA-a10", "S00": "SST-a00"}.get(i)
            if ref_id and ref48y:
                p = os.path.join(ref48y, "runs", ref_id, "output", "monitors.csv")
                if os.path.exists(p):
                    r["bits_vs_adr0159"] = bit_compare(its, p)
        elif os.path.exists(vtu):
            r["wall"] = plate_wall(vtu)
        r["history"] = [(e["index"], {k: e["monitors"].get(k) for k in ("cd", "cd_eff", "cl", "nut_avg", "cf_v",
                                                                         "cf_veff")})
                        for e in its if e["index"] % 100 == 0 or e["index"] == last]
        runs[i] = r
    return runs


def evaluate(runs):
    tmr = tmr_friction()
    ev = {"tmr": tmr}

    def w(i, k):
        try:
            return runs[i]["window"][k]["mean"]
        except (KeyError, TypeError):
            return None

    def band(v, ref, rel):
        return None if v is None else abs(v / ref - 1.0) <= rel

    # LP: plate
    for i, rel in (("P01", 0.05), ("P05", 0.10), ("P40", 0.10)):
        wl = runs.get(i, {}).get("wall")
        if wl:
            ev[f"LP1_{i}"] = {"cf097_law": wl["cf097_law"], "vs_tmr": wl["cf097_law"] / TMR_PLATE_CF097 - 1.0,
                              "band": rel, "met": band(wl["cf097_law"], TMR_PLATE_CF097, rel)}
            ev[f"ratio_{i}"] = wl["cf097_molecular"] / wl["cf097_law"]
            cf_eff = w(i, "cf_veff")
            if cf_eff is not None:
                ev[f"LP4_{i}"] = {"CF_report_effective": cf_eff / 2.0 / plate_depth(runs[i]),
                                  "CF_law_integral": wl["CF_law"]}
    if "ratio_P01" in ev:
        ev["LP2"] = {"ratio": ev["ratio_P01"], "met": abs(ev["ratio_P01"] - 1.0) <= 0.03}
    if "ratio_P40" in ev:
        ev["LP3"] = {"ratio": ev["ratio_P40"], "met": ev["ratio_P40"] <= 0.6}
    for i in ("P01", "P40"):
        x = ev.get(f"LP4_{i}")
        if x:
            x["rel"] = x["CF_report_effective"] / x["CF_law_integral"] - 1.0
            x["met"] = abs(x["rel"]) <= 0.02
    # LA: aerofoil
    if w("A00", "cd_v") is not None:
        ev["LA1"] = {"ratio_molecular_to_effective": w("A00", "cd_v") / w("A00", "cd_veff"),
                     "met": w("A00", "cd_v") / w("A00", "cd_veff") <= 0.6}
    for i in ("A00", "B00"):
        if w(i, "cd_veff") is not None:
            ev[f"LA2_{i}"] = {"cd_friction_effective": w(i, "cd_veff"), "tmr": tmr["cd_friction_alpha0"],
                              "rel": w(i, "cd_veff") / tmr["cd_friction_alpha0"] - 1.0,
                              "met": band(w(i, "cd_veff"), tmr["cd_friction_alpha0"], 0.10)}
            ev[f"LA3_{i}"] = {"cd_effective": w(i, "cd_eff"), "tmr": tmr["cd_alpha0"],
                              "rel": w(i, "cd_eff") / tmr["cd_alpha0"] - 1.0,
                              "met": band(w(i, "cd_eff"), tmr["cd_alpha0"], 0.12)}
    if w("B10", "cd_eff") is not None:
        ev["LA4"] = {"cd_effective": w("B10", "cd_eff"), "cl": w("B10", "cl"),
                     "cd_rel": w("B10", "cd_eff") / tmr["cd_alpha10"] - 1.0,
                     "cl_rel": w("B10", "cl") / tmr["cl_alpha10"] - 1.0,
                     "met": bool(band(w("B10", "cd_eff"), tmr["cd_alpha10"], 0.25)
                                 and band(w("B10", "cl"), tmr["cl_alpha10"], 0.05))}
    fs = {i: (runs.get(i, {}).get("freestream_nutilde") or {}).get("median") for i in ("A10", "B00", "B10")}
    if all(v is not None for v in fs.values()):
        ev["LA5"] = {"median_nutilde_over_nu": fs,
                     "met": fs["A10"] >= 1000.0 and fs["B00"] <= 5.0 and fs["B10"] <= 5.0}
    if "B10" in runs and runs["B10"].get("drift_cd_1k2k_to_last") is not None:
        d = runs["B10"]["drift_cd_1k2k_to_last"]
        ev["LA6"] = {"drift_B10": d, "drift_A10": runs.get("A10", {}).get("drift_cd_1k2k_to_last"),
                     "met": abs(d) <= 0.02}
    for i in ("A00", "A10"):
        b = runs.get(i, {}).get("bits_vs_adr0159")
        if b:
            ev[f"LR_{i}"] = {**b, "met": b["different"] == 0 and b["compared"] > 0}
    ev["L1"] = {i: runs[i]["exit"] for i in runs}
    ev["L1"]["met"] = all(runs[i]["exit"] in (0, 5) for i in runs)
    return ev


def plate_depth(run):
    return run.get("depth", 1.0)


def collect(work, outdir, ref48y, plots):
    runs = analyse(work, ref48y)
    # plate depth from the mesh points (box_2d), so that the report's force coefficient becomes C_F per unit width
    for i, r in runs.items():
        vtu = os.path.join(work, "runs", i, "output", "solution.vtu")
        if r["kind"] == "plate" and os.path.exists(vtu):
            zs = [p[2] for p in read_vtu(vtu)["Points"]]
            r["depth"] = max(zs) - min(zs)
    ev = evaluate(runs)
    os.makedirs(outdir, exist_ok=True)
    with open(os.path.join(outdir, "results.json"), "w", encoding="utf-8", newline="\n") as f:
        json.dump({"runs": runs, "evaluation": ev}, f, indent=1)
    cols = ["id", "exit", "iterations", "wall_s", "cl", "cd", "cd_p", "cd_v", "cd_eff", "cd_veff", "cl_eff", "nut_avg",
            "cf_v", "cf_veff"]
    with open(os.path.join(outdir, "results.csv"), "w", encoding="utf-8", newline="\n") as f:
        wr = csv.writer(f)
        wr.writerow(cols)
        for i, r in runs.items():
            row = [i, r["exit"], r["iterations"], round(r["wall_s"], 1)]
            for k in cols[4:]:
                v = (r["window"].get(k) or {}).get("mean")
                row.append("" if v is None else f"{v:.6g}")
            wr.writerow(row)
    if plots:
        make_plots(runs, outdir)
    return ev


def make_plots(runs, outdir):
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    fig, axes = plt.subplots(1, 2, figsize=(10, 3.8))
    for ax, key, title in ((axes[0], "cd", "cd (default report)"), (axes[1], "cd_eff", "cd (total_effective)")):
        for i in ("A00", "A10", "B00", "B10", "C10"):
            if i in runs:
                h = [(it, m[key]) for it, m in runs[i]["history"] if m.get(key) is not None]
                if h:
                    ax.plot([a for a, _ in h], [b for _, b in h], label=i)
        ax.set_yscale("log")
        ax.set_xlabel("iteration")
        ax.set_title(title)
        ax.legend(fontsize=8)
    fig.tight_layout()
    fig.savefig(os.path.join(outdir, "cd_histories.png"), dpi=80)
    plt.close(fig)


# ------------------------------------------------------------------------------------------------------------- selftest


def selftest():
    reg = registry()
    for i, spec in reg.items():
        t = case_text(i, spec)
        if spec["kind"] == "af":
            base = w48.case_text({"arm": spec["arm"], "alpha": spec["alpha"], "mesh": spec["mesh"],
                                  **({"iterations": SST_ITER} if spec["arm"] == "SST" else {})})
            body = t.split("\n", 1)[1]
            if spec["fix_farfield"]:
                assert body.startswith(base.replace(FARFIELD, FARFIELD_FIXED)), i
            else:
                assert body.startswith(base), i  # arm A / S: ADR 0159's case text unchanged
            assert ('turbulence = "inlet"' in t) == spec["fix_farfield"], i
    assert abs(interp([0.0, 1.0], [0.0, 2.0], 0.25) - 0.5) < 1e-15
    g = w48.MESHES["F500w"]["grading"]
    f1 = w48.geometric_first(1.0e4, 128) * 50.0
    f2 = w48.geometric_first(g, 128) * 500.0
    assert abs(f2 / f1 - 1.0) < 1e-6, (f1, f2)
    if os.path.exists(TMR_CF0):
        t = tmr_friction()
        assert 0.006 < t["cd_friction_alpha0"] < 0.0075, t
    print("selftest passed")


def main():
    a = sys.argv[1:]
    if not a:
        print(__doc__)
        return
    cmd = a[0]
    if cmd == "mesh":
        rcfd, work = a[1], a[2]
        for name in a[3:] or ["M1w", "F500w"]:
            info = w48.build_mesh(rcfd, work, name)
            print(name, info["sha256"], info["first_cell_mid_chord"], info["quality"].get("max_non_orthogonality",
                                                                                           info["quality"]))
    elif cmd == "cases":
        print(write_cases(a[1]))
    elif cmd == "run":
        par = 1
        ids = []
        k = 3
        while k < len(a):
            if a[k] == "--parallel":
                par = int(a[k + 1])
                k += 2
            else:
                ids.append(a[k])
                k += 1
        run_many(a[1], a[2], ids, par)
    elif cmd == "collect":
        ref = a[a.index("--ref48y") + 1] if "--ref48y" in a else None
        ev = collect(a[1], a[2], ref, "--plots" in a)
        print(json.dumps(ev, indent=1))
    elif cmd == "tmr":
        print(json.dumps(tmr_friction(), indent=1))
    elif cmd == "list":
        print("\n".join(registry()))
    elif cmd == "selftest":
        selftest()
    else:
        raise SystemExit(f"unknown command {cmd}")


if __name__ == "__main__":
    main()
