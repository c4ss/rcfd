# ADR 0563: F-636 registration: a bore-zone witness for the finite-strain growth of the B-bar creep cylinder

Status: **REGISTERED 2026-11-15, written before any code of F-636 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-636 was written, compiled or run in the repository. The arithmetic below was done by
hand and with plain numpy prototypes in the scratch folder `w49i-work/f636-scratch` (no rcfd code; disclosed, their numbers
quoted below): `probe_mut1d.py` (a copy of ADR 0549's `probe_fbar1d_loc.py`, the one-dimensional F-bar proxy of the registered
FE, with the mutations MU3 and MU4 as flags and the bore displacement printed; logs `probe_base.log`, `probe_mu3.log`,
`probe_mu4.log`), `probe_mut1d_small.py` (the same at `p/1000`; logs `probe_*_small.log`), `probe_drift.py` (log
`probe_drift.log`); the reference displacements are those of ADR 0549's `probe_ref1d.log`. Results go in the addendum at the end;
nothing above the addendum is changed after a run.

Roadmap item F-636 (dep F-580), the follow-up 1 of [ADR 0549](0549-f580-registration-creep-on-the-b-bar-hex8-against-a-finite-strain-reference.md)
(its open question 1). Number 0563 given by the lead. Branch `f636-work` from 0e1e4ea0, worktree `w49i-work/f636`, scratch
`w49i-work/f636-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test edited; no new crate, no unsafe
code; clean-room. Choices the maintainer would normally be asked about are taken on the maintainer's standing instruction of
2026-11-11 (take the recommendation), as marked.

## What F-636 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. On the cylinder of ADR 0549 at the registered p, the error of the points with r < 1.1 a against the finite-strain reference of ADR 0549, its growth between 5 t* and 20 t* within a registered bound; the mutations of ADR 0549 that were not caught (the linear strain in the BBar arm, the creep rate halved) caught by a registered check; the checks of ADR 0549 unchanged.

Clauses: (b0) every limit registered here; (b1) the bore-zone error `e_B` (points with reference radius `r < 1.1 a`) against the
finite-strain reference of ADR 0549, and its growth `|e_B(20 t*) - e_B(5 t*)|` within a registered bound; (b2) MU3 (the linear
strain in the BBar arm) and MU4 (the creep rate halved) of ADR 0549 caught by a registered check; (b3) the checks of ADR 0549
unchanged.

## What was read (no run)

`AGENT_RULES.md`; the brief; ADR 0549 in full (the registration, amendment 1, addendum 1, the diagnosis of MU3 and MU4, the open
questions, the follow-ups); ADR 0528 (the bore displacement witnesses LC-u5, LC-u20 and why they did not see locking);
`crates/rcfd-fem/tests/si_fbar.rs` (the helpers `Checks`, `worst`, `max_abs`, `rebuild`, `with_formulation`, `element_coords`,
`element_u`, `newton`, `cylinder`, `PolarPoint`, `polar_points`, `creep_material`, `lame`, `stationary`, `creep_times`,
`creep_run`, `bore_radial`, and the F-580 section `ref_return`, `Reference`, `reference_with`, `ref_at`, `ref_differences`,
`ref_error`, `f580_checks`); `crates/rcfd-fem/src/nonlinear.rs` (`fbar_kinematics`, the site of MU3; the Newton tolerances,
residual and energy `1e-10` relative); ADR 0549's `mutate.py` (the exact MU3 and MU4 replacements) and `mut-MU3.log`,
`mut-MU4.log` (the numbers of the library under the mutations, as recorded in ADR 0549 addendum 1); `docs/roadmap.json`
(F-636, F-580).

## Readings, registered

- **(b1)** "The error of the points with `r < 1.1 a`" is ADR 0549's `e_F` restricted to the stiffness-rule points whose
  reference radius is below `1.1`: `e_B(t) = max_{points, r < 1.1} max(|sigma_r - sigma_r^ref|, |sigma_theta -
  sigma_theta^ref|) / p`, the reference that of ADR 0549 with its corrected stop (amendment 1: `reference_with(p, 400, 1e-9,
  false)`); the growth is `G = |e_B(20 t*) - e_B(5 t*)|`. The bound is registered below from the proxy estimate (arithmetic 2).
- **(b2)** "Caught by a registered check": the mutated library, run once against the new test, misses at least one of its
  registered checks (or the solve fails). The catcher is registered with its estimate before any run; the prediction of each
  check for each mutation is stated here (arithmetic 4), including the prediction that `G` itself catches neither.
- **(b3)** ADR 0549's checks are those of `f580_amendment_1_creep_bbar_against_the_finite_strain_reference` in `si_fbar.rs`.
  "Unchanged": `si_fbar.rs` is not edited (U1: `git diff 0e1e4ea0 -- crates/rcfd-fem/tests/si_fbar.rs` empty), no library file
  changes (S0), and that test passes in the full `cargo test -p rcfd-fem --release` (U2).

## Arithmetic (before any code; probes disclosed)

1. **Why the bore-zone error alone does not see MU3 and MU4** (ADR 0549 addendum 1, and the proxy). In the proxy (48 graded
   linear elements, two points, the variational F-bar, finite strain; it reproduced the library's F3 within 10 percent, and
   under the mutations it gives F3 `5.09e-4` for MU3, the library's `5.09e-4`, and `3.29e-4` for MU4, the library's `4.51e-4`):
   `e_B` is `5.09e-3` at `5 t*` and `9.42e-3` at `20 t*` (both at `R = 1.093`, the coarsest elements of the bore zone), so
   `G = 4.33e-3`. This growth is the pressure drift inside the F-bar element that ADR 0549 reported as F4 (`4.4e-3` at `r = 1.05`,
   equal in `sigma_r` and `sigma_theta`, F-637's subject): the signed differences at the worst point are `(+5.1e-3, +3.9e-3)` at
   `5 t*` and `(+9.4e-3, +8.2e-3)` at `20 t*`. Under MU3 `e_B` is `4.94e-3` and `8.02e-3`, `G = 3.08e-3`; under MU4 `5.63e-3`
   and `9.02e-3`, `G = 3.40e-3`. **Both mutations lower the bore-zone growth** (the drift they remove is a few `1e-3` of `p` at
   the bore, against an O(h) error and an element pressure drift of the same size), so no upper bound on `G` can catch them, and
   a two-sided window around the unmutated estimate would be set from the mutants' numbers. `G` is registered with its bound
   (the acceptance's clause), and the mutations are given a witness that does not carry the O(h) stress error.
2. **The bound of `G`.** Estimate `4.33e-3` (proxy). The library departed from the proxy by up to 10 percent on the unmutated
   F3 (`1.51e-3` against `1.37e-3`), not at all on F4 (`4.396e-3` against `4.394e-3`), and by 37 percent on a small number
   (MU4's F3). **Bound `6e-3`** (1.39 times the estimate). It stays below the reference's own departure from the small-strain
   stationary field between `5 t*` and `20 t*` (`6.30e-3`, ADR 0549 R3): an element whose bore-zone error grew by the full
   finite-strain drift would be near it. With `e_B(5 t*)` and `e_B(20 t*)` themselves (B1, B2) under ADR 0549's `2e-2`.
3. **The witness of the finite-strain drift: the bore displacement at `p` against the bore displacement at a small pressure
   `q`.** The O(h) error of the linear element is linear in the load; at a small `q` the solution scaled by `p/q` carries the
   same discretisation error and no finite-strain drift (with `A = 1/(E q^3)` the small-strain problem in `sigma/q` and `t/t*`
   is the same at every `q`: ADR 0549 arithmetic 3). So the difference of the two runs isolates the finite-strain part of the
   element's solution, and the same difference of the reference isolates the finite-strain part of the continuum problem. The
   bore radial displacement is used because the stress at the bore is fixed by the load (equilibrium) and does not see the
   kinematics at first order, while the displacement integrates the strain measure and the creep strain (MU3 changes the first,
   MU4 the second). Defined over the growth interval of the acceptance: `Du(x) = u_a(20 t*; x) - u_a(5 t*; x)` (FE: the mean
   radial displacement of the bore nodes, `bore_radial` of ADR 0528; reference: `u` at `R = a`), `delta = Du(p)/p - Du(q)/q`,
   and the check **D: `|delta_FE / delta_ref - 1|`**. Numbers (`probe_drift.log`, the reference to the five digits of ADR
   0549's `probe_ref1d.log`, `q = p/1000`): reference `Du(p) = 6.1846e-3`, `Du(q) p/q = 6.2056e-3`, `delta_ref = -2.100e-5`
   (`+-1.4e-7` from the printed digits; `-3.4e-3` of `Du`); proxy unmutated `delta = -2.0946e-5`, **D `2.6e-3`** (within the
   reference's digit uncertainty `6.7e-3`), while the absolute error of the proxy's `Du` against the reference is `-2.0e-4` of
   `Du` at both pressures (cancelled). MU3: `delta = -1.698e-5`, **D `0.19`**; MU4 (the mutated law in both FE runs, the reference
   unmutated): `delta = -5.67e-6`, **D `0.73`** (the drift goes with the square of the creep strain: half the creep, a quarter
   of the drift). **Limit `5e-2`**: 5 times above the unmutated estimate bounded by the digit uncertainty (`<= 1e-2`), 3.8 times
   below MU3's. The three-dimensional mesh adds effects the proxy does not have (the bore as a polygon of 24 chords, sagitta
   `5.3e-4 a`; the angular variation of the points); being linear in the load at first order they cancel in `delta` like the
   O(h) error. A miss would be recorded, not tuned.
4. **Predictions per check and mutation** (registered; the mutations run once each on a scratch copy against the new test):

   | Mutation | B1, B2 | G (`<= 6e-3`) | D (`<= 5e-2`) | Caught |
   |---|---|---|---|---|
   | none | `5.1e-3`, `9.4e-3` | `4.33e-3` met | `<= 1e-2` met | |
   | MU3 (ADR 0549's replacement of `green_lagrange(&h)` by `sym(h)` in `fbar_kinematics`) | `4.9e-3`, `8.0e-3` met | `3.08e-3` met | `0.19` missed | yes, by D |
   | MU4 (ADR 0549's `time_factor` for `m = 0` returning `dt/2`) | `5.6e-3`, `9.0e-3` met | `3.40e-3` met | `0.73` missed | yes, by D |

   Neither mutation is declared equivalent.
5. **The small pressure.** `q = p/100` (taken on the maintainer's standing instruction of 2026-11-11, the recommendation): the
   library's Newton tolerances are relative (`1e-10`) and its Green-Lagrange strain is formed from `h` without a subtraction,
   but no FE run at `p/1000` exists in the repository while ADR 0528 ran the cylinder at `p/10`; `p/100` keeps the strains at
   `1e-6` and above. Its residual finite-strain drift is `1/100` of that at `p`, in the FE and in the reference alike (both
   are run at the same `q`), so `delta` falls by a common factor `0.99` and D is unchanged to first order; the estimates above
   at `q = p/1000` stand. The reference at `q` uses the corrected stop `1e-9` (ADR 0549 amendment 1, whose rounding floors at
   `p/1000` were at most `2e-11`).
6. **Resolution of the witness.** The reference stop `1e-9` of `max |u|` gives a displacement error at most `1e-9 u_a`, so at
   most `2e-11` in `Du` against `delta_ref Du ~ 2e-5`: `1e-6` relative. The library's Newton stop `1e-10` relative on the
   residual: of the same order. The reference's convergence in `N` (ADR 0549 probe: `u_a` at `N = 200` and `400` agree to the
   five printed digits at both pressures): R4, `|delta_ref(400) / delta_ref(800) - 1| <= 1e-2`, estimate `<= 2e-3`. The
   control R5 that the witness measures a finite-strain effect: `|delta_ref| / |Du_ref(q)/q| >= 1e-3` (estimate `3.4e-3`; a
   reference without the finite-strain terms gives 0).
7. **Cost.** Two library runs (`creep_run(BBAR, p)`, `creep_run(BBAR, q)`, about 15 s each in release, ADR 0549 log) and four
   references (`N = 400, 800` at `p` and `q`, under a second each). One test, below one minute.

## Design

Test only, a new file `crates/rcfd-fem/tests/si_fbar_bore.rs`; no library change, `si_fbar.rs` not edited. The helpers of
`si_fbar.rs` needed here are copied verbatim (`Checks`, `worst`, `max_abs`, `rebuild`, `with_formulation`, `element_coords`,
`element_u`, `newton`, `cylinder`, `PolarPoint`, `polar_points`, `creep_material`, `creep_times`, `creep_run`, `bore_radial`,
`ref_return`, `RefElement`, `ref_at`, `ref_differences`), since a test binary cannot import another's functions and including
`si_fbar.rs` as a module would run its tests a second time. One change in the copy: the copied `reference_with` also records
the radial displacement of the node at `R = a` at every step end (`ua: Vec<f64>`), the solve itself unchanged.

- `fn bore_error(pts, r, k, p) -> (f64, f64)`: `e_B` and the radius of its worst point.
- `fn bore_mean(model, u) -> (f64, f64)`: the mean and the spread (max minus min) of `bore_radial`.
- One test `f636_bore_zone_witness_of_the_b_bar_creep_cylinder`, run once: `creep_run(BBAR, CR_P)`, `creep_run(BBAR, CR_P /
  100)`, `reference_with(p, 400 | 800, 1e-9, false)` at `p` and at `q`; all checks printed and aggregated by `Checks`.

## Checks

`p = 2e5`, `q = p/100`; steps 40 and 70 are `5 t*` and `20 t*`. Every measure is required finite before it is compared.

| Id | Clause | Measure | Limit | Estimate |
|---|---|---|---|---|
| A0 | b1 | the two BBar runs complete the 71 step ends (`creep_run` asserts the times) and the bore-zone set is not empty (at least 16 points with `r < 1.1`) | holds | 12 of 48 element rings, 24 radial point rows, 2304 points |
| B1 | b1 | `e_B(5 t*)` against the reference (`N = 400`) | `<= 2e-2` | `5.1e-3` (proxy) |
| B2 | b1 | `e_B(20 t*)` | `<= 2e-2` | `9.4e-3` (proxy) |
| G | b1 | `|e_B(20 t*) - e_B(5 t*)|` | `<= 6e-3` | `4.33e-3` (proxy) |
| D | b2 | `|delta_FE / delta_ref - 1|`, `delta = Du(p)/p - Du(q)/q` | `<= 5e-2` | `<= 1e-2` (proxy `2.6e-3`) |
| R1B | b1 | reference `N = 400` against `N = 800` at the bore-zone points, steps 40 and 70, over `p` | `<= 1e-4` | ADR 0549 R1 `2.2e-6` over all points |
| R4 | b2 | `|delta_ref(400) / delta_ref(800) - 1|` | `<= 1e-2` | `<= 2e-3` |
| R5 | b2 (control) | `|delta_ref| / |Du_ref(q)/q|` (`N = 400`) | `>= 1e-3` | `3.4e-3` |
| U1 | b3 | `git diff 0e1e4ea0 -- crates/rcfd-fem/tests/si_fbar.rs` | empty | not edited |
| U2 | b3 | `f580_amendment_1_creep_bbar_against_the_finite_strain_reference` passes (in S1) | pass | unchanged bits |
| S0 | | `git diff --stat 0e1e4ea0 -- crates/*/src` | empty | no library change |
| S1 | | `cargo test -p rcfd-fem --release` (all), `cargo test -p rcfd-api --release --test baseline_hashes`, `cargo test -p rcfd-cli --release --test docs_site`, `cargo fmt -p rcfd-fem --check`, `cargo clippy -p rcfd-fem --release --all-targets -- -D warnings` | 0 failed, clean | |

Reported without a limit: `e_B` at `10 t*` and `15 t*` with the worst radii; the bore-zone point-wise growth `max_{r < 1.1}
|d(20 t*) - d(5 t*)|` (ADR 0549's F4 restricted); `Du` of the FE and of the reference at `p` and at `q` and their relative
differences (the cancelled discretisation error, proxy `-2.0e-4`); `delta` at `20 t*` alone (drift from `t = 0`); the spread of
the bore displacement over the bore nodes; the number of bore-zone points.

## Planned mutations (scratch copy `f636-scratch/mut` of the code commit, its own target dir; each run once against the new test)

| Id | Mutation (the replacement text of ADR 0549's `mutate.py`, unchanged) | Expected catcher |
|---|---|---|
| MU3 | `nonlinear.rs`, `fbar_kinematics`: `let e = green_lagrange(&h);` replaced by the linear strain `[h00, h11, h22, h01 + h10, h12 + h21, h20 + h02]` | D (`0.19`); not G, B1, B2 (arithmetic 4) |
| MU4 | `mat_creep.rs`, `time_factor` for `m == 0.0`: `return dt;` replaced by `return 0.5 * dt;` | D (`0.73`); not G, B1, B2 |

## Decision rule

- **done**: A0, B1, B2, G, D, R1B, R4, R5, U1, U2, S0, S1 met in the single registered run, and MU3 and MU4 each caught by a
  registered check (or a failed solve).
- **partial**: any registered limit missed (an ignored `*_known_miss` test with the limit intact and the number), MU3 or MU4 not
  caught, or any check met only after an amendment written after the run.
- **todo**: A0 or G missed together with D.

## Limits (what is not covered)

The witness D sees the finite-strain part of the bore displacement; it does not localise the error in the field, and it does
not diagnose the element's pressure drift (F-637). `G` is bounded above its estimate, which contains that drift; a bound of
ADR 0506's `2e-3` on the bore zone is not claimed. The reference shares the continuum problem with the library (the small-strain
law on `E` and `S`, ADR 0549 Limits); time discretisation is common. Only the registered mesh, `m = 0`, `n = 4`, `b/a = 2`,
`nu = 0.3`. No case-file key.
