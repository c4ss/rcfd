#!/usr/bin/env python3
"""Check of F-849 (ADR 0740) from the logs of full gates: every test of the limits table took at most half of its limit.

Usage:
    python scripts/gate_limit_check.py --limits scripts/gate_limits.tsv GATE GATE GATE [GATE ...]
GATE       a folder with the downloaded log artifacts of one gate run (`gh run download RUN -p "gate-<os>-*" -D GATE`): one
           times.tsv, or one per shard, anywhere below it. times_hooks.tsv is not read.
--gates N     the least number of gates (default 3, the acceptance of F-849).
--fraction F  the share of its limit a test may take (default 0.5).
For every row of the table and every gate there must be exactly one row of that test in the gate's times, with outcome `ok`
and at most F times the limit in seconds. One line per test and gate is printed. Exit status 0 only if at least N gates were
given and every line is met. Whether the gates are consecutive full gates on the wanted runner is not something the logs
say: the folders read are printed for the reader to compare with the run list.
"""
import argparse
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from gate_test import read_limits  # noqa: E402


def times_files(gate):
    """Every times.tsv below the folder `gate` (or `gate` itself if it is such a file), sorted."""
    if os.path.isfile(gate):
        return [gate]
    return sorted(os.path.join(d, f) for d, _, files in os.walk(gate) for f in files if f == "times.tsv")


def read_rows(paths):
    """{(binary, test): [(outcome, seconds), ...]} of the given times files."""
    rows = {}
    for p in paths:
        with open(p, encoding="utf-8") as fh:
            for ln in fh.read().splitlines()[1:]:
                f = ln.split("\t")
                if len(f) < 4:
                    continue
                try:
                    sec = float(f[3])
                except ValueError:
                    sec = float("nan")
                rows.setdefault((f[0], f[1]), []).append((f[2], sec))
    return rows


def judge(found, limit, fraction):
    """(met, text) for the rows `found` of one test in one gate."""
    half = fraction * limit
    if not found:
        return False, "no row of this test (not run, removed or renamed)"
    if len(found) > 1:
        return False, f"{len(found)} rows of this test"
    outcome, sec = found[0]
    if outcome != "ok":
        return False, f"outcome {outcome} after {sec:.0f} s"
    if not sec <= half:
        return False, f"{sec:.0f} s is above {half:.0f} s ({fraction:g} of the limit {limit:.0f} s)"
    return True, f"{sec:.0f} s of {half:.0f} s ({fraction:g} of the limit {limit:.0f} s)"


def check(table, gates, need=3, fraction=0.5):
    """The lines of the report and whether the check is met."""
    lines, met = [], True
    for g in gates:
        files = times_files(g)
        lines.append(f"gate {g}: {len(files)} times file(s)")
        if not files:
            met = False
            lines.append(f"  MISSED {g}: no times.tsv (the tests step did not finish)")
        rows = read_rows(files)
        for (binary, test), limit in sorted(table.items()):
            ok, text = judge(rows.get((binary, test), []), limit, fraction)
            met = met and ok
            lines.append(f"  {'met   ' if ok else 'MISSED'} {binary}::{test} in {g}: {text}")
    if len(gates) < need:
        met = False
        lines.append(f"MISSED: {len(gates)} gate(s) given, {need} are needed")
    if not table:
        met = False
        lines.append("MISSED: the limits table has no row")
    lines.append(f"gate_limit_check: {'met' if met else 'MISSED'} ({len(table)} tests, {len(gates)} gates)")
    return lines, met


def main(argv=None):
    ap = argparse.ArgumentParser()
    ap.add_argument("--limits", required=True)
    ap.add_argument("--gates", type=int, default=3)
    ap.add_argument("--fraction", type=float, default=0.5)
    ap.add_argument("gate", nargs="*")
    a = ap.parse_args(argv)
    lines, met = check(read_limits(a.limits), a.gate, a.gates, a.fraction)
    print("\n".join(lines))
    return 0 if met else 1


if __name__ == "__main__":
    sys.exit(main())
