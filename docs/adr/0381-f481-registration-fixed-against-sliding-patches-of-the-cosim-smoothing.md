# ADR 0381: F-481 registration: a check that sees fixed against sliding patches of the co-simulation smoothing

Status: **REGISTERED 2026-11-13**, written before any code of F-481 and before any run of its checks. Not yet accepted by
the maintainer. Results go in the addendum at the end; nothing above the addendum is changed after a run.

Parent: [ADR 0380](0380-f444-registration-cosim-participants-from-the-case.md) (F-444: the `[cosim]` case table,
`rcfd_api::cosim_case::fluid_from_case`; its mutation MU5 and question Q3). Roadmap item F-481 (depends on F-444), size S.
Number 0381 given by the lead. Branch `f481-cosim-fixed-sliding` from main, worktree `w49i-work/f481`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here, not changed after a run; a missed limit becomes an ignored
`*_known_miss`; defaults bit-identical; no hash table or golden file is edited; no roadmap file is edited; no push.

## What was read

ADR 0380 (MU5, its diagnosis, Q3), `crates/rcfd-api/src/cosim_case.rs` (the class list built in `fluid_from_case`: the
moving patch `Prescribed`, a name in `cosim.fixed_patches` `Fixed`, every other patch `Slip`; the smoothing is
`rcfd_fv::smoothing::smooth_displacement`, one Laplace-type solve per component), `crates/rcfd-api/tests/cosim_case_f444.rs`
(the helpers of CWc). No code was compiled or run for this item before this text. Arithmetic done beforehand is written
below.

## Why MU5 was not caught, and what can see it

On CWc the wall displacement is along `x`. The component solves are independent; a `Slip` patch keeps the component along
its normal at zero, as `Fixed` does, and the tangential components have no source. So `Slip` and `Fixed` on `xmax` give the
same mesh. If the wall displacement has a component `d_y` that is tangential to `xmax`, the two classes differ: `Fixed`
imposes `d_y = 0` on `xmax` (Dirichlet), `Slip` leaves `d_y` free there (zero normal derivative, Neumann).

**First-principles estimate.** Take the smoothing as the Laplace equation for `d_y` on a box `0 <= x <= L`, `0 <= y <= W`,
with `d_y = a sin(pi y / W)` on `x = 0` and `d_y = 0` on `y = 0, W` (the slip walls `ymin`, `ymax` have `y` as normal, so
`d_y = 0` there). The mode `sin(pi y/W)` gives `d_y = a sin(pi y/W) f(x)` with `f'' = k^2 f`, `k = pi/W`. Fixed `xmax`:
`f = sinh(k(L-x))/sinh(kL)`, `f(L) = 0`. Sliding `xmax`: `f = cosh(k(L-x))/cosh(kL)`, `f(L) = 1/cosh(kL)`. With `L = W =
0.4`, `kL = pi`, `1/cosh(pi) = 0.0863`. With `a = 0.005` m the sliding patch's mid-point moves `4.3e-4` m in `y`; a
second-order discretisation of 8 cells across `y` and 8 along `x` changes this by a few per cent. (The CWs length `L = 1`
would give `1/cosh(7.85) = 8e-4`, ten times less, so the case below is shorter.)

## Set-up (fixed now)

Case `CWf`: as CWc except `mesh` `nx, ny, nz = 8, 8, 2`, `lx, ly, lz = 0.4, 0.4, 0.1`; `[cosim] patch = "xmin"`,
`smoothing_tolerance = 1e-13`, `initial_displacement` (27 wall points, ascending mesh point index) `= [0, a sin(pi j/8),
0]` with `j = round(y / 0.05)` the point's row, `a = 0.005`, `sin` set to exactly 0 for `j = 0` and `j = 8`. Two variants:
`CWf-fixed` (`fixed_patches = ["xmax"]`) and `CWf-slide` (`fixed_patches = []`). Reference mesh `M0` is `box_mesh([8,8,2],
[0.4,0.4,0.1])`; the initial mesh of a participant is `fluid.flow.geom.mesh`. `CWx-fixed` and `CWx-slide`: the same with
the displacement `[0.002, 0, 0]` at every wall point (the old set-up of CWc, an `x` displacement only).

