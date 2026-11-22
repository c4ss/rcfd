"""ADR 0770 (F-884): why the steady SIMPLEC iteration of ADR 0760 does not reach the attached NACA 0012 state at 17 degrees.

    python scripts/f884_naca0012_17_steady.py ids                      the run ids (K0, KU, KS, KT, KC, KG at 17; Kx-a15)
    python scripts/f884_naca0012_17_steady.py cases <dir>              the case texts (K0 is ADR 0760's SA-a17.toml)
    python scripts/f884_naca0012_17_steady.py run <rcfd> <work> <ids...> [--parallel 3]
    python scripts/f884_naca0012_17_steady.py collect <work> <outdir> [--binary <rcfd>] [--commit <sha>]
    python scripts/f884_naca0012_17_steady.py selftest

Each changed case is ADR 0760's committed case of its angle (docs/investigations/f868-naca0012-lift-curve/cases/) with
the title and the head comment reworded and only the registered lines added (ADR 0770, the table of causes). The mesh is
made by ADR 0167's driver: python scripts/w49d_aoa_sweep_fixed.py mesh <rcfd> <work>. Each run is judged by ADR 0167's D5
(its driver's `summary`, unchanged): steady when the window range of cl_eff <= 0.005 and of cd_eff <= 0.0005; a cause is
named when its run is steady; the target is abs(cl - 1.5516) <= 0.018 (ADR 0764) and the control at 15 degrees
abs(cl - 1.4932034) <= 0.018 (ADR 0760). Nothing is written outside <work> and <outdir>; a run folder is never overwritten.
"""
import json
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import w48y_aoa_sweep as w48  # noqa: E402  (ADR 0159 driver: the status stream reader)
import w49d_aoa_sweep_fixed as w49d  # noqa: E402  (ADR 0167 driver: D5 summary, run command line, mesh)

REPO = w48.REPO
CASES760 = os.path.join(REPO, "docs", "investigations", "f868-naca0012-lift-curve", "cases")
RESULTS760 = os.path.join(REPO, "docs", "investigations", "f868-naca0012-lift-curve", "results.json")

TARGET = 1.5516  # ADR 0764, U17: the time mean of cl_eff at 17 degrees
TOL = 0.018  # acceptance of F-884; ADR 0764 C15
CONTROL = 1.4932034  # ADR 0760: the steady cl_eff at 15 degrees
HISTORY_EVERY = 50

# ADR 0770, the table of causes: (anchor line in ADR 0760's case, the same line followed by the added lines)
SETTINGS = {
    "KU": ("the velocity under-relaxation", 'convection = "linear_upwind"\n',
           'convection = "linear_upwind"\nalpha_u = 0.7   # ADR 0770 KU: the SIMPLE default of SimpleSettings::new\n'),
    "KS": ("the start state", "[initial]\nvelocity = [1.0, 0.0, 0.0]\n",
           '[initial]\nvelocity = [1.0, 0.0, 0.0]\nmethod = "potential"   # ADR 0770 KS: the potential-flow start\n'),
    "KT": ("the coupling of the turbulence model", 'model = "spalart_allmaras"\n',
           'model = "spalart_allmaras"\nrelax_nut = 1.0   # ADR 0770 KT: the eddy viscosity not relaxed\n'),
    "KC": ("the corrector settings", 'convection = "linear_upwind"\n',
           'convection = "linear_upwind"\nnon_orthogonal_correctors = 1   # ADR 0770 KC: one non-orthogonal corrector\n'
           "simplec_non_orthogonal_compensation = true   # ADR 0770 KC: with ADR 0050's compensation\n"),
    "KG": ("the local character of the pseudo time step", 'convection = "linear_upwind"\n',
           'convection = "linear_upwind"\npseudo_mode = "physical"   # ADR 0770 KG: one global pseudo time step\n'
           "pseudo_dt = 0.02   # ADR 0770 KG: ADR 0764's time step\n"),
}
ORDER = ["KU", "KS", "KT", "KC", "KG"]
ANGLES = [17, 15]


def base_text(alpha):
    with open(os.path.join(CASES760, f"SA-a{alpha:02d}.toml"), encoding="utf-8") as f:
        return f.read().replace("\r\n", "\n")


def rid(k, alpha):
    return f"{k}-a{alpha:02d}"


def ids():
    return [rid("K0", 17)] + [rid(k, a) for a in ANGLES for k in ORDER]


def case_text(i):
    k, a = i.split("-a")
    alpha = int(a)
    text = base_text(alpha)
    if k == "K0":
        assert alpha == 17
        return text
    what, anchor, repl = SETTINGS[k]
    lines = text.split("\n", 2)
    assert lines[0].startswith("# ADR 0167") and lines[1].startswith("title = "), i
    head = f"# ADR 0770 (F-884) run {i}: ADR 0760's case SA-a{alpha:02d}.toml with one setting added ({what})"
    title = (f'title = "NACA 0012, alpha {alpha} deg, steady SA, mesh M1w-far500, ADR 0760\'s set-up with one setting '
             f'added: {what} (ADR 0770, F-884)"')
    body = lines[2]
    assert body.count(anchor) == 1, (i, anchor)
    return head + "\n" + title + "\n" + body.replace(anchor, repl)


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


