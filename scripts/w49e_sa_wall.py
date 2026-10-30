"""ADR 0168 (job 49E): Spalart-Allmaras wall treatment with the wall cell in the buffer layer, run on demand.

    python scripts/w49e_sa_wall.py model                                   1-D prediction of the plate Cf error (D7)
    python scripts/w49e_sa_wall.py mesh <rcfd> <work>                       M1w, M1w15, M1w05 into <work>/meshes
    python scripts/w49e_sa_wall.py cases <work>                             every case of the registry to <work>/cases
    python scripts/w49e_sa_wall.py run <rcfd> <work> <id> ... [--parallel K] run cases (one thread each)
    python scripts/w49e_sa_wall.py collect <work> <outdir> --ref49a <w49a-work/w> [--plots]
                                                                            results.json, results.csv (and the plot)
    python scripts/w49e_sa_wall.py list | selftest

Arms (ADR 0168):
  Dnn   SA flat plate of ADR 0164 (Re 5e6 per m, plate 0..2, `rcfd run`, SIMPLEC, linear upwind), wall-cell y+ nn at
        x = 0.97, default wall mode `auto`. D01, D05, D40 are ADR 0164's P01, P05, P40 (must match them bit for bit).
  Enn   Dnn with `turbulence.sa.wall_mode = "auto_sa_law"` (the opt-in of this job).
  ADyy  ADR 0164's F00 (NACA 0012, alpha 0, far-field turbulence role `freestream`, effective force reports) on the mesh
        of wall y+ yy (M1w05, M1w15, M1w), default wall mode. AD40 is F00 (must match it bit for bit).
  AEyy  ADyy with the opt-in wall mode.

Nothing is written outside <work> and <outdir>; an existing run folder is never overwritten.
"""
import csv
import json
import math
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import w49a_sa_drag as w49a  # noqa: E402  (ADR 0164 driver: plate case, wall reading, TMR files)

w48 = w49a.w48

TMR_CF097 = w49a.TMR_PLATE_CF097  # 2.70565e-3, TMR CFL3D SA flat plate Cf(x = 0.97)
NU = w49a.NU_PLATE
X = w49a.X_STATION
RE_L = 2.0 / NU  # 1e7
AF_ITER = w49a.AF_ITER
PLATE_ITER = w49a.PLATE_ITER
WINDOW = w49a.WINDOW
OPT_IN = '\n[turbulence.sa]\nwall_mode = "auto_sa_law"   # ADR 0168 opt-in\n'

# ---------------------------------------------------------------------------------------------------- registry (ADR 0168)

# target wall-cell centre y+ at x = 0.97 -> (wall layers, total ny); growth 1.15 throughout. 1, 5, 40 are ADR 0164's;
# the others keep the layer stack at 0.08 to 0.1 of the plate height (D2), ny = layers + 20 as there
PLATES = {1: (50, 70), 2: (45, 65), 3: (43, 63), 5: (40, 60), 8: (37, 57), 12: (34, 54), 15: (32, 52),
          30: (27, 47), 40: (25, 45), 60: (22, 42)}

# M1w (grading 1e4) has the wall-cell median y+ 42 (ADR 0164, ADR 0159's SA-a00); M1w15 and M1w05 scale its first cell
M1W_MEDIAN_YPLUS = 42.0
AF_MESHES = {"M1w05": 5.0, "M1w15": 15.0}
for _name, _yp in AF_MESHES.items():
    w48.MESHES[_name] = {"nx": 256, "ny": 128, "far": 50.0, "blend": 0.1, "te_fan": 0.05,
                         "grading": w48.grading_for_first(w48.geometric_first(1.0e4, 128) * _yp / M1W_MEDIAN_YPLUS,
                                                          128)}
AF_MESH_OF = {5: "M1w05", 15: "M1w15", 40: "M1w"}


