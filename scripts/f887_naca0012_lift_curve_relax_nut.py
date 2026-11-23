"""ADR 0772 (F-887): ADR 0760's twelve steady NACA 0012 cases with turbulence.relax_nut = 1.0 added.

    python scripts/f887_naca0012_lift_curve_relax_nut.py ids                      the twelve run ids RN-a00 ... RN-a17
    python scripts/f887_naca0012_lift_curve_relax_nut.py cases <dir>              the twelve case texts
    python scripts/f887_naca0012_lift_curve_relax_nut.py run <rcfd> <work> <ids...> [--parallel 3]
    python scripts/f887_naca0012_lift_curve_relax_nut.py collect <work> <outdir> [--binary <rcfd>] [--commit <sha>]
    python scripts/f887_naca0012_lift_curve_relax_nut.py selftest

Each case is ADR 0760's committed case of its angle (docs/investigations/f868-naca0012-lift-curve/cases/) with the head
comment and the title reworded and one line added after `model = "spalart_allmaras"`: relax_nut = 1.0. The mesh is made
by ADR 0167's driver: python scripts/w49d_aoa_sweep_fixed.py mesh <rcfd> <work>. Each run is judged by ADR 0167's D5 (its
driver's `summary`, unchanged) and D7 (the curve value: the D5 value when steady, the window mean otherwise), then:

    A   steady by D5 and abs(cl - ref) <= 0.018, ref ADR 0760's steady value (0 to 16) or ADR 0764's 1.5516 (17)
    V   flow value (exit 0 or steady) and abs(cl - cl_L) <= max(0.05 abs(cl_L), 0.018)   (ADR 0760's band)
    VS  flow value and abs(cl - cl_L) <= 0.018                                           (the scatter alone)

Nothing is written outside <work> and <outdir>; a run folder is never overwritten.
"""
import json
import os
import re
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import w48y_aoa_sweep as w48  # noqa: E402  (ADR 0159 driver: the status stream reader, Ladson's table)
import w49d_aoa_sweep_fixed as w49d  # noqa: E402  (ADR 0167 driver: D5 summary, run command line, mesh)
import f868_naca0012_lift_curve as f868  # noqa: E402  (ADR 0760 driver: the band)

REPO = w48.REPO
CASES760 = os.path.join(REPO, "docs", "investigations", "f868-naca0012-lift-curve", "cases")
RESULTS760 = os.path.join(REPO, "docs", "investigations", "f868-naca0012-lift-curve", "results.json")
RESULTS770 = os.path.join(REPO, "docs", "investigations", "f884-naca0012-17-steady", "results.json")

ALPHAS = f868.ALPHAS  # ADR 0760: 0, 2, 4, 6, 8, 10, 12, 13, 14, 15, 16, 17
TARGET17 = 1.5516  # ADR 0764, U17: the time mean of cl_eff at 17 degrees
TOL = 0.018  # acceptance of F-887
SCATTER = f868.SCATTER  # 0.018, ADR 0760 S1
CL_MAX_LADSON = f868.CL_MAX_LADSON  # 1.6116
CL_MAX_BAND = f868.CL_MAX_BAND  # 0.05
SLOPE_ALPHAS = [0, 2, 4, 6, 8]  # ADR 0167 / ADR 0760 L2
SLOPE_BAND = (0.90, 1.05)
SYMMETRY = 0.005  # L4
HISTORY_EVERY = 50
ANCHOR = 'model = "spalart_allmaras"\n'
ADDED = 'relax_nut = 1.0   # ADR 0772 RN: the eddy viscosity not relaxed (ADR 0770 KT)\n'
REPRO = {"RN-a15": "KT-a15", "RN-a17": "KT-a17"}  # D15, D17


def rid(alpha):
    return f"RN-a{alpha:02d}"


def ids():
    return [rid(a) for a in ALPHAS]


def alpha_of(i):
    return int(i.split("-a")[1])


def base_text(alpha):
    with open(os.path.join(CASES760, f"SA-a{alpha:02d}.toml"), encoding="utf-8") as f:
        return f.read().replace("\r\n", "\n")


def case_text(i):
    alpha = alpha_of(i)
    lines = base_text(alpha).split("\n", 2)
    assert lines[0].startswith("# ADR 0167") and lines[1].startswith("title = "), i
    head = f"# ADR 0772 (F-887) run {i}: ADR 0760's case SA-a{alpha:02d}.toml with relax_nut = 1.0 added"
    title = (f'title = "NACA 0012, alpha {alpha} deg, steady SA, mesh M1w-far500, ADR 0760\'s set-up with the eddy '
             f'viscosity not relaxed (ADR 0772, F-887)"')
    body = lines[2]
    assert body.count(ANCHOR) == 1, i
    return head + "\n" + title + "\n" + body.replace(ANCHOR, ANCHOR + ADDED)


