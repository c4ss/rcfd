#!/usr/bin/env python3
"""Prototype a lossless round trip between roadmap.json and item-owned files.

Split only into a new directory. Join writes the compatibility array to stdout.
The existing roadmap remains authoritative until a separate cutover.
"""

import argparse
from collections import deque
import json
from pathlib import Path
import re
import sys
import tempfile


ITEM_ID = re.compile(r"F-[0-9]{3,}")
STATUSES = ("done", "partial", "todo")


def validate(items):
    if not isinstance(items, list) or not items:
        raise ValueError("roadmap must be a nonempty array")
    dependencies = {}
    for item in items:
        if not isinstance(item, dict):
            raise ValueError("each roadmap item must be an object")
        item_id = item.get("id")
        if not isinstance(item_id, str) or not ITEM_ID.fullmatch(item_id):
            raise ValueError(f"invalid item ID {item_id!r}")
        if item_id in dependencies:
            raise ValueError(f"duplicate item {item_id}")
        if item.get("status") not in STATUSES:
            raise ValueError(f"{item_id} has an invalid status")
        if any(not isinstance(item.get(field), str) or not item[field].strip()
               for field in ("title", "acceptance")):
            raise ValueError(f"{item_id} requires a title and acceptance text")
        deps = item.get("deps")
        if not isinstance(deps, list) or any(not isinstance(dep, str) for dep in deps):
            raise ValueError(f"{item_id} requires a list of dependency IDs")
        if len(set(deps)) != len(deps):
            raise ValueError(f"{item_id} repeats a dependency")
        dependencies[item_id] = deps

    dependents = {item_id: [] for item_id in dependencies}
    remaining = {item_id: len(deps) for item_id, deps in dependencies.items()}
    for item_id, deps in dependencies.items():
        for dep in deps:
            if dep not in dependents:
                raise ValueError(f"{item_id} depends on missing item {dep}")
            dependents[dep].append(item_id)
    ready = deque(item_id for item_id, count in remaining.items() if count == 0)
    while ready:
        for item_id in dependents[ready.popleft()]:
            remaining[item_id] -= 1
            if remaining[item_id] == 0:
                ready.append(item_id)
    if any(remaining.values()):
        affected = ", ".join(item_id for item_id, count in remaining.items() if count)
        raise ValueError(f"dependency cycle blocks {affected}")


def read_json(path):
    return json.loads(path.read_text(encoding="utf-8"))


def encode(data):
    return json.dumps(data, ensure_ascii=False, indent=2) + "\n"


def split(source, destination):
    items = read_json(source)
    validate(items)
    if destination.exists() or destination.is_symlink():
        raise ValueError(f"destination already exists: {destination}")
    # Stage beside the destination so promotion stays on the same filesystem.
    with tempfile.TemporaryDirectory(prefix=".roadmap-", dir=destination.parent) as temporary:
        staged = Path(temporary) / "items"
        staged.mkdir()
        for position, item in enumerate(items):
            record = {"legacy_position": position, "item": item}
            with (staged / f"{item['id']}.json").open("w", encoding="utf-8", newline="\n") as file:
                file.write(encode(record))
        staged.rename(destination)


def join(directory):
    if not directory.is_dir():
        raise ValueError(f"not an item directory: {directory}")
    records = []
    old_positions = set()
    for path in sorted(directory.glob("*.json")):
        record = read_json(path)
        item = record.get("item") if isinstance(record, dict) else None
        if not ITEM_ID.fullmatch(path.stem) or not isinstance(item, dict) or item.get("id") != path.stem:
            raise ValueError(f"item ID must match filename: {path.name}")
        position = record.get("legacy_position")
        if position is not None:
            if type(position) is not int or position < 0 or position in old_positions:
                raise ValueError(f"invalid or repeated legacy position in {path.name}")
            old_positions.add(position)
        # Existing consumers get the original order. New IDs need no shared ordering file.
        key = (position is None, position if position is not None else int(path.stem[2:]), path.stem)
        records.append((key, item))
    if not records:
        raise ValueError(f"no item records in {directory}")
    items = [item for _, item in sorted(records)]
    validate(items)
    return items


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    split_command = commands.add_parser("split", help="write one record per item into a new directory")
    split_command.add_argument("source", type=Path)
    split_command.add_argument("destination", type=Path)
    join_command = commands.add_parser("join", help="validate records and emit an ordered JSON array")
    join_command.add_argument("directory", type=Path)
    args = parser.parse_args()
    try:
        if args.command == "split":
            split(args.source, args.destination)
        else:
            sys.stdout.buffer.write(encode(join(args.directory)).encode("utf-8"))
    except (OSError, ValueError) as error:
        parser.exit(1, f"{error}\n")


if __name__ == "__main__":
    main()
