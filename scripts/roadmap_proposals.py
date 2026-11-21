#!/usr/bin/env python3
"""Validate roadmap proposals of form 1 and apply them to a roadmap (ADR 0755, F-864). Shadow mode: docs/roadmap.json stays
the one source and is written by the lead; this script writes it only when it is named as output with --in-place.

A proposal is one JSON file per item, named <adr>-<item>.json:

    {"form": 1, "adr": "0755", "item": "F-864", "base": "<40 hex: the commit of main it was written against>",
     "status": "done" | "partial" | "todo", "note": "ADR 0755, 2026-11-21, done: one sentence.",
     "gap": "only with partial: one line of at most 500 characters; it replaces the previous gap",
     "follow_ups": [{"title": "...", "deps": ["F-001", "new:<key of another follow-up>"], "acceptance": "...",
                     "phase": 0..9 (default: the phase of the item), "key": "a-name"}]}

Usage (from the repository root):
    python scripts/roadmap_proposals.py check FILE... [--roadmap PATH] [--at REV] [--repo DIR] [--start N]
    python scripts/roadmap_proposals.py apply FILE... [--roadmap PATH] [--at REV] [--repo DIR] [--start N]
                                                      [--output PATH] [--in-place]

--roadmap is the roadmap read (default docs/roadmap.json of the repository), --at the commit it is from (default HEAD),
--start the number of the first new id. apply writes the result to standard output or to --output. Every failure has a
name (malformed, unknown-item, same-item, stale-base, stale-item, duplicate-id, missing-dependency, cycle,
invalid-roadmap, refused-output), is printed as "<name>: <message>" with exit status 1, and nothing is written.
"""
import argparse
import copy
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile

sys.path.insert(0, str(Path(__file__).resolve().parent))
import roadmap_shards as rs  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
ROADMAP = "docs/roadmap.json"
KEYS = {"form", "adr", "item", "base", "status", "note", "gap", "follow_ups"}
REQUIRED = KEYS - {"gap", "follow_ups"}
FOLLOW_KEYS = {"title", "deps", "acceptance", "phase", "key"}
FOLLOW_REQUIRED = {"title", "deps", "acceptance"}
ADR = re.compile(r"[0-9]{4}")
BASE = re.compile(r"[0-9a-f]{40}")
KEY = re.compile(r"[a-z0-9]+(-[a-z0-9]+)*")
DATE = r"[0-9]{4}-[0-9]{2}-[0-9]{2}"
PHASES = range(10)
GAP_MAX = 500


class ProposalError(ValueError):
    """A named failure: str() is "<name>: <message>"."""

    def __init__(self, name, message):
        super().__init__(f"{name}: {message}")
        self.name = name


def one_line(value):
    return isinstance(value, str) and bool(value.strip()) and "\n" not in value and "\r" not in value


def check_shape(proposal, where="proposal"):
    """The keys and values of one proposal, without a roadmap. Raises `malformed`."""
    def bad(message):
        raise ProposalError("malformed", f"{where}: {message}")

    if not isinstance(proposal, dict):
        bad("not an object")
    if set(proposal) - KEYS:
        bad(f"unknown key {sorted(set(proposal) - KEYS)}")
    if REQUIRED - set(proposal):
        bad(f"missing key {sorted(REQUIRED - set(proposal))}")
    if type(proposal["form"]) is not int or proposal["form"] != 1:
        bad(f"form {proposal['form']!r} is not 1")
    for key, shape, what in (("adr", ADR, "four digits"), ("item", rs.ITEM_ID, "an id F-NNN"),
                             ("base", BASE, "the 40 hexadecimal characters of a commit")):
        if not isinstance(proposal[key], str) or not shape.fullmatch(proposal[key]):
            bad(f"{key} {proposal[key]!r} is not {what}")
    if proposal["status"] not in rs.STATUSES:
        bad(f"status {proposal['status']!r} is not one of {', '.join(rs.STATUSES)}")
    note = proposal["note"]
    if not one_line(note):
        bad("the note is not one line of text")
    if not re.match(rf"ADR {proposal['adr']}, {DATE}, \S", note):
        bad(f"the note does not start \"ADR {proposal['adr']}, <YYYY-MM-DD>, \"")
    if "gap" in proposal:
        if proposal["status"] != "partial":
            bad("a gap with a status that is not partial")
        if not one_line(proposal["gap"]) or len(proposal["gap"]) > GAP_MAX:
            bad(f"the gap is not one line of 1 to {GAP_MAX} characters")
    ups = proposal.get("follow_ups", [])
    if not isinstance(ups, list):
        bad("follow_ups is not a list")
    for n, up in enumerate(ups, 1):
        if not isinstance(up, dict):
            bad(f"follow-up {n} is not an object")
        if set(up) - FOLLOW_KEYS:
            bad(f"follow-up {n} has an unknown key {sorted(set(up) - FOLLOW_KEYS)}")
        if FOLLOW_REQUIRED - set(up):
            bad(f"follow-up {n} has no {sorted(FOLLOW_REQUIRED - set(up))}")
        for key in ("title", "acceptance"):
            if not one_line(up[key]):
                bad(f"the {key} of follow-up {n} is not one line of text")
        deps = up["deps"]
        if not isinstance(deps, list) or any(
                not isinstance(d, str) or not (rs.ITEM_ID.fullmatch(d) or (d.startswith("new:") and KEY.fullmatch(d[4:])))
                for d in deps):
            bad(f"the deps of follow-up {n} are not a list of ids and new:<key>")
        if len(set(deps)) != len(deps):
            bad(f"follow-up {n} repeats a dependency")
        if "phase" in up and (type(up["phase"]) is not int or up["phase"] not in PHASES):
            bad(f"the phase of follow-up {n} is not 0 to 9")
        if "key" in up and (not isinstance(up["key"], str) or not KEY.fullmatch(up["key"])):
            bad(f"the key of follow-up {n} is not a name of lower-case letters, digits and hyphens")


