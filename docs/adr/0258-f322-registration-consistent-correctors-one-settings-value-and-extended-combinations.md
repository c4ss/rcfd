# ADR 0258: F-322 registration: the consistent correctors as one settings value, one validation with located refusals, and three extended combinations

Status: **REGISTERED 2026-11-12**, written before any code of F-322 and before any run of its checks. Not yet accepted by the
maintainer. Before this text no check was run: the code and the documents listed under "What was read" were read, and the
unchanged tree of main was compiled once (`cargo test --release -p rcfd-fv --no-run` in the target directory
`E:/projects/cfd/w49i-work/target-f322`, to fill it; nothing was executed). Results go in the addendum at the end; nothing above
the addendum is changed after a run.

Parents: [ADR 0214](0214-f299-registration-piso-instability-at-large-cell-diffusion-number-across-a-thin-gap.md) (F-299: the
consistent correctors F0 and their refusals R1), [ADR 0223](0223-f301-registration-consistent-piso-correctors-moving-mesh-lag-estimate-case-key-and-benchmarks.md)
(F-301: the extension flags `consistent_periodic` and `consistent_body_force`, the case table, the steady-identity set-ups and
their amendments, questions Q3 and Q5 whose recommendations are this item),
[ADR 0050](0050-wave27b-robustness-adaptive-stepping.md) (the non-orthogonal part of the steady SIMPLEC compensation),
[ADR 0102](0102-wave41b-simplec-boundary-compensation-forward-vent.md) (the boundary compensation), ADR 0069 F3 (the component
diagonal of the transient kernel). Roadmap item F-322 (depends on F-301, merged).

Number: the lead gave 0258 to F-322. Branch `f322-consistent-correctors` from main 2e0eefd0, worktree `w49i-work/f322`.

