# ADR 0463: F-545 registration: the transient isotropic equivalence with a limit taken from a rounding probe

Status: **REGISTERED 2026-11-14, written before any code of F-545 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-545 was written in the repository. One scratch probe was run, which this text
discloses in full (section "The probe"): a temporary test file, kept outside the repository in `w49i-work/f545-scratch`,
that runs the transient set-ups of the checks with the isotropic constants changed by one unit in the last place. It compares
no orthotropic path against an isotropic one. Results go in the addenda at the end; nothing above the addenda is changed after
a run.

Follow-up of F-542 ([ADR 0459](0459-f542-registration-orthotropic-and-tabulated-materials-on-rotating-buckling-transient.md),
its check TR2, a known miss on the Hex20 block: 1.65e-12 to 9.25e-12 against 1e-12; its amendment 1 diagnosed the miss with a
one-ulp probe made after the run and proposed this item). Roadmap item F-545 (deps F-542). Number 0463 given by the lead.
Branch `f545-work` from main 1043d8c6, worktree `w49i-work/f545`, scratch `w49i-work/f545-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no unsafe code; no existing
test edited (the new tests are in a new file); clean-room.

## What F-545 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run, after a rounding probe of each set-up (a one-ulp change of the isotropic constants) made and disclosed before the registration. The transient integrator with isotropic constants in an orthotropic material matches the isotropic material on a Hex20 and a Hex8 model on both paths within limits derived from the probe; default paths bit-identical.

Clauses: (c1) a rounding probe of each set-up before the registration, disclosed; (c2) every limit registered after it;
(c3) the transient integrator with isotropic constants in an orthotropic material equals the isotropic material on a Hex20
and on a Hex8 model, on both paths, within the probed limits; (c4) default paths bit-identical.

"Both paths" are the two paths of `Integrator::new` of ADR 0457 and 0459: the elimination path (displacement constraints only)
and the transformation path (linear constraints and elastic supports, `on_transformation`). F-542 checked the Hex20 model on
the elimination path (`BLK`) and the Hex8 model on the transformation path (`TRB`); this item adds the two missing
combinations, so that each of the two element kinds is checked on each of the two paths.

## What was read, and what no run tells

- ADR 0459: the TR2 definition (D7), its miss, amendment 1 and its probe log; `crates/rcfd-fem/tests/sa_ortho_rotating.rs`
  (the helpers `block`, `tr_block`, `transient_run`, `iso_o`, `iso_c`, which the new file copies; the existing file is not
  edited, and its ignored record `tr1_tr2_tr3_transient_known_miss` stays).
- `mat_ortho::material_elasticity`, `strain_rotation`, `rotate_elasticity`, `element_stiffness` (the per-point path that the
  orthotropic material takes and the isotropic one does not).
- No library file changes in this item. The check is test-only.

## Why the 1e-12 of F-542 was wrong, and what the limit is

The transient integrator advances `(c_M M + c_C C + c_K K) u_{n+1} = rhs` for 25 steps of a load switched on at once (a step
load excites the highest modes of the mesh). A change of the stiffness by one rounding unit changes the response by far more
than one unit of the last place; how much is a property of the set-up (mesh, kind, path), not of the material code. The
orthotropic material with isotropic constants builds `D` by a different sequence of operations (a compliance inverted by
cofactors, a 6 x 6 rotation) than the isotropic one: it is *allowed* to differ from the isotropic `D` by a few ulps per entry,
exactly the size of a change of the constants by one ulp. The honest limit of the equivalence is therefore the sensitivity of
the set-up to such a change, with a stated margin. Nothing more is claimed: a difference above that margin is an error of the
per-point path, and the mutations below show that the check sees one.

## The probe (made before this registration; log `w49i-work/f545-scratch/probe.log`, file `zz_f545_probe.rs` there)

Four set-ups (the transient set-up of ADR 0459 D7: nodal force `(0, 0, -1e4)` N on the tip face, `LoadHistory::model_loads`
with `LoadTable::constant(1, 0, 1)`, average acceleration, `Rayleigh::from_ratio(0.02, 500, 5000)`, `dt = 2e-4`, 25 steps from
rest, the displacement and the velocity at the end), all on the box `1 x 0.4 x 0.25`, material `E = 2e11`, `nu = 0.3`,
`rho = 7850`:

| Set-up | Element | Path | Mesh and support |
|---|---|---|---|
| H20E | Hex20 | elimination | 4 x 2 x 1 upgraded, all three components of `xmin` held (the `BLK` of ADR 0459) |
| H20T | Hex20 | transformation | 5 x 2 x 1 upgraded, `u_x = u_y = 0` on `xmin`, `u_z = 0` at each `xmin` node as a one-term linear constraint, `NodalSpring` in z of 5e7 N/m on the tip nodes |
| H8E | Hex8 | elimination | 4 x 2 x 1, all three components of `xmin` held |
| H8T | Hex8 | transformation | 5 x 2 x 1 with the H20T support (the `TRB` of ADR 0459) |

For each set-up the probe ran: (1) the isotropic material twice (rerun: the difference is 0 in all four, the run is
deterministic); (2) the isotropic material against itself with `nu` one ulp above (`f64::next_up`); (3) against itself with
`E` one ulp above; (4) the orthotropic material with `Fixed(I)` and `G = E/(2 (1 + nu))` against itself with `G` one ulp above.
The measure is the one of the checks: `max |a - b| / max |b|` over all unknowns, for the displacement `u` and the velocity `v`
at the end. Measured (relative):

| Set-up | `nu` + ulp (u; v) | `E` + ulp (u; v) | `G` + ulp, `ISO_O` (u; v) | largest of the six |
|---|---|---|---|---|
| H20E | 5.11e-12; 5.61e-12 | 1.53e-12; 9.09e-12 | 3.32e-12; 1.08e-11 | 1.08e-11 |
| H20T | 2.60e-13; 4.33e-13 | 1.92e-13; 4.15e-13 | 2.18e-13; 2.95e-13 | 4.33e-13 |
| H8E | 3.00e-14; 6.93e-14 | 1.47e-13; 7.96e-14 | 9.82e-14; 4.09e-14 | 1.47e-13 |
| H8T | 6.60e-15; 2.61e-13 | 1.73e-14; 2.74e-13 | 6.16e-15; 3.48e-13 | 3.48e-13 |

(The H20E and H8T rows reproduce the post-run probe of ADR 0459 amendment 1: 5.1e-12 and 5.6e-12, 1.5e-12 and 9.1e-12; 6.6e-15
and 2.6e-13.) The sensitivity differs by a factor 70 between the Hex20 block with its stiff high modes and the Hex8 block;
one limit for all four would be either blind on three or false on one.

## The rule that gives the limits (fixed here, before any orthotropic comparison)

The limit of a set-up is **4 times the largest of its six probe values, rounded up to one significant figure**. The factor 4
is chosen from the earlier measurements: ADR 0459 measured 3.85e-12 and 6.24e-12 (`Fixed(rot)`) and 1.65e-12 and 9.25e-12
(`Fixed(I)`) on H20E against a one-ulp spread of 9.1e-12, and 3.4e-13 on H8T against 2.7e-13, that is, the orthotropic path
landed within about 1.3 times the largest single-ulp spread. The factor 4 leaves room for the three ulp-sized changes (each of
`E`, `nu`, `G`) acting together. It is stated, not searched for, and it is applied to the table above.

| Set-up | 4 x largest probe | Registered limit L |
|---|---|---|
| H20E | 4.32e-11 | **5e-11** |
| H20T | 1.73e-12 | **2e-12** |
| H8E | 5.9e-13 | **6e-13** |
| H8T | 1.39e-12 | **2e-12** |

## Design

A new test file `crates/rcfd-fem/tests/sa_transient_iso_equiv.rs` with the helpers copied (builder `setup(mat, hex20, linear)`
as in the probe, `transient_run`, `dev`, `iso_o`, `iso_c`, `rot`). No library line changes; the test-only item has nothing
default to alter.

Reference run per set-up: `ISO` (`Material::isotropic_elastic(2e11, 0.3, 7850, 0)`). Compared runs: `ISO_O(I)` (`Fixed(I)`),
`ISO_O(rot)` (`Fixed(rot((1, 2, 3), 0.5))`, a general rotation: the strain rotation `T` is applied to an isotropic `C`),
`ISO_C` (`Cylindrical { origin: (-0.5, 0.2, 0), axis: (0, 0, 1) }`: axes that change from point to point), all with
`G = E/(2 (1 + nu))`. The rotation of an isotropic elasticity by any proper rotation is the identity in exact arithmetic, so
each must equal `ISO` up to rounding.

## Checks

| Id | Clause | Check | Limit | Estimate |
|---|---|---|---|---|
| P0 | c1, c2 | this ADR: the probe table and the rule above exist before the run | present | by construction |
| EQ-H20E | c3 | `ISO_O(I)`, `ISO_O(rot)`, `ISO_C` against `ISO`, displacement and velocity | `<= 5e-11` each | up to 1e-11 (F-542: 9.25e-12) |
| EQ-H20T | c3 | the same on H20T | `<= 2e-12` each | up to 4e-13 |
| EQ-H8E | c3 | the same on H8E | `<= 6e-13` each | up to 1.5e-13 |
| EQ-H8T | c3 | the same on H8T | `<= 2e-12` each | up to 3.5e-13 (F-542: 3.4e-13) |
| NC1 | c3 (sensitivity) | negative control: `ISO_O(I)` with `E` raised by a factor `1 + 1e-6` against `ISO`, on each set-up, `u` | `> 100 L` of the set-up | about 1e-6, 4 orders above the limits |
| NC2 | c1 | the probe's own control, rerun: `ISO` twice on each set-up, `u` and `v` | `== 0` | 0 (measured in the probe) |
| B1 | c4 | no file under `crates/*/src` differs from main 1043d8c6 (`git diff --stat 1043d8c6 -- crates/*/src` is empty); `sa_ortho_rotating_default_bits` (20 digests of ADR 0459) runs as before | empty diff; passes | unchanged |

Each of EQ is 3 materials x 2 outputs = 6 numbers per set-up; a set-up is met if all six are within L.

## Mutations (library, on a scratch copy of the worktree; recorded in the addendum, none lands)

| Id | Mutant (`mat_ortho.rs`) | Expected |
|---|---|---|
| Mu1 | `rotate_elasticity` computes `T C T^T` in place of `T^T C T` (the transposed Bond product; `T` is not orthogonal in engineering Voigt, so isotropy is lost) | caught by `ISO_O(rot)` and `ISO_C` on every set-up; `ISO_O(I)` unaffected (`T = I`) |
| Mu2 | `material_elasticity` shear `c[3][3] = 2 g[0]` in place of `g[0]` (a wrong shear factor) | caught by every material on every set-up |
| Mu3 | `material_elasticity` `c[0][1]` taken with the sign changed (a wrong Poisson coupling) | caught by every material on every set-up |
| Mu4 | `material_elasticity` shear diagonal order `c[4][4] = g[1]`, `c[5][5] = g[2]` (the pairs swapped) | **predicted equivalent**: the three shear moduli are equal for isotropic constants, so no isotropic comparison can see it (it is seen by the orthotropic checks of ADR 0452 and 0457) |
| Mu5 | `strain_rotation` built from the transposed axes `R^T` (the inverse rotation) | **predicted equivalent**: the isotropic `C` is invariant under any rotation, `R` or `R^T` |

Mu4 and Mu5 are listed because the identity tested here cannot see them, and saying so is part of the limits of the check. They
are registered as predicted equivalent, are run, and are excluded from the decision rule; if one is caught, it is recorded as a
surprise, not as a failure.

## Decision rule

- **done**: every EQ row met (six numbers per set-up, all within the registered L), NC1 and NC2 as stated, B1 as stated, Mu1 to
  Mu3 caught, and no limit, set-up or test changed after the run.
- **partial**: any registered EQ number missed (kept as an ignored `*_known_miss` test with the limit intact), or Mu1, Mu2 or
  Mu3 not caught, or any check met only after an amendment made after its run.
- **todo**: the tests do not run.

## Limits of this item

- It tests isotropic constants only: it says nothing about an orthotropic material's transient response (that is TR1 of ADR
  0459, the identity against the modal analysis). Mu4 and Mu5 above show what an isotropic comparison cannot see.
- One load, one integrator method (average acceleration), one damping, one step size, 25 steps; other set-ups have other
  sensitivities. The probed limits belong to these four set-ups.
- The limits are sized to the set-up's own rounding sensitivity: they do not show that the per-point path is accurate to 1e-12
  on a stiff model; they show that it is as accurate as the isotropic path's own conditioning allows.
- The tabulated material (TR3 of ADR 0459, bit-equal) and the cyclic paths are not re-checked here.