def write_cases(folder):
    os.makedirs(folder, exist_ok=True)
    for i in ids():
        with open(os.path.join(folder, i + ".toml"), "w", encoding="utf-8", newline="\n") as f:
            f.write(case_text(i))
    return folder


def command(rcfd, work, i):
    cases = os.path.join(work, "cases")
    run = os.path.join(work, "runs", i)
    if os.path.exists(run):
        raise SystemExit(f"{run} exists; refusing to overwrite a run")
    with open(os.path.join(cases, i + ".toml"), encoding="utf-8") as f:
        text = f.read()
    if text != case_text(i):
        raise SystemExit(f"{i}: the case file differs from the registered text; rewrite the cases")
    os.makedirs(os.path.join(run, "output"))
    with open(os.path.join(run, "case.snapshot"), "w", encoding="utf-8", newline="\n") as f:
        f.write(text)
    cmd = [rcfd, "run", os.path.join(run, "case.snapshot"), "--case-dir", cases, "--out", os.path.join(run, "output"),
           "--status-file", os.path.join(run, "solver-status.jsonl"), "--status-every", "1", "--quiet",
           "--threads", "1", "--store-dir", os.path.join(run, "fields"), "--status-monitors",
           "--max-elapsed", str(w49d.WALL_CAP)]
    with open(os.path.join(run, "cmd.txt"), "w", encoding="utf-8") as f:
        f.write(json.dumps(cmd, indent=1) + "\n")
    return run, cmd


def run_many(rcfd, work, run_ids, parallel):
    unknown = [i for i in run_ids if i not in ids()]
    if unknown:
        raise SystemExit(f"unknown ids {unknown}")
    queue, live = list(run_ids), []
    while queue or live:
        while queue and len(live) < parallel:
            i = queue.pop(0)
            run, cmd = command(rcfd, work, i)
            log = open(os.path.join(run, "log.txt"), "ab")
            t0 = time.time()
            p = subprocess.Popen(cmd, cwd=run, stdin=subprocess.DEVNULL, stdout=log, stderr=log)
            live.append((i, run, p, log, t0))
            print(f"started {i} pid {p.pid} at {time.strftime('%H:%M:%S')}", flush=True)
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


def references():
    """Row A: ADR 0760's steady values (its evaluation.angles cl) at 0 to 16 degrees, ADR 0764's 1.5516 at 17."""
    with open(RESULTS760, encoding="utf-8") as f:
        ev = json.load(f)["evaluation"]["angles"]
    ref = {r["alpha"]: r["cl"] for r in ev if r["alpha"] != 17}
    ref[17] = TARGET17
    return ref


def verdict(window, exit_code, alpha, ref, table):
    """D5, D7 and the rows A, V and VS of one angle from its window statistics."""
    rcl = window["cl_eff"]["max"] - window["cl_eff"]["min"]
    rcd = window["cd_eff"]["max"] - window["cd_eff"]["min"]
    steady = rcl <= w49d.STEADY_CL_RANGE and rcd <= w49d.STEADY_CD_RANGE
    d5 = window["cl_eff"]["final"] if exit_code == 0 else window["cl_eff"]["mean"]
    d7 = d5 if steady else window["cl_eff"]["mean"]
    flow = exit_code == 0 or steady
    cl_l = f868.cl_ladson(alpha, table)
    b = f868.band(alpha, table)
    return {"range_cl_eff": rcl, "range_cd_eff": rcd, "steady": bool(steady), "d5_value": d5, "cl": d7,
            "flow_value": bool(flow), "reference_A": ref, "difference_A": d5 - ref,
            "A_met": bool(steady and abs(d5 - ref) <= TOL),
            "cl_ladson": cl_l, "band": b, "difference_ladson": d7 - cl_l,
            "V_met": bool(flow and abs(d7 - cl_l) <= b), "VS_met": bool(flow and abs(d7 - cl_l) <= SCATTER)}


