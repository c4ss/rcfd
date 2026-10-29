"""ADR 0156 (job 48V): the registered run matrix (adaptive pressure correctors on the cylinder O-grid family).

usage: python scripts/w48v_run_matrix.py <rcfd binary> <work dir> <id> [<id> ...] [--parallel K]

<work dir> holds a copy of example-projects/wind-tunnel (folder wind-tunnel/). Every run gets <work dir>/runs/<id>/ with
case.snapshot (a copy of the case file), output/, fields/, solver-status.jsonl, log.txt, cmd.txt and wall.json (process wall
time). The command line is the one rcfd-runner builds (crates/rcfd-runner/src/runner.rs, solver_cmd + launch) without its stop
and pause files, plus the registered threads and wall cap (--max-elapsed). Ids are those of ADR 0156's run matrix; 'all' lists
them. Nothing outside <work dir> is written; an existing run folder is never overwritten.
"""
import json
import os
import shutil
import subprocess
import sys
import time

DT16, DT32, DT64, DT128 = 1 / 16, 1 / 32, 1 / 64, 1 / 128
AD = {"solver.corrector_control": "adaptive"}
H = 3600


def arm(*parts):
    d = {}
    for p in parts:
        d.update(p)
    return d


def mesh(nx, ny, dt):
    return {"mesh.nx": nx, "mesh.ny": ny, "solver.dt": dt}


# id -> (overrides of the committed case, end time, threads, wall cap in s)
MATRIX = {
    "V64": (arm(AD), 100.0, 1, 2 * H),
    "V128": (arm(mesh(128, 96, DT32), AD), 100.0, 2, 6 * H),
    "V256": (arm(mesh(256, 256, DT64), AD), 100.0, 4, 10 * H),
    "V512": (arm(mesh(512, 512, DT128), AD), 10.0, 4, 10 * H),
    "V64x256": (arm(mesh(64, 256, DT64), AD), 10.0, 1, 2 * H),
    "TV256": (arm(mesh(256, 256, DT64), AD), 3.0, 1, 2 * H),
    "TW5": (arm(mesh(256, 256, DT64), {"solver.n_correctors": 3}), 3.0, 1, 2 * H),
    "TW3": (arm(mesh(256, 256, DT64), {"solver.time_advance": "fractional_step"}), 3.0, 1, 2 * H),
    "TR256": (arm(mesh(256, 256, DT64)), 3.0, 1, 2 * H),
}


def command(binary, work, rid):
    sets, end, threads, cap = MATRIX[rid]
    cases = os.path.join(work, "wind-tunnel", "cases")
    run = os.path.join(work, "runs", rid)
    if os.path.exists(run):
        raise SystemExit(f"{run} exists; refusing to overwrite a registered run")
    os.makedirs(os.path.join(run, "output"))
    shutil.copyfile(os.path.join(cases, "cylinder-re100.toml"), os.path.join(run, "case.snapshot"))
    cmd = [binary, "run", os.path.join(run, "case.snapshot"), "--case-dir", cases]
    for k, v in sets.items():
        cmd += ["--set", f"{k}={json.dumps(v)}"]
    cmd += ["--set", f"solver.end_time={json.dumps(end)}"]
    cmd += [
        "--out", os.path.join(run, "output"),
        "--status-file", os.path.join(run, "solver-status.jsonl"),
        "--status-every", "1", "--quiet", "--threads", str(threads),
        "--store-dir", os.path.join(run, "fields"),
        "--status-monitors",
        "--max-elapsed", str(cap),
    ]
    with open(os.path.join(run, "cmd.txt"), "w", encoding="utf-8") as f:
        f.write(json.dumps(cmd, indent=1) + "\n")
    return run, cmd


def main():
    args = sys.argv[1:]
    parallel = 6
    if "--parallel" in args:
        i = args.index("--parallel")
        parallel = int(args[i + 1])
        del args[i : i + 2]
    binary, work, ids = args[0], args[1], args[2:]
    if ids == ["all"]:
        print("\n".join(MATRIX))
        return
    unknown = [i for i in ids if i not in MATRIX]
    if unknown:
        raise SystemExit(f"unknown ids {unknown}")
    queue, live = list(ids), []
    while queue or live:
        while queue and len(live) < parallel:
            rid = queue.pop(0)
            run, cmd = command(binary, work, rid)
            log = open(os.path.join(run, "log.txt"), "ab")
            t0 = time.time()
            p = subprocess.Popen(cmd, cwd=run, stdin=subprocess.DEVNULL, stdout=log, stderr=log)
            live.append((rid, run, p, log, t0))
            print(f"started {rid} pid {p.pid} at {time.strftime('%H:%M:%S')}", flush=True)
        time.sleep(2)
        for item in list(live):
            rid, run, p, log, t0 = item
            if p.poll() is not None:
                log.close()
                live.remove(item)
                wall = time.time() - t0
                with open(os.path.join(run, "wall.json"), "w", encoding="utf-8") as f:
                    json.dump({"id": rid, "exit": p.returncode, "process_wall_s": wall, "started": t0}, f)
                print(f"finished {rid} exit {p.returncode} after {wall:.1f} s", flush=True)


if __name__ == "__main__":
    main()
