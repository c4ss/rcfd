# ADR 0666: F-742 registration: the warm start of the incompatible-mode iteration (the remedy W of ADR 0660) with a J2 material on the punch block of ADR 0636 and on the distorted block DB4 of ADR 0649, on the nonlinear transient path with its energy record from the committed modes, and on the rotating path

Status: **REGISTERED 2026-11-16, written before any code of F-742 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-742 was written, compiled or run: the files listed under "What was
read" were read and the arithmetic below was done by hand (disclosed). No probe was run for this item. Results go in the
addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-742 (deps F-740), the follow-up proposed by
[ADR 0660](0660-f740-registration-a-warm-start-of-the-incompatible-mode-iteration-beside-a-trust-region-on-the-mode-step.md)
("For the lead", follow-up proposed; its remaining question 4). The element is the incompatible-modes Hex8 at finite strain of
ADR 0608 with the stabilisation of ADR 0614 D1 and the rule `IncompatibleStabilisation::LinearStress` of ADR 0630 D1 (rule L);
the remedy W is that of ADR 0660 reading c3 (warm start, backtracking, decrease test, no line search); the reference is the
F-bar Hex8 of ADR 0506; the active-set record is that of
[ADR 0515](0515-f560-registration-newton-order-reading-for-an-active-set-change.md) D2 (`NewtonOptions::record_active_set`,
`LoadStep::yielded`, `LoadStep::set_changed`). Number 0666 given by the lead. Branch `f742-work` from c10a3519 (F-740 / ADR
0660 merged), worktree `w49i-work/f742`, scratch `w49i-work/f742-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical (the new behaviour only with the opt-in `set_incompatible_mode_warm_start(true)` of ADR 0660); no edit of
`baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test edited; no new dependency, no
unsafe code, the crate graph unchanged; clean-room. Every choice below that the maintainer would normally be asked about is
taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation), except the open questions at the end.

## What F-742 asks

The acceptance, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. The remedy W of ADR 0660 with a J2 material on the punch block of ADR 0636 and on one distorted mesh of ADR 0649, and on the nonlinear transient path (its energy record from the committed modes): the results within the ADR 0636 tolerances of the F-bar Hex8 on the same set-ups, the active-set record of ADR 0515 unchanged by the committed modes, the energy record consistent with the driver's modes; the mutations of the new code caught by the item's own checks; the default bits unchanged.

Clauses: **c0** every limit registered here; **c1** W with a J2 material on the punch block of ADR 0636; **c2** the same on one
distorted mesh of ADR 0649; **c3** W on the nonlinear transient path, its energy record from the committed modes; **c4** the
results of c1 to c3 within the ADR 0636 tolerances of the F-bar Hex8 on the same set-ups; **c5** the active-set record of ADR
0515 unchanged by the committed modes; **c6** the energy record consistent with the driver's modes; **c7** the mutations of
the new code caught by this item's own checks; **c8** the default bits unchanged. The title adds the rotating path (**c9**);
the brief: cover it if cheap, else list it as not covered.

### Readings, registered

- **c1: the punch block.** ADR 0636 reading c1 on its coarse regular mesh: `hex_box` `[0, 1]^3` on `4^3`, `xmin`, `xmax`
  `u_x = 0`, `ymin`, `ymax` `u_y = 0`, `zmin` `u_z = 0`, the punch `x, y <= 0.5` (tolerance `1e-9`) on `zmax` at `u_z =
  -0.1`, 10 increments, `NewtonOptions::new(10).threads(4)` with `record_active_set(true)`; measures `R = -sum_punch r_z /
  mu` and `w_c = u_z(1, 1, 1)` with `mu = 1e6` (the normalisation of ADR 0636; with J2 `mu` is only the unit). The helpers of
  `si_eas_hg8.rs` are copied (existing tests are not edited).
- **c2: the distorted mesh.** DB4 of ADR 0649 D2 (the hardest block of ADR 0658 and ADR 0660): the `8^3` block, interior
  nodes moved by `0.25 h (2U - 1)` per component with SplitMix64 seed `0xF719_0008`, the same load case. Fixed now; no other
  mesh is tried.
