"""ADR 0760 (F-868): the NACA 0012 lift curve of ADR 0167's SA arm on current main, against Ladson's measured lift.

    python scripts/f868_naca0012_lift_curve.py ids                      the twelve run ids (ADR 0167's SA arm, 0 to 17 deg)
    python scripts/f868_naca0012_lift_curve.py cases <dir>              the twelve case texts, as ADR 0167's driver writes them
    python scripts/f868_naca0012_lift_curve.py collect <work> <outdir> [--binary <rcfd>] [--commit <sha>]
                                                                        results.json and the case files into <outdir>
    python scripts/f868_naca0012_lift_curve.py selftest

ADR 0764 (F-868), the unsteady arm at 17 degrees with its control at 15 (time-accurate SA on pimple, same mesh):

    python scripts/f868_naca0012_lift_curve.py ucases <dir>             the two case texts U-a15, U-a17 (add --dt1 for the fallback pair)
    python scripts/f868_naca0012_lift_curve.py urun <rcfd> <work> [ids] the runs, side by side, one thread each
    python scripts/f868_naca0012_lift_curve.py ucollect <work> <outdir> [--binary <rcfd>] [--commit <sha>] [--dt1]
                                                                        the judgement, results.json and the case files

The mesh, the cases and the runs are made by ADR 0167's driver, unchanged, in a work folder outside the repository:

    python scripts/w49d_aoa_sweep_fixed.py mesh <rcfd> <work>
    python scripts/w49d_aoa_sweep_fixed.py cases <work>
    python scripts/w49d_aoa_sweep_fixed.py run <rcfd> <work> SA-a00 ... SA-a17 --parallel 3

`collect` takes the curve of that driver (its D5 and D7: cl_eff, the final iterate of a run that met the residual targets,
the window mean of a run at its cap, an angle not steady by D5 flagged) and judges it by ADR 0760's band:

    abs(cl - cl_L(alpha)) <= max(0.05 abs(cl_L(alpha)), 0.018)

with cl_L Ladson's 80-grit lift (validation-data/reference/w31c_naca0012_ladson1988_80grit_cl_cd.csv) interpolated
linearly. A value that is neither from a run that met the residual targets nor steady by D5 is a miss whatever it is.
Nothing is written outside <outdir>.
"""
import hashlib
import json
import math
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import w48y_aoa_sweep as w48  # noqa: E402  (ADR 0159 driver: Ladson's table, interpolation)
import w49d_aoa_sweep_fixed as w49d  # noqa: E402  (ADR 0167 driver: registry, case text, run reader, the curve)

REPO = w48.REPO
REF49D = os.path.join(REPO, "docs", "investigations", "job49d-aoa-sweep-fixed", "results.json")

ALPHAS = [0, 2, 4, 6, 8, 10, 12, 13, 14, 15, 16, 17]  # ADR 0760: the angles compared
NOT_COMPARED = [18, 20, 22, 24]  # ADR 0760: beyond the measured maximum lift, no steady value; not run
SCATTER = 0.018  # ADR 0760: the largest spread of Ladson's three grit zones from 0 to 17 degrees (0.01769), rounded up
RELATIVE = 0.05  # ADR 0167 LT10 / LT15: cl within 5 %
CL_MAX_LADSON = 1.6116  # the 80-grit row at 17.13 degrees
CL_MAX_BAND = 0.05  # VM


def ids():
    return [w49d.rid("SA", a) for a in ALPHAS]


def cl_ladson(alpha, table=None):
    return w48.interp(table or w48.ladson(), float(alpha), 0)


def band(alpha, table=None):
    return max(RELATIVE * abs(cl_ladson(alpha, table)), SCATTER)


def case_texts():
    reg = w49d.registry()
    return {i: w49d.case_text(i, reg[i]) for i in ids()}


def write_cases(folder):
    os.makedirs(folder, exist_ok=True)
    for i, text in case_texts().items():
        with open(os.path.join(folder, i + ".toml"), "w", encoding="utf-8", newline="\n") as f:
            f.write(text)
    return folder


