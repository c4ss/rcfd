"""ADR 0156 (job 48V): forces, shedding and corrector counts of a cylinder run's status stream.

usage: python scripts/w48v_shedding.py <out.json> <label>=<solver-status.jsonl> ...

For each stream (the `rcfd run --status-every 1 --status-monitors` stream of the run matrix) it reads the iteration events
(index, time, monitors cd and cl, correctors when the run is adaptive, wall_s) and the finish event, and reports:
  cd and cl at t = 1, 3, 10 (the step nearest each time)
  shedding over the window [50, t_end] (ADR 0156 C3, C5): the upward zero crossings of cl - mean(cl) (mean over the window,
  linear interpolation between steps), St = 1 / (mean period) with D = U = 1, the mean cd over the whole periods between the
  first and the last crossing (trapezoid), cl peak to peak over the window; "not reached" when fewer than four crossings
  (three periods) lie in the window
  correctors (A6): mean over the steps with t in (0, 10] and over the whole run, max, the count histogram
  wall time (A7): wall_s at the first event with t >= 3 per simulated second, and over the whole run; and the stream's finish
  event (state, the correctors summary)
"""
import json
import sys


def read(path):
    its, fin = [], None
    with open(path, encoding="utf-8") as f:
        for line in f:
            try:
                e = json.loads(line)
            except json.JSONDecodeError:
                continue
            if e.get("event") == "iteration":
                its.append(e)
            elif e.get("event") == "finish":
                fin = e
    # the stream reports every step once (status-every 1); keep the last record of a step
    by = {}
    for e in its:
        by[e["index"]] = e
    return [by[k] for k in sorted(by)], fin


def at(its, t, key):
    best = min(its, key=lambda e: abs(e["time"] - t), default=None)
    if best is None or abs(best["time"] - t) > 0.6 * (its[1]["time"] - its[0]["time"] if len(its) > 1 else 1.0):
        return None
    return best.get("monitors", {}).get(key)


def shedding(its, t0=50.0):
    w = [(e["time"], e["monitors"]["cl"], e["monitors"]["cd"]) for e in its if e["time"] >= t0 and "cl" in e.get("monitors", {})]
    if len(w) < 3:
        return {"window": [t0, None], "reached": False}
    t_end = w[-1][0]
    mean_cl = sum(c for _, c, _ in w) / len(w)
    ups = []
    for (ta, ca, _), (tb, cb, _) in zip(w, w[1:]):
        a, b = ca - mean_cl, cb - mean_cl
        if a < 0 <= b:
            ups.append(ta + (tb - ta) * (-a) / (b - a))
    out = {"window": [t0, t_end], "crossings": ups, "cl_peak_to_peak": max(c for _, c, _ in w) - min(c for _, c, _ in w)}
    if len(ups) < 4:
        out["reached"] = False
        return out
    periods = [b - a for a, b in zip(ups, ups[1:])]
    period = (ups[-1] - ups[0]) / (len(ups) - 1)
    # mean cd over [ups[0], ups[-1]] by the trapezoid rule on the step values (linear inside a step)
    acc, span = 0.0, 0.0
    for (ta, _, da), (tb, _, db) in zip(w, w[1:]):
        lo, hi = max(ta, ups[0]), min(tb, ups[-1])
        if hi <= lo:
            continue
        f = lambda t: da + (db - da) * (t - ta) / (tb - ta)
        acc += 0.5 * (f(lo) + f(hi)) * (hi - lo)
        span += hi - lo
    out.update({
        "reached": True,
        "periods": periods,
        "period": period,
        "St": 1.0 / period,
        "cd_mean": acc / span,
        "period_spread": (max(periods) - min(periods)) / period,
    })
    return out


def correctors(its):
    n = [(e["time"], e["correctors"]) for e in its if "correctors" in e]
    if not n:
        return None
    first10 = [c for t, c in n if t <= 10.0 + 1e-9]
    hist = {}
    for _, c in n:
        hist[c] = hist.get(c, 0) + 1
    return {
        "steps": len(n),
        "mean_t_le_10": sum(first10) / len(first10) if first10 else None,
        "mean_all": sum(c for _, c in n) / len(n),
        "max": max(c for _, c in n),
        "histogram": {str(k): v for k, v in sorted(hist.items())},
        "cap_bound_last": its[-1].get("corrector_cap"),
    }


def wall(its):
    out = {}
    e3 = next((e for e in its if e["time"] >= 3.0 - 1e-9), None)
    if e3:
        out["wall_s_to_t3"] = e3["wall_s"]
        out["wall_per_sim_s_0_3"] = e3["wall_s"] / e3["time"]
    if its:
        out["wall_s_total"] = its[-1]["wall_s"]
        out["wall_per_sim_s_total"] = its[-1]["wall_s"] / its[-1]["time"]
        out["t_end"] = its[-1]["time"]
        out["steps"] = its[-1]["index"]
    return out


def main():
    out_path, specs = sys.argv[1], sys.argv[2:]
    result = {}
    for spec in specs:
        label, path = spec.split("=", 1)
        its, fin = read(path)
        r = {
            "cd": {str(t): at(its, t, "cd") for t in (1.0, 3.0, 10.0)},
            "cl": {str(t): at(its, t, "cl") for t in (1.0, 3.0, 10.0)},
            "shedding": shedding(its),
            "correctors": correctors(its),
            "wall": wall(its),
            "finish": {k: fin.get(k) for k in ("state", "reason", "iterations", "time", "correctors")} if fin else None,
        }
        result[label] = r
        sh = r["shedding"]
        st = f"St {sh['St']:.4f}, cd {sh['cd_mean']:.4f}, cl p-p {sh['cl_peak_to_peak']:.3f}" if sh.get("reached") else "shedding not reached"
        c = r["correctors"]
        cs = f"correctors mean {c['mean_t_le_10']:.3f} (t <= 10), {c['mean_all']:.3f} (all), max {c['max']}" if c else "fixed"
        print(f"{label}: cd(3) {r['cd']['3.0']}, cd(10) {r['cd']['10.0']}; {st}; {cs}; wall {r['wall']}")
    with open(out_path, "w", encoding="utf-8") as f:
        json.dump(result, f, indent=1)


if __name__ == "__main__":
    main()
