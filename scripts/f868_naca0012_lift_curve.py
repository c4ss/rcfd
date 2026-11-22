"""ADR 0760 (F-868): the NACA 0012 lift curve of ADR 0167's SA arm on current main, against Ladson's measured lift.

    python scripts/f868_naca0012_lift_curve.py ids                      the twelve run ids (ADR 0167's SA arm, 0 to 17 deg)
    python scripts/f868_naca0012_lift_curve.py cases <dir>              the twelve case texts, as ADR 0167's driver writes them
    python scripts/f868_naca0012_lift_curve.py collect <work> <outdir> [--binary <rcfd>] [--commit <sha>]
                                                                        results.json and the case files into <outdir>
    python scripts/f868_naca0012_lift_curve.py selftest

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
import os
import sys

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
    elif a[0] == "selftest":
        selftest()
    else:
        raise SystemExit(__doc__)


if __name__ == "__main__":
    main()
