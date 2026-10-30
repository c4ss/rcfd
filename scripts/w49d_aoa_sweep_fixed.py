"""ADR 0167 (job 49D): ADR 0159's NACA 0012 angle-of-attack sweep again, with ADR 0164's two opt-in fixes and a far
field of 500 chords. Run on demand.

    python scripts/w49d_aoa_sweep_fixed.py mesh <rcfd> <work>                  M1w at 500 chords into <work>/meshes
    python scripts/w49d_aoa_sweep_fixed.py cases <work>                        every case of the registry to <work>/cases
    python scripts/w49d_aoa_sweep_fixed.py run <rcfd> <work> <id> ... [--parallel K]   run cases (one thread each)
    python scripts/w49d_aoa_sweep_fixed.py collect <work> <outdir> [--c10 <w49a-work>] [--plots]
                                                                               results.json, results.csv, histories.csv
    python scripts/w49d_aoa_sweep_fixed.py list | selftest

Arms (ADR 0167):
  SA-aNN   steady SA (ADR 0159 D3) on M1w-far500, both fixes: boundaries.farfield.turbulence = "freestream" (ADR 0164
           amendment 1) and the force reports cl_eff / cd_eff with component = "total_effective" (ADR 0164 D1); the
           molecular cl / cd (component = "total", written out) are reported next to them. Primary arm, 16 angles.
  N-aNN    the same with boundaries.farfield.turbulence = "zero_gradient" written out (today's `auto` for a freestream
           patch): ADR 0159's far-field treatment. With SA-aNN at the same angle this gives the four-way comparison:
           N cl/cd = old set-up, N cl_eff/cd_eff = effective force only, SA cl/cd = freestream role only, SA cl_eff/cd_eff
           = both. Angles 0, 10, 15, 18.
  SST-aNN  steady k-omega SST (ADR 0159 amendment 2: 4000 iterations, its 8 angles), both fixes; the far field's inflow
           values set by intensity 0.05 and viscosity ratio 10, the values of SST's initial field (report only).

Nothing is written outside <work> and <outdir>; an existing run folder is never overwritten. The explicit keys make the
cases mean the same thing whether or not the defaults of `component` and of the far-field turbulence role change later.
"""
import csv
import hashlib
import json
import math
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import w48y_aoa_sweep as w48  # noqa: E402  (ADR 0159 driver: mesh builder, case text, stream reader, slope, stall)
import w49a_sa_drag as w49a  # noqa: E402  (ADR 0164 driver: vtu reader, outer nuTilda, TMR files)

REPO = w48.REPO
TMR_CLCD = w49a.TMR_CLCD
REF48Y = os.path.join(REPO, "docs", "investigations", "job48y-aoa-sweep", "results.json")
REF49A = os.path.join(REPO, "docs", "investigations", "job49a-sa-drag", "results.json")

# ---------------------------------------------------------------------------------------------------- registry (ADR 0167)

FAR = 500.0
MESH = w48.far_field_variant("M1w", FAR)  # "M1w-far500": ADR 0164's F500w, the same points
MESH_SHA_EXPECTED = {  # ADR 0159 amendment 1 (M1w) and ADR 0164 D3 (F500w)
    "M1w": "bd8ef3c6da2dd3681d38c18914c8fbdcddab77bb1381c0688e1e2dce5f0c0299",
    MESH: "59e707a866e34ec2fef6dd13a41de6411f844ceff99a2b6a644cb45f3a70c889",
}
ALPHAS = w48.ALPHAS + w48.EXTENSION  # D1: the extension always runs
BASE_ALPHAS = list(w48.ALPHAS)
FOURWAY = [0, 10, 15, 18]  # D4
SST_ALPHAS = list(w48.SST_ALPHAS)  # ADR 0159 amendment 2
SA_ITER = w48.STEADY_ITERATIONS  # 8000
SST_ITER = w48.SST_ITERATIONS  # 4000
WINDOW = 1000
STEADY_CL_RANGE = w48.STEADY_CL_RANGE  # 0.005
STEADY_CD_RANGE = w48.STEADY_CD_RANGE  # 0.0005
SLOPE_ALPHAS = w48.SLOPE_ALPHAS
SLOPE_BAND = w48.SLOPE_BAND  # (0.90, 1.05)
CL0_LIMIT = w48.CL0_LIMIT  # 0.005
DRIFT_LIMIT = 0.005  # LD: |relative change of cd_eff, last quarter against the quarter before| <= 0.5 %
TMR_BANDS = {  # LT: alpha -> (cd relative band, cl relative band or None)
    0: (0.12, None),
    10: (0.15, 0.05),
    15: (0.25, 0.05),
}
WALL_CAP = 6 * 3600


