"""ADR 0154 (job 48T): the registered run matrix of the cylinder O-grid spoke investigation.

usage: python scripts/w48t_run_matrix.py <rcfd binary> <work dir> <id> [<id> ...] [--threads N] [--parallel K]

<work dir> holds a copy of example-projects/wind-tunnel (folder wind-tunnel/) and, for S0b, the user's saved case copied to
wind-tunnel/cases/cylinder-re100.user-saved.toml. Every run gets <work dir>/runs/<id>/ with case.snapshot (a copy of the case
file), output/, fields/, solver-status.jsonl, log.txt and cmd.txt. The command line is the one rcfd-runner builds
(crates/rcfd-runner/src/runner.rs, solver_cmd + launch) without its stop and pause files. Ids are those of ADR 0154's set-up
table; 'all' lists them. Nothing outside <work dir> is written.
"""
import json
import os
import shutil
import subprocess
import sys
import time

DT16, DT32, DT64, DT128, DT256 = 1 / 16, 1 / 32, 1 / 64, 1 / 128, 1 / 256
R256 = {"mesh.nx": 256, "mesh.ny": 256, "solver.dt": DT64}


def arm(base, **extra):
    d = dict(base)
    d.update({k.replace("__", "."): v for k, v in extra.items()})
    return d


# id -> (case file, overrides, end time, restart spec or None)
MATRIX = {
    "S0a": ("cylinder-re100.toml", {"mesh.nx": 64, "mesh.ny": 48, "solver.dt": 0.0625}, 1.25, None),
    "S0b": ("cylinder-re100.user-saved.toml", {"mesh.nx": 64, "mesh.ny": 48, "solver.dt": 0.0625}, 1.25, None),
    "R64": ("cylinder-re100.toml", {}, 10.0, None),
    "R128": ("cylinder-re100.toml", {"mesh.nx": 128, "mesh.ny": 96, "solver.dt": DT32}, 10.0, None),
    "R256": ("cylinder-re100.toml", R256, 30.0, None),
    "RU": ("cylinder-re100.toml", {"mesh.nx": 256, "mesh.ny": 256, "solver.dt": 0.0156}, 10.0, None),
    "B1": ("cylinder-re100.toml", {"mesh.nx": 256, "mesh.ny": 48, "solver.dt": DT64}, 10.0, None),
    "B1b": ("cylinder-re100.toml", {"mesh.nx": 256, "mesh.ny": 48, "solver.dt": DT16}, 10.0, None),
    "B2": ("cylinder-re100.toml", {"mesh.nx": 64, "mesh.ny": 256, "solver.dt": DT16}, 10.0, None),
    "B2b": ("cylinder-re100.toml", {"mesh.nx": 64, "mesh.ny": 256, "solver.dt": DT64}, 10.0, None),
    "B3": ("cylinder-re100.toml", arm(R256, mesh__grading=10.88), 10.0, None),
    "B4a": ("cylinder-re100.toml", arm(R256, solver__dt=DT16), 3.0, None),
    "B4b": ("cylinder-re100.toml", arm(R256, solver__dt=DT128), 3.0, None),
    "B4c": ("cylinder-re100.toml", arm(R256, solver__dt=DT256), 3.0, None),
    "B5": ("cylinder-re100.toml", arm(R256, initial__velocity_expression_y="0 [m/s]"), 3.0, None),
    "B6": ("cylinder-re100.toml", R256, 13.0, ("R64", "cylinder-re100.toml")),
    "B6r": (
        "cylinder-re100.toml",
        arm(
            R256,
            boundaries__farfield__velocity="fixed",
            boundaries__farfield__velocity_expression_x="1 [m/s] * (1 - exp(-t / 0.5 [s]))",
            initial__velocity_expression_x="0 [m/s]",
            initial__velocity_expression_y="0 [m/s]",
        ),
        3.0,
        None,
    ),
    "B7": ("cylinder-re100.toml", arm(R256, solver__convection="upwind"), 3.0, None),
    "B8a": ("cylinder-re100.toml", arm(R256, solver__n_correctors=3), 3.0, None),
    "B8b": ("cylinder-re100.toml", arm(R256, solver__n_correctors=4), 3.0, None),
    "B9": ("cylinder-re100.toml", arm(R256, solver__non_orthogonal_correctors=1), 3.0, None),
    "B10": (
        "cylinder-re100.toml",
        arm(
            R256,
            solver__pressure_rel_tol=1e-12,
            solver__pressure_abs_tol=1e-20,
            solver__pressure_max_iter=20000,
            solver__momentum_rel_tol=1e-12,
            solver__momentum_max_iter=5000,
        ),
        3.0,
        None,
    ),
    "B11": ("cylinder-re100.toml", arm(R256, solver__ddt_correction=False), 3.0, None),
    "B12": ("cylinder-re100.toml", arm(R256, solver__time_scheme="bdf2"), 3.0, None),
    "W1": (
        "cylinder-re100.toml",
        arm(R256, solver__algorithm="pimple", solver__n_outer=2, solver__alpha_u=0.9, solver__alpha_p=0.9),
        10.0,
        None,
    ),
    "W2": (
        "cylinder-re100.toml",
        arm(R256, solver__algorithm="pimple", solver__n_outer=4, solver__alpha_u=0.9, solver__alpha_p=0.9),
        10.0,
        None,
    ),
    "W3": ("cylinder-re100.toml", arm(R256, solver__time_advance="fractional_step"), 10.0, None),
    "W4": ("cylinder-re100.toml", arm(R256, solver__dt=DT256), 10.0, None),
    # amendment 2
    "W5": ("cylinder-re100.toml", arm(R256, solver__n_correctors=3), 10.0, None),
    "W6": (
        "cylinder-re100.toml",
        {"mesh.nx": 256, "mesh.ny": 256, "solver.dt": 0.0156, "solver.n_correctors": 3},
        10.0,
        None,
    ),
    "B2c": ("cylinder-re100.toml", {"mesh.nx": 64, "mesh.ny": 256, "solver.dt": DT64, "solver.n_correctors": 3}, 3.0, None),
}


