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

# Reference data: F-627 monosymmetric thin-walled sections for the warping element (ADR 0553)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-627 monosymmetric thin-walled sections for the warping element (ADR 0553)" (lines 2214-2232) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-627 monosymmetric thin-walled sections (ADR 0553).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read for F-627. The books below are cited from memory for the names and the published forms (editions and chapters as recalled, not checked). Every formula used is derived in ADR 0553 (D1 to D6, arithmetic 1 to 6) and checked by a plain numpy script (`f627-scratch/arith.py`, `arith.log`), no rcfd code.

| Item | Status | Source |
|---|---|---|
| Shear-centre kinematics `v_c = v_s + z_0 phi`, `w_c = w_s - y_0 phi`; normal-strain energy uncoupled in the shear-centre displacements; mass exact in the centroid ones | derived, cited | ADR 0553 D1; Vlasov, Timoshenko and Gere, op. cit. (F-598): NOT read, from memory |
| Wagner coefficient about the shear centre (monosymmetry coefficient) `beta_x = int z (y^2 + z^2) dA/I_y - 2 z_0` | derived, cited | ADR 0553 D2 (the work of `sigma` on `(1/2)(v_P'^2 + w_P'^2)`); Trahair, op. cit., ch. 6: NOT read, from memory |
| LTB of a monosymmetric beam under uniform moment, forks, warping free: `M = (beta_x P_y/2) +/- sqrt((beta_x P_y/2)^2 + P_y (G J + pi^2 E C_w/L^2))`, `P_y = pi^2 E I_z/L^2` | derived, cited | ADR 0553 D3; Trahair, op. cit., ch. 6; Timoshenko and Gere, op. cit., ch. 6: NOT read, from memory |
| Thin-walled monosymmetric I: shear centre `h_c I_1/(I_1 + I_2)` from flange 2, `C_w = I_1 I_2 h_c^2/(I_1 + I_2)` | derived, cited | ADR 0553 D5 (the principal sectorial coordinate of the flanges); Timoshenko and Gere, Trahair, op. cit.: NOT read, from memory |
| Approximation `beta_x ~ 0.9 h_c (2 rho_f - 1)(1 - I_z/I_y)^2` (Kitipornchai and Trahair) | cited, not used | ADR 0553 D5: from memory; `0.115` against the derived `0.153` for the section of the checks |
| Flexural-torsional column `(P_y - P)(G J + E C_w k^2 - P r_0^2) - P^2 z_0^2 = 0`, `r_0^2 = I_p/A + z_0^2` | derived, cited | ADR 0553 arithmetic 3; Timoshenko and Gere, op. cit., ch. 5: NOT read, from memory |
| Coupled lateral-torsional vibration `det(diag(E I_z k^4, G J k^2 + E C_w k^4) - omega^2 rho [A, A z_0; A z_0, I_p + A z_0^2]) = 0` | derived | ADR 0553 arithmetic 4 |
| Tip bimoment on a cantilever with the root warping held: `phi(L) = (B/(G J))(1 - 1/cosh kL)`, `chi(L) = B tanh(kL)/(E C_w k)` | derived | ADR 0553 arithmetic 6 |
| Bimoment pre-stress coefficient `beta_omega = int omega_S (y^2 + z^2) dA/C_w`, zero for every section with an axis of symmetry | derived, cited | ADR 0553 D6; Trahair, op. cit.: NOT read, from memory |
| Measured: monosymmetric I, `L = 3`, 16 elements: larger flange in compression `1.29e-3` of the closed form, in tension `3.9e-4`; column `3.5e-3`; vibration `1.7e-3`, `3.0e-3`; lateral-force twist `2.0e-6`; bimoment `4.5e-7` | computed | ADR 0553 addendum 1 |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
