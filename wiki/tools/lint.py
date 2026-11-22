#!/usr/bin/env python3
"""Mechanical lint for the rcfd wiki. Run from anywhere: python wiki/tools/lint.py

Checks: broken [[wikilinks]], orphan pages (no inbound links), pages missing from index.md,
missing/invalid frontmatter, stale `updated` dates (> STALE_DAYS). Judgement checks
(contradictions, superseded claims, missing concepts) are the LLM's job; see CLAUDE.md > Lint.
Exit code 1 if any broken link or index gap is found.
"""
import datetime as dt
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PAGE_DIRS = ["sources", "entities", "concepts", "analyses"]
TOP_PAGES = ["overview.md", "ingest-plan.md"]
SPECIAL = {"index", "log", "overview", "ingest-plan"}
TYPES = {"source", "entity", "concept", "analysis", "overview", "plan"}
REQUIRED = ["type", "created", "updated", "status"]
STALE_DAYS = 180
MATHY = re.compile(r"`([^`\n]{3,})`")
MATH_HINT = re.compile(r"(\^|_[a-zA-Z0-9]|\b(sqrt|max|min|exp|log|sin|cos|nabla|partial|rho|nu|mu|omega|tau|epsilon|beta|alpha|sigma|kappa|phi|delta|Delta)\b|[a-zA-Z]\s*[=<>]\s*[-a-zA-Z0-9(])")
CODE_HINT = re.compile(r"(\.toml|\.rs|\.md|\.json|/|\\|::|\"|\[[a-z_]+\]|^[a-z_]+\.[a-z_]+|--)")
LINK = re.compile(r"\[\[([^\]|#^]+)(?:[#^][^\]|]*)?(?:\|[^\]]*)?\]\]")


def pages():
    out = {}
    for d in PAGE_DIRS:
        for p in (ROOT / d).rglob("*.md"):
            out[p.stem] = p
    for name in TOP_PAGES + ["index.md", "log.md"]:
        p = ROOT / name
        if p.exists():
            out[p.stem] = p
    return out


def frontmatter(text):
    if not text.startswith("---"):
        return None
    end = text.find("\n---", 3)
    if end < 0:
        return None
    fm = {}
    for line in text[3:end].splitlines():
        if ":" in line and not line.startswith(" "):
            k, v = line.split(":", 1)
            fm[k.strip()] = v.strip()
    return fm


def strip_code(text):
    text = re.sub(r"```.*?```", "", text, flags=re.S)
    return re.sub(r"`[^`\n]*`", "", text)


def main():
    all_pages = pages()
    names = set(all_pages)
    inbound = {n: set() for n in names}
    broken, fm_issues, stale, mathy = [], [], [], []
    today = dt.date.today()

    for name, path in all_pages.items():
        text = path.read_text(encoding="utf-8")
        if name not in {"index", "log"}:
            for span in MATHY.findall(re.sub(r"```.*?```", "", text, flags=re.S)):
                if re.fullmatch(r"[A-Za-z0-9_.\-]+", span):
                    continue  # a bare identifier (case key, crate, file stem), not math
                if MATH_HINT.search(span) and not CODE_HINT.search(span):
                    mathy.append((path.relative_to(ROOT), span[:60]))
        for target in LINK.findall(strip_code(text)):
            t = Path(target.strip()).stem
            if t in names:
                if t != name:
                    inbound[t].add(name)
            else:
                broken.append((path.relative_to(ROOT), target))
        if name in {"index", "log"}:
            continue
        fm = frontmatter(text)
        if fm is None:
            fm_issues.append((path.relative_to(ROOT), "no frontmatter"))
            continue
        missing = [k for k in REQUIRED if not fm.get(k)]
        if missing:
            fm_issues.append((path.relative_to(ROOT), "missing " + ", ".join(missing)))
        if fm.get("type") and fm["type"] not in TYPES:
            fm_issues.append((path.relative_to(ROOT), "bad type " + fm["type"]))
        try:
            upd = dt.date.fromisoformat(fm.get("updated", ""))
            if (today - upd).days > STALE_DAYS:
                stale.append((path.relative_to(ROOT), fm["updated"]))
        except ValueError:
            pass

    index_text = (ROOT / "index.md").read_text(encoding="utf-8") if "index" in names else ""
    indexed = {Path(t.strip()).stem for t in LINK.findall(strip_code(index_text))}
    not_indexed = sorted(n for n in names - SPECIAL if n not in indexed)
    # index/log links don't count as real inbound links for orphan detection
    orphans = sorted(n for n in names - SPECIAL if not (inbound[n] - {"index", "log"}))

    def section(title, rows):
        print(f"\n## {title} ({len(rows)})")
        for r in rows:
            print("- " + (" -> ".join(str(x) for x in r) if isinstance(r, tuple) else str(r)))

    print(f"# wiki lint {today}  ({len(names)} pages)")
    section("Broken links", broken)
    section("Not in index.md", not_indexed)
    section("Orphans (no inbound links besides index/log)", orphans)
    section("Frontmatter", fm_issues)
    section(f"Stale (updated > {STALE_DAYS} days ago)", stale)
    files = sorted({str(p) for p, _ in mathy})
    print()
    print(f"## Possible math in backticks (warning only): {len(mathy)} spans in {len(files)} pages")
    for f in files[:15]:
        print("- " + f)
    return 1 if broken or not_indexed else 0


if __name__ == "__main__":
    sys.exit(main())
