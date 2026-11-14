#!/usr/bin/env python3
"""Write docs/roadmap-progress.svg and docs/roadmap-categories.svg from docs/roadmap.json.

The category of each item is listed in CATEGORIES below (roadmap.json has only phases).
An item that is in no category is drawn under "Other" and reported, so add new ids here.
"""
import json
import pathlib
import sys
from collections import Counter

root = pathlib.Path(__file__).resolve().parent.parent
data = json.loads((root / "docs/roadmap.json").read_text(encoding="utf-8"))
items = data["items"] if isinstance(data, dict) and "items" in data else data
status = {int(i["id"][2:]): i["status"] for i in items}

# (family, name, ids)
CATEGORIES = [
    ("Solver", "Numerics and solvers", [3, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 105, 106, 107,
        135, 201, 202, 204, 205, 208, 209, 210, 211, 246, 248, 256, 285, 291, 299, 301, 309, 321, 322, 344, 366, 367, 442, 443, 454, 467, 471, 474, 530, 531]),
    ("Solver", "Turbulence", [40, 41, 42, 43, 44, 45, 46, 203, 206, 207, 286, 294, 295, 296, 300, 302]),
    ("Solver", "Heat, materials and chemistry", [50, 51, 52, 53, 54, 110, 111, 112, 113, 129, 130, 212, 213, 214,
        215, 216, 217, 220, 222, 223, 224, 225, 289, 298, 311]),
    ("Solver", "Compressible flow and acoustics", [60, 61, 62, 136, 200, 221, 243, 316, 317, 318, 319, 320, 330, 331, 332, 333, 340, 341, 407, 446, 451, 452, 453, 473, 478, 480]),
    ("Solver", "Multiphase and particles", [55, 56, 127, 128, 226, 227, 228, 245, 252, 253, 254, 255, 284, 292, 323, 326]),
    ("Solver", "Moving meshes, rotors and coupling", [63, 114, 126, 134, 138, 139, 218, 219, 229, 230, 237, 242,
        244, 247, 250, 251, 257, 258, 259, 290, 303, 304, 305, 308, 312, 313, 314, 334, 335, 336, 337, 338, 345, 346, 347, 349, 350, 351, 352, 353, 354, 355, 356, 357, 358, 359, 360, 361, 362, 363, 364, 365, 368, 369, 437, 440, 444, 445, 450, 466, 468, 472, 479, 481, 482, 483, 487, 494, 501, 503, 515, 517]),
    ("Solver", "Structural solver", [370, 371, 372, 373, 374, 375, 376, 377, 378, 379, 380, 381, 382, 383, 384, 385, 386, 387, 388, 389, 390, 391, 392, 393, 394, 395, 396, 397, 398, 399, 400, 401, 402, 403, 404, 405, 406, 408, 409, 410, 411, 412, 413, 414, 415, 416, 417, 418, 419, 420, 421, 422, 423, 424, 425, 426, 427, 428, 429, 430, 431, 432, 433, 434, 435, 436, 455, 457, 458, 459, 460, 461, 462, 463, 464, 465, 469, 470, 477, 484, 485, 486, 488, 489, 490, 491, 492, 493, 495, 496, 497, 498, 499, 500, 502, 504, 505, 506, 507, 508, 509, 510, 511, 512, 513, 514, 516, 518, 519, 520, 521, 522, 523, 524, 525, 526, 527, 528, 529, 532]),
    ("Solver", "Parallel and GPU", [16, 100, 101, 236]),
    ("Data", "Meshing and mesh formats", [10, 11, 12, 13, 14, 15, 102, 103, 108, 115, 118, 119, 131, 132, 133,
        231, 232, 233, 234, 235, 306, 324, 325, 327, 342, 343, 348, 438, 439, 441]),
    ("Data", "Post-processing and output", [70, 71, 72, 73, 74, 109, 116, 117, 123, 124, 238, 476]),
    ("Data", "Design studies and optimisation", [95, 137, 249, 260, 268, 297, 310, 315, 329, 339]),
    ("Use", "Command line, Python and API", [5, 80, 81, 82, 83, 84, 85, 104, 120, 121, 122, 239, 240, 241, 307, 328]),
    ("Use", "Claude and the MCP server", [90, 91, 92, 93, 94, 145, 146, 147, 148, 149, 150, 265, 267, 270, 271,
        272, 273, 274, 275, 287, 288]),
    ("Use", "Studio desktop app", [140, 141, 142, 143, 144, 151, 261, 262, 263, 264, 266, 293, 447, 448, 449, 456, 475]),
    ("Project", "Quality, docs and validation", [1, 2, 4, 6, 96, 125, 269]),
]

seen = {}
for _, name, ids in CATEGORIES:
    for n in ids:
        if n in seen:
            sys.exit(f"F-{n:03d} is in both '{seen[n]}' and '{name}'")
        if n not in status:
            sys.exit(f"F-{n:03d} in '{name}' is not in roadmap.json")
        seen[n] = name
other = sorted(set(status) - set(seen))
cats = list(CATEGORIES)
if other:
    print("not in a category:", ", ".join(f"F-{n:03d}" for n in other), file=sys.stderr)
    cats.append(("Project", "Other", other))

