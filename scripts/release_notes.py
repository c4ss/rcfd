#!/usr/bin/env python3
"""Check that a release tag matches the repository and print its notes from CHANGELOG.md (docs/releasing.md).

Usage (from the workspace root):
    python scripts/release_notes.py v0.1.0 [--out notes.md]
Fails (exit 1, reason on stderr) unless:
  * the tag is vX.Y.Z,
  * X.Y.Z is the version in Cargo.toml [workspace.package], python/rcfd/__init__.py and the Claude Code plugin manifest,
  * CHANGELOG.md has a heading "## X.Y.Z - YYYY-MM-DD" (a real date) with a non-empty body.
The notes are that section's body, up to the next "## " heading. Python 3.9 or later, standard library only.
"""
import argparse
import json
import re
import sys

PLUGIN = "clients/claude-panel/.claude-plugin/plugin.json"


def workspace_version(cargo_toml):
    m = re.search(r'^\[workspace\.package\][^\[]*?^version\s*=\s*"([^"]+)"', cargo_toml, re.M | re.S)
    return m.group(1) if m else None


def changelog_section(changelog, version):
    """Body of the '## <version> - <date>' section, or None if the heading is missing or has no real date."""
    lines = changelog.splitlines()
    head = re.compile(r"^## " + re.escape(version) + r" - \d{4}-\d{2}-\d{2}\s*$")
    for i, ln in enumerate(lines):
        if head.match(ln):
            body = []
            for nxt in lines[i + 1:]:
                if nxt.startswith("## "):
                    break
                body.append(nxt)
            return "\n".join(body).strip() + "\n"
    return None


def check(tag, cargo_toml, py_init, plugin_json, changelog):
    """Returns (notes, []) or (None, [reasons])."""
    m = re.fullmatch(r"v(\d+\.\d+\.\d+)", tag)
    if not m:
        return None, [f"tag {tag!r} is not vX.Y.Z"]
    v = m.group(1)
    errs = []
    found = {
        "Cargo.toml [workspace.package]": workspace_version(cargo_toml),
        "python/rcfd/__init__.py": (re.search(r'^__version__\s*=\s*"([^"]+)"', py_init, re.M) or [None, None])[1],
        PLUGIN: json.loads(plugin_json).get("version"),
    }
    errs += [f"{where} has version {got!r}, the tag says {v}" for where, got in found.items() if got != v]
    notes = changelog_section(changelog, v)
    if notes is None:
        errs.append(f"CHANGELOG.md has no heading '## {v} - YYYY-MM-DD'")
    elif not notes.strip():
        errs.append(f"CHANGELOG.md section {v} is empty")
    return (None if errs else notes), errs


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("tag")
    ap.add_argument("--out")
    a = ap.parse_args()

    def read(p):
        return open(p, encoding="utf-8").read()

    notes, errs = check(a.tag, read("Cargo.toml"), read("python/rcfd/__init__.py"), read(PLUGIN), read("CHANGELOG.md"))
    if errs:
        for e in errs:
            print(f"release_notes: {e}", file=sys.stderr)
        return 1
    if a.out:
        open(a.out, "w", encoding="utf-8").write(notes)
    else:
        sys.stdout.write(notes)
    return 0


if __name__ == "__main__":
    sys.exit(main())