def read_run(work, i, ref, table):
    rdir = os.path.join(work, "runs", i)
    its, fin, warnings = w48.read_stream(os.path.join(rdir, "solver-status.jsonl"))
    with open(os.path.join(rdir, "wall.json"), encoding="utf-8") as f:
        wall = json.load(f)
    s = w49d.summary(its, wall["exit"])
    alpha = alpha_of(i)
    r = {"id": i, "alpha": alpha, "exit": wall["exit"], "wall_s": wall["process_wall_s"],
         "finished": fin is not None, "state": (fin or {}).get("state"), "finish_reason": (fin or {}).get("reason"),
         "warnings": sorted({w.get("rule") or w.get("message", "")[:60] for w in warnings}),
         "iterations": s["iterations"], "window_iterations": s["window_iterations"], "value_rule": s["value_rule"],
         "window": {k: s["window"][k] for k in ("cl_eff", "cd_eff")}, "value": {k: s["value"][k] for k in ("cl_eff", "cd_eff")},
         "history": [[e["index"], e["monitors"]["cl_eff"], e["monitors"]["cd_eff"]] for e in its
                     if e["index"] % HISTORY_EVERY == 0 or e["index"] == s["iterations"]],
         "case": case_text(i)}
    with open(os.path.join(rdir, "case.snapshot"), encoding="utf-8") as f:
        if f.read() != r["case"]:
            raise SystemExit(f"{i}: the run's case snapshot differs from the registered text")
    r["verdict"] = verdict(r["window"], r["exit"], alpha, ref[alpha], table)
    assert r["verdict"]["steady"] == s["steady"], i
    return r


def slope_factor(points):
    """L2: least squares with intercept of cl against alpha in radians over 0 to 8 degrees, as a factor of 2 pi."""
    import math
    xs = [math.radians(a) for a, _ in points]
    ys = [c for _, c in points]
    n = len(xs)
    mx, my = sum(xs) / n, sum(ys) / n
    k = sum((x - mx) * (y - my) for x, y in zip(xs, ys)) / sum((x - mx) ** 2 for x in xs)
    return k / (2 * math.pi)


def evaluate(runs):
    ev = {}
    flow = [(r["alpha"], r["verdict"]["cl"]) for r in runs.values() if r["verdict"]["flow_value"]]
    if flow:
        a, top = max(flow, key=lambda p: p[1])
        rel = top / CL_MAX_LADSON - 1.0
        ev["VM"] = {"alpha": a, "cl_max": top, "relative": rel, "met": abs(rel) <= CL_MAX_BAND}
    pts = [(r["alpha"], r["verdict"]["cl"]) for r in runs.values() if r["alpha"] in SLOPE_ALPHAS]
    if len(pts) == len(SLOPE_ALPHAS):
        k = slope_factor(sorted(pts))
        ev["L2"] = {"factor": k, "met": SLOPE_BAND[0] <= k <= SLOPE_BAND[1]}
    if rid(0) in runs:
        c0 = runs[rid(0)]["verdict"]["cl"]
        ev["L4"] = {"cl0": c0, "met": abs(c0) <= SYMMETRY}
    ev["R1"] = {"run": sorted(runs), "not_run": [i for i in ids() if i not in runs],
                "finished": sorted(i for i, r in runs.items() if r["finished"]),
                "met": all(i in runs and runs[i]["finished"] for i in ids())}
    with open(RESULTS770, encoding="utf-8") as f:
        kt = json.load(f)["runs"]
    for i, k in REPRO.items():
        if i not in runs:
            continue
        a, b = runs[i], kt[k]
        d = {"iterations": a["iterations"] == b["iterations"], "exit": a["exit"] == b["exit"]}
        for m in ("cl_eff", "cd_eff"):
            for q in ("mean", "min", "max", "final"):
                d[f"{m}_{q}"] = a["window"][m][q] == b["window"][m][q]
        ev["D" + str(alpha_of(i))] = {"against": k, "fields": d, "equal_bits": all(d.values())}
    ev["A_missed"] = [r["alpha"] for r in runs.values() if not r["verdict"]["A_met"]]
    ev["V_missed"] = [r["alpha"] for r in runs.values() if not r["verdict"]["V_met"]]
    ev["VS_missed"] = [r["alpha"] for r in runs.values() if not r["verdict"]["VS_met"]]
    return ev


def compact(text):
    """One line for each history point [i, cl, cd], to keep the record small."""
    num = r"-?\d+(?:\.\d+)?(?:[eE][-+]?\d+)?"
    return re.sub(rf"\[\s+({num}),\s+({num}),\s+({num})\s+\]", r"[\1, \2, \3]", text)


