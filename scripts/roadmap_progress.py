#!/usr/bin/env python3
"""Write docs/roadmap-progress.svg from docs/roadmap.json (counts by status)."""
import json
import pathlib
from collections import Counter

root = pathlib.Path(__file__).resolve().parent.parent
data = json.loads((root / "docs/roadmap.json").read_text(encoding="utf-8"))
items = data["items"] if isinstance(data, dict) and "items" in data else data
c = Counter(i["status"] for i in items)
done, partial, todo = c["done"], c["partial"], c["todo"]
total = done + partial + todo

W, X0, BAR_W, BAR_Y, BAR_H = 720, 24, 672, 78, 44
wd = BAR_W * done / total
wp = BAR_W * partial / total
wt = BAR_W - wd - wp
pct = round(100 * done / total)

svg = f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} 176" width="{W}" role="img" aria-labelledby="t d">
  <title id="t">Roadmap progress</title>
  <desc id="d">{done} of {total} roadmap items are done, {partial} are partly done and {todo} are not started.</desc>
  <style>
    :root {{ --ink:#1f2328; --mute:#59636e; --done:#1a7f4b; --part:#d4a72c; --todo:#d0d7de; --bg:#ffffff; }}
    @media (prefers-color-scheme: dark) {{
      :root {{ --ink:#f0f3f6; --mute:#9198a1; --done:#3fb97a; --part:#e0b341; --todo:#3d444d; --bg:#0d1117; }}
    }}
    text {{ font-family: -apple-system, "Segoe UI", Helvetica, Arial, sans-serif; fill: var(--ink); }}
    .big {{ font-size: 34px; font-weight: 700; }}
    .sub {{ font-size: 15px; fill: var(--mute); }}
    .lab {{ font-size: 14px; font-weight: 600; }}
    .num {{ font-size: 13px; fill: var(--mute); }}
  </style>
  <rect width="{W}" height="176" fill="var(--bg)"/>
  <defs>
    <pattern id="h" width="8" height="8" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">
      <rect width="8" height="8" fill="var(--part)"/>
      <rect width="3" height="8" fill="var(--bg)" opacity="0.35"/>
    </pattern>
    <clipPath id="r"><rect x="{X0}" y="{BAR_Y}" width="{BAR_W}" height="{BAR_H}" rx="6"/></clipPath>
  </defs>
  <text x="{X0}" y="40" class="big">{pct}% done</text>
  <text x="{X0 + 196}" y="40" class="sub">{done} of {total} roadmap items finished, {partial} more under way</text>
  <g clip-path="url(#r)">
    <rect x="{X0}" y="{BAR_Y}" width="{wd:.1f}" height="{BAR_H}" fill="var(--done)"/>
    <rect x="{X0 + wd:.1f}" y="{BAR_Y}" width="{wp:.1f}" height="{BAR_H}" fill="url(#h)"/>
    <rect x="{X0 + wd + wp:.1f}" y="{BAR_Y}" width="{wt:.1f}" height="{BAR_H}" fill="var(--todo)"/>
  </g>
  <text x="{X0}" y="{BAR_Y + BAR_H + 26}" class="lab">Done</text>
  <text x="{X0}" y="{BAR_Y + BAR_H + 44}" class="num">{done}</text>
  <text x="{X0 + wd:.1f}" y="{BAR_Y + BAR_H + 26}" class="lab">Partly done</text>
  <text x="{X0 + wd:.1f}" y="{BAR_Y + BAR_H + 44}" class="num">{partial}</text>
  <text x="{X0 + BAR_W}" y="{BAR_Y + BAR_H + 26}" class="lab" text-anchor="end">Not started</text>
  <text x="{X0 + BAR_W}" y="{BAR_Y + BAR_H + 44}" class="num" text-anchor="end">{todo}</text>
</svg>
"""
with open(root / "docs/roadmap-progress.svg", "w", encoding="utf-8", newline="\n") as f:
    f.write(svg)
print(f"done {done}, partial {partial}, todo {todo}, total {total}")
