#!/usr/bin/env python3
"""The replay of ADR 0755 (F-864): the roadmap proposals written from the addenda of six merged ADRs, applied to the roadmap
before the lead's edit and compared with what the lead wrote. Reads the history; writes into the scratch folder only.

    python scripts/roadmap_replay_f864.py commits                  A, L, the base and the start number of each ADR
    python scripts/roadmap_replay_f864.py compare --scratch DIR    apply each proposal and print the differences

The rules are those of the section "The replay, registered" of ADR 0755. On the first-parent line of main up to TIP:
A is the first commit at which the ADR's file has the heading "For the lead: roadmap entries"; L is the first commit at or
after A in which the item's entry in docs/roadmap.json differs from that of its first parent; the base is the merge base
of A's second parent and L^ when A is a merge, and A's first parent when it is not.

Amendment 1 of ADR 0755: a follow-up is matched with the item L adds that has its title, and only then by place; an entry
that belongs to a proposal of another of the six with the same L is not counted; the proposals with one L are also applied
as one set, in both orders (check J).
"""
import argparse
import contextlib
import io
import json
from pathlib import Path
import subprocess
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
import roadmap_proposals as rp  # noqa: E402
import roadmap_shards as rs  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
TIP = "47961cd58bfe64d4368cf19a12907557f20038dd"
ADRS = ("0748", "0749", "0750", "0751", "0752", "0753")
PROPOSALS = Path(__file__).resolve().parent / "roadmap_replay_f864"
HEADING = "For the lead: roadmap entries"
FIELDS = ("title", "deps", "acceptance", "phase", "note")


def git(*args, check=True):
    done = subprocess.run(["git", "-C", str(ROOT), *args], capture_output=True)
    if check and done.returncode != 0:
        raise SystemExit(f"git {' '.join(args)}: {done.stderr.decode('utf-8', 'replace')}")
    return done.stdout.decode("utf-8")


def roadmap_at(commit):
    return json.loads(git("show", f"{commit}:docs/roadmap.json"))


def changed(path, since=None):
    """The first-parent commits up to TIP that change `path` against their first parent, oldest first."""
    span = TIP if since is None else f"{since}^..{TIP}"
    return git("log", "--first-parent", "--reverse", "--format=%H", span, "--", path).split()


def entry(items, item_id):
    return next((e for e in items if e["id"] == item_id), None)


def commits(adr, item_id):
    """(A, L, base, start) by the registered rules."""
    path = git("ls-tree", "--name-only", TIP, "docs/adr/").splitlines()
    path = next(p for p in path if p.startswith(f"docs/adr/{adr}-"))
    a = next(c for c in changed(path)
             if any(ln.startswith("#") and ln.rstrip().endswith(HEADING) for ln in git("show", f"{c}:{path}").splitlines()))
    lead = next(c for c in changed("docs/roadmap.json", since=a)
                if entry(roadmap_at(c), item_id) != entry(roadmap_at(c + "^1"), item_id))
    parents = git("rev-list", "--parents", "-n", "1", a).split()[1:]
    base = git("merge-base", parents[1], lead + "^1").strip() if len(parents) > 1 else parents[0]
    before, after = roadmap_at(lead + "^1"), roadmap_at(lead)
    old = {e["id"] for e in before}
    new = sorted(rp.number(e["id"]) for e in after if e["id"] not in old)
    start = new[0] if new else max(rp.number(i) for i in old) + 1
    return a, lead, base, start


def files_of(adr):
    return sorted(PROPOSALS.glob(f"{adr}-F-*.json"))


def run(*argv):
    out, err = io.StringIO(), io.StringIO()
    with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
        status = rp.main([str(a) for a in argv])
    return status, err.getvalue().strip()


def added_note(before, after):
    old, new = (before or {}).get("note", ""), after.get("note", "")
    return new[len(old):].strip() if old and new.startswith(old) else new


def show(label, ours, theirs):
    print(f"  DIFFERENT {label}")
    print(f"    script: {ours!r}")
    print(f"    lead:   {theirs!r}")


def matched(proposal, before, after):
    """{index of a follow-up: the item L adds for it}: by the title letter for letter, then by place among the rest."""
    known = {e["id"] for e in before}
    left = [e for e in after if e["id"] not in known]
    ups = proposal.get("follow_ups", [])
    pairs = {}
    for n, up in enumerate(ups):
        same = next((e for e in left if e["title"] == up["title"]), None)
        if same is not None:
            pairs[n] = same
            left.remove(same)
    for n in range(len(ups)):
        if n not in pairs and left:
            pairs[n] = left.pop(0)
    return pairs


def of_others(adr, lead):
    """The ids that proposals of the other ADRs with the same L name or are matched to (Amendment 1, point 2)."""
    out = set()
    for other in ADRS:
        if other == adr:
            continue
        for file in files_of(other):
            proposal = rp.read_proposal(file)
            if commits(other, proposal["item"])[1] == lead:
                out.add(proposal["item"])
                out |= {e["id"] for e in matched(proposal, roadmap_at(lead + "^1"), roadmap_at(lead)).values()}
    return out