def read_proposal(path):
    """The proposal of a file, with its shape and its file name checked. Raises `malformed`."""
    path = Path(path)
    try:
        proposal = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as error:
        raise ProposalError("malformed", f"{path.name}: {error}") from None
    check_shape(proposal, path.name)
    if path.name != f"{proposal['adr']}-{proposal['item']}.json":
        raise ProposalError("malformed", f"{path.name}: the file name is not {proposal['adr']}-{proposal['item']}.json")
    return proposal


def git_base_items(repo, at="HEAD"):
    """A lookup for apply(): base -> the items of docs/roadmap.json at that commit, which must be `at` or an ancestor of
    it. Raises `stale-base`."""
    def git(*args):
        return subprocess.run(["git", "-C", str(repo), *args], capture_output=True)

    def lookup(base):
        if git("cat-file", "-e", base + "^{commit}").returncode != 0:
            raise ProposalError("stale-base", f"base {base} is not a commit of {repo}")
        if git("merge-base", "--is-ancestor", base, at).returncode != 0:
            raise ProposalError("stale-base", f"base {base} is not an ancestor of {at}, the commit of the roadmap")
        shown = git("show", f"{base}:{ROADMAP}")
        if shown.returncode != 0:
            raise ProposalError("stale-base", f"base {base} has no {ROADMAP}")
        return json.loads(shown.stdout.decode("utf-8"))

    return lookup


def number(item_id):
    return int(item_id[2:])


def new_id(n):
    return f"F-{n:03d}"


def validated(items, what, given=False):
    """roadmap_shards.validate and the gap rule. A fault of the roadmap given is `invalid-roadmap` whatever it is; a
    fault of the result comes from the follow-ups and has their names."""
    try:
        rs.validate(items)
    except ValueError as error:
        text = str(error)
        name = ("invalid-roadmap" if given else
                "duplicate-id" if text.startswith("duplicate item") else
                "missing-dependency" if "depends on missing item" in text else
                "cycle" if text.startswith("dependency cycle") else "invalid-roadmap")
        raise ProposalError(name, f"{what}: {text}") from None
    for entry in items:
        gap = entry.get("gap")
        if gap is not None and (entry["status"] != "partial" or not one_line(gap) or len(gap) > GAP_MAX):
            raise ProposalError("invalid-roadmap", f"{what}: {entry['id']} has a gap that is not one line of 1 to "
                                                   f"{GAP_MAX} characters on a partial item")


