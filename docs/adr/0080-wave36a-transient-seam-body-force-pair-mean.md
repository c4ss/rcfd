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
* **K is not in the code.** Commit 163c9c2 removed F-G, F-S and F-R by the registered rule of ADR 0076. HEAD 6c3ddc9 is the default kernel. The term of
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

### Part K: information with K (scratch tree at fbcc05c plus the same term; K stays removed, no verdict on K)

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

## Final addendum (measured values, verdicts, deviations; nothing above was changed)

Order of work: smoke (default code), registration (commit 2945ca0), F-BP written (scratch tree of HEAD), P1 + N2b + `w33b_default_bits`, the B rows (B-R2,
B-R2b, B-R3, B-R4; one process, each row once), then the scratch K tree (fbcc05c plus the same term, never committed): IC (the ADR 0076 R1/R2 test, once) and
the R3/R4 information rows (once). Code and tests: commit 1d87359.

### Part B (the key on HEAD)

| row | measured with B | default | limit | verdict |
|---|---|---|---|---|
| P1 seam pair conservation (12 configurations) | worst relative pair sum 6.68e-11 (n 16 PISO dt 0.05); 7.4e-12 to 6.7e-11 | 2.35e-7 to 1.56e-4 | <= 1e-8 | met |
| B-R4 SECT-M PISO, dt 0.05 / 0.025 / 0.0125 | velocity ratio 1.0009 / 0.9998 / 0.9997; pressure 1.0005 / 1.0002 / 1.0001 | - | <= 1.05 | met |
| B-R4 SECT-M PIMPLE-4, same dt | velocity 0.9984 / 0.9990 / 0.9995; pressure 1.0001 / 1.0000 / 1.0000 | - | <= 1.05 | met |
| N2 `w33b_default_bits` | 5 of 5 identical (scratch run; gate below) | - | identical | met |
| N2b SWIRL PISO / PIMPLE-4; MMS box PISO; TG PIMPLE-C | bit-identical u, p, phi key on vs off | - | identical | met |
| B-R2 SECT-M PISO fine | 2.0310e-3 / 9.8906e-4, **1.038** | 1.9762e-3 / 9.6010e-4, 1.041 | [1.9, 2.3] | MISS (predicted) |
| B-R2b SECT-M PIMPLE-4 fine | 6.5763e-4 / 4.5505e-4, **0.531** | 6.5924e-4 / 4.5413e-4, 0.538 | [1.9, 2.3] | MISS (predicted) |
| B-R3 SECT-M PISO joint refinement vs exact | 1.5438e-2, 5.0965e-3, 2.8510e-3: **1.599, 0.838** | 1.598, 0.838 | >= 1.8 | MISS (predicted) |

On the default kernel the term changes the SECT-M velocity error against the exact solution by at most 0.16 %, and against the dt = 1/1280 reference by
+2.8 % / +3.0 % (PISO) and -0.2 % / +0.2 % (PIMPLE-4) on the fine pair: the seam
body-force mismatch is a small part of the default error, which is dominated by the seam convection lag (PISO) and the face-consistency floor (PIMPLE-4).

### Part K (scratch, K + term; information, K stays removed)

| row | measured | ADR 0076 value | |
|---|---|---|---|
| IC R2 | 7.7563e-4 / 1.8884e-4, 2.038 | Y5 7.7563e-4 / 1.8884e-4, 2.038 | met (all printed digits) |
| IC R2b | 2.4919e-5 / 6.1413e-6, 2.021 | Y5 2.4919e-5 / 6.1413e-6, 2.021 | met (all printed digits) |
| R1 / R1b SWIRL (no body force) | 2.161 / 2.021 | K 2.161 / 2.021 | identical, as predicted |
| R3 SECT-M | 1.6413e-2, 4.8054e-3, 1.4064e-3: 1.772, 1.773 | K 1.773, 1.773 | still below 1.8 |
| R4 SECT-M velocity, dt 0.05 / 0.025 / 0.0125 | 1.0641 / 1.0250 / 1.0108 (pressure 1.0037 / 1.0154 / 1.0082) | K 1.0651 / 1.0266 / 1.0121 | dt 0.05 still above 1.05 |

### Which wave-35A misses close

* **R2 (1.717 with K) and R2b (0.593 with K): close only together with K** (2.038, 2.021; Y5 reproduced exactly). With the term alone on HEAD: 1.038 and 0.531,
  i.e. not closed. Causes: R2, the first-outer seam convection lag (F-S, removed); R2b, the face-consistency floor of the default kernel (F-G, removed).
* **C-O2 (1.332 with K): does not close.** TG has no body force; the term is bit-inert there (N2b). On HEAD C-O2 is the default 1.223 (ADR 0073). Cause: the
  PIMPLE-C relaxation floor (H-RELAX, ADR 0076; F-R removed; the face-consistent relaxation hypothesis is untested).
* **N1 TG-S pressure (1.0952 / 1.0521 with K): does not close.** TG-S has no body force (bit-inert). The miss was K's seam terms removing a cancellation of
  seam and spatial pressure errors that the default enjoys (ADR 0076 observation); a body-force coefficient cannot change it.
* **R3 SECT-M and R4 SECT-M (K misses, not in the task list but measured): do not close even with K + term** (1.772 / 1.773; 1.0641). Both are dominated by the
  spatial error (16 x 16 exact error 1.52e-2) and by the loss of the default's partial cancellation of temporal and spatial error at dt 0.05, not by the seam
  body force.

### Verdict and what was done

* **F-BP is kept** (opt-in `PimpleSettings::coupled_body_pair_mean`, default false, library-level, no schema key): P1, B-R4, N2, N2b and IC hold. B-R2, B-R2b and
  B-R3 missed as predicted and are ignored `*_known_miss` tests with the registered limits; by the rule registered in advance they do not remove the key.
* What the key does on HEAD: it makes the two faces of every periodic pair carry opposite fluxes when a body force acts on the seam (P1: from up to 1.6e-4 to
  below 7e-11 relative), with a change of the SECT-M exact-solution error below 0.2 %. Its order benefit (Y5) needs seam terms of the F-G/F-S kind that are not in the code.
* Not wired into `schema.rs` / `sim.rs` (the transient kernel has no rotating-frame sources yet; exposing the key is the user's decision).
* Out of scope and unchanged: the fractional path, the VOF and variable-density transient kernels (their body-force face terms were not examined).

### Deviations from the plan

* None in the registered rows. The P1 and N2b(i) tests run by default (they take about 2 s in release); the heavy rows are ignored.
* The K tree used its own target directory (`target-w36a-k`) and was not committed.

### Unverified references

None used.

### Final gate (fresh `git archive` of 22976fb)

`cargo fmt --all --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean; `cargo test --workspace --release --no-fail-fast`:
**1856 passed, 0 failed, 499 ignored** (wave 35: 1853 / 0 / 494; this ADR adds 3 default tests, P1 and the two N2b checks, and 5 ignored ones, the smoke,
B-R4 and the three known misses). `baseline_hashes` and `w33b_default_bits` passed unchanged. This section is docs only and was not re-gated.