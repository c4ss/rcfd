# ADR 0606: F-680 registration: the centrifugal pre-stress of a free cyclic sector (the index-0 rigid motions removed by reaction-free gauge constraints)

Status: **REGISTERED 2026-11-15, written before any code of F-680 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-680 was written and nothing was run; the arithmetic below was done by
hand (no script; disclosed), from the derivation in D1 to D3 and the numbers measured by ADR 0602. Results go in the addenda at
the end; nothing above the addenda is changed after a run.

Roadmap item F-680 (dep F-679), the follow-up proposed by
[ADR 0602](0602-f679-registration-an-independent-whirl-reference-for-spinning-beams.md) (its remaining question 1, option (b)).
Number 0606 given by the lead. Branch `f680-work` from the local main tip c64046db (which holds F-672, F-674, F-675, F-679),
worktree `w49i-work/f680`, scratch `w49i-work/f680-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored `*_known_miss`
test with the limit intact and the measured number); each registered run made once; existing tests not edited; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no
unsafe code; clean-room. The roadmap files are not edited (the entries go in "For the lead"). Every choice below that the
maintainer would normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the
recommendation).

## What F-680 asks

The acceptance, binding:

> Every limit not stated here is registered in the ADR before the run. RotatingSector::new with stress stiffening on a free sector (no constraint removing the index-0 rigid motions), behind an opt-in, solves the index-0 centrifugal static problem with the rigid motions removed and gives the pre-stress of the ADR 0602 free ring within 1e-10 of the constrained-copy route, with the whirl frequencies of ADR 0602 unchanged within 1e-10; the defaults bit-identical.

Clauses: **a0** every limit registered here; **a1** `RotatingSector::new` with stress stiffening on a free sector, behind an
opt-in; **a2** it solves the index-0 centrifugal static problem with the rigid motions removed (D1 to D3: which motions, why the
removal leaves the stress unchanged; verified by RM, OR, RX); **a3** the pre-stress of the ADR 0602 free ring within `1e-10` of
the constrained-copy route (FP1); **a4** the whirl frequencies of ADR 0602 unchanged within `1e-10` (WH); **a5** the defaults
bit-identical (DF, NC0, B1, B2, B3).

## What was read

`AGENT_RULES.md`; the brief; ADR 0602 (all: D4 the constrained copy with `t . u = 0`, `a . u = 0` at node 1, NC0, PR, AG, its
remaining question 1); ADR 0598 D1 (the opt-in routes of the rotating paths) and ADR 0561 (the shells on the rotating cyclic
paths, for the pattern of an opt-in on `RotatingOptions`). Sources at c64046db: `rotating.rs` (`RotatingOptions`, `RotatingPart::new`,
`unit_centrifugal_model`, `check_model`, `RotatingSector::new`, `with_unit_prestress`, `frame`), `cyclic.rs` (`CyclicBoundary`:
`pairs`, `axis_nodes`, `interpolated`, `two_sided`, `the_cyclic_constraint`, `boundary`), `model.rs` (`Constraint::Linear`,
`LinearTerm`, `SolidModel::constraints`, `supports`, `DofMap`), `solve.rs` (`solve_static`; reactions are filled on the
elimination path only), `geometric.rs` (`Prestress::from_static`), `loads.rs` (`nodal_load_vector`), `assemble.rs`
(`stiffness`), the test `sa_ring_whirl_beams_f679.rs` (its ring, `prestress`, `pairs`, `AG_TABLE`, `PR_TABLE`; helpers copied
where used, not edited). No other crate uses `RotatingOptions` (grep), so a new field reaches no schema, CLI or MCP golden.

## Design

### D1. The rigid motions of index 0 of a cyclic sector (derived)

The index-0 problem of a sector is the full rotor's problem restricted to the cyclically symmetric fields: `u(high) = R u(low)`
and `theta(high) = R theta(low)` for the rotation `R` by `2 pi / N` about the axis `a` through `O` (the constraint of ADR 0386,
with the rotations of ADR 0556 / 0595 for shells and beams). A rigid motion of the free rotor is `u(x) = c + w x (x - O)`,
`theta = w` (six parameters). It is cyclically symmetric if and only if for every boundary pair `x_h = O + R (x_l - O)`:
`c + w x R (x_l - O) = R c + (R w) x R (x_l - O)` and `w = R w`. The second gives `R w = w`, so `w = omega a` (`N >= 2`, `R`
fixes only the axis). Then `w x R(x_l - O) = (R w) x R(x_l - O)` and the first reduces to `c = R c`, so `c = gamma a`. Hence
**exactly two** rigid motions survive: the axial translation `r_1 = (u = a, theta = 0)` and the rotation about the axis through
`O`, `r_2 = (u = a x (x - O), theta = a)`. A rotation about a parallel axis through another point `P` is `r_2` plus the
translation `a x (O - P)`, perpendicular to `a`, which is not invariant; the in-plane translations and the rotations about
in-plane axes are rotated by `R` and violate the constraint (the mismatch of a unit in-plane vector is `|R - I| = 2 sin(pi / N)
= 0.2611` for `N = 24`, arithmetic 1). This is what ADR 0602 D4 stated without proof; RM checks it on the ring.

### D2. The load is orthogonal to both, and the stress of the singular problem is unique (derived)

The centrifugal body force at 1 rad/s is `b(x) = rho (I - a a^T)(x - O)`. Pointwise `r_1 . b = rho a . (I - a a^T)(x - O) = 0`
and `r_2 . b = rho (a x (x - O)) . (x - O)_perp = rho (a x (x - O)_perp) . (x - O)_perp = 0`. The consistent load is `f_i = int
N_i b`; a rigid motion is interpolated exactly by every element's functions (linear translations and a constant rotation for
Beam2 and the shells, linear fields for the solids), so `r^T f = int r . b = 0` element by element, at any quadrature. So `f` is
orthogonal to the null space `N = span(r_1, r_2)` of the symmetric index-0 stiffness `K_0` (assuming the sector has no other
mechanism), the singular system `K_0 u = f` has the solutions `u* + N alpha`, and because a rigid motion has zero strain the
stress, hence the pre-stress, is the same for all of them.

**Removal by two reaction-free gauge constraints.** Add two linear constraints `C^T u = 0` with `C^T N` invertible. The
constrained solution satisfies `K_0 u + C lambda = f`; multiplied by `N^T` (`N^T K_0 = 0`): `N^T C lambda = N^T f = 0`, so
`lambda = 0` **exactly**: the constraints carry no reaction, the solution is one of the free solutions, and its stress is the free
ring's. The choice of `C` changes only the rigid part (the gauge). The gauge of this ADR: one node `g` (D3), `c_1 = a` on its
translations (`c_1 . r_1 = 1`, `c_1 . r_2 = a . (a x (x_g - O)) = 0`) and `c_2 = t_g = a x (x_g - O) / rho_g` with `rho_g = |(I -
a a^T)(x_g - O)|` (`c_2 . r_1 = 0`, `c_2 . r_2 = rho_g`): `C^T N = diag(1, rho_g)`, invertible if and only if `rho_g > 0`. This
is ADR 0602 D4's copy, now built by the library at a node it chooses.

A projected solve (`(K_0 + s N N^T) u = f`, or two constraints `N^T u = 0` over every node) was considered and not taken: both
make a dense row and column of the reduced matrix (the fill of a skyline factor of the whole sector), for the same stress.

### D3. The opt-in and its scope (fixed now)

- `RotatingOptions::free_sector_prestress: bool` (default **false**) with the builder `free_sector_prestress(on)`. It is read
  only by `RotatingSector::new`, and only when `stress_stiffening` is true; `RotatingPart::new`, `with_unit_prestress` and every
  other path ignore it (documented). Without it every line runs as before (NC0 stays refused).
- With it, `RotatingSector::new` checks the model (`check_frame`, `check_model`, as `RotatingPart::new`), then refuses, with
  `FemError::InvalidValue { key: "free_sector_prestress" }`, a model with an elastic support or any constraint other than its one
  cyclic constraint (the two gauge constraints are reaction-free only when nothing else acts on `N`; whether a support already
  removes `N` is not decided here), and a model without a gauge node.
- The gauge node: among the nodes of the mesh's elements that are in none of the cyclic boundary's node lists (`pairs` both
  members, `axis_nodes`, `interpolated` both sides, `two_sided` all) and that have translation components 0, 1, 2, the one with
  the largest `rho`, the first in index order on equal values (strict `>`); refused if none or if its `rho` is not above the
  boundary's match tolerance (`CyclicBoundary::tolerance`). Its `t_g` is computed from the node's coordinates and the boundary's
  axis and origin.
- The static solve: `unit_centrifugal_model` (the copy of `RotatingPart::new`) plus `Constraint::linear` with `c_2` then with
  `c_1` (each three terms, value 0), `solve_static` with `options.static_options`, `Prestress::from_static(&copy, ..)`; then
  `RotatingPart::with_unit_prestress` on the model, as `RotatingSector::with_unit_prestress`.
- A public `rotating::free_sector_unit_static(model, options) -> Result<FreeSectorStatic>` returns the static result (on the
  model's equations), the gauge node and the pre-stress, so that the test can check RX on the library's own solution.

Only `crates/rcfd-fem/src/rotating.rs` changes: a field and a builder, a new function, and one branch at the head of
`RotatingSector::new` taken only with the option and stress stiffening on.

## Arithmetic (disclosed, by hand)

1. `2 sin(pi / 24) = 0.26105`.
2. Scales of the ADR 0602 ring (coarse `n_c = 2`, element length `L = 2 R sin(pi / 48) = 0.0327` m): `E A / L = 2e11 x 6e-4 /
   0.0327 = 3.7e9` N/m; the hoop strain at 1 rad/s `rho R^2 / E = 7850 x 0.25 / 2e11 = 9.8e-9`, radial displacement `4.9e-9` m; nodal
   centrifugal force `rho A R L = 7850 x 6e-4 x 0.5 x 0.0327 = 0.077` N. A residual computed in double: `eps |K| |u| ~ 2.2e-16 x
   3.7e9 x 4.9e-9 x (a few, the row length) ~ 1e-14` N, `~ 2e-13` of the nodal force at worst; ADR 0602 PR measured the stress
   of the copy route against numpy to `2.4e-15`.
3. Dense whirl rounding (ADR 0602 arithmetic 6 and NC1): the bound `10 n_q eps omega_max / omega_lo <= 1.1e-10` coarse, `3.7e-10`
   fine; the measured spread of a degenerate pair (NC1) `2.2e-13 .. 6.1e-12` (coarse). Two dense solves of matrices that differ
   in the last bits differ by about that measured rounding, scaled by `omega_max` (`3.56e5` coarse, `6.01e5` fine): `<= 2e-11`.
   This is the risk of WH: the acceptance's `1e-10` is below the fine mesh's bound (`3.7e-10`) but five times above the scaled
   measured rounding. If the gauge node is node 1 and the coefficients round alike, the input is the same to the last bits and the
   difference may be zero.

## Checks (limits fixed now)

The ring is the ADR 0602 free ring (its D2, copied: `R = 0.5`, oblique axis `(1, 2, 2)/3` through `(0.3, -0.2, 0.5)`, `N = 24`,
`n_c = 2` and `4`, Beam2, `CyclicOptions::new().shells(true).beams(true)`). "The copy route" is ADR 0602's `prestress(nc)`
(constraints at node 1) copied verbatim. Set F: `RotatingOptions::new(3).coriolis(true)`, the speeds `Omega_j = (j / 6)
omega_1`, `omega_1 = 312.434403687127` rad/s (ADR 0602 table), `k = 2, 3`. The opt-in sector:
`RotatingSector::new(&free, &RotatingOptions::new(3).coriolis(true).free_sector_prestress(true))`.

| Id | Clause | Check | Limit | Estimate behind it |
|---|---|---|---|---|
| FP1 | a1, a3 | both meshes: the opt-in sector's `part().unit_prestress()` against the copy route, `max |S_new - S_copy|` over every element and component over `max |S_copy|` | `<= 1e-10` | the acceptance; arithmetic 2: `<= 1e-13` |
| FP2 | a3 | both meshes: the hoop force of every element of the opt-in pre-stress over `rho A R^2` minus 1 against ADR 0602's `PR_TABLE` | `<= 1e-8` absolute | ADR 0602 PR (`3.6e-15`) |
| WH | a4 | both meshes, set F, three speeds, `k = 2, 3`: every whirl mode returned by `whirl_modes(k, Omega_j)` of the opt-in sector against the same of `with_unit_prestress(copy route)`: `max |omega_new / omega_copy - 1|`; the same count of modes and the same direction of each | `<= 1e-10`; counts and directions equal | the acceptance; arithmetic 3: `<= 2e-11` |
| AG | a4 | coarse mesh, set F, three speeds, `k = 2, 3`: the four lowest `omega` of the opt-in sector against ADR 0602's `AG_TABLE` (the independent polygon model) | `<= 1e-8` relative | ADR 0602 AG (`3.1e-12`) |
| RX | a2 | both meshes: `free_sector_unit_static` of the free sector; `K u - f` (`assemble::stiffness` of the free sector, `nodal_load_vector` of its centrifugal copy) at every equation of every node not on the cyclic boundary (the gauge node among them), `max |.|` over `max |f|`: the gauge constraints carry no reaction | `<= 1e-10` | D2: `lambda = 0` exactly; arithmetic 2: `~ 2e-13` |
| OR | a2 | coarse mesh: `|r_i^T f| / (|r_i| |f|)` for `r_1`, `r_2` of D1 (Euclidean norms over all equations of the sector) | `<= 1e-13` | D2: zero pointwise; rounding `~ sqrt(n) eps` |
| RM | a2 | coarse mesh: (i) the six rigid motions `r` of the free sector, `|K r|_inf / (|K|_inf |r|_inf)`; (ii) the cyclic mismatch `max |r(high) - R r(low)|` (translations and rotations of the pair) over `|r|_inf` | (i) `<= 1e-12`; (ii) `<= 1e-14` for `r_1`, `r_2`, `>= 0.1` for the other four | D1; arithmetic 1 (`0.261`) |
| NI | a2 | fine mesh: the copy route with the gauge at node 3 in place of node 1, the pre-stress against node 1's, as FP1 | `<= 1e-10` | D2: the gauge does not change the stress |
| GN | D3 | both meshes: the gauge node is not on the cyclic boundary and `rho_g / R - 1` | node in `1 ..= n_c - 1`; `abs <= 1e-12` | D3 |
| NC0b | a5 | `RotatingSector::new` on the free sector with stress stiffening and `free_sector_prestress(false)` | refused, `FemError::SingularStiffness` | ADR 0602 NC0 |
| NC2 | D3 | the opt-in on the coarse free sector with (a) an added `Constraint::displacement` of node 1, component 2, value 0; (b) an added `ElasticSupport` at node 1; (c) an added `Constraint::linear` at node 1 | each refused, `FemError::InvalidValue { key: "free_sector_prestress" }` | D3 |
| NC3 | a5 | coarse, `stress_stiffening(false)`: the whirl frequencies at `Omega_3`, `k = 2`, with and without the opt-in | bit-identical | D3: the option is not read |
| DF | a5 | `RotatingOptions::new(3).free_sector_prestress` | `false` | D3 |
| NC0 | a5 | ADR 0602's `nc0_the_free_ring_static_solve_is_refused`, unedited | passes | the default path unchanged |
| B1 | a5 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | the default path unchanged |
| B2 | a5 | the existing tests covering `rotating.rs`: `sa_ring_whirl_beams_f679`, `sa_rotating_cyclic_beams_f675`, `cargo test -p rcfd-fem --release --lib rotating` | pass | as B1 |
| B3 | a5 | `git diff c64046db -- crates/*/src`: `rotating.rs` only; the lines of `RotatingPart::new`, `unit_centrifugal_model`, `check_model` and the default branch of `RotatingSector::new` unchanged | as stated | D3 |

## Negative controls

NC0 and NC0b (without the option the free sector stays refused), NC2 (the opt-in refuses what it cannot guarantee), NC3 (the
option does nothing without stress stiffening), RM (ii) (the four other rigid motions are not cyclic: the gauge removes exactly
what is free).

## Mutations (on a scratch copy, own `CARGO_TARGET_DIR`)

| Id | Mutation (site, `rotating.rs`) | Expected to be caught by |
|---|---|---|
| M1 | the axial gauge constraint `c_1` not added | FP1, WH, RX (the opt-in solve refused `SingularStiffness`: the test panics) |
| M2 | the tangential gauge constraint `c_2` not added | as M1 |
| M3 | `c_2` the radial direction `(x_g - O)_perp / rho_g` in place of `t_g` (`c_2 . r_2 = 0`) | as M1 |
| M4 | `t_g` from `a x x_g` (the origin not subtracted): a direction with a radial part, `c_2 . r_2 != 0` | **not caught (equivalent by D2)**: FP1, WH, RX expected met; registered to demonstrate D2 |
| M5 | a third gauge constraint on the radial component of `g` (over-constraint) | FP1, FP2, WH, RX |
| M6 | the default `free_sector_prestress: true` | NC0, DF |
| M7 | the refusal of supports and other constraints removed | NC2 |
| M8 | the opt-in branch not taken (`RotatingSector::new` always through `RotatingPart::new`) | FP1, WH (panics) |
| M9 | the branch taken with the option regardless of `stress_stiffening` | NC3 |

Each is applied by a script to a `git archive` copy of the test commit and the new test file run once per mutation (M6 also runs
the ADR 0602 NC0 test).

## Decision rule

**done**: every check FP1 to B3 met in the registered run, every mutation except M4 caught by at least one check, M4 not
changing FP1, WH, RX beyond their limits. **partial**: any registered check missed (recorded as a `*_known_miss`), a check met
only after a post-run amendment, a check not run, or a mutation (other than M4) not caught. **todo**: FP1 and WH not run.

## Limits (what is not covered)

- Only a sector with exactly its cyclic constraint and no support: a sector with other constraints or supports is refused with
  the opt-in (deciding whether they remove `N`, or carry a reaction, is not done). A sector whose index-0 stiffness has a
  mechanism beyond `N` stays singular and is refused by the solve as before.
- `RotatingPart::new` (a free part that is not a sector: six rigid motions, and a centrifugal load with a resultant unless the
  mass centre is on the axis) is not changed; the option has no effect there.
- Only Beam2 is run (the ADR 0602 ring); D2 holds for solids and shells by the same argument but is not checked on them here.
- The static solve's own refusal threshold is unchanged; a sector that is singular for another reason is still refused.
- Indices `k = 0, 1` of the free ring (its rigid motions in the whirl) are not solved, as in ADR 0602.
