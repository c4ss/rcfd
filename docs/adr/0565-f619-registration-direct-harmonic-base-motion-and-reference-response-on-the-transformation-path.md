# ADR 0565: F-619 registration: direct harmonic base motion and the reference response on the transformation path

Status: **REGISTERED 2026-11-15, written before any code of F-619 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-619 was written, compiled or run: the files under "What was read" were read, and the
arithmetic below was done by hand (no script, no rcfd run; disclosed). The default-bits record B0 is a test file written and
run on the unchanged base cf32422b after this commit and before any library code. Results go in the addenda at the end;
nothing above the addenda is changed after a run.

Roadmap item F-619 (deps F-528), the follow-up proposed by [ADR 0543](0543-f528-registration-direct-harmonic-solve-on-the-transformation-path-and-with-pre-stress.md)
(its refusal of base motion on the transformation path, D4, and its limit "the reference unknowns of the rigid sets are not
returned"). Number 0565 given by the lead. Branch `f619-work` from cf32422b, worktree `w49i-work/f619`, scratch
`w49i-work/f619-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored `*_known_miss`
test with the limit intact and the measured number); each registered run made once; defaults bit-identical; no edit of an
existing test, of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no unsafe
code; clean-room; `docs/ROADMAP.md` and `docs/roadmap.json` not edited (the entries go to the lead in the last section). Every
choice below that the maintainer would normally be asked about is taken on the maintainer's standing instruction of 2026-11-11
(take the recommendation).

## What F-619 asks

The acceptance, binding:

> Every limit not stated here is registered in the ADR before the run. A harmonic base motion on a prescribed set of a model with constraints enters x = T q + G with right-hand side T^T (F - A_ext G); the bar of ADR 0448 H6 with a tip spring equals the 1-D chain within 1e-8; the reference unknowns of the rigid sets are returned; defaults bit-identical.

Clauses: **c0** every limit registered here; **c1** a harmonic base motion on a prescribed set of a model with constraints enters
`x = T q + G`, right-hand side `T^T (F - A_ext G)`; **c2** the bar of ADR 0448 H6 with a tip spring against the 1-D chain within
`1e-8`; **c3** the reference unknowns of the rigid sets returned; **c4** defaults bit-identical.

The brief adds: audit the existing tests that assert a refusal this change would lift, before registering; if one would break,
an opt-in route that leaves it passing; learn from ADR 0543's M2 (a mutation caught only by an amended check, because the
registered catcher compared two solves of the same path): every mutation registered below names a catcher whose reference is
independent of the code mutated.

## What was read (no run)

`AGENT_RULES.md`; the brief; ADR 0543 whole (D1 to D7, the checks, amendment 1 and its diagnosis of M2, addendum 1); ADR 0448
(D2 the operator and base motion `A_FF X_F = F_F - A_FP G_P`, the response the absolute displacement with the base values at
their equations, H6 the bar with `1 m` base motion on `xmin` at `0.5 w1` against `1/cos(pi/4)`, H2 every bar response against
the 1-D chain, R7 a base motion on a free equation); `crates/rcfd-fem/src/harmonic_direct.rs` at cf32422b (1371 lines: `BaseMotion`,
`DirectHarmonicOptions`, `DirectHarmonic::{new, response, response_with_base, sweep, stress, solve}`, `transformed`,
`solve_transformed` with its refusal of base motion); `crates/rcfd-fem/src/mpc.rs` (`transformation`, `Elimination::process`:
the pivot of a row chosen from the coefficients only, the redundant / contradictory decision `|b| <= tau s_b` the only use of
the values; `eliminate`, `Transformation::{t, g, expand, transpose_times, reduce, references, independent}`, `support_triplets`,
`add_supports`); `crates/rcfd-fem/src/constraint.rs` (`prescribed_equations`: an extra prescribed value on an equation the model
already prescribes with other bits is a `ConflictingConstraint`, so the model's base equations cannot be revalued through
`extra`); ADR 0296 (`x = T q + g`, `f_r = T^T (f - K g)`, the reference unknowns, step 5 for a row whose designated unknown is
already dependent); the tests `sa_harmonic_direct.rs`, `sa_harmonic_direct_tp.rs` (helpers `split`, `merged`, `chain`,
`e_chain`, `e_same`, `all_modes`, `c_load`, `rs2_options`), `sa_harmonic_direct_tp_default_bits.rs` (the digest form).

### Audit of the existing refusals (before this registration)

`grep -rn "base motion\|transformation path\|response_with_base" crates/*/tests/` at cf32422b. The rows that assert a refusal
this item concerns:

* `sa_harmonic_direct_tp.rs::rt1_to_rt5_refusals_of_the_transformation_path`, row **RT1**: `DirectHarmonic::response_with_base`
  on the merged cantilever (rigid set and spring) with a base motion on an `xmin` node is refused, key `"base motion"`, message
  containing `"transformation path"`. Lifting that refusal in `response_with_base` would break RT1.
* `sa_harmonic_direct.rs` rows R7 (base motion on a free equation of the plain path, key `"base motion"`) and the copies in
  `sa_harmonic_direct_tp.rs`: plain path, untouched by this item.

Hence the **opt-in route** (D4): `response_with_base` keeps its code and its refusal on the transformation path (RT1 passes
unchanged); the base motion on the transformation path and the reference response are taken by a new method,
`DirectHarmonic::response_with_references`. No existing test is edited or ignored.

## Design

### D1. The base motion on the transformation path (c1)

On the transformation of ADR 0296 the unknowns `x = (u, r)` over `N = n + 6 n_R` satisfy every constraint row `C x = b`. The
model's prescribed values are zero (ADR 0543 D4); a base motion gives the prescribed equations `P` the complex amplitudes `G_P`
(zero at the prescribed equations not listed). The row values `b` are then complex: `b = b_re + i b_im` with `b_re = Re G_P`,
`b_im = Im G_P` at the prescribed rows and zero elsewhere. The elimination of ADR 0296 is linear in `b` for a fixed choice of
dependents, and that choice (the pivot of each row, the pruning of small coefficients, the Gauss-Jordan substitution) reads the
coefficients only (`Elimination::process`), so the elimination with values `b` gives the same `T` and the constant

`G = g(b_re) + i g(b_im)`, `x = T q + G` for every `x` with `C x = b`

(`g(b)` the vector `g` of `eliminate` for row values `b`; `g` linear in `b` because every constant is formed by the same linear
operations on `b` as the coefficients are on the row terms). `G` equals `G_P` at the prescribed equations (their rows come
first and have no coefficients), and carries the base motion into every dependent whose expression contained a base unknown:
a linear-constraint dependent, a rigid-set reference unknown made dependent by step 5 when the set contains a prescribed node
(D5's BM3), and so on.

The steady harmonic balance with the constraint force `R`, `A_ext X = F + R`, admissible `X = T Q + G`, and `T^T R = 0` (the
constraint force does no work on an admissible variation `delta x = T delta q`; ADR 0296, virtual work) give

`A_r(omega) Q = T^T (F - A_ext(omega) G)`, `A_r = T^T A_ext T`, `X = T Q + G`,

with `A_ext(omega)` of ADR 0543 D1 on its extended pattern (stiffness parts with their coefficients, supports `K_s`, pre-stress
`K_sigma`, `- omega^2 M`, `i omega (alpha M + beta K_beta + C_d)`; reference rows and columns zero). `A_r` is the matrix of ADR
0543 unchanged: only the right-hand side and the response change. On a model with prescribed equations only this is the
`A_FF X_F = F_F - A_FP G_P` of ADR 0448 (the rows of `T` at `P` are zero, `G` is `G_P`), the reduction used by its plain path.

Because `A_ext G` needs the unreduced matrices, the transformation path keeps the real matrices of ADR 0543 D1 on the extended
pattern (`n x n`) beside their reductions; `A_ext(omega) G` is formed entry by entry with the same coefficients as `A_r` (one
complex value per stored entry, the mesh part of `G`). A ground dashpot at a base equation acts only on its own row, which `T^T`
removes (as on the plain path, where it enters `A_PP` only); a two-node dashpot or a foundation face between a base node and a
free node couples them through `A_ext G`.

A contradictory base motion (values that no admissible `x` meets, e.g. a rigid set over base nodes moved non-rigidly) makes a
row that was redundant at zero values contradictory: the elimination refuses it as `ADR 0296` does (`key "constraint"`,
"contradicts"). After the two eliminations the code checks that their `T` and independent unknowns equal those of the stored
transformation (refused as an internal `key "base motion"` error otherwise; not expected).

### D2. Which equations take a base motion

A base motion is given at the model's prescribed equations (the rows of `crate::constraint::prescribed_equations(model, &[])`,
values zero), as on the plain path. Refused with `key "base motion"` and the node and component: an equation that is not an
unknown of the model; an equation that is not prescribed (free, or a dependent of a linear constraint or a rigid set: "is not a
prescribed equation of the model: base motion is given at prescribed equations"); an equation given twice ("given twice"); a
non-finite amplitude ("is not finite").

### D3. The reference response (c3)

The solution over all unknowns `x = T Q + G` (length `N`); its mesh part is the `HarmonicResponse`, and for each rigid set (in
the order of `Transformation::references`, i.e. of the constraints) the six complex reference unknowns `x[r0 .. r0 + 6]`: the
translation of the reference point (x, y, z) and its small rotation (x, y, z), in the convention `x(t) = Re(X e^{i omega t})`.
New public types in `harmonic_direct`:

* `HarmonicReference { constraint: usize, re: [f64; 6], im: [f64; 6] }` (`#[non_exhaustive]`), the constraint index in
  `SolidModel::constraints` and the translation then rotation;
* `DirectHarmonicSolution { response: HarmonicResponse, references: Vec<HarmonicReference> }` (`#[non_exhaustive]`).

The constraint forces are not computed (limit).

### D4. The opt-in route; the code of the default

`DirectHarmonic::response_with_references(load, base, omega) -> Result<DirectHarmonicSolution>`:

* on the transformation path: D1 to D3;
* on the plain path: the `response_with_base(load, base, omega)` response (the same call) and no references.

Everything that existed keeps its code: `response`, `response_with_base` (with its refusal of a base motion on the
transformation path, RT1 of ADR 0543), `sweep`, `stress`, `new` (except that the transformation path also keeps its
extended-pattern matrices and the prescribed-equation flags, D1, D2, which no existing method reads). The solve of the
transformation path is shared: with an empty base motion the right-hand side is formed as before (`T^T F`, no `A_ext G` term,
not even a zero one) and the response is `expand(Q)` truncated, as before, so `response` gives the bits of ADR 0543 (checked by
B0 and Z2).

`mpc.rs` gets one crate-internal function, `transformation_with_values(model, values)`, the body of `transformation(model,
&[])` with the value of each prescribed row taken from `values` where it is listed; `transformation` calls the shared body with
no values, so its rows, values and result are unchanged (B0 covers the transformation path; `baseline_hashes` is run).

### D5. Bit identity and threads

The new terms are formed sequentially (`A_ext G`, the two eliminations, `T^T`, `expand`); the assembly keeps its thread rule,
so the result is bit-identical for any thread count (T1).

## Arithmetic done beforehand (disclosed)

By hand, no script:

* Bar of ADR 0448: `E = 2e11`, `rho = 7850`, `A = 1.6e-3 m^2`, `L = 1`, 80 Hex8, `nu = 0`, `y, z` held on `all`, `x` held on
  `xmin`, `w1 = pi sqrt(E/rho)/(2L) = 7928.8 rad/s`; tip spring `k = EA/(4L) = 8e7 N/m` on each of the four `xmax` nodes (`K_t =
  3.2e8`, ADR 0543). The Hex8 bar with `nu = 0` and uniform section motion is exactly the 1-D chain of linear elements with
  consistent mass (ADR 0543 arithmetic; ADR 0448 H2 measured 5.6e-14 to 1.31e-11; ADR 0543 K1 1.4e-13 to 3.9e-13).
* The chain with base motion: element matrix `Z_e = k_e [1 -1; -1 1] + m_e [2 1; 1 2]`, `k_e = (E*/h) A` with `E* = E (1 + i eta
  + i omega beta)` (loss factor and Rayleigh `beta` on the elastic stiffness), `m_e = (rho A h / 6)(- omega^2 + i omega alpha)`,
  node 0 the base with `x_0 = G`; rows `1..N` solved with the right-hand side `f_j - Z_(j,0) G`, i.e. `- (-k_e + m_e) G` in row 1
  (Thomas algorithm, complex, in the test). The tip spring enters `K_beta` (ADR 0543 D2): `K_t (1 + i omega beta)` on the last
  diagonal. A dashpot `c` between node 0 and node 1 (four section dashpots of `c/4`) adds `i omega c [1 -1; -1 1]` to the first
  element. Both ends driven (BM4): rows `1..N-1` with `- Z_(1,0) G` and `- Z_(N-1,N) G`.
* H6 at `0.5 w1` with the tip spring: `kL = pi/4`, away from the sprung-bar roots `kL = 2.029, 4.913` (ADR 0543): the undamped
  `A_r` is regular; `1.5 w1` and `2.5 w1` (`kL = 2.356, 3.927`) likewise. Fixed-fixed (BM4) first root `kL = pi` (`2 w1`): `0.5
  w1` is away from it.
* Rigid set on the bar's `xmin` (BM3): its four nodes at `y, z in {0, B}`, reference `(0, B/2, B/2)`, offsets `d = (0, +-B/2,
  +-B/2)`. Every node component there is prescribed (`x` by the base, `y, z` held), so each rigid row meets a dependent and takes
  a reference unknown as pivot (step 5): the six reference unknowns become dependents with constants only, `t = (G, 0, 0)`, `w =
  0` (a uniform `u_x = G` of the four nodes is a translation), the six remaining rows redundant (four `x` values of a square
  moved together: `u(0,0) + u(B,B) - u(B,0) - u(0,B) = 0`). A base on one `xmin` node only breaks that sum (`G != 0`): the row is
  contradictory (RB5). The free axial unknowns are those of BM1, so BM3's response equals BM1's chain.
* Relative-motion identity (BM2): for a rigid translation `G e_y` of the whole body, `G_full` (every node's `y` and the
  reference translation `y` equal to `G`) is admissible for the merged cantilever (the rigid-set rows hold with `t = (0, G, 0)`,
  `w = 0`), and `G_full - G_P` (zero at `xmin`) lies in the range of `T`. So the response to the base `G_P` on `xmin` equals `y +
  G_full` with `y` the response, on the same model with `xmin` held, to the load `F - A_ext G_full` (`A_ext` formed in the test
  from `stiffness_with`, `add_supports`, `geometric_stiffness`, `consistent_with` and the dashpot), and the references equal
  those of `y` plus `(0, G, 0, 0, 0, 0)`: equal up to rounding. `y` is a load-only solve of ADR 0543's code (checked there
  against F-399), so the comparison is independent of the code of D1 to D3 except for the returned references of `y`.
* Scales of BM2: cantilever `hex_box([0;3], [0.6, 0.1, 0.1], [6, 2, 2])`, steel `nu = 0.3`; bending stiffness `3 E I / L^3 = 3 *
  2e11 * 8.33e-6 / 0.216 = 2.3e7 N/m`; foundation `k_f = 1e9 N/m^3` on `ymin` (area `0.06 m^2`, `6e7 N/m` in all, the same order:
  its coupling of the base edge to the free face is a visible part of `A_ext G`); base `G = (1 + 0.5 i) 1e-7 m` against a static
  tip response of about `1 N / 2.3e7 = 4e-8 m` (the same order, so neither part dwarfs the other). `K_sigma` of `sigma = 2e8`
  is about `sigma h / (E h) = 1e-3` of `K` entry by entry: its part of `A_ext G` is far above `1e-8` relative.
* Estimates: every comparison is the same reduced problem in exact arithmetic; its rounding is the condition of `A_r` (at most
  `1e5`, ADR 0448) times `eps`, about `1e-11`; the registered limit of a response comparison is `1e-8` (the acceptance's). The
  references of BM3 are constants of the elimination (a few operations on `G`): `1e-15`, limit `1e-10`. The rigid-set kinematics
  RF1 reproduce the dependents' rows of `T` from the returned references: `1e-15`, limit `1e-10`.

## Checks

Common: `e(X, Y) = max_j |X_j - Y_j| / max_j |Y_j|` over every equation compared (complex modulus). Every aggregated error is
asserted finite before its limit is tested (F1). "Chain" is the 1-D chain of the arithmetic, every axial unknown of the bar
(node 0 included, whose value must be `G`). Bar `S` = the bar of ADR 0448 with `NodalSpring("xmax", 0, 8e7)`. Merged
cantilever `C_R` = the merged model of ADR 0543 RS2 (`xmin` clamped, `RigidSet("junction", (0.3, 0.05, 0.05))`,
`NodalSpring("xmax", 2, 1e6)`). All solves of the new behaviour by `response_with_references`.

The acceptance's own checks first:

| check | clause | set-up | limit | estimate |
|---|---|---|---|---|
| A1 = BM1a | c1, c2 | ADR 0448 H6 with the tip spring: `S` undamped, base `1 m` on every `xmin` node component 0, no load, 0.5 `w1`, against the chain with base `G = 1` and tip spring `3.2e8`: `e` | `<= 1e-8` | 1e-11 |
| A2 = BM3r | c3 | `S` plus `RigidSet("xmin", (0, B/2, B/2))`, undamped, base `G = 0.6 - 0.8 i` on `xmin` component 0, 0.5 `w1`: the returned references against `t = (G, 0, 0)`, `w = 0`: `max_k |R_k - R*_k| / |G|`; one reference returned with the constraint index of the rigid set | `<= 1e-10`; as stated | 1e-15 |
| A3 = RF1 | c3 | BM2b's solution: for every node `a` of `junction`, `max |U_a - t - w x (x_a - x_R)| / max_j |U_j|` with `t, w` the returned reference | `<= 1e-10` | 1e-15 |
| A4 = B0 | c4 | the digests of cf32422b (below) | 0 differ | 0 |

Every other registered check:

| check | clause | set-up | limit | estimate |
|---|---|---|---|---|
| BM1b..c | c1 | A1 at 1.5 and 2.5 `w1` | `<= 1e-8` | 1e-11 |
| BM1d | c1 | `S`, loss factor 0.02, base `G = 1` plus the tip load (0.25 N per `xmax` node, chain tip force 1 N), 1.0 `w1` | `<= 1e-8` | 1e-11 |
| BM1e | c1 | `S`, loss factor 0.02, base `G = 0.6 - 0.8 i`, no load, 1.0 `w1` | `<= 1e-8` | 1e-11 |
| BM1f | c1 | `S`, Rayleigh `alpha = 0.02 w1`, `beta = 0.02 / w1` (spring in `K_beta`), base `G = 0.6 - 0.8 i`, 1.5 `w1` | `<= 1e-8` | 1e-11 |
| BM1g | c1 | `S` undamped plus `Dashpot::between(a, a', 0, c/4)` for each `xmin` node `a` and its `x`-neighbour `a'` (`c = 0.1 A sqrt(E rho)`), base `G = 1`, 1.0 `w1` | `<= 1e-8` | 1e-11 |
| BM2a..c | c1 | `C_R` plus `Foundation("ymin", 1e9)`, loss factor 0.02, Rayleigh `alpha = 0.02 w_1`, `beta = 0.02 / w_1`, `Dashpot::ground(J, 1, 1e3)`, `Prestress::uniform([2e8, 0, 0, 0, 0, 0])`, load of RS2 (`c_load(_, true)`), base `G = (1 + 0.5 i) 1e-7` on every `xmin` node component 1; 0.5, 1.7, 2.5 `w_1` (`w_1 = sqrt(lambda_1)` of the dense pencil `T^T (K + K_s + K_sigma) T, T^T M T` of that model): the response against the relative-motion reference `y + G_full`: `e` over every mesh equation | `<= 1e-8` | 1e-11 |
| BM2r a..c | c1, c3 | the same: the returned references against those of `y` plus `(0, G, 0, 0, 0, 0)`: `e` over the six | `<= 1e-8` | 1e-11 |
| BM3 | c1 | A2's response against the chain of A1 with `G = 0.6 - 0.8 i` | `<= 1e-8` | 1e-11 |
| BM4 | c1 | the bar of ADR 0448 (no spring), loss factor 0.02, with `Constraint::linear([(t_k, 0, 1), (b_k, 0, -1)], 0)` for each of the four `xmax` nodes `t_k` and the `xmin` node `b_k` with the same `y, z`; base `G = 0.6 - 0.8 i` on `xmin` component 0, 0.5 `w1`, against the chain with both ends at `G`: `e` | `<= 1e-8` | 1e-11 |
| Z1 | D4 | plain path (the bar of ADR 0448 without spring, loss 0.02, base `G = 1`, 0.7 `w1`): `response_with_references` against `response_with_base`: bits of `re`, `im`, and no references | 0 bits differ; empty | 0 |
| Z2 | D4 | `C_R` with `rs2_options`, the RS2 load, empty base, 1.7 `w_1` of RS2: `response_with_references(..).response` against `response(..)`: bits; one reference | 0 bits differ; one | 0 |
| T1 | D5 | BM2b with assembly threads 1 and 8: bits of the response and of the references | 0 bits differ | 0 |
| F1 | brief | every aggregated error above finite | finite | finite |
| RB1 | D2 | `C_R`, base on an `xmax` node component 0 (free): `key "base motion"`, message containing "prescribed equation" | as stated | as stated |
| RB2 | D2 | `C_R`, base on `J` component 0 (a rigid-set dependent): as RB1 | as stated | as stated |
| RB3 | D2 | `C_R`, the same `xmin` node and component given twice: `key "base motion"`, "given twice" | as stated | as stated |
| RB4 | D2 | `C_R`, a non-finite amplitude on `xmin`: `key "base motion"`, "not finite" | as stated | as stated |
| RB5 | D1 | A2's model with the base on one `xmin` node only: `key "constraint"`, "contradicts" | as stated | as stated |
| RB6 | D4 | `C_R`: `response_with_base` with a base on `xmin` still refused, `key "base motion"`, "transformation path" (ADR 0543 RT1) | as stated | as stated |

B0 (written and run on cf32422b before any library code, digests printed by that run and written into the file; only API of
cf32422b): the F-528 responses of the transformation path `response` of `S` with loss 0.02 at 1.0 `w1`; of `C_R` with
`rs2_options` and the RS2 load at 1.7 `w_1` (`w_1` taken as the constant `1700.8` rad/s, ADR 0543's measured value, to need no
eigensolution); of `C_R` with the pre-stress at 1.7 times that constant; of the foundation bar; the refusal message of RT1; the
plain path's base-motion response of the bar of `sa_harmonic_direct_tp_default_bits.rs`; and `transformation(C_R, &[])`'s `T`
values and `g` (the shared body of D4). After the library code the same file gives the same digests.

## Negative controls and mutations

Controls (each must show a difference, so that the equality checks are not vacuous):

| control | set-up | limit |
|---|---|---|
| N1 | BM1d against the same solve without the base (tip load only): `e` | `>= 1e-2` |
| N2 | BM2b: `y` (without `G_full`) against `y + G_full`: `e` | `>= 1e-2` |

Mutations, each run once on a scratch copy (`f619-scratch`, its own `CARGO_TARGET_DIR`), the new test file run, the catching
rows recorded. Each named catcher compares against a reference independent of the mutated code (the chain, the closed form of
BM3, the relative-motion solve of ADR 0543's load-only path, or the kinematics of the rigid set):

| mutation | change | expected to be caught by |
|---|---|---|
| M1 | right-hand side `T^T F` (the `A_ext G` term dropped) | BM1, BM2, BM3, BM4 |
| M2 | `T^T (F + A_ext G)` | BM1, BM2, BM3, BM4 |
| M3 | `G` not added to the response (`X = T Q`) | BM1 (node 0), BM2, A2 |
| M4 | `G` only at the base entries (not carried by the elimination; the dependents' constants zero) | A2, BM4 |
| M5 | the imaginary part of `G` dropped | BM1e, BM1f, BM2, BM3, BM4 |
| M6 | the mass omitted in `A_ext G` | BM1, BM2 |
| M7 | the dashpots omitted in `A_ext G` | BM1g |
| M8 | the Rayleigh term omitted in `A_ext G` | BM1f, BM2 |
| M9 | the supports omitted in `A_ext G` | BM2 (the foundation on `ymin` couples the base edge) |
| M10 | `K_sigma` omitted in `A_ext G` | BM2 |
| M11 | the loss-factor coefficient omitted in `A_ext G` (`K_m` real) | BM1d, BM1e, BM2 |
| M12 | the references returned as zero | A2, A3, BM2r |
| M13 | the references read one unknown later (`x[r0 + 1 ..]`) | A2, A3 |

## Decision rule

* **done**: A1 to A4 and every other registered check met, every control met, every mutation caught by at least one registered
  check (no amendment needed for it).
* **partial**: base motion and the reference response built, but a check missed (a `*_known_miss` record) or not run, a
  mutation not caught by the registered checks, or a check met only after a post-run amendment.
* **todo**: not built, or B0 missed (a default changed).

## Limits (not covered)

* The constraint forces on the transformation path are not computed (the reaction at the base, `T^T` of it, the rigid-set
  forces).
* A base motion at a prescribed equation only (not at a linear-constraint value or a reference unknown); a base motion on a
  cyclic model stays refused (the cyclic constraint is refused, ADR 0543 D4). Non-zero static constraint values stay refused.
* `response_with_base` keeps refusing base motion on the transformation path (opt-in route, D4); `sweep` has no reference or
  base variant.
* The supernodal factor stays refused on the transformation path (ADR 0543 D4).
* No case-file key: the case file has no direct harmonic analysis.

## Addendum 1: results (2026-11-15)

Code: `crates/rcfd-fem/src/harmonic_direct.rs` (module note; `HarmonicReference`, `DirectHarmonicSolution`; `Transformed`
keeps the extended-pattern matrices, the prescribed flags and the model; `dynamic_entry`, the entry of `A(omega)` of ADR 0543
with the same operations in the same order; `solve_transformed` now calls `solve_transformed_full` with no base;
`base_constant`; `response_with_references`), `crates/rcfd-fem/src/mpc.rs` (`transformation` calls `transformation_body` with
no values; `transformation_with_values`, crate-internal; a prescribed row takes its value from the map where listed). Tests:
`sa_harmonic_direct_bm_default_bits.rs` (B0, recorded on cf32422b before the library code, `f619-scratch/b0-base.log`) and
`sa_harmonic_direct_bm.rs` (every other row). No existing test edited or ignored. Each registered row was run once
(`f619-scratch/run1.log`, release profile).

| check | limit | measured | met |
|---|---|---|---|
| A1 = BM1a H6 with the tip spring, undamped, 0.5 `w1`, against the chain | `<= 1e-8` | 2.38e-13 | yes |
| A2 = BM3r references of the rigid set on the base against `t = (G, 0, 0)`, `w = 0` | `<= 1e-10` | 0 (one reference, constraint 3) | yes |
| A3 = RF1 rigid-set kinematics from the returned reference (BM2b) | `<= 1e-10` | 1.02e-17 | yes |
| A4 = B0 default bits (7 digests of cf32422b) | 0 differ | 0 | yes |
| BM1b..c undamped, 1.5 / 2.5 `w1` | `<= 1e-8` | 1.18e-13, 2.60e-13 | yes |
| BM1d loss 0.02, base and tip load, 1.0 `w1` | `<= 1e-8` | 4.49e-13 | yes |
| BM1e loss 0.02, complex base | `<= 1e-8` | 4.50e-13 | yes |
| BM1f Rayleigh, complex base, 1.5 `w1` | `<= 1e-8` | 1.67e-13 | yes |
| BM1g dashpots between the base and the next section | `<= 1e-8` | 5.92e-13 | yes |
| BM2a..c cantilever (rigid set, spring, foundation, loss, Rayleigh, dashpot, pre-stress) against the relative motion (`w_1 = 1985.6` rad/s) | `<= 1e-8` | 8.58e-13, 5.92e-14, 7.68e-14 | yes |
| BM2r a..c references against the relative motion's | `<= 1e-8` | 1.73e-12, 3.73e-14, 3.83e-14 | yes |
| BM3 rigid set on the base, response against the chain | `<= 1e-8` | 2.39e-13 | yes |
| BM4 both ends driven through linear constraints | `<= 1e-8` | 1.28e-13 | yes |
| Z1 plain path: bits of `response_with_base`, no references | 0 bits; empty | 0; empty | yes |
| Z2 transformation path without base: bits of `response`, one reference | 0 bits; one | 0; one | yes |
| T1 threads 1 and 8 (BM2b, response and references) | 0 bits | 0 | yes |
| F1 every aggregated error finite | finite | finite (asserted in every row) | yes |
| N1 BM1d against the tip load alone | `>= 1e-2` | 1.00 | yes |
| N2 BM2b `y` against `y + G_full` | `>= 1e-2` | 0.690 | yes |
| RB1, RB2 free / rigid-set dependent equation | key `base motion`, "prescribed equation" | as stated | yes |
| RB3 given twice, RB4 non-finite | as stated | as stated | yes |
| RB5 base on one node of the rigid set | key `constraint`, "contradicts" | as stated | yes |
| RB6 `response_with_base` on the transformation path (ADR 0543 RT1) | key `base motion`, "transformation path" | as stated | yes |

Mutations (`f619-scratch/mutate.py`, copy `f619-scratch/mut` from `git archive` of 4240d999, `CARGO_TARGET_DIR`
`f619-scratch/target-mut`; log `f619-scratch/mutations.log`), each run once:

| mutation | caught by | as registered? |
|---|---|---|
| M1 `A_ext G` dropped | BM1a..g, BM3, BM4, BM2a..c, BM2r a..c | yes |
| M2 `+ A_ext G` | BM1a..g, BM3, BM4, BM2a..c, BM2r a..c | yes |
| M3 `G` not added to the response | BM1a..g, BM3, A2, BM4, BM2a..c | yes |
| M4 `G` only at the base entries | A2, BM4 | yes |
| M5 `Im G` dropped | BM1e, BM1f, BM3, A2, BM4, BM2a..c, BM2r a..c | yes |
| M6 mass omitted in `A_ext G` | BM1a..g, BM3, BM4, BM2a..c, BM2r a..c | yes |
| M7 dashpots omitted in `A_ext G` | BM1g | yes |
| M8 Rayleigh omitted in `A_ext G` | BM1f, BM2a..c, BM2r a..c | yes |
| M9 supports omitted in `A_ext G` | BM2a..c, BM2r a..c | yes |
| M10 `K_sigma` omitted in `A_ext G` | BM2a..c, BM2r a..c | yes |
| M11 loss coefficient omitted in `A_ext G` | BM1d, BM1e, BM4, BM2a..c, BM2r a..c | yes |
| M12 references zero | A2, A3, BM2r a..c | yes |
| M13 references one unknown later | A2, A3, BM2r a..c | yes |

Thirteen of thirteen caught by the registered rows, each by a row whose reference is independent of the mutated code (the
lesson of ADR 0543 M2).

Also run: `cargo test -p rcfd-fem --release` (263 test binaries, 1121 passed, 0 failed, 302 ignored); `cargo test -p rcfd-api --release --test baseline_hashes` (ok, 3 passed, 1 ignored, unchanged); `cargo test -p rcfd-cli
--release --test docs_site` (ok, 2 passed); `cargo clippy -p rcfd-fem --release --all-targets -- -D warnings` (clean);
`cargo fmt -p rcfd-fem -- --check` (clean). No case file, schema or golden changed: the case file has no direct harmonic
analysis.

Deviations: (1) the mutation script stopped at M12 the first time because its search text had the indentation from before
`cargo fmt`; M12 had not been applied, so no M12 or M13 run was made then; the text was corrected and M12 and M13 were each
run once. (2) The B0 base binary was run a second time to capture the whole log (the same binary, the same digests). (3) The
transformation path of `DirectHarmonic` keeps a clone of the model (for the elimination with the base values, D1), so
`response_with_references` has the registered signature without a model argument.

**Decision by the registered rule: done.** A1 to A4 and every other registered check and control met; every mutation caught by
a registered check; no amendment.

Remaining questions:

1. The constraint forces (base reaction, rigid-set forces) on the transformation path are not computed. Options: a follow-up
   item now; or on demand. Recommendation: on demand (no consumer yet; the case file has no direct harmonic analysis).
2. `response_with_base` still refuses base motion on the transformation path (the opt-in route, kept so that ADR 0543 RT1
   holds). Options: keep; or route it to the new solve and supersede RT1 by an ignored record. Recommendation: keep until the
   case file gets a direct harmonic analysis, and decide with that item.

## For the lead: roadmap entries

Proposed status of F-619: done by the registered rule. Note:

> Structural solver, advanced tier. Follow-up of F-528 (ADR 0543, 2026-11-15). ADR 0565, 2026-11-15: rcfd_fem::harmonic_direct::DirectHarmonic::response_with_references takes a BaseMotion at the prescribed equations of a model on the transformation path (rigid sets, linear constraints, elastic supports): x = T q + G, G the constant of the ADR 0296 elimination with the real and with the imaginary base values (T unchanged; crate-internal mpc::transformation_with_values), A_r Q = T^T (F - A_ext G) with A_ext of ADR 0543 on its extended pattern (stiffness parts with their coefficients, supports, K_sigma, mass, Rayleigh, dashpots); it returns DirectHarmonicSolution, the mesh response and the six complex reference unknowns of each rigid set (HarmonicReference). response_with_base keeps its refusal on the transformation path (opt-in route; ADR 0543 RT1 unchanged). Met: the ADR 0448 H6 bar with the tip spring against the 1-D chain within 2.4e-13 (limit 1e-8; with loss factor, Rayleigh, base dashpots, complex base 1.2e-13 to 5.9e-13); a rigid set on the base nodes: chain 2.4e-13 and references equal to t = (G, 0, 0), w = 0 exactly; both ends driven through linear constraints 1.3e-13; a cantilever with rigid set, spring, foundation, loss factor, Rayleigh, dashpot and pre-stress against the relative-motion solve within 8.6e-13 (references 1.7e-12), the rigid-set kinematics of the returned references 1.0e-17; 1 and 8 threads bit-identical; defaults bit-identical (7 digests, baseline hashes). 13 of 13 mutations caught by registered checks. Not covered: constraint forces on the transformation path; no case-file key (the case file has no direct harmonic analysis).

No follow-up item is proposed (remaining question 1: the constraint forces on demand).