House rules (the campaign's `AGENT_RULES.md`, ADR 0214's and ADR 0223's):

- limits are fixed here, before the code and the runs, and are not changed after a run;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact and the number recorded, and the cause is
  diagnosed, not tuned away; a defective set-up gets a corrected test by an amendment written and committed before the corrected
  run, and the old one stays as an ignored record;
- each registered run is made once; existing tests and ignored records are not edited;
- defaults stay bit-identical; `baseline_hashes.rs`, the hash tables and the goldens are not edited or re-recorded;
- no new third-party crate, no unsafe code, no commit trailers, no push.

Scope of this round (the lead's brief): the **library** part. `crates/rcfd-fv/src/pimple.rs` and `pimple/moving.rs` are this
item's; the case-tree files (`crates/rcfd-api/src/sim.rs`, `schema.rs`, `dynmesh.rs`, `comp.rs`, `docs/case-schema.md`) and the
overset files (`crates/rcfd-fv/src/overset.rs`, `crates/rcfd-mesh/src/overset.rs`) belong to other work of this round and are
not edited. The case-key part is therefore the remaining step, specified under "Not delivered".

Clean-room: everything below is derived from the discrete equations of the solver as read in the repository and from the
SIMPLEC split (Van Doormaal and Raithby, 1984, cited from memory; what is used, `rAtU = V/(a_P - H1)` and the compensation of the
old pressure, is derived in ADR 0214 F0 and re-derived here for each new term). No source outside the repository was read.

## What F-322 asks

Title: "Consistent correctors: fold the flags into one key and extend the refused combinations". Acceptance: "The optional table
carries all consistent-corrector flags in one place with a located refusal for each unsupported combination; defaults
bit-identical." Note: from ADR 0223 questions 3 and 4, recommendation taken.

**Reading of the acceptance (registered here).** Three clauses:

- **(A) one place.** At library level: one settings value that carries every consistent-corrector flag (V1). At case level: the
  optional table `[solver.consistent_correctors]` carrying the same flags. The case level is not delivered in this round (the
  files are another agent's), so (A) can be met only in its library half.
- **(B) a located refusal for each unsupported combination.** One function returns, for a combination, which flag, with which
  other setting, and why (V2); the solver refuses through it; every refusal of today still fires, with a message at least as
  specific.
- **(C) defaults bit-identical.** A run that does not ask for the new capability produces the bytes it produced before.

The title's "extend the refused combinations" (ADR 0223 Q5: non-orthogonal correctors, then the component diagonal and face
jumps, each with its steady identity check; the fractional step stays refused, it has no correctors) is done one at a time (X1 to
X3), each kept only if its steady identity and its no-harm check are met. Nothing is extended on a moving mesh (F-321 needs
the maintainer).

## What was read (no run)

`docs/roadmap.json` (F-299, F-301, F-321, F-322); ADR 0214, ADR 0223 (with its three amendments and the addendum), ADR 0050 part
1, ADR 0102; `crates/rcfd-fv/src/pimple.rs` (the settings, `consistent_conflict`, the per-outer check of hook sources,
`step_with`, `lag_map`, `add_consistent_h1`, `consistent_rau`, `consistent_split`, `Divergence::message`), `pimple/moving.rs`,
`pressure.rs` (`solve_pressure_inner`, `corrected_flux_impl`, `coupled_gamma`, `pressure_bc`), `diffusion.rs`
(`laplacian_impl`), `lumped.rs` (`FaceJump`), `overset.rs` (its refusal of settings outside the overset path, read only);
`crates/rcfd-api/src/sim.rs` (the mapping of `solver.consistent_correctors.enabled` onto the three flags and its located errors)
and `schema.rs` (the table), read only; the tests `piso_thin_gap_f299.rs` (R1) and `consistent_correctors_f301.rs` (S-B2, S-p3,
R2, N1), `w34c_symmetry.rs`.

What the read shows:

1. **The flags today.** `PimpleSettings::consistent_correctors`, `consistent_periodic`, `consistent_body_force` (three `bool`
   fields, all `false` by default). The case table has one key, `enabled`, which sets all three. `overset.rs` refuses
   `s.consistent_correctors`.
2. **The refusals today** (`consistent_conflict`, before any work, in this order): the fractional step, `non_orth_correctors >
   0`, `component_diagonal`, a body force without `consistent_body_force`, a face jump, periodic faces without
   `consistent_periodic`; and per outer iteration after the hook: a face jump, else a body force without the flag. Each stops the
   run through the divergence guard with `quantity = "consistent correctors"`, `stage` = the conflicting setting, the state
   unchanged, and the message `PIMPLE consistent correctors (ADR 0214) are not defined with the <stage>: the run is stopped at
   step <n>`. ADR 0214's R1 and ADR 0223's R2 assert the stage, the quantity, the unchanged state and "ADR 0214" in the message.
3. **Non-orthogonal correctors** (`solve_pressure_inner`, `corrected_flux_impl`, `laplacian_impl`, no gradient options). The
   pressure equation of a pass is assembled with the face coefficient `gamma` and the explicit term of the pressure `p_in` the
   pass starts from: on an internal face `gamma_f k_f . (w g_o + (1 - w) g_n)`, `g` the least-squares gradient of `p_in`, `k_f =
   S_f - d_f |S_f|^2 / (S_f . d_f)`; on a fixed-pressure boundary face (`a = 0`) the equation also has `gamma_f k_f . g_owner`.
   The flux correction subtracts `gamma_f (lap_f (p_n - p_o) + k_f . g_f(p_new))` on internal faces and has **no**
   non-orthogonal part on a fixed-pressure boundary face (ADR 0204 Q3: the default's flux and equation differ there).
4. **The component diagonal**: `HbyA_k = (H_k + (a_P - a_k) u_k)/a_P` with `a_P` the component average; nothing else changes.
5. **A face jump** (`step_with`): with `G = gamma_f lap_f`, rise `r` and resistance `R` of the face, the default has
   `phiHbyA_f <- (phiHbyA_f + G r)/(1 + G R)` and the face coefficient `gamma_f/(1 + G R)`, which is the solution of
   `phi = phiHbyA - G ((p_n - p_o) - (r - R phi))`; the cell gradient of the predictor and of the velocity update is the
   Green-Gauss gradient plus `gradient_fix(totals(phi))`, which removes the jump `r - R phi` from the face interpolation.
6. **The moving-mesh step** sets `self.ale` and calls `step`, so every refusal of `step_with` also acts there; `moving.rs` holds
   no consistent-corrector code.

## Design

### V1. One settings value

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ConsistentCorrectors {
    pub enabled: bool,            // ADR 0214 F0
    pub periodic: bool,           // ADR 0223 E2
    pub body_force: bool,         // ADR 0223 E3
    pub non_orthogonal: bool,     // X1
    pub component_diagonal: bool, // X2
    pub face_jump: bool,          // X3
}
```

with `ConsistentCorrectors::all()` (every flag that is supported at the end of this item set) and a new field
`PimpleSettings::consistent: ConsistentCorrectors` (default: all `false`). The three existing `bool` fields **stay as they are**
and are read together with the value: the solver uses only `PimpleSettings::consistent_flags()`, which is the value with
`enabled`, `periodic` and `body_force` each OR-ed with its old field. `PimpleSettings::set_consistent(c)` stores the value and
also writes the three old fields from it.

*Why kept and OR-ed, not removed or replaced (decided here).* (i) `sim.rs` and `overset.rs` set and read the old fields and are
not this round's to edit, and the registered tests of ADR 0214 and ADR 0223 set them and are not edited: every existing caller
compiles and behaves as before, bit for bit (N1, N2). (ii) OR-ing means there is no state in which an old caller's request is
lost. (iii) `set_consistent` writes the old fields so that code that reads only the old `consistent_correctors` (the overset
refusal) still sees a request made through the value. A value whose `enabled` is set directly on the field `consistent`, without
`set_consistent`, is not seen by `overset.rs`: that one line is listed for the lead ("Not delivered"). The new flags exist only
in the value. Removing the old fields is a later step once the case tree sets the value (Q1).

### V2. One validation with located refusals

```rust
pub struct ConsistentContext {       // what the combination is checked against
    pub fractional: bool,            // settings.fractional.is_some()
    pub non_orth_correctors: usize,
    pub gradient_pressure: bool,     // settings.gradient_pressure.is_some()
    pub component_diagonal: bool,
    pub body_force: bool,            // body_force or the hook's source_force
    pub face_jump: bool,
    pub periodic_faces: bool,        // a coupled pressure face exists
    pub moving_mesh: bool,           // an ALE or sliding step
}
impl ConsistentCorrectors {
    pub fn conflicts(&self, ctx: &ConsistentContext) -> Vec<ConsistentRefusal>;      // every conflict, in the order below
    pub fn validate(&self, ctx: &ConsistentContext) -> Result<(), ConsistentRefusal>; // the first one
}
```

`ConsistentRefusal` is an enum; each variant gives `stage()` (the `&'static str` the divergence record carries; the six existing
strings are kept), `flag()` (the flag of the value that is missing, or `enabled` when no flag can allow it), `setting()` (the
other setting, as the library names it), `reason()` and `message()`. With `enabled` off there is no conflict. Order and content
(the first six stages are today's, in today's order):

| # | Variant | Fires when (with `enabled`) | Stage | Flag | Other setting |
| --- | --- | --- | --- | --- | --- |
| 1 | `FractionalStep` | `fractional` | `fractional step` | `enabled` | `PimpleSettings::fractional` |
| 2 | `NonOrthogonal` | `non_orth_correctors > 0`, no `non_orthogonal` | `non-orthogonal correctors` | `non_orthogonal` | `PimpleSettings::non_orth_correctors` |
| 3 | `NonOrthogonalGradientOptions` | `non_orth_correctors > 0`, `non_orthogonal`, `gradient_pressure` | `non-orthogonal correctors with pressure gradient options` | `non_orthogonal` | `PimpleSettings::gradient_pressure` |
| 4 | `NonOrthogonalPeriodic` | `non_orth_correctors > 0`, `non_orthogonal`, `periodic_faces` | `non-orthogonal correctors with periodic faces` | `non_orthogonal` | periodic pressure faces |
| 5 | `NonOrthogonalMovingMesh` | `non_orth_correctors > 0`, `non_orthogonal`, `moving_mesh` | `non-orthogonal correctors on a moving mesh` | `non_orthogonal` | the moving-mesh step |
| 6 | `ComponentDiagonal` | `component_diagonal`, no flag `component_diagonal` | `component diagonal` | `component_diagonal` | `PimpleSettings::component_diagonal` |
| 7 | `ComponentDiagonalPeriodic` | both, `periodic_faces` | `component diagonal with periodic faces` | `component_diagonal` | periodic pressure faces |
| 8 | `ComponentDiagonalMovingMesh` | both, `moving_mesh` | `component diagonal on a moving mesh` | `component_diagonal` | the moving-mesh step |
| 9 | `BodyForce` | `body_force`, no flag `body_force` | `body force` | `body_force` | `PimpleSolver::body_force` or the hook's `source_force` |
| 10 | `FaceJump` | `face_jump`, no flag `face_jump` | `face jump` | `face_jump` | `PimpleSolver::face_jump` or the hook's `face_jump` |
| 11 | `FaceJumpPeriodic` | both, `periodic_faces` | `face jump with periodic faces` | `face_jump` | periodic pressure faces |
| 12 | `FaceJumpMovingMesh` | both, `moving_mesh` | `face jump on a moving mesh` | `face_jump` | the moving-mesh step |
| 13 | `PeriodicFaces` | `periodic_faces`, no flag `periodic` | `periodic faces` | `periodic` | periodic pressure faces |

Reasons (the text of `reason()`): 1: the fractional step has no pressure correctors; 2, 6, 9, 10, 13: the extension is not
asked for, with the flag to set; 3: the compensation of X1 is derived for the plain non-orthogonal flux only; 4, 7, 11: no steady
identity is registered for the extension with periodic faces (the periodic identity of ADR 0223 is itself a known miss in
pressure, S1p3); 5, 8, 12: nothing is extended on a moving mesh (F-321).

**Use by the solver.** `step_with` builds the context once before any work (the periodic search as today, only when it can
matter) and refuses with the first conflict; per outer iteration, after the hook, it takes the conflicts again and refuses with
one that names a source the hook can supply (variants 9 to 12), a face-jump one first (today's order). The divergence record is
unchanged (`quantity = "consistent correctors"`, `stage = refusal.stage()`), so R1 and R2 of the earlier ADRs hold unedited.
`Divergence::message` finds the refusal by its stage and reads `PIMPLE consistent correctors (ADR 0214) are not defined with
the <stage>: the run is stopped at step <n><outer> (ConsistentCorrectors::<flag> with <setting>: <reason>; ADR 0258)`: today's
text, unchanged, followed by the located part.

### X1. Non-orthogonal correctors (flag `non_orthogonal`; fixed mesh, no gradient options, no periodic faces)

Notation of one corrector: `g = interp(rAU)`, `gt = interp(rAtU)`, `dg = gt - g`, `p*` the pressure that drove it, `O_f(p) =
lap_f (p_n - p_o)`, `N_f(p) = k_f . (w g_o(p) + (1 - w) g_n(p))` with the least-squares gradient (item 3). ADR 0214 F0 adds `dg_f
O_f(p*)` to `phiHbyA` and solves and corrects with `gt`. With non-orthogonal correctors the flux correction subtracts `gt_f (O_f
+ N_f)(p_new)`, so at `p_new = p*` the flux is `phiHbyA - g O(p*) - gt N(p*)`: the default's flux minus `dg N(p*)`, the excess
of ADR 0050 item 4. **Built:** the same compensation for the non-orthogonal part, `phiHbyA_f += dg_f N_f(p*)` on every internal
face. Then at `p_new = p*` the flux is the default's, and the pressure equation of a pass, `sum_f gt_f (O_f(p) + N_f(p_in)) = div
(phiHbyA + dg (O + N)(p*))`, has `p = p_in = p*` as its solution whenever the default's has (`gt = g + dg`, everything linear in
the coefficient).

*The fixed-pressure boundary face.* The equation there carries `gt_f k_f . g_owner(p_in)` and the flux carries nothing (item
3). For the equation to keep the default's solution its right-hand side needs `dg_f k_f . g_owner(p*)` on that face; for the
flux to stay the default's it must not be in the flux. **Built:** that term is added to the flux divergence of the pressure
equation only (`phiHbyA` as passed to the pressure solve), not to the `phiHbyA` passed to the flux correction. Fixed-velocity
and periodic faces have no non-orthogonal part in either.

The cell velocity is untouched (its gradient is Green-Gauss, already compensated by F0). The pressure-lag map (`lag_map`) gets the
same two terms, so the estimate has the step's algebra (ADR 0214 F3).

Refused with the flag: pressure gradient options (their limited gradients, face correction and boundary treatments are other
operators), periodic faces, a moving mesh (table rows 3 to 5).

### X2. The component diagonal (flag `component_diagonal`; fixed mesh, no periodic faces)

With the component diagonal the cell equation of component `k` is `a_P u_k = H_k + (a_P - a_k) u_k - V grad_k p`, `a_P` the
component average: `u = HbyA - rAU grad p` with the same scalar `rAU = V/a_P` as without it. F0's split adds `-(rAU - rAtU)
grad(p*)` to `HbyA` and uses `rAtU` for the new gradient, and adds `dg O(p*)` to the flux; both cancel at `p_new = p*` whatever
`HbyA` is. So the steady identity needs **no new term**; `rAtU = V/(a_P - H1)` keeps the component averages `a_P` and `H1`
(a per-component `rAtU` would need a tensor coefficient in the pressure equation; not built). **Built:** the refusal is lifted
under the flag. What the flag does not claim: that the damping of ADR 0214 M1 is as strong per component as with equal
diagonals (not measured here).

### X3. Face jumps (flag `face_jump`; fixed mesh, no periodic faces)

The default's face flux on a jump face is `phi = phiHbyA - G (D - T)`, `D = p_n - p_o`, `T = r - R phi` the total rise (item 5).
The consistent form compensates the whole driving difference `D - T`: `phi = phiHbyA - G (D* - T*) - Gt ((D - T) - (D* - T*))`,
`Gt = gt_f lap_f`, starred = of the pressure and the flux `phi*` that go with it (the solver's flux when the corrector starts).
Solved for `phi`:

`phi (1 + Gt R) = phiHbyA + (Gt - G)(D* - r + R phi*) + Gt r - Gt D`.

At `p = p*`, `phi = phi*`: `phi (1 + G R) = phiHbyA + G r - G D*`, the default's fixed point. **Built:** on a jump face, after
F0's `(Gt - G) D*`, `phiHbyA_f += (Gt - G)(R phi*_f - r)`; then the default's two statements with `Gt` in place of `G`
(`phiHbyA_f <- (phiHbyA_f + Gt r)/(1 + Gt R)`, face coefficient `gt_f/(1 + Gt R)`). With X1 the non-orthogonal compensation `dg
N(p*)` enters the same numerator and the identity holds the same way. In the cells F0's term `-(rAU - rAtU) grad(p*)` must use
the gradient the predictor used, so **built:** `HbyA -= (rAU - rAtU) gradient_fix(totals(phi*))` in addition. The lag map
ignores face jumps, as it does for the default (not changed).

### Questions settled by taking the recommendation

Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation):

- ADR 0223 Q3 (one flag): one value carrying the flags, the old fields kept (V1). Folding the old fields away would change the
  record R1 of ADR 0214, which is not edited here.
- ADR 0223 Q5 (order): non-orthogonal correctors, the component diagonal, face jumps; the fractional step stays refused.
- The new flags stay separate flags inside the value (not implied by `enabled`), because R1 and R2 assert that `enabled` with
  the old flags refuses these combinations; the case table is to set them (the remaining step).

## Set-ups

Common: the grid **G0** `box_2d_graded(20, 12, 0.3, 0.1, [Uniform, Symmetric(3.0)], false, false)` (ADR 0223's SPC grid, not
periodic); `nu = 0.01`; ADR 0211's `fb_settings` (piso, central, tight solvers, AMG pressure; two correctors); `dt = 0.002`;
`U0 = 0.1`. **Duct conditions**: `xmin` fixed velocity `(U0, 0, 0)`; `xmax` fixed pressure 0 (velocity zero gradient); `ymin`
fixed velocity zero; `ymax` fixed velocity zero unless stated; from rest. Reynolds number `U0 H / nu = 1`; viscous time `H^2/nu =
1 s`.

- **DN** (non-orthogonal): the grid `warp(G0, (x, y, z) -> (x + 0.5 y, y, z))` (a shear: on every internal face and on the
  inlet and outlet faces `|k_f|/|S_f| = 0.5`, 26.6 degrees; by hand: a face between `x` neighbours has `S = S_x (1, -0.5)`, `d`
  along `x`, `k = S - d |S|^2/(S . d) = S_x (-0.25, -0.5)`, `|k|/|S| = 0.559/1.118`; a face between `y` neighbours has `S = (0,
  A)`, `d = dy (0.5, 1)`, `k = (-0.5 A, 0)`); duct conditions; `non_orth_correctors = 2`.
- **DD** (component diagonal): G0; duct conditions with `ymax` a `Symmetry` plane; `component_diagonal = true`.
- **DJ** (face jump): G0; duct conditions; a jump on the internal faces whose centre has `x = 0.15` (within 1e-9) and `y <
  0.05`: rise `R0 sign(S_f . e_x)`, `R0 = 0.2 m^2/s^2`, resistance `0.2 R0/(U0 |S_f|)`; no jump on the other faces. (The duct's
  viscous pressure drop is about `12 nu U0 L/H^2 = 0.36`, so the jump is of the size of the flow's own pressure differences; at
  the mean flux `U0 |S_f|` the resistance takes 0.2 R0.) Set as the solver's `face_jump`; variant **DJh**: the same jump from
  the hook in every outer iteration.
- **DX** (all together): the grid of DN; duct conditions with `ymax` a `Symmetry` plane; `non_orth_correctors = 2`,
  `component_diagonal = true`; the jump of DJ on the faces whose centre has `x - 0.5 y = 0.15` (within 1e-9) and `y < 0.05`,
  with `sign(S_f . e_x)`; the body force `BodyForce::uniform(geom, (0.5, -1, 0))`.

The option **C** of a set-up: `consistent` with `enabled` and the flag(s) the set-up needs (DN: `non_orthogonal`; DD:
`component_diagonal`; DJ: `face_jump`; DX: these three and `body_force`). **D** is the default (the value all `false`).

**The steady identity** (ADR 0223 amendment 1's form): 3000 default steps from rest give `S0`; branch A (default) and branch B
(C), two solvers restored from `S0`, advance side by side for 50 steps.

**The transient** (no-harm): from rest, D and C, at `dt_1 = 0.002` (50 steps) and `dt_2 = 0.001` (100 steps), both to `t* =
0.1`; `e(dt) = max_cells |u_C - u_D|` at `t*`.

## Registered checks

Test files (new): `crates/rcfd-fv/tests/consistent_value_f322.rs` (V, R, N), `consistent_extensions_f322.rs` (S, T, L). Release
profile. Existing test files are not edited.

### V, R: the value and the refusals (clauses A and B)

| Id | Check | Limit |
| --- | --- | --- |
| V1 | `PimpleSettings::piso` and `pimple`: `consistent == ConsistentCorrectors::default()`, every flag `false`, the three old fields `false`; `consistent_flags()` after setting only the old fields equals the value with those three; `set_consistent(c)` then `consistent_flags() == c` and the old fields equal `c`'s | exact |
| V2 | `conflicts` as a pure function: for each of the 13 rows a context that produces it first, with stage, flag and setting as in the table; with `enabled` off: none for any context; `all()` on a fixed mesh without periodic faces, gradient options and the fractional step: none with every other setting present; the order of several conflicts is the table's | exact |
| R3 | on ADR 0214's 2-D box (8 x 6, one `step` or `step_ale`): every row of the table produced through the solver (rows 5, 8, 12 through `step_ale` with the same geometry and zero mesh flux; rows 9 and 10 also from the hook) | stopped at step 1; `quantity = "consistent correctors"`; the stage of the row; `u`, `p`, `phi` bit-identical to before; the message contains "ADR 0214", the old sentence `are not defined with the <stage>`, `ConsistentCorrectors::<flag>` and the setting of the row |
| R4 | the same box, accepted: `non_orth_correctors = 1` with `non_orthogonal`; `component_diagonal` with its flag; a face jump with `face_jump` (static and from the hook); the periodic box and a body force through the value alone (`periodic`, `body_force`, old fields `false`) | one step, not stopped |
| R1, R2 (existing, unedited) | `piso_thin_gap_f299` `r1_refusals_of_the_consistent_correctors`, `consistent_correctors_f301` `r2_refusals_with_the_extension_flags` | pass |

### S, T, L: the extensions

Limits of the identity are ADR 0223's (S-B2 measured 7.8e-12, 6.6e-12 and 6.1e-12 against them on the same grid and settings).

| Id | Check | Limit |
| --- | --- | --- |
| S0N, S0D, S0J, S0X (set-up) | branch A over the 50 steps | `max|u - u_S0| <= 2e-11 U0` |
| SNs (set-up) | DN: `|k_f|/|S_f|` over the internal faces | minimum `>= 0.4` |
| SDs (set-up) | DD from rest, one default step with `component_diagonal` `true` and one with `false` | `max|u_true - u_false| >= 1e-6 U0` (the diagonals differ) |
| SJs (set-up) | DJ at `S0`: the mean over the jump faces of the total rise `|r - R phi|` and of its resistance part `|R phi|` | `>= 0.3 R0` and `>= 0.05 R0` |
| SE (set-up, each of DN, DD, DJ, DX) | 5 steps from rest, C against D | `max|u_C - u_D| >= 1e-6 U0` (the option acts in a transient, so the identity is not empty) |
| **S1N** | DN, B against A after every step | `max|u_B - u_A| <= 1e-10 U0`; `max|p_B - p_A| <= 1e-10 (max p_S0 - min p_S0)` |
| **S1D** | DD, the same | the same |
| **S1J**, **S1Jh** | DJ, the same; DJh: branch B' takes the jump from the hook | the same |
| **S1X** | DX, the same | the same |
| TN, TD, TJ (no-harm, the set-up's own transient) | the transient of DN, DD, DJ | no guard trip in any arm; C's largest continuity error over the steps `<= max(1e-10, 2 x D's)`; `e(dt_2) <= 0.05 U0`; `e(dt_1)/e(dt_2) >= 1.3` |
| TI (no-harm, existing transient cases) | ADR 0223's SBC with `A = 10` (body force) and SPC (periodic), 20 steps from rest each, three arms: (i) the old fields (`consistent_correctors` and the set-up's old flag); (ii) the value alone with the same flags; (iii) the value with the same flags and `non_orthogonal`, `component_diagonal`, `face_jump` | `u`, `p`, `phi` of (ii) and of (iii) bit-identical to (i) |
| LN | DN restored to `S0`, C, one step with `pressure_lag_check` (`Warn`, this step, 20 iterations), and the same step without the check | one estimate, finite; `u`, `p`, `phi` bit-identical between the two (the estimate does not change the step) |

Estimates behind the limits. S0, S1: above. SE and SDs: the consistent corrector changes the first corrector's pressure by a
factor of about the cell diffusion number (ADR 0214 M1; here `nu dt/dy^2` about 1 at the walls), far above 1e-6. SJs: the total
rise is `R0 - 0.2 R0 phi/(U0 |S|)`; with a local flux between 0 and 3 times the mean it lies between 0.4 R0 and R0, its
resistance part above 0.05 R0 once the local flux exceeds a quarter of the mean. T: D and C are two splittings of the same
implicit Euler step and both converge to the semi-discrete solution as `dt -> 0`, each with a first-order splitting error, so
`e` is expected to halve with `dt` (ratio about 2; 1.3 leaves room for the impulsive start); `e(dt_2)` is expected at the per
cent level or below (ADR 0223's benchmark set: the option changed St and cd by at most 0.6 %). Continuity: with tight solvers
the orthogonal cases hold 1e-10 (ADR 0214 A3's limit); with non-orthogonal correctors the flux uses the gradient of the new
pressure and the equation that of the pass's start, so the default itself is not at the floor in a transient, hence the
comparison with D.

### N: the defaults (clause C)

| Id | Check | Limit |
| --- | --- | --- |
| N1 | the existing rcfd-fv test files that pin transient, PISO and PIMPLE results, unedited, release: the library unit tests of `rcfd-fv`; `piso_thin_gap_f299`, `consistent_correctors_f301`, `consistent_defaults_f301`, `consistent_benchmarks_f301`, `w33b_default_bits`, `w33b_stability`, `w33b_transient_order`, `w34b_lag`, `w34c_symmetry`, `w48v_corrector_control`, `ale_flows`, `ale_gcl`, `sixdof_coupled`, `sliding_interface_f250`, `overset_f134` (those of these names that exist; the ignored records stay ignored) | all pass |
| N2 | `cargo test -p rcfd-api --release --test baseline_hashes --test case_hashes_w48u --test w48v_case_hashes` | all pass, no file edited |
| N3 | `rcfd-api` `consistent_correctors_case_f301` and `consistent_defaults_case_f301` (the case key through the unedited `sim.rs`: C1 is bit-identity of the case path with the library option) | pass |
| N4 | `consistent_value_f322`: the box of R3 with a body force, periodic in x, 3 steps, default settings, against the same with the value's five extension flags set and `enabled` off | `u`, `p`, `phi` bit-identical (the flags without `enabled` are inert) |

### Mutations (scratch copy in `w49i-work/f322-scratch`, not committed)

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU1 | X1 without the internal-face term `dg N(p*)` | S1N |
| MU2 | X1 without the fixed-pressure boundary term in the equation | S1N |
| MU3 | X3 with the default's `G` on the jump faces (no `Gt`, no `(Gt - G)(R phi* - r)`) | S1J |
| MU4 | X3 without the cell term `gradient_fix(totals(phi*))` | S1J |
| MU5 | row 5 of the table removed (non-orthogonal correctors accepted on a moving mesh) | R3 |
| MU6 | `consistent_flags()` ignores the value's `periodic` | R4 or TI |
| MU7 | the message without the located part | R3 |
| MU8 | row 6 fires also with the flag (the component diagonal always refused) | R4, S1D |

### Classes

Fast (well under two minutes in release, not ignored): V1, V2, R3, R4, N4, TI, LN. The S and T tests make 3000 + 100 steps (S)
and 300 steps (T) on 240 cells; they are expected to take seconds each and are gated to release (`cfg_attr(debug_assertions,
ignore)`), as ADR 0223's S-B2.

## Predictions (mine, before any run)

S1N, S1D, S1J, S1X: differences of a few 1e-12, as S-B2. T ratios between 1.7 and 2.2. The least certain: S0N and S0X (whether the
default with two non-orthogonal passes reaches a state that is steady to 2e-11 U0 on a 26.6 degree shear), and the T ratio of DJ.

## Decision rule

- An extension (X1, X2, X3) is **kept** when its set-up checks hold and its S1 and T checks are met (X3: S1J and S1Jh). An
  extension whose check is missed, with the set-up sound, **stays refused**: its flag stays in the value, `conflicts` refuses the
  combination whatever the flag, `all()` does not set it, and the miss is an ignored `*_known_miss` test.
- S1X covers the extensions together (with a body force). If it is missed with the set-up sound, combinations of two or more of
  the new flags (and of a new flag with a body force) are refused by a further row, and S1X is the known miss.
- **done** needs clauses A, B and C whole. Clause A's case half is not in this round, so the outcome is at most **partial**:
  partial when V1, V2, R3, R4, N1 to N4 hold (the library half of A, B, C); **todo** if any of those fails.

## Limits of this registration

Not covered: the case table (below); any of the extensions on a moving mesh, with periodic faces, or (X1) with pressure gradient
options; the fractional step; a per-component `rAtU` (X2); face jumps in the pressure-lag map; the size of the stabilising effect
of the option with each extension (only the identity and a transient comparison are checked); BDF2 with the extensions (the
set-ups are implicit Euler); three-dimensional meshes (the set-ups are two-dimensional; the algebra has no dimension in it).

## Not delivered (the remaining steps, for after the case-tree branch is merged)

1. **The case table.** `[solver.consistent_correctors]` (optional, `hash_omit_default`, as today) gets, beside `enabled`, the keys
   `periodic`, `body_force`, `non_orthogonal`, `component_diagonal`, `face_jump` (booleans). To keep every existing case
   bit-identical their defaults must reproduce today's mapping (`enabled = true` sets `periodic` and `body_force`): either the
   keys `periodic` and `body_force` default to `true` and the three new ones to `false`, or one key selects the set. This is a
   schema change (the golden and `docs/case-schema.md`), hence the maintainer's (Q2).
2. **`sim.rs`**: build the value from the table with `PimpleSettings::set_consistent`, and replace the hand-written chain of
   located errors by `ConsistentCorrectors::conflicts` with a `ConsistentContext` made from the case (`fractional`,
   `non_orthogonal_correctors`, the pressure gradient options, `component_diagonal`, body forces, lumped components, periodic
   patches, `dynamic_mesh`), reporting each refusal at `solver.consistent_correctors.<flag>`.
3. **`overset.rs`** (one line): the refusal reads `s.consistent_correctors`; it should read `s.consistent_flags().enabled`.
4. The moving-mesh rows (F-321) and the periodic rows.

## Open questions

- **Q1.** Remove the three old fields once the case tree sets the value? (It edits the tests of ADR 0214 and ADR 0223, which
  set them.)
- **Q2.** The case table's keys and their defaults (step 1 above).
- **Q3.** The periodic rows (4, 7, 11): a periodic steady identity with a pressure Dirichlet condition, or acceptance on the
  velocity part as ADR 0223 Q6 recommends.

Recommendations are given in the addendum with the results.

## Addendum 1: results (2026-11-12)

Nothing above this line was changed after a run. No amendment was needed: every registered run was made once and no set-up was
defective.

### What was built

All in `crates/rcfd-fv/src/pimple.rs` (`pimple/moving.rs` is unchanged: the moving-mesh step goes through `step_with`, where
the refusals act):

- V1: `ConsistentCorrectors` (six flags, `all()`), the field `PimpleSettings::consistent`, `consistent_flags()` (the value OR-ed
  with the three older fields) and `set_consistent()` (stores the value and writes the older fields). The older fields are kept.
- V2: `ConsistentContext`, `ConsistentRefusal` (13 variants; `stage`, `flag`, `setting`, `reason`, `message`, `from_stage`,
  `ALL`, `Display`), `ConsistentCorrectors::conflicts` and `validate`. `consistent_conflict` and the per-outer check of hook
  sources go through them; `Divergence::message` appends the located part to the unchanged sentence of ADR 0214.
- X1: `consistent_non_orth` (the internal-face term in `phiHbyA`, the fixed-pressure boundary term in the flux divergence of the
  pressure equation only), in the step and in `lag_map`. X2: the refusal lifted under the flag, no new term. X3: the jump-face
  compensation with `interp(rAtU)` and the cell term with `gradient_fix`.

Lines changed in `pimple.rs`, in the numbering of this tree (new file): 126-130 (the field), 138-421 (the new types), 779-784
(the message), 848 and 853-872 (default and the two methods), 1239-1268 (`consistent_context`, `consistent_conflict`), 1293 and
1300 (the refusal before any work), 1331-1332 (`cf`), 1407-1423 (the per-outer check), 1489, 1593-1594, 1665 (`cf` in place of
the older fields), 1720-1721 and 1725-1740 (X1 and the cell term of X3 after `consistent_split`), 1752-1762 (the jump face),
1769-1777 (`phi_eq`, `phi_solve`), 1802-1805 (the two `solve_pressure` calls take `phi_solve`), 1967, 1989, 2011-2012, 2025,
2047-2052 (`lag_map`), 2441-2484 (`consistent_non_orth`). In the numbering of main 2e0eefd0, near `gamma_p` (old line 1341) and
the pressure solve: old 1344 (the `r_t` line), insertions after old 1398, 1401, 1412 and 1418, and old 1443-1446 (the argument
`&phi_hby_a` of `solve_pressure` and `solve_pressure_src` became `phi_solve`). The line of `gamma_p` itself and the
`gradient_pressure` branch of the solve are untouched.

### Checks

Release profile, target directory `w49i-work/target-f322`, code of commit f63ff42a. Each run made once.

| Id | Limit | Measured | Met |
| --- | --- | --- | --- |
| V1 | exact | pass | yes |
| V2 | exact, 13 rows, order | pass | yes |
| R3 | every row through the solver, located message, state unchanged | 16 cases (13 rows; rows 9, 10, 11 also from the hook), all as registered | yes |
| R4 | accepted combinations, one step, not stopped | 8 cases, none stopped | yes |
| R1, R2 (existing) | pass, unedited | `r1_refusals_of_the_consistent_correctors`, `r2_refusals_with_the_extension_flags` pass | yes |
| SNs | `>= 0.4` | 0.5000 | yes |
| SDs | `>= 1e-6 U0` | 1.838e-1 U0 | yes |
| SJs | `>= 0.3 R0`, `>= 0.05 R0` | 0.7901 R0, 0.2099 R0 | yes |
| SE (N, D, J, X) | `>= 1e-6 U0` | 8.94e-3, 7.93e-3, 9.03e-3, 1.04e-2 U0 | yes |
| S0N, S0D, S0J, S0X | `<= 2e-11 U0` | 3.05e-12, 1.76e-12, 3.02e-12, 1.73e-12 U0 | yes |
| **S1N** | `1e-10 U0`, `1e-10` of the range | 3.71e-12 U0, 5.05e-11 | yes |
| **S1D** | the same | 3.75e-12 U0, 3.24e-11 | yes |
| **S1J** | the same | 2.51e-12 U0, 2.50e-11 | yes |
| **S1Jh** | the same | 2.51e-12 U0, 2.50e-11 | yes |
| **S1X** | the same | 2.96e-12 U0, 4.22e-11 | yes |
| TN | no trip; continuity `<= max(1e-10, 2 x D's)`; `e(dt_2) <= 0.05 U0`; ratio `>= 1.3` | no trip; C 6.85e-3, D 6.75e-3; 2.42e-5 U0; 3.49 | yes |
| TD | the same | no trip; C 1.59e-13, D 1.00e-13; 7.93e-5 U0; 3.34 | yes |
| TJ | the same | no trip; C 1.45e-13, D 1.06e-13; 5.56e-6 U0; 5.94 | yes |
| TI | bit-identical, SBC and SPC, arms (ii) and (iii) against (i) | bit-identical (and (i) differs from the default) | yes |
| LN | one finite estimate, the step bit-identical | one estimate, rho 0.306; bit-identical | yes |
| N1 | all pass | `rcfd-fv` lib 100 passed (5 ignored); `ale_flows` 5 (5 ignored); `ale_gcl` 4; `consistent_benchmarks_f301` 0 (9 ignored records); `consistent_correctors_f301` 12 (4 ignored); `consistent_defaults_f301` 6; `overset_f134` 12 (2 ignored); `piso_thin_gap_f299` 18 (2 ignored); `sixdof_coupled` 2 (1 ignored); `sliding_interface_f250` 14 (4 ignored); `w33b_default_bits` 1; `w33b_stability` 2 (9 ignored); `w33b_transient_order` 6 (9 ignored); `w34b_lag` 3 (3 ignored); `w34c_symmetry` 9 (4 ignored); `w48v_corrector_control` 5; 0 failed | yes |
| N2 | all pass, no file edited | `baseline_hashes` 3 passed (1 ignored); `case_hashes_w48u` 1 passed; `w48v_case_hashes` 0 passed, 1 ignored (its only test is ignored in the tree, so it shows nothing) | yes |
| N3 | pass | `consistent_correctors_case_f301` 4 passed (8 ignored); `consistent_defaults_case_f301` 3 passed | yes |
| N4 | bit-identical | bit-identical | yes |

Printed, no limit: the two default arms of the transients differ between `dt_1` and `dt_2` by 3.56e-3 (N), 8.67e-4 (D) and
5.29e-4 U0 (J), so the option's difference to the default is 5 to 100 times smaller than the default's own step-size error.

### Mutations (scratch copy `w49i-work/f322-scratch/mut`, target `target-f322-mut`, not committed)

| Id | Caught by | Number |
| --- | --- | --- |
| MU1 | S1N | 7.95e-2 U0, 2.62e-1 of the range |
| MU2 | S1N | 3.31e-3 U0, 3.80e-3 of the range |
| MU3 | S1J, S1Jh | 1.29e-1 U0, 1.85e-1 of the range |
| MU4 | S1J, S1Jh | 3.00e-1 U0, 1.15e-1 of the range |
| MU5 | R3 | the moving-mesh row not refused |
| MU6 | R4 and TI | the periodic box refused |
| MU7 | R3 | the located part missing |
| MU8 | R4 and S1D | stopped |

### Deviations and disclosures

- My prediction of the transient ratios (1.7 to 2.2) was wrong: measured 3.3 to 5.9, so the difference between the option and the
  default falls faster than first order in `dt` on these set-ups. The limit (>= 1.3) is met; the ratio is not explained here.
- TN's continuity error is 6.8e-3 in both arms: the default's own, from the explicit non-orthogonal pass (the flux uses the
  gradient of the new pressure, the equation that of the pass's start). The registered limit compares C with D for that reason.
- X2 has no algebra of its own; S1D and TD check that the existing split holds with the component diagonal, MU8 only that the
  flag is wired.
- The N1 list is the registered one; `varden_transient*`, `transient_buoyancy`, `w34b_floor`, `w35a_seam` and the rest of the
  crate's tests were not run (the lead runs the gate).
- `cargo fmt -p rcfd-fv --check` clean; `cargo clippy -p rcfd-fv --all-targets -- -D warnings` clean. No topic document was
  updated: the one that describes the subject, `docs/case-schema.md`, is another agent's in this round.

### Not run

The whole workspace suite (the gate is the lead's); anything through the case tree with the new flags (no case key yet); the
extensions with BDF2, in three dimensions, or on the falling-block mesh of ADR 0211.

### Decision by the registered rule

X1, X2 and X3 are **kept** (set-up checks held; S1N, S1D, S1J, S1Jh, TN, TD, TJ met). S1X met, so no combination row is added.
V1, V2, R3, R4 and N1 to N4 hold: the library half of clauses A, B and C. Clause A's case half is not delivered, so F-322 is
**partial**.

### Open questions (each with options and a recommendation)

- **Q1. The older fields.** (a) keep them beside the value; (b) remove them once `sim.rs` sets the value, with an amendment to
  ADR 0214 and ADR 0223 for the tests that set them. Recommendation: (b), in the case-key step, because two ways to set one flag
  is one too many; until then (a).
- **Q2. The case table.** (a) five further boolean keys, `periodic` and `body_force` defaulting to `true` and the three new ones
  to `false` (every existing case keeps its hash and its result); (b) the three new keys default to `true` as well (`enabled`
  then means "everything supported"; no existing result changes, because those cases are refused today, but an enabled case with
  non-orthogonal correctors would start to run instead of failing); (c) keep one key. Recommendation: (a). It needs the schema
  golden and `docs/case-schema.md`, so the maintainer's approval.
- **Q3. Periodic faces with the new flags** (rows 4, 7, 11). (a) leave refused; (b) register a periodic identity on the velocity
  part with the mean-removed pressure against the default's own drift (ADR 0223 Q6). Recommendation: (b) as a follow-up item.
- **Q4. The transient ratio** above 2: (a) leave; (b) diagnose in the follow-up of Q3. Recommendation: (a), it is on the safe
  side.

### For the lead: roadmap entries

F-322: status **partial**. Note:

> PARTIAL (ADR 0258, 2026-11-12), the library half. Delivered: one settings value `ConsistentCorrectors` (enabled, periodic, body_force, non_orthogonal, component_diagonal, face_jump) as `PimpleSettings::consistent`, read together with the three older fields, which are kept (`consistent_flags`, `set_consistent`); one validation `ConsistentCorrectors::conflicts` / `validate` returning a located `ConsistentRefusal` (flag, other setting, reason) for each of 13 unsupported combinations, used by the solver before any work and per outer iteration, every earlier refusal kept with its stage and its sentence and the located part appended (R1 of ADR 0214 and R2 of ADR 0223 pass unedited); three combinations extended on fixed meshes without periodic faces, each with a steady identity and a transient check: non-orthogonal correctors (the non-orthogonal part of the compensation as ADR 0050, with the fixed-pressure boundary part in the pressure equation only; S1N 3.7e-12 U0 and 5.1e-11 of the pressure range against 1e-10), the component diagonal (no new term; S1D 3.8e-12, 3.2e-11) and face jumps (S1J and from the hook 2.5e-12, 2.5e-11), all together with a body force (S1X 3.0e-12, 4.2e-11); the option differs from the default in a start-up transient by 2.4e-5, 7.9e-5 and 5.6e-6 U0 at dt 0.001, falling by 3.3 to 5.9 per halving; eight mutations each fail a test. Defaults bit-identical (the rcfd-fv transient files, baseline_hashes, case_hashes_w48u, the F-301 case tests). Missing (why partial): the case table still has only `enabled` (schema golden and docs/case-schema.md: the maintainer's), sim.rs still writes its own located errors instead of calling `conflicts`, overset.rs reads the older field. Still refused: the new flags with periodic faces or on a moving mesh (F-321), non-orthogonal correctors with pressure gradient options, the fractional step.

Follow-up items proposed:

- Title: "Consistent correctors: the case table carries every flag". Deps: F-322. Acceptance:

> `[solver.consistent_correctors]` has the keys enabled, periodic, body_force, non_orthogonal, component_diagonal and face_jump with defaults that leave every existing case's hash and result unchanged; sim.rs sets `PimpleSettings::set_consistent` from it and reports each `ConsistentRefusal` of `ConsistentCorrectors::conflicts` at `solver.consistent_correctors.<flag>`; overset.rs reads `consistent_flags().enabled`; the three older PimpleSettings fields are removed with amendments to ADR 0214 and ADR 0223; the schema golden edit is made only with the maintainer's approval; defaults bit-identical.

- Title: "Consistent correctors: the new combinations with periodic faces". Deps: F-322. Acceptance:

> Non-orthogonal correctors, the component diagonal and face jumps are each accepted with periodic pressure faces under the consistent correctors once a registered periodic steady identity (velocity, and pressure with the mean removed against the default's own drift) is met for it; a combination that misses stays refused with its located message; defaults bit-identical.
