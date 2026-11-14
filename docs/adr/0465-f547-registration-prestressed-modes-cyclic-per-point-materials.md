# ADR 0465: F-547 registration: pre-stressed modes with a cyclic constraint and per-point materials

Status: **REGISTERED 2026-11-14, written before any code of F-547 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-547 was written, compiled or run, except the bit record of the paths it touches
(B1, a test file using only the API of main a52a5291, run once at that commit with no library line changed; disclosed below).
Results go in the addenda at the end; nothing above the addenda is changed after a run.

Parent: [ADR 0462](0462-f544-registration-cyclic-symmetry-with-cylindrical-material-axes.md) (F-544:
`mat_ortho::check_cyclic_axes`, `CYCLIC_AXES_TOLERANCE`, its D1 derivation, its finding 5 and remaining question 2,
recommendation (a), and its practice of registering a 1e-12-class limit only on a set-up probed for its one-ulp
sensitivity). Grandparents: [ADR 0459](0459-f542-registration-orthotropic-and-tabulated-materials-on-rotating-buckling-transient.md)
(F-542: `refuse_with_cyclic`, the D3 refusal) and [ADR 0457](0457-f537-registration-orthotropic-and-tabulated-materials-on-modal-paths.md)
(F-537: the pre-stressed modes on both paths take the per-point stiffness, D2; its D3 refusal of a cyclic constraint with
either variant, placed in `mpc::check_transformation_model`, which `prestressed_modes` does not call). The pre-stressed modes
on the transformation: ADR 0398 D2. The cyclic rows and the full rotor: ADR 0386. Work package SA-2 of the structural
solver, advanced tier ([ADR 0310](0310-structural-solver-advanced-tier-plan.md)). Roadmap item F-547 (deps F-544). Number
0465 given by the lead. Branch `f547-work` from main a52a5291, worktree `w49i-work/f547`, scratch `w49i-work/f547-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no unsafe code, the crate
graph unchanged; no existing test edited; clean-room. Another agent (F-532, `damped.rs`) edits `rcfd-fem` concurrently: the
only shared library file edited here is `geometric.rs`, at the lines named in D2.

## What F-547 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. The pre-stressed modes on the transformation refuse a cyclic constraint with an orthotropic or tabulated material by name unless every material with its own axes has cylindrical axes about the cyclic axis and origin; with such axes the index-0 pre-stressed modes of a sector match the full rotor of the same material and pre-stress within registered limits; default paths bit-identical.

Clauses: (a0) every limit registered before the run (this document); (a1) the pre-stressed modes on the transformation
refuse a cyclic constraint with either variant by name unless every material with its own axes has cylindrical axes about
the cyclic axis and origin; (a2) with such axes the index-0 pre-stressed modes of a sector match the full rotor of the same
material and pre-stress within registered limits; (a3) default paths bit-identical. The brief of the lead adds: (b1) record
the digests on the base before code (B1); (b2) state that the new refusal changes results only of models that were silently
wrong, and list it as a point for the maintainer (D5, remaining question 1); (b3) register 1e-12-class limits only on set-ups
probed for one-ulp sensitivity, disclosed (arithmetic 2).

## What was read (main a52a5291, read only)

1. `AGENT_RULES.md`; ADR 0462 whole (D1, D2, finding 5, the checks, addendum 1, remaining question 2 and its follow-up text);
   ADR 0457 (findings, D2, D3, D4, the checks); ADR 0459 (D1 to D3, D8 RQ1).
2. Code: `crates/rcfd-fem/src/{geometric.rs (prestressed_modes, prestressed_modes_on_transformation), mat_ortho.rs
   (refuse_with_cyclic, CYCLIC_AXES_TOLERANCE, coaxial, check_cyclic_axes), mpc.rs (uses_transformation,
   check_transformation_model), cyclic.rs (FullRotor: node, expand, expand_field), rotating.rs (RotatingPart::new,
   with_unit_prestress, operator, mass)}`; the tests `sa_ortho_cyclic.rs` (whole: the sector D, the full rotor with the
   field copied, the procedure of ADR 0386 E2), `sa_ortho_cyclic_default_bits.rs`, `sa_ortho_modal.rs` (`tr_block`, the R
   rows), `sa_ortho_rotating.rs` (the E8 rows); `grep prestressed_modes` over `crates/` (callers: tests of `rcfd-fem` only;
   no case file, no other crate).
3. No paper is used. The design rests on ADR 0462 D1 (derived there from the definitions of the cylindrical axes and of the
   cyclic rows) and on the definition of the index-0 rows (ADR 0386).

Findings that shape the design:

1. `prestressed_modes` checks `check_supported_solids_with_materials` and, for a model on the transformation (any cyclic
   constraint is), calls `prestressed_modes_on_transformation`, which calls `refuse_nonzero_values`, `transformation`,
   `point_stiffness`, `add_supports`, `assemble_geometric` and `consistent_with`: no step looks at the material axes against a
   cyclic constraint. A sector with `Fixed`, `PerElement` or non-coaxial cylindrical axes is solved today without a word.
2. ADR 0462 D1: such a sector describes no rotor whose sectors are copies of it with the material as given (the cyclic rows
   assume the same sector stiffness in each sector's own frame; with `Fixed(A)` axes the copies of a full model have the
   same global axes, not rotated ones). The result is the result of some other rotor (every sector carrying axes turned with
   it), which the user did not write.
3. No existing test runs `prestressed_modes` on a model with a cyclic constraint and either variant (grep: the E8 rows of
   `sa_ortho*.rs` and `sa_scaffold.rs` use models without a cyclic constraint), and no case file uses the variants (added on
   2026-11-14). No existing test changes; no record-and-copy step is needed.
4. The index-0 rows of a cyclic constraint (ADR 0386) are `u_high = R u_low`: the transformation's solution space is the
   field of the full rotor that repeats in every sector (harmonic 0). The pre-stressed modes of a sector are therefore the
   harmonic-0 modes of the full rotor, provided the pre-stress also repeats in every sector (ADR 0462 D1: the centrifugal
   pre-stress about the cyclic axis does; a uniform stress `diag(s, s, t)` in global components is invariant under rotation
   about z and does).

## Arithmetic before this registration (disclosed)

1. **B1**: `crates/rcfd-fem/tests/sa_ortho_cyclic_prestress_default_bits.rs` was written with the API of main a52a5291
   only and run once at that commit (release; log `f547-scratch/b1-base.log`, the run ended at the `BASE = None` expectation
   after printing); the 15 digests it printed are its constants `BASE`.
2. **No rounding probe.** No limit of this registration is of the 1e-12 class: the sector-against-full-rotor limits are
   1e-8 and 1e-7 (estimates 1e-12, from the rounding measured by ADR 0462 on the same disc, at most 1.3e-13), and the only
   tighter check (A1) is an equality derived from the arithmetic of the axes (ADR 0462 arithmetic 3), not a rounding bound.
   By the practice of ADR 0462 a probe is needed only for a 1e-12-class limit; none is registered, so none was run. The
   thin disc D moves 2.2e-13 per ulp (ADR 0462 arithmetic 2), 4.5e4 times below the 1e-8 limits.
3. **Sizing of the full computation** (from the log of ADR 0462 F1, `f544-scratch/run-sa_ortho_cyclic.log`, no new run): at
   rest the four lowest harmonic-0 frequencies of the disc D with `ORTHO_C` are 215.8, 285.9, 1188.3 and 1738.7 Hz, and the
   full rotor has 33 eigenvalues below `0.999 (2 pi 1738.7)^2` (F1's count with the same cut-off). The pre-stress of D8 raises
   every eigenvalue (stress stiffening only; `prestressed_modes` has no spin softening), the bending modes more than the
   in-plane ones; the full count below the cut-off of D6 is therefore expected between 25 and 40. Sixty full modes are
   computed (D6); a full spectrum that does not pass the cut-off is a defective set-up, not a miss.
4. **The speed.** `Omega = 680 rad/s` (about half the lowest rest frequency of the sector, 1355.887 rad/s measured by ADR
   0462 F3, rounded down to two digits): `Omega^2 = 4.62e5 rad^2/s^2` against the lowest rest eigenvalue `(2 pi 215.8)^2 =
   1.84e6`; with a Southwell-type coefficient of order 1 the lowest frequency rises by about ten percent (NC1).

No other run was made.

## Design

Every choice below that the maintainer would normally be asked about is taken on the maintainer's standing instruction of
2026-11-11 (take the recommendation); none changes a default result, a golden or a hash table.

### D1 The refusal (a1)

`prestressed_modes_on_transformation` calls `crate::mat_ortho::check_cyclic_axes(model)?` as its first step (before
`refuse_nonzero_values`), the check that `check_transformation_model` makes for the static solve and the modal analysis on
the transformation (ADR 0462 D2). For a model without a cyclic constraint or without either variant it returns `Ok` at once;
otherwise every material of either variant with its own axes (an `Orthotropic` material, a `Tabulated` one on an orthotropic
base) must have `Cylindrical` axes about the axis and origin of every cyclic constraint within `CYCLIC_AXES_TOLERANCE`
(either sense, the origin anywhere on the line), and the first that has not is refused with
`NotYetSupported { item: "material", variant, location: "material {i}, element set '{set}', with the cyclic constraint {j}",
package: FOLLOW_UP }` (`FOLLOW_UP = "a follow-up of ADR 0452"`), the error of ADR 0457 D3, ADR 0459 D3 and ADR 0462 D2. No
new function, constant or message.

The elimination path of `prestressed_modes` is not changed: a model with a cyclic constraint always takes the transformation
(`uses_transformation`).

### D2 The code (shared file: these lines only)

- `geometric.rs`, `prestressed_modes_on_transformation`: one call and a two-line comment at its head; the doc comment of
  `prestressed_modes` gains one sentence naming the refusal.

For a model without either variant `check_cyclic_axes` returns `Ok` before reading anything else, so every later line runs as
before: the same bits (B1) and the same errors. For a model with the variants and coaxial cylindrical axes (accepted before
and after) it returns `Ok` too: the same bits (B1 records two such models).

### D3 Why coaxial axes give the full rotor's harmonic-0 pre-stressed modes (a2)

By ADR 0462 D1, with cylindrical axes about the cyclic axis the elasticity at the image `y = o + Q (x - o)` of a point is the
rotated tensor, so the full rotor's stiffness is block-circulant in the sectors' own components; the mass needs the density
only. The geometric stiffness `K_sigma` of an element depends on the element's stress and shape-function gradients only; if
the stress at the image points is the rotated stress (`sigma(y) = Q sigma(x) Q^T`), the copy's `K_sigma` is
`blockdiag(Q^j) K_sigma,e blockdiag(Q^j)^T` (the same objectivity argument), and `K + K_sigma` is block-circulant too. The
eigenproblem `(K + K_sigma) phi = lambda M phi` then splits by harmonic index, and its harmonic-0 part on the full rotor is
the sector's problem on the rows `u_high = R u_low` (ADR 0386). The pre-stresses used (D8) repeat in this sense: the static
solution of the centrifugal load about the cyclic axis (ADR 0462 D1) and a uniform `diag(s, s, 0)`.

### D4 What the refusal changes (b2)

Before F-547 a sector with a cyclic constraint and an orthotropic or tabulated material whose axes are not cylindrical about
the cyclic axis was solved by `prestressed_modes` without a warning. Its result is not the result of the rotor that the
sector and the material describe (finding 2, ADR 0462 D1; shown on the base by NC0). After F-547 it is refused by name. The
new refusal therefore changes the results only of models that were silently wrong: isotropic models, models without a
cyclic constraint, and sectors whose per-point materials have coaxial cylindrical axes keep their bits (B1). The variants were
added this week and no case file uses them (finding 3). This is listed for the maintainer (remaining question 1).

### D5 Paths not changed

Buckling and the transient integrator with a cyclic constraint keep refusing either variant (`refuse_with_cyclic`, ADR 0462
D6); the cyclic harmonic paths and the rotating sector keep `check_cyclic_axes` (ADR 0462). Harmonic indices other than 0 of
the pre-stressed modes have no entry point of their own (`RotatingSector` covers the centrifugal case at every index, ADR
0462 F3); none is added.

### D6 The sector-against-full-rotor procedure (a2)

For a sector model `S` with the pre-stress `P_S`, and its full rotor `F` (by `cyclic::full_rotor`, the same material on
`"all"`, `xmin` clamped, a table's field copied node by node as in ADR 0462 D4) with the pre-stress `P_F` of the same kind
computed on `F`:

1. `prestressed_modes(S, P_S, eq_opts(4))`: four eigenvalues `lambda_S`; the cut-off `lambda_c = 0.999 max lambda_S`; the
   compared set `C` = the sector eigenvalues below `lambda_c` (expected the three lowest).
2. `prestressed_modes(F, P_F, eq_opts(60))` (the elimination path: `F` has no linear constraint). Defective set-up (an
   assertion, not a miss): the largest full eigenvalue not above `lambda_c`, or any sector or full eigenvalue within 1e-7
   (relative) of `lambda_c`.
3. The harmonic-0 fraction of each full mode `phi` below `lambda_c`: with `u_0(p) = (1/N) sum_j R_j^T phi(node(j, p))` at
   every sector node `p` (`R_j` the rotation by `2 pi j/N` about the cyclic axis, `cyclic::rotation_matrix`; `node(j, p)` of
   `FullRotor`), `psi = expand_field(0, u_0, 0)` (the field repeating `u_0` in every sector) and `f = psi^T M psi /
   phi^T M phi`. The average over the rotations is the M-orthogonal projection on the fields that repeat (the full rotor's
   mass is invariant under the sector rotation), so `f = 1` for a harmonic-0 mode and `f = 0` for every other index. The
   full harmonic-0 set `F_0` = the full eigenvalues below `lambda_c` with `f >= 0.5`, ascending.
4. Checks: `|F_0| = |C|`; `max |sqrt(c_i / f_i) - 1|` over the pairs in order; every full mode below `lambda_c` has `f <= 1e-6`
   or `f >= 1 - 1e-6` (the separation is clean: no near-degeneracy of harmonic 0 with another index); the residual
   `|A x - lambda M x| / |A x|` on the free rows of `F` for each sector mode of `C` expanded (`x = expand_field(0, mode, 0)`),
   `A = K + K_sigma(P_F)` of the full rotor as `RotatingPart::with_unit_prestress(F, axis, origin, P_F, options with
   spin_softening(false)).operator(1)` (`K + 1^2 K_sigma,1`, ADR 0459 D8 RQ1) and `M` its mass.
5. For a centrifugal pre-stress also: the stress of `P_F` in the elements of copy 0 (element `e` of copy 0 is element `e` of
   `F`, `full_rotor`'s numbering) against `P_S`, `max |sigma_F - sigma_S| / max |sigma_S|`.

`eq_opts(n)` = `ModalOptions::new(n).tolerance(1e-9)`, 400 iterations (ADR 0386, ADR 0462 D8).

### D7 The record of the gap on the base (NC0)

A test `nc0_base_gap_record` (ignored after its single run, a record) is run once at the registration commit, before any
library line of F-547: the sector D with `ORTHO` and `Fixed(I)` axes and its centrifugal pre-stress at `Omega` is solved by
`prestressed_modes` (accepted on the base); its compared modes, expanded, are tested on the full rotor of the same material
as given (`Fixed(I)` on the full mesh, its own centrifugal pre-stress) with the residual of D6 step 4. After F-547 the sector
is refused (R1) and the test, if run, prints the refusal and returns.

### D8 Models of the checks

- Materials (ADR 0462 D8): `ORTHO_C` = orthotropic `E = (1.4e11, 1e10, 1e10)`, `nu = (0.3, 0.3, 0.45)`, `G = (5e9, 5e9,
  3.5e9)`, `rho = 1600`, `alpha = 0`, `Cylindrical { origin: (0, 0, 0), axis: (0, 0, 1) }`; `ORTHO(axes)` the same constants
  with other axes; `TAB_O` = `Tabulated(ORTHO_C, [200, 600] K, e_1 = [1.4e11, 1.0e11], g_12 = [5e9, 4e9])` with the field
  `T = 293 + 250 (r - 0.25)/0.75 + 200 z` (reference 293 K; no expansion, so no thermal stress); `ISO` isotropic `E = 2e11`,
  `nu = 0.3`, `rho = 7850`, `alpha = 0`; `TAB_F` = `Tabulated(ORTHO(Fixed(I)), ...)` with the same columns and field.
- **Disc D** (ADR 0395, ADR 0462): the sector `r in [0.25, 1]`, `phi in [0, 2 pi/12]`, `z in [-0.1, 0.1]`, Hex20 3 x 2 x 1,
  `xmin` clamped, `Cyclic("ymin", "ymax", (0, 0, 1), (0, 0, 0), 12)` (constraint index 3).
- **Pre-stresses**: `CEN` = `Prestress::from_static` of the static solve of the model with `Load::centrifugal((0, 0, 1),
  (0, 0, 0), 680)` added (the sector on the transformation, the full rotor by elimination); `UNI` = `Prestress::uniform(diag(1e8,
  1e8, 0))` (tension in x and y).
- **Transformation block** TRB (ADR 0457 D7): Hex8 5 x 2 x 1 on `1 x 0.4 x 0.25`, `u_x = u_y = 0` on `xmin`, `u_z = 0` at each
  `xmin` node as a one-term linear constraint, a spring of `5e7` N/m in z on the `xmax` nodes; no cyclic constraint.

## Checks

| Check | Clause | Quantity | Limit | Estimate |
| --- | --- | --- | --- | --- |
| P1 | a2 | D with `ORTHO_C`, `CEN`: D6 steps 1 to 5: count; frequencies; separation; expanded residual; pre-stress of copy 0 | equal; `<= 1e-8`; every `f <= 1e-6` or `>= 1 - 1e-6`; `<= 1e-7`; `<= 1e-8` | 1e-12 (the same discrete problem by D3; ADR 0462 F1 3.6e-14 at rest, F3 7.9e-14 with the centrifugal pre-stress); `f` within 1e-15 of 0 or 1 (the square of the eigenvector error, below 1e-9); residual 1e-11 (ADR 0462 F1 1.8e-11); stress 1e-13 (ADR 0462 F2 6.2e-14 on displacements) |
| P1t | a2 | the same with `TAB_O` and its field (the full field copied) | as P1 | as P1 (ADR 0462 F1t 6.5e-14, F3t 1.3e-13) |
| P2 | a2 | D with `ORTHO_C`, `UNI` on both: D6 steps 1 to 4 | as P1 without the pre-stress row | as P1 |
| A1 | a1 (D1) | D: `ORTHO(Cylindrical about (0, 0, -1) through the origin)` and `ORTHO(Cylindrical about (0, 0, 1) through (0, 0, 0.3))` against `ORTHO_C`, each with the `CEN` of the `ORTHO_C` sector: `prestressed_modes` with `ModalOptions::new(3)`, Backward, 1e-12: eigenvalues and modes | equal as numbers (`==`) | equal (ADR 0462 arithmetic 3: the same elasticity as numbers; `K_sigma` and `M` take no axes; ADR 0462 A1 measured equal) |
| A2 | a1 (D1) | D: `ORTHO(Cylindrical about (0, 1e-11, 1))` (inside the tolerance), `prestressed_modes` with the `CEN` of `ORTHO_C` | solves | solves |
| R1 | a1 | D with each of `ORTHO(Fixed(I))`; `ORTHO` cylindrical about `(0, 0.1, 1)`; about `(0, 1e-9, 1)` (outside the tolerance); about `(0, 0, 1)` through `(0.05, 0, 0)`; `ORTHO(PerElement)` (one identity per element); `TAB_F` with the field: `prestressed_modes` with `Prestress::uniform` zero, refused `NotYetSupported { item: "material", variant, location: "material 0, element set 'all', with the cyclic constraint 3", package: FOLLOW_UP }` | as stated (6 rows) | as stated |
| R2 | a1 | TRB with `ORTHO(Fixed(I))`, and with `TAB_F` and the field `T = 293 + 250 x`: `prestressed_modes` (uniform `S_xx = 2e8`, `ModalOptions::new(3)`) solves (no cyclic constraint: the check returns `Ok`) | solves (2 rows) | solves |
| B1 | a3 | `sa_ortho_cyclic_prestress_default_bits.rs` (15 digests of main a52a5291: an isotropic sector's centrifugal pre-stress and its pre-stressed modes, a uniform pre-stress, a sector with a temperature field, `ORTHO_C` and `TAB_O` sectors with coaxial axes, TRB isotropic on the transformation, the isotropic block by elimination) against `BASE`; and the existing tests reaching the changed lines or the per-point paths (`sa_ortho_cyclic_default_bits`, `sa_ortho_modal_default_bits`, `sa_ortho_rotating_default_bits`, `sa_ortho_default_bits`, `sa_ortho_cyclic`, `sa_ortho_modal`, `sa_ortho_rotating`, `sa_ortho`, `sa_scaffold`, `sa_scaffold_bits`, `si_geometric`, `si_prestress_mpc`, `si_harmonic_correction`, the `rcfd-fem` lib tests) pass | `==`; pass | `==`; pass |

### Negative controls

| Control | Quantity | Expected |
| --- | --- | --- |
| NC0 | (run once on the base, D7) D with `ORTHO(Fixed(I))` and its `CEN`: `prestressed_modes` accepted on the base; the smallest over `C` of the residual of D6 step 4 of the expanded sector modes on the full rotor with `ORTHO(Fixed(I))` and its own `CEN` | accepted; `> 1e-2` (the full rotor as written has the fibres along x in every sector; `E_1/E_2 = 14`) |
| NC1 | P1 against the same sector with the zero pre-stress (`Prestress::uniform` zero): `max |f_CEN/f_0 - 1|` over `C` | `> 1e-2` (about 0.1, arithmetic 4: the checks see the pre-stress) |
| NC2 | the residual of D6 step 4 of P1's expanded modes on the full rotor's `K` alone (`operator(0)`): the largest over `C` | `> 1e-2` (the residual check sees the geometric stiffness) |

### Planned mutations (scratch copy `w49i-work/f547-scratch/mut`, the test file `sa_ortho_cyclic_prestress`)

| Mutation | Change | Expected to catch |
| --- | --- | --- |
| Mu1 | the call of `check_cyclic_axes` removed | R1 (six rows solve) |
| Mu2 | `refuse_with_cyclic` in place of `check_cyclic_axes` | P1, P1t, P2, A1, A2 (the coaxial sectors refused) |
| Mu3 | the transformation path leaves out the geometric stiffness (`A = K`) | P1, P1t, P2 (frequencies, residual) |
| Mu4 | the transformation path scales the geometric stiffness by `1 + 1e-4` | P1, P2 (frequencies about 1e-5 off, the residual about 1e-5) |

## Decision rule

- **done**: every check P1, P1t, P2, A1, A2, R1, R2 and B1 met in its single registered run, NC0 to NC2 as expected, and every
  mutation caught by at least one registered check.
- **partial**: any registered check missed (kept as a `*_known_miss` record with its limit), a mutation not caught, or a check
  met only after an amendment made after its run.
- **todo**: the refusal not added.

## Limits (what is not covered)

- Only cylindrical axes about the cyclic axis are taken; `Fixed`, `PerElement` and other cylindrical axes are refused (D1).
- Only the harmonic-0 pre-stressed modes (the transformation's rows) are checked; no harmonic-k entry point of the
  pre-stressed modes exists (D5).
- The pre-stress must repeat in every sector for the sector to describe the full rotor (D3); `prestressed_modes` cannot test
  that for a caller's pre-stress (a pre-stress is data), and this is not checked.
- Buckling and the transient integrator with a cyclic constraint keep refusing both variants (D5).
