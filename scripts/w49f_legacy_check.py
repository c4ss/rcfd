"""ADR 0169 R1 (job 49F): the old far-field behaviour stays spellable.

    python scripts/w49f_legacy_check.py run --bin-dir DIR --work DIR [--base COMMIT]
    python scripts/w49f_legacy_check.py compare --work DIR [--base COMMIT]

`run` copies example-projects/wind-tunnel (without its runs) to <work>/wind-tunnel, adds the pre-ADR 0166/0169 spellings
to cases/naca0012-aoa4.toml (`turbulence = "zero_gradient"` on the far field, `component = "total_molecular"` on `cl`
and `cd`), and runs that one case through the run service exactly as scripts/make_example_projects.py does (one
thread). `compare` checks the run against the stored run r-20261028T152128-cb3a as committed at <base> (default 1a04d46,
the base of job 49F, before it regenerated the run): residuals.jsonl byte for byte, and the 10 stored snapshots of ux, uy, p, nut bit for bit.
Writes <work>/compare.json and exits 0 when everything matches.
"""

import argparse
import json
import os
import shutil
import struct
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import make_example_projects as mep  # noqa: E402

CASE = "cases/naca0012-aoa4.toml"
STORED = "example-projects/wind-tunnel/runs/r-20261028T152128-cb3a"
FIELDS = ["ux", "uy", "p", "nut"]

EDITS = [
    ('[boundaries.farfield]\nvelocity = "freestream"\nvelocity_value = [0.9975640502598242, 0.0697564737441253, 0.0]\n'
     'pressure = "freestream"\n',
     '[boundaries.farfield]\nvelocity = "freestream"\nvelocity_value = [0.9975640502598242, 0.0697564737441253, 0.0]\n'
     'pressure = "freestream"\nturbulence = "zero_gradient"   # ADR 0169 D5: the far field before ADR 0169\n'),
    ('[reports.cl]\ntype = "force_coefficient"\n',
     '[reports.cl]\ntype = "force_coefficient"\ncomponent = "total_molecular"   # ADR 0166 D4: the force before ADR 0166\n'),
    ('[reports.cd]\ntype = "force_coefficient"\n',
     '[reports.cd]\ntype = "force_coefficient"\ncomponent = "total_molecular"\n'),
]


def git_show(base, path):
    r = subprocess.run(["git", "-C", mep.REPO, "show", "%s:%s" % (base, path)], capture_output=True)
    if r.returncode != 0:
        raise RuntimeError("git show %s:%s: %s" % (base, path, r.stderr.decode("utf-8", "replace")))
    return r.stdout


def do_run(a):
    pdir = os.path.join(a.work, "wind-tunnel")
    if os.path.exists(pdir):
        sys.exit("%s exists: use a fresh --work folder" % pdir)
    shutil.copytree(os.path.join(mep.EXAMPLES, "wind-tunnel"), pdir, ignore=shutil.ignore_patterns("runs"))
    os.makedirs(os.path.join(pdir, "runs"))
    case = os.path.join(pdir, CASE.replace("/", os.sep))
    text = mep.read(case).decode("utf-8")
    for old, new in EDITS:
        if text.count(old) != 1:
            sys.exit("%s: the text to edit is not found once: %r" % (CASE, old))
        text = text.replace(old, new)
    mep.write_text(case, text)
    bins = mep.binaries(os.path.abspath(a.bin_dir))
    mep.clean_dot(pdir)
    runner = mep.Runner(pdir, bins, 1)
    runner.start()
    try:
        rid = runner.submit(case)
        runner.wait(rid, 3600)
    finally:
        runner.stop()
    mep.clean_dot(pdir)
    st = mep.read_json(os.path.join(pdir, "runs", rid, "status.json"))
    print("R1 run %s: %s, step %s, %s" % (rid, st["state"], st["step"], st.get("reason", "")))
    with open(os.path.join(a.work, "run_id.txt"), "w") as f:
        f.write(rid + "\n")
    return 0


def field_bytes(store, i, name):
    vals = mep.store_field(store, i, name)
    return struct.pack("<%df" % len(vals), *vals)


def do_compare(a):
    rid = open(os.path.join(a.work, "run_id.txt")).read().strip()
    rd = os.path.join(a.work, "wind-tunnel", "runs", rid)
    out = {"run": rid, "base": a.base, "stored": STORED}
    st = mep.read_json(os.path.join(rd, "status.json"))
    met = mep.read_json(os.path.join(rd, "metrics.json"))
    out["state"] = st["state"]
    out["solver_state"] = (met.get("solver_finish") or {}).get("state")
    out["steps"] = met.get("steps")
    new_res = mep.read(os.path.join(rd, "residuals.jsonl"))
    old_res = git_show(a.base, STORED + "/residuals.jsonl")
    out["residuals_lines"] = [len(new_res.splitlines()), len(old_res.splitlines())]
    out["residuals_byte_equal"] = new_res == old_res
    if not out["residuals_byte_equal"]:
        for i, (x, y) in enumerate(zip(new_res.splitlines(), old_res.splitlines())):
            if x != y:
                out["first_difference_line"] = i + 1
                break
    tmp = os.path.join(a.work, "stored-store.rts")
    with open(tmp, "wb") as f:
        f.write(git_show(a.base, STORED + "/fields/store.rts"))
    _, _, s_new = mep.store_snapshots(os.path.join(rd, "fields", "store.rts"))
    _, _, s_old = mep.store_snapshots(tmp)
    out["snapshots"] = [[s[0] for s in s_new], [s[0] for s in s_old]]
    diffs = []
    compared = 0
    if out["snapshots"][0] == out["snapshots"][1]:
        for i in range(len(s_new)):
            for name in FIELDS:
                compared += 1
                if field_bytes(os.path.join(rd, "fields", "store.rts"), i, name) != field_bytes(tmp, i, name):
                    diffs.append([s_new[i][0], name])
    out["fields_compared"] = compared
    out["fields_different"] = diffs
    out["met"] = (out["state"] == "finished" and out["solver_state"] == "not_converged" and out["steps"] == 2000
                  and out["residuals_byte_equal"] and out["residuals_lines"][0] == 2000 and compared == 40
                  and not diffs)
    mep.write_text(os.path.join(a.work, "compare.json"), json.dumps(out, indent=2) + "\n")
    print(json.dumps(out, indent=2))
    return 0 if out["met"] else 1


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("what", choices=["run", "compare"])
    ap.add_argument("--bin-dir", default=os.path.join(mep.REPO, "target", "release"))
    ap.add_argument("--work", required=True)
    ap.add_argument("--base", default="1a04d46")
    a = ap.parse_args()
    a.work = os.path.abspath(a.work)
    os.makedirs(a.work, exist_ok=True)
    return do_run(a) if a.what == "run" else do_compare(a)


if __name__ == "__main__":
    sys.exit(main())