def rid(arm, alpha):
    return f"{arm}-a{alpha:02d}"


def registry():
    out = {}
    for a in ALPHAS:
        out[rid("SA", a)] = {"arm": "SA", "alpha": a, "farfield": "freestream"}
    for a in FOURWAY:
        out[rid("N", a)] = {"arm": "N", "alpha": a, "farfield": "zero_gradient"}
    for a in SST_ALPHAS:
        out[rid("SST", a)] = {"arm": "SST", "alpha": a, "farfield": "freestream_sst"}
    return out


# --------------------------------------------------------------------------------------------------------------- cases

FARFIELD = 'pressure = "freestream"\n'
FARFIELD_ROLE = {
    "freestream": (
        'pressure = "freestream"\n'
        'turbulence = "freestream"   # ADR 0164 amendment 1 (opt-in): nuTilda = 3 nu on the inflow faces, zero gradient '
        'on the outflow faces\n'),
    "zero_gradient": (
        'pressure = "freestream"\n'
        'turbulence = "zero_gradient"   # ADR 0167: ADR 0159\'s far-field turbulence (today\'s auto for a freestream '
        'patch), written out\n'),
    "freestream_sst": (
        'pressure = "freestream"\n'
        'turbulence = "freestream"   # ADR 0164 amendment 1 (opt-in): k, omega fixed on the inflow faces, zero gradient '
        'on the outflow faces\n'
        'turbulence_spec = "intensity_viscosity_ratio"   # ADR 0167: the default intensity_length with no length gives '
        'omega of about 1e29\n'
        'turbulence_intensity = 0.05   # = turbulence.initial_intensity (default): the initial field\n'
        'turbulence_viscosity_ratio = 10.0   # = turbulence.initial_viscosity_ratio (default)\n'),
}
REPORT_TAIL = "u_ref = 1.0\n"
COMPONENT_TOTAL = 'u_ref = 1.0\ncomponent = "total"   # ADR 0167: the molecular wall shear (today\'s default), written out\n'

EXTRA_REPORTS = """
# ADR 0167: the effective force (ADR 0164 D1), the curve's values; and the parts of cd (reports do not change the solution)
[reports.cl_eff]
type = "force_coefficient"
patch = "airfoil"
direction = [0.0, 1.0, 0.0]
a_ref = 0.1
u_ref = 1.0
component = "total_effective"

[reports.cd_eff]
type = "force_coefficient"
patch = "airfoil"
direction = [1.0, 0.0, 0.0]
a_ref = 0.1
u_ref = 1.0
component = "total_effective"

[reports.cd_p]
type = "force_coefficient"
patch = "airfoil"
direction = [1.0, 0.0, 0.0]
a_ref = 0.1
u_ref = 1.0
component = "pressure"

[reports.cd_veff]
type = "force_coefficient"
patch = "airfoil"
direction = [1.0, 0.0, 0.0]
a_ref = 0.1
u_ref = 1.0
component = "viscous_effective"
"""

NT_REPORT = """
[reports.nut_avg]
type = "volume_average"
field = "nut"
"""

ARM_TITLE = {
    "SA": "steady SA, both fixes (freestream far-field role, effective force)",
    "N": "steady SA, ADR 0159 far field (zero gradient), effective force reported",
    "SST": "steady SST, both fixes (report only)",
}


def case_text(i, spec):
    base_spec = {"arm": "SST" if spec["arm"] == "SST" else "SA", "alpha": spec["alpha"], "mesh": MESH}
    if spec["arm"] == "SST":
        base_spec["iterations"] = SST_ITER
    text = w48.case_text(base_spec)
    title, rest = text.split("\n", 1)
    assert title.startswith("title = ")
    title = f'title = "NACA 0012, alpha {spec["alpha"]} deg, {ARM_TITLE[spec["arm"]]}, mesh {MESH} (ADR 0167, job 49D)"'
    assert rest.count(FARFIELD) == 1 and rest.count(REPORT_TAIL) == 2
    rest = rest.replace(FARFIELD, FARFIELD_ROLE[spec["farfield"]]).replace(REPORT_TAIL, COMPONENT_TOTAL)
    text = f"# ADR 0167 (job 49D) run {i}; built on ADR 0159's case text\n{title}\n{rest}{EXTRA_REPORTS}"
    if spec["arm"] != "SST":
        text += NT_REPORT
    return text


