# ADR 0659: F-724 registration: the threshold scale of the coupled interface solves

Status: **REGISTERED 2026-11-16, written before any code of F-724 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-724 was written and no run of any kind was made; the numbers quoted
are those measured and printed in ADR 0638, ADR 0640, ADR 0641 and ADR 0654. Results go in the addenda at the end; nothing
above the addenda is changed after a run.

Roadmap item F-724 (deps F-721, F-710), proposed by [ADR 0641](0641-f721-registration-periodic-sliding-at-coinciding-nodes-without-the-parts-symmetry.md)
remaining question 3 (B1u, MU4). Parents: [ADR 0638](0638-f710-registration-sliding-interface-at-coinciding-nodes.md)
(F-710: `sliding_step` coupled at coincidences, the threshold D3), [ADR 0640](0640-f720-registration-periodic-sliding-at-coinciding-nodes.md)
(F-720: the uniform flows CS and CP of the periodic path), ADR 0641 (F-721: `periodic_sliding_step_coupled`, its Q1 on the
default), [ADR 0654](0654-f730-registration-sector-sliding-in-the-tree-at-node-coincidences.md) (F-730: the opt-in tree key,
its Q4). Number 0659 given by the lead. Branch `f724-work` from main adc8402f (F-710, F-720, F-721 and F-730 merged), worktree
`w49i-work/f724`, scratch `w49i-work/f724-scratch`.

House rules (`w49i-work/AGENT_RULES.md`) apply: limits fixed here; each registered run made once; existing tests not edited;
defaults bit-identical except where the lead's brief allows (below); no edit of `baseline_hashes.rs`, of a hash table or of a
golden; no new dependency, no unsafe code; clean-room; the roadmap files are not edited. Every choice the maintainer would
normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation), except
where listed as open.

Clean-room: the scale is the classical a-priori bound of a floating-point sum, `|fl(sum x_i) - sum x_i| <= gamma_{n-1} sum
|x_i|` with `gamma_n = n u / (1 - n u)` and `u = 2^-53` (the standard error analysis of recursive summation, as in Higham,
"Accuracy and Stability of Numerical Algorithms", ch. 4; cited from memory, and the bound used here is derived below from
the definition of the rounded operations, not taken from the book). No source was read; no vendor code, text or install was
used. The references are the repository's own steps (ADR 0638 and 0641's coupled and base steps) and closed forms (a uniform
field).

## What F-724 asks

The acceptance, binding (from the lead's brief):

> the part-mean threshold of sliding_step and periodic_sliding_step_coupled is scaled by a flux scale of the interface faces rather than by the largest target, registered from a derivation; the coupled step equals the base step bit for bit on every step of the uniform flows of ADR 0638 and ADR 0640 (coincidence steps included), and every result of ADR 0638 and ADR 0641 at coincidences with a non-symmetric flow is kept within registered limits; then periodic_sliding_step is switched to the coupled path with the maintainer's approval (ADR 0640's N1 test becomes an ignored record). Every limit not stated here is registered in the ADR before the run.

Clauses:

