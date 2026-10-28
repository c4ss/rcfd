"""ADR 0154 (job 48T): spoke metrics of stored cylinder O-grid pressure fields, the screen of the status streams, figures.

usage:
  python scripts/w48t_spoke_metrics.py metrics <out.json> <label>=<dump prefix>[@<status stream>] ...
  python scripts/w48t_spoke_metrics.py compare-status <stream a> <stream b>
  python scripts/w48t_spoke_metrics.py compare-dumps <prefix a> <prefix b> [<field>]
  python scripts/w48t_spoke_metrics.py figures <out dir> <metrics.json> <label>=<dump prefix> ...

Dumps are written by crates/rcfd-studio/tests/w48t_cylinder_spokes.rs (w48t_dump_stores): <prefix>.centres.f64 (cells x 3),
<prefix>.p.f64 (snapshots x cells), <prefix>.meta.json. Definitions (ADR 0154, "Metrics"): cells are grouped into rings by
their centre radius (equal to 1e-9 relative) and ordered by angle; q = 0.5 (rho = 1, U = 1, kinematic p).
  M_hf   RMS over the band rings (centre radius in [0.55, 0.70]) of the part of p in the circumferential modes
         n/8 <= k <= n/2 (wavelength <= 8 cells), / q                                                     (primary)
  A_N    Nyquist amplitude |(1/n) sum_i p_i (-1)^i| / q: band mean, wall ring, max over rings r <= 0.70
  M_2    RMS over the band of p(i+1) - 2 p(i) + p(i-1), / q
  lambda* wavelength n/k* (cells) of the band-averaged |P(k)| maximum over k >= n/8; also in D at r = 0.6
  phys   RMS over the band of the modes 1 <= k < n/8, / q
Screen (status stream, iteration events): RMS of the step-to-step change of cd over the steps with t in [2, 3], median res_p
over the same steps, wall seconds per simulated second.
"""
import json
import math
import os
import sys

import numpy as np

Q = 0.5
BAND = (0.55, 0.70)
EVAL_TIMES = (1.0, 3.0, 10.0, 30.0)


def load_dump(prefix, field="p"):
    meta = json.load(open(prefix + ".meta.json", encoding="utf-8"))
    n = meta["cells"]
    c = np.fromfile(prefix + ".centres.f64", "<f8").reshape(n, 3)
    v = np.fromfile(prefix + f".{field}.f64", "<f8").reshape(-1, n)
    return meta, c, v


def rings_of(c):
    """List of (radius, cell indices ordered by angle); every ring must have the same count."""
    r = np.hypot(c[:, 0], c[:, 1])
    order = np.argsort(r, kind="stable")
    rs = r[order]
    cuts = np.nonzero(np.diff(rs) > 1e-9 * np.maximum(rs[1:], 1e-300))[0] + 1
    groups = np.split(order, cuts)
    out = []
    for g in groups:
        th = np.mod(np.arctan2(c[g, 1], c[g, 0]), 2 * math.pi)
        out.append((float(r[g].mean()), g[np.argsort(th, kind="stable")]))
    counts = {len(g) for _, g in out}
    if len(counts) != 1:
        raise SystemExit(f"rings of unequal size {sorted(counts)[:5]}: not an O-grid")
    return out