def verdict(s, alpha):
    """ADR 0770: steady by D5 (the cause named), and the target (17 degrees) or the control (15 degrees)."""
    w = s["window"]
    rcl = w["cl_eff"]["max"] - w["cl_eff"]["min"]
    rcd = w["cd_eff"]["max"] - w["cd_eff"]["min"]
    steady = rcl <= w49d.STEADY_CL_RANGE and rcd <= w49d.STEADY_CD_RANGE
    ref = TARGET if alpha == 17 else CONTROL
    cl = s["value"]["cl_eff"]
    return {"range_cl_eff": rcl, "range_cd_eff": rcd, "steady": steady, "named": steady, "cl": cl,
            "reference": ref, "difference": cl - ref, "met": bool(steady and abs(cl - ref) <= TOL)}


def read_run(work, i):
    rdir = os.path.join(work, "runs", i)
    its, fin, warnings = w48.read_stream(os.path.join(rdir, "solver-status.jsonl"))
    with open(os.path.join(rdir, "wall.json"), encoding="utf-8") as f:
        wall = json.load(f)
    s = w49d.summary(its, wall["exit"])
    alpha = int(i.split("-a")[1])
    r = {"id": i, "alpha": alpha, "setting": i.split("-a")[0], "exit": wall["exit"], "wall_s": wall["process_wall_s"],
         "state": (fin or {}).get("state"), "finish_reason": (fin or {}).get("reason"),
         "warnings": sorted({w.get("rule") or w.get("message", "")[:60] for w in warnings}),
         "iterations": s["iterations"], "window_iterations": s["window_iterations"], "value_rule": s["value_rule"],
         "window": {k: s["window"][k] for k in ("cl_eff", "cd_eff")}, "value": {k: s["value"][k] for k in ("cl_eff", "cd_eff")},
         "history": [[e["index"], e["monitors"]["cl_eff"], e["monitors"]["cd_eff"]] for e in its
                     if e["index"] % HISTORY_EVERY == 0 or e["index"] == s["iterations"]],
         "case": case_text(i)}
    r["verdict"] = verdict(s, alpha)
    return r


def collect(work, outdir, binary=None, commit=None):
    runs = {}
    for i in ids():
        if os.path.exists(os.path.join(work, "runs", i, "wall.json")):
            runs[i] = read_run(work, i)
    with open(RESULTS760, encoding="utf-8") as f:
        ref = json.load(f)["runs"]["SA-a17"]
    d0 = None
    if "K0-a17" in runs:
        k = runs["K0-a17"]
        d0 = {"iterations": k["iterations"] == ref["iterations"], "exit": k["exit"] == ref["exit"]}
        for m in ("cl_eff", "cd_eff"):
            for q in ("mean", "min", "max", "final"):
                d0[f"{m}_{q}"] = k["window"][m][q] == ref[m][q]
        d0["equal_bits"] = all(d0.values())
    mesh = w49d.mesh_info(work).get(w49d.MESH)
    out = {"adr": "0770", "item": "F-884", "target": TARGET, "tolerance": TOL, "control": CONTROL,
           "d5": {"cl_range": w49d.STEADY_CL_RANGE, "cd_range": w49d.STEADY_CD_RANGE, "window": w49d.WINDOW},
           "binary": {"path": binary, "sha256": w49d.sha256_of(binary) if binary else None, "commit": commit},
           "mesh": mesh, "D0": d0, "runs": runs}
    os.makedirs(os.path.join(outdir, "cases"), exist_ok=True)
    for i in ids():
        if not i.startswith("K0"):
            with open(os.path.join(outdir, "cases", i + ".toml"), "w", encoding="utf-8", newline="\n") as f:
                f.write(case_text(i))
    with open(os.path.join(outdir, "results.json"), "w", encoding="utf-8", newline="\n") as f:
        json.dump(out, f, indent=1, sort_keys=True)
        f.write("\n")
    for i, r in runs.items():
        v = r["verdict"]
        print(f"{i}: exit {r['exit']} it {r['iterations']} cl {v['cl']:.6f} range cl {v['range_cl_eff']:.3e} "
              f"cd {v['range_cd_eff']:.3e} steady {v['steady']} met {v['met']}")
    print("D0", d0)
    return out


def selftest():
    texts = {i: case_text(i) for i in ids()}
    assert texts["K0-a17"] == base_text(17)
    for i, t in texts.items():
        if i.startswith("K0"):
            continue
        k, a = i.split("-a")
        base = base_text(int(a)).split("\n")
        new = t.split("\n")
        added = [ln for ln in new[2:] if ln not in base[2:]]
        removed = [ln for ln in base[2:] if ln not in new[2:]]
        assert not removed, (i, removed)
        assert len(added) == SETTINGS[k][2].count("\n") - SETTINGS[k][1].count("\n"), (i, added)
        assert f"rotate_z_deg = -{int(a)}" in t
    s = {"window": {"cl_eff": {"min": 1.0, "max": 1.004}, "cd_eff": {"min": 0.04, "max": 0.0404}},
         "value": {"cl_eff": 1.56}}
    v = verdict(s, 17)
    assert v["steady"] and v["met"], v
    s["window"]["cl_eff"]["max"] = 1.006
    assert not verdict(s, 17)["steady"]
    s["window"]["cl_eff"]["max"] = 1.004
    s["value"]["cl_eff"] = 1.4932034
    assert verdict(s, 17)["steady"] and not verdict(s, 17)["met"]
    assert verdict(s, 15)["met"]
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
