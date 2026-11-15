# ADR 0594: F-670 registration: load height on the pre-stressed warping modes and harmonic path

Status: **REGISTERED 2026-11-15, written before any code of F-670 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-670 was written, compiled or run in the repository: the files listed under "What was
read" were read, and the arithmetic below was done with one plain numpy script in the scratch folder
(`w49i-work/f670-scratch/arith.py`, log `arith.log`: the section and the sine-series Rayleigh-Ritz energy of ADR 0588 re-typed,
with the kinetic energy of D2 added; its convergence in the number of terms; an independent cubic-Hermite Euler-Bernoulli Vlasov
model of the same energy with the consistent mass; the Timoshenko pinned-beam frequency in closed form; the resonance search of
D4 on the Rayleigh-Ritz modal series; no rcfd code; disclosed, its numbers quoted below). Results go in the addenda at the end;
nothing above the addenda is changed after a run.

Roadmap item F-670 (deps F-663, F-661), structural solver, advanced tier, the follow-up proposed by
[ADR 0588](0588-f663-registration-distributed-loads-at-a-height-on-warping-beams.md) (its "Limits": the load-height term reaches
the pre-stressed modes and the pre-stressed harmonic path through `K_sigma` but is not checked there). The element, mass and
modes are those of [ADR 0548](0548-f626-registration-warping-unknown-on-the-static-solve-modes-and-pre-stressed-modes.md), the
pre-stressed harmonic path that of [ADR 0585](0585-f661-registration-pre-stress-on-the-warping-harmonic-path.md) (with the
resonance-pivot lesson of [ADR 0574](0574-f635-registration-warping-unknown-on-the-harmonic-and-transient-paths.md) amendment
2), the distributed load-height term that of ADR 0588 D1. Number 0594 given by the lead. Branch `f670-work` from `f668-work`
(3c4335ab..f10979ae, which carries F-635, F-643, F-661, F-663, F-667 and F-668), worktree `w49i-work/f670`, scratch
`w49i-work/f670-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test edited;
no new dependency, no unsafe code, the crate graph unchanged; clean-room. Every choice below that the maintainer would normally
be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation), except the open
questions at the end.

## What F-670 asks

The acceptance, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. The lowest flexural-torsional frequency of the
> fork-supported doubly symmetric I of ADR 0588 under a uniformly distributed load of 0.5 of its buckling load on the top flange,
> at the shear centre and on the bottom flange (prestressed_modes, and the resonance of the pre-stressed WarpingHarmonic) equals
> a Rayleigh-Ritz reference of the same energy with the consistent mass derived in the ADR within 1e-2 at 16 elements and falls
> to zero at the ADR 0588 buckling load within 1e-2; results without a load height bit-identical.

Clauses: **a0** every limit registered here; **a1** the lowest flexural-torsional frequency at `0.5` of the buckling load, top
flange, shear centre, bottom flange, by `prestressed_modes` and by the resonance of the pre-stressed `WarpingHarmonic`, within
`1e-2` of the Rayleigh-Ritz reference of D3 at 16 elements; **a2** the same frequency falls to zero at the ADR 0588 buckling load
within `1e-2` (the measure of D5); **a3** results without a load height bit-identical.

### Audit (done before this text): does the term already reach both paths?

Yes, by reading. `Warping::line_load_point` (ADR 0588 D2) stores its entries on the `Warping`; `add_line_load_points` pushes
`c M_H` into the `geometric` triplets at the end of `matrices_impl`. Both paths call `matrices_impl` with the caller's `Warping`:

- `prestressed_modes` -> `modes_impl` with `Some(p)`: `matrices_impl(model, &p.prestress, warping, ..)` and
  `geometric::add_on_pattern(&mats.stiffness, &mats.geometric)`, so `K + K_sigma` holds the term;
- `WarpingHarmonic::new` with `WarpingHarmonicOptions::prestress` -> `with_prestress` -> `reduced_geometric`:
  `matrices_impl(model, &prestress.prestress, warping, ..)` reduced on the same free set, and `response` forms `k + g - omega^2
  m`.

So **no library change is needed for the capability; F-670 is test-only** (as the brief prefers). One point of use the audit
found, registered as D1 and as a negative control (N2): the entry restates its force (ADR 0588 Q1), and the term is linear in
that force, not in the pre-stress. In `buckling_factors` both scale with the factor `lambda`; on the modes and the harmonic path
nothing scales, so the entry must hold the load of the pre-stress as used. A pre-stress made at `q_ref` and scaled by
`WarpingPrestress::scaled(s)` needs the entry at `s q_ref`. The only library edit of F-670 is a sentence saying so in the doc
comment of `Warping::line_load_point` (no code line; D6).

### Readings, registered

- **"The fork-supported doubly symmetric I of ADR 0588"**: `doubly_symmetric_i(0.1, 0.01, 0.18, 0.006, 5/6)`, `L = 3` along
  `x`, 16 Beam2 elements, orientation `y`, steel `E = 2e11`, `nu = 0.3`, `rho = 7850` (ADR 0585), `u = 0` at node 0, `v = w =
  phi = 0` at both ends, warping free; node sets `x0`, `xl`, `mid` (node 8), element set `all`.
- **"Uniformly distributed load of 0.5 of its buckling load"**: the load `q = -0.5 lambda_ref q_ref e_3`, `q_ref = 1000` N/m,
  `lambda_ref` the ADR 0588 Rayleigh-Ritz buckling factor of the case (top `45.1856414209`, shear centre `59.0640165887`, bottom
  `77.1475881351`): a physical load, the same for the model and the reference. (`0.5` of the model's own factor differs by
  `0.5 * 1.7e-5` of the factor; arithmetic 5 turns that into `<= 1e-5` of `omega^2`.)
- **"On the top flange, at the shear centre, on the bottom flange"**: the entry `line_load_point("all", q, (0, +0.095))`, no
  entry, and `(0, -0.095)` (ADR 0588 readings), its force the applied `q` (D1).
- **The pre-stress**: the warping static solve (`solve_static`) of a copy of the fork model with `Load::line_load("all", q)`,
  `WarpingPrestress::from_static(.., [0, 0])` (ADR 0588 A7 measured it within `1.4e-15` of the six-unknown pre-stress on the
  buckling path; the harmonic path takes a `WarpingPrestress` in any case).
- **"prestressed_modes"**: `prestressed_modes(&fork, &pre, &w, &ModalOptions::new(1))`, the frequency `sqrt(eigenvalues[0])`;
  the lowest mode of the beam is the coupled lateral-torsional one (arithmetic 3: the next pole is above `4e4`).
- **"The resonance of the pre-stressed WarpingHarmonic"**: the fork model with the harmonic torque `T = 1000` about `x` at `mid`
  (ADR 0585), `WarpingHarmonicOptions::new().prestress(pre)`, the search of D4 on the undamped midspan twist.
- **"The ADR 0588 buckling load"**: `lambda_cr`, the lowest factor of `buckling_factors_with(&fork, &pre_ref, &w_ref,
  &BucklingOptions::new(3))` of the same 16-element model with the pre-stress and entry at `q_ref` (what ADR 0588 measured; the
  load `lambda_cr q_ref` then reproduces the singular `K + lambda K_sigma` of that model). Supporting checks at `lambda_ref`
  (the Rayleigh-Ritz value) are registered beside it (Y).
- **"Results without a load height bit-identical"**: no library code line changes (D6); the existing bit records and tests run
  unchanged (B0); and on both paths an entry at the shear centre (`c = 0`) gives the bits of no entry (B1).

## What was read (no run)

- `AGENT_RULES.md`; the brief; ADR 0588 (whole), ADR 0585 (whole), ADR 0592 (head), ADR 0574 amendment 2 as quoted in 0585.
- `crates/rcfd-fem/src/beam_warping.rs`: `Warping` and its builders, `matrices_impl`, `add_line_load_points`,
  `hermite_mass_block`, `local_mass`, `WarpingPrestress` (`from_static`, `scaled`, `check_for`), `modes_impl`,
  `reduced_dynamic`, `reduced_geometric`, `WarpingHarmonic` (`new`, `with_prestress`, `response`).
- `crates/rcfd-fem/src/beam.rs`: `local_mass`, `plane_mass`, `plane_stiffness`; `crates/rcfd-fem/src/modal.rs`
  (`ModalOptions`); `crates/rcfd-linalg/src/eigen.rs` (`Shift`, `ResidualTest`, `factor_shifted`: positive pivots on `K + sigma
  M`, `lowest_eigenpairs`: values `mu - sigma`).
- Tests: `sa_beam_warping_f661.rs` (the fork, the search, H3), `sa_beam_warping_f663.rs` (the UDL set-up); the ADR 0588 script
  `f663-scratch/arith.py`.

## Design

### D1. Use of the entry on the modes and the harmonic path (no change)

The second-order potential of a dead line load `q` along the section point `p` is `(1/2) int (q . r) phi^2 dx`, `r = p - s`
(ADR 0588 D1); on the modes and the harmonic path the operator is `K + K_sigma(pre) - omega^2 M` with `K_sigma(pre)` including
`c M_H`, `c = q . r` of the entry. For the static state about which the beam vibrates to be the one the pre-stress describes,
the entry's `q` is the load of the pre-stress: here `q = -0.5 lambda_ref q_ref e_3` in both. This is the existing contract of
ADR 0588 D2 ("it must be the line load of the reference load"), here the reference load is the applied load.

### D2. The kinetic energy (the consistent mass, derived)

The library's mass (ADR 0548 D4, `local_mass`): the Timoshenko consistent mass of each bending plane, `int (rho A N_d^T N_d + rho
I N_psi^T N_psi) dx` with the plane's own interpolation (`plane_mass`: translation and rotary inertia of the section rotation
`psi`), the axial pair `rho A`, and on the Hermite twist `rho I_p int H_i H_j dx`, `I_p = I_y + I_z`, no warping inertia. It is
the consistent discretisation of the kinetic energy

`T = (1/2) int (rho A (v_t^2 + w_t^2 + u_t^2) + rho I_z psi_z,t^2 + rho I_y psi_y,t^2 + rho I_p phi_t^2) dx`.

For the doubly symmetric section the centroid is the shear centre, so the lateral translation `v` of the centroid and the twist
`phi` about the shear centre have no inertial coupling. The reference, Euler-Bernoulli as in ADR 0588 (`psi_z = v'`), keeps the
lateral-torsional part