def collect(work, outdir, binary=None, commit=None):
    ref, table = references(), w48.ladson()
    runs = {}
    for i in ids():
        if os.path.exists(os.path.join(work, "runs", i, "wall.json")):
            runs[i] = read_run(work, i, ref, table)
    mesh = w49d.mesh_info(work).get(w49d.MESH)
    out = {"adr": "0772", "item": "F-887", "setting": "turbulence.relax_nut = 1.0", "tolerance_A": TOL,
           "target17": TARGET17, "scatter": SCATTER,
           "d5": {"cl_range": w49d.STEADY_CL_RANGE, "cd_range": w49d.STEADY_CD_RANGE, "window": w49d.WINDOW},
           "binary": {"sha256": f868.sha256_of(binary) if binary else None, "commit": commit},
           "mesh": mesh, "evaluation": evaluate(runs), "runs": runs}
    os.makedirs(os.path.join(outdir, "cases"), exist_ok=True)
    write_cases(os.path.join(outdir, "cases"))
    with open(os.path.join(outdir, "results.json"), "w", encoding="utf-8", newline="\n") as f:
        f.write(compact(json.dumps(out, indent=1, sort_keys=True)) + "\n")
    for i, r in runs.items():
        v = r["verdict"]
        print(f"{i}: exit {r['exit']} it {r['iterations']} cl {v['cl']:.6f} (D5 {v['d5_value']:.6f}) "
              f"range cl {v['range_cl_eff']:.3e} cd {v['range_cd_eff']:.3e} steady {v['steady']} "
              f"A {v['difference_A']:+.5f} {v['A_met']} V {v['difference_ladson']:+.4f} {v['V_met']} VS {v['VS_met']}")
    print(json.dumps({k: v for k, v in out["evaluation"].items()}, default=str)[:2000])
    return out


def selftest():
    for i in ids():
        t = case_text(i)
        a = alpha_of(i)
        base = base_text(a).split("\n")
        new = t.split("\n")
        added = [ln for ln in new[2:] if ln not in base[2:]]
        removed = [ln for ln in base[2:] if ln not in new[2:]]
        assert not removed and added == [ADDED.rstrip("\n")], (i, added, removed)
        assert ("rotate_z_deg = 0.0 " if a == 0 else f"rotate_z_deg = -{a} ") in t, i
    # the body at 15 and 17 equals ADR 0770's KT cases apart from the comment of the added line
    for i, k in REPRO.items():
        with open(os.path.join(os.path.dirname(RESULTS770), "cases", k + ".toml"), encoding="utf-8") as f:
            kt = f.read().split("\n")[2:]
        strip = lambda ls: [ln.split("#")[0].rstrip() if ln.startswith("relax_nut") else ln for ln in ls]
        assert strip(kt) == strip(case_text(i).split("\n")[2:]), i
    ref, table = references(), w48.ladson()
    assert ref[17] == TARGET17 and abs(ref[15] - 1.4932034) < 1e-7 and len(ref) == 12
    w = {"cl_eff": {"min": 1.058, "max": 1.062, "mean": 1.06, "final": 1.061},
         "cd_eff": {"min": 0.04, "max": 0.0404, "mean": 0.0402, "final": 0.0403}}
    v = verdict(w, 0, 10, 1.061, table)
    assert v["steady"] and v["A_met"] and v["d5_value"] == 1.061 and v["V_met"] and v["VS_met"], v
    v = verdict(w, 5, 10, 1.061, table)
    assert v["d5_value"] == 1.06 and v["A_met"]
    assert not verdict(w, 0, 10, 1.061 + 0.0185, table)["A_met"]
    w["cl_eff"]["max"] = 1.064
    v = verdict(w, 0, 10, 1.061, table)
    assert not v["steady"] and not v["A_met"] and v["flow_value"] and v["cl"] == 1.06 and v["V_met"], v
    v = verdict(w, 5, 10, 1.061, table)
    assert not v["flow_value"] and not v["V_met"]
    assert abs(slope_factor([(a, 0.108074057624087 * a + 0.00305) for a in SLOPE_ALPHAS]) - 0.9855172) < 1e-6
    assert compact('[\n 1,\n 0.5,\n -1e-05\n]') == "[1, 0.5, -1e-05]"
    print("selftest ok")


def main():
    a = sys.argv[1:]
    if not a:
        print(__doc__)
        return
    opt = lambda k: a[a.index(k) + 1] if k in a else None
    if a[0] == "ids":
        print(" ".join(ids()))
    elif a[0] == "cases":
        print(write_cases(a[1]))
    elif a[0] == "run":
        par = int(opt("--parallel") or 3)
        rest = [x for x in a[3:] if x != "--parallel" and x != opt("--parallel")]
        run_many(a[1], a[2], rest, par)
    elif a[0] == "collect":
        collect(a[1], a[2], opt("--binary"), opt("--commit"))
    elif a[0] == "selftest":
        selftest()
    else:
        raise SystemExit(__doc__)


if __name__ == "__main__":
    main()