def registry():
    out = {}
    for yp, (layers, ny) in PLATES.items():
        for arm in ("D", "E"):
            out[f"{arm}{yp:02d}"] = {"kind": "plate", "y_plus": float(yp), "layers": layers, "ny": ny,
                                     "opt_in": arm == "E"}
    for yp, mesh in AF_MESH_OF.items():
        for arm in ("AD", "AE"):
            out[f"{arm}{yp:02d}"] = {"kind": "af", "y_plus": float(yp), "mesh": mesh, "opt_in": arm == "AE"}
    # amendment 1 (binary rcfd-w49e-am1.exe): Fnn / AFyy = Enn / AEyy with the amended opt-in; G05, G40, GA40 = D05,
    # D40, AD40 again (default bits of the amended build)
    for yp, (layers, ny) in PLATES.items():
        out[f"F{yp:02d}"] = {"kind": "plate", "y_plus": float(yp), "layers": layers, "ny": ny, "opt_in": True,
                             "amended": True}
    for yp, mesh in AF_MESH_OF.items():
        out[f"AF{yp:02d}"] = {"kind": "af", "y_plus": float(yp), "mesh": mesh, "opt_in": True, "amended": True}
    for yp in (5, 40):
        layers, ny = PLATES[yp]
        out[f"G{yp:02d}"] = {"kind": "plate", "y_plus": float(yp), "layers": layers, "ny": ny, "opt_in": False,
                             "amended": True}
    out["GA40"] = {"kind": "af", "y_plus": 40.0, "mesh": "M1w", "opt_in": False, "amended": True}
    return out


# --------------------------------------------------------------------------------------------------------------- cases


def case_text(i, spec):
    if spec["kind"] == "plate":
        text = w49a.PLATE.format(yp=w48.fmt(spec["y_plus"]), id=i, ny=spec["ny"], layers=spec["layers"],
                                 n=PLATE_ITER)
        text = text.replace("(ADR 0164, job 49A, arm", "(ADR 0168, job 49E, arm")
    else:
        f00 = {"kind": "af", "arm": "SA", "alpha": 0, "mesh": spec["mesh"], "fix_farfield": "freestream"}
        text = w49a.case_text(i, f00)
        text = f"# ADR 0168 (job 49E) arm {i}: ADR 0164's F00 on mesh {spec['mesh']}\n" + text
    if spec["opt_in"]:
        text += OPT_IN
    return text


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


# ------------------------------------------------------------------------------------------- 1-D model (D7, prediction)

K, CV1, E_SPALDING = 0.41, 7.1, 9.8
UE_PLUS = math.sqrt(2.0 / TMR_CF097)  # edge velocity in wall units at x = 0.97


def fv1(c):
    return c ** 3 / (c ** 3 + CV1 ** 3)


def nutp_sa(y):
    return K * y * fv1(K * y)


def uplus_sa(yp, n=4000):
    """SA inner-layer law: integral of 1/(1 + nut+) (Simpson in ln y+ from 1e-6)."""
    if yp <= 1e-6:
        return yp
    a, b = math.log(1e-6), math.log(yp)
    h = (b - a) / n
    s = 0.0
    for k in range(n + 1):
        y = math.exp(a + k * h)
        s += (1 if k in (0, n) else (4 if k % 2 else 2)) * y / (1.0 + nutp_sa(y))
    return 1e-6 + s * h / 3.0


def spalding_yp(u):
    k = K * u
    return u + (math.exp(k) - 1 - k - k * k / 2 - k ** 3 / 6) / E_SPALDING


def spalding_up(yp):
    lo, hi = 0.0, 60.0
    for _ in range(100):
        m = 0.5 * (lo + hi)
        if spalding_yp(m) < yp:
            lo = m
        else:
            hi = m
    return lo


def spalding_nutp(yp):
    k = K * spalding_up(yp)
    return (K / E_SPALDING) * (math.exp(k) - 1 - k - k * k / 2)


def chi_from_nutp(n):
    if n <= 0:
        return 0.0
    lo, hi = n, n + 2 * CV1 + 1
    while hi * fv1(hi) < n:
        hi *= 2
    for _ in range(100):
        m = 0.5 * (lo + hi)
        if m * fv1(m) < n:
            lo = m
        else:
            hi = m
    return 0.5 * (lo + hi)


