#!/usr/bin/env python3
"""Rebuild wiki/index.md from every page in the vault.

Summary per page: the one-line summary an ingest agent wrote in a handoff (`.handoff/*.md`, section
"## Index lines") if there is one, else the summary already in the current index.md, else the page's
first sentence. Pages are grouped by type, and source pages are sub-grouped (repo docs, ADR threads,
other). Run from wiki/: python tools/build_index.py
"""
import datetime as dt
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TODAY = dt.date.today().isoformat()
LINE = re.compile(r"^- \[\[([^\]|]+)\]\]\s+[—-]+\s+(.*)$")


def known_summaries():
    out = {}
    idx = ROOT / "index.md"
    if idx.exists():
        for l in idx.read_text(encoding="utf-8").splitlines():
            m = LINE.match(l)
            if m:
                out[m.group(1)] = strip_tail(m.group(2))
    for h in sorted((ROOT / ".handoff").glob("*.md")):
        t = h.read_text(encoding="utf-8")
        if "## Index lines" not in t:
            continue
        sec = t.split("## Index lines", 1)[1].split("\n## ", 1)[0]
        for l in sec.splitlines():
            m = LINE.match(l.strip())
            if m:
                out[m.group(1)] = strip_tail(m.group(2))
    return out


def strip_tail(s):
    return re.sub(r"\s*\((\d+ sources?|origin [^)]*|updated [^)]*)[^)]*\)\s*$", "", s).strip()


def fm(text):
    if not text.startswith("---"):
        return {}
    end = text.find("\n---", 3)
    d = {}
    for line in text[3:end].splitlines():
        if ":" in line and not line.startswith(" "):
            k, v = line.split(":", 1)
            d[k.strip()] = v.strip()
    return d


def first_sentence(text):
    body = text.split("\n---", 1)[1] if text.startswith("---") else text
    if "## Summary" in body:
        body = body.split("## Summary", 1)[1]
    for l in body.splitlines():
        s = l.strip()
        if s and not s.startswith(("#", "|", ">", "<!--", "**Origin", "**Kind", "**Where")):
            s = re.sub(r"\*\*What it is:\*\*\s*", "", s)
            s = re.sub(r"\s*\(\[\[[^)]*\]\][^)]*\)", "", s)
            s = re.sub(r"\[\[([^\]|]+)(\|[^\]]*)?\]\]", r"\1", s).lstrip("- ").strip()
            m = re.match(r"(.{30,}?\.)(\s|$)", s)
            s = m.group(1) if m else s
            return (s[:157].rsplit(" ", 1)[0] + " ...") if len(s) > 160 else s
    return ""


def nsrc(d):
    v = d.get("sources", "[]").strip("[]")
    return len([x for x in v.split(",") if x.strip()])


def main():
    summ = known_summaries()
    groups = {k: [] for k in ["overview", "plan", "src-doc", "src-adr", "src-other", "entity", "concept", "analysis"]}
    for folder in ["sources", "entities", "concepts", "analyses"]:
        for p in sorted((ROOT / folder).glob("*.md")):
            t = p.read_text(encoding="utf-8")
            d = fm(t)
            s = summ.get(p.stem) or first_sentence(t)
            upd = d.get("updated", TODAY)
            if folder == "sources":
                key = "src-adr" if p.stem.startswith("adr-thread") else "src-doc" if p.stem.startswith("doc-") else "src-other"
                tail = f"origin `{d.get('origin', '?')}`, updated {upd}"
            else:
                key = {"entities": "entity", "concepts": "concept", "analyses": "analysis"}[folder]
                tail = f"{nsrc(d)} sources, updated {upd}"
            groups[key].append(f"- [[{p.stem}]] — {s} ({tail})")
    counts = {k: len(v) for k, v in groups.items()}
    total = sum(counts.values()) + 2
    out = ["---", "type: index", f"updated: {TODAY}", "---", "", "# Index", "",
           f"Catalog of every wiki page ({total} pages). Read this first on any query. Format: `- [[page-name]] — one-line summary (n sources, updated YYYY-MM-DD)`. Rebuilt by `tools/build_index.py`.", "",
           "## Overview", "", f"- [[overview]] — {summ.get('overview', 'The top-level synthesis of the wiki.')}", "",
           "## Plans", "", f"- [[ingest-plan]] — {summ.get('ingest-plan', 'Phased plan to ingest the existing repo docs.')}", ""]
    sections = [("Sources: repo docs", "src-doc"), ("Sources: ADR threads", "src-adr"), ("Sources: other", "src-other"),
                ("Entities", "entity"), ("Concepts", "concept"), ("Analyses", "analysis")]
    for title, key in sections:
        out += [f"## {title} ({counts[key]})", ""] + (groups[key] or ["_none yet_"]) + [""]
    (ROOT / "index.md").write_text("\n".join(out), encoding="utf-8")
    print(f"index: {total} pages", counts)


if __name__ == "__main__":
    main()
