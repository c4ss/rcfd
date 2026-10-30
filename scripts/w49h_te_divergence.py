"""ADR 0171 (job 49H): the NACA 0012 trailing-edge divergence at fine wall spacing, reproduction and diagnosis runs.

    python scripts/w49h_te_divergence.py cases <work>            write <work>/cases/<id>.toml for every registered run
    python scripts/w49h_te_divergence.py list                    the registry
    python scripts/w49h_te_divergence.py run <rcfd> <work> <id>...   run (at most 2 at a time is the caller's job)
    python scripts/w49h_te_divergence.py summary <work>          one line per run: end, iteration, guard text

The case text is ADR 0159's (`w48y_aoa_sweep.case_text`, unchanged: SA or SST, SIMPLEC, linear upwind, current defaults)
with a short iteration cap and a snapshot every iteration. Meshes are built with `w48y_aoa_sweep.py mesh` (M1) and
`w49e_sa_wall.py mesh` (M1w15, M1w05) into <work>/meshes.
"""
import json
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import w48y_aoa_sweep as w  # noqa: E402
import w49a_sa_drag as w49a  # noqa: E402

# id -> spec. Phase 1 (ADR 0171 set-up): reproduction. Later phases are added by amendments of the ADR.
REGISTRY = {
    "RA": {"mesh": "M1", "alpha": 0, "arm": "SA", "iterations": 300},
    "RB": {"mesh": "M1", "alpha": 6, "arm": "SA", "iterations": 300},
    "RC": {"mesh": "M1w05", "alpha": 0, "arm": "SA", "iterations": 300},
    "RD": {"mesh": "M1w05", "alpha": 6, "arm": "SA", "iterations": 300},
    "RE": {"mesh": "M1", "alpha": 6, "arm": "SST", "iterations": 300},
    "RF": {"mesh": "M1w15", "alpha": 6, "arm": "SA", "iterations": 300},
}

# Phase 2 (ADR 0171 amendment 1): one change against RB (M1, 6 degrees, SA, 300 iterations) per run.
_B = {"mesh": "M1", "alpha": 6, "arm": "SA", "iterations": 300}
_A = 'algorithm = "simplec"\n'
_NOROT = ("[mesh.transform]\nrotate_z_deg = -6   # angle of attack 6 deg: the aerofoil turns by -alpha about the quarter "
          "chord (ADR 0155)\nrotate_origin = [0.25, 0.0, 0.0]\n\n", "")
_V6 = "[0.9945218953682733, 0.10452846326765347, 0.0]"
REGISTRY.update({
    "PA": dict(_B, edit={"a": (_A, _A + 'velocity_correction = "face_flux"\n')}),
    "PB": dict(_B, edit={"a": (_A, _A + "alpha_p = 0.3\n")}),
    "PC": dict(_B, edit={"a": (_A, _A + "non_orthogonal_correctors = 2\nsimplec_non_orthogonal_compensation = true\n")}),
    "PD": dict(_B, edit={"a": (_A, 'algorithm = "simple"\n')}),
    "PE": dict(_B, edit={"a": ('model = "spalart_allmaras"', 'model = "laminar"')}),
    "PF": dict(_B, edit={"a": ('convection = "linear_upwind"', 'convection = "upwind"')}),
    "PG": dict(_B, edit={"a": _NOROT,
                         "b": ("[initial]\nvelocity = [1.0, 0.0, 0.0]", "[initial]\nvelocity = " + _V6),
                         "c": ("velocity_value = [1.0, 0.0, 0.0]", "velocity_value = " + _V6)}),
    "PH": dict(_B, edit={"a": (_A, _A + "alpha_u = 0.7\n")}),
    "PI": dict(_B, edit={"a": (_A, _A + "alpha_p = 0.7\n")}),
})


# Phase 3 (ADR 0171 amendment 2): the existing Barth-Jespersen limiter of the linear-upwind extrapolation.
_BJ = ('algorithm = "simplec"\n', 'algorithm = "simplec"\nlinear_upwind_limiter = "barth_jespersen"\n')
REGISTRY.update({
    "QA": dict(_B, edit={"a": _BJ}),
    "QB": dict(_B, alpha=16, edit={"a": _BJ}),
    "QC": dict(_B, arm="SST", edit={"a": _BJ}),
    "QD": dict(_B, mesh="M1w05", edit={"a": _BJ}),
    "QE": dict(_B, alpha=0, edit={"a": _BJ}),
})


# Phase 4 (ADR 0171 amendment 3): full-length runs on M1, 8000 iterations, the last snapshot only, ADR 0164's extra reports.
_L = {"mesh": "M1", "arm": "SA", "iterations": 8000, "long": True}
REGISTRY.update({
    "LA0": dict(_L, alpha=0),
    "LB0": dict(_L, alpha=0, edit={"a": _BJ}),
    "LB6": dict(_L, alpha=6, edit={"a": _BJ}),
})


