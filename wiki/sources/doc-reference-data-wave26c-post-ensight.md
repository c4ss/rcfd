---
type: source
tags: [reference-data, wave-section]
sources: []
origin: docs/reference-data.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Reference data: Wave 26C post-object catalogue and EnSight export (F-238, F-103, ADR 0048)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Wave 26C post-object catalogue and EnSight export (F-238, F-103, ADR 0048)" (lines 1037-1062) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Wave 26C post-object catalogue and EnSight export references (ADR 0048).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No literature value is used. Every expected number is a definition or a closed form derived here.

**Verified by derivation (first principles).**

- Sphere of radius `r`: area `4 pi r^2`; outward flux of `F = x - c` is `r * 4 pi r^2 = 4 pi r^3` (divergence theorem, `3 V`); a linear field `z - c_z` on the sphere has mean 0 and variance `r^2 / 3` (equal-area slices, `z - c_z` is uniform on `[-r, r]` by Archimedes' hat-box theorem).
- Ellipsoid: flux of `x - c` is `3 V = 4 pi a b c`.
- Unit cube: surface area 6; outward flux of `(x,y,z)` is `3`; area-weighted mean of `x` over the six faces is `(0 + 1 + 4 * 1/2)/6 = 1/2`; mean of `x^2` is `(0 + 1 + 4/3)/6 = 7/18`, so the variance is `7/18 - 1/4 = 5/36` and the standard deviation `sqrt(5)/6`.
- Plane sections of the unit cube: `x = const` area 1; `x + y = 1` area `sqrt 2`; `x + y + z = 3/2` the regular hexagon of side `sqrt 2 / 2`, area `3 sqrt 3 / 4`.
- Cylinder: lateral area `2 pi r h`.
- Circular cone: lateral area `pi R l` with `R = h tan a`, `l = h / cos a`.
- Weighted variance of `1e9 + {0..6}` is 4 (variance of the integers 0..6 is `(49 - 1)/12`).
- Momentum flux of a uniform velocity `u` through a patch of area vector `S` is `(u . S) u`.

**Unverified / not read.** Layouts of the EnSight Gold files were taken from the vendor's public user-manual page (case file sections, geometry and variable file layouts, `nfaced`, 80-character binary records, wildcard expansion). The page text read did not state the `hexa8` node numbering (figure), the `nfaced` face orientation or the binary integer / float widths. The hexahedron convention (bottom face counter-clockwise seen from the top), outward `nfaced` faces and 4-byte little-endian integers and floats are from memory. HDF5 and CGNS layouts were not read or used.


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
