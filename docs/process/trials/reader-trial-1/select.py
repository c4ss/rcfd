#!/usr/bin/env python3
"""Draw the sample of the reader trial (docs/process/trials/reader-trial-1/registration.md).

Run from the repository root: python docs/process/trials/reader-trial-1/select.py
Prints the eligible items, the sample, and which sampled items get which planted defect. Reads only the work tree.
"""
import glob
import importlib.util
import io
import contextlib
import json
import os
import random
import re

SEED = 20261118
CATEGORY = "Structural solver"
N_SAMPLE = 8
KINDS = ["result", "test", "limit", "result", "test", "limit", "none", "none"]
MAX_ADR_BYTES = 60_000
MAX_TEST_BYTES = 60_000


def category_ids():
    spec = importlib.util.spec_from_file_location("rp", "scripts/roadmap_progress.py")
    mod = importlib.util.module_from_spec(spec)
    with contextlib.redirect_stdout(io.StringIO()):
        spec.loader.exec_module(mod)
    for _, name, ids in mod.CATEGORIES:
        if name == CATEGORY:
            return ids
    raise SystemExit("no category " + CATEGORY)


def eligible():
    status = {int(i["id"][2:]): i["status"] for i in json.load(open("docs/roadmap.json", encoding="utf-8"))}
    out = []
    for n in sorted(category_ids()):
        if status.get(n) != "done":
            continue
        adrs = [p for p in glob.glob("docs/adr/[0-9]*.md") if re.search(rf"-f{n}-", os.path.basename(p))]
        if len(adrs) != 1 or os.path.getsize(adrs[0]) > MAX_ADR_BYTES:
            continue
        num = os.path.basename(adrs[0])[:4]
        tests = sorted(
            p.replace("\\", "/")
            for p in glob.glob("crates/*/tests/*.rs")
            if re.search(rf"(_f{n}|adr{num})(\D|$)", os.path.basename(p)[:-3])
        )
        if not tests or sum(os.path.getsize(t) for t in tests) > MAX_TEST_BYTES:
            continue
        out.append((n, adrs[0].replace("\\", "/"), tests))
    return out


def main():
    el = eligible()
    print(f"eligible: {len(el)}: " + " ".join(f"F-{n}" for n, _, _ in el))
    rng = random.Random(SEED)
    sample = rng.sample(el, N_SAMPLE)
    kinds = KINDS[:]
    rng.shuffle(kinds)
    for (n, adr, tests), k in zip(sample, kinds):
        print(f"F-{n}\t{k}\t{adr}\t{' '.join(tests)}")


if __name__ == "__main__":
    main()