def judge(curve):
    """ADR 0760's rows V00 to V17 and VM from the curve points [{alpha, cl, steady, exit, ...}] of ADR 0167's driver."""
    table = w48.ladson()
    rows = []
    for a in ALPHAS:
        p = next((q for q in curve if q["alpha"] == a), None)
        ref, b = cl_ladson(a, table), band(a, table)
        row = {"alpha": a, "cl_ladson": ref, "band": b, "run": p is not None}
        if p is not None:
            flow = p["exit"] == 0 or bool(p["steady"])
            d = p["cl"] - ref
            row.update({"cl": p["cl"], "cl_final": p["cl_final"], "cl_min": p["cl_min"], "cl_max": p["cl_max"],
                        "cd": p["cd"], "exit": p["exit"], "steady": bool(p["steady"]), "iterations": p["iterations"],
                        "flow_value": flow, "difference": d, "within_band": abs(d) <= b,
                        "within_scatter": abs(d) <= SCATTER, "met": flow and abs(d) <= b})
        else:
            row["met"] = False
        rows.append(row)
    out = {"angles": rows, "missed": [r["alpha"] for r in rows if not r["met"]],
           "within_scatter": [r["alpha"] for r in rows if r.get("within_scatter") and r.get("flow_value")]}
    flow_rows = [r for r in rows if r.get("flow_value")]
    if flow_rows:
        top = max(flow_rows, key=lambda r: r["cl"])
        rel = top["cl"] / CL_MAX_LADSON - 1.0
        out["VM"] = {"cl_max": top["cl"], "alpha": top["alpha"], "cl_max_ladson": CL_MAX_LADSON, "relative": rel,
                     "band": CL_MAX_BAND, "met": abs(rel) <= CL_MAX_BAND}
    return out


def against_adr0167(curve):
    """D1 (report): cl, iterations and exit code of each angle against ADR 0167's committed curve."""
    if not os.path.exists(REF49D):
        return None
    with open(REF49D, encoding="utf-8") as f:
        ref = {p["alpha"]: p for p in json.load(f)["evaluation"]["curve"]}
    out = []
    for p in curve:
        r = ref.get(p["alpha"])
        if r is None or p["alpha"] not in ALPHAS:
            continue
        out.append({"alpha": p["alpha"], "cl": p["cl"], "cl_adr0167": r["cl"], "cl_difference": p["cl"] - r["cl"],
                    "cl_final": p["cl_final"], "cl_final_adr0167": r["cl_final"],
                    "cl_final_difference": p["cl_final"] - r["cl_final"],
                    "cd": p["cd"], "cd_adr0167": r["cd"], "iterations": p["iterations"],
                    "iterations_adr0167": r["iterations"], "exit": p["exit"], "exit_adr0167": r["exit"],
                    "equal_bits": p["cl"] == r["cl"] and p["cd"] == r["cd"] and p["iterations"] == r["iterations"]})
    return out