def command(binary, work, rid, threads):
    case_file, sets, end, restart = MATRIX[rid]
    cases = os.path.join(work, "wind-tunnel", "cases")
    run = os.path.join(work, "runs", rid)
    if os.path.exists(run):
        raise SystemExit(f"{run} exists; refusing to overwrite a registered run")
    os.makedirs(os.path.join(run, "output"))
    shutil.copyfile(os.path.join(cases, case_file), os.path.join(run, "case.snapshot"))
    cmd = [binary, "run", os.path.join(run, "case.snapshot"), "--case-dir", cases]
    for k, v in sets.items():
        cmd += ["--set", f"{k}={json.dumps(v)}"]
    cmd += ["--set", f"solver.end_time={json.dumps(end)}"]
    if restart:
        src_id, src_case = restart
        cmd += [
            "--restart-from",
            os.path.join(work, "runs", src_id, "output", "restart.rcfd"),
            "--source-case",
            os.path.join(cases, src_case),
            "--method",
            "gradient",
        ]
    cmd += [
        "--out", os.path.join(run, "output"),
        "--status-file", os.path.join(run, "solver-status.jsonl"),
        "--status-every", "1", "--quiet", "--threads", str(threads),
        "--store-dir", os.path.join(run, "fields"),
        "--status-monitors",
    ]
    with open(os.path.join(run, "cmd.txt"), "w", encoding="utf-8") as f:
        f.write(json.dumps(cmd, indent=1) + "\n")
    return run, cmd


def main():
    args = sys.argv[1:]
    threads, parallel = 1, 4
    if "--threads" in args:
        i = args.index("--threads")
        threads = int(args[i + 1])
        del args[i : i + 2]
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
            run, cmd = command(binary, work, rid, threads)
            log = open(os.path.join(run, "log.txt"), "ab")
            p = subprocess.Popen(cmd, cwd=run, stdin=subprocess.DEVNULL, stdout=log, stderr=log)
            live.append((rid, p, log, time.time()))
            print(f"started {rid} pid {p.pid}", flush=True)
        time.sleep(2)
        for item in list(live):
            rid, p, log, t0 = item
            if p.poll() is not None:
                log.close()
                live.remove(item)
                print(f"finished {rid} exit {p.returncode} after {time.time() - t0:.1f} s", flush=True)


if __name__ == "__main__":
    main()