def model_1d(y1, opt_in, growth=1.15, yref=400.0, fix=5.0):
    """Constant-stress layer (tau+ = 1) on the plate's wall layers (first centre y1+, growth 1.15), cells above the wall
    cell at the SA inner-layer solution nuT+ = kappa y+. Default: Spalding u+ of the wall cell, its nut fixed to
    Spalding's above y+ 5, linear nut on the faces. Opt-in: the SA law, nuT+ = kappa y1+, the law's series resistance
    on the first face above y+ 5, nut of the interpolated nuT elsewhere. Returns (u+ offset at yref, Cf error)."""
    h = 2.0 * y1
    faces, cent, y, k = [0.0], [], 0.0, 0
    while y < 3 * yref:
        hk = h * growth ** k
        cent.append(y + hk / 2)
        y += hk
        faces.append(y)
        k += 1
    chi = [K * c for c in cent]
    nut = [nutp_sa(c) for c in cent]
    up = uplus_sa if opt_in else spalding_up
    if y1 > fix and not opt_in:
        nut[0] = spalding_nutp(y1)
        chi[0] = chi_from_nutp(nut[0])
    u = [up(y1)]
    for i in range(len(cent) - 1):
        if opt_in and i == 0 and y1 > fix:
            u.append(u[-1] + up(cent[1]) - up(cent[0]))
            continue
        w = (cent[i + 1] - faces[i + 1]) / (cent[i + 1] - cent[i])
        if opt_in:
            cf = w * chi[i] + (1 - w) * chi[i + 1]
            nf = cf * fv1(cf)
        else:
            nf = w * nut[i] + (1 - w) * nut[i + 1]
        u.append(u[-1] + (cent[i + 1] - cent[i]) / (1 + nf))
    ud = None
    for i in range(len(cent) - 1):
        if cent[i] <= yref <= cent[i + 1]:
            ud = u[i] + (u[i + 1] - u[i]) * (yref - cent[i]) / (cent[i + 1] - cent[i])
            break
    d = ud - uplus_sa(yref)
    return d, (1.0 + d / UE_PLUS) ** -2 - 1.0


def model_table():
    return {yp: {"default": model_1d(yp, False)[1], "opt_in": model_1d(yp, True)[1]} for yp in PLATES}


# ------------------------------------------------------------------------------------------------------------ analysis


def schlichting_cf(re_x):
    """Schlichting's local skin-friction law cf = (2 log10 Re_x - 0.65)^-2.3."""
    return (2.0 * math.log10(re_x) - 0.65) ** -2.3


def stream_compare(its, ref_stream, keys):
    """Every iteration's monitors `keys` of this run against a reference run's status stream (exact floats)."""
    ref, _, _ = w48.read_stream(ref_stream)
    rmap = {e["index"]: e.get("monitors", {}) for e in ref}
    n, diff, first = 0, 0, None
    for e in its:
        r = rmap.get(e["index"])
        if r is None:
            continue
        for k in keys:
            if k in r and k in e.get("monitors", {}):
                n += 1
                if r[k] != e["monitors"][k]:
                    diff += 1
                    if first is None:
                        first = (e["index"], k, e["monitors"][k], r[k])
    return {"compared": n, "different": diff, "first_difference": first, "ref_iterations": len(ref),
            "run_iterations": len(its)}