## Registered checks

File: `crates/rcfd-api/tests/cosim_case_f481.rs`. Release. "Bits differ" counts values whose 64 bits differ.

| Id | Clause | Check | Limit | Estimate behind it |
| --- | --- | --- | --- | --- |
| S1 (set-up) | c1 | both CWf variants parse and `fluid_from_case` builds each; the wall points of the two initial meshes against `M0` plus the prescribed displacement | builds; 0 bits differ between the two variants' wall points | the wall is prescribed in both |
| F1 | c1 | the points of `xmax` of the `CWf-fixed` initial mesh against `M0` | 0 bits differ (all three components) | `Fixed` imposes exactly zero displacement: the solve's unknown is absent or its Dirichlet value is 0.0 |
| M1 | c1 | `CWf-slide` against `CWf-fixed`: number of mesh points whose coordinates differ in bits; largest `abs(dy)` between the two meshes over the points of `xmax` | `> 0` points; `>= 1e-5` m | `4.3e-4` m by the closed form above; the limit is a factor 40 below, for the discretisation |
| M2 | c1 | `CWf-slide`: the points of `xmax` keep their `x` coordinate: largest `abs(x - x_M0)` | `<= 1e-12` m | the normal component of a `Slip` patch is zero; no source of an `x` displacement exists in CWf (the wall has `dx = 0`) |
| N1 (negative control: the old set-up cannot see it) | c1 | `CWx-slide` against `CWx-fixed`: largest absolute coordinate difference over all points | `<= 1e-12` m | ADR 0380's diagnosis: the two classes give the same mesh for an `x`-only wall displacement |
| N2 (the comparison is deterministic) | c1 | `CWf-fixed` built twice: bits | 0 bits differ | the same build, the same case bytes |
| N3 (the wall displacement is seen) | c1 | `CWf-fixed` initial mesh against the `M0` plus nothing: some wall point has moved in `y` | largest `abs(dy)` over the wall `>= 1e-3` m | the amplitude `a = 5e-3` m at the middle row |
| B1 | c2 | unedited, release: `rcfd-api` `baseline_hashes`, `case_hashes_w48u`, `cosim_case_f444` (its default tests) | all pass | no library line is changed, so the case bytes without the table cannot move |

N3 exists so that M1 cannot pass because both meshes are garbage and F1 cannot pass because nothing moved.

## Mutations (scratch copy, not committed)

Each applied alone to a `git archive` copy of the final tree, exact replacement occurring once; the named tests are run once
and must fail; the file is restored and compared byte for byte.

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU5 (of ADR 0380) | the class of a name in `fixed_patches` is `Slip` instead of `Fixed` | F1 and M1 (on the first run) |
| MU9 | every patch other than the moving one is `Fixed` (a sliding patch is treated as fixed) | M1, M2 not needed |
| MU10 | `initial_displacement` is applied as its `x` component only | N3 and M1 |

## Decision rule

- **done**: S1, F1, M1, M2, N1, N2, N3, B1 met, and MU5 fails the new test on its first run, and MU9, MU10 fail theirs.
- **partial**: a check is missed (kept as `*_known_miss`, diagnosed) or a mutation is not caught.
- **todo**: the participant cannot be built on CWf.

A defective set-up (an error in CWf, not in the participant) gets an amendment committed before the corrected run; the old
test stays as an ignored record.

## Open choices taken

None that changes a default result, golden or hash table. The case shape (a shorter box than CWs) is chosen from the
estimate above and not searched.

## Limits of this registration

The `Laplacian` method only; one geometry; the initial mesh only (the same smoothing object serves every `advance`, and
`advance` is exercised by ADR 0380's checks); the `y` component only as the tangential one; no `inverse_distance` or
`inverse_volume`; no `rcfd serve` process.