def apply(items, proposals, start=None, base_items=None):
    """The roadmap with the proposals applied, as a new list; `items` and `proposals` are not changed. `start` is the
    number of the first new id (the highest number plus one without it). `base_items(base)` gives the items at a base
    (git_base_items); without it the stale checks are not made. Raises ProposalError."""
    for proposal in proposals:
        check_shape(proposal, f"{proposal.get('adr')}-{proposal.get('item')}" if isinstance(proposal, dict) else "proposal")
    validated(items, "the roadmap given", given=True)
    by_id = {entry["id"]: entry for entry in items}
    seen = {}
    for proposal in proposals:
        item_id = proposal["item"]
        if item_id not in by_id:
            raise ProposalError("unknown-item", f"ADR {proposal['adr']}: {item_id} is not in the roadmap")
        if item_id in seen:
            raise ProposalError("same-item", f"{item_id} has two proposals, of ADR {seen[item_id]} and ADR "
                                             f"{proposal['adr']}; they are applied one by one, by the lead")
        seen[item_id] = proposal["adr"]
    ordered = sorted(proposals, key=lambda p: (p["adr"], number(p["item"])))
    if base_items is not None:
        for proposal in ordered:
            item_id, base = proposal["item"], proposal["base"]
            then = next((e for e in base_items(base) if e.get("id") == item_id), None)
            if then is None:
                raise ProposalError("stale-base", f"{item_id} is not in the roadmap of base {base}")
            now = by_id[item_id]
            moved = sorted(k for k in set(then) | set(now) if then.get(k) != now.get(k))
            if moved:
                raise ProposalError("stale-item", f"{item_id} changed since base {base[:8]}: {', '.join(moved)}")

    out = copy.deepcopy(items)
    place = {entry["id"]: n for n, entry in enumerate(out)}
    next_number = max(number(i) for i in by_id) + 1 if start is None else start
    added = []
    for proposal in ordered:
        old = out[place[proposal["item"]]]
        gap = proposal.get("gap", old.get("gap")) if proposal["status"] == "partial" else None
        note = (old["note"] + " " if old.get("note") else "") + proposal["note"]
        entry = {k: v for k, v in old.items() if k not in ("gap", "note")}
        entry["status"] = proposal["status"]
        if gap is not None:
            entry["gap"] = gap
        entry["note"] = note
        out[place[proposal["item"]]] = entry

        ups = proposal.get("follow_ups", [])
        keys = {}
        for n, up in enumerate(ups):
            if "key" in up:
                if up["key"] in keys:
                    raise ProposalError("duplicate-id", f"ADR {proposal['adr']}, {proposal['item']}: two follow-ups have "
                                                        f"the key {up['key']}")
                keys[up["key"]] = new_id(next_number + n)
        for up in ups:
            item_id = new_id(next_number)
            next_number += 1
            if item_id in by_id:
                raise ProposalError("duplicate-id", f"{item_id} exists already: the start number is too low")
            deps = []
            for dep in up["deps"]:
                if dep.startswith("new:"):
                    if dep[4:] not in keys:
                        raise ProposalError("missing-dependency", f"ADR {proposal['adr']}, {proposal['item']}: the "
                                                                  f"follow-up \"{up['title']}\" depends on {dep}, and "
                                                                  "no follow-up of the file has that key")
                    deps.append(keys[dep[4:]])
                else:
                    deps.append(dep)
            added.append({"id": item_id, "title": up["title"], "phase": up.get("phase", old["phase"]), "deps": deps,
                          "status": "todo", "acceptance": up["acceptance"]})
    out += added
    validated(out, "the result")
    return out


def summary(items, out):
    """One line per entry that the application changes or adds."""
    before = {entry["id"]: entry for entry in items}
    lines = []
    for entry in out:
        old = before.get(entry["id"])
        if old is None:
            lines.append(f"{entry['id']}: new, todo, phase {entry['phase']}, deps {', '.join(entry['deps']) or 'none'}: "
                         f"{entry['title']}")
        elif old != entry:
            gap = "" if old.get("gap") == entry.get("gap") else "; gap " + ("set" if "gap" in entry else "removed")
            lines.append(f"{entry['id']}: {old['status']} -> {entry['status']}{gap}; note + "
                         f"{len(entry['note']) - len(old.get('note', ''))} characters")
    return lines


def same_file(a, b):
    return os.path.normcase(os.path.realpath(a)) == os.path.normcase(os.path.realpath(b))


def write_atomically(path, text):
    """Through a temporary file beside the target, so that a failure leaves no half-written file."""
    path = Path(path)
    handle, temporary = tempfile.mkstemp(prefix=".roadmap-", suffix=".tmp", dir=path.parent)
    try:
        with os.fdopen(handle, "w", encoding="utf-8", newline="\n") as file:
            file.write(text)
        os.replace(temporary, path)
    except BaseException:
        if os.path.exists(temporary):
            os.unlink(temporary)
        raise


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    commands = parser.add_subparsers(dest="command", required=True)
    for name, text in (("check", "validate the proposals against the roadmap and say what they would change"),
                       ("apply", "validate and write the roadmap with the proposals applied")):
        command = commands.add_parser(name, help=text)
        command.add_argument("files", nargs="+", type=Path, metavar="FILE")
        command.add_argument("--repo", type=Path, default=ROOT)
        command.add_argument("--roadmap", type=Path, help=f"the roadmap read (default: {ROADMAP} of the repository)")
        command.add_argument("--at", default="HEAD", help="the commit the roadmap read is from (default: HEAD)")
        command.add_argument("--start", type=int, help="the number of the first new id")
        if name == "apply":
            command.add_argument("--output", type=Path, help="where the result is written (default: standard output)")
            command.add_argument("--in-place", action="store_true", help="allow --output to be the roadmap read")
    args = parser.parse_args(argv)
    source = args.roadmap if args.roadmap is not None else args.repo / ROADMAP
    try:
        proposals = [read_proposal(path) for path in args.files]
        if args.command == "apply":
            if args.start is None and any(p.get("follow_ups") for p in proposals):
                parser.error("a follow-up needs --start, the number of the first new id")
            if args.output is not None and same_file(args.output, source) and not args.in_place:
                raise ProposalError("refused-output", f"{args.output} is the roadmap that is read; give another path, or "
                                                      "--in-place")
        try:
            items = rs.read_json(source)
        except (OSError, ValueError) as error:
            raise ProposalError("invalid-roadmap", f"{source}: {error}") from None
        out = apply(items, proposals, start=args.start, base_items=git_base_items(args.repo, args.at))
        if args.command == "check":
            print("\n".join(summary(items, out)))
        elif args.output is None:
            sys.stdout.write(rs.encode(out))
        else:
            write_atomically(args.output, rs.encode(out))
    except ProposalError as error:
        print(error, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