def write_cases(work):
    cdir = os.path.join(work, "cases")
    os.makedirs(cdir, exist_ok=True)
    for i, spec in registry().items():
        with open(os.path.join(cdir, i + ".toml"), "w", encoding="utf-8", newline="\n") as f:
            f.write(case_text(i, spec))
    return cdir


# ----------------------------------------------------------------------------------------------------------------- run


def command(rcfd, work, i):
    cases = os.path.join(work, "cases")
    run = os.path.join(work, "runs", i)
    if os.path.exists(run):
        raise SystemExit(f"{run} exists; refusing to overwrite a run")
    os.makedirs(os.path.join(run, "output"))
    with open(os.path.join(cases, i + ".toml"), encoding="utf-8") as f:
        text = f.read()
    if text != case_text(i, registry()[i]):
        raise SystemExit(f"{i}: the case file differs from the registry's text; rewrite the cases")
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


def window_len(n):
    """D5: the steady window, the last min(1000, floor(n/2)) iterations of a run of n iterations (ADR 0164 amendment 1)."""
    return min(WINDOW, n // 2)


def mean_of(its, key, lo, hi):
    """Mean of monitor `key` over iterations lo < index <= hi."""
    v = [e["monitors"][key] for e in its if lo < e["index"] <= hi and key in e.get("monitors", {})]
    return sum(v) / len(v) if v else None


def drift(its, key, n):
    """D6 / LD: (mean over (3n/4, n] minus mean over (n/2, 3n/4]) / mean over (3n/4, n]."""
    late = mean_of(its, key, 3 * n // 4, n)
    prev = mean_of(its, key, n // 2, 3 * n // 4)
    if late is None or prev is None or late == 0.0:
        return None
    return (late - prev) / late


def summary(its, exit_code):
    """D5: window statistics of every monitor; the value is the final iterate for a run that met the residual targets
    (exit 0) and the window mean for a run at its cap (exit 5); steady iff the window ranges of cl_eff and cd_eff are
    within ADR 0159 D4's limits."""
    n = its[-1]["index"] if its else 0
    nw = window_len(n)
    keys = sorted({k for e in its[-1:] for k in e.get("monitors", {})})
    win = {}
    for k in keys:
        v = [e["monitors"][k] for e in its if e["index"] > n - nw and k in e.get("monitors", {})]
        if v:
            win[k] = {"mean": sum(v) / len(v), "min": min(v), "max": max(v), "n": len(v),
                      "final": its[-1]["monitors"][k]}
    value = {k: (w["final"] if exit_code == 0 else w["mean"]) for k, w in win.items()}
    steady = (nw > 0 and "cl_eff" in win and win["cl_eff"]["max"] - win["cl_eff"]["min"] <= STEADY_CL_RANGE
              and win["cd_eff"]["max"] - win["cd_eff"]["min"] <= STEADY_CD_RANGE)
    out = {"iterations": n, "window_iterations": nw, "window": win, "value": value, "steady": bool(steady),
           "value_rule": "final iterate (exit 0)" if exit_code == 0 else "window mean (exit 5 or other)"}
    out["drift_cd_eff"] = drift(its, "cd_eff", n)
    out["drift_cd"] = drift(its, "cd", n)
    la, pr = mean_of(its, "cl_eff", 3 * n // 4, n), mean_of(its, "cl_eff", n // 2, 3 * n // 4)
    out["drift_cl_eff_abs"] = (la - pr) if la is not None and pr is not None else None
    return out


def mesh_info(work):
    out = {}
    for name in ("M1w", MESH):
        p = os.path.join(work, "meshes", name + ".json")
        if os.path.exists(p):
            with open(p, encoding="utf-8") as f:
                m = json.load(f)
            q = m.get("quality", {})
            out[name] = {"sha256": m["sha256"], "expected": MESH_SHA_EXPECTED[name],
                         "equal": m["sha256"] == MESH_SHA_EXPECTED[name], "grading": m["params"]["grading"],
                         "far": m["params"]["far"], "first_cell_mid_chord": m["first_cell_mid_chord"],
                         "max_non_orthogonality": q.get("max_non_orthogonality"), "n_cells": q.get("n_cells")}
    return out


def read_runs(work):
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
        r = {"id": i, **spec, "exit": wall["exit"], "wall_s": wall["process_wall_s"],
             "state": (fin or {}).get("state"), "finish_reason": (fin or {}).get("reason"),
             "warnings": sorted({w.get("kind") or w.get("rule") or w.get("message", "")[:60] for w in warnings})}
        r["complete"] = wall["exit"] in (0, 5)
        r.update(summary(its, wall["exit"]) if its else {"iterations": 0})
        vtu = os.path.join(rdir, i, "output", "solution.vtu")
        if spec["arm"] in ("SA", "N") and os.path.exists(vtu):
            r["outer_nutilde"] = w49a.freestream_nutilde(vtu, w48.NU)
        r["history"] = [(e["index"], e["monitors"].get("cl_eff"), e["monitors"].get("cd_eff"))
                        for e in its if "cl_eff" in e.get("monitors", {})]
        r["_its"] = its
        runs[i] = r
    return runs


def tmr_table():
    out = {}
    with open(TMR_CLCD, encoding="utf-8") as f:
        for row in csv.reader(f):
            if row and not row[0].startswith("#") and row[0] != "alpha_deg":
                out[int(round(float(row[0])))] = {"cl": float(row[1]), "cd": float(row[2])}
    return out


def c10_compare(its, w49a_work):
    """Report only (D8): every iteration's cl, cd and cd_eff of SA-a10 against ADR 0164's C10 status stream."""
    p = os.path.join(w49a_work, "runs", "C10", "solver-status.jsonl")
    if not os.path.exists(p):
        return None
    ref, _, _ = w48.read_stream(p)
    by = {e["index"]: e["monitors"] for e in ref}
    n, diff, first, maxrel = 0, 0, None, 0.0
    for e in its:
        r = by.get(e["index"])
        if r is None:
            continue
        for k in ("cl", "cd", "cd_eff"):
            if k in r and k in e["monitors"]:
                n += 1
                a, b = e["monitors"][k], r[k]
                if a != b:
                    diff += 1
                    maxrel = max(maxrel, abs(a - b) / max(abs(b), 1e-300))
                    if first is None:
                        first = [e["index"], k, a, b]
    return {"compared": n, "different": diff, "first_difference": first, "max_rel_difference": maxrel,
            "iterations_this": its[-1]["index"] if its else 0, "iterations_c10": ref[-1]["index"] if ref else 0}


def analyse(runs, work, w49a_work=None):
    reg = registry()
    sa = {r["alpha"]: r for r in runs.values() if r["arm"] == "SA"}
    nn = {r["alpha"]: r for r in runs.values() if r["arm"] == "N"}
    sst = {r["alpha"]: r for r in runs.values() if r["arm"] == "SST"}
    res = {"mesh": mesh_info(work)}
    res["P2"] = {"met": bool(res["mesh"]) and all(m["equal"] for m in res["mesh"].values()) and len(res["mesh"]) == 2}
    # D7 the curve: SA, effective values
    curve = []
    for a in sorted(sa):
        r = sa[a]
        if "value" not in r:
            continue
        w = r["window"]
        # D7: a steady angle enters with its D5 value; an angle not steady by D5 with its window mean, flagged (the
        # driver of the registration commit took the D5 value in both cases; corrected to the ADR's text, see its
        # addendum)
        v = r["value"] if r["steady"] else {k: w[k]["mean"] for k in ("cl_eff", "cd_eff", "cl", "cd")}
        curve.append({"alpha": a, "cl": v["cl_eff"], "cd": v["cd_eff"],
                      "cl_molecular": v["cl"], "cd_molecular": v["cd"],
                      "cl_final": w["cl_eff"]["final"], "cd_final": w["cd_eff"]["final"],
                      "cl_min": w["cl_eff"]["min"], "cl_max": w["cl_eff"]["max"], "cd_min": w["cd_eff"]["min"],
                      "cd_max": w["cd_eff"]["max"], "steady": r["steady"], "exit": r["exit"],
                      "iterations": r["iterations"],
                      "source": "SA steady" if r["steady"] else "SA iteration mean (not steady, flagged)"})
    res["curve"] = curve
    # L1
    missing = sorted(i for i in reg if i not in runs)
    res["L1"] = {"counted": len(runs), "incomplete": sorted(i for i, r in runs.items() if not r["complete"]),
                 "not_run": missing, "met": all(r["complete"] for r in runs.values()) and bool(runs)}
    sa_missing = [i for i in missing if reg[i]["arm"] == "SA"]
    if sa_missing:
        res["L1"]["met"] = False
    # L2
    pts = [(p["alpha"], p["cl"]) for p in curve if p["alpha"] in SLOPE_ALPHAS]
    if len(pts) == len(SLOPE_ALPHAS):
        b, icpt = w48.slope_fit(pts)
        res["L2"] = {"slope_per_rad": b, "slope_per_deg": b * math.pi / 180.0,
                     "factor": b / (2 * math.pi), "intercept": icpt, "band": list(SLOPE_BAND),
                     "met": SLOPE_BAND[0] <= b / (2 * math.pi) <= SLOPE_BAND[1]}
    # L3
    res["L3_base"] = w48.stall([(p["alpha"], p["cl"]) for p in curve if p["alpha"] in BASE_ALPHAS])
    res["L3"] = w48.stall([(p["alpha"], p["cl"]) for p in curve])
    res["L3"]["met"] = res["L3"]["detected"]
    # L4
    c0 = next((p for p in curve if p["alpha"] == 0), None)
    if c0:
        res["L4"] = {"cl0": c0["cl"], "limit": CL0_LIMIT, "met": abs(c0["cl"]) <= CL0_LIMIT}
    # LT against TMR SA (CFL3D)
    tmr = tmr_table()
    res["tmr"] = tmr
    for a, (cd_band, cl_band) in TMR_BANDS.items():
        c = next((p for p in curve if p["alpha"] == a), None)
        if c is None or a not in tmr:
            continue
        t = tmr[a]
        e = {"cd": c["cd"], "cd_tmr": t["cd"], "cd_rel": c["cd"] / t["cd"] - 1.0, "cd_band": cd_band,
             "cl": c["cl"], "cl_tmr": t["cl"]}
        met = abs(e["cd_rel"]) <= cd_band
        if cl_band is not None:
            e["cl_rel"] = c["cl"] / t["cl"] - 1.0
            e["cl_band"] = cl_band
            met = met and abs(e["cl_rel"]) <= cl_band
        e["met"] = met
        res[f"LT{a}"] = e
    # LD drift of the primary arm
    dr = {str(a): sa[a].get("drift_cd_eff") for a in sorted(sa)}
    res["LD"] = {"drift_cd_eff": dr, "limit": DRIFT_LIMIT,
                 "over": sorted((int(a) for a, d in dr.items() if d is None or abs(d) > DRIFT_LIMIT)),
                 "met": bool(dr) and all(d is not None and abs(d) <= DRIFT_LIMIT for d in dr.values())}
    # reported: the four-way comparison (D4)
    ref48 = {}
    if os.path.exists(REF48Y):
        with open(REF48Y, encoding="utf-8") as f:
            for p in json.load(f)["evaluation"]["curve"]:
                ref48[p["alpha"]] = {"cl": p["cl"], "cd": p["cd"]}
    res["adr0159_curve"] = {str(a): v for a, v in sorted(ref48.items())}
    four = {}
    for a in FOURWAY:
        if a in sa and a in nn and "value" in sa[a] and "value" in nn[a]:
            vs, vn = sa[a]["value"], nn[a]["value"]
            four[str(a)] = {
                "old": {"cl": vn["cl"], "cd": vn["cd"]},
                "effective_only": {"cl": vn["cl_eff"], "cd": vn["cd_eff"]},
                "freestream_only": {"cl": vs["cl"], "cd": vs["cd"]},
                "both": {"cl": vs["cl_eff"], "cd": vs["cd_eff"]},
                "adr0159_50_chords": ref48.get(a),
                "tmr": tmr.get(a),
                "N_iterations": nn[a]["iterations"], "N_exit": nn[a]["exit"], "N_steady": nn[a]["steady"],
                "N_drift_cd": nn[a].get("drift_cd"), "N_outer_nutilde": (nn[a].get("outer_nutilde") or {}).get("median"),
                "SA_outer_nutilde": (sa[a].get("outer_nutilde") or {}).get("median"),
                "cd_pressure": {"N": vn.get("cd_p"), "SA": vs.get("cd_p")},
            }
    res["fourway"] = four
    # reported: SST
    sst_curve = []
    for a in sorted(sst):
        r = sst[a]
        if "value" in r:
            v = r["value"] if r["steady"] else {k: r["window"][k]["mean"] for k in ("cl_eff", "cd_eff", "cl", "cd")}
            sst_curve.append({"alpha": a, "cl": v["cl_eff"], "cd": v["cd_eff"],
                              "cl_molecular": v["cl"], "cd_molecular": v["cd"],
                              "cl_min": r["window"]["cl_eff"]["min"], "cl_max": r["window"]["cl_eff"]["max"],
                              "steady": r["steady"], "exit": r["exit"], "iterations": r["iterations"],
                              "drift_cd_eff": r.get("drift_cd_eff")})
    res["sst_curve"] = sst_curve
    if sst_curve:
        res["sst_stall"] = w48.stall([(p["alpha"], p["cl"]) for p in sst_curve])
        spts = [(p["alpha"], p["cl"]) for p in sst_curve if p["alpha"] in SLOPE_ALPHAS]
        if len(spts) >= 3:
            res["sst_slope_factor"] = w48.slope_fit(spts)[0] / (2 * math.pi)
            res["sst_slope_alphas"] = [a for a, _ in spts]
    # reported: Ladson
    lad = w48.ladson()
    lad_curve = [(a, c) for a, c, _ in lad]
    res["ladson_80grit"] = {"cl_max": max(c for _, c in lad_curve),
                            "alpha_cl_max": max(lad_curve, key=lambda t: t[1])[0],
                            "slope_factor": w48.slope_fit([(a, c) for a, c in lad_curve if -4.1 <= a <= 8.31])[0]
                            / (2 * math.pi)}
    res["vs_ladson"] = [{"alpha": p["alpha"], "cl": p["cl"], "cl_ladson": w48.interp(lad, p["alpha"], 0),
                         "cd": p["cd"], "cd_ladson": w48.interp(lad, p["alpha"], 1)} for p in curve]
    # reported: outer nuTilda of every SA run, SA-a10 against ADR 0164's C10
    res["outer_nutilde_median"] = {i: (r.get("outer_nutilde") or {}).get("median") for i, r in sorted(runs.items())
                                   if r["arm"] in ("SA", "N")}
    if w49a_work and rid("SA", 10) in runs:
        res["c10_bits"] = c10_compare(runs[rid("SA", 10)]["_its"], w49a_work)
    return res


def write_outputs(runs, res, outdir, plots):
    os.makedirs(outdir, exist_ok=True)
    slim = {i: {k: v for k, v in r.items() if k not in ("history", "_its")} for i, r in runs.items()}
    with open(os.path.join(outdir, "results.json"), "w", encoding="utf-8", newline="\n") as f:
        json.dump({"adr": "0167", "runs": slim, "evaluation": res}, f, indent=1, sort_keys=True)
        f.write("\n")
    cols = ["cl_eff", "cd_eff", "cl", "cd", "cd_p", "cd_veff"]
    with open(os.path.join(outdir, "results.csv"), "w", encoding="utf-8", newline="") as f:
        w = csv.writer(f, lineterminator="\n")
        w.writerow(["id", "arm", "alpha_deg", "farfield", "exit", "iterations", "steady", *cols, "cl_eff_min",
                    "cl_eff_max", "drift_cd_eff", "outer_nutilde_median", "wall_s"])
        for i, r in sorted(runs.items()):
            v = r.get("value", {})
            wn = r.get("window", {})
            on = (r.get("outer_nutilde") or {}).get("median")
            w.writerow([i, r["arm"], r["alpha"], r["farfield"], r["exit"], r.get("iterations"), r.get("steady"),
                        *(f"{v[k]:.6g}" if k in v else "" for k in cols),
                        f"{wn['cl_eff']['min']:.6g}" if "cl_eff" in wn else "",
                        f"{wn['cl_eff']['max']:.6g}" if "cl_eff" in wn else "",
                        "" if r.get("drift_cd_eff") is None else f"{r['drift_cd_eff']:.3e}",
                        "" if on is None else f"{on:.4g}", f"{r['wall_s']:.0f}"])
    with open(os.path.join(outdir, "histories.csv"), "w", encoding="utf-8", newline="") as f:
        w = csv.writer(f, lineterminator="\n")
        w.writerow(["id", "index", "cl_eff", "cd_eff"])
        for i, r in sorted(runs.items()):
            h = r["history"]
            stride = max(1, math.ceil(len(h) / 60))  # at most 61 samples per run (B1)
            sel = h[::stride]
            if h and sel[-1] is not h[-1]:
                sel.append(h[-1])
            for idx, cl, cd in sel:
                w.writerow([i, idx, f"{cl:.6g}", f"{cd:.6g}"])
    if plots:
        make_plots(runs, res, outdir)


def make_plots(runs, res, outdir):
    import matplotlib  # only here: the rest of the driver needs the standard library only

    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    lad = w48.ladson()
    tmr = res["tmr"]
    curve = res["curve"]
    old = sorted((int(a), v) for a, v in res.get("adr0159_curve", {}).items())
    fig, ax = plt.subplots(figsize=(6.4, 4.4), dpi=80)
    ax.plot([a for a, _, _ in lad], [c for _, c, _ in lad], "s", mfc="none", color="0.35", ms=5,
            label="Ladson 1988, Re 6e6, 80 grit (experiment)")
    ax.plot(list(tmr), [t["cl"] for t in tmr.values()], "*", color="k", ms=9, label="TMR SA (CFL3D)")
    if old:
        ax.plot([a for a, _ in old], [v["cl"] for _, v in old], ":", color="0.55", lw=1.2,
                label="ADR 0159 SA (50 c, no fixes)")
    if res.get("sst_curve"):
        sc = res["sst_curve"]
        ax.errorbar([p["alpha"] for p in sc], [p["cl"] for p in sc],
                    yerr=[[p["cl"] - p["cl_min"] for p in sc], [p["cl_max"] - p["cl"] for p in sc]], fmt="v--",
                    mfc="none", color="#b07aa1", lw=1.0, capsize=2, label="SST, both fixes (report only)")
    xs = [p["alpha"] for p in curve]
    ax.errorbar(xs, [p["cl"] for p in curve], yerr=[[p["cl"] - p["cl_min"] for p in curve],
                                                     [p["cl_max"] - p["cl"] for p in curve]],
                fmt="o-", color="#4e79a7", lw=1.6, ms=4, capsize=3, label="SA, both fixes, 500 c (this job)")
    flag = [p for p in curve if not p["steady"]]
    if flag:
        ax.plot([p["alpha"] for p in flag], [p["cl"] for p in flag], "x", color="#e15759", ms=8,
                label="SA not steady (iteration mean)")
    ax.plot([0, 20], [0, 2 * math.pi * math.radians(20)], ":", color="0.7", lw=1, label="2 pi alpha")
    ax.set_xlabel("angle of attack alpha [deg]")
    ax.set_ylabel("lift coefficient cl")
    ax.set_title("NACA 0012, Re 6e6: cl(alpha) (ADR 0167)")
    ax.set_ylim(-0.1, 2.4)
    ax.grid(alpha=0.3)
    ax.legend(fontsize=6, loc="upper left")
    fig.tight_layout()
    fig.savefig(os.path.join(outdir, "cl_alpha.png"))
    plt.close(fig)

    fig, ax = plt.subplots(figsize=(6.4, 4.4), dpi=80)
    ax.plot([cd for _, _, cd in lad], [c for _, c, _ in lad], "s", mfc="none", color="0.35", ms=5,
            label="Ladson 1988, 80 grit")
    ax.plot([t["cd"] for t in tmr.values()], [t["cl"] for t in tmr.values()], "*", color="k", ms=9,
            label="TMR SA (CFL3D)")
    if old:
        ax.plot([v["cd"] for _, v in old], [v["cl"] for _, v in old], ":", color="0.55", lw=1.2,
                label="ADR 0159 SA (50 c, no fixes)")
    ax.plot([p["cd"] for p in curve], [p["cl"] for p in curve], "o-", color="#4e79a7", ms=4,
            label="SA, both fixes, 500 c")
    if res.get("sst_curve"):
        sc = res["sst_curve"]
        ax.plot([p["cd"] for p in sc], [p["cl"] for p in sc], "v--", mfc="none", color="#b07aa1", lw=1.0,
                label="SST, both fixes (report only)")
    for p in curve:
        ax.annotate(str(p["alpha"]), (p["cd"], p["cl"]), fontsize=6, xytext=(3, -3), textcoords="offset points")
    ax.set_xscale("log")
    ax.set_xlabel("drag coefficient cd (log)")
    ax.set_ylabel("lift coefficient cl")
    ax.set_title("NACA 0012, Re 6e6: drag polar (ADR 0167)")
    ax.grid(alpha=0.3, which="both")
    ax.legend(fontsize=6, loc="lower right")
    fig.tight_layout()
    fig.savefig(os.path.join(outdir, "polar.png"))
    plt.close(fig)

    # iteration histories of the SA runs that are not steady by D5, and of the N runs (the old far field)
    sel = sorted((r for r in runs.values() if (r["arm"] == "SA" and not r.get("steady", True)) or r["arm"] == "N"),
                 key=lambda r: (r["arm"], r["alpha"]))
    if sel:
        fig, (ax, ax2) = plt.subplots(2, 1, figsize=(6.4, 5.6), dpi=72, sharex=True)
        for r in sel:
            h = r["history"]
            stride = max(1, len(h) // 80)  # B1: the PNG at most 120 000 bytes
            ls = "-" if r["arm"] == "SA" else "--"
            ax.plot([i for i, _, _ in h[::stride]], [c for _, c, _ in h[::stride]], ls, lw=0.6, label=r["id"])
            ax2.plot([i for i, _, _ in h[::stride]], [d for _, _, d in h[::stride]], ls, lw=0.6, label=r["id"])
        ax.set_ylabel("cl_eff")
        ax.set_ylim(-0.5, 2.5)
        ax.set_title("SA runs not steady by D5 (solid) and the old far field N (dashed) (ADR 0167)", fontsize=9)
        ax.grid(alpha=0.3)
        ax.legend(fontsize=6, ncol=4)
        ax2.set_ylabel("cd_eff")
        ax2.set_yscale("log")
        ax2.set_xlabel("iteration")
        ax2.grid(alpha=0.3, which="both")
        fig.tight_layout()
        fig.savefig(os.path.join(outdir, "histories.png"))
        plt.close(fig)


# ------------------------------------------------------------------------------------------------------------ selftest


def selftest():
    reg = registry()
    assert len([s for s in reg.values() if s["arm"] == "SA"]) == 16
    assert MESH == "M1w-far500" and w48.MESHES[MESH]["far"] == 500.0
    assert w48.MESHES[MESH]["grading"] == w48.MESHES["F500w"]["grading"]  # ADR 0164's F500w
    for i, spec in reg.items():
        t = case_text(i, spec)
        assert t.count('component = "total"   #') == 2, i
        assert t.count('component = "total_effective"') == 2, i
        assert f'file = "../meshes/{MESH}"' in t, i
        assert ('turbulence = "freestream"' in t) == (spec["arm"] in ("SA", "SST")), i
        assert ('turbulence = "zero_gradient"' in t) == (spec["arm"] == "N"), i
        assert ('model = "k_omega_sst"' in t) == (spec["arm"] == "SST"), i
        n = SST_ITER if spec["arm"] == "SST" else SA_ITER
        assert f"max_iterations = {n}\n" in t, i
        # the case is ADR 0159's text apart from the title, the far-field lines and the reports
        base = w48.case_text({"arm": "SST" if spec["arm"] == "SST" else "SA", "alpha": spec["alpha"], "mesh": MESH,
                              **({"iterations": SST_ITER} if spec["arm"] == "SST" else {})})
        body = base.split("\n", 1)[1].replace(FARFIELD, FARFIELD_ROLE[spec["farfield"]]).replace(REPORT_TAIL,
                                                                                                COMPONENT_TOTAL)
        assert body in t, i
    # windows, value rule, drift
    assert window_len(8000) == 1000 and window_len(533) == 266
    its = [{"index": k, "monitors": {"cl_eff": 1.0, "cd_eff": 0.01 * (1.0 + 0.1 / k), "cl": 1.0, "cd": 0.005}}
           for k in range(1, 801)]
    s = summary(its, 0)
    assert s["value"]["cd_eff"] == its[-1]["monitors"]["cd_eff"] and s["steady"]
    s5 = summary(its, 5)
    assert abs(s5["value"]["cd_eff"] - sum(e["monitors"]["cd_eff"] for e in its[400:]) / 400) < 1e-15
    d = drift(its, "cd_eff", 800)
    late = sum(0.01 * (1 + 0.1 / k) for k in range(601, 801)) / 200
    prev = sum(0.01 * (1 + 0.1 / k) for k in range(401, 601)) / 200
    assert abs(d - (late - prev) / late) < 1e-15 and d < 0
    its2 = [{"index": k, "monitors": {"cl_eff": 1.0 + 0.01 * math.sin(k), "cd_eff": 0.01, "cl": 1.0, "cd": 0.005}}
            for k in range(1, 3001)]
    assert not summary(its2, 5)["steady"]
    t = tmr_table()
    assert set(t) >= {0, 10, 15} and abs(t[10]["cl"] - 1.0909) < 1e-4
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
        for name in ("M1w", MESH):
            info = w48.build_mesh(a[1], a[2], name)
            print(name, info["sha256"], "expected", MESH_SHA_EXPECTED[name], info["first_cell_mid_chord"])
    elif cmd == "cases":
        print(write_cases(a[1]))
    elif cmd == "run":
        args = a[1:]
        par = 2
        if "--parallel" in args:
            k = args.index("--parallel")
            par = int(args[k + 1])
            del args[k:k + 2]
        run_many(args[0], args[1], args[2:], par)
    elif cmd == "collect":
        c10 = a[a.index("--c10") + 1] if "--c10" in a else None
        runs = read_runs(a[1])
        res = analyse(runs, a[1], c10)
        write_outputs(runs, res, a[2], "--plots" in a)
        print(json.dumps({k: res.get(k) for k in ("P2", "L1", "L2", "L3", "L4", "LT0", "LT10", "LT15", "LD")},
                         indent=1))
    else:
        raise SystemExit(__doc__)


if __name__ == "__main__":
    main()
