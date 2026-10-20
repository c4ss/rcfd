# ADR 0080: Wave 36A, the periodic-pair mean of `rAU` for the body-force face flux in the transient (PISO/PIMPLE) kernel

Status: accepted (registration). Everything under "Registered" was written BEFORE any solver code of this wave was changed and BEFORE any run with the
new key. Limits are not changed, widened, replaced or deleted after a run. A miss is reported as measured and its assertion moves to an ignored
`*_known_miss` test with the limit intact. The fix is opt-in; the default stays bit-identical (`w33b_default_bits` fingerprints) and
`crates/rcfd-api/tests/baseline_hashes.rs` is never re-recorded. Clean-room: first principles only.

Parent: ADR 0076 (wave 35A; Y5 and its open item), ADR 0071 (steady `coupled_body_pair_mean`).

## What was known before this registration (and how)

* ADR 0076 numbers (quoted, not re-measured). Y5 measured the seam body-force term with the coupled mean `rAU` **on top of K = F-G + F-S + F-R**:
  R2 (SECT-M PISO) 7.7563e-4 / 1.8884e-4, order 2.038; R2b (SECT-M PIMPLE-4) 2.4919e-5 / 6.1413e-6, order 2.021. K alone: R2 1.717, R2b 0.593,
  R3 SECT-M 1.773 / 1.773, R4 SECT-M velocity ratio 1.0651 at dt 0.05; C-O2 1.332; N1 TG-S pressure ratios 1.0952 / 1.0521.
* **K is not in the code.** Commit 5b36209 removed F-G, F-S and F-R by the registered rule of ADR 0076. HEAD c89495f is the default kernel. The term of
  this ADR therefore lands on the default kernel, not on K, and Y5 does not predict its effect there.
* Code reading (no run). In `PimpleSolver::step_with` the body-force face term is `phi_hby_a[f] += gamma_p[f] * bf.face[f]` with
  `gamma_p = face_interpolate(rAU)`, i.e. the OWNER's `rAU` on every boundary face, including periodic ones; the pressure term of the same faces uses
  `coupled_gamma` (distance-weighted pair mean) inside `solve_pressure` / `corrected_flux`; `boundary_hby_a` interpolates owner and rotated partner with the
  same weights. So every part of a seam pair's flux is exactly antisymmetric (`phi_f = -phi_g`) except the body-force part, which leaves
  `(rAU_o - rAU_q) f.S` per pair: a mass source at the seam. The steady kernel removes this with the opt-in `coupled_body_pair_mean` (simple.rs, ADR 0071).
