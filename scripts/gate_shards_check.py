#!/usr/bin/env python3
"""Show that the shards of one full gate together ran every test once (ADR 0653 Amendment 3, D10).

Usage: python scripts/gate_shards_check.py DIR [DIR ...]
Each DIR holds the logs of one shard (`test.log` and `times.tsv`, as gate.yml uploads them). The check passes when
- the shards are exactly 1 to N for one N,
- every shard states the same total M and the same digest of the split,
- the parts m sum to M,
- each shard's times hold as many result rows (not `ignored`) as its m.
A shard that did not finish has too few rows and fails. Exit status 0 when all hold, 1 otherwise; the reasons are printed.
"""
import os
import re
import sys

LINE = re.compile(r"^gate_test: shard (\d+)/(\d+) runs (\d+) of (\d+) tests, split ([0-9a-f]{64})\s*$", re.M)


def read_shard(folder):
    """(k, n, m, total, digest, rows) of one shard's folder, or a string saying why it cannot be read."""
    log = os.path.join(folder, "test.log")
    if not os.path.isfile(log):
        return f"{folder}: no test.log"
    with open(log, encoding="utf-8", errors="replace") as h:
        hits = LINE.findall(h.read())
    if len(hits) != 1:
        return f"{folder}: {len(hits)} shard lines in test.log, expected 1"
    k, n, m, total, digest = hits[0]
    rows = 0
    tsv = os.path.join(folder, "times.tsv")
    if os.path.isfile(tsv):
        with open(tsv, encoding="utf-8", errors="replace") as h:
            lines = h.read().splitlines()[1:]
        for ln in lines:
            f = ln.split("\t")
            if len(f) >= 3 and f[2] != "ignored":
                rows += 1
    return int(k), int(n), int(m), int(total), digest, rows


def check(shards):
    """The list of reasons the set of shards (results of read_shard) is not a complete run; empty when it is."""
    bad = [s for s in shards if isinstance(s, str)]
    ok = [s for s in shards if not isinstance(s, str)]
    if not ok:
        return bad + ["no shard could be read"]
    ns = sorted({s[1] for s in ok})
    if len(ns) != 1:
        bad.append(f"the shards name different counts N: {ns}")
        return bad
    n = ns[0]
    ks = sorted(s[0] for s in ok)
    if ks != list(range(1, n + 1)):
        bad.append(f"shards present {ks}, expected 1 to {n}")
    totals = sorted({s[3] for s in ok})
    if len(totals) != 1:
        bad.append(f"the shards state different totals M: {totals}")
    if len({s[4] for s in ok}) != 1:
        bad.append("the shards dealt from different lists (the split digests differ)")
    if len(totals) == 1 and sum(s[2] for s in ok) != totals[0] and ks == list(range(1, n + 1)):
        bad.append(f"the parts sum to {sum(s[2] for s in ok)}, the total is {totals[0]}")
    for k, _, m, _, _, rows in ok:
        if rows != m:
            bad.append(f"shard {k}: {rows} result rows in times.tsv, its part is {m}")
    return bad


def main(argv):
    if not argv:
        print(__doc__)
        return 2
    shards = [read_shard(d) for d in argv]
    bad = check(shards)
    ok = [s for s in shards if not isinstance(s, str)]
    if ok:
        print(f"gate_shards_check: {len(ok)} shards, parts {[s[2] for s in sorted(ok)]}, total {sorted({s[3] for s in ok})}")
    for b in bad:
        print("gate_shards_check: FAILED: " + b)
    if not bad:
        print("gate_shards_check: every test ran in exactly one shard")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