def af_wall_yplus(vtu):
    a = w49a.read_vtu(vtu)
    v = sorted(x for x in a["y_plus"] if x > 0.0)
    if not v:
        return None
    return {"cells": len(v), "median": v[len(v) // 2], "min": v[0], "max": v[-1],
            "p10": v[len(v) // 10], "p90": v[(9 * len(v)) // 10], "above_5": sum(1 for x in v if x > 5.0)}


def plate_profile(vtu):
    """Amendment 1 diagnostics: the wall-cell y+ along x in [0.8, 1.2] (non-increasing?) and, in the cell column nearest
    x = 0.97, the second cell's nuTilda / (kappa u_tau y) with u_tau from the wall cell's y+."""
    a = w49a.read_vtu(vtu)
    pts = a["Points"]
    cells = []
    for c, ids in enumerate(w49a.cell_points(a)):
        xc, yc, _ = w49a.centre(pts, ids)
        cells.append((round(xc, 9), yc, c))
    wall = sorted((x, a["y_plus"][c]) for x, y, c in cells if a["y_plus"][c] > 0.0)
    band = [v for x, v in wall if 0.8 <= x <= 1.2]
    x0 = min({x for x, _, _ in cells}, key=lambda x: abs(x - X))
    col = sorted((y, c) for x, y, c in cells if x == x0)
    (y1, c1), (y2, c2) = col[0], col[1]
    ut = a["y_plus"][c1] * NU / y1
    return {"wall_yplus_08_12": [round(v, 4) for v in band],
            "non_increasing": all(b <= a_ + 1e-9 for a_, b in zip(band, band[1:])),
            "x_column": x0, "second_cell_yplus": y2 * ut / NU,
            "second_cell_nutilde_over_kappa_ut_y": a["nuTilda"][c2] / (K * ut * y2)}


REF_OF = {"D01": "P01", "D05": "P05", "D40": "P40", "AD40": "F00", "G05": "P05", "G40": "P40", "GA40": "F00"}
PLATE_KEYS = ("cf_v", "cf_veff")
AF_KEYS = ("cl", "cd", "cd_p", "cd_v", "cd_eff", "cd_veff", "cl_eff", "nut_avg")


def analyse(work, ref49a):
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
        r = {"id": i, **spec, "exit": wall["exit"], "wall_s": round(wall["process_wall_s"], 1), "iterations": last,
             "state": (fin or {}).get("state"), "finish": (fin or {}).get("reason") or (fin or {}).get("message"),
             "warnings": [w.get("kind") or w.get("rule") or w.get("message", "")[:60] for w in warnings][:5]}
        keys = sorted({k for e in its[-1:] for k in e.get("monitors", {})})
        nwin = min(WINDOW, last // 2)
        r["window_iterations"] = nwin
        r["window"] = {k: w49a.mean_range(its, k, last - nwin + 1, last) for k in keys}
        vtu = os.path.join(rdir, i, "output", "solution.vtu")
        if os.path.exists(vtu):
            if spec["kind"] == "plate":
                r["wall"] = w49a.plate_wall(vtu)
                r["profile"] = plate_profile(vtu)
                zs = [p[2] for p in w49a.read_vtu(vtu)["Points"]]
                r["depth"] = max(zs) - min(zs)
            else:
                r["wall_yplus"] = af_wall_yplus(vtu)
        ref = REF_OF.get(i)
        if ref and ref49a:
            p = os.path.join(ref49a, "runs", ref, "solver-status.jsonl")
            if os.path.exists(p):
                r["bits_vs_adr0164"] = {"ref": ref, **stream_compare(its, p, PLATE_KEYS if spec["kind"] == "plate"
                                                                     else AF_KEYS)}
        runs[i] = r
    return runs


def evaluate(runs, tmr):
    ev = {}
    model = model_table()
    ev["model_1d"] = model

    def w(i, k):
        try:
            return runs[i]["window"][k]["mean"]
        except (KeyError, TypeError):
            return None

    def law(i):
        wl = (runs.get(i) or {}).get("wall")
        return wl["cf097_law"] if wl and wl.get("cf097_law") else None

    ok = lambda r: r is not None and r.get("exit") in (0, 5)  # noqa: E731
    # L1
    reg = registry()
    for key, amended in (("L1", False), ("L1_A1", True)):
        ids = [i for i, s in reg.items() if bool(s.get("amended")) == amended]
        ev[key] = {i: runs[i]["exit"] for i in ids if i in runs}
        ev[key]["missing"] = [i for i in ids if i not in runs]
        ev[key]["met"] = all(runs[i]["exit"] in (0, 5) for i in ids if i in runs) and not ev[key]["missing"]
    # LR (registered: D01, D05, D40, AD40) and LR_A1 (amendment 1: G05, G40, GA40)
    for key, amended in (("LR", False), ("LR_A1", True)):
        lr = {i: (runs.get(i) or {}).get("bits_vs_adr0164") for i in REF_OF if bool(reg[i].get("amended")) == amended}
        ev[key] = {"runs": lr, "met": all(b is not None and b["compared"] > 0 and b["different"] == 0
                                          and b["ref_iterations"] == b["run_iterations"] for b in lr.values())}
    # plate table
    table = {}
    for yp in PLATES:
        row = {}
        for arm in ("D", "E", "F"):
            i = f"{arm}{yp:02d}"
            r = runs.get(i)
            if not ok(r) or law(i) is None:
                row[arm] = None
                continue
            wl = r["wall"]
            cf_eff = w(i, "cf_veff")
            row[arm] = {"y_plus_097": wl["y_plus_097"], "cf097_law": wl["cf097_law"],
                        "vs_tmr": wl["cf097_law"] / TMR_CF097 - 1.0,
                        "vs_schlichting": wl["cf097_law"] / schlichting_cf(X / NU) - 1.0,
                        "molecular_over_law": wl["cf097_molecular"] / wl["cf097_law"],
                        "CF_law": wl["CF_law"], "CF_vs_ks": wl["CF_law"] / w49a.ks_mean_cf(RE_L) - 1.0,
                        "CF_report": None if cf_eff is None else cf_eff / 2.0 / r["depth"]}
            if row[arm]["CF_report"] is not None:
                row[arm]["report_vs_law_integral"] = row[arm]["CF_report"] / wl["CF_law"] - 1.0
            row[arm]["profile"] = r.get("profile")
            row[arm]["exit"] = r["exit"]
            row[arm]["iterations"] = r["iterations"]
        table[yp] = row
    ev["plate"] = table
    # LH1 / LH2: the default curve against the 1-D model shifted by D01's measured offset
    d01 = (table.get(1) or {}).get("D")
    if d01:
        # the model at each run's own wall-cell y+ at x = 0.97, shifted by D01's measured offset (D7)
        base = d01["vs_tmr"] - model_1d(d01["y_plus_097"], False)[1]
        lh1 = {}
        for yp in PLATES:
            d = table[yp]["D"]
            if d:
                pred = model_1d(d["y_plus_097"], False)[1] + base
                lh1[yp] = {"measured": d["vs_tmr"], "predicted": pred, "diff_points": 100 * (d["vs_tmr"] - pred)}
        ev["LH1"] = {"offset_from_D01": base, "rows": lh1, "band_points": 3.0,
                     "met": len(lh1) == len(PLATES) and all(abs(x["diff_points"]) <= 3.0 for x in lh1.values())}
    mx = [table[yp]["D"]["vs_tmr"] for yp in (5, 8, 12, 15) if table.get(yp, {}).get("D")]
    ev["LH2"] = {"max_default_error_yplus_5_to_15": max(mx) if mx else None,
                 "met": bool(mx) and len(mx) == 4 and max(mx) >= 0.10}
    # LP1-LP4: the opt-in (registered on E; amendment 1: the same limits as LPn_A1 on F)
    for arm, sfx in (("E", ""), ("F", "_A1")):
        e = {yp: table[yp][arm] for yp in PLATES}
        full = all(v is not None for v in e.values())
        ev["LP1" + sfx] = {"vs_tmr": {yp: (v or {}).get("vs_tmr") for yp, v in e.items()}, "band": 0.05,
                           "met": full and all(abs(v["vs_tmr"]) <= 0.05 for v in e.values())}
        if e.get(1):
            ref = e[1]["cf097_law"]
            ev["LP2" + sfx] = {f"vs_{arm}01": {yp: (v["cf097_law"] / ref - 1.0 if v else None) for yp, v in e.items()},
                               "band": 0.03,
                               "met": full and all(abs(v["cf097_law"] / ref - 1.0) <= 0.03 for v in e.values())}
        else:
            ev["LP2" + sfx] = {"met": False, "not_evaluated": True}
        ev["LP3" + sfx] = {"report_vs_law_integral": {yp: (v or {}).get("report_vs_law_integral")
                                                      for yp, v in e.items()},
                           "band": 0.02, "met": full and all(v.get("report_vs_law_integral") is not None
                                                             and abs(v["report_vs_law_integral"]) <= 0.02
                                                             for v in e.values())}
        ev["LP4" + sfx] = {"CF_vs_ks": {yp: (v or {}).get("CF_vs_ks") for yp, v in e.items()},
                           "ks": w49a.ks_mean_cf(RE_L), "band": 0.05,
                           "met": full and all(abs(v["CF_vs_ks"]) <= 0.05 for v in e.values())}
    # amendment 1 mechanism checks on F
    fp = {yp: table[yp]["F"] for yp in PLATES}
    ffull = all(v is not None and v.get("profile") for v in fp.values())
    ev["M1_A1"] = {"non_increasing": {yp: ((v or {}).get("profile") or {}).get("non_increasing") for yp, v in fp.items()},
                   "met": ffull and all(v["profile"]["non_increasing"] for v in fp.values())}
    fp5 = {yp: v for yp, v in fp.items() if yp >= 5}  # the second cell next to the law face in the buffer / log layer
    ev["M2_A1"] = {"second_cell_ratio": {yp: ((v or {}).get("profile") or {}).get("second_cell_nutilde_over_kappa_ut_y")
                                         for yp, v in fp5.items()}, "band": 1.10,
                   "met": ffull and all(v["profile"]["second_cell_nutilde_over_kappa_ut_y"] <= 1.10
                                        for v in fp5.values())}
    m3 = [f"F{yp:02d}" for yp in PLATES if yp >= 5] + ["AF15", "AF40"]
    ev["M3_A1"] = {"exit": {i: (runs.get(i) or {}).get("exit") for i in m3},
                   "met": all((runs.get(i) or {}).get("exit") == 0 for i in m3)}
    # aerofoil
    af = {}
    for yp in AF_MESH_OF:
        for arm in ("AD", "AE", "AF"):
            i = f"{arm}{yp:02d}"
            r = runs.get(i)
            if not ok(r) or w(i, "cd_eff") is None:
                af[i] = None
                continue
            af[i] = {k: w(i, k) for k in AF_KEYS}
            af[i]["cd_eff_vs_tmr"] = w(i, "cd_eff") / tmr["cd_alpha0"] - 1.0
            af[i]["friction_vs_tmr"] = w(i, "cd_veff") / tmr["cd_friction_alpha0"] - 1.0
            af[i]["pressure_vs_tmr"] = (w(i, "cd_eff") - w(i, "cd_veff")) / tmr["cd_pressure_alpha0"] - 1.0
            af[i]["wall_yplus"] = r.get("wall_yplus")
            af[i]["iterations"] = r["iterations"]
    ev["aerofoil"] = af
    for arm, sfx in (("AE", ""), ("AF", "_A1")):
        ae = [af.get(f"{arm}{yp:02d}") for yp in AF_MESH_OF]
        aefull = all(v is not None for v in ae)
        ev["LA1" + sfx] = {"cd_eff_vs_tmr": {k: (v or {}).get("cd_eff_vs_tmr") for k, v in af.items()
                                             if k.startswith(arm)},
                           "band": 0.06, "met": aefull and all(abs(v["cd_eff_vs_tmr"]) <= 0.06 for v in ae)}
        ev["LA2" + sfx] = {"friction_vs_tmr": {k: (v or {}).get("friction_vs_tmr") for k, v in af.items()
                                               if k.startswith(arm)},
                           "band": 0.06, "met": aefull and all(abs(v["friction_vs_tmr"]) <= 0.06 for v in ae)}
        if aefull:
            cds = [v["cd_eff"] for v in ae]
            ev["LA3" + sfx] = {"spread_over_tmr": (max(cds) - min(cds)) / tmr["cd_alpha0"], "band": 0.04,
                               "met": (max(cds) - min(cds)) / tmr["cd_alpha0"] <= 0.04}
        else:
            ev["LA3" + sfx] = {"met": False, "not_evaluated": True}
    ad05 = af.get("AD05")
    ev["LH3"] = {"AD05_friction_vs_tmr": (ad05 or {}).get("friction_vs_tmr"),
                 "met": ad05 is not None and ad05["friction_vs_tmr"] >= 0.08}
    ev["reported"] = {"schlichting_cf097": schlichting_cf(X / NU), "ks_mean_CF": w49a.ks_mean_cf(RE_L),
                      "tmr": tmr}
    return ev


def collect(work, outdir, ref49a, plots):
    runs = analyse(work, ref49a)
    tmr = w49a.tmr_friction()
    ev = evaluate(runs, tmr)
    os.makedirs(outdir, exist_ok=True)
    slim = {i: {k: v for k, v in r.items()} for i, r in runs.items()}
    with open(os.path.join(outdir, "results.json"), "w", encoding="utf-8", newline="\n") as f:
        json.dump({"runs": slim, "evaluation": ev}, f, indent=1)
    cols = ["id", "exit", "iterations", "wall_s", "y_plus_097", "cf097_law", "vs_tmr", "cd", "cd_eff", "cd_veff"]
    with open(os.path.join(outdir, "results.csv"), "w", encoding="utf-8", newline="\n") as f:
        wr = csv.writer(f, lineterminator="\n")
        wr.writerow(cols)
        for i, r in runs.items():
            wl = r.get("wall") or {}
            cf = wl.get("cf097_law")
            row = [i, r["exit"], r["iterations"], r["wall_s"],
                   "" if wl.get("y_plus_097") is None else f"{wl['y_plus_097']:.4g}",
                   "" if cf is None else f"{cf:.6g}", "" if cf is None else f"{cf / TMR_CF097 - 1.0:+.4f}"]
            for k in ("cd", "cd_eff", "cd_veff"):
                v = (r["window"].get(k) or {}).get("mean")
                row.append("" if v is None else f"{v:.6g}")
            wr.writerow(row)
    if plots:
        make_plot(ev, outdir)
    return ev


def make_plot(ev, outdir):
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    fig, ax = plt.subplots(figsize=(6.4, 3.8))
    ys = sorted(PLATES)
    ax.axhspan(-5, 5, color="0.9", label="LP1 band (TMR SA +-5 %)")
    for arm, lab, mk in (("D", "default (auto)", "o"), ("E", "opt-in as registered", "s"),
                         ("F", "opt-in, amendment 1", "^")):
        pts = [(ev["plate"][y][arm]["y_plus_097"], 100 * ev["plate"][y][arm]["vs_tmr"]) for y in ys
               if ev["plate"][y][arm]]
        if pts:
            ax.plot([a for a, _ in pts], [b for _, b in pts], marker=mk, label=lab)
    m = ev["model_1d"]
    ax.plot(ys, [100 * m[y]["default"] for y in ys], ":", color="C0", label="1-D model, default")
    ax.plot(ys, [100 * m[y]["opt_in"] for y in ys], ":", color="C1", label="1-D model, opt-in")
    ax.set_xscale("log")
    ax.set_xlabel("wall-cell y+ at x = 0.97")
    ax.set_ylabel("Cf(0.97) / TMR SA - 1  [%]")
    ax.legend(fontsize=7)
    fig.tight_layout()
    fig.savefig(os.path.join(outdir, "cf_vs_yplus.png"), dpi=80)
    plt.close(fig)


# ------------------------------------------------------------------------------------------------------------- selftest


def selftest():
    reg = registry()
    assert len(reg) == 3 * len(PLATES) + 9 + 3
    for i, spec in reg.items():
        t = case_text(i, spec)
        assert ("auto_sa_law" in t) == spec["opt_in"], i
        assert t.count("[turbulence.sa]") == (1 if spec["opt_in"] else 0), i
        if spec["kind"] == "plate":
            base = w49a.PLATE.format(yp=w48.fmt(spec["y_plus"]), id=i, ny=spec["ny"], layers=spec["layers"],
                                     n=PLATE_ITER)
            body = t[:-len(OPT_IN)] if spec["opt_in"] else t
            assert body.split("\n", 1)[1] == base.split("\n", 1)[1], i  # ADR 0164's plate below the title
            # layer stack 0.075 to 0.11 of the plate height with the flat-plate u_tau at x = 0.97
            ut = 1.0 * math.sqrt(0.5 * 0.0576 * (X / NU) ** -0.2)
            h1 = 2.0 * spec["y_plus"] * NU / ut
            stack = h1 * (1.15 ** spec["layers"] - 1.0) / 0.15
            assert 0.06 < stack < 0.12, (i, stack)
        else:
            f00 = w49a.case_text("F00", {"kind": "af", "arm": "SA", "alpha": 0, "mesh": "M1w",
                                         "fix_farfield": "freestream"}).split("\n", 1)[1]
            body = t[:-len(OPT_IN)] if spec["opt_in"] else t
            want = f00.replace("meshes/M1w\"", f"meshes/{spec['mesh']}\"").replace("mesh M1w (", f"mesh {spec['mesh']} (")
            assert body.split("\n", 2)[2] == want, i  # F00's text with the mesh name only
    # mesh first cells scale with the target y+
    f40 = w48.geometric_first(1.0e4, 128)
    for name, yp in AF_MESHES.items():
        f = w48.geometric_first(w48.MESHES[name]["grading"], 128)
        assert abs(f / f40 - yp / M1W_MEDIAN_YPLUS) < 1e-6, name
    # the SA inner law: log intercept of the model about 5.0, viscous limit
    assert abs(uplus_sa(0.5) - 0.5) < 1e-4
    b = uplus_sa(1e4) - math.log(1e4) / K
    assert 4.9 < b < 5.2, b
    assert abs(schlichting_cf(4.85e6) - 0.00288) < 2e-5
    print("selftest passed")


def main():
    a = sys.argv[1:]
    if not a:
        print(__doc__)
        return
    cmd = a[0]
    if cmd == "model":
        for yp, v in model_table().items():
            print(f"y+ {yp:3d}  default {100 * v['default']:+6.1f} %   opt-in {100 * v['opt_in']:+6.1f} %")
    elif cmd == "mesh":
        rcfd, work = a[1], a[2]
        for name in a[3:] or ["M1w", "M1w15", "M1w05"]:
            info = w48.build_mesh(rcfd, work, name)
            print(name, info["sha256"], info["first_cell_mid_chord"], info["y_plus_centre_estimate"],
                  info["quality"].get("max_non_orthogonality", info["quality"]))
    elif cmd == "cases":
        print(write_cases(a[1]))
    elif cmd == "run":
        par, ids, k = 1, [], 3
        while k < len(a):
            if a[k] == "--parallel":
                par = int(a[k + 1])
                k += 2
            else:
                ids.append(a[k])
                k += 1
        run_many(a[1], a[2], ids, par)
    elif cmd == "collect":
        ref = a[a.index("--ref49a") + 1] if "--ref49a" in a else None
        ev = collect(a[1], a[2], ref, "--plots" in a)
        print(json.dumps({k: v for k, v in ev.items() if k not in ("plate", "aerofoil", "model_1d")}, indent=1))
    elif cmd == "list":
        print("\n".join(registry()))
    elif cmd == "selftest":
        selftest()
    else:
        raise SystemExit(f"unknown command {cmd}")


if __name__ == "__main__":
    main()