def ring_metrics(p, rings):
    n = len(rings[0][1])
    kmin = n // 8
    band = [(rj, g) for rj, g in rings if BAND[0] <= rj <= BAND[1]]
    hp_sq, lp_sq, d2_sq, cnt = 0.0, 0.0, 0.0, 0
    an_band, spec = [], np.zeros(n // 2 + 1)
    for rj, g in band:
        x = p[g]
        X = np.fft.rfft(x)
        hp = X.copy()
        hp[:kmin] = 0.0
        lp = X.copy()
        lp[0] = 0.0
        lp[kmin:] = 0.0
        xh = np.fft.irfft(hp, n)
        xl = np.fft.irfft(lp, n)
        d2 = np.roll(x, -1) - 2 * x + np.roll(x, 1)
        hp_sq += float(np.sum(xh * xh))
        lp_sq += float(np.sum(xl * xl))
        d2_sq += float(np.sum(d2 * d2))
        cnt += n
        an_band.append(abs(float(np.sum(x * (-1.0) ** np.arange(n)))) / n / Q)
        spec += np.abs(X) / n / Q
    spec /= max(len(band), 1)
    k_star = kmin + int(np.argmax(spec[kmin:]))
    profile = []
    for rj, g in rings:
        if rj > 1.5:
            break
        x = p[g]
        X = np.fft.rfft(x)
        X[:kmin] = 0.0
        profile.append(
            {
                "r": rj,
                "A_N": abs(float(np.sum(x * (-1.0) ** np.arange(n)))) / n / Q,
                "hp_rms": float(np.sqrt(np.mean(np.fft.irfft(X, n) ** 2))) / Q,
            }
        )
    an_inner = [row["A_N"] for row in profile if row["r"] <= BAND[1]]
    return {
        "n_theta": n,
        "rings_in_band": len(band),
        "M_hf": math.sqrt(hp_sq / cnt) / Q,
        "A_N_band_mean": float(np.mean(an_band)),
        "A_N_wall": profile[0]["A_N"],
        "A_N_max_r_le_0p7": float(max(an_inner)),
        "M_2": math.sqrt(d2_sq / cnt) / Q,
        "k_star": k_star,
        "lambda_cells": n / k_star,
        "lambda_D_at_r0p6": 2 * math.pi * 0.6 / k_star,
        "phys_lowpass_rms": math.sqrt(lp_sq / cnt) / Q,
        "spectrum_band_mean": spec.tolist(),
        "profile": profile,
        "p_min": float(p.min()),
        "p_max": float(p.max()),
        "p_pct": {q: float(np.percentile(p, q)) for q in (1, 5, 50, 95, 99)},
    }


def read_stream(path):
    rows, finish = [], None
    for line in open(path, encoding="utf-8"):
        line = line.strip()
        if not line:
            continue
        e = json.loads(line)
        if e.get("event") == "iteration":
            rows.append(e)
        elif e.get("event") == "finish":
            finish = e
    return rows, finish


def screen(path, t0=0.0):
    """The screen over the steps with t in [t0 + 2, t0 + 3] (t0 = the start time of a restarted run)."""
    rows, finish = read_stream(path)
    t = np.array([r["time"] for r in rows])
    cd = np.array([r.get("monitors", {}).get("cd", float("nan")) for r in rows])
    rp = np.array([r.get("res_p", float("nan")) for r in rows])
    w = (t >= t0 + 2.0 - 1e-9) & (t <= t0 + 3.0 + 1e-9)
    idx = np.nonzero(w)[0]
    d = np.diff(cd[idx]) if len(idx) > 1 else np.array([float("nan")])
    wall = rows[-1]["wall_s"] - rows[0]["wall_s"] if len(rows) > 1 else float("nan")
    span = t[-1] - t[0] if len(rows) > 1 else float("nan")
    out = {
        "steps": len(rows),
        "t_last": float(t[-1]) if len(t) else None,
        "dcd_rms_t2_3": float(np.sqrt(np.mean(d * d))),
        "res_p_median_t2_3": float(np.median(rp[idx])) if len(idx) else None,
        "wall_s_per_sim_s": wall / span if span and span > 0 else None,
        "wall_s_total": float(rows[-1]["wall_s"]) if rows else None,
        "finish": {k: finish.get(k) for k in ("state", "reason", "exit_code", "elapsed_s")} if finish else None,
    }
    for tt in EVAL_TIMES:
        if len(t) and t[0] - 1e-9 <= tt <= t[-1] + 1e-9:
            i = int(np.argmin(np.abs(t - tt)))
            out[f"cd_t{tt:g}"] = float(cd[i])
            out[f"cl_t{tt:g}"] = float(rows[i].get("monitors", {}).get("cl", float("nan")))
    return out


def metrics(out_path, specs):
    result = {}
    for spec in specs:
        label, rest = spec.split("=", 1)
        prefix, stream, t0 = (rest.split("@") + [None, None])[:3]
        t0 = float(t0) if t0 else 0.0
        meta, c, p = load_dump(prefix)
        rings = rings_of(c)
        times = meta["times"]
        entry = {"prefix": prefix, "cells": meta["cells"], "n_rings": len(rings), "n_theta": len(rings[0][1]),
                 "index_source": meta.get("index_source"), "snapshots": {}}
        for i, tt in enumerate(times):
            m = ring_metrics(p[i], rings)
            m["step"] = meta["steps"][i]
            entry["snapshots"][f"{tt:.6g}"] = m
        if stream:
            entry["screen"] = screen(stream, t0)
        result[label] = entry
        brief = ", ".join(
            f"t={k}: M_hf {v['M_hf']:.3g} A_N {v['A_N_band_mean']:.3g} lam {v['lambda_cells']:.3g}c"
            for k, v in entry["snapshots"].items()
            if any(abs(float(k) - e) < 0.05 for e in EVAL_TIMES)
        )
        print(f"{label}: {brief}", flush=True)
        if stream:
            s = entry["screen"]
            print(f"   screen: dcd_rms {s['dcd_rms_t2_3']:.3g}, res_p median {s['res_p_median_t2_3']}, wall/sim {s['wall_s_per_sim_s']}")
    json.dump(result, open(out_path, "w", encoding="utf-8"), indent=1)


def compare_status(a, b):
    ra, _ = read_stream(a)
    rb, _ = read_stream(b)
    keys = ("index", "time", "res_u", "res_p", "continuity")
    diffs = 0
    for x, y in zip(ra, rb):
        if any(x.get(k) != y.get(k) for k in keys) or x.get("monitors") != y.get("monitors"):
            diffs += 1
    print(json.dumps({"events_a": len(ra), "events_b": len(rb), "differing_events": diffs,
                      "equal": diffs == 0 and len(ra) == len(rb)}))


def compare_dumps(a, b, field="p"):
    ma, ca, pa = load_dump(a, field)
    mb, cb, pb = load_dump(b, field)
    rows = []
    for i, ta in enumerate(ma["times"]):
        for j, tb in enumerate(mb["times"]):
            if abs(ta - tb) < 1e-9 and ma["steps"][i] == mb["steps"][j]:
                same = pa[i].tobytes() == pb[j].tobytes()
                rows.append({"time": ta, "step": ma["steps"][i], "bit_identical": same,
                             "max_abs_diff": float(np.max(np.abs(pa[i] - pb[j])))})
    print(json.dumps({"centres_identical": ca.tobytes() == cb.tobytes(), "common": rows}))


def figures(out_dir, metrics_path, specs):
    import matplotlib

    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    os.makedirs(out_dir, exist_ok=True)
    allm = json.load(open(metrics_path, encoding="utf-8"))
    for spec in specs:
        label, prefix = spec.split("=", 1)
        meta, c, p = load_dump(prefix)
        rings = rings_of(c)
        n = len(rings[0][1])
        rr = np.array([rj for rj, _ in rings])
        nr = int(np.searchsorted(rr, 1.0))
        for tt in (1.0, 3.0, 10.0):
            i = int(np.argmin(np.abs(np.array(meta["times"]) - tt)))
            if abs(meta["times"][i] - tt) > 0.05:
                continue
            P = np.array([p[i][g] for _, g in rings[:nr]])
            th_edges = np.linspace(0, 360, n + 1)
            # ring edges at the midpoints between ring centres (cell-centre radii; plotting only)
            r_mid = 0.5 * (rr[1:nr] + rr[: nr - 1])
            r_edges = np.concatenate([[0.5], r_mid, [rr[nr - 1] + (rr[nr - 1] - r_mid[-1])]])
            fig, axs = plt.subplots(1, 2, figsize=(13, 4.2), constrained_layout=True)
            for ax, (lo, hi, name) in zip(
                axs,
                [(P.min(), P.max(), "range = min/max of r <= 1 (like the Studio's snapshot range)"),
                 (np.percentile(P, 1), np.percentile(P, 99), "range = 1st..99th percentile")],
            ):
                im = ax.pcolormesh(th_edges, r_edges, P, cmap="viridis", vmin=lo, vmax=hi, shading="flat")
                ax.set_xlabel("theta [deg] (0 = downstream, x axis)")
                ax.set_ylabel("r [D]")
                ax.set_title(f"{label} p at t = {meta['times'][i]:.4g}: {name}", fontsize=8)
                fig.colorbar(im, ax=ax)
            fig.savefig(os.path.join(out_dir, f"{label}-p-polar-t{tt:g}.png"), dpi=110)
            plt.close(fig)
    # spectra at t = 3 and M_hf against time
    fig, ax = plt.subplots(figsize=(7, 4.2), constrained_layout=True)
    for label, e in allm.items():
        k3 = min(e["snapshots"], key=lambda k: abs(float(k) - 3.0))
        if abs(float(k3) - 3.0) > 0.05:
            continue
        s = np.array(e["snapshots"][k3]["spectrum_band_mean"])
        n = e["n_theta"]
        k = np.arange(len(s))
        ax.semilogy(k[1:] / n, s[1:], label=f"{label} (n={n})", lw=1)
    ax.axvline(1 / 8, color="grey", ls=":", lw=1)
    ax.set_xlabel("k / n_theta (0.5 = 2-cell wave)")
    ax.set_ylabel("band-mean |P_k| / q at t = 3")
    ax.legend(fontsize=7)
    fig.savefig(os.path.join(out_dir, "spectra-t3.png"), dpi=110)
    plt.close(fig)
    fig, ax = plt.subplots(figsize=(7, 4.2), constrained_layout=True)
    for label, e in allm.items():
        ts = sorted((float(k), v["M_hf"]) for k, v in e["snapshots"].items())
        if len(ts) > 1:
            ax.semilogy([a for a, _ in ts], [b for _, b in ts], marker=".", lw=1, label=label)
    ax.axhline(0.1, color="red", ls=":", lw=1)
    ax.axhline(0.01, color="green", ls=":", lw=1)
    ax.set_xlabel("t [D/U]")
    ax.set_ylabel("M_hf")
    ax.legend(fontsize=7)
    fig.savefig(os.path.join(out_dir, "m_hf-vs-time.png"), dpi=110)
    plt.close(fig)


def classify(entry, control, t0=0.0, checked=(1.0, 3.0, 10.0)):
    """ADR 0154 L3: clean if M_hf <= max(2 M_hf(control at the same time), 0.01) at every checked time the run reached, spoky if
    M_hf >= 0.1 at any, marginal otherwise; 'diverged' when the run stopped before t0 + 1 (no metric). Times are relative to t0
    (the start of a restarted run). Returns (class, {t: M_hf})."""
    snaps = {float(k): v for k, v in entry["snapshots"].items()}
    ctrl = {float(k): v for k, v in control["snapshots"].items()}
    vals = {}
    for t in checked:
        hit = [s for s in snaps if abs(s - (t0 + t)) < 0.05]
        if hit:
            vals[t] = snaps[hit[0]]["M_hf"]
    finish = (entry.get("screen") or {}).get("finish") or {}
    if not vals:
        return "diverged", vals
    limit = {t: max(2 * ctrl[min(ctrl, key=lambda c: abs(c - t))]["M_hf"], 0.01) for t in vals}
    if any(v >= 0.1 for v in vals.values()):
        cls = "spoky"
    elif all(vals[t] <= limit[t] for t in vals):
        cls = "clean"
    else:
        cls = "marginal"
    if finish.get("state") == "diverged":
        cls += " then diverged"
    return cls, vals


def table(metrics_path, control_label="R64", starts=None):
    """Markdown rows: label, M_hf at t0 + 1, 3, 10, A_N and lambda* at t0 + 3, screen, wall time per simulated second, class."""
    allm = json.load(open(metrics_path, encoding="utf-8"))
    ctrl = allm[control_label]
    starts = starts or {}
    print("| Run | M_hf t=1 | M_hf t=3 | M_hf t=10 | A_N t=3 | lambda* t=3 (cells) | dcd RMS t in [2,3] | median res_p | wall s per sim s | class |")
    print("|---|---|---|---|---|---|---|---|---|---|")
    for label, e in allm.items():
        t0 = starts.get(label, 0.0)
        cls, vals = classify(e, ctrl, t0)
        snaps = {float(k): v for k, v in e["snapshots"].items()}
        s3 = [v for k, v in snaps.items() if abs(k - (t0 + 3)) < 0.05]
        sc = e.get("screen") or {}
        f = lambda x: "-" if x is None or (isinstance(x, float) and math.isnan(x)) else f"{x:.3g}"
        print(
            f"| {label} | {f(vals.get(1.0))} | {f(vals.get(3.0))} | {f(vals.get(10.0))} | {f(s3[0]['A_N_band_mean']) if s3 else '-'} | "
            f"{f(s3[0]['lambda_cells']) if s3 else '-'} | {f(sc.get('dcd_rms_t2_3'))} | {f(sc.get('res_p_median_t2_3'))} | "
            f"{f(sc.get('wall_s_per_sim_s'))} | {cls} |"
        )


def main():
    cmd, args = sys.argv[1], sys.argv[2:]
    if cmd == "metrics":
        metrics(args[0], args[1:])
    elif cmd == "table":
        starts = dict((a.split("=")[0], float(a.split("=")[1])) for a in args[1:])
        table(args[0], starts=starts)
    elif cmd == "compare-status":
        compare_status(args[0], args[1])
    elif cmd == "compare-dumps":
        compare_dumps(args[0], args[1], *(args[2:3] or ["p"]))
    elif cmd == "figures":
        figures(args[0], args[1], args[2:])
    else:
        raise SystemExit(__doc__)


if __name__ == "__main__":
    main()
