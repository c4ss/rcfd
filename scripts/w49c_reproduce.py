#!/usr/bin/env python3
"""ADR 0166 (job 49C) runs R1-R3: the old defaults stay reproducible by name, and the new default is ADR 0164's opt-in.

R1, R2: ADR 0159's SA-a00 / SA-a10 case (its stored `case.snapshot`) with ADR 0166 D4's legacy spellings
(`component = "total_molecular"` on cl and cd, `turbulence = "zero_gradient"` on the far field), against ADR 0159's
stored `monitors.csv`, every iteration present in both, by bits.
R3: ADR 0164's F10 case with its `turbulence = "freestream"` line removed (the new default), against F10's stored
`monitors.csv`: the same iteration count and stop reason, and per iteration cd = cd_eff, cl = cl_eff, cd_v = cd_veff,
and the other columns equal.

The work folders of ADR 0159 and ADR 0164 are read only. Usage:
    python scripts/w49c_reproduce.py run RCFD_EXE WORK ID...      (ID in R1 R2 R3; one process per ID, started together)
    python scripts/w49c_reproduce.py compare WORK                 (prints and writes WORK/compare.json)
    python scripts/w49c_reproduce.py selftest
"""
import csv
import json
import os
import struct
import subprocess
import sys

W48Y = r"E:\projects\cfd\w48y-work\w"
W49A = r"E:\projects\cfd\w49a-work\w"

LEGACY_FARFIELD = ('[boundaries.farfield]\nvelocity = "freestream"\nvelocity_value = [1.0, 0.0, 0.0]\npressure = "freestream"\n',
                   '[boundaries.farfield]\nvelocity = "freestream"\nvelocity_value = [1.0, 0.0, 0.0]\npressure = "freestream"\n'
                   'turbulence = "zero_gradient"   # ADR 0166 D4: the far-field role before ADR 0166\n')
F10_LINE = ('turbulence = "freestream"   # ADR 0164 amendment 1: nuTilda = 3 nu on the inflow faces, zero gradient on the '
            'outflow faces\n')

RUNS = {
    # id: (source run folder, case folder for the relative mesh path, wall cap in s)
    "R1": (os.path.join(W48Y, "runs", "SA-a00"), os.path.join(W48Y, "cases"), 240),
    "R2": (os.path.join(W48Y, "runs", "SA-a10"), os.path.join(W48Y, "cases"), 240),
    "R3": (os.path.join(W49A, "runs", "F10"), os.path.join(W49A, "cases"), 1200),
}


def legacy(text):
    """ADR 0159's case with D4's legacy spellings."""
    if text.count(LEGACY_FARFIELD[0]) != 1:
        raise SystemExit("far-field block not found once")
    text = text.replace(LEGACY_FARFIELD[0], LEGACY_FARFIELD[1])
    for name in ("cl", "cd"):
        head = f"[reports.{name}]\ntype = \"force_coefficient\"\npatch = \"airfoil\"\n"
        i = text.index(head)
        j = text.index("u_ref = 1.0\n", i) + len("u_ref = 1.0\n")
        text = text[:j] + 'component = "total_molecular"   # ADR 0166 D4: the default before ADR 0166\n' + text[j:]
    return text


def new_default(text):
    """ADR 0164's F10 with its explicit far-field role removed."""
    if text.count(F10_LINE) != 1:
        raise SystemExit("F10's turbulence line not found once")
    return text.replace(F10_LINE, "")


def case_text(rid):
    src = open(os.path.join(RUNS[rid][0], "case.snapshot"), encoding="utf-8").read()
    return new_default(src) if rid == "R3" else legacy(src)


def command(rcfd, work, rid):
    run = os.path.join(work, rid)
    if os.path.exists(run):
        raise SystemExit(f"{run} exists; refusing to overwrite a run")
    os.makedirs(os.path.join(run, "output"))
    with open(os.path.join(run, "case.snapshot"), "w", encoding="utf-8", newline="\n") as f:
        f.write(case_text(rid))
    cmd = [rcfd, "run", os.path.join(run, "case.snapshot"), "--case-dir", RUNS[rid][1],
           "--out", os.path.join(run, "output"), "--status-file", os.path.join(run, "solver-status.jsonl"),
           "--status-every", "1", "--quiet", "--threads", "1", "--store-dir", os.path.join(run, "fields"),
           "--status-monitors", "--max-elapsed", str(RUNS[rid][2])]
    with open(os.path.join(run, "cmd.txt"), "w", encoding="utf-8") as f:
        f.write(json.dumps(cmd, indent=1) + "\n")
    return run, cmd


def run_many(rcfd, work, ids):
    procs = []
    for rid in ids:
        run, cmd = command(rcfd, work, rid)
        log = open(os.path.join(run, "log.txt"), "w", encoding="utf-8")
        procs.append((rid, subprocess.Popen(cmd, stdout=log, stderr=subprocess.STDOUT), log))
    for rid, p, log in procs:
        code = p.wait()
        log.close()
        with open(os.path.join(work, rid, "exit.json"), "w", encoding="utf-8") as f:
            json.dump({"exit_code": code}, f)
        print(rid, "exit", code)


