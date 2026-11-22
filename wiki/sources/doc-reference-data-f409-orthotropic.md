---
type: source
tags: [reference-data, structural]
sources: []
origin: docs/reference-data.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Reference data: F-409 orthotropic elasticity, material axes, orthotropic expansion, temperature tables (ADR 0452)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-409 orthotropic elasticity, material axes, orthotropic expansion, temperature tables (ADR 0452)" (lines 1658-1670) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-409 orthotropic elasticity references (ADR 0452).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


| Item | Status | Source |
|---|---|---|
| Orthotropic compliance from nine engineering constants, `S_12 = -nu_12/E_1` etc., reciprocity `nu_ji/E_j = nu_ij/E_i`; the stiffness its inverse, isotropic constants give `lambda + 2 mu`, `lambda`, `mu` | derived | first principles (symmetry of the strain energy), ADR 0452 F1, F3; Jones, Mechanics of Composite Materials: NOT read |
| Positive definiteness: `nu_ij^2 < E_i/E_j` and `1 - nu12 nu21 - nu13 nu31 - nu23 nu32 - 2 nu21 nu32 nu13 > 0` | derived | Sylvester's criterion on the normal block, ADR 0452 F2; the restrictions of Lempriere as quoted in Jones: NOT read |
| Rotation of the engineering-strain Voigt vector `eps' = T eps`, `D = T^T C T` by energy conjugacy | derived | tensor transformation, ADR 0452 F4 |
| Off-axis lamina `1/E_x = c^4/E1 + s^4/E2 + (1/G12 - 2 nu12/E1) s^2 c^2`, `S16 = (2 S11 - 2 S12 - S66) s c^3 - (2 S22 - 2 S12 - S66) s^3 c` (axes turned by `+theta`, rows `(c, s, 0)`, `(-s, c, 0)`) | derived | ADR 0452 F5, checked against a numerical rotation (`f409-scratch/py/verify.py`); Jones' `S-bar` from memory, sign convention of this ADR |
| Polar-orthotropic ring under internal pressure: `sigma_r = A r^(k-1) + B r^(-k-1)`, `k = sqrt(E_theta/E_r)`, bore hoop `k p (a^(2k) + b^(2k))/(b^(2k) - a^(2k))` (`34 p/15` for `b/a = 2`, `k = 2`) | derived | equilibrium and compatibility, ADR 0452 F7; Lekhnitskii, Theory of Elasticity of an Anisotropic Body: NOT read |
| Bar with a linear modulus: `delta = F L ln(E1/E0)/(A (E1 - E0))`; midpoint-rule error of 20 Hex8 `-1.126e-4` | derived | one integral, ADR 0452 F8 |
| Thermal strain of a tabulated coefficient `int_Tref^T alpha dT`, for `alpha = a0 + a1 T` `a0 (T - Tref) + a1 (T^2 - Tref^2)/2` | derived | ADR 0312 goal (instantaneous coefficient), ADR 0452 F9 |
| Timoshenko cantilever `P L^3/(3 E1 I) + P L/(kappa G13 A)`, `kappa = 5/6` | from memory | Timoshenko, Strength of Materials: NOT read; the support of beam theory of ADR 0385 D1 |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