def case_text(spec):
    s = dict(spec)
    text = w.case_text(s)
    n = s["iterations"]
    if spec.get("long"):
        text += w49a.EXTRA_REPORTS + w49a.NT_REPORT
    else:
        # a snapshot every iteration (scratch only), instead of the last one
        text = text.replace(f"store_interval = {n}   # the last snapshot only", "store_interval = 1")
    for k, v in spec.get("edit", {}).items():  # amendments: textual edits of the case, key -> (old, new)
        old, new = v
        assert old in text, (k, old)
        text = text.replace(old, new)
    return text


def write_cases(work):
    cdir = os.path.join(work, "cases")
    os.makedirs(cdir, exist_ok=True)
    for i, spec in REGISTRY.items():
        with open(os.path.join(cdir, i + ".toml"), "w", encoding="utf-8", newline="\n") as f:
            f.write(f"# ADR 0171 (job 49H) run {i}: {json.dumps(spec)}\n" + case_text(spec))
    return cdir


def run_one(rcfd, work, i):
    cases = os.path.join(work, "cases")
    run = os.path.join(work, "runs", i)
    if os.path.exists(run):
        raise SystemExit(f"{run} exists; refusing to overwrite a run")
    os.makedirs(os.path.join(run, "output"))
    snap = os.path.join(run, "case.snapshot")
    with open(os.path.join(cases, i + ".toml"), encoding="utf-8") as f, open(snap, "w", encoding="utf-8",
                                                                            newline="\n") as g:
        g.write(f.read())
    cmd = [rcfd, "run", snap, "--case-dir", cases, "--out", os.path.join(run, "output"),
           "--status-file", os.path.join(run, "solver-status.jsonl"), "--status-every", "1", "--quiet",
           "--threads", "1", "--store-dir", os.path.join(run, "fields"), "--status-monitors", "--max-elapsed", "9000" if i.startswith("L") else "1500"]
    t0 = time.time()
    with open(os.path.join(run, "log.txt"), "w", encoding="utf-8") as log:
        r = subprocess.run(cmd, stdout=log, stderr=subprocess.STDOUT)
    with open(os.path.join(run, "wall.json"), "w", encoding="utf-8") as f:
        json.dump({"id": i, "exit": r.returncode, "wall_s": time.time() - t0}, f)
    return r.returncode


def summary(work):
    for i in REGISTRY:
        run = os.path.join(work, "runs", i)
        if not os.path.isdir(run):
            continue
        try:
            wall = json.load(open(os.path.join(run, "wall.json")))
        except OSError:
            wall = {"exit": None}
        last = None
        with open(os.path.join(run, "solver-status.jsonl"), encoding="utf-8") as f:
            for line in f:
                ev = json.loads(line)
                if ev.get("event") == "iteration":
                    last = ev
        text = open(os.path.join(run, "log.txt"), encoding="utf-8", errors="replace").read().strip().split("\n")
        guard = [t for t in text if "divergence" in t or "guard" in t]
        print(i, json.dumps(REGISTRY[i]), "exit", wall.get("exit"), "last it", last and last.get("iteration"),
              "|", guard[0][:260] if guard else text[-1][:160])


def long_summary(work):
    """Window means (last 1000 iterations) of every report of the full-length runs, and the steady rule of ADR 0159 D4."""
    out = {}
    for i, spec in REGISTRY.items():
        if not spec.get("long"):
            continue
        path = os.path.join(work, "runs", i, "solver-status.jsonl")
        if not os.path.exists(path):
            continue
        its, fin, _ = w.read_stream(path)
        if fin is None:
            out[i] = {"finished": False, "iterations": its[-1]["index"] if its else 0}
            continue
        win = [e for e in its if "cl" in e.get("monitors", {})][-1000:]
        keys = sorted(win[0]["monitors"]) if win else []
        r = {"finished": True, "exit": fin.get("exit_code"), "iterations": its[-1]["index"], "wall_s": fin.get("wall_s"),
             "window": {k: w.stats([e["monitors"][k] for e in win]) for k in keys}}
        if "divergence" in fin:
            r["divergence"] = fin["divergence"]
        cl, cd = r["window"].get("cl"), r["window"].get("cd")
        if cl and cd:
            r["steady"] = (cl["max"] - cl["min"] <= 0.005) and (cd["max"] - cd["min"] <= 0.0005)
        out[i] = r
    print(json.dumps(out, indent=1))
    return out


def main():
    a = sys.argv[1:]
    if not a:
        print(__doc__)
    elif a[0] == "list":
        for i, s in REGISTRY.items():
            print(i, json.dumps(s))
    elif a[0] == "cases":
        print(write_cases(a[1]))
    elif a[0] == "run":
        for i in a[3:]:
            print(i, run_one(a[1], a[2], i), flush=True)
    elif a[0] == "summary":
        summary(a[1])
    elif a[0] == "longsummary":
        long_summary(a[1])
    else:
        raise SystemExit(__doc__)


if __name__ == "__main__":
    main()