def joint(scratch):
    """Check J: the proposals that have one L applied as one set, in both orders of the arguments. None when no two
    proposals have one L."""
    groups = {}
    for adr in ADRS:
        for file in files_of(adr):
            groups.setdefault(commits(adr, rp.read_proposal(file)["item"])[1], []).append((adr, file))
    held = None
    for lead, members in groups.items():
        if len(members) < 2:
            continue
        folder = scratch / "replay-joint"
        folder.mkdir(parents=True, exist_ok=True)
        files = [file for _, file in members]
        start = commits(members[0][0], rp.read_proposal(files[0])["item"])[3]
        source = scratch / f"replay-{members[0][0]}" / "before.json"
        before = {e["id"]: e for e in rs.read_json(source)}
        outs = []
        for name, order in (("forward.json", files), ("backward.json", files[::-1])):
            status, message = run("apply", *order, "--repo", ROOT, "--roadmap", source, "--at", lead + "^1", "--start",
                                  start, "--output", folder / name)
            if status != 0:
                print(f"== J, L {lead[:8]}: NOT APPLIED: {message}")
                return False
            outs.append((folder / name).read_bytes())
        both = rs.read_json(folder / "forward.json")
        singles = True
        for adr, _ in members:
            for e in rs.read_json(scratch / f"replay-{adr}" / "after.json"):
                if before.get(e["id"]) != e and entry(both, e["id"]) != e:
                    singles = False
                    print(f"  J: {e['id']} of the set is not that of the single application of ADR {adr}")
        print(f"== J, L {lead[:8]}, ADR {' and '.join(adr for adr, _ in members)}, start {start}: both orders give the "
              f"same bytes: {outs[0] == outs[1]}; entries equal to the single applications: {singles}; "
              f"{len(both) - len(before)} item(s) added")
        held = (held is None or held) and outs[0] == outs[1] and singles
    return held


def compare(adr, scratch):
    """Print the differences of one ADR; return their number."""
    files = files_of(adr)
    proposals = [rp.read_proposal(f) for f in files]
    count = 0
    for proposal, file in zip(proposals, files):
        x = proposal["item"]
        a, lead, base, start = commits(adr, x)
        folder = scratch / f"replay-{adr}"
        folder.mkdir(parents=True, exist_ok=True)
        before, after = roadmap_at(lead + "^1"), roadmap_at(lead)
        source, result = folder / "before.json", folder / "after.json"
        with source.open("w", encoding="utf-8", newline="\n") as fh:
            fh.write(rs.encode(before))
        print(f"\n== ADR {adr}, {x}: A {a[:8]}, L {lead[:8]} ({git('log', '-1', '--format=%s', lead).strip()[:110]})")
        print(f"  base by the rule {base[:8]}; in the file {proposal['base'][:8]}; start {start}")
        if proposal["base"] != base:
            raise SystemExit(f"{file.name}: its base is not the base by the rule")
        args = ("--repo", ROOT, "--roadmap", source, "--at", lead + "^1", "--start", start, "--output", result)
        status, message = run("apply", file, *args)
        if status != 0 and message.startswith(("stale-item: ", "stale-base: ")):
            count += 1
            print(f"  DIFFERENT first application refused, {message}")
            moved = dict(proposal, base=git("rev-parse", lead + "^1").strip())
            copy = folder / file.name
            copy.write_text(json.dumps(moved, ensure_ascii=False, indent=2), encoding="utf-8")
            status, message = run("apply", copy, *args)
        if status != 0:
            print(f"  NOT APPLIED: {message}")
            count += 1
            continue
        ours = rs.read_json(result)
        mine, theirs, was = entry(ours, x), entry(after, x), entry(before, x)
        print(f"  status: before {was['status']}, script {mine['status']}, lead {theirs['status']}")
        if mine["status"] != theirs["status"]:
            count += 1
            show("status", mine["status"], theirs["status"])
        if mine.get("gap") != theirs.get("gap"):
            count += 1
            show("gap", mine.get("gap"), theirs.get("gap"))
        if added_note(was, mine) != added_note(was, theirs):
            count += 1
            show("note added", added_note(was, mine), added_note(was, theirs))
        for key in ("title", "deps", "acceptance", "phase"):
            if was.get(key) != theirs.get(key):
                count += 1
                show(f"{key} of {x} (the form has no key for it)", was.get(key), theirs.get(key))
        known = {e["id"] for e in before}
        our_new = [e for e in ours if e["id"] not in known]
        pairs = matched(proposal, before, after)
        for n, up in enumerate(our_new):
            if n not in pairs:
                count += 1
                print(f"  DIFFERENT follow-up {up['title']!r}: L adds no item for it")
                continue
            other = pairs[n]
            print(f"  follow-up {up['id']} against the lead's {other['id']}")
            for key in FIELDS:
                if up.get(key) != other.get(key):
                    count += 1
                    show(f"{key} of {other['id']}", up.get(key), other.get(key))
        named = {x} | {e["id"] for e in pairs.values()} | of_others(adr, lead)
        for e in after:
            old = entry(before, e["id"])
            if e["id"] in named or e == old:
                continue
            count += 1
            if old is None:
                print(f"  DIFFERENT other entry {e['id']}: added by the lead, not proposed: {e['title']!r}")
            else:
                keys = sorted(k for k in set(e) | set(old) if e.get(k) != old.get(k))
                print(f"  DIFFERENT other entry {e['id']}: the lead changed {', '.join(keys)}; note added: "
                      f"{added_note(old, e)!r}")
    return count


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("commits")
    commands.add_parser("compare").add_argument("--scratch", type=Path, required=True)
    args = parser.parse_args(argv)
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8")
    if args.command == "commits":
        items = dict(line.split() for line in sys.stdin.read().splitlines() if line.strip())
        for adr in ADRS:
            a, lead, base, start = commits(adr, items[adr])
            print(adr, items[adr], "A", a, "L", lead, "base", base, "start", start)
        return 0
    total = 0
    for adr in ADRS:
        n = compare(adr, args.scratch)
        print(f"  differences of ADR {adr}: {n}")
        total += n
    print(f"\ndifferences in all: {total}\n")
    held = joint(args.scratch)
    print("J: " + ("no two proposals have one L" if held is None else "holds" if held else "DOES NOT HOLD"))
    return 0


if __name__ == "__main__":
    sys.exit(main())