`T_ref = (1/2) int (rho A v_t^2 + rho I_z v'_t^2 + rho I_p phi_t^2) dx`.

In the sine basis of D3 the mass is diagonal: `M_vv[n] = (rho A + rho I_z k_n^2) L/2`, `M_phiphi[n] = rho I_p L/2`. The rotary
term is kept (it is in the library's mass); arithmetic 3 records its size (`5.3e-4` to `5.5e-4` of `omega^2`).

### D3. The reference (Rayleigh-Ritz, derived)

The energy of ADR 0588 D3 (shear-centre unknowns, Euler-Bernoulli, forks; `beta_x = 0` here) at the load factor `lambda` (load
`lambda q_ref`) with the kinetic energy of D2. Hamilton's principle on `v = sum a_n sin(k_n x) e^{i omega t}`, `phi = sum b_n
sin(k_n x) e^{i omega t}`, `k_n = n pi/L`, `n = 1 .. N` (each meets the fork conditions) gives

`(K + lambda G) x = omega^2 M x`,

`K`, `G` those of ADR 0588 D4 (the integrals by 400-point Gauss-Legendre; `M(x) = (q_z/2) x (L - x)`, `q_z = -q_ref`; `G`
holds `int M sin_m (-k_n^2 sin_n)` and `(q . r) int sin_m sin_n`), `M` of D2. The reference is the lowest `omega^2` at `N = 81`
and `lambda = 0.5 lambda_ref` (arithmetic 3, converged: `N = 41` against `81` within `1.8e-9`).

| case | `c = q_ref . r` per unit `lambda` (N) | `omega_ref^2` (rad^2/s^2) | `omega_ref` (rad/s) | `omega^2/omega_0^2` | `|phi(L/2)/v(L/2)|` (rad/m) |
|---|---|---|---|---|---|
| top (`p_z = +0.095`) | `-95` | `1.249887847956e4` | **`111.798383170594`** | `0.7529` | `4.08927986` |
| shear centre | `0` | `1.154124955201e4` | **`107.430207818881`** | `0.6952` | `3.85897491` |
| bottom (`p_z = -0.095`) | `+95` | `1.067904006072e4` | **`103.339440973516`** | `0.6432` | `3.46106352` |

`omega_0^2 = 1.660191376344e4` (the unloaded lateral mode, equal to `E I_z k_1^4/(rho A + rho I_z k_1^2)` to `2e-16`). The
lowest eigenvalue of the same reference at `lambda = lambda_ref` is `<= 4.2e-12 omega_0^2` for every case (the reference
buckling load is the zero of its own frequency).

### D4. The resonance search (registered; the ADR 0574/0585 lesson applied)

At `0.5 lambda_ref`: the search of ADR 0585 D4 unchanged (secant on `g(s) = 1/Re X_d(s)`, `X_d` the undamped midspan twist,
`s = omega^2`; started at `0.98` and `1.02` of `omega_ref^2`; ends when `|s_{j+1} - s_j| <= 1e-6 |s_j|`, at most 20 steps; a
solve refused as a resonance, key `"frequency"`, ends the search with that iterate as the result). **On the default resonance
pivot a refusal is expected to end the search**: it refuses an iterate within about `1e-8` relative of the model's eigenvalue
(ADR 0574 amendment 2; ADR 0585 H2 ended so at step 2), which is `1e6` times below the limit `1e-2`. The consistency checks C
run the search with `resonance_pivot(0.0)`, so that a refusal cannot end them.

Near the zero (D5): the same secant started at `0.02` and `0.01` of `omega_0^2` (the model's), with one added rule: an iterate
`s_{j+1} < 0` ends the search with that iterate as the result (the response is not defined for `omega^2 < 0`; the extrapolated
root is the measure). On the Rayleigh-Ritz modal series (arithmetic 6) the search ends by the step rule in 6 steps at the
eigenvalue (to round-off) when the zero is positive, and by the negative rule in 2 steps within `1.7e-6 omega_0^2` of the
eigenvalue when it is about `-3e-5 omega_0^2`.

### D5. "Falls to zero within 1e-2", the measure (registered)

`z = |omega^2(lambda)| / omega_0^2`, `omega_0^2` the lowest eigenvalue of `modal_analysis` of the unloaded 16-element model
(the same mass and stiffness), `omega^2(lambda)` the lowest eigenvalue of `prestressed_modes` (or the root `s_r` of the search
D4) at the load `lambda q_ref`; met when `z <= 1e-2` at `lambda = lambda_cr` (acceptance, Z) and, as a supporting check, at
`lambda = lambda_ref` (Y). The squared ratio is the eigenvalue itself (it may be slightly negative at `lambda_cr` and is about
`-3e-5` at `lambda_ref`: the shear-flexible model buckles `1.7e-5` below the Euler-Bernoulli reference, ADR 0588 A1 to A3).

`prestressed_modes` at a singular or slightly indefinite `K + K_sigma` cannot factor at the default shift 0 (`factor_shifted`
requires positive pivots of `K + sigma M`), so the zero checks use `ModalOptions::new(1).shift(Shift::Value(sigma))`, `sigma =
0.5 omega_0^2` (`K + sigma M` positive definite while the lowest eigenvalue is above `-sigma`), the other options default.

### D6. What changes

`tests/sa_beam_warping_f670.rs` (new). In `beam_warping.rs` one doc sentence on `Warping::line_load_point` (D1): no code line,
so every result is the same bit for bit. No other file of the library.

## Arithmetic done before the code (disclosed; `arith.log`)

1. The ADR 0588 buckling references re-derived through the re-typed functions: within `6.7e-13` of the registered values.
2. Unloaded: `omega_0^2` Euler-Bernoulli `1.66117847e4`, with the rotary inertia (Rayleigh) `1.660191376344e4` (`-5.9e-4`);
   Timoshenko-Rayleigh pinned (the 2 x 2 frequency equation of `v = sin`, `psi = cos`, mode 1, `kappa = 5/6`):
   **`1.657120999148e4`, `omega_T = 128.729211880906` rad/s**, `-1.85e-3` from Rayleigh in `omega^2` (`E I_z k^2/(kappa G A) =
   1.855e-3`).
3. The table of D3; the next eigenvalues at `0.5 lambda_ref`: top `4.99e4`, shear centre `6.27e4`, bottom `8.38e4` (then
   `2.5e5`); the twist share of the kinetic energy of mode 1: `0.110`, `0.099`, `0.081`.
4. The Hermite Euler-Bernoulli FE of the same energy with the consistent mass of D2, 16 elements, the library's per-element
   moment (the `L^2` projection of ADR 0588 D3): `omega` `+1.2e-6` to `+1.5e-6` from the reference (8 elements `+1.8e-5` to
   `+2.4e-5`); `omega^2(lambda_FE)/omega_0^2` `<= 6.5e-11`; `omega^2(lambda_ref)/omega_0^2` `+5.7e-6` to `+7.0e-6`.
   **Estimate for the rcfd model at 16 elements: `omega` low by about `1e-3`** (the Timoshenko lateral plane, arithmetic 2, the
   lateral kinetic share `0.92` of mode 1), discretisation `1.5e-6`.
5. Sensitivity `d ln omega^2/d ln lambda` at `0.5`: `-0.70`, `-0.80`, `-0.88`; `omega^2` at `lambda_ref (1 + 1.72e-5)`:
   `-3.2e-5`, `-2.7e-5`, `-2.4e-5` of `omega_0^2`. **Estimate for Y: `3e-5`.**
6. The search D4 on the Rayleigh-Ritz modal series (`X_d = T sum phi_i(L/2)^2/(omega_i^2 - s)`, M-orthonormal modes): at `0.5`
   4 steps, `<= 2.4e-11 omega_0^2` from the eigenvalue; at `lambda_ref` (zero `<= 1.4e-7`) 6 steps, at the eigenvalue; at
   `lambda_RR (1 + 1.7e-5)` the negative rule after 2 steps, `<= 1.7e-6 omega_0^2` from the eigenvalue.
7. Negative controls at `0.5 lambda_ref`: the top case without the entry (the load at the shear centre): `omega` `+4.01e-2`
   from the top reference; the bottom case without it `-1.10e-1`; the top entry left at `q_ref` while the load is `0.5
   lambda_ref q_ref` (the scale of D1 forgotten): `+3.87e-2`; the entry with the wrong sign: top `+6.4e-2`, bottom `-3.6e-1`.
   Ratios: `omega_top/omega_sc = 1.040660587`, `omega_bot/omega_sc = 0.961921633`.

## Checks (limits fixed now)

Every aggregated error is asserted finite before it is compared (a non-finite value is a miss). The set-up of the readings at 16
elements unless stated; "the case" runs top, shear centre and bottom (suffixes T, S, B).

| id | check | limit | estimate |
|---|---|---|---|
| B0 | `sa_beam_warping_f598_bits`, `_f626_bits`, `_f635_bits`, `_f661_bits`, `_f663_bits`, `_f667_bits`, `_f661`, `_f663`, `_f667`, `_f668` pass unchanged; `cargo test -p rcfd-api --release --test baseline_hashes` passes | pass | pass |
| B1 | **acceptance a3** on both paths: the shear-centre case with an entry at `(0, 0)` against no entry: the `prestressed_modes` eigenvalue and the harmonic response (re and im, every equation) at `0.5 omega_ref^2` bit-equal | bit-equal | bit-equal |
| M0 | the mass: `omega_0` of `modal_analysis` (unloaded) against `omega_T` of arithmetic 2 | `<= 1e-4` relative | `2e-6` |
| A-T/S/B | **acceptance a1, modes**: `|sqrt(eigenvalues[0])/omega_ref - 1|` at `0.5 lambda_ref` | `<= 1e-2` | `1e-3` |
| H-T/S/B | **acceptance a1, harmonic**: the search D4 (default pivot), `|sqrt(s_r)/omega_ref - 1|` | `<= 1e-2` | `1e-3` |
| C-T/S/B | the search with `resonance_pivot(0.0)` against `eigenvalues[0]` of `prestressed_modes` (same pre-stress) | `<= 1e-8` relative | `1e-10` |
| Z-T/S/B | **acceptance a2, modes**: `z` of D5 at `lambda_cr` (shift `sigma`) | `<= 1e-2` | `1e-8` |
| ZH-T/S/B | **acceptance a2, harmonic**: `z = |s_r|/omega_0^2` of the search D4 near the zero at `lambda_cr` | `<= 1e-2` | `1e-8` |
| Y-T/S/B | `z` at `lambda_ref`, modes | `<= 1e-2` | `3e-5` |
| YH-T/S/B | `z` at `lambda_ref`, harmonic | `<= 1e-2` | `3e-5` |
| S-T/S/B | the mode: `|phi(L/2)/v(L/2)|` of mode 1 of `prestressed_modes` (components 3 and 1 of node 8) against D3 | `<= 2e-2` relative | `2e-3` |
| N1 | negative control, modes: the top load without the entry against the top reference `> 2e-2` (`4.0e-2`); the bottom load without it against the bottom reference `> 5e-2` (`1.1e-1`) | as stated | as stated |
| N2 | negative control (D1): the top load with the entry at `q_ref` (not scaled) against the top reference | `> 2e-2` | `3.9e-2` |
| N3 | `omega_top/omega_sc` against `1.040660587` and `omega_bot/omega_sc` against `0.961921633` (modes) | `<= 2e-3` relative each | `5e-4` |

Negative controls: N1 (the term acts on the modes path at its size), N2 (the force of the entry is the load as used), the
mutations X1 to X7 (each path separately). B1 and the zero checks Z are self-consistent and cannot see a term missing on both
the buckling and the modes path (mutation X1); A, H, Y and N3 can.

## Planned mutations (on a scratch copy, one at a time, own `CARGO_TARGET_DIR`)

| id | mutation | expected caught by |
|---|---|---|
| X1 | `add_line_load_points` not called in `matrices_impl` (every path) | A-T/B, H-T/B, Y-T/B, YH-T/B, N3 (not Z, ZH: consistent) |
| X2 | `reduced_geometric` with the entries removed (the harmonic path only) | H-T/B, C-T/B, ZH-T/B, YH-T/B |
| X3 | `modes_impl` with the entries removed (the modes path only) | A-T/B, C-T/B, Z-T/B, Y-T/B, S, N3 |
| X4 | the term doubled (`2 c`) | A-T/B, H-T/B, Y, YH, N3 |
| X5 | the term with the wrong sign | A-T/B, H-T/B, Y, YH, N3 |
| X6 | the rotary inertia of the plane x-y dropped (`plane_mass` with `m_i = 0` for that plane) | M0 |
| X7 | `K_sigma` left out of the real part of the pre-stressed harmonic (kept in the Rayleigh term) | H, C, ZH, YH |

## Decision rule

**done**: every registered check (B0, B1, M0, A, H, C, Z, ZH, Y, YH, S, N1, N2, N3) met in the one run and every mutation
caught by at least one check. **partial**: any registered check missed (a `*_known_miss` record), any mutation not caught, or
any check met only after a post-run amendment. **todo**: A or H missed for any case, or B0/B1 missed.

## Limits (not covered)

- The doubly symmetric I only (the acceptance); the monosymmetric I couples the lateral translation of the centroid with the
  twist in the mass (shear centre off the centroid), not checked here.
- The simply supported span under a UDL; other supports, other loads, damping on these paths with a load height, the
  `WarpingTransient::with_prestress` path (ADR 0591: it takes the same `matrices_impl`; not checked here).
- The Euler-Bernoulli reference against a shear-flexible element (arithmetic 2, 4); no warping inertia (as the library).
- The force restated on the entry and not tied to the pre-stress (D1, ADR 0588 Q1): a scaled pre-stress with an unscaled
  entry is accepted silently (N2 shows its size).
- No case-file key (F-625).

## Open questions (none changes a default)

- **Q1. Tie the entry's force to the pre-stress** (D1): (a) leave it restated and documented (this item); (b) carry a
  `line_load_point` scale in `WarpingPrestress` (scaled by `scaled`) so that the term follows the pre-stress; (c) read the force
  from the static model's `Load::LineLoad` in `from_static` (ADR 0588 Q1 (b)). Recommendation (a) now, (c) with a case-file key
  (F-625); taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).

## Addendum 1 (2026-11-15): results

Written after the one registered run of `tests/sa_beam_warping_f670.rs` (release, log `f670-scratch/run1.log`). Commits:
e66bee5c (this registration), fd3cdee3 (the test file, and in `beam_warping.rs` the doc comment of `Warping::line_load_point`,
lines 344 to 347: three comment lines added, no code line). `cargo fmt -p rcfd-fem` was applied after the run (it re-wrapped
two signatures of the test file; no value or limit changed). No limit was changed, no amendment written, nothing re-run to
pick a result. Logs in `w49i-work/f670-scratch`: `run1.log`, `b0.log`, `baseline.log`, `fmt.log`, `clippy.log`,
`docs_site.log`, `mutations.log`, `mut_X*.log`; `arith.py`, `arith.log` (before any code).

### Results against the registered limits

| id | measured | limit | verdict |
|---|---|---|---|
| B0 | `sa_beam_warping_f598_bits`, `_f626_bits`, `_f635_bits`, `_f661_bits`, `_f663_bits`, `_f667_bits` 1 each; `_f661` 4, `_f663` 5, `_f667` 5, `_f668` 3 passed; `baseline_hashes` 3 passed 1 ignored | pass | met |
| B1 (a3) | shear-centre entry at `(0, 0)` against none: modes eigenvalue `1.1522422304682117e4` both; harmonic response at every equation, re and im: bit-equal | bit-equal | met |
| M0 | `omega_0` against `omega_T`: `4.0e-6` (eigenvalue `16571.3425`) | `1e-4` | met |
| A-T / S / B (a1, modes) | `8.09e-4` / `8.16e-4` / `8.29e-4` (model low) | `1e-2` | met |
| H-T / S / B (a1, harmonic) | `8.09e-4` / `8.16e-4` / `8.29e-4`; each search ended at step 3 by the resonance refusal D4 predicted | `1e-2` | met |
| C-T / S / B | `3.8e-11` / `6.0e-12` / `7.4e-13` | `1e-8` | met |
| Z-T / S / B (a2, modes) | `lambda_cr` `45.184863919`, `59.063014527`, `77.147015523` (as ADR 0588 measured); `z` `2.8e-13` / `7.7e-13` / `4.5e-13` | `1e-2` | met |
| ZH-T / S / B (a2, harmonic) | `9.3e-11` / `1.7e-10` / `2.7e-10` (each ended at step 3 by a resonance refusal, the refused iterate the result) | `1e-2` | met |
| Y-T / S / B | `3.20e-5` / `2.71e-5` / `1.04e-5` (eigenvalue negative: beyond the model's buckling load) | `1e-2` | met |
| YH-T / S / B | `3.11e-5` / `2.58e-5` / `8.75e-6` (the negative rule after 2 steps) | `1e-2` | met |
| S-T / S / B | `1.59e-3` / `1.40e-3` / `1.27e-3` | `2e-2` | met |
| N1 | top without the entry `3.92e-2`; bottom without it `1.109e-1` | `> 2e-2`; `> 5e-2` | met |
| N2 | top entry left at `q_ref`: `3.78e-2` | `> 2e-2` | met |
| N3 | top/SC `7.1e-6`; bottom/SC `1.28e-5` | `2e-3` | met |

Every value is near its estimate. The frequencies are `8e-4` below the Euler-Bernoulli reference: the Timoshenko lateral plane
of arithmetic 2 (the Timoshenko `omega_0` is `9.2e-4` below the Rayleigh value; the lateral share of mode 1 is about `0.9`).
The offset cancels in the ratios N3 to `1e-5`. At the shift `sigma` the eigensolver converged with the default relative
residual test (residuals `<= 2.5e-11`, Sturm counts 1 as expected).

### Mutations (scratch copy `f670-scratch/mut` from `git archive` of fd3cdee3, own target directory, one at a time; the unmutated copy passes)

| id | mutation | caught by |
|---|---|---|
| X1 | `add_line_load_points` not called | A-T/B, H-T/B, S-T/B, N3, Y-T, YH-T; then the modes refuse at Y-B (`K + sigma M` not positive definite: the mutated beam is far beyond its buckling load), which ends that test. Z and ZH not caught (self-consistent), as registered |
| X2 | entries removed on the harmonic path | H-T/B, C-T/B, ZH-T/B, YH-T/B |
| X3 | entries removed on the modes path | A-T/B, S-T/B, C-T/B, N3, Z-T, Y-T; then the modes refuse at Y-B |
| X4 | term doubled | A-T/B, H-T/B, S-T/B, N3; then the modes refuse at Y-T |
| X5 | term with the wrong sign | A-T/B, H-T/B, S-T/B, N3, Y-T, YH-T; then the modes refuse at Y-B |
| X6 | rotary inertia of the plane x-y dropped | M0 (`3.0e-4`) |
| X7 | `K_sigma` left out of the real part of the harmonic | H, C, ZH, YH for every case |

Seven of seven caught. S also caught X1 and X3 to X5, which the registration did not list.

### Other runs

`cargo fmt -p rcfd-fem --check`: clean. `cargo clippy -p rcfd-fem --release --all-targets -- -D warnings`: clean. `cargo test
-p rcfd-cli --release --test docs_site` for the links of this ADR and of `docs/reference-data.md` (which has a new F-670
section): passed (`docs_site.log`). Not run: the whole `rcfd-fem` suite and the workspace (the lead runs the gate).

### Decision by the registered rule

**done**: every registered check met in the one run, no amendment, seven of seven mutations caught. By clause: a1 met on both
paths for the three load heights (`<= 8.3e-4` against `1e-2`); a2 met on both paths at the model's buckling load (`<=
2.7e-10`) and at the Rayleigh-Ritz one (`<= 3.2e-5`); a3 met (no code line changed; B0, B1, `baseline_hashes`). The audit
found the capability already in place, so F-670 adds the checks and documents how the entry's force must be given. Whether it
is done is the maintainer's call.

### Remaining questions

- Q1 (tie the entry's force to the pre-stress): options (a) restated and documented (as now; N2 shows that a forgotten scale
  moves the top-flange frequency by `3.8e-2`); (b) a scale carried in `WarpingPrestress` and applied by `scaled`; (c) the force
  read from the static model's line load in `from_static`. Recommendation (a) now, (c) with a case-file key (F-625); taken on
  the maintainer's standing instruction of 2026-11-11 (take the recommendation).
- Two cases are not checked: the monosymmetric I (inertial coupling of the centroid translation with the twist about the shear
  centre) and the transient path with a load height. A follow-up is proposed below for the first. The second is a small
  extension of F-667 if a case needs it.

## For the lead: roadmap entries

> Proposed status: done.

> F-670 note: Structural solver, advanced tier. Follow-up of F-663 and F-661 (ADR 0588, ADR 0585, 2026-11-15). DONE. ADR 0594, 2026-11-15: the distributed load height of `Warping::line_load_point` reaches `prestressed_modes` and the pre-stressed `WarpingHarmonic` through K_sigma of `matrices` (audit; no code change, only a doc sentence: the entry holds the load as applied, so a pre-stress scaled by `WarpingPrestress::scaled` needs its force scaled too). Fork-supported doubly symmetric I of ADR 0588, L = 3, 16 elements, UDL of 0.5 of the ADR 0588 buckling load on the top flange, at the shear centre and on the bottom flange: the lowest flexural-torsional frequency, from the modes and from the harmonic resonance, is 8.1e-4 to 8.3e-4 below a sine-series Rayleigh-Ritz reference derived in the ADR (Euler-Bernoulli with rotary inertia, the kinetic energy of the library's consistent mass). The offset comes from the Timoshenko lateral plane: the unloaded frequency is within 4e-6 of the Timoshenko closed form. Harmonic resonance and pre-stressed eigenvalue agree to 4e-11, the mode shape to 1.6e-3. At the model's buckling load omega^2/omega_0^2 <= 7.7e-13 (modes) and 2.7e-10 (harmonic); at the Rayleigh-Ritz buckling load 3.2e-5. Results without a load height bit-identical (no code line changed; an entry at the shear centre gives the bits of none on both paths; the ADR 0546 to 0591 bit records; baseline_hashes). Seven of seven mutations caught. Not covered: monosymmetric sections, the transient path with a load height, other supports and loads, warping inertia, the force not tied to the pre-stress, no case-file key.

> Follow-up proposed: Load height on the pre-stressed modes of a monosymmetric warping beam. Deps F-670, F-627. Acceptance: Every limit not stated here is registered in the ADR before the run. The lowest flexural-torsional frequency of the fork-supported ADR 0553 monosymmetric I under a uniformly distributed load of 0.5 of its ADR 0588 buckling load at the shear centre, at the centroid and on flange 1 (prestressed_modes) equals a Rayleigh-Ritz reference with the consistent mass, including the inertial coupling of the shear-centre offset, derived in the ADR within 1e-2 at 16 elements; results without a load height bit-identical.
