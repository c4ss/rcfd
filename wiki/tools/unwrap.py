#!/usr/bin/env python3
"""Unwrap hard-wrapped prose in wiki pages: one line per paragraph / list item / callout line.

Obsidian renders a single newline as a line break, so prose wrapped at ~110 columns shows broken
mid-sentence. This joins continuation lines and leaves structure alone: frontmatter, headings, code
fences, tables, HTML comments, blank lines, list markers, callout markers, `**Label:**` lines and
explicit hard breaks (two trailing spaces or a trailing backslash). Idempotent.

Usage (from wiki/): python tools/unwrap.py [--check] [files...]   (default: every page in the vault)
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LIST = re.compile(r"^\s*([-*+]|\d+[.)])\s+")
LABEL = re.compile(r"^\*\*[^*]{1,40}:\*\*")
BLOCK_START = re.compile(r"^\s*(#{1,6}\s|\||<!--|-->|```|~~~|---\s*$|\[\^|<)")


def is_structural(line):
    s = line.strip()
    return (not s) or BLOCK_START.match(line) or s.startswith("$$")


def hard_break(line):
    return line.endswith("  ") or line.endswith("\\")


def unwrap(text):
    lines = text.split("\n")
    out = []
    i = 0
    # frontmatter
    if lines and lines[0].strip() == "---":
        j = 1
        while j < len(lines) and lines[j].strip() != "---":
            j += 1
        out.extend(lines[: j + 1])
        i = j + 1
    fence = False
    in_comment = False
    for line in lines[i:]:
        s = line.strip()
        if s.startswith("```") or s.startswith("~~~"):
            fence = not fence
            out.append(line)
            continue
        if fence:
            out.append(line)
            continue
        if in_comment:
            out.append(line)
            if "-->" in line:
                in_comment = False
            continue
        if s.startswith("<!--") and "-->" not in s:
            in_comment = True
            out.append(line)
            continue
        prev = out[-1] if out else ""
        joinable_prev = prev.strip() and not is_structural(prev) and not hard_break(prev)
        if s.startswith(">"):
            body = s.lstrip(">").strip()
            pbody = prev.strip().lstrip(">").strip() if prev.strip().startswith(">") else None
            if (pbody and joinable_prev and body and not body.startswith("[!")
                    and not LIST.match(body) and not LABEL.match(body) and not pbody.startswith("[!")):
                out[-1] = prev.rstrip() + " " + body
                continue
            out.append(line)
            continue
        if (joinable_prev and not is_structural(line) and not LIST.match(line) and not LABEL.match(s)
                and not prev.strip().startswith(">")):
            out[-1] = prev.rstrip() + " " + s
            continue
        out.append(line)
    return "\n".join(out)


def pages(args):
    if args:
        return [Path(a) for a in args]
    skip = {"templates", "tools", ".handoff", ".obsidian", "raw"}
    return [p for p in ROOT.rglob("*.md") if not (set(p.relative_to(ROOT).parts) & skip)]


def main():
    args = [a for a in sys.argv[1:] if a != "--check"]
    check = "--check" in sys.argv
    changed = []
    for p in pages(args):
        old = p.read_text(encoding="utf-8")
        new = unwrap(old)
        if new != old:
            changed.append(p)
            if not check:
                p.write_text(new, encoding="utf-8")
    print(("would change " if check else "unwrapped ") + f"{len(changed)} file(s)")
    for p in changed[:20]:
        print(" ", p.resolve().relative_to(ROOT))
    return 1 if (check and changed) else 0


if __name__ == "__main__":
    sys.exit(main())
