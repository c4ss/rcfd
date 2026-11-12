# ADR 0360: F-367 registration: the consistent correctors' new combinations with periodic faces

Status: **REGISTERED 2026-11-12**, written before any code of F-367 and before any run of its checks. Not yet accepted by the
maintainer. Before this text no test was run: the code and documents under "What was read" were read, and the unchanged tree
of main 2ad9dcb9 was compiled once (`cargo test --release -p rcfd-fv --no-run`, target directory `w49i-work/f367-scratch/target`,
to fill it; nothing was executed). The arithmetic under "Arithmetic before this registration" was done by hand from numbers
already recorded in ADR 0223 and ADR 0258. Results go in the addendum at the end; nothing above the addendum is changed after a
run.

Parents: [ADR 0258](0258-f322-registration-consistent-correctors-one-settings-value-and-extended-combinations.md) (F-322:
`ConsistentCorrectors`, the 13 located refusals, the extensions X1 non-orthogonal correctors, X2 the component diagonal, X3
face jumps; its question Q3, from which this item comes), [ADR 0223](0223-f301-registration-consistent-piso-correctors-moving-mesh-lag-estimate-case-key-and-benchmarks.md)
(F-301: E2, the periodic extension; the periodic channel SPC; amendment 2's mean-removed pressure and the known miss S1p3;
question Q6), [ADR 0214](0214-f299-registration-piso-instability-at-large-cell-diffusion-number-across-a-thin-gap.md) (F0,
the consistent correctors), [ADR 0352](0352-f443-registration-step-consistent-face-coefficient-remaining-paths.md) (F-443: the
step-consistent face coefficient acts with the consistent correctors and is bit-inert on coupled faces, D6 and P1). Roadmap
item F-367 (depends on F-322, merged).

Number 0360, branch `f367-periodic-consistent` from main 2ad9dcb9, worktree `w49i-work/f367`, given by the lead. House rules:
`w49i-work/AGENT_RULES.md` (limits fixed here and not changed after a run; a miss becomes an ignored `*_known_miss` test with
the limit intact; a defective set-up is corrected only by an amendment committed before the corrected run; each registered run
is made once; existing tests and ignored records are not edited; defaults bit-identical; no hash table or golden edited; no new
crate, no unsafe; no push). The overset files (`crates/rcfd-fv/src/overset.rs`, `crates/rcfd-mesh/src/overset.rs`) are another
agent's this round and are not edited.

Clean-room: no new algorithm. Everything below is derived from the discrete equations of the solver as read in the repository
(ADR 0214 F0's SIMPLEC split, ADR 0223 E2's coupled-face compensation, ADR 0258 X1 to X3), re-derived here for the faces where
they meet. The SIMPLEC split (Van Doormaal and Raithby, 1984) is cited from memory as in ADR 0214; no formula is taken from it
that is not derived in those ADRs. No source outside the repository was read.

## What F-367 asks