def sha256_of(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def collect(work, outdir, binary=None, commit=None):
    runs = w49d.read_runs(work)
    mine = {i: r for i, r in runs.items() if i in ids()}
    res = w49d.analyse(mine, work)
    curve = [p for p in res["curve"] if p["alpha"] in ALPHAS]
    ev = judge(curve)
    ev["curve"] = curve
    ev["L2"], ev["L4"] = res.get("L2"), res.get("L4")
    mesh = res["mesh"].get(w49d.MESH)
    ev["P2"] = {"mesh": w49d.MESH, **(mesh or {}), "met": bool(mesh) and mesh["equal"]}
    ev["R1"] = {"runs": len(mine), "complete": sorted(i for i, r in mine.items() if r["complete"]),
                "not_run": sorted(i for i in ids() if i not in mine),
                "met": len(mine) == len(ids()) and all(r["complete"] for r in mine.values())}
    ev["D1"] = against_adr0167(curve)
    ev["not_compared"] = NOT_COMPARED
    texts = case_texts()
    for i in mine:
        with open(os.path.join(work, "runs", i, "case.snapshot"), encoding="utf-8") as f:
            if f.read() != texts[i]:
                raise SystemExit(f"{i}: the run's case snapshot differs from the registry's text")
    slim = {}
    for i, r in sorted(mine.items()):
        w = r.get("window", {})
        slim[i] = {"alpha": r["alpha"], "exit": r["exit"], "iterations": r.get("iterations"), "steady": r.get("steady"),
                   "wall_s": r["wall_s"], "state": r.get("state"), "finish_reason": r.get("finish_reason"),
                   "warnings": r.get("warnings"), "value_rule": r.get("value_rule"),
                   "window_iterations": r.get("window_iterations"),
                   "cl_eff": w.get("cl_eff"), "cd_eff": w.get("cd_eff"), "drift_cd_eff": r.get("drift_cd_eff"),
                   "outer_nutilde_median": (r.get("outer_nutilde") or {}).get("median")}
    out = {"adr": "0760", "item": "F-868", "model": "spalart_allmaras", "mesh": w49d.MESH,
           "band": {"relative": RELATIVE, "scatter": SCATTER, "cl_max_ladson": CL_MAX_LADSON, "cl_max_band": CL_MAX_BAND},
           "binary": {"sha256": sha256_of(binary) if binary else None, "commit": commit},
           "evaluation": ev, "runs": slim, "cases": {i: texts[i] for i in ids()}}
    os.makedirs(outdir, exist_ok=True)
    with open(os.path.join(outdir, "results.json"), "w", encoding="utf-8", newline="\n") as f:
        json.dump(out, f, indent=1, sort_keys=True)
        f.write("\n")
    write_cases(os.path.join(outdir, "cases"))
    return out


# ------------------------------------------------------------------- ADR 0764: the unsteady arm at 17 degrees (F-868)

U_ALPHAS = [15, 17]  # ADR 0764: the control and the angle of ADR 0760's known miss
U_CONTROL, U_ANGLE = 15, 17
U_DT = [0.02, 0.01]  # the registered step and the one fallback
U_END = 200.0  # c/U
U_SAMPLE = 0.1  # the samples of the mean: every fifth step (every tenth of the fallback)
U_WINDOW = (100.0, 200.0)  # the second half
U_SETTLED = 0.018  # abs(M2 - M1): the scatter of the measurement (ADR 0760, S1)
U_CONTROL_TOL = 0.018  # abs(M(15) - steady value of ADR 0760)
U_STEADY_RANGE = 0.005  # ADR 0167 D5: a cl range up to this is "steady in time"
U_MIN_STEPS_PER_PERIOD = 40  # below: reported as under-resolved
REF0760 = os.path.join(REPO, "docs", "investigations", "f868-naca0012-lift-curve", "results.json")

U_TABLES = """[solver]
algorithm = "pimple"
time_scheme = "bdf2"
n_outer = 3
n_correctors = 2
corrector_control = "adaptive"
convection = "linear_upwind"
dt = {dt}
end_time = 200.0

[output]
store_time_interval = 200.0
store_precision = "f32"
store_fields = ["ux", "uy", "p", "nut"]
"""
U_AFTER = "\n# ADR 0167: the effective force"


def uid(alpha, level=0):
    return f"U-a{alpha:02d}" + ("-dt1" if level else "")


def uids(level=0):
    return [uid(a, level) for a in U_ALPHAS]


def ucase_text(alpha, level=0):
    """ADR 0760's case text of the angle with the head, the title and the tables [solver] and [output] replaced."""
    steady = case_texts()[w49d.rid("SA", alpha)]
    head, title, rest = steady.split("\n", 2)
    assert head.startswith("# ADR 0167") and title.startswith("title = ")
    a, b = rest.index("[solver]\n"), rest.index(U_AFTER)
    assert rest.count("[solver]\n") == 1 and rest.count(U_AFTER) == 1 and rest.index("[output]\n") > a
    i = uid(alpha, level)
    return (f"# ADR 0764 (F-868) run {i}; ADR 0760's case SA-a{alpha:02d} with the tables [solver] and [output] replaced\n"
            f'title = "NACA 0012, alpha {alpha} deg, time-accurate SA (pimple, bdf2, dt {U_DT[level]}), both fixes, '
            f'mesh {w49d.MESH} (ADR 0764, F-868)"\n'
            + rest[:a] + U_TABLES.format(dt=U_DT[level]) + rest[b:])


def ucase_texts(level=0):
    return {uid(a, level): ucase_text(a, level) for a in U_ALPHAS}


def uwrite_cases(folder, level=0):
    os.makedirs(folder, exist_ok=True)
    for i, text in ucase_texts(level).items():
        with open(os.path.join(folder, i + ".toml"), "w", encoding="utf-8", newline="\n") as f:
            f.write(text)
    return folder


def urun(rcfd, work, ids):
    """The runs, started together, one thread each, no limit on wall time. A run folder that exists is not overwritten."""
    texts = {**ucase_texts(0), **ucase_texts(1)}
    cases = os.path.join(work, "ucases")
    os.makedirs(cases, exist_ok=True)
    live = []
    for i in ids:
        if i not in texts:
            raise SystemExit(f"unknown id {i}")
        run = os.path.join(work, "uruns", i)
        if os.path.exists(run):
            raise SystemExit(f"{run} exists; refusing to overwrite a run")
        os.makedirs(os.path.join(run, "output"))
        for path in (os.path.join(cases, i + ".toml"), os.path.join(run, "case.snapshot")):
            with open(path, "w", encoding="utf-8", newline="\n") as f:
                f.write(texts[i])
        cmd = [rcfd, "run", os.path.join(run, "case.snapshot"), "--case-dir", cases,
               "--out", os.path.join(run, "output"),
               "--status-file", os.path.join(run, "solver-status.jsonl"),
               "--status-every", "1", "--quiet", "--threads", "1",
               "--store-dir", os.path.join(run, "fields"), "--status-monitors"]
        with open(os.path.join(run, "cmd.txt"), "w", encoding="utf-8") as f:
            f.write(json.dumps(cmd, indent=1) + "\n")
        log = open(os.path.join(run, "log.txt"), "ab")
        t0 = time.time()
        p = subprocess.Popen(cmd, cwd=run, stdin=subprocess.DEVNULL, stdout=log, stderr=log)
        live.append((i, run, p, log, t0))
        print(f"started {i} pid {p.pid} at {time.strftime('%H:%M:%S')}", flush=True)
    while live:
        time.sleep(5)
        for item in list(live):
            i, run, p, log, t0 = item
            if p.poll() is not None:
                log.close()
                live.remove(item)
                wall = time.time() - t0
                with open(os.path.join(run, "wall.json"), "w", encoding="utf-8") as f:
                    json.dump({"id": i, "exit": p.returncode, "process_wall_s": wall, "started": t0}, f)
                print(f"finished {i} exit {p.returncode} after {wall:.1f} s", flush=True)


def tmean(ts, ys, lo, hi):
    """The mean over [lo, hi] of the piecewise-linear signal through the samples (the trapezoid rule)."""
    acc, span = 0.0, 0.0
    for k in range(len(ts) - 1):
        ta, tb, a, b = ts[k], ts[k + 1], ys[k], ys[k + 1]
        a0, b0 = max(ta, lo), min(tb, hi)
        if b0 <= a0:
            continue
        fa = a + (b - a) * (a0 - ta) / (tb - ta)
        fb = a + (b - a) * (b0 - ta) / (tb - ta)
        acc += 0.5 * (fa + fb) * (b0 - a0)
        span += b0 - a0
    return acc / span if span > 0 else None


def upward(ts, ys, level, lo, hi):
    """The times in [lo, hi] at which the signal crosses `level` upwards (linear interpolation between samples)."""
    out = []
    for k in range(len(ts) - 1):
        if ts[k] < lo or ts[k + 1] > hi:
            continue
        a, b = ys[k] - level, ys[k + 1] - level
        if a < 0 <= b:
            out.append(ts[k] + (ts[k + 1] - ts[k]) * (-a) / (b - a))
    return out


def ujudge(ts, cl, cd, alpha, whole_periods=True):
    """ADR 0764: the time mean over the window, the half means (whole periods) and whether the mean has settled, the
    range and the frequency. `ts`, `cl`, `cd` are the samples of the window."""
    eps = 1e-6
    lo, hi = ts[0], ts[-1]
    mid = 0.5 * (U_WINDOW[0] + U_WINDOW[1])
    covered = lo <= U_WINDOW[0] + eps and hi >= U_WINDOW[1] - eps
    m, md = tmean(ts, cl, lo, hi), tmean(ts, cd, lo, hi)
    halves = []
    for a, b in ((lo, mid), (mid, hi)):
        u = upward(ts, cl, m, a, b) if whole_periods else []
        if len(u) >= 2:
            a, b, rule, periods = u[0], u[-1], "whole periods", len(u) - 1
        else:
            rule, periods = "whole half", 0
        halves.append({"from": a, "to": b, "rule": rule, "periods": periods, "cl": tmean(ts, cl, a, b),
                       "cd": tmean(ts, cd, a, b)})
    drift = halves[1]["cl"] - halves[0]["cl"]
    rng = max(cl) - min(cl)
    out = {"alpha": alpha, "window": [lo, hi], "samples": len(ts), "covered": covered, "M": m, "Md": md,
           "M1": halves[0]["cl"], "M2": halves[1]["cl"], "Md1": halves[0]["cd"], "Md2": halves[1]["cd"],
           "halves": halves, "drift": drift, "settled_limit": U_SETTLED, "settled": covered and abs(drift) <= U_SETTLED,
           "cl_min": min(cl), "cl_max": max(cl), "range": rng, "cd_min": min(cd), "cd_max": max(cd),
           "oscillating": rng > U_STEADY_RANGE, "amplitude": 0.5 * rng}
    u = upward(ts, cl, m, lo, hi)
    out["crossings"] = len(u)
    if out["oscillating"] and len(u) >= 4:
        period = (u[-1] - u[0]) / (len(u) - 1)
        out.update({"period": period, "strouhal_chord": 1.0 / period,
                    "strouhal_projected": math.sin(math.radians(alpha)) / period})
    return out


def uverdict(j, complete, alpha=U_ANGLE):
    """The registered statement at 17 degrees."""
    ref, b = cl_ladson(alpha), band(alpha)
    d = j["M"] - ref
    within = abs(d) <= b
    if not complete:
        text = "the run is not complete"
    elif not j["settled"]:
        text = "not settled in the registered time"
    elif within:
        text = "within the band, " + ("an unsteady flow" if j["oscillating"] else "steady in time")
    elif d < 0:
        text = "premature stall of this model and mesh"
    else:
        text = "outside the band"
    return {"cl_ladson": ref, "band": b, "M": j["M"], "difference": d, "within_band": within, "settled": j["settled"],
            "complete": complete, "met": complete and j["settled"] and within, "verdict": text}


def ucontrol(j, complete, steady):
    d = j["M"] - steady
    return {"steady_cl": steady, "M": j["M"], "difference": d, "tolerance": U_CONTROL_TOL, "settled": j["settled"],
            "complete": complete, "holds": complete and j["settled"] and abs(d) <= U_CONTROL_TOL}


def steady_cl(alpha):
    with open(REF0760, encoding="utf-8") as f:
        return next(p["cl"] for p in json.load(f)["evaluation"]["curve"] if p["alpha"] == alpha)


def uread(work, i, level):
    run = os.path.join(work, "uruns", i)
    its, fin, warnings = w48.read_stream(os.path.join(run, "solver-status.jsonl"))
    with open(os.path.join(run, "wall.json"), encoding="utf-8") as f:
        wall = json.load(f)
    with open(os.path.join(run, "case.snapshot"), encoding="utf-8") as f:
        snapshot = f.read()
    dt = U_DT[level]
    on = lambda e, q: abs(e["time"] / q - round(e["time"] / q)) < 1e-6  # a step that ends on a multiple of q
    have = [e for e in its if "cl_eff" in e.get("monitors", {})]
    row = lambda e: [e["time"], e["monitors"]["cl_eff"], e["monitors"]["cd_eff"]]
    window = [row(e) for e in have if on(e, U_SAMPLE) and U_WINDOW[0] - 1e-6 <= e["time"] <= U_WINDOW[1] + 1e-6]
    start = [row(e) for e in have if on(e, 1.0) and e["time"] < U_WINDOW[0] - 1e-6]
    kinds = sorted({w.get("kind") or w.get("rule") or w.get("message", "")[:60] for w in warnings})
    cors = [e["correctors"] for e in its if "correctors" in e]
    t_end = its[-1]["time"] if its else 0.0
    reached = t_end >= U_END - 1e-6
    caps = sum(1 for e in its if e.get("corrector_cap"))
    r = {"id": i, "alpha": int(i[3:5]), "dt": dt, "exit": wall["exit"], "wall_s": wall["process_wall_s"],
         "steps": its[-1]["index"] if its else 0, "time_end": t_end, "reached": reached,
         "state": (fin or {}).get("state"), "finish_reason": (fin or {}).get("reason"), "warnings": kinds,
         "corrector_cap_warning": "corrector_cap" in kinds, "corrector_cap_steps": caps,
         "correctors": {"mean": sum(cors) / len(cors), "min": min(cors), "max": max(cors)} if cors else None,
         "complete": wall["exit"] == 0 and reached and "corrector_cap" not in kinds,
         "window_samples": window, "start_samples": start, "_snapshot": snapshot}
    if len(window) >= 3:
        ts, cl, cd = ([s[k] for s in window] for k in range(3))
        r["judgement"] = ujudge(ts, cl, cd, r["alpha"])
        if "period" in r["judgement"]:
            n = r["judgement"]["period"] / dt
            r["judgement"].update({"steps_per_period": n, "under_resolved": n < U_MIN_STEPS_PER_PERIOD})
    return r


def ucollect(work, outdir, binary=None, commit=None, level=0):
    texts = ucase_texts(level)
    runs = {}
    for i in uids(level):
        r = uread(work, i, level)
        if r.pop("_snapshot") != texts[i]:
            raise SystemExit(f"{i}: the run's case snapshot differs from the driver's text")
        runs[i] = r
    mesh = w49d.mesh_info(work).get(w49d.MESH)
    ev = {"P2": {"mesh": w49d.MESH, **(mesh or {}), "met": bool(mesh) and mesh["equal"]},
          "R1": {"complete": sorted(i for i, r in runs.items() if r["complete"]),
                 "met": all(r["complete"] for r in runs.values())}}
    c, u = runs[uid(U_CONTROL, level)], runs[uid(U_ANGLE, level)]
    if "judgement" in c:
        ev["C15"] = ucontrol(c["judgement"], c["complete"], steady_cl(U_CONTROL))
    if "judgement" in u:
        ev["U17"] = uverdict(u["judgement"], u["complete"])
    out = {"adr": "0764", "item": "F-868", "model": "spalart_allmaras", "mesh": w49d.MESH,
           "setup": {"algorithm": "pimple", "time_scheme": "bdf2", "n_outer": 3, "n_correctors": 2,
                     "corrector_control": "adaptive", "dt": U_DT[level], "end_time": U_END, "fallback_level": level,
                     "sample_interval": U_SAMPLE, "window": list(U_WINDOW)},
           "limits": {"band_17": band(U_ANGLE), "cl_ladson_17": cl_ladson(U_ANGLE), "settled": U_SETTLED,
                      "control": U_CONTROL_TOL, "steady_range": U_STEADY_RANGE},
           "binary": {"sha256": sha256_of(binary) if binary else None, "commit": commit},
           "counted": uids(level), "evaluation": ev, "runs": runs, "cases": texts}
    os.makedirs(outdir, exist_ok=True)
    with open(os.path.join(outdir, "results.json"), "w", encoding="utf-8", newline="\n") as f:
        json.dump(out, f, indent=None, sort_keys=True, separators=(",", ":"))
        f.write("\n")
    uwrite_cases(os.path.join(outdir, "cases"), level)
    return out


def uselftest():
    assert uids() == ["U-a15", "U-a17"] and uids(1) == ["U-a15-dt1", "U-a17-dt1"]
    steady = case_texts()
    for a in U_ALPHAS:
        for level in (0, 1):
            t, s = ucase_text(a, level), steady[w49d.rid("SA", a)]
            assert U_TABLES.format(dt=U_DT[level]) in t and f"dt = {U_DT[level]}\n" in t, a
            tl, sl = t.split("\n"), s.split("\n")
            ia, ib = sl.index("[solver]"), next(k for k, x in enumerate(sl) if x.startswith("# ADR 0167: the effective"))
            ja, jb = tl.index("[solver]"), next(k for k, x in enumerate(tl) if x.startswith("# ADR 0167: the effective"))
            assert tl[2:ja] == sl[2:ia] and tl[jb:] == sl[ib:], a  # every line outside the head and the two tables
            assert f"rotate_z_deg = {-a} " in t and 'algorithm = "pimple"' in t and "simplec" not in t
            assert "max_iterations" not in t and "des" not in t.split("[turbulence]")[1].split("[solver]")[0]
    ts = [100.0 + 0.1 * k for k in range(1001)]
    flat = [0.03] * len(ts)
    # a constant: settled, steady in time
    j = ujudge(ts, [1.6] * len(ts), flat, 17)
    assert j["settled"] and not j["oscillating"] and abs(j["M"] - 1.6) < 1e-12 and abs(j["drift"]) < 1e-12
    assert uverdict(j, True)["verdict"] == "within the band, steady in time" and uverdict(j, True)["met"]
    assert uverdict(j, False)["verdict"] == "the run is not complete" and not uverdict(j, False)["met"]
    # a ramp of 0.001 per time unit: half means 0.05 apart, not settled
    j = ujudge(ts, [1.0 + 0.001 * (t - 100.0) for t in ts], flat, 17)
    assert abs(j["drift"] - 0.05) < 1e-9 and not j["settled"]
    assert uverdict(j, True)["verdict"] == "not settled in the registered time"
    # a sine of ADR 0159's swing and period: settled with whole periods; the plain halves differ by more
    sine = [1.0 + 0.76 * math.sin(2 * math.pi * t / 4.2) for t in ts]
    j, plain = ujudge(ts, sine, flat, 17), ujudge(ts, sine, flat, 17, whole_periods=False)
    assert j["settled"] and abs(j["drift"]) < 1e-3 and abs(j["drift"]) < abs(plain["drift"]), (j["drift"], plain["drift"])
    assert j["oscillating"] and abs(j["period"] - 4.2) < 1e-3 and abs(j["amplitude"] - 0.76) < 5e-3
    assert abs(j["strouhal_chord"] - 1 / 4.2) < 1e-4
    assert uverdict(j, True)["verdict"] == "premature stall of this model and mesh"
    j = ujudge(ts, [1.62 + 0.05 * math.sin(2 * math.pi * t / 1.5) for t in ts], flat, 17)
    assert uverdict(j, True)["verdict"] == "within the band, an unsteady flow"
    assert uverdict(ujudge(ts, [1.75] * len(ts), flat, 17), True)["verdict"] == "outside the band"
    # the control: the steady value of ADR 0760 at 15 degrees, tolerance 0.018
    s15 = steady_cl(15)
    assert abs(s15 - 1.4932034) < 1e-6
    assert ucontrol(ujudge(ts, [s15 - 0.0052] * len(ts), flat, 15), True, s15)["holds"]
    assert not ucontrol(ujudge(ts, [s15 - 0.02] * len(ts), flat, 15), True, s15)["holds"]
    assert not ucontrol(ujudge(ts, [s15] * len(ts), flat, 15), False, s15)["holds"]
    # a window that does not reach t = 200 is not settled
    assert not ujudge(ts[:900], [1.6] * 900, flat[:900], 17)["settled"]


def selftest():
    assert ids() == ["SA-a00", "SA-a02", "SA-a04", "SA-a06", "SA-a08", "SA-a10", "SA-a12", "SA-a13", "SA-a14", "SA-a15",
                     "SA-a16", "SA-a17"]
    assert not set(ALPHAS) & set(NOT_COMPARED) and sorted(ALPHAS + NOT_COMPARED) == sorted(w49d.ALPHAS)
    # the table of ADR 0760: cl_L and the band at the twelve angles
    want = {0: (-0.0072, 0.0180), 2: (0.2071, 0.0180), 4: (0.4272, 0.0214), 6: (0.6448, 0.0322), 8: (0.8557, 0.0428),
            10: (1.0586, 0.0529), 12: (1.2493, 0.0625), 13: (1.3384, 0.0669), 14: (1.4189, 0.0709),
            15: (1.4938, 0.0747), 16: (1.5563, 0.0778), 17: (1.6057, 0.0803)}
    for a, (c, b) in want.items():
        assert abs(cl_ladson(a) - c) < 5e-5 and abs(band(a) - b) < 5e-5, (a, cl_ladson(a), band(a))
    for i, t in case_texts().items():
        a = int(i[-2:])
        assert 'model = "spalart_allmaras"' in t and 'turbulence = "freestream"' in t, i
        assert f'file = "../meshes/{w49d.MESH}"' in t and t.count('component = "total_effective"') == 2, i
        assert f"rotate_z_deg = {'0.0' if a == 0 else -a} " in t, i
    # the judgement: ADR 0167's recorded curve gives the differences ADR 0760 states, and 17 degrees is a miss
    with open(REF49D, encoding="utf-8") as f:
        old = [p for p in json.load(f)["evaluation"]["curve"] if p["alpha"] in ALPHAS]
    ev = judge(old)
    d = {r["alpha"]: r["difference"] for r in ev["angles"]}
    assert abs(d[2] - 0.0130) < 1e-4 and abs(d[16] + 0.0143) < 1e-4 and abs(d[17] + 0.558) < 1e-3, d
    assert ev["missed"] == [17] and not next(r for r in ev["angles"] if r["alpha"] == 17)["flow_value"]
    assert ev["VM"]["alpha"] == 16 and abs(ev["VM"]["relative"] + 0.043) < 1e-3
    # a value inside the band that is not a flow value is a miss
    fake = [dict(p) for p in old]
    for p in fake:
        if p["alpha"] == 17:
            p["cl"] = 1.60
    assert judge(fake)["missed"] == [17]
    uselftest()
    print("selftest: ok")


def main():
    a = sys.argv[1:]
    if not a:
        print(__doc__)
    elif a[0] == "ids":
        print(" ".join(ids()))
    elif a[0] == "cases":
        print(write_cases(a[1]))
    elif a[0] == "collect":
        opt = {k: a[a.index(k) + 1] for k in ("--binary", "--commit") if k in a}
        out = collect(a[1], a[2], opt.get("--binary"), opt.get("--commit"))
        ev = out["evaluation"]
        for r in ev["angles"]:
            if r["run"]:
                print(f'{r["alpha"]:3d}  cl {r["cl"]:.4f}  Ladson {r["cl_ladson"]:.4f}  diff {r["difference"]:+.4f}  '
                      f'band {r["band"]:.4f}  flow value {r["flow_value"]}  met {r["met"]}')
            else:
                print(f'{r["alpha"]:3d}  not run')
        print(json.dumps({k: ev.get(k) for k in ("missed", "within_scatter", "VM", "L2", "L4", "R1")}, indent=1))
    elif a[0] == "ucases":
        print(uwrite_cases(a[1], 1 if "--dt1" in a else 0))
    elif a[0] == "urun":
        urun(a[1], a[2], a[3:] or uids())
    elif a[0] == "ucollect":
        opt = {k: a[a.index(k) + 1] for k in ("--binary", "--commit") if k in a}
        out = ucollect(a[1], a[2], opt.get("--binary"), opt.get("--commit"), 1 if "--dt1" in a else 0)
        print(json.dumps(out["evaluation"], indent=1))
    elif a[0] == "selftest":
        selftest()
    else:
        raise SystemExit(__doc__)


if __name__ == "__main__":
    main()