STYLE = """
    :root { --ink:#1f2328; --mute:#59636e; --done:#1a7f4b; --part:#d4a72c; --todo:#d0d7de; --bg:#ffffff; }
    @media (prefers-color-scheme: dark) {
      :root { --ink:#f0f3f6; --mute:#9198a1; --done:#3fb97a; --part:#e0b341; --todo:#3d444d; --bg:#0d1117; }
    }
    text { font-family: -apple-system, "Segoe UI", Helvetica, Arial, sans-serif; fill: var(--ink); }
    .big { font-size: 34px; font-weight: 700; }
    .sub { font-size: 15px; fill: var(--mute); }
    .lab { font-size: 14px; font-weight: 600; }
    .row { font-size: 14px; }
    .num { font-size: 13px; fill: var(--mute); }
    .fam { font-size: 12px; font-weight: 700; letter-spacing: 0.08em; fill: var(--mute); }"""
DEFS = """  <defs>
    <pattern id="h" width="8" height="8" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">
      <rect width="8" height="8" fill="var(--part)"/>
      <rect width="3" height="8" fill="var(--bg)" opacity="0.35"/>
    </pattern>
%s
  </defs>"""


def counts(ids):
    c = Counter(status[n] for n in ids)
    return c["done"], c["partial"], c["todo"]


def write(name, text):
    with open(root / "docs" / name, "w", encoding="utf-8", newline="\n") as f:
        f.write(text)


# ---- overall bar
done, partial, todo = counts(status)
total = done + partial + todo
W, X0, BAR_W, BAR_Y, BAR_H = 720, 24, 672, 78, 44
wd, wp = BAR_W * done / total, BAR_W * partial / total
wt = BAR_W - wd - wp
pct = round(100 * done / total)
clip = f'    <clipPath id="r"><rect x="{X0}" y="{BAR_Y}" width="{BAR_W}" height="{BAR_H}" rx="6"/></clipPath>'
write("roadmap-progress.svg", f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} 176" width="{W}" role="img" aria-labelledby="t d">
  <title id="t">Roadmap progress</title>
  <desc id="d">{done} of {total} roadmap items are done, {partial} are partly done and {todo} are not started.</desc>
  <style>{STYLE}
  </style>
  <rect width="{W}" height="176" fill="var(--bg)"/>
{DEFS % clip}
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
""")

# ---- per-category bars: every row is scaled to its own total, so rows compare as shares
TRK_X, TRK_W, ROW_H, BAR = 262, 360, 30, 14
FAMILY = {"Solver": "THE SOLVER", "Data": "MESHES, RESULTS AND DESIGN", "Use": "HOW YOU USE IT",
          "Project": "THE PROJECT"}
y, rows, last = 24, [], None
clips = []
for k, (fam, name, ids) in enumerate(cats):
    d, p, t = counts(ids)
    n = d + p + t
    if fam != last:
        y += 14 if last else 0
        rows.append(f'  <text x="{X0}" y="{y}" class="fam">{FAMILY[fam]}</text>')
        y += 22
        last = fam
    by = y - 11
    wd_, wp_ = TRK_W * d / n, TRK_W * p / n
    wt_ = TRK_W - wd_ - wp_
    clips.append(f'    <clipPath id="c{k}"><rect x="{TRK_X}" y="{by}" width="{TRK_W}" height="{BAR}" rx="3"/></clipPath>')
    rows.append(
        f'  <text x="{X0}" y="{y}" class="row">{name}</text>\n'
        f'  <g clip-path="url(#c{k})">\n'
        f'    <rect x="{TRK_X}" y="{by}" width="{wd_:.1f}" height="{BAR}" fill="var(--done)"/>\n'
        f'    <rect x="{TRK_X + wd_:.1f}" y="{by}" width="{wp_:.1f}" height="{BAR}" fill="url(#h)"/>\n'
        f'    <rect x="{TRK_X + wd_ + wp_:.1f}" y="{by}" width="{wt_:.1f}" height="{BAR}" fill="var(--todo)"/>\n'
        f'  </g>\n'
        f'  <text x="{TRK_X + TRK_W + 14}" y="{y}" class="num">{d} of {n} done</text>'
    )
    y += ROW_H
H = y + 40
W2 = TRK_X + TRK_W + 14 + 110
rows.append(
    f'  <text x="{X0}" y="{y + 14}" class="num">Each bar is scaled to its own item count: green is done, '
    f'hatched is partly done, grey is not started.</text>'
)
desc = "; ".join(f"{n}: {counts(i)[0]} of {sum(counts(i))} done" for _, n, i in cats)
write("roadmap-categories.svg", f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W2} {H}" width="{W2}" role="img" aria-labelledby="t d">
  <title id="t">Roadmap progress by area</title>
  <desc id="d">Done, partly done and not started roadmap items in each of {len(cats)} areas. {desc}.</desc>
  <style>{STYLE}
  </style>
  <rect width="{W2}" height="{H}" fill="var(--bg)"/>
{DEFS % chr(10).join(clips)}
{chr(10).join(rows)}
</svg>
""")
print(f"done {done}, partial {partial}, todo {todo}, total {total}; {len(cats)} categories")