def read_csv(path):
    with open(path, newline="", encoding="utf-8") as f:
        rows = list(csv.DictReader(f))
    return rows


def bits(x):
    return struct.unpack("<Q", struct.pack("<d", float(x)))[0]


def finish_reason(status):
    reason = None
    with open(status, encoding="utf-8") as f:
        for line in f:
            if '"reason"' in line:
                try:
                    reason = json.loads(line).get("reason", reason)
                except json.JSONDecodeError:
                    pass
    return reason


def compare_pairs(mine, ref, pairs):
    """Rows of `mine` against `ref` by iteration: [(column of mine, column of ref)], counts of compared and different."""
    by_it = {r["iteration"]: r for r in ref}
    n, diff, first = 0, 0, None
    for r in mine:
        q = by_it.get(r["iteration"])
        if q is None:
            continue
        n += 1
        for a, b in pairs:
            if bits(r[a]) != bits(q[b]):
                diff += 1
                if first is None:
                    first = {"iteration": r["iteration"], "mine": [a, r[a]], "ref": [b, q[b]]}
    return n, diff, first


def compare(work):
    out = {}
    for rid in ("R1", "R2"):
        mon = os.path.join(work, rid, "output", "monitors.csv")
        if not os.path.exists(mon):
            out[rid] = {"evaluated": False}
            continue
        mine = read_csv(mon)
        ref = read_csv(os.path.join(RUNS[rid][0], "output", "monitors.csv"))
        n, diff, first = compare_pairs(mine, ref, [("cd", "cd"), ("cl", "cl")])
        out[rid] = {"evaluated": True, "iterations_run": len(mine), "iterations_compared": n, "values_different": diff,
                    "first_difference": first, "met": n >= 100 and diff == 0,
                    "last": {"iteration": mine[-1]["iteration"], "cd": mine[-1]["cd"], "cl": mine[-1]["cl"]}}
    mon = os.path.join(work, "R3", "output", "monitors.csv")
    if os.path.exists(mon):
        mine = read_csv(mon)
        ref = read_csv(os.path.join(RUNS["R3"][0], "output", "monitors.csv"))
        pairs = [("cd", "cd_eff"), ("cl", "cl_eff"), ("cd_v", "cd_veff")] + [
            (c, c) for c in ("cd_eff", "cd_veff", "cl_eff", "cd_p", "res_u", "res_p", "res_nut", "continuity", "nut_avg")]
        n, diff, first = compare_pairs(mine, ref, pairs)
        reason = finish_reason(os.path.join(work, "R3", "solver-status.jsonl"))
        ref_reason = finish_reason(os.path.join(RUNS["R3"][0], "solver-status.jsonl"))
        out["R3"] = {"evaluated": True, "iterations": len(mine), "f10_iterations": len(ref), "iterations_compared": n,
                     "values_different": diff, "first_difference": first, "reason": reason, "f10_reason": ref_reason,
                     "met": len(mine) == len(ref) and n == len(ref) and diff == 0 and reason == ref_reason,
                     "last": {"iteration": mine[-1]["iteration"], "cd": mine[-1]["cd"], "cl": mine[-1]["cl"],
                              "cd_v": mine[-1]["cd_v"], "cd_p": mine[-1]["cd_p"]}}
    else:
        out["R3"] = {"evaluated": False}
    with open(os.path.join(work, "compare.json"), "w", encoding="utf-8") as f:
        json.dump(out, f, indent=1)
    print(json.dumps(out, indent=1))


def selftest():
    a00 = open(os.path.join(RUNS["R1"][0], "case.snapshot"), encoding="utf-8").read()
    t = legacy(a00)
    assert t.count('component = "total_molecular"') == 2 and t.count('turbulence = "zero_gradient"') == 1
    assert len(t.splitlines()) == len(a00.splitlines()) + 3
    f10 = open(os.path.join(RUNS["R3"][0], "case.snapshot"), encoding="utf-8").read()
    t = new_default(f10)
    assert "turbulence = \"freestream\"" not in t and len(t.splitlines()) == len(f10.splitlines()) - 1
    assert bits("1.1126859658358679e0") == bits(1.1126859658358679)
    assert compare_pairs([{"iteration": "1", "a": "1.0"}], [{"iteration": "1", "b": "1.0"}], [("a", "b")])[:2] == (1, 0)
    assert compare_pairs([{"iteration": "1", "a": "1.0"}], [{"iteration": "1", "b": "1.0000000000000002"}],
                         [("a", "b")])[:2] == (1, 1)
    print("selftest passed")


if __name__ == "__main__":
    if len(sys.argv) >= 2 and sys.argv[1] == "selftest":
        selftest()
    elif len(sys.argv) >= 5 and sys.argv[1] == "run":
        run_many(sys.argv[2], sys.argv[3], sys.argv[4:])
    elif len(sys.argv) == 3 and sys.argv[1] == "compare":
        compare(sys.argv[2])
    else:
        raise SystemExit(__doc__)
