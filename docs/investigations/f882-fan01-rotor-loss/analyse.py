"""F-882 (ADR 0768): the summary table of the rotor loss budget from the run logs.

Usage: python analyse.py <folder with B3.log, B2.log, B1.log, B0c.log> > summary.txt

Reads the F882S (sampled loss) and F882B (budget) lines that the test file
crates/rcfd-api/tests/f882_fan01_rotor_loss.rs prints, and writes, per mesh, the last
sample: torque, pressure rise, L_s at the three planes, L_ann, and the budget at
x = 90 mm for the three widths of the wall layer, with the closure of the two routes.
"""
import os
import sys

MESHES = ["B3", "B2", "B1", "B0c"]
CELLS = {"B3": 4920, "B2": 13627, "B1": 38184, "B0c": 106692}


def parse(path):
    s_last, b_last, it_last = None, {}, None
    for line in open(path, encoding="utf-8", errors="replace"):
        w = line.split()
        if not w:
            continue
        if w[0] == "F882S":
            it = int(w[2])
            kv = dict(zip(w[3::2], w[4::2]))
            s_last, it_last = {k: float(v) for k, v in kv.items()}, it
        elif w[0] == "F882B":
            it, plane, delta = int(w[2]), w[3], w[4]
            rest = w[5:]
            d = {}
            i = 0
            while i < len(rest):
                k = rest[i]
                if k == "conv":
                    d[k] = [float(x) for x in rest[i + 1:i + 5]]
                    i += 5
                elif k == "closes":
                    d[k] = rest[i + 1] == "true"
                    i += 2
                else:
                    d[k] = float(rest[i + 1])
                    i += 2
            b_last[(plane, delta)] = (it, d)
    return it_last, s_last, b_last


def main():
    folder = sys.argv[1]
    print("mesh cells iter torque_Nm dp_ts_Pa L_s88.7 L_s90 L_s91.3 L_ann90")
    rows = {}
    for m in MESHES:
        p = os.path.join(folder, m + ".log")
        if not os.path.exists(p):
            print(f"{m} (no log)")
            continue
        it, s, b = parse(p)
        if s is None:
            print(f"{m} (no sample)")
            continue
        rows[m] = (it, s, b)
        print(f"{m} {CELLS[m]} {it} {s['torque']:.6f} {s['dp_ts']:.4f} {s['L_s88.7']:.4f} {s['L_s90']:.4f} "
              f"{s['L_s91.3']:.4f} {s['L_ann90']:.4f}")
    print()
    print("budget at x = 90 mm, last sample [Pa]: mesh delta_mm whole blade hub_casing gap interior "
          "| hub_wall casing_wall upstream | route_A - route_B | closes")
    for m, (it, s, b) in rows.items():
        for delta in ["1.25", "2.5", "5"]:
            if ("90", delta) not in b:
                continue
            bit, d = b[("90", delta)]
            print(f"{m} {delta} {d['whole_b']:.4f} {d['blade']:.4f} {d['hubcasing']:.4f} {d['gap']:.4f} "
                  f"{d['interior']:.4f} | {d['hub']:.4f} {d['casing']:.4f} {d['upstream']:.4f} | "
                  f"{d['whole_a'] - d['whole_b']:.3e} | {d['closes']}")
    print()
    print("budget whole at the three planes, delta 2.5 mm [Pa]: mesh 88.7 90 91.3")
    for m, (it, s, b) in rows.items():
        v = [b.get((pl, "2.5"), (0, {"whole_b": float("nan")}))[1]["whole_b"] for pl in ["88.7", "90", "91.3"]]
        print(f"{m} {v[0]:.4f} {v[1]:.4f} {v[2]:.4f}")
    print()
    print("budget lines that do not close, over all sampled iterations:")
    for m in rows:
        n = sum(1 for line in open(os.path.join(folder, m + ".log"), encoding="utf-8", errors="replace")
                if line.startswith("F882B") and "closes false" in line)
        print(f"{m} {n}")
    print()
    print("observed order p and Richardson value (rule of ADR 0768, reported beside, no rule):")
    print("quantity triplet p value_at_p value_at_order_1 value_at_order_2")

    def q(m, name):
        it, s, b = rows[m]
        if name in ("L_s90", "L_ann90"):
            return s[name]
        d = b[("90", "2.5")][1]
        return d["whole_b" if name == "whole" else name]

    for trip in (["B2", "B1", "B0c"], ["B3", "B2", "B1"]):
        if not all(m in rows for m in trip):
            continue
        r21 = (CELLS[trip[1]] / CELLS[trip[0]]) ** (1 / 3)
        r32 = (CELLS[trip[2]] / CELLS[trip[1]]) ** (1 / 3)
        for name in ["L_s90", "L_ann90", "whole", "blade", "hubcasing", "gap", "interior"]:
            f1, f2, f3 = (q(m, name) for m in trip)
            p = order(f1, f2, f3, r21, r32)
            o1 = f3 + (f3 - f2) / (r32 - 1)
            o2 = f3 + (f3 - f2) / (r32 ** 2 - 1)
            val = f3 + (f3 - f2) / (r32 ** p - 1) if p is not None else float("nan")
            ps = f"{p:.3f}" if p is not None else "none"
            print(f"{name} {'-'.join(trip)} {ps} {val:.3f} {o1:.3f} {o2:.3f}")


def order(f1, f2, f3, r21, r32):
    """The root of r32^p (r21^p - 1) / (r32^p - 1) = (f1 - f2) / (f2 - f3) on [0.01, 20], or None."""
    if (f1 - f2) * (f2 - f3) <= 0:
        return None
    want = (f1 - f2) / (f2 - f3)

    def h(p):
        return r32 ** p * (r21 ** p - 1) / (r32 ** p - 1) - want

    lo, hi = 0.01, 20.0
    if h(lo) * h(hi) > 0:
        return None
    for _ in range(200):
        mid = 0.5 * (lo + hi)
        if h(lo) * h(mid) <= 0:
            hi = mid
        else:
            lo = mid
    return 0.5 * (lo + hi)


if __name__ == "__main__":
    main()
