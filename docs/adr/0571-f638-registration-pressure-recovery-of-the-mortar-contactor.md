# ADR 0571: F-638 registration: pressure recovery of the mortar contactor

Status: **REGISTERED 2026-11-15, written before any code of F-638 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-638 was written or run in the repository. The arithmetic below was done by hand (no
rcfd code, no script). Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-638 (deps F-623), the follow-up proposed by [ADR 0551](0551-f623-registration-mortar-contactor-for-quadratic-faces.md)
(its remaining question 1: the contact radius of HS20m and HS10m read from the nodal multipliers). Number 0571 given by the
lead. Branch `f638-work` from d1730051 (main), worktree `w49i-work/f638`, scratch `w49i-work/f638-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no
unsafe code, the crate graph unchanged; clean-room; `docs/ROADMAP.md` and `docs/roadmap.json` not edited (the entries are
proposed at the end of the addendum). This item adds to `crates/rcfd-fem/src/contact.rs` only (a new public type and a new
method in their own blocks; no existing line of the solve paths is changed, so the concurrent F-639 work on the total
Lagrangian path of the same file is not touched), adds test files, and documentation. No existing test is edited.

## What F-638 asks

The acceptance, verbatim, binding:

> Every limit not stated here is registered in the ADR before the run. The traction sum_a (t_a/A_a) Phi_a of ADR 0551's mortar contactor recovered at the load-rule points (or projected onto the face's shape functions) and reported per point; the Hertz problems of ADR 0551 HS20m and HS10m read from it meet radius 0.15, peak 0.20, approach 0.10 with the gap tolerance 1e-8; the patch tests PT10m and PT20m within 1e-8 on the recovered traction; defaults bit-identical.

Clauses: **c1** every other limit registered here before the run; **c2** the traction `sum_a (t_a/A_a) Phi_a` recovered at the
load-rule points and reported per point; **c3** HS20m and HS10m read from it meet radius 0.15, peak 0.20, approach 0.10 with the
gap tolerance `1e-8`; **c4** PT10m and PT20m within `1e-8` on the recovered traction; **c5** defaults bit-identical.

## What was read (no run)

ADR 0551 (whole: D1 to D7, the checks, addendum 1, its diagnosis and remaining questions), ADR 0540 D3 and D5 (the Hertz
reading). Code: `crates/rcfd-fem/src/contact.rs` (the types, `mortar_basis`, `mortar_kin`, the mortar branch of
`ContactSet::new`, `ContactSet::evaluate`), `loads::face_rule` and `face_shape_functions`, `mesh::face_nodes`; the tests
`sa_contact_mortar_f623.rs`, `sa_contact_mortar_hertz_f623.rs`, `f601_common/mod.rs` (`hertz_read`, `hertz_theory`,
`hertz_prescribed`, `hertz_block`, `Checks`), `f623_common/mod.rs` (`patch_model`, the T1 models). The per-node table of ADR
0551's diagnostic run (`f623-scratch/diag-hertz.log`) was **not** read; only the summary quoted in ADR 0551's diagnosis (the
corner-only fits 0.116 and 0.113, the mid-edge-only fits 0.269 and 0.132, the Tet10 mid-edge values 0.033 to 0.100 near
`r = 0.06` to `0.07`), which was itself a diagnostic outside any registration. Nothing outside the repository.

## Design

### D1. The recovered traction (c2)

ADR 0551 D2 and D3 give each mortar node `a` the force `t_a` (the augmented-Lagrangian force of its weighted gap
`gbar_a = int Phi_a g dA / A_a`, `A_a = int Phi_a dA`). The virtual work of the contact forces is

    sum_a t_a d gbar_a = sum_a (t_a/A_a) int Phi_a d g dA = int lambda_h d g dA,   lambda_h = sum_a lambda_a Phi_a,   lambda_a = t_a/A_a,

so `lambda_h` is the contact traction (a nominal pressure, per reference area) that the discrete problem actually applies, and
the `lambda_a` are its coefficients in the basis `Phi`, not its values (this is ADR 0551's diagnosis, stated as an identity).
At a load-rule point `zeta_g` of a contactor face `f` the recovered traction is

    p_g = sum_{a in f} lambda_a Phi_a^f(zeta_g),

with `Phi^f` the weighting functions of ADR 0551 D1 (`mortar_basis`) of the face's shape-function values at `zeta_g`, the sum
over the face's nodes. Inactive nodes have `t_a = 0` (ADR 0551 D3), so they contribute nothing. This is the standard reading
of a mortar multiplier field as a function in its own discrete space (the mortar literature, e.g. Puso and Laursen, Comput.
Methods Appl. Mech. Engrg. 193 (2004) 601, and Popp, Gee and Wall, Int. J. Numer. Meth. Engng. 79 (2009) 1354; both from
memory, not read): derived here from the virtual work above, nothing taken from those papers.

### D2. Projection onto the face's shape functions is the same field (c2, the acceptance's alternative)

On a face, `Phi = T N` with `T` the invertible map of ADR 0551 D1, so `span{Phi_a} = span{N_a}`. Across faces `lambda_h` is
continuous: on an edge shared by two faces only the edge's three nodes have non-zero `N` (Quad8 serendipity and Tri6 shape
functions restricted to an edge are the 1D quadratic functions of its nodes), and the restriction of `Phi_corner(i)` there is
`N_i + alpha N_e` (`N_e` of that edge only), of `Phi_edge(e)` is `(1 - 2 alpha) N_e`, the same on both faces. Hence the L2
projection of `lambda_h` onto the continuous space of the face shape functions is `lambda_h` itself; its nodal values are
`lambda_i` at a corner and `(lambda_i + lambda_j)/4 + lambda_e/2` at the mid-edge node of the edge `(i, j)` (`Phi_corner = 1`,
`Phi_edge = 0` at a corner; `Phi_corner(i) = alpha`, `Phi_edge = 1 - 2 alpha` at a mid-edge node). The reading of this item is
the first option of the acceptance, the values at the load-rule points; the projection is not built (it adds nothing).

### D3. The API (c2, c5)

Additive, in its own blocks of `contact.rs`:

    #[derive(Debug, Clone, PartialEq)]
    pub struct RecoveredTraction {
        pub pair: usize,           // the pair index
        pub face: FaceRef,         // the contactor face
        pub local: [f64; 2],       // the load-rule point on it
        pub position: Vec3,        // X(zeta_g) = sum_b N_b X_b, reference configuration
        pub weight: f64,           // W_g = w_g |X_{,u} x X_{,v}| (ADR 0551 D2)
        pub pressure: f64,         // p_g = sum_a (t_a/A_a) Phi_a(zeta_g), Pa (nominal)
    }

    impl ContactSet {
        pub fn recovered_tractions(&self, nodes: &[ContactNode]) -> Result<Vec<RecoveredTraction>>
    }

`nodes` is a list of contactor-node states (`ContactResult::nodes`, `ContactStep::nodes` or `ContactSet::evaluate`); the
`lambda_a` are their `force / area` looked up by `(pair, node)`. The output has one entry per load-rule point of every mortar
pair, pairs in order, faces in set order, points in rule order (the order of ADR 0551's points); pairs that are not mortar
pairs give no entries. Refused: a mortar node missing from `nodes`, or a `force` or `area` not finite (key `"contact nodes"`).
`weight` and `position` are recomputed from the reference nodes and `face_rule` / `face_shape_functions` exactly as in ADR 0551's
construction. Nothing in the solve paths calls it: every existing result is unchanged (c5).

### D4. The Hertz reading (c3)

HS20m and HS10m are ADR 0551's set-ups exactly (D6 there: the same mesh, constraints, penalty `100 E/h`, 1 step, tolerance
`1e-8`, Newton `NewtonOptions::new(1).max_iterations(30)`, the mortar contactor, the default factor, the linear body). The Hertz
pressure `p = p_0 sqrt(1 - r^2/a^2)` makes `p^2` linear in `r^2` (Johnson, Contact Mechanics, 1985, ch. 4, from memory; the
formulas used are those of `f601_common::hertz_theory`, derived in ADR 0540). The solve
is the same code with the same inputs, so it reproduces ADR 0551's solve (augmentations, `F_fe`, the approach) bit for bit;
only the reading changes. A second contact set `ContactSet::new(model, [the pair], options)` (deterministic, the same points)
gives `recovered_tractions(&r.nodes)`. The reading is ADR 0540's (`hertz_read`) with the points in place of the nodes, fixed
here: over the points with `p_g >= 0.5 max_g p_g`, unit weights, the least-squares fit `p^2 = c_0 + c_1 r^2` with `r^2 = X_1^2 +
X_2^2` of the point's reference position; `a_fe = sqrt(-c_0/c_1)`, the peak `sqrt(c_0)`; against `hertz_theory(F_fe)` with
`F_fe = 4 |sum of the z reactions|` (ADR 0551 D6); the approach error is `hertz_theory`'s (it depends on `F_fe` only).

### D5. Patch tests (c4)

PT10m and PT20m are ADR 0551's (`f623_common::patch_model`, `p = 1e8`, `eps = 100 E/0.5`, 2 steps, tolerance `1e-13`). Every node
has `lambda_a = p` up to ADR 0551's measured rounding (`2.8e-14`, `5.9e-14`), so by the partition of unity `sum_a Phi_a = 1`
(ADR 0551 D1) `p_g = p` at every point. The total Lagrangian versions (PT10t, PT20t of ADR 0551) are recovered as well (cheap,
same argument).

## Arithmetic done before the code (disclosed)

1. **Point counts**: PT20m `upper.zmin` 2 x 2 Quad8 faces x 9 = 36 points; PT10m 8 Tri6 faces x 9 = 72 (ADR 0540 measured 72);
   HS20m 729 and HS10m 1152 points (ADR 0551's recorded counts of the load-rule points); M20s, M10s one face, 9 points each.
2. **Patch**: `|p_g/p - 1| <= sum_a Phi_a |lambda_a/p - 1| + |sum_a Phi_a - 1|`; `sum |Phi_a| <= 1 + 2 max|N_corner<0|`, about 1.3
   at the rule points, times `6e-14`, plus the partition-of-unity rounding `1e-16`: below `1e-13`. Limit `1e-8` (acceptance).
3. **Force identity FW**: `sum_g W_g p_g = sum_a lambda_a sum_g Phi_a(zeta_g) W_g = sum_a lambda_a A_a = sum_a t_a` (the areas
   are the same sums, ADR 0551 D2). Rounding: sums of a few thousand terms of one sign, `1e-14` relative. Limit `1e-12`.
4. **Point values PV**: `p_g` against an independent evaluation in the test (the face nodes from `SolidMesh::face_nodes`, `N` from
   `loads::face_shape_functions`, `Phi` coded again in the test from ADR 0551 D1, `lambda_a = force/area` of the evaluated nodes):
   the same operations in a possibly different order, `1e-15` relative; limit `1e-13` relative to `max |p|`. The positions
   (coordinates of order 1) `1e-15`, limit `1e-13` absolute; the weights `1e-15` relative, limit `1e-13`.
5. **Hertz**: the solve is ADR 0551's: approach error 1.18e-2 (HS20m) and 1.40e-2 (HS10m), the gap tolerance `1e-8` reached
   (10 and 14 augmentations). The peak and radius change with the reading. From ADR 0551's summary: on Hex20 the coefficients
   oscillate by a few percent (corner fit 0.113, mid-edge 0.132, all 0.121 against `a_H` 0.1006); the field at a mid-edge node is
   `lambda_e/2 + (lambda_i + lambda_j)/4` (D2), the mid-edge excursion halved, and at interior points a weighted mean with
   positive and negative `N` contributions. Estimate HS20m: radius error 0.05 to 0.15 (a pass likely but not sure), peak within
   0.05. On Tet10 the mid-edge coefficients range 0.033 to 0.100 where Hertz gives 0.035 to 0.045; halved, the field still swings
   by about `+-0.015` near the edge against a local value near 0.04, and the `split_hexahedra` diagonals are not symmetric about
   the axis. Estimate HS10m: radius error 0.1 to more than 0.5 (uncertain; a miss is as likely as a pass), peak within 0.10.
   Run time: about 110 s and 140 s per solve (ADR 0551); heavy records (D6).
6. **NC-n** (negative control): ADR 0551's nodal reading `hertz_read` of the same solve, bit-identical solve: radius 2.07e-1
   (HS20m), 4.89 (HS10m), both above 0.15.

## Checks (limits fixed now)

Every aggregated error is the maximum of finite values; a non-finite value is a miss of its check. Relative errors are
`|x/x_ref - 1|`.

| id | clause | quantity | limit | estimate |
|---|---|---|---|---|
| HS20r | c3 | HS20m (D4) read from the recovered traction: the solve succeeds with the gap tolerance `1e-8`; contact radius, peak, approach against Hertz at `F_fe`; heavy record (ignored), run once | succeeds; `0.15`, `0.20`, `0.10` | arithmetic 5 |
| HS10r | c3 | the same for HS10m | succeeds; `0.15`, `0.20`, `0.10` | arithmetic 5 |
| PT20r | c4 | PT20m: `|p_g/p - 1|` at every point; the number of points | `<= 1e-8`; 36 | arithmetic 2 |
| PT10r | c4 | PT10m: as PT20r | `<= 1e-8`; 72 | arithmetic 2 |
| PT20rt, PT10rt | (c4) | the same on PT20t and PT10t (total Lagrangian body) | `<= 1e-8`; 36, 72 | arithmetic 2 |
| RP | c2 | per point: `pair`, `face`, `local` equal (bits) to the face's load-rule points in set and rule order; `position`, `weight` against the test's evaluation (PT20m, PT10m, M20s, M10s) | equal; `1e-13` abs, `1e-13` rel | arithmetic 4 |
| PV | c2 | M20s and M10s (ADR 0551's sphere models, multipliers `0.1 + 0.01 k`, the state `t1_state`, `evaluate`): `p_g` against the test's own `sum_a (t_a/A_a) Phi_a`, over `max |p|` | `<= 1e-13` | arithmetic 4 |
| FW | c2 | `sum_g W_g p_g` against `sum_a t_a` on PT20m, PT10m, M20s, M10s, HS20r, HS10r | `<= 1e-12` relative | arithmetic 3 |
| NR | c2, c5 | `recovered_tractions` of a Gauss-point set (the default contactor of PT20m's pair) is empty; with a mortar node removed from `nodes` it is refused with the key `"contact nodes"` | empty; refused | D3 |
| B | c5 | every test of `rcfd-fem` (the default-bits records `sa_contact_default_bits.rs`, `sa_contact_project_f604_default_bits.rs`, `sa_contact_f615_default_bits.rs`, `sa_contact_f601_default_bits.rs`, `sa_contact_f623_default_bits.rs` included), `cargo test -p rcfd-api --release --test baseline_hashes`, `cargo test -p rcfd-cli --release --test docs_site` | pass | D3 |

Recorded without a limit: the number of points, active points (`p_g > 0`) and the points used by the fit for HS20r and HS10r,
`F_fe`, `a_fe`, `a_H`, the peak, the largest and smallest `p_g`, the run times, and the per-point table `(r, p_g)` of the points
with `p_g > 0` (printed in the log: the acceptance's "reported per point").

Negative control:

| id | control | expected |
|---|---|---|
| NC-n | in HS20r and HS10r, ADR 0551's nodal reading (`hertz_read` over `t_a/A_a`) of the same solve | radius error `> 0.15` each (arithmetic 6) |

### Planned mutations (on a `git archive` copy of the final tree in `f638-scratch/mut`, target `f638-scratch/target-mut`, each alone by exact text replacement in `contact.rs`, the fast test file `sa_contact_mortar_recovery_f638.rs` run once, the file restored)

| id | mutation | expected to be caught by |
|---|---|---|
| MR1 | `Phi` replaced by `N` (the face's shape-function values) in the recovery | PV, FW (M20s, M10s) |
| MR2 | `lambda_a = t_a` (not divided by `A_a`) | PT20r, PT10r, PV |
| MR3 | the reference Jacobian left out of `weight` | RP, FW |
| MR4 | `position` the first node of the face | RP |
| MR5 | a missing node taken as `lambda = 0` instead of refused | NR |

No mutation is declared equivalent.

## Decision rule

- **done**: every check met on its registered run (HS20r and HS10r included), the negative control as expected, every planned
  mutation caught by a check named for it, B unchanged, fmt and clippy clean.
- **partial**: B unchanged, PT20r, PT10r, RP, PV and FW met, and any other miss (or a check not run) diagnosed and recorded as
  a `*_known_miss` test, or a mutation not caught.
- **todo**: otherwise, in particular any change of the default bits.

## Files

`crates/rcfd-fem/src/contact.rs` (D3: the type after `ContactResult`, the method in its own `impl ContactSet` block after the
mortar helpers; the module doc gains one bullet); new tests `crates/rcfd-fem/tests/sa_contact_mortar_recovery_f638.rs` (PT20r,
PT10r, PT20rt, PT10rt, RP, PV, FW, NR; fast) and `sa_contact_mortar_recovery_hertz_f638.rs` (HS20r, HS10r with FW and NC-n;
ignored heavy records, run once). The shared helpers of `tests/f601_common` and `tests/f623_common` are used as they are.

## Limits (not covered)

- A reading, not a change of the contactor: the weighted gaps and the forces are ADR 0551's; the recovered traction inherits
  their oscillation where the coefficients oscillate (D1). Dual (biorthogonal) weighting functions, which would make the
  coefficients local averages, are not built.
- Nominal traction on the reference face (as ADR 0551's weights); no Cauchy traction on the current face.
- No projection onto other spaces (D2: onto the face's shape functions it is the identity), no smoothing, no superconvergent
  patch recovery.
- No case-file key, no CLI or API surface outside `rcfd_fem::contact`.

## Open questions taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. **Values at the points or the projection**: options (a) the values at the load-rule points, (b) the L2 projection onto the
   face's shape functions. Taken (a): by D2 (b) is the same field, so (a) is the acceptance's first option at no extra code.
2. **Where it lives**: options (a) a method of `ContactSet` taking the node states, (b) a new field of `ContactResult`. Taken (a):
   it leaves `ContactResult` and every default result unchanged and avoids the solve paths that F-639 changes.

Open (not taken): switching the default contactor of quadratic faces (ADR 0551 open question 3).

## Addendum 1: results (2026-11-15)

Each registered run made once (logs in `w49i-work/f638-scratch`: `run-recovery.log`, `run-hertz.log`, `run-mut.log` and
`mut-MR*.log`, `clippy.log`, `run-fem-full.log`, `run-baseline.log`, `run-docs.log`). No limit changed, no amendment. The
library and the tests were committed (48c7fe6a) before the runs.

| id | limit | measured | met |
|---|---|---|---|
| HS20r | succeeds; `0.15`, `0.20`, `0.10` | succeeds with the gap tolerance `1e-8` (10 augmentations, 68 s, penetration `9.7e-9`); radius 3.54e-2 (`a_fe` 0.10415, `a_H` 0.10059), peak 7.38e-3, approach 1.18e-2 | yes |
| HS10r | succeeds; `0.15`, `0.20`, `0.10` | succeeds (14 augmentations, 101 s, penetration `9.5e-9`); radius 7.43e-2 (`a_fe` 0.10819, `a_H` 0.10071), peak 4.12e-2, approach 1.40e-2 | yes |
| PT20r | `1e-8`; 36 | 3.4e-14; 36 | yes |
| PT10r | `1e-8`; 72 | 2.5e-14; 72 | yes |
| PT20rt, PT10rt | `1e-8`; 36, 72 | 1.5e-14, 1.6e-14; 36, 72 | yes |
| RP | equal; `1e-13`, `1e-13` | order equal on PT20m, PT10m, M20s, M10s (9 points each on M20s, M10s); position and weight differences 0 | yes |
| PV | `1e-13` | 0 (M20s, M10s; 8 of 8 and 6 of 6 nodes active) | yes |
| FW | `1e-12` | PT20r 1.1e-16, PT10r 3.3e-16, M20s 1.1e-16, M10s 0, HS20r 2.2e-16, HS10r 2.2e-16 | yes |
| NR | empty; refused | 0 entries; refused with `"contact nodes"` | yes |
| B | pass | see "Crate runs" | yes |
| NC-n | `> 0.15` | 2.07e-1 (HS20r), 4.89 (HS10r): ADR 0551's misses reproduced from the same solve | yes |

Recorded (no limit): HS20r 729 points, 123 with `p_g > 0`, 87 in the fit, `p_g` from `-9.9e-4` to `7.05e-2`, `F_fe`
1.491464e-3 (ADR 0551: 1.4915e-3); HS10r 1152 points, 144 with `p_g > 0`, 110 in the fit, `p_g` from 0 to `7.27e-2`, `F_fe`
1.496492e-3 (ADR 0551: 1.4965e-3). The Newton iterations per solve are ADR 0551's (`[9, 3, 2, ..]`, `[7, 3, 2, ..]`). The
per-point table `(r, p_g)` of the points with `p_g > 0` is printed in `run-hertz.log`. On HS20r a few points near the contact
edge carry a small negative recovered traction (smallest `-9.9e-4`, 1.4 percent of the peak): `lambda_h` is a quadratic field
with `lambda_a >= 0` but `Phi_a` not positive everywhere (the Quad8 corner function is negative inside the face), so the
recovered traction is not sign-preserving pointwise; the fit only uses points at half the peak or above.

The estimates of arithmetic 5 were pessimistic on Tet10 (radius 0.074 against the estimate 0.1 to more than 0.5): the
evaluation of the field at the 9 points of a face mixes all six coefficients of the face, and the mid-edge excursions cancel
more than the nodal value `lambda_e/2 + (lambda_i + lambda_j)/4` alone suggests.

### Mutations

Each alone on a `git archive` copy of 48c7fe6a (`f638-scratch/mut`, target `target-mut`), `sa_contact_mortar_recovery_f638.rs`
run once (`mutate.py`).

| id | caught by |
|---|---|
| MR1 (`Phi` replaced by `N`) | PV M20s 0.40, M10s 0.16; FW M20s 0.29, M10s 0.14 |
| MR2 (`lambda_a = t_a`) | PT20r 0.97, PT10r 0.98, PT20rt, PT10rt; PV; FW |
| MR3 (no Jacobian in `weight`) | RP weight 15.0, 3.0, 15.2, 1.8; FW |
| MR4 (`position` the first node) | RP position 0.44 to 0.48 |
| MR5 (missing node as `lambda = 0`) | NR (not refused) |

Every planned mutation was caught by a check named for it.

### Crate runs

`cargo fmt -p rcfd-fem --check`: clean. `cargo clippy -p rcfd-fem --release --all-targets -- -D warnings`: clean.
`cargo test -p rcfd-fem --release`: 1145 passed, 0 failed, 306 ignored (every default-bits record, B2 of ADR 0551 and every
`sa_contact*.rs` test included). `cargo test -p rcfd-api --release --test baseline_hashes`: 3 passed, 1 ignored (the table not
edited). `cargo test -p rcfd-cli --release --test docs_site`: 2 passed.

`crates/rcfd-fem/src/contact.rs` changed by additions only: lines 29 to 31 (module doc), 368 to 384 (`RecoveredTraction`), 584
to 645 (the `impl ContactSet` block with `recovered_tractions`); no existing line changed.

### Deviations

None. The heavy records HS20r and HS10r are ignored tests (run once, about 170 s together); the fast file covers the same
code path. Not run: the workspace test suite (the lead's gate). `docs/reference-data.md` not changed (no new reference value).

### Decision

**done** by the registered rule: every check met on its registered run (HS20r and HS10r included), the negative control as
expected, every planned mutation caught, B and `baseline_hashes` unchanged, fmt and clippy clean.

### Remaining questions

1. Making the recovered traction the pressure reported in `ContactNode::pressure` for mortar nodes. Options: (a) keep
   `t_a/A_a` there and the recovery as a separate call (now), (b) add the recovered points to `ContactResult`. (b) changes a
   public result type and the words of the default-bits records of mortar runs. Recommendation: (a), until the mortar contactor
   becomes a default (ADR 0551 open question 3, the maintainer's call).
2. Pointwise negative recovered tractions near the contact edge (above). Options: (a) accept (a reading of the discrete field),
   (b) dual or positive weighting functions in the contactor. Recommendation: (a); (b) only with a new registered item.