Acceptance (`docs/roadmap.json`, binding in full): "Non-orthogonal correctors, the component diagonal and face jumps are each
accepted with periodic pressure faces under the consistent correctors once a registered periodic steady identity (velocity, and
pressure with the mean removed against the default's own drift) is met for it; a combination that misses stays refused with its
located message; defaults bit-identical."

Clauses (registered reading):

- **(a)** for each of the three combinations (rows 4, 7 and 11 of ADR 0258 V2: `NonOrthogonalPeriodic`,
  `ComponentDiagonalPeriodic`, `FaceJumpPeriodic`), a periodic steady identity is registered, in velocity and in the
  mean-removed pressure against the default's own drift, and run; where it is met the combination is accepted.
- **(b)** a combination whose identity misses stays refused, with its located message (stage, flag, setting, reason).
- **(c)** defaults bit-identical.

## What was read (no run)

`docs/roadmap.json` (F-322, F-367); ADR 0258 whole (design, checks, addendum, Q3), ADR 0223 (E2, SPC, amendments 1 and 2, the
addendum's S1p3 row and Q6), ADR 0352 (the head, D6, P1, the roadmap text), ADR 0214 F0 by reference through them;
`crates/rcfd-fv/src/pimple.rs` (`PimpleSettings` and its constructor, `ConsistentCorrectors`, `ConsistentContext`,
`ConsistentRefusal`, `consistent_flags`, `consistent_context`, `consistent_conflict`, the refusal before any work and the
per-outer check in `step_with`, the momentum predictor's `PeriodicLinks` and `add_consistent_h1`, the corrector loop with
`consistent_split`, `consistent_non_orth`, the jump faces, `phi_eq` / `phi_solve`), `pressure.rs` (`pressure_bc`,
`boundary_values_pbc`, `coupled_gamma`, `solve_pressure_inner` with its implicit periodic coupling, `corrected_flux_impl`),
`bc.rs` (`Periodic::coeffs`), `diffusion.rs` (`laplacian_impl`, the non-orthogonal part on boundary faces), `lumped.rs`
(`FaceJump::total`, `totals`, `gradient_fix`), `geometry.rs` (`periodic_map`), `rcfd-mesh` `gen.rs` (`warp`); the tests
`consistent_value_f322.rs` (V1, V2, R3, R4, N4), `consistent_extensions_f322.rs` (the duct set-ups, `identity`, `transient`,
TI), `consistent_correctors_f301.rs` (`spc_solver`, `mean_removed`, the S-p3 test).

What the read shows:

1. **The refusals.** `ConsistentCorrectors::conflicts` pushes row 4 when `non_orth_correctors > 0`, the flag
   `non_orthogonal` and periodic faces; row 7 when the component diagonal, its flag and periodic faces; row 11 when a face jump,
   its flag and periodic faces. Reason of the three: "no steady identity is registered for this extension with periodic faces".
   The solver refuses through `consistent_conflict` (before any work) and the per-outer check of hook sources (a face-jump row
   first). Nothing else in the step depends on these rows: with the rows removed, the step takes the code paths that each
   extension and E2 already take on their own.
2. **Coupled faces in the default** (`Periodic::coeffs`: `a = w > 0`; `laplacian_impl` adds a boundary non-orthogonal part only
   where `a == 0`; `corrected_flux_impl` has no non-orthogonal part on a coupled face): the default has **no non-orthogonal
   part on a coupled face**, neither in the pressure equation nor in the flux. The coupled face enters the equation as
   `gamma_c |S| (p_q - p_o)/span` (`gamma_c = coupled_gamma`, the pair mean), implicitly, and the flux the same way.
3. **The face jump** acts on internal faces only: the jump loop of `step_with` and `FaceJump::gradient_fix` run over
   `0..n_internal`; a jump value on a boundary face (coupled or not) is ignored by the default and by X3 alike.
4. **The cell gradients** of the split (`green_gauss` with `boundary_values_pbc`) and of X1 (`least_squares` with
   `boundary_values_pbc`) take on a coupled face the pair-weighted value of owner and partner, the same values as the default's
   predictor gradient, pressure equation and flux correction.
5. **`consistent_non_orth`** skips coupled faces on both its loops (the internal loop does not reach them; the boundary loop
   skips `coupled` and `a != 0`).
6. **The existing registered tests** `consistent_value_f322.rs` `v2_the_validation_names_flag_setting_and_reason` and
   `r3_every_refusal_through_the_solver_is_located` assert that `ConsistentCorrectors::all()` is refused with rows 4, 7 and 11
   (as a pure function and through the solver), and the test literals of `ConsistentCorrectors` and `ConsistentContext` in that
   file name every field. So (i) a new field in either struct breaks the compile of an existing test, and (ii) accepting the
   three rows under the existing flags alone fails two existing tests. Both are edits of existing tests, which the house rules
   forbid (see the design decision below).

## Derivation: the three combinations need no new term

Notation of ADR 0258: `g = interp(rAU)`, `gt = interp(rAtU)`, `dg = gt - g`, `p*` the pressure that drove the corrector,
internal-face operators `O_f(p) = lap_f (p_n - p_o)`, `N_f(p) = k_f . (w g_o(p) + (1 - w) g_n(p))`; coupled-face operator
`C_f(p) = |S_f| (p_q - p_o)/span` with coefficient `coupled_gamma(.)`; the cell gradient `G(p)` (Green-Gauss with the
boundary values of `pbc`).

The default's corrector, at its fixed point `p_new = p*`: face flux `phi = phiHbyA - g (O + N)(p*)` on internal faces (the `N`
part only with non-orthogonal correctors), `phi = phiHbyA - cg(g) C(p*)` on coupled faces, `phiHbyA - g B(p*)` on other
pressure-set boundary faces; cell velocity `HbyA - rAU G(p*)`.

The consistent corrector with E2 and X1 (and X3 on the jump faces) adds, with `p*`: on internal faces `dg O(p*)` (F0) and
`dg N(p*)` (X1); on coupled faces `cg(dg) C(p*)` (E2); on fixed-pressure faces `dg B(p*)` (F0) and the equation-only term of X1;
in the cells `-(rAU - rAtU) G(p*)`; and solves and corrects with `gt`. At `p_new = p*`:

- internal face: `phiHbyA + dg (O + N)(p*) - gt (O + N)(p*) = phiHbyA - g (O + N)(p*)`, the default's;
- coupled face: `phiHbyA + cg(dg) C(p*) - cg(gt) C(p*) = phiHbyA - cg(g) C(p*)`, the default's, because `coupled_gamma` is
  linear in its argument (the pair mean, read 2) and `gt = g + dg` face by face;
- coupled face, non-orthogonal part: none in the default, none in either compensation, none in the option's equation or flux
  (reads 2 and 5): nothing to compensate;
- cell: `HbyA - (rAU - rAtU) G(p*) - rAtU G(p*) = HbyA - rAU G(p*)`, the default's, with the same `G` (read 4);
- the pressure equation of a pass is linear in the coefficient and its explicit non-orthogonal part is taken from the pass's
  start pressure on internal and fixed-pressure faces only; with every compensation above it has `p = p*` as a solution whenever
  the default's has.

So with periodic faces, **X1 needs no term beyond X1 and E2**: their sum is the compensation of every face. **X2** needs none
(ADR 0258 X2: the split cancels whatever `HbyA` is; `HbyA` with the component diagonal changes only `H`, and the coupled rows'
implicit links are counted in `h1` by E2, which changes `rAtU` only, not the fixed point). **X3** needs none: its jump terms
live on internal faces only (read 3), its cell term `(rAU - rAtU) gradient_fix(totals(phi*))` is internal-face data, and the
coupled faces carry E2's term unchanged. The rows were refused only because no identity was registered (their reason text);
F-367 registers one per row.

Nothing changes in `rAtU` either: `add_consistent_h1` counts the periodic links with E2's flag whatever the other flags, and
the identity does not depend on the value of `rAtU` (all the cancellations above hold for any positive `rAtU`).

## Design

**D1. An opt-in that lifts the three rows.** `PimpleSettings::consistent_periodic_extensions: bool` (default `false`). With
it, the combinations whose periodic identity is met (the list `ConsistentRefusal::PERIODIC_EXTENSIONS`, a `&'static
[ConsistentRefusal]`; at the start of the runs all three rows) are not refused. Without it, every refusal is as today.

*Why an opt-in and not the existing flags alone (decided here, on the maintainer's standing instruction of 2026-11-11, take
the recommendation).* Accepting the rows under the flags alone fails two registered tests of ADR 0258 (read 6:
`consistent_value_f322::v2_the_validation_names_flag_setting_and_reason` and
`consistent_value_f322::r3_every_refusal_through_the_solver_is_located`), and a new field of `ConsistentCorrectors` or
`ConsistentContext` breaks the compile of that file. The house rules forbid editing either. A separate setting of
`PimpleSettings` (whose literals are only in `pimple.rs`) changes no existing test and no existing behaviour. Folding it into
the flags later (so that `ConsistentCorrectors::all()` accepts the rows) needs those two tests retired by the lead; that is
question Q1.

**D2. One validation, extended.** `ConsistentCorrectors::conflicts_extended(&self, ctx, periodic_extensions: bool) ->
Vec<ConsistentRefusal>`: `conflicts(ctx)` with, when `periodic_extensions`, the refusals of `PERIODIC_EXTENSIONS` removed (the
order of the others kept); `validate_extended` its first. `conflicts` and `validate` are unchanged (ADR 0258's table, as a pure
function). The solver's two refusal points (`consistent_conflict` before any work and the per-outer check of hook sources) call
`conflicts_extended` with `PimpleSettings::consistent_periodic_extensions`. Nothing else in the step changes: no numerical
line is added or edited.

**D3. The located message.** Rows 4, 7 and 11 keep stage, flag and setting. Their `reason()` names the opt-in for a row in
`PERIODIC_EXTENSIONS` ("accepted only with PimpleSettings::consistent_periodic_extensions, its periodic steady identity is met
(ADR 0360)") and the miss for a row not in it ("its periodic steady identity was missed (ADR 0360)"). The text of the reason
is not asserted by any existing test beyond its presence in the message (R3 reads `reason()`).

**D4. A miss.** A row whose identity misses (with its set-up sound) is removed from `PERIODIC_EXTENSIONS` and stays refused with
or without the opt-in; its identity test becomes an ignored `*_known_miss` record with the limit intact (its body, run again,
would then stop at the refusal; the record carries the measured numbers).

Not changed: the moving-mesh rows 5, 8, 12 (F-321), row 3 (gradient options), row 13 (periodic faces without `periodic`), the
fractional step; the case tree (`rcfd-api` has no key for the new flags, ADR 0258 "Not delivered"); the pressure-lag map
(`lag_map` gets the same refusal-free paths; its estimate with these combinations is not checked here).

## Set-ups

Common: ADR 0223's periodic channel **PC**: `box_2d_graded(20, 12, 0.3, 0.1, [Uniform, Symmetric(3.0)], true, false)`, periodic
in x; `nu = 0.01`; ADR 0211's `fb_settings` (PISO, central, tight solvers 1e-16 / 1e-13, AMG pressure, two correctors); `dt =
0.002`; from rest; no pressure Dirichlet condition (the level is set by the reference-cell pin); `U0 = 0.1`, `V0 = 0.02`, the
wall profile `w(x) = (U0, V0 sin(2 pi x_f / 0.3), 0)` per face (zero net normal flux: a sine sampled at equally spaced points
over one whole period sums to zero).

- **PN** (row 4): the grid of PC sheared, `warp(PC, (x, y, z) -> (x + 0.5 y, y, z))` (ADR 0258 DN's shear; the periodic patches
  stay related by the translation `(0.3, 0, 0)`, so `periodic_map` pairs them); `ymin` fixed velocity zero, `ymax` the profile
  `w`; `non_orth_correctors = 2`. Option: `enabled`, `periodic`, `non_orthogonal`.
- **PD** (row 7): PC's grid; `ymin` the profile `w` (a moving bottom drives the flow), `ymax` a `Symmetry` plane (so the
  component diagonals differ, as ADR 0258 DD); `component_diagonal = true`. Option: `enabled`, `periodic`,
  `component_diagonal`.
- **PJ** (row 11): PC exactly (`ymin` at rest, `ymax` the profile); ADR 0258 DJ's jump on the internal faces whose centre has `x
  = 0.15` (within 1e-9) and `y < 0.05` (6 faces): rise `R0 sign(S_f . e_x)`, `R0 = 0.2 m^2/s^2`, resistance `0.2 R0/(U0
  |S_f|)`. Set as the solver's `face_jump`; variant **PJh**: the same jump from the hook in every outer iteration. Option:
  `enabled`, `periodic`, `face_jump`.
- **PX** (all three): PN's grid; PD's conditions (`ymin` the profile, `ymax` symmetry); `non_orth_correctors = 2`,
  `component_diagonal = true`; the jump of PJ on the faces whose centre has `x - 0.5 y = 0.15` (within 1e-9) and `y < 0.05`.
  Option: the five flags.
- **P0** (calibration, printed only): PC with E2 alone (`enabled`, `periodic`), ADR 0223's S-p3 under this item's pressure
  limit.

Every option **C** above also sets `consistent_periodic_extensions = true` (except P0, which needs none). **D** is the default.

**The periodic steady identity** (ADR 0223 amendment 2's form): 3000 default steps from rest give `S0`; branch A (D) and branch
B (C), two solvers restored from `S0`, advance side by side for 50 steps (PJ: and B', the jump from the hook). `p'` is the
pressure with its volume mean removed (`mean_removed` of `consistent_correctors_f301.rs`, copied). Per step `n`: `dA_u(n) =
max|u_A - u_S0|`, `dA_p(n) = max|p'_A - p'_S0|`, `dBA_u(n) = max|u_B - u_A|`, `dBA_p(n) = max|p'_B - p'_A|`; over the 50 steps
`dA_p = max_n dA_p(n)` (the default's own drift) and so on. `rg = max p_S0 - min p_S0`.

**The transient** (no-harm): from rest, D and C at `dt_1 = 0.002` (50 steps) and `dt_2 = 0.001` (100 steps), both to `t* =
0.1`; `e(dt) = max_cells |u_C - u_D|` at `t*`.

## Registered checks

Test files (new): `crates/rcfd-fv/tests/consistent_periodic_defaults_f367.rs` (NF; the API of 2ad9dcb9 only, committed with
its expected fingerprints before any library edit) and `crates/rcfd-fv/tests/consistent_periodic_f367.rs` (the rest). Release
profile. Existing test files are not edited.

### S: the periodic identities (clause a)

| Id | Check | Limit |
| --- | --- | --- |
| S0x (set-up; x = PN, PD, PJ, PX) | branch A over the 50 steps | `dA_u <= 2e-11 U0` |
| SFx (set-up, floor) | the default's own mean-removed drift | `dA_p <= 1e-8 rg` (so that the pressure limit below is not vacuous) |
| SPsx (set-up) | at `S0`, the largest pressure difference across a periodic pair (owner and partner cells) | `>= 1e-2 rg` (the coupled faces carry a pressure difference, so E2's term acts) |
| SEx (set-up) | 5 steps from rest, C against D | `max|u_C - u_D| >= 1e-6 U0` (the option acts, so the identity is not empty) |
| SNs (set-up, PN) | `|k_f|/|S_f|` over the internal faces | minimum `>= 0.4` |
| SDs (set-up, PD) | from rest, one default step with `component_diagonal` `true` and one with `false` | `max|u_true - u_false| >= 1e-6 U0` |
| SJs (set-up, PJ) | at `S0`, the mean over the jump faces of `|r - R phi|` and of `|R phi|` | `>= 0.3 R0` and `>= 0.01 R0` |
| **S1PN**, **S1PD**, **S1PJ**, **S1PJh**, **S1PX** | B (and B') against A after every step | `dBA_u(n) <= 1e-10 U0` for every `n`; `max_n dBA_p(n) <= max(1e-10 rg, 2 dA_p)` |
| TPN, TPD, TPJ (no-harm) | the transient of PN, PD, PJ | no guard trip in any arm, finite; `e(dt_2) <= 0.05 U0` |

Printed, no limit: P0's numbers under the S1 limits; each branch B's own drift `dB_p = max_n max|p'_B - p'_S0|`; the raw-pressure
difference; the T ratio `e(dt_1)/e(dt_2)` and the continuity errors.

### R: refusal and acceptance (clauses a, b)

On ADR 0214's 2-D box (`box_2d(8, 6, 1, 0.5, periodic, false)`, copied from `consistent_value_f322.rs`), one step:

| Id | Check | Limit |
| --- | --- | --- |
| RP1 | `ConsistentCorrectors::all()` without the opt-in, periodic box: row 4 (`non_orth_correctors = 1`), row 7 (component diagonal), row 11 (static jump and from the hook) | each stopped at step 1, `quantity = "consistent correctors"`, the row's stage, `u`, `p`, `phi` bit-identical to before, the message has "ADR 0214", `are not defined with the <stage>:`, `ConsistentCorrectors::<flag>`, the setting, the reason, "ADR 0258"; the reason of a row in `PERIODIC_EXTENSIONS` names `consistent_periodic_extensions` |
| RP2 | the same with the opt-in: rows 4, 7, 11 (static and hook), and all three together with a body force | not stopped (for the rows in `PERIODIC_EXTENSIONS`; a row not in it: stopped as RP1) |
| RP3 | with the opt-in, what stays refused: on the periodic box through `step_ale` (same geometry, zero mesh flux) rows 5, 8, 12; row 3 (gradient options, periodic box); row 13 (`periodic` off, `enabled` and the other flags on) | stopped at step 1 with the row's stage, state unchanged |
| RP4 | pure functions: for every one of the 2^7 x 2 contexts (`fractional`, `gradient_pressure`, `component_diagonal`, `body_force`, `face_jump`, `periodic_faces`, `moving_mesh` each `false`/`true`; `non_orth_correctors` 0 or 1) and the flag sets `only enabled`, `all()`, `all()` without `periodic`: `conflicts_extended(ctx, false) == conflicts(ctx)`; `conflicts_extended(ctx, true)` equals `conflicts(ctx)` with exactly the members of `PERIODIC_EXTENSIONS` removed, the order kept; `validate_extended` is the first | exact |

### N: the defaults (clause c)

| Id | Check | Limit |
| --- | --- | --- |
| NF | `consistent_periodic_defaults_f367.rs`, written against the API of 2ad9dcb9 and committed with its expected fingerprints (FNV-1a of the bits of `u`, `p`, `phi` after 20 steps) recorded by running it on the unchanged library: F1 PC default; F2 PN default; F3 PD default; F4 PJ default; F5 PC with E2 (the older fields `consistent_correctors`, `consistent_periodic`); F6 ADR 0258's DN duct with X1 (the value: `enabled`, `non_orthogonal`); F7 ADR 0258's DJ duct with X3 | the fingerprints on the final tree equal those recorded on 2ad9dcb9 |
| NO | the opt-in inert where it lifts nothing: (i) without `enabled` (the periodic box with a body force, 3 steps, `all()` with `enabled` off, opt-in on, against the default); (ii) F5's and F6's runs with the opt-in added | `u`, `p`, `phi` bit-identical |
| NE | the existing tests, unedited, release: `rcfd-fv` `--lib`; `consistent_value_f322`, `consistent_extensions_f322`, `consistent_correctors_f301`, `consistent_defaults_f301`, `piso_thin_gap_f299`, `w33b_default_bits`, `w33b_transient_order`, `w34b_lag`, `w48v_corrector_control`, `step_consistent_paths_f443`, `bdf2_remainder_f344`, `seam_no_harm_f344`, `sliding_interface_f250`, `ale_flows`, `ale_gcl` (the ignored records stay ignored) | all pass |
| NH | `cargo test --release -p rcfd-api --test baseline_hashes --test case_hashes_w48u` | pass, no file edited |

### Mutations (scratch copy in `w49i-work/f367-scratch`, not committed)

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU1 | E2's coupled-face compensation dropped in `consistent_split` | S1PN, S1PD or S1PJ (each is expected to) |
| MU2 | X1's internal-face term `dg N(p*)` dropped | S1PN |
| MU3 | X1's internal-face term also applied on coupled faces (owner gradient, `k_f` of the coupled face) | S1PN |
| MU4 | X3's `(Gt - G)(R phi* - r)` dropped | S1PJ, S1PJh |
| MU5 | X3's cell term `gradient_fix(totals(phi*))` dropped | S1PJ |
| MU6 | the cell term of `consistent_split` skipped in the owner cells of coupled faces | S1PD |
| MU7 | the per-outer check ignores the opt-in (calls `conflicts`) | S1PJh or RP2 (hook) |
| MU8 | the opt-in also lifts row 5 | RP3 |
| MU9 | the rows lifted whatever the opt-in | RP1 |

### Classes

Fast (well under two minutes in release, not ignored): RP1 to RP4, NO, NF. The S and T tests make 3000 + 50 (S) and 300 (T)
steps on 240 cells; seconds each in ADR 0258's runs; gated to release with `cfg_attr(debug_assertions, ignore)` as there.

## Arithmetic before this registration

- The pressure limit. If B satisfies the identity, it is another trajectory of the same fixed point whose mean-removed pressure
  wanders at the solvers' floor as A's does. With `|p'_B - p'_S0| <= dB` and `|p'_A - p'_S0| <= dA_p`, `|p'_B - p'_A| <= dB +
  dA_p`; with `dB` of the size of `dA_p`, `<= 2 dA_p` (the triangle inequality, a factor 2 for two independent wanders). The
  floor `1e-10 rg` keeps ADR 0223's limit where the drift is smaller. ADR 0223 S1p3 measured on PC with E2 alone: B against A
  2.49e-10 rg, A's own drift 3.32e-10 rg: under this limit (6.64e-10 rg) it would be met (ratio 0.37 of the limit), so P0 is
  predicted at those numbers (the code paths of P0 are unchanged since).
- SFx: `dA_p` of PC was 3.32e-10 rg; 1e-8 rg is 30 times that. The mutations of ADR 0223 and ADR 0258 moved the identity by
  1e-3 to 3e-1 (in U0 or of the range), so a limit of `2 dA_p` (expected below 1e-9 rg) separates them by six orders.
- SJs: on PC the moving top wall gives a Couette mean `U0/2`; the jump, half the height at `R0`, drives about `(R0/2) H^2 /
  (12 nu L) = 0.1 x 0.01 / 0.036 = 0.028 m/s`. With a local velocity in the lower half between 0.01 and 0.05 m/s the resistance
  part `0.2 R0 u / U0` lies between 0.02 R0 and 0.1 R0 (limit 0.01 R0), the total `R0 - 0.2 R0 u/U0` between 0.9 R0 and R0
  (limit 0.3 R0).
- SNs: `|k|/|S| = 0.5` on every face between x-neighbours and between y-neighbours (ADR 0258 SNs: 0.5000).
- S0x: ADR 0223 S0p 2.09e-12 U0 on PC; ADR 0258 S0N 3.05e-12, S0D 1.76e-12, S0J 3.02e-12 on the ducts (limit 2e-11).
- T: ADR 0258 measured `e(dt_2)` 2.4e-5 to 7.9e-5 U0 on the ducts; the limit 0.05 U0 is ADR 0258's.

## Predictions (mine, before any run)

S1Px velocity a few 1e-12 U0; pressure below `dA_p` (ratio to the limit 0.3 to 0.6), `dA_p` between 1e-10 and 1e-9 rg. Least
certain: SFx of PN and PX (the explicit non-orthogonal passes and the pressure null mode together), and S0PX.

## Decision rule

- A combination (row 4: PN; row 7: PD; row 11: PJ) is **accepted** (kept in `PERIODIC_EXTENSIONS`) when its set-up checks (S0,
  SF, SPs, SE and its own of SNs, SDs, SJs) hold and its S1 (PJ: S1PJ and S1PJh) and T are met. A set-up check missed is a
  defective set-up, corrected only by an amendment registered and committed before the corrected run (the old test an ignored
  record). An S1 or T missed with the set-up sound: the row **stays refused** (D4) and the test is an ignored `*_known_miss`.
- S1PX covers the three together. If it misses with its set-up sound while the singles are met, the opt-in accepts one of the
  three with periodic faces at a time: a second one is refused with its own row (`conflicts_extended` keeps the later rows when
  more than one is present), and S1PX is the known miss.
- **done**: all three accepted, S1PX met, RP1 to RP4, NF, NO, NE, NH pass, and every mutation is caught by a registered check
  of this table without a post-run amendment. **partial** (Rule I): one or two accepted, or any registered check missed, or a
  mutation not caught or caught only after a post-run amendment. **todo**: none accepted.

## Limits of this registration

Not covered: the moving mesh with any of the extensions (F-321); rotational periodicity (the set-ups are translational;
`PeriodicLinks` skips rotational patches, and the identity on them is not run); a face jump on a coupled face (ignored by the
default and by X3 alike, read 3: not a jump the solver supports on either path); BDF2 (implicit Euler set-ups); three
dimensions; the pressure-lag estimate with these combinations; the size of the stabilising effect with periodic faces (only the
identity and a transient comparison); the case tree (no key for the new flags, ADR 0258's remaining step).