- **a1** the threshold of `sliding_step` and of `periodic_sliding_step_coupled` scaled by a flux scale of the interface
  (cells') faces instead of the largest target, from a derivation (this document, D1);
- **a2** the coupled step equals the base step bit for bit on every step of the uniform flows of ADR 0638 and ADR 0640,
  coincidence steps included (reading below);
- **a3** every result of ADR 0638 and ADR 0641 at coincidences with a non-symmetric flow kept within registered limits;
- **a4** then `periodic_sliding_step` switched to the coupled path, with the maintainer's approval, ADR 0640's N1 test
  becoming an ignored record.

The lead's brief adds: **a4 is not made here** (it changes default results and an existing test: rule 3 and rule 6); it is
built up to the switch and reported as the remaining step. `sliding_step` is on a default path: if changing its threshold
changes any default bits on `baseline_hashes` and the named sliding suites, the new scale is made opt-in for it and the
default change is reported for approval instead.

**Reading of a2 (registered; question Q1).** On ADR 0640's uniform flows (CS: S3 with `U = (0, 0, 1)`; CP: PP with `U = (1,
0.5, 0)`) every remap target and every GCL target is rounding (ADR 0640's first-principles argument: the flow is invariant
under the motion), so "the coupled step equals the base step" means `periodic_sliding_step_coupled` equals
`periodic_sliding_step` bit for bit on all 200 and 650 steps: the check ADR 0641 B1u missed. On ADR 0638's uniform flows the
pair P with `U = (1, 0.5, 0)` (UC) is **not** invariant under the rotation: its coincidence steps have a remap part mean of
`2.16e-5` (ADR 0638 diagnosis), which is the defect F-710 fixed, and the uncoupled step loses the uniform state there (ADR
0638 N1, `5.7e-4`). The literal reading (the coupled step equal to the uncoupled one on UC's coincidence steps) would undo
F-710 and contradict a3 and ADR 0638's A1; it cannot be what is asked. On ADR 0638's flows the "base step" is read as the
step of F-710 (`sliding_step` with ADR 0638's threshold): the new `sliding_step` equals it bit for bit on every step of UC,
U2, U3 and LP, coincidence steps included, and equals the uncoupled step wherever ADR 0638 B1/B2 showed F-710 to equal it.
The uncoupled-equality reading for `sliding_step` is checked on the flow where it is meaningful: an axial uniform flow on
the 3-D full annulus P3 at coincidences (E3, the full-annulus analogue of CS).

## What the code read shows (adc8402f, read only)

1. `rcfd_fv::sliding::coupled_balance` (ADR 0638 D2, D3): coupled when `removed_mean > PART_MEAN_TOL * tmax`, `PART_MEAN_TOL
   = 1e-10`, `tmax` the largest `|t_c|` over the owners and neighbours of the interface faces, and coupling faces exist.
   Called for the GCL targets `t` and the remap targets `e` in `sliding_step`. Its unit test T1 calls it directly.
2. `rcfd_fv::sliding_periodic::coupled_solve` (ADR 0641 D2): the same test with `tmax` over the endpoints of the graph's
   edges (direct faces and wrap pairs); only on the path `couple = true` (`periodic_sliding_step_coupled`). Its unit test T1
   calls it directly.
3. The targets: GCL `t_c = (V1_c - V0_c)/dt - sum_{f in faces(c), f not new} sigma_cf m_f`; remap `e_c = -sum_{f not new}
   sigma_cf phi_f - sum_{f new} sigma_cf x_f`, `x_f` the interpolated flux of a new face (`interp_new` on the full annulus,
   `interp_edge` on a direct face or wrap pair of the periodic graph). "New" faces: the interface faces (and the wrap copies).
4. Callers of `periodic_sliding_step_coupled`: the tree's sector loop with `[mesh.sliding.coincidences] coupled = true`
   (F-730, opt-in), F-721's and F-730's tests. Callers of `sliding_step`: the tree's full-annulus loop (default), the tests
   named in the brief, `step_consistent_paths_f443`, `wall_pressure_case_f358` (au2).
5. ADR 0641 amendment 1 (B1u): on CS and CP the coupled step differs from the base on every coincidence step (25, 81), by
   `<= 4.8e-16` (CS) and `<= 3.2e-15` (CP) of `max|S_f|` in the old flux and up to `9.8e-14` in CP's mesh flux: with every
   target at rounding, `tmax` is itself rounding and `1e-10 tmax` lets rounding-level means couple.

## Design

**D1. The scale (derivation).** A computed target is a sum of a few terms; its rounding error, and so a part mean that is
zero in exact arithmetic, is bounded by the sum of the absolute values of its terms, not by the size of the computed targets
(which for an exactly balanced cell is itself rounding). Derivation: each rounded addition `fl(a + b) = (a + b)(1 + d)`,
`|d| <= u`; summing `k` terms recursively, the error is at most `sum_j |partial sum_j| u + O(u^2) <= (k - 1) u sum_i |x_i| +
O(u^2)`. So for the cell `c` with the term magnitude

- GCL: `a_c = (V1_c + V0_c)/dt + sum_{f in faces(c), f not new} |m_f|` (the two volumes are terms of the difference; each
  volume and each swept volume carries its own geometric rounding of a few `u` of itself, which the same sum bounds);
- remap: `a_c = sum_{f in faces(c), f not new} |phi_f| + sum_{f new, at c} |x_f|` (on the periodic graph a wrap pair adds
  `|interp_edge|` at both its cells);

the computed `t_c` (or `e_c`) differs from the exact one by at most `(k_c + 2) u a_c`, `k_c` the number of terms (`<= 12` on
the meshes here: a hexahedron's 6 faces, an interface cell's slivers up to 4 more). The part mean computed by
`graph_balance` (`sum over the part / n_p`) adds at most `n_p u max |t_c| <= n_p u max a_c`. The input fluxes themselves are a
solver's output: the old flux is divergence-free only to the pressure solve's tolerance, a cell divergence of `eps_s a_c`
with `eps_s` the solver's relative tolerance (`1e-13` in every run here, `tol_rel`). Altogether, for a part whose exact
mean is zero:

`removed_mean <= ((k + 2 + n_p) u + eps_s) A`, `A = max_{c in graph} a_c`.

With `k <= 12`, `n_p <= 1e3` (the parts here hold at most a few hundred cells: P has 264 interface cells in one part off coincidences) and `eps_s = 1e-13`: `removed_mean
<= 2.2e-13 A`. The genuine means of the coincidences, from the measured base numbers and hand arithmetic of `A` (below), are
at least `9e-5 A`. The threshold is `PART_MEAN_TOL * max(A, tmax)` with **`PART_MEAN_TOL = 1e-10` kept** (ADR 0638 D3's
value): 2.7 decades above the rounding bound, 5.9 below the smallest genuine mean. Taking `max(A, tmax)` (in exact arithmetic
`|t_c| <= a_c`, so `A >= tmax` up to rounding) makes the new threshold never smaller than ADR 0638's, so the new rule couples
on a subset of the steps the old rule coupled on: it can only return a coupled step to the base, never couple a step the old
rule left alone (property P).

Hand arithmetic of `A` and the margins (from the ADRs' printed numbers, face areas by hand):

| Set-up | genuine or rounding mean | `A` (hand) | `mean / A` |
| --- | --- | --- | --- |
| UC (P, `U = (1, 0.5, 0)`), remap, coincidence | `2.16e-5` (ADR 0638 diagnosis) | `|U| sum |S_f| ~ 1.12 (6.25 + 6.25 + 6.54 + 6.27) 1e-3 = 2.8e-2` | `7.7e-4` |
| NS on SS (ADR 0641), remap, coincidence | `>= 3.98e-4 max|S_f| = 4.2e-6` (the base residual is the part mean, `max|S_f| = 2 (pi/60) 0.1 = 1.05e-2`) | `<= 1.5 (2.9e-2) = 4.4e-2` | `>= 9.5e-5` |
| FS (ADR 0654), remap, coincidence | `>= 1.4e-4 max|S_f|` (0654) | as SS | `>= 3e-5` |
| CS (S3, axial), remap | `<= 2.2e-15 max|S_f| = 6e-17` (the base residual, ADR 0640) | `>= 2 |S_z| = 2 (1.44 (pi/36) 0.125) = 3.1e-2` | `<= 2e-15` |
| CP (PP), remap | `<= 5.3e-15 (6.25e-3) = 3.3e-17` | `>= 2 (6.25e-3) + 2 (0.5)(5e-3) = 1.75e-2` | `<= 2e-15` |
| CP (PP), GCL | `<= 1.1e-14 mean V / dt = 2.2e-15` | `>= 2 V/dt = 2 (3.1e-4)(640) = 0.4` | `<= 6e-15` |

**D2. API.** `crates/rcfd-fv/src/sliding.rs`: a public enum `PartCoupling { Off, LargestTarget, FluxScale }` (Off: the
uncoupled step of ADR 0216; LargestTarget: ADR 0638 D3's rule; FluxScale: D1) and `sliding_step_with(old_geom, old, new,
phi_old, u_old, dt, coupling)`; the body of `sliding_step` moves there; the decision for `LargestTarget` is made by the
unchanged `coupled_balance` (same expression, same operations), for `FluxScale` by a new `coupled_balance_above(.., threshold)`
that `coupled_balance` itself calls (so T1 is untouched and the coupled solve is one code); `Off` never calls them. The term
magnitudes `a_c` are computed only for `FluxScale`, in a separate vector; no target, value or operation of the step changes.
`crates/rcfd-fv/src/sliding_periodic.rs`: `periodic_sliding_step_with(.., coupling)`; `periodic_sliding_step` calls it with
`Off` (the operations of today's `couple = false`), `periodic_sliding_step_coupled` with `FluxScale`; `coupled_solve` keeps
its signature (T1) and calls a new `coupled_solve_above(.., threshold)`. The periodic `PART_MEAN_TOL` stays a copy of the
constant.

**D3. The default of `sliding_step` (conditional, registered).** `sliding_step` is made `sliding_step_with(..,
FluxScale)` **if** FL (below) counts no decision flip on the named suites; otherwise `sliding_step` stays
`sliding_step_with(.., LargestTarget)` (bit-identical to F-710), FluxScale is reachable through `sliding_step_with` only,
and the default switch is reported for approval. By property P a flip can only be a step that F-710 coupled on a rounding
mean (all targets rounding, at a coincidence), whose result changes by rounding. Inputs of that class outside the named
suites (for example E3's axial uniform flow on a 3-D full annulus) do change bits under the new default; that is the purpose
of a1 and is reported plainly for the maintainer either way.

**D4. Not changed.** `periodic_sliding_step` (the base; a4 open), ADR 0640's N1 test, every existing test and record,
`graph_balance`, `interface_balance`, `pimple/`, `rcfd-mesh`, `rcfd-api` (the tree calls the same two public functions; the
opt-in sector loop inherits FluxScale through `periodic_sliding_step_coupled`), goldens, hash tables. New test file
`crates/rcfd-fv/tests/sliding_threshold_scale_f724.rs`; `docs/dynamic-mesh.md` one sentence.

## Set-ups

As registered in the parent ADRs (code copied into the new test file; nothing edited):

- **UC, U2, U3, LP** of ADR 0638 (its `uniform_run`: PS `nu = 0.1`, central, 2 non-orthogonal correctors, tight solvers;
  every non-empty patch fixed at `U`): UC P `U = (1, 0.5, 0)`, `dt = 2 pi / 144 / 8`, 50 steps (coincidences 8, ..., 48);
  U2 P `dt = 0.005`, 100 steps; U3 P3 `U = (1, 0.5, 0.3)`, `dt = 0.01`, 40 steps; LP `U = (1, 0.5, 0.3)`, `dt = 0.01`, 20
  steps. The loop is driven by the default `sliding_step`; on every step `sliding_step_with(.., LargestTarget)` and `(..,
  Off)` are built from the same inputs.
- **CS, CP** of ADR 0640 (ADR 0641's `uniform_compare`, PS6): CS S3 `U = (0, 0, 1)`, `dt = pi/288`, 200 steps; CP PP `U =
  (1, 0.5, 0)`, `dt = 1/640`, 650 steps. Driven by `periodic_sliding_step`; on every step `periodic_sliding_step_coupled` and
  `periodic_sliding_step_with(.., LargestTarget)` from the same inputs.
- **AX** (new, E3): P3 of ADR 0216 (`annulus_3d(48, 4, 2, 1, 1.5, 0.4)` against `annulus_3d(40, 4, 3, 1.5, 2, 0.4)`), `U =
  (0, 0, 1)` fixed on every non-empty patch, uniform start, ADR 0638's `uniform_run` settings, `dt = 2 pi / 48 / 8` (the
  rotor pitch over 8), **48 steps**. Arithmetic: rotor nodes `theta + i 2 pi/48`, stator `j 2 pi/40`, meeting at multiples
  of `2 pi/240`; `n 2 pi/384 = m 2 pi/240` needs `5 n = 8 m`: coincidences at `n = 8, 16, ..., 48` (6 steps), each with
  `gcd(48, 40) = 8` coinciding angles, so **8 parts** (the two blocks' axial layers, 2 and 3, share only `z = 0` and `0.4`,
  so no axial cut); 1 part elsewhere. Driven by the default `sliding_step`; on every step `(.., Off)` and `(..,
  LargestTarget)` from the same inputs.
- **NS** of ADR 0641 (SS, inlet `(1 + 0.5 sin 6 theta) x_hat` on `inner` reset every step, outlet `outer`, start `(r_i/r)
  x_hat`, PS6, `dt = pi/576`, 400 steps; coincidences on the 50 steps `n % 8 == 0`): driven by
  `periodic_sliding_step_coupled`; on every step `periodic_sliding_step_with(.., LargestTarget)` (= ADR 0641's coupled step)
  and `periodic_sliding_step` from the same inputs.
- **PL** of ADR 0641 (PP, PF inlet, `dt = 1/640`, 160 steps): as NS.

## Checks (limits fixed now)

| Id | Clause | Set-up | Check | Limit |
| --- | --- | --- | --- | --- |
| S | a1 | this document | the scale derived and registered before the code (this commit, alone, first) | present |
| E1 | a2 (ADR 0638), a3 | UC, U2, U3, LP | default `sliding_step` against `LargestTarget` (F-710's step): `mesh_flux`, `phi_old`, `old_defect`, `vol_old`, both residuals, interface faces | bit-equal on every step (50 + 100 + 40 + 20) |
| E1u | a3 (ADR 0638 A1, S1 kept) | UC | velocity and pressure deviation every step; `flux_residual`, `gcl_residual` on the 6 coincidence steps | `<= 1e-10`; `<= 1e-12` |
| E2 | a2 (ADR 0640) | CS, CP | `periodic_sliding_step_coupled` against `periodic_sliding_step`, every field as E1 plus wrap pairs, images, reimage | bit-equal on every step (200, 650) |
| E3 | a2 (full annulus, uncoupled reading) | AX | default `sliding_step` against `Off`, as E1; velocity deviation every step | bit-equal on all 48; `<= 1e-10` |
| X3 | set-up of E3 | AX | interface-graph parts (union-find in the test): 8 on exactly the steps `n % 8 == 0`, 1 elsewhere | exact |
| K1 | a3 (ADR 0641) | NS | `periodic_sliding_step_coupled` against `LargestTarget` (ADR 0641's coupled step), as E2 | bit-equal on all 400 |
| K1r | a3 (ADR 0641 A1 kept) | NS | coupled `flux_residual` steps 2 to 400; `gcl_residual` every step | `<= 1e-12` each |
| K2 | a3 (ADR 0641 A2) | PL | as K1; coupled `flux_residual` steps 2 to 160 | bit-equal on all 160; `<= 1e-12` |
| FL | D3 (default bits of `sliding_step`) | the named suites on an instrumented scratch copy (D3; instrumentation: one `eprintln!` where the LargestTarget and the FluxScale decisions differ, in `sliding_step`'s path only) | decision flips | 0 |
| B | a3, defaults | existing suites, unedited, on the worktree code | `rcfd-fv`: lib, `sliding_coincidence_f710`, `sliding_interface_f250`, `sliding_periodic_coincidence_f720`, `sliding_periodic_coupled_f721`, `sliding_periodic_f304`, `sliding_reimaging_f704`, `step_consistent_paths_f443`; `rcfd-api`: `baseline_hashes`, `sliding_coincidence_f710`, `sliding_sector_coincidence_f730`, `sliding_sector_case_f711`, `sliding_case_f303`, `dynamic_mesh`, `wall_pressure_case_f358` (its `au2` test, the only one that calls `sliding_step`) | pass as before (non-ignored tests) |
| B4 | D4 | code | `git diff adc8402f -- crates/` touches `sliding.rs`, `sliding_periodic.rs` and the new test file only | as stated |
| NC1 | control of E2 | CS, CP | `LargestTarget` against `periodic_sliding_step`: steps that differ | `>= 1` on each of CS and CP (ADR 0641: 25 and 81) |
| NC2 | control of E3 | AX | `LargestTarget` against `Off`: steps that differ | `>= 1` |
| NC3 | control of E1, K1, K2 (the coupling acts) | UC; NS; PL | `Off` against the default (UC) / coupled (NS, PL) step: steps that differ | exactly the coincidence steps: 6 of 50; 50 of 400; 20 of 160 |

FL's scope: the non-ignored tests of the `rcfd-fv` files `sliding_coincidence_f710`, `sliding_interface_f250`,
`sliding_periodic_coupled_f721`, `sliding_reimaging_f704`, `step_consistent_paths_f443`, the lib, the new file, and of the
`rcfd-api` files `baseline_hashes`, `sliding_coincidence_f710`, `sliding_sector_coincidence_f730`, `sliding_case_f303`,
`dynamic_mesh`, `wall_pressure_case_f358` (au2). FL is made on the instrumented copy of the committed code, once.

Estimates. E1: property P with the table's margins: on UC the remap's coincidence decisions are coupled under both rules
(`7.7e-4 A` against `1e-10 A`), every other decision is "base" under the old rule and so under the new; on U2, U3 the graph
is connected (no coupling faces), on LP the old rule keeps the base (ADR 0638 B2). E1u: equal to ADR 0638's measured
`8.9e-15`, `5.2e-14`, `1.2e-15`, `1.1e-14` if E1 holds. E2: the table's CS and CP rows, more than four decades below the new
threshold. E3: as CS (the same flow and the same two node sets, on the full annulus): every target rounding, `A >= 2 |S_z|
|U|`. K1, K2: NS's and PL's genuine means about six decades above the threshold, the GCL means rounding by the node sets'
symmetry (ADR 0641) and base under both rules. NC1: ADR 0641 B1u. NC2: E3's flow is CS's flow on the full annulus, where the
old rule's rounding-relative threshold is expected to couple at least once on the 6 coincidence steps; if it never does,
E3 is shown only for the steps it covers and NC2 is reported as not held. NC3: ADR 0638 B1 (6), ADR 0641 B1 (50); PL: 20
coincidence steps (ADR 0641 PL). FL: 0 expected by property P, because every named suite's coincidence steps carry a
flow not invariant under the motion or no flow at a coincidence. Each run is made once; the new file's tests are expected to
take under two minutes together in release (ADR 0641's LS, with three solvers and 400 steps, took 21 s).

Mutations (on a scratch copy of the committed library, after the checks; each applied alone; the named tests run once; each
must fail):

- **MU1** `periodic_sliding_step_coupled`'s threshold scale `A` replaced by `tmax` (ADR 0641's rule): E2 must fail.
- **MU2** the same in `sliding_step`'s FluxScale: E3 must fail.
- **MU3** `PART_MEAN_TOL` of the FluxScale decision raised to `1e-2` (both files): E1 (UC) and K1 must fail.
- **MU4** the remap scale from the new faces' interpolated fluxes only (the interface faces and wrap pairs, the literal
  "interface faces" reading, Q2) in the periodic step: E2 (CS: an axial flow has no flux through a cylindrical interface)
  must fail.
- **MU5** the FluxScale threshold of `sliding_step` set to zero (couple whenever coupling faces exist): E1 (LP) or E3 must
  fail.

Decision rule: **done** is not available while a4 is open (it needs the maintainer's approval). **Partial (all but a4)** when
S, E1, E1u, E2, E3, X3, K1, K1r, K2, FL, B, B4 are met, the controls NC1 to NC3 hold and every mutation is caught; the
report then names a4 as the one remaining step. **Partial** with the named misses when any of E1, E2, E3, K1 or a control or
mutation is missed. **Todo** when E2 is missed (the threshold does not keep the base path on ADR 0640's flows). If FL counts
flips, D3's opt-in branch is taken (by an amendment written before any further run), and the decision says so. A B failure
stops the work and is reported (no existing test is edited).

## Limits (not covered)

- a4 (the switch of `periodic_sliding_step`, ADR 0640's N1 test to an ignored record): not made; needs the maintainer.
- The bound of D1 assumes the solver tolerance `eps_s` of the runs here (`1e-13`). With a looser pressure tolerance the old
  flux's divergence (`eps_s A`) can exceed `1e-10 A` and couple; the coupled solve is then still correct (it closes every
  cell), only not bit-equal to the base. Not checked.
- The GCL branch of the FluxScale rule is exercised by CP (where the old rule coupled the GCL on rounding, ADR 0641's B1u
  diagnosis) and AX; no set-up here has a genuine GCL part mean (T1 of ADR 0638 and 0641 cover the coupled GCL solve).
- FL covers the named suites only; inputs outside them whose old decision was a rounding coupling change by rounding (D3).
- The case tree's opt-in sector loop is not re-run beyond `sliding_sector_coincidence_f730`'s non-ignored tests (its
  lockstep records are ignored heavy records of ADR 0654).

## Open questions taken on the standing instruction

- **Q1. Reading of a2 on ADR 0638's flows.** (a) The new step equals F-710's step bit for bit on UC, U2, U3, LP, and the
  uncoupled step on a flow invariant under the rotation (E3) (taken); (b) the literal reading (equal to the uncoupled step on
  UC's coincidence steps), which would undo F-710. Taken (a) on the maintainer's standing instruction of 2026-11-11 (take the
  recommendation).
- **Q2. "A flux scale of the interface faces".** (a) The term magnitudes of the targets of the interface graph's cells
  (D1: the fluxes of every face of those cells, the interface faces among them) (taken): the rounding of a target is bounded
  by all its terms; (b) the interface faces' own fluxes: for an axial flow on a cylindrical interface those are zero and the
  threshold would again be rounding-relative (MU4 checks this). Taken (a) on the standing instruction.
- **Q3. The constant.** (a) Keep `1e-10` with the new scale (taken; D1's margins: 2.7 decades above the bound, 5.9 below the
  smallest genuine mean); (b) a new constant at the geometric middle (`~1e-8.5`). Taken (a) on the standing instruction (one
  change at a time; no margin is short).

## Open (not taken)

- **Q4. a4: `periodic_sliding_step` to the coupled path** (with ADR 0641 Q1 and ADR 0654 Q4): changes default results of the
  sector and planar paths at coincidence steps for flows without the parts' symmetry (ADR 0640 N1 becomes an ignored record;
  any other test that asserts the base's result at coincidences would need the same review). Needs the maintainer.

## Amendment 1 (2026-11-16, written after the registered run of the new test file and before any further run)

The registered run of `sliding_threshold_scale_f724` (code and tests 86452826, run once, two threads, 172 s, log
`w49i-work/f724-scratch/run.log`) met E1, E1u, E2, E3, X3, K1, K1r and the controls NC1, NC2, NC3 (UC, NS, PL), and missed
K2. One diagnostic run followed (disclosed; scratch test `w49i-work/f724-scratch/zz_f724_diag.rs`, moved out of the worktree
after the run and never committed, log `diag.log`). No limit is changed; the results go in addendum 1.

**K2 missed: an error of the registration's estimate, diagnosed, not corrected.** On PL the coupled step differs from
F-721's on each of the 20 coincidence steps. The diagnosis printed, on those steps, the old flux and the old defect
bit-equal to F-721's, and the mesh flux bit-equal to the uncoupled step's and different from F-721's by at most `1.38e-13
max|S_f|`. Cause: PL is the pair PP, whose interface is flat, so every GCL target is rounding on every step (ADR 0640); at
a coincidence F-721's rule (relative to the largest, rounding-level, target) coupled the GCL closure, the new rule keeps
the base, exactly as this ADR's own table row "CP (PP), GCL" foresees. The estimate of K2 ("the GCL means rounding by the
node sets' symmetry, base under both rules") was carried over from SS and is wrong for PP. The remap (the genuine part
means) is coupled under both rules and identical. ADR 0641's registered limit for PL (A2, the coupled `flux_residual <=
1e-12` from step 2) is met in the same run (`4.44e-15`), so a3 holds for PL within its registered limit; the bit-equality
asked by K2 does not.

**Records.** The test `k2_...` becomes the ignored record `k2_known_miss_...` with its limits intact and the measured
numbers in the ignore text. `k1_...` took 146 s in the registered run (two threads): it becomes an ignored heavy record
(rule 7; its checks were met), the same code path covered fast by E2 and by ADR 0641's `ls_corrected` test (part of B).
No corrected test is added (no set-up was defective). No other change.

## Amendment 2 (2026-11-16, written after the FL run, before the mutations and before any further run)

FL was run once on the instrumented copy of e1ed0432 (`w49i-work/f724-scratch/fl`, script `fl_run.sh`, log `fl.log`). It
counted **6** flips, all `remap old true new false`, all in the new file `sliding_threshold_scale_f724`: the 6 coincidence
steps of AX, the run built to show such a flip (E3 and NC2; D3 names AX as the example of an input whose bits change).
Every existing suite of FL's scope counted **0** (log sections: `rcfd-fv` lib, `sliding_coincidence_f710` (12 coupled
remaps), `sliding_interface_f250`, `sliding_periodic_coupled_f721` (50, the annulus of LS), `sliding_reimaging_f704`,
`step_consistent_paths_f443`; `rcfd-api` `baseline_hashes`, `sliding_coincidence_f710` (5), `sliding_sector_coincidence_f730`,
`sliding_case_f303` (27), `dynamic_mesh`, `wall_pressure_case_f358` au2); no GCL decision was coupled under the new rule in
any of them.

**FL as registered is a defective set-up check:** its scope included the new file, whose AX run flips by design, while D3
asks whether the *named* (existing) suites' default bits change. FL is recorded as missed as registered (6). The corrected
check **FLc**: flips in the existing suites of FL's scope, limit 0, read from the per-suite sections of the same log (the
run is deterministic and each suite is a separate section; no re-run is made, and the numbers are those printed above).
D3's branch is decided on FLc. No other change.

## Addendum 1: results (2026-11-16)

Measured against the limits registered above and in amendments 1 and 2; no limit was changed. Release builds on Windows.
The registration is c77eb228 (alone, first); the code and the tests 86452826 (committed before their run); amendment 1 and
the records e1ed0432; amendment 2 and `docs/dynamic-mesh.md` dbf17c4a. The registered run of the new file (172 s, two
threads) was made once; one diagnostic run followed (amendment 1); FL once on the instrumented copy (amendment 2); B once.

**Built as registered** (D1 to D3). `crates/rcfd-fv/src/sliding.rs` (shared solver file): module text line 18;
`PartCoupling`, `largest_at`, lines 258 to 281; `coupled_balance` now calls the new `coupled_balance_above`, lines 290 to
303; the `sliding_step` doc and body and the new `sliding_step_with`, lines 351 to 374; the FluxScale decisions of M3 and
M4, lines 419 to 437 and 491 to 508. `crates/rcfd-fv/src/sliding_periodic.rs` (shared solver file): module text lines 19
to 20, import line 28; `periodic_sliding_step`, `periodic_sliding_step_coupled` and the new `periodic_sliding_step_with`,
lines 99 to 132; `coupled_solve` calls the new `largest_on` and `coupled_solve_above`, lines 208 to 232; the rule of
`step_impl`, lines 265 to 274, and its FluxScale decisions, lines 349 to 365 and 451 to 471. No value, target or operation
of a step changed, only the decision. New `crates/rcfd-fv/tests/sliding_threshold_scale_f724.rs` (5 tests; K1 is a heavy
record, K2 a known-miss record). D3 was decided on FLc (0): **`sliding_step` is `FluxScale` by default**.
`periodic_sliding_step` is unchanged (`Off`).

### Results

| Check | Limit | Measured | Verdict |
| --- | --- | --- | --- |
| S (a1) the scale derived and registered first | present | c77eb228 alone, before 86452826 | met |
| E1 (a2, a3) the default `sliding_step` against F-710's step: UC 50, U2 100, U3 40, LP 20 | bit-equal every step | no step differs (UC differs from `Off` exactly on 8, ..., 48; LP has 2 parts on every step) | met |
| E1u (a3) UC deviation; coincidence residuals | `<= 1e-10`; `<= 1e-12` | `8.86e-15`, `5.20e-14`; flux `1.24e-15`, GCL `1.05e-14` (ADR 0638's numbers) | met |
| E2 (a2) `periodic_sliding_step_coupled` against `periodic_sliding_step`, CS 200, CP 650 | bit-equal every step | no step differs (ADR 0641's B1u is now met) | met |
| E3 (a2) AX, the axial flow on the 3-D full annulus: the default against `Off`; velocity | bit-equal on all 48; `<= 1e-10` | none differs; `4.04e-15` | met |
| X3 AX parts | 8 on `n % 8 == 0`, 1 elsewhere | as stated | met |
| K1 (a3) NS, 400 steps: the coupled step against F-721's | bit-equal on all 400 | none differs | met (heavy record, 146 s) |
| K1r (a3, ADR 0641 A1) NS flux residual, steps 2 to 400; GCL | `<= 1e-12` | `2.46e-14`; `1.17e-14` | met |
| K2 (a3) PL, 160 steps: the coupled step against F-721's; flux residual from step 2 | bit-equal; `<= 1e-12` | differs on the 20 coincidence steps (mesh flux only, `<= 1.38e-13 max|S_f|`, from the diagnosis); `4.44e-15` (ADR 0641's A2 limit met) | **missed** (amendment 1) |
| FL (D3) as registered | 0 flips | 6, all in the new file's AX (by design) | **missed** (defective scope, amendment 2) |
| FLc the existing suites | 0 flips | 0 (100 coupled remap decisions seen, 94 of them in the existing suites; no GCL coupling) | met |
| B the existing suites, unedited | pass | `rcfd-fv` lib 113/6, `sliding_coincidence_f710` 3/0, `sliding_interface_f250` 14/4, `sliding_periodic_coincidence_f720` 4/0 (its N1 holds: the default is unchanged), `sliding_periodic_coupled_f721` 2/2, `sliding_periodic_f304` 8/2, `sliding_reimaging_f704` 12/2, `step_consistent_paths_f443` 5/2; `rcfd-api` `baseline_hashes` 3/1, `sliding_coincidence_f710` 1/0, `sliding_sector_coincidence_f730` 2/3, `sliding_sector_case_f711` 5/0, `sliding_case_f303` 7/4, `dynamic_mesh` 5/1, `wall_pressure_case_f358` au2 1/0; `rcfd-cli` `docs_site` 2/0 (passed / ignored) | met |
| B4 the diff of `crates/` | the two files and the new test file | as stated | met |
| NC1 LargestTarget against the base, CS; CP | `>= 1` each | 25; 81 | held |
| NC2 LargestTarget against Off, AX | `>= 1` | 6 (every coincidence step) | held |
| NC3 Off against the default or coupled step: UC; NS; PL | exactly the 6; 50; 20 coincidence steps | as stated | held |

Slow runs (reported, not stopped; three builds shared the machine): `step_consistent_paths_f443` 531 s,
`sliding_reimaging_f704` 175 s, `sliding_sector_case_f711` 130 s.

### Mutations (scratch copy `w49i-work/f724-scratch/mut` of e1ed0432, script `mutate.py`, log `mutations.log`, target `target-mut`)

| Mutation | Must fail | Caught by |
| --- | --- | --- |
| MU1 the periodic scale `A` replaced by `tmax` | E2 | E2 (CS 25, CP 81 steps differ) |
| MU2 the scale of `sliding_step` replaced by `tmax` | E3 | E3 (6 steps differ from `Off`) |
| MU3 the FluxScale constant `1e-2` | E1 (UC), K1 | E1 and E1u (UC `5.74e-4`, flux residual `2.07e-3`), NC3; K1, K1r (`2.19e-3`), NC3 |
| MU4 the periodic remap scale from the new faces only | E2 | E2 (CS 25 steps differ; CP not, since a planar interface carries flux) |
| MU5 the threshold of `sliding_step` zero | E1 (LP) or E3 | E1 (UC 6, LP 20 steps differ), E3 (6) |

Five of five caught as registered. The files were restored and compared after each. `cargo fmt -p rcfd-fv --check` clean;
`cargo clippy --release -p rcfd-fv --all-targets -- -D warnings` clean. Not run: the whole workspace suite and the
whole-crate integration suites (the daily gate, ADR 0653); the `rcfd-cli` sliding case tests.

### Decision

By the registered rule: S, E1, E1u, E2, E3, X3, K1, K1r, FLc, B and B4 met, the controls held, five of five mutations
caught; K2 missed (the registration's estimate; ADR 0641's own limit for PL met) and FL missed as registered (a defective
scope; FLc met); a4 open (the maintainer's approval). **Partial.** What is shown: a1 (the threshold of both steps is
relative to the term magnitudes of the targets, derived in D1); a2 (on ADR 0640's uniform flows the coupled periodic step
is the base step on every step, coincidences included; on ADR 0638's flows `sliding_step` is F-710's step bit for bit, and
on an axial flow on the full annulus it is the uncoupled step); a3 (ADR 0641's NS run is F-721's bit for bit; PL is F-721's
except that the rounding-level GCL closure is no longer coupled, within ADR 0641's limit). **Default change:**
`sliding_step` takes the new rule by default (the brief's condition: no flip in `baseline_hashes` and the named suites). Its
bits change, by rounding, only on steps where F-710 coupled a rounding-level part mean, that is flows whose targets are all
rounding at a coincidence (AX: an axial uniform flow on a 3-D full annulus). If the maintainer wants F-710's bits kept for
such inputs too, the one-line fallback is to have `sliding_step` call `sliding_step_with(.., LargestTarget)`.

### Remaining questions

1. **Q4 (open, not taken): a4.** Options: (a) switch `periodic_sliding_step` to `FluxScale` (F-720's
   `n1_non_symmetric_source_flow_leaves_the_part_means_at_coincidences` becomes an ignored record, and the sector loop of the
   tree is coupled by default, so ADR 0654's key becomes a no-op, to be kept or removed with ADR 0654 Q4); (b) keep it
   opt-in. Recommendation (a): E2 shows that the coupled step no longer touches flows whose targets are rounding, so the
   switch changes only steps with a genuine part mean, which the base leaves divergent (ADR 0640 N1). It changes default
   results and needs the maintainer's approval.
2. **The default of `sliding_step` (made; reported for approval).** Options: (a) keep `FluxScale` (made: no flip in the
   named suites); (b) revert to `LargestTarget`. Recommendation (a).
3. **K2's record.** Options: (a) a later item that registers PL's GCL closure against the base; (b) leave the known-miss
   record as it is. Recommendation (b): the difference is the intended one, and E2 checks the same pair PP.

## For the lead: roadmap entries

> Proposed status: partial.

> ADR 0659, 2026-11-16: the threshold scale of the coupled interface solves. The part-mean threshold of rcfd_fv::sliding::sliding_step and of sliding_periodic::periodic_sliding_step_coupled is now 1e-10 of the largest term magnitude of the targets over the interface cells (the sum of the absolute fluxes, and for the GCL of (V1 + V0)/dt, that make up a cell's target: the bound of its rounding, derived), no longer of the largest target; new rcfd_fv::sliding::PartCoupling (Off, LargestTarget, FluxScale) with sliding_step_with and periodic_sliding_step_with. On ADR 0640's uniform flows the coupled periodic step equals periodic_sliding_step bit for bit on every step (200 and 650, coincidences included; F-721's B1u now met); on ADR 0638's uniform flows sliding_step equals F-710's step bit for bit (UC, U2, U3, LP), UC still uniform to 8.9e-15; an axial uniform flow on the 3-D full annulus stays on the uncoupled step (F-710's rule coupled its 6 coincidence steps on rounding). ADR 0641's sector run equals F-721's bit for bit over 400 steps (divergence 2.5e-14); its planar run differs only in the rounding-level GCL closure, which is no longer coupled (1.4e-13 of max|S_f|; a registered bit-equality missed, ADR 0641's limit met). sliding_step takes the new rule by default: no decision flip in baseline_hashes and the named F-250, F-303, F-704, F-710, F-721 and F-730 suites, which all pass unedited; its bits change only for flows whose targets are all rounding at a coincidence. A registered flip count was defective in scope (it included the new test built to flip; corrected count 0). Five of five mutations caught. Open: switching periodic_sliding_step to the coupled path (changes F-720's N1 test and the sector default at coincidences; maintainer).

> F-721 note: append "Threshold scale: F-724 (ADR 0659); periodic_sliding_step_coupled now equals the base on F-720's uniform flows."

> F-710 note: append "Threshold relative to the targets' term magnitudes since F-724 (ADR 0659)."

## Amendment 3 (2026-11-16, at the lead's request, written before the code change and before any further run)

The lead does not accept the default change of addendum 1 (D3's branch "FluxScale as the default of `sliding_step`"): rule
3 and the brief keep every default bit-identical, and a default change goes to the maintainer. No limit of any check is
changed.

**The default restored.** `sliding_step` calls `sliding_step_with(.., PartCoupling::LargestTarget)`: the decision of ADR 0638
D3 with the same expression and the same operations, so every default result is that of main 14b155cd (whose `rcfd-fv` is
that of adc8402f: `git diff adc8402f 14b155cd -- crates/rcfd-fv crates/rcfd-mesh` is empty). `FluxScale` stays reachable
through `sliding_step_with` (opt-in). `periodic_sliding_step` (`Off`) and `periodic_sliding_step_coupled` (`FluxScale`,
opt-in already) are unchanged. FL and FLc no longer decide anything (D3's branch is now the opt-in one); their numbers stay
as recorded.

**How the checks are read now.**

- E1, E1u and E3, X3, NC2 were measured at e1ed0432, where `sliding_step` was `sliding_step_with(.., FluxScale)`: the same
  code as today's opt-in `sliding_step_with(.., FluxScale)`. Their recorded numbers stand as the results of the opt-in path
  against F-710's step (E1) and against the uncoupled step (E3). Their tests, which call `sliding_step`, become ignored
  records of that run (E3's would now fail by design: the restored default couples AX's coincidence steps on rounding, as
  NC2 showed). They are re-checked on the opt-in path by new tests with the same limits:
  - **E1o**: ADR 0638's loop on UC, U2, U3, LP driven by `sliding_step_with(.., FluxScale)`; on every step
    `LargestTarget` from the same inputs; bit-equal every step; on UC the limits of E1u (`1e-10`; `1e-12`) and NC3 (`Off`
    differs exactly on 8, ..., 48).
  - **E3o**: AX driven by `sliding_step_with(.., FluxScale)`; against `Off` bit-equal on all 48 steps, velocity `<= 1e-10`;
    X3 (8 parts exactly on `n % 8 == 0`, 1 elsewhere); NC2 (`LargestTarget` differs from `Off` on at least one step).
- K1, K1r, K2, E2, NC1 concern `periodic_sliding_step_coupled`, unchanged: read as before, not re-run.
- **D0 (new, the lead's point 3)**: the default `sliding_step` against **R0**, a replica in the test file of main's
  `sliding_step` (copied from `crates/rcfd-fv/src/sliding.rs` at adc8402f: its `sliding_step`, `coupling_faces` and
  `coupled_balance`, the same operations in the same order, calling the public `interface_balance` and `graph_balance`);
  ADR 0638's loop driven by the default `sliding_step` on UC (50), U2 (100), U3 (40), LP (20) and AX (48); every field of
  the step data (`mesh_flux`, `phi_old`, `old_defect`, `vol_old`, both residuals, interface faces, angles) bit-equal on every
  step. Limit: no step differs. **D0n** (its control): on AX, R0 against `sliding_step_with(.., FluxScale)` differs on at
  least one step (the replica is the coupled rule of F-710, not the opt-in rule).

**Runs.** Once each: the new tests E1o, E3o, D0 (with D0n) of `sliding_threshold_scale_f724`; `rcfd-api` `baseline_hashes`;
the named sliding suites of B (`rcfd-fv` lib, `sliding_coincidence_f710`, `sliding_interface_f250`,
`sliding_periodic_coincidence_f720`, `sliding_periodic_coupled_f721`, `sliding_periodic_f304`, `sliding_reimaging_f704`,
`step_consistent_paths_f443`; `rcfd-api` `sliding_coincidence_f710`, `sliding_sector_coincidence_f730`,
`sliding_sector_case_f711`, `sliding_case_f303`, `dynamic_mesh`, `wall_pressure_case_f358` au2). The mutations MU2 and MU5
(which act on the FluxScale branch of `sliding_step`) were caught by E3 and E1 at e1ed0432 on the same branch code; they are
not re-run.

**Decision rule.** Unchanged, with E1o and E3o in place of E1 and E3 and with D0 added to the checks that must be met.
`FluxScale` as the default of `sliding_step` moves to the open questions that need the maintainer (Q5, next to Q4).