- **c1, c2: the J2 material.** `Material::j2(Material::isotropic_elastic(E, 0.49, 1000, 0), sigma_y, H, 0)` with `E = 2 mu
  (1 + nu) = 2.98e6` (the shear modulus `1e6` of ADR 0636's neo-Hookean material), `sigma_y = 6.0e4`, isotropic hardening `H
  = 3.0e5` (`E / 10`, so the elastoplastic tangent stays positive), no kinematic hardening. Arithmetic 2 gives the onset.
- **c3: the transient set-up.** The coarse cantilever of ADR 0636 reading c2, regular: `[0, 1] x [0, 0.2]^2` on `10 x 2 x 2`,
  `xmin` clamped, the neo-Hookean material of ADR 0636 (`mu = 1e6`, `kappa(nu = 0.49)`, density `1000`); a tip force `P =
  200` N in `y` split equally over the nine `xmax` nodes, its factor `0` at `t = 0`, `1` at `t = 0.5`, `1` to `t = 1`;
  `NlTransientOptions::new(0.02)` (average acceleration), mass-proportional Rayleigh damping `alpha_R = 22.16` 1/s, `threads(4)`,
  the driver's other defaults; 50 steps from rest. Measures at `t = 1`: `d = mean_tip u_y` and the strain energy `W_s =
  Integrator::strain_energy`. Arithmetic 3 gives the numbers. A history material is not used here: `strain_energy` refuses a
  material with history (ADR 0479 D3), so the energy record of c3 exists only for a material without history.
- **c3, c6: "its energy record from the committed modes", "consistent with the driver's modes".** With the opt-in, the
  modes at which `strain_energy` evaluates an Incompatible Hex8 are found by the local iteration started from the modes in the
  committed state of the element (the start rule of ADR 0660 D1, with its fallback to `alpha = 0`), with the model's update.
  At a committed state the driver's evaluation at the committed displacement from those modes passes its test at the first
  iterate (ADR 0660 arithmetic 4), so the energy record's modes are the committed modes **bitwise** (TE1). Without the opt-in
  the energy record is that of today (cold, an empty state).
- **c4: the tolerances.** ADR 0636 (and ADR 0649 reading c4): `0.15` on a coarse mesh (block `4^3`, cantilever `10 x 2 x 2`),
  `0.10` on a refined one (DB4); for `e_R = |R / R_Fbar - 1|`, `e_w`, and on the transient `e_d`, `e_W` (the strain energy);
  F-bar solved on the same mesh in the same test with the same options except the incompatible opt-ins (which F-bar ignores).
- **c5: "unchanged by the committed modes".** With W the committed state of an Incompatible element is its material state
  (`points x 13` values for J2) followed by nine modes; the record of ADR 0515 D2 (`yielded_points`) reads the values in
  chunks of `state_size` and would count the modes as a further point that changes at every iteration. Read: the record of W
  is the record of the same solve without the modes in the state. The control that has no modes in the state and the same
  local update and root is S (ADR 0658, cold); W differs from S only in the start of the local iteration (ADR 0660 reading
  c3) and reaches the same root to the local tolerance, so the record of W must equal the record of S, entry by entry (AS1).
- **c9: the rotating path.** `RotatingPart::new` (`rotating.rs`) computes its pre-stress by the linear static solve
  (`solve_static` of the copy `unit_centrifugal_model`) and its matrices by the linear assembly; the linear Incompatible Hex8
  (`hex8x.rs`) has closed-form modes and no local iteration, and the nonlinear driver is not called. So the warm start can
  have no effect there; c9 is covered by the record that it has none (RO1: bitwise) and by nothing else (Limits).
- **c7: the new code.** D1 to D3 below; the mutations MA1, MA2, ME1, MR1, MR2, and MJ1 (the driver's opt-in, ADR 0660 MW1,
  re-run against this item's checks).
- **c8: default bits.** Every new branch is behind `model.incompatible_mode_warm_start()`, off by default, and without it the
  library performs the operations of c10a3519 in the same order. Records: the bit tests `si_eas_hg3_bits.rs`,
  `si_eas_hg_default_bits.rs`, `si_eas_default_bits.rs`, `si_fbar_default_bits.rs`, `sa_hyper_default_bits.rs`,
  `sa_emc_default_bits.rs`, `sa_dg_default_bits.rs` (B6); `baseline_hashes` (S5); the existing tests of the changed lines (S7).

## What was read

- `AGENT_RULES.md`; the brief; ADR 0660 (whole, with amendment 1, Addendum 1 and the roadmap section); ADR 0636 (the
  tolerances, the set-ups, PB1 to PC2); ADR 0649 (D2); ADR 0515 (the record, D2); ADR 0658 (through ADR 0660).
- `crates/rcfd-fem/src/nl_incompatible.rs` (`converged`, `evaluate`, `warm_state`, `modes_and_strains_opt`, the record
  functions, `point_gradients`, `stabilisation_energy_with`, `rest_moduli`, `rule_coefficient`), `nonlinear.rs`
  (`yielded_points`, `StateLayout`, `attempt_with` around the record, the two warm call sites, `hex8_evaluate`,
  `creep_increment`), `nl_transient.rs` (`Integrator::new`, `start`, `restore`, `advance`, `substep`, `strain_energy`,
  `NlTransientState`), `rotating.rs` (`unit_centrifugal_model`, `RotatingPart::new`, `with_unit_prestress`, `check_model`,
  `modes`), `solve.rs` (`solve_static_report`), `model.rs` (`Material::j2`, the opt-ins).
- `crates/rcfd-fem/tests/si_eas_hg8.rs` (helpers, load cases), `si_eas_hg.rs` (`r_nt1_strain_energy_of_a_stabilised_bar`,
  the J2 patch), `sa_follower.rs` (the use of `RotatingPart`).

## Arithmetic done before the code (disclosed)

1. **What the committed modes do to the record (c5).** A J2 Incompatible element has `8 x 13 = 104` material values; with W
   its committed state has `113`. `chunks(13)` of 113 values gives 8 chunks of 13 and one of 9 (the modes). In the first
   increment the committed state is the initial one (104 values) and the zip with the trial chunks stops at 8: no extra
   point. From the second increment on both have 9 chunks, and the ninth (the modes) differs bitwise at every iteration where
   the displacement of the element moved: the record without the remedy D1 counts up to one extra "point" per element per
   iteration (up to `64` on the `4^3` block, `512` on DB4). With D1 the counts are those of S (AS1).
2. **The J2 onset (c1, c2).** In confined uniaxial strain `eps` the deviatoric strain is `eps diag(2/3, -1/3, -1/3)`, the
   deviatoric stress `2 mu` times it and the von Mises stress `sqrt(3/2 s:s) = 2 mu |eps|`. With `2 mu = 2e6`, `sigma_y =
   6e4` is reached at `|eps| = 0.03`: under the punch in the mean at about the third of ten increments of `0.01`, earlier at
   the punch edge where the strain concentrates. So the active set grows over the increments (the record has changes to
   see) and at the end a proper part of the 512 (`4^3`) and 4096 (DB4) points has yielded. JY requires at least one plastic
   point (`p[12] > 0`, the equivalent plastic strain of `mat_j2`, as `si_eas_hg.rs` R-PT4 reads it) for W and F-bar.
3. **Transient numbers (c3).** `E I = 2.98e6 x 0.2^4 / 12 = 397` N m^2, `rho A = 40` kg/m; the first bending frequency
   (Euler-Bernoulli, clamped-free, `1.8751^2 = 3.516`, cited from memory) `omega_1 = 3.516 sqrt(397 / 40) = 11.08` rad/s
   (period `0.567` s). `alpha_R = 2 zeta omega_1` with `zeta = 1` (critical damping of the first mode): `22.16` 1/s. The linear
   static deflection `P L^3 / (3 E I) = 200 / 1191 = 0.168` (moderately large; the solid's shear and the coarse mesh make it
   smaller). The ramp over `0.5` s and the hold of `0.5` s (`omega_1 t = 5.5`): with critical damping the dynamic deviation at
   the end of the ramp, of the order `2 zeta / (omega_1 T_r) = 0.36` of the static value, decays by `(1 + 5.5) e^-5.5 =
   0.027`, so `d` at `t = 1` is within about one per cent of the static deflection for both elements, and their difference is
   that of their static stiffness. ADR 0636 PC1 measured `e_P = 0.0567` (displacement-driven reaction) on this mesh; under a
   force the deflection error is about `e_P / (1 + e_P) = 0.054`, and the strain energy (`P d / 2` at static equilibrium)
   the same. Estimate `e_d`, `e_W` about `0.05` to `0.06` against `0.15`. 50 steps at 40 elements: seconds.
4. **The J2 errors (c4).** Not probed. On the elastic block ADR 0636 PB1 measured `e_R = 0.121`, `e_w = 0.013` (`4^3`) and ADR
   0660 WB4 `0.075`, `0.029` (DB4). With J2 the deviatoric response softens after yield while the coefficient of rule L is
   computed from the elastic rest stress (`rest_moduli` takes the elastic base, `rule_coefficient` the rest stress of the
   centre strain): the stabilisation is relatively stiffer in the plastic zone, which may raise `R_L`; plastic flow is
   isochoric, so both elements see the same near-incompressible constraint as in the elastic case. Estimate: `e_R` of the
   elastic order (`0.08` to `0.14` on `4^3`, `0.05` to `0.10` on DB4), with a real risk of a miss on `4^3`; `e_w` below
   `0.05`. This is the honest state of knowledge, not a margin.
5. **JL0 with a history material.** The committed modes are the root of the mode equations at the committed displacement
   with the material history of the start of the last increment (`state_n`); a record started from them with the committed
   material state `state_{n+1}` evaluates the J2 return at the same strain from the returned state. Its elastic trial stress
   equals the returned stress up to rounding (`D (eps - eps_p,n+1)` against the radial-return formula), a relative change of
   about `1e-15`, so the mode residual stays near `1e-15 scale`, far below the test `1e-12 scale` (ADR 0608 D2): **one
   iterate** for every element, as LW0 of ADR 0660. A count other than one says the stored modes are not the converged modes.
6. **Same root, same record (AS1).** W and S converge to the same root of each element to `1e-12` of its scale (ADR 0660
   Addendum 1: W against S within `3.4e-14`); the global iterates differ in the last bits; a point's yield decision changes
   only if its trial von Mises stress is within about `1e-13` relative of the yield stress at some iteration. Expected: equal
   records (every entry of `yielded`, `set_changed`, and the number of increments and iterations). Registered as exact; the
   risk is a point that sits at the yield surface to rounding.
7. **NCE.** The cold local iteration (S from `alpha = 0`) at the committed displacement converges to the same root by a
   different path, so its modes differ from the committed ones in the last bits for some elements (ADR 0660 arithmetic 8,
   NCW1 `252` bit differences): `> 0` elements. That shows TE1 discriminates the cold energy record (ME1).
8. **Cost.** `4^3` J2: three solves (F-bar, W, S) of 375 unknowns, under a few seconds; DB4 J2: three solves of 2187 unknowns
   and 4096 J2 points, about 6 s each for the neo-Hookean in ADR 0660, J2 perhaps twice: 20 to 60 s; transient: three runs (F-bar,
   W, default L) plus the restore steps, seconds; rotating: two `RotatingPart` builds with four modes on 40 elements, seconds.

## Design

### D1. The active-set record with the committed modes (`nonlinear.rs`, `yielded_points`)

`yielded_points` takes `modes_after: bool` (the model's `incompatible_mode_warm_start()`, passed at its one call site in
`attempt_with`). With it, the committed and the trial values of each element are cut to their first `points x values_per_point`
values (the material part; for an element without the modes the whole) before the chunks are compared. Without it the
operations are those of c10a3519. Only the record reads it; no solve value depends on it.

### D2. The energy record from the committed modes (`nl_transient.rs`, `nl_incompatible.rs`)

- `nl_incompatible::modes_and_strains_warm(coords, material, displacement, state, context, stabilised, update)`
  (`pub(crate)`): `warm_state` splits the state into the material part and the committed modes, if present; the local
  iteration starts from the modes and, if that fails (any error) or there are none, runs from `alpha = 0` (the start rule of
  ADR 0660 D1); returns the converged modes and the strains as `modes_and_strains_opt`.
- `Integrator::strain_energy`: with the model's warm start, an Incompatible element takes its modes and strains from
  `modes_and_strains_warm` with the committed state `s.states[e]`; without it, the call of c10a3519 (an empty state). The rest
  (the stabilisation energy at the modes, the point energies) is unchanged. The mode computation is one private helper used by
  both `strain_energy` and the new check accessor.
- `Integrator::strain_energy_modes()` (for checks): per element the modes at which the energy record evaluates it (`None` for
  an element that is not an Incompatible Hex8).

### D3. The restore of a warm state (`nl_transient.rs`, `Integrator::restore`)

With the warm start an Incompatible element's committed state has `k + 9` values (`k` its layout length); `restore` refuses it
today (the layout check), so `state()` and `restore()` do not round-trip on the transient path with W. With the model's warm
start, `restore` accepts for an Incompatible element `k` or `k + 9` values; without it, the check of c10a3519.

### D4. The rotating path: no change

The copy of `unit_centrifugal_model` already keeps the opt-in (ADR 0660 D1); nothing else reads it on that path (reading c9).

### D5. The test file `crates/rcfd-fem/tests/si_eas_hg9.rs`

Helpers copied from `si_eas_hg8.rs`.

- `j2_w_on_the_punch_block`: `4^3`, F-bar, W, S; JB1, JY, AS1, AS2, JL0, JL2, NCA; S against W and SL of W reported.
- `j2_w_on_the_distorted_block_db4`: M0, M1, F-bar, W, S on DB4; JB2, JY, AS2, JL0, JL2; AS1 on DB4 reported (S may fail
  there, ADR 0658).
- `w_on_the_transient_cantilever`: F-bar, W, default L; TT1, TT2, TE1, TE2, NCE, TS1, NCR; the default L against W reported.
- `rotating_path_with_w_inert`: RO1.

## Checks

Every measure is required finite before it is compared (a non-finite value or a failed solve misses).

| Id | Clause | Measure | Limit | Estimate |
|---|---|---|---|---|
| JB1 | c1, c4 | W, J2, block `4^3`: `e_R`, `e_w` against F-bar | `<= 0.15` each | arithmetic 4: `0.08` to `0.14`; below `0.05` |
| JB2 | c2, c4 | W, J2, DB4: `e_R`, `e_w` against F-bar | `<= 0.10` each | arithmetic 4: `0.05` to `0.10`; below `0.05` |
| M0, M1 | c2 | DB4: smallest corner `det J` over centre `det J`; moved nodes | `> 0`; `343` | ADR 0660 (`0.0762`, `343`) |
| JY | c1, c2 | plastic points (`p[12] > 0`) at the end, W and F-bar, each block | `> 0` each | arithmetic 2 |
| AS1 | c5 | block `4^3`: entries of W's active-set record (over all increments: `yielded`, `set_changed`, and the counts of increments and of iterations per increment) that differ from S's | `0` | arithmetic 6 |
| AS2 | c5 | each block: the largest `yielded` entry of W over the number of material points (`8 x elements`) | `<= 1` | AS1 |
| JL0 | c1, c2, c7 | each block, at W's result: elements whose warm record (update S) from the committed modes, with the committed material state, at the result's displacement, has another number of iterates than one (or fails) | `0` | arithmetic 5 |
| JL2 | c1, c2 | each block: elements whose committed state of W does not have `104 + 9` values | `0` | D1 of ADR 0660 |
| NCA | c5 | block `4^3`: elements of W whose committed state read in chunks of 13 gives another number of chunks than 8 | `= 64` (all) | arithmetic 1: the hazard D1 removes is present |
| TT1 | c3, c4 | transient: `e_d`, `e_W` of W against F-bar at `t = 1` | `<= 0.15` each | arithmetic 3: about `0.05` |
| TT2 | c3 | transient: elements whose committed state of W does not have 9 values at `t = 1` | `0` | D1 of ADR 0660 (`state_size` 0) |
| TE1 | c3, c6 | transient W at `t = 1`: elements whose energy-record modes (`strain_energy_modes`) differ in any bit from the last nine committed values | `0` | reading c3, c6 |
| TE2 | c6 | transient W at `t = 1`: `strain_energy` against the test's own energy at the committed modes (`F = I + H_c + F_m` of `point_gradients`, the neo-Hookean energy `mat_hyper::energy` of its Green-Lagrange strain at the stiffness points with their weights, plus `stabilisation_energy_with` at the committed modes), relative | `<= 1e-10` | rounding, about `1e-14` |
| NCE | c6 | transient W at `t = 1`: elements whose cold record (update S, `alpha = 0`) at the committed displacement differs in a bit from the committed modes | `> 0` | arithmetic 7 |
| TS1 | c3 | transient W: after 25 steps `state()`, through `to_bytes` and `from_bytes`; one step; `restore`; the same step again: bit differences of `u`, `v`, `a` between the two | restore succeeds; `0` | D3 (the same operations from the same bits) |
| NCR | c8 | the default-L integrator of the same set-up: `restore` of W's state (with modes) | refused | D3: the default check unchanged |
| RO1 | c9 | `RotatingPart::new` (four modes) of the cantilever of c3 with the isotropic elastic material of c1 (`E = 2.98e6`, `nu = 0.49`, density `1000`), rule L, axis `z` through the origin: W's opt-ins against none: bit differences of the frequencies at `0` and at `omega_1 = 11.08` rad/s | `0`; frequencies finite and positive | reading c9 |
| B6 | c8 | the seven bit records of reading c8 | pass | |
| S5 | c8 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | |
| S6 | | `cargo fmt -p rcfd-fem --check`, `cargo clippy -p rcfd-fem --all-targets -- -D warnings` | clean | |
| S7 | c8 | `cargo test -p rcfd-fem --release --lib`; `--test si_eas_hg8`, `si_eas_hg7`, `si_eas_hg6`, `si_eas_hg5`, `si_eas_hg3`, `si_hex8x`, `si_eas`, `si_eas_hg`, `si_eas_hg2`, `sa_j2_order`, `sa_j2_unload`, `sa_nl_transient` | pass | |

Reported without a limit: W against S on both blocks (`|a_W / a_S - 1|`), S's outcome on DB4 and AS1 there if S converges;
the SL of W (negative eigenvalues of `K_ff`, lowest over the largest diagonal) on both blocks; the plastic point counts; the
increments, halvings and iterations of every solve; the default L against W on the transient; the transient values of all
three elements; the energy record's cold modes against the committed ones (the largest relative difference).

### Negative controls

| Id | Control | Expected |
|---|---|---|
| NCA | the naive chunk reading of W's committed states (checked above) | 9 chunks per element: the record would read a further point without D1 |
| NCE | the cold record at the committed state (checked above) | differs in bits: TE1 sees whether the energy record started from the committed modes |
| NCR | the default integrator refuses an extended state (checked above) | D3 acts only with the opt-in |
| S on DB4 | reported | ADR 0658: S passes DB4 with the neo-Hookean material; with J2 not known |

### Planned mutations (on a scratch copy, each run once against the default tests of `si_eas_hg9.rs`)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MA1 | `yielded_points` ignores `modes_after` (no cut) | AS1 (W counts more than S from the second increment), possibly AS2 |
| MA2 | the cut one point short (`(points - 1) x values_per_point`) | AS1 (W counts fewer than S) |
| ME1 | the energy record ignores the warm start (the cold call) | TE1 (per NCE) |
| MR1 | `restore` without D3 (the check of before also with the opt-in) | TS1 |
| MR2 | `restore` accepts `k + 9` whatever the opt-in | NCR |
| MJ1 | the driver ignores the warm start (ADR 0660 MW1: `false` at both call sites) | JL2, TT2, TE1 |

## Decision rule

- **done**: JB1, JB2, M0, M1, JY, AS1, AS2, JL0, JL2, NCA, TT1, TT2, TE1, TE2, NCE, TS1, NCR, RO1, B6, S5, S6, S7 met in
  their single registered run; every mutation MA1, MA2, ME1, MR1, MR2, MJ1 caught by a check of `si_eas_hg9.rs`; no amendment.
- **partial**: B6, S5, NCR met, but another check missed, or a mutation not caught, or a check met only after an amendment.
- **todo**: B6, S5 or NCR fails (a default bit changed), or W cannot be run with J2 or on the transient path.

## Limits (not covered)

- One J2 material (one yield stress and hardening, isotropic hardening only), one load level, two meshes (the regular `4^3`
  block and DB4); no cantilever with J2; no statistics.
- J2 on the transient path: the energy record refuses a history material (ADR 0479 D3), so the transient set-up is
  hyperelastic; W with J2 on the transient path is not run.
- Creep and viscoelastic materials with W: not run. `creep_increment` (`nonlinear.rs`, the time-stepping path of ADR 0489)
  reads the states in chunks of `mat_creep::STATE_SIZE = 7` and indexes the last value of each chunk; with W the nine modes
  make chunks of 7 and 2 values, and the index of the short chunk is out of range (a panic). Found by reading, not run; a
  follow-up below.
- The arc-length and contact paths with W, the energy-momentum step with an Incompatible element, and W with the line
  search: not run.
- The rotating path is covered only by RO1 (the opt-in is inert there because the path is linear); a nonlinear rotating
  pre-stress does not exist in the repository.
- The default stays cold (ADR 0660 open question 2).

## Open questions (taken or open)

1. **The design of D1 to D3** (the cut of the record to the material part behind the opt-in, the energy record from the
   committed modes with the start rule of the driver, the extended restore behind the opt-in, the accessor for checks):
   taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).
2. **Default.** W stays opt-in (ADR 0660 open question 2): open for the maintainer, not taken.