* Which wave-35A misses the term can touch at all (code reading): TG, TG-S and MODE have no body force (`step`, no hook), so the term is never executed
  there; the box MMS has a body force but no periodic faces, so the pair mean returns `gamma_p` unchanged. **Prediction: C-O2 and all N1 rows are
  bit-identical with the key on; the term cannot close C-O2 (1.332 was K's value; on HEAD C-O2 is the default 1.223 of ADR 0073) or the N1 TG-S pressure
  rows (on HEAD there is no K; with the key on, every N1 ratio is exactly 1.0000 by bit identity, which is not a closure).** Causes, from ADR 0076: C-O2 is the PIMPLE-C relaxation floor (F-R removed it
  partly; the face-consistent relaxation hypothesis of ADR 0076 is untested); the N1 TG-S pressure rows were K's seam terms against a default whose seam
  errors partly cancel the spatial pressure error.
* Prediction for SECT-M with the term on the default kernel (written before any run with it):
  - R2 (PISO) stays near the default 1.041: its floor is the first-outer seam convection lag, which SWIRL (no body force) shows on its own (default 1.135)
    and which F-S removed. The term does not touch it. Predicted MISS.
  - R2b (PIMPLE-4): the default kernel keeps the face-consistency floor that F-G removed (TG has no seam body force and its default PIMPLE-4 order is 1.031,
    ADR 0073 C-O3), and ADR 0076 measured that the F-G seam terms are not zero on SECT-M. Predicted below 1.9 (MISS), value uncertain.
  - R3 (SECT-M PISO joint refinement vs exact): default 1.598 / 0.838; predicted MISS for the same reason as R2.
* **Smoke (default code, this session, scratch tree; rule d).** SECT-M, 4 steps, set-ups of ADR 0076 (`smoke_w36a_pair_sums`):

  | n | path | dt | flux pair sum / max seam flux | body-force face pair sum / max |
  |---|---|---|---|---|
  | 16 | PISO | 0.05 / 0.0125 | 1.028e-4 / 1.618e-5 | 1.559e-9 / 1.521e-9 |
  | 16 | PIMPLE-4 | 0.05 / 0.0125 | 1.560e-4 / 1.643e-5 | same |
  | 32 | PISO | 0.05 / 0.0125 | 2.560e-5 / 2.978e-6 | 1.969e-9 / 1.918e-9 |
  | 32 | PIMPLE-4 | 0.05 / 0.0125 | 1.378e-5 / 3.288e-6 | same |
  | 64 | PISO | 0.05 / 0.0125 | 8.090e-6 / 6.764e-7 | 2.190e-9 / 2.131e-9 |
  | 64 | PIMPLE-4 | 0.05 / 0.0125 | 1.957e-6 / 2.351e-7 | same |

  The body-force face values themselves are antisymmetric to about 2e-9 relative (the fourth-order finite differences of the SECT-M source; absolute
  pair sums <= 2.9e-11), and every run is finite. With the pair mean the flux pair sum is `rAU_mean * (bf_f + bf_g)`, bounded by
  `0.05 * 2.9e-11 / 7.1e-4 = 2.0e-9` relative on these runs (rAU <= dt <= 0.05, smallest max seam flux 7.16e-4), plus round-off; the limit below is 1e-8.
* Literature read in this session: none.

## The fix (specified at code level before it is written)

**F-BP `PimpleSettings::coupled_body_pair_mean` (default false; iterative path, the PISO/PIMPLE loop of `step_with`).** After
`gamma_p = face_interpolate(rAU)`, when the key is on and a body force is present, the body-force term of the face flux uses
`coupled_gamma(geom, pbc, gamma_p)` (made `pub(crate)` in `pressure.rs`): on every periodic face (translational and rotational) the distance-weighted mean
`w rAU_o + (1 - w) rAU_q`, `w = dn_q / (dn_o + dn_q)`, the same coefficient the pressure term of that face already uses and the same formula as the steady
`coupled_body_pair_mean`. Nothing else changes: the ddt correction, ALE wall flux, face jumps, `corrected_flux`, the cell velocity reconstruction
`rAU_c f_c`, and the pressure-lag check (which runs without body force) are untouched. Not in scope: the fractional path (it adds no body-force face flux
of this form), varden, API/schema wiring (exposing the key is the user's decision; the steady key is switched on by the API only with rotating-frame
sources, and the transient kernel has no rotating-frame sources, ADR 0045).

## Registered

Set-ups, tolerances and error measures are those of ADR 0076 Part R (`w35a_seam.rs`): 60-degree sector, central convection, BDF2, nu 0.02, t_end 1,
tolerances 1e-14 / 1e-12, fine pair 0.025 / 0.0125 on 16 x 16 against the same-scheme dt = 1/1280 reference, pressure error with volume means removed.
**B** = the default kernel with `coupled_body_pair_mean = true`; "default" = every option off.

### Part B: the key on HEAD (binding rows marked)

* **P1 (binding, analytic)** pair conservation: on the 12 smoke configurations above (n 16, 32, 64; PISO and PIMPLE-4; dt 0.05 and 0.0125; 4 steps) the
  flux pair sum divided by the largest seam flux is <= 1e-8 with B (the default value is printed alongside).
* **B-R4 (binding, exact solution, rule b of wave 34)** no-harm: SECT-M 16 x 16, PISO and PIMPLE-4, dt in {0.05, 0.025, 0.0125}: exact velocity AND pressure
  error with B <= 1.05 x the default (6 velocity + 6 pressure comparisons).
* **N2 (binding)** with every option off the five `w33b_default_bits` fingerprints are identical (gate); `baseline_hashes` unchanged (gate).
* **N2b (binding)** the key is inert where the prediction says so: bit-identical cell velocity, pressure and face flux, key on vs off, for (i) SWIRL 16 x 16 (rotational
  seams, no body force) PISO and PIMPLE-4, 8 steps of dt 0.05; (ii) the ADR 0069 MMS box (body force, no periodic faces) N = 16, PISO, 8 steps of dt 0.05;
  (iii) the ADR 0069 TG periodic box (no body force) N = 16, PIMPLE-C, 8 steps of dt 0.05.
* **B-R2** SECT-M PISO fine pair in [1.9, 2.3] (default 1.041); **B-R2b** SECT-M PIMPLE-4 fine pair in [1.9, 2.3] (default 0.538); **B-R3** SECT-M PISO joint
  refinement vs exact, (16, 0.05), (32, 0.025), (64, 0.0125), both orders >= 1.8 (limit of ADR 0076 R3). Registered rows with their ADR 0076 limits; a miss is
  reported as MISS and gets a `*_known_miss` test. **Registered in advance (rule b exception): a miss of B-R2, B-R2b or B-R3 does not remove the key**, because
  the predicted causes (seam convection lag, face-consistency floor) are defects outside F-BP that the default kernel keeps; F-BP's own job is P1.

### Part K: information with K (scratch tree at 79e838d plus the same term; K stays removed, no verdict on K)

* **IC (implementation check, binding for keeping B)**: R2 and R2b with K + term reproduce the Y5 errors 7.7563e-4 / 1.8884e-4 and 2.4919e-5 / 6.1413e-6 to three
  significant digits. A failure means this code is not what Y5 measured: B is then removed.
* Reported with K + term (no limit, no verdict): R3 SECT-M (PISO joint refinement vs exact), R4 SECT-M (PISO, exact velocity and pressure error vs default).
  C-O2 and the N1 TG/TG-S/MODE/MMS rows are not rerun with K + term: the term is bit-inert there (N2b), so they equal ADR 0076's K values.

### Decision rule (fixed now)

* **B is kept** (opt-in, default false, library-level, no schema key) if P1, B-R4, N2, N2b and IC all hold. Otherwise F-BP is removed with its tests and
  the record commit is named.
* One run per registered row. No other set-up, no further fix in this wave.

### Out of scope

Re-introducing F-G, F-S or F-R (removed in ADR 0076; their misses C-O2 and N1 TG-S pressure do not involve a body force); the fractional path; API wiring.

## Sources

No literature. The pair-conservation bound is analytic (antisymmetry of each flux part), the order limits and the 1.05 ratio are those of ADR 0076.
