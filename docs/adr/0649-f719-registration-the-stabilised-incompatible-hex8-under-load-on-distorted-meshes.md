# ADR 0649: F-719 registration: the stabilised incompatible-modes Hex8 (LinearStress) under load on distorted meshes (the punch block and the cantilever of ADR 0636 with the interior nodes moved, and a negative control of the F-bar reference)

Status: **REGISTERED 2026-11-16, written before any code of F-719 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-719 was written, compiled or run in the repository: the files listed
under "What was read" were read, and the arithmetic below was done by hand and evaluated with a plain numpy prototype in the
scratch folder (`w49i-work/f719-scratch`; no rcfd code; disclosed, its numbers quoted below): `probe_load.py` copied unchanged
from `w49i-work/f708-scratch` (the probe of ADR 0636: total Lagrangian, neo-Hookean, Hex8 Full, F-bar, plain and stabilised
incompatible elements, Newton with element Hessians by central differences of the exact element gradient), `probe_dist.py`
(the two load cases of ADR 0636 on the distorted meshes of D2 below, the generator of D2 in Python, the smallest corner and
point Jacobian of each mesh; logs `pd_b4_0.1.log`, `pd_b4_0.25.log`, `pd_b8_0.1.log`, `pd_b8_0.25.log`, `pd_c10_0.1.log`,
`pd_c10_0.25.log`, `pd_c20_0.1.log`, `pd_c20_0.25.log`) `probe_nf1.py` (the estimate of NF1), and `probe_load2.py` with `probe_dist2.py` and `dbg.py` (arithmetic 3, P2). At amplitude 0 the probe
reproduces ADR 0636 arithmetic 2 (`R_Fbar = 0.385977`, `lowest 7.5748e-3`). Results go in the addenda at the end; nothing above
the addenda is changed after a run.

Roadmap item F-719 (deps F-708), the follow-up proposed by
[ADR 0636](0636-f708-registration-the-linear-stress-coefficient-of-the-stabilised-incompatible-hex8-under-load.md) ("For the
lead", follow-up proposed). The element under test is the incompatible-modes Hex8 at finite strain of
[ADR 0608](0608-f596-registration-incompatible-modes-hex8-at-finite-strain.md) with the opt-in stabilisation of ADR 0614 D1 and
the rule `IncompatibleStabilisation::LinearStress` of
[ADR 0630](0630-f703-registration-a-deviatoric-coefficient-for-the-stabilised-incompatible-hex8-near-incompressibility.md) D1
(rule L); the reference is the F-bar Hex8 of
[ADR 0506](0506-f587-registration-b-bar-and-f-bar-hex8-in-the-nonlinear-driver.md) (`SolidFormulation::BBar` in the nonlinear
driver). Number 0649 given by the lead. Branch `f719-work` from the local main tip d40cd665 (F-703 / ADR 0630 and F-708 / ADR
0636 merged), worktree `w49i-work/f719`, scratch `w49i-work/f719-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test edited;
no new dependency, no unsafe code, the crate graph unchanged; clean-room. Every choice below that the maintainer would normally
be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation), except the open
questions at the end.

## What F-719 asks

The acceptance, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. The confined punch block and the displacement-controlled cantilever of ADR 0636 on meshes with the interior nodes moved by 0.1 h (2U - 1) and by 0.25 h (2U - 1), rule L against the F-bar Hex8 of ADR 0506 on the same meshes, reaction and displacement within registered tolerances; the stability of the loaded state reported; a negative control of the F-bar reference within the item's own tests; the default bits unchanged.

Clauses: **c0** every limit registered here; **c1** the confined punch block of ADR 0636 on the distorted meshes; **c2** the
displacement-controlled cantilever of ADR 0636 on the distorted meshes; **c3** the two amplitudes `0.1 h` and `0.25 h`;
**c4** rule L against F-bar on the same meshes, reaction and displacement within registered tolerances; **c5** the stability
of the loaded state reported; **c6** a negative control of the F-bar reference within the item's own tests; **c7** the default
bits unchanged. The brief adds: a seeded generator for `U`, the seeds registered; tests only unless a library change is
unavoidable; negative controls and mutations registered before the run.

### Readings, registered

- **c1, c2: the set-ups.** Exactly those of ADR 0636 readings c1 and c2 (same domains, boundary conditions, material
  `neo_hookean(mu = 1e6, kappa(nu = 0.49))`, loads `delta = 0.1` and `v = 0.3`, 10 increments, the driver's defaults, the
  measures `R = -sum_punch r_z / mu`, `w_c = u_z(1, 1, 1)`, `P = sum_tip r_y / mu`, `d = -mean_tip u_x`), on both mesh levels of
  ADR 0636 (block `4^3` and `8^3`, cantilever `10 x 2 x 2` and `20 x 4 x 4`), with the nodes moved (D2). Every node that carries
  a boundary condition or a measure is on a face of the box and is not moved, so the punch, the corner and the tip are the same
  node sets as in ADR 0636.
- **c3: "interior nodes" and "h".** The interior nodes are the nodes on none of the six faces of the box (the `BOX_FACES` node
  sets; the definition of ADR 0614 c1 and of the patch of `si_eas_hg4.rs`). `h` is the element edge of the regular mesh in the
  direction of the component (`h_c = length_c / n_c`; every regular element here is a cube, so one `h` per mesh: `0.25`,
  `0.125`, `0.1`, `0.05`). Each component of each interior node is moved by `a h (2U - 1)`, `a = 0.1` or `0.25`, `U` uniform on
  `[0, 1)`. The boundary nodes stay where they are: the faces stay plane and the boundary conditions are those of ADR 0636.
  A consequence registered as a limit: on the coarse cantilever (`ny = 2`) only the 9 nodes of the centre line are interior; the
  refined cantilever moves 171 of its 525 nodes, the blocks 27 of 125 and 343 of 729.
- **c4: the comparison and the tolerances.** Element against element on the same distorted mesh, as in ADR 0636:
  `e_R = |R_L / R_Fbar - 1|`, `e_w`, `e_P`, `e_d` likewise. **Tolerances: those of ADR 0636, `0.15` on the coarse and `0.10` on
  the refined mesh, for each measure and both amplitudes.** Rationale, fixed before the refined probes had finished: the
  tolerances of ADR 0636 are the discretisation scale of the two meshes (the F-bar's own change between them, `0.122`, `0.133`,
  `0.078`); the question of F-719 is whether moving the nodes takes rule L out of that band, so the band is kept, not widened
  for the distortion. A distortion-dependent tolerance would be a tolerance found from the probe numbers. The locking control N1
  of ADR 0636 (the Full Hex8 reaction against F-bar, `>= 0.15` coarse, `>= 0.10` refined) is repeated on every distorted mesh.
- **c5: the stability of the loaded state.** At the converged loaded state of each solve of rule L and of F-bar on all eight
  distorted meshes: the number of negative eigenvalues of the free block `K_ff` of the driver's tangent (the negative pivots of
  its `L D L^T`, Sylvester's law of inertia, `rcfd_linalg::eigen::count_below` with `M = I`, shift 0), and the lowest
  eigenvalue of `K_ff` over its largest diagonal entry (`lowest_eigenpairs` with `M = I`, one pair, the default options). Limit
  for rule L: no negative eigenvalue and the ratio `> 0` (SL); F-bar's are reported. On the two coarse meshes of each amplitude
  the ratio is also computed by the dense Jacobi solver of ADR 0636 SL, and the two must agree (SLx).
- **c6: the negative control of the F-bar reference.** ADR 0636 MU24 (the F-bar ratio `a2m1` forced to 0) moved the reference
  by under two per cent and was caught only by the ADR 0506 tests; ADR 0636 remaining question 1 left open whether the item's
  own tests should see a broken reference. Reading registered: the item's own test file carries (i) **FR1**, the consistency of
  the reference at the very states it is used at: at the converged loaded F-bar state of each of the eight distorted meshes,
  every element's tangent (`hex8_force_and_tangent_at`, `BBar`) against central differences of its internal force
  (`hex8_internal_force_at`), step `1e-7`, the measure of ADR 0506 T1 (`max |K - K_fd| / max |K|` per element, worst element);
  (ii) **FR2**, the reference's values on the regular meshes against the values recorded in ADR 0636 arithmetic 2 and 3
  (`R_Fbar(4^3) = 0.385977`, `w_c = 0.072210`, `P_Fbar(10 x 2 x 2) = 6.1658e-4`, `d = 0.052429`), so a change of the reference
  that keeps it consistent is seen too; (iii) **NF1**, a negative control of FR1 itself: the Full Hex8 tangent against the
  F-bar force differences at the converged F-bar state of the coarse block at `a = 0.25` must be far outside FR1's limit (a
  wrong tangent is seen); (iv) the planned mutations MU24 and MU25 of the reference must be caught by this file.
- **c7: default bits.** F-719 changes no library file (D1): `git diff d40cd665 -- crates/*/src` empty is the primary record; the
  bit records `si_eas_hg3_bits.rs`, `si_eas_hg_default_bits.rs`, `si_eas_default_bits.rs`, `si_fbar_default_bits.rs` and
  `cargo test -p rcfd-api --release --test baseline_hashes` are run.
- **Mesh validity** (added): every distorted element is valid, the smallest determinant of the Jacobian at the eight corners
  over its value at the element centre `> 0` on every mesh (M0), and the number of moved nodes is the registered one (M1). The
  same smallest ratio printed by the probe checks that the test's mesh is the probe's (reported).

## What was read

- `AGENT_RULES.md`; the brief; ADR 0636 (whole, including Addendum 1 and its roadmap section); ADR 0630 (head); ADR 0506 (head,
  T1 and SS2 in `si_fbar.rs`); ADR 0572 and 0573 (the form of the roadmap section).
- `crates/rcfd-fem/src/nonlinear.rs` (`fbar_kinematics`, `evaluate_fbar`, `hex8_force_and_tangent_at`,
  `hex8_internal_force_at`, `tangent`, `solve`, `NonlinearResult`), `builders.rs` (`hex_box`, the node order
  `i + (nx + 1) (j + (ny + 1) k)`), `rcfd-linalg/src/eigen.rs` (`symmetric_eigen`, `count_below`, `lowest_eigenpairs`,
  `SubspaceOptions`).
- Tests: `si_eas_hg4.rs` (whole: the set-ups, the helpers, the generator), `si_fbar.rs` (T1, SS2), the logs of ADR 0636 in
  `w49i-work/f708-scratch` (`run1-default.log`, `mutate.py`, `mutations.log`).

## Arithmetic done before the code (disclosed)

1. **The meshes** (`probe_dist.py`, the generator of D2). Moved nodes and the smallest corner and 2 x 2 x 2-point Jacobian over
   the element centre's:

   | mesh | seed | moved | `a = 0.1`: corner, point | `a = 0.25`: corner, point |
   |---|---|---|---|---|
   | block `4^3` | `0xF719_0004` | 27 | `0.6977`, `0.8584` | `0.2179`, `0.6134` |
   | block `8^3` | `0xF719_0008` | 343 | `0.6648`, `0.8233` | `0.0762`, `0.5883` |
   | cantilever `10 x 2 x 2` | `0xF719_1010` | 9 | `0.8557`, `0.9273` | `0.6113`, `0.8041` |
   | cantilever `20 x 4 x 4` | `0xF719_1020` | 171 | `0.6611`, `0.8123` | `0.0773`, `0.5347` |

   At `a = 0.25` some corners of the refined meshes are near degenerate (`0.08` of the centre's) but every one is positive.
   By first principles a corner Jacobian stays positive if no node moves by more than a quarter of the edge in each component
   only in two dimensions; in three it is not guaranteed, hence M0 as a check.
2. **The block** (`R / mu`, `w_c`; `delta = 0.1`, 10 increments):

   | mesh, `a` | F-bar | rule L | Full | plain Inc | rule L x 4 (MU22) |
   |---|---|---|---|---|---|
   | `4^3`, 0 (ADR 0636) | `0.385977`, `0.072210` | `0.432843`, `0.073175` | `0.690762` | `0.407252` | `0.454276` |
   | `4^3`, 0.1 | `0.383490`, `0.071377` | `0.429575`, `0.074274` | `0.686722` | `0.405522` | `0.449473` |
   | `4^3`, 0.25 | `0.383162`, `0.070448` | `0.432116`, `0.076577` | `0.697753` | `0.409471` | `0.451745` |
   | `8^3`, 0 (ADR 0636) | `0.344038`, `0.063721` | `0.366596`, `0.066317` | `0.480858` | fails in rcfd | |
   | `8^3`, 0.1 | `0.342631`, `0.063708` | `0.365378`, `0.065934` | `0.477998` | not run | |
   | `8^3`, 0.25 | `0.342202`, `0.063899` | `0.367871`, `0.065734` | `0.480893` | not run | |

   Rule L against F-bar: `e_R = 0.1202`, `e_w = 0.0406` (`4^3`, 0.1); `0.1278`, `0.0870` (`4^3`, 0.25); `0.0664`, `0.0349` (`8^3`, 0.1); `0.0750`,
   `0.0287` (`8^3`, 0.25). The Full Hex8: `0.791`, `0.821` (`4^3`), `0.395`, `0.405` (`8^3`). Rule L x 4: `e_R = 0.172`, `0.179` (`4^3`). The distortion moves F-bar's punch reaction by under
   one per cent and rule L's by under one per cent; it moves rule L's corner bulge by `+4.6` per cent at `0.25` (F-bar's by
   `-2.4`), so `e_w` grows from `0.013` to `0.087` on the coarse block: the displacement, not the reaction, is what the distortion
   moves.
3. **The cantilever** (`P / mu`, `d`; `v = 0.3`, 10 increments):

   | mesh, `a` | F-bar | rule L | Full | plain Inc | rule L x 4 |
   |---|---|---|---|---|---|
   | `10 x 2 x 2`, 0 (ADR 0636) | `6.1658e-4`, `0.052429` | `6.5152e-4`, `0.052754` | `1.0677e-3` | `6.4860e-4` | `6.5800e-4` |
   | `10 x 2 x 2`, 0.1 | `6.1727e-4`, `0.052402` | `6.5304e-4`, `0.052708` | `1.0745e-3` | `6.4999e-4` | `6.5969e-4` |
   | `10 x 2 x 2`, 0.25 | `6.1992e-4`, `0.052369` | `6.6292e-4`, `0.052605` | `1.0951e-3` | `6.5933e-4` | `6.7031e-4` |
   | `20 x 4 x 4`, 0 (ADR 0636) | `5.7223e-4`, `0.052350` | `5.8279e-4`, `0.052478` | `7.6773e-4` | | |
   | `20 x 4 x 4`, 0.1 | `5.7284e-4`, `0.052343` | `5.8449e-4`, `0.052485` | `7.7063e-4` | not run | |
   | `20 x 4 x 4`, 0.25 | `5.7578e-4`, `0.052326` | `5.9289e-4`, `0.052474` (P2) | `7.8399e-4` | not run | |

   Rule L against F-bar: `e_P = 0.0580`, `e_d = 0.0058` (0.1); `0.0694`, `0.0045` (0.25) on `10 x 2 x 2`; `0.0203`, `0.0027` (0.1),
   `0.0297`, `0.0028` (0.25) on `20 x 4 x 4`. The Full Hex8 `0.741`, `0.767` (coarse), `0.345`, `0.362` (refined). **P2**: the
   first probe run of rule L on `20 x 4 x 4` at `0.25` aborted (`Singular matrix`: a Newton iterate made `C = F^T F` singular
   at a point, an inverted iterate, which `probe_load.py` raises instead of halving the increment as rcfd's driver does); the
   value quoted is from `probe_load2.py`, a copy of `probe_load.py` whose only change is that such an iterate or a singular
   solve is a non-converged increment (halved), log `pd2_c20_0.25_lin.log`, 61 extra increments. The traceback was taken with
   `dbg.py`. The distortion moves F-bar's tip reaction by `+0.1` to `+0.6` per cent and rule L's by `+0.2` to `+1.7`. Rule L x 4 `e_P = 0.069`, `0.081` (not a discriminator on the cantilever, as in ADR 0636).
4. **Stability** (the probe's lowest eigenvalue of the free tangent over its largest diagonal, dense): rule L `6.63e-3`,
   `6.01e-3` (block `4^3`, 0.1, 0.25), `1.57e-5`, `1.53e-5` (cantilever `10 x 2 x 2`), `1.94e-3`, `1.81e-3` (block `8^3`), `5.16e-6`, `4.54e-6` (cantilever
   `20 x 4 x 4`). F-bar `8.04e-3`, `7.12e-3`,
   `1.76e-5`, `1.67e-5`; `2.68e-3`, `2.39e-3`; `5.72e-6`, `4.82e-6`. The probe has no pivot test; the inertia count is rcfd's only.
5. **FR1** (first principles): the central difference of the force with step `h` has truncation error `h^2 |f'''| / 6`
   and rounding error about `eps |f| / h`. With `|f| <~ |K| |u_e|`, `|u_e| <~ 0.1` and `|f'''| / |K| <~ 1 / l^2` for an element
   of edge `l >= 0.05`: `h = 1e-7` gives `1e-16 * 0.1 / 1e-7 = 1e-10` and `1e-14 * 400 / 6 < 1e-12` relative, so a consistent
   tangent is within about `1e-9`. Limit `1e-6`, the limit of ADR 0506 T1. Under MU24 ADR 0506 T1 (a) measured `8.4e-4` at a
   small strain (ADR 0636 `mut-MU24-diag-si_fbar.log`).
6. **NF1** (`probe_nf1.py`): the Full Hex8 Hessian against the F-bar Hessian at the converged F-bar state of the coarse block,
   per element `max |H_full - H_fbar| / max |H_fbar|`: smallest over the elements `0.561` (`a = 0.25`; `0.567` regular), largest
   `1.18`. Limit: the smallest over the elements `>= 0.1`.
7. **FR2.** rcfd reproduced the ADR 0636 probe values to six digits (ADR 0636 Addendum 1; its run log gives `0.38597738`,
   `0.07220995`, `6.16583391e-4`, `0.0524285567` against the tabled `0.385977`, `0.072210`, `6.1658e-4`, `0.052429`: relative
   `1.0e-6`, `7e-7`, `5.5e-6`, `8.5e-6`). Limits `1e-5` (block, six digits tabled) and `2e-5` (cantilever, five digits tabled,
   a rounding of up to `8.1e-6`). Under MU24 the block reaction moved by `1.7e-2` (ADR 0636).
8. **Cost.** In rcfd release the largest solves are the `8^3` block and the `20 x 4 x 4` cantilever (about 2000 unknowns, 10
   increments, seconds each, ADR 0636); FR1 is 48 element force evaluations per element; the sparse eigen solves of SL are one
   factorisation each. The whole file is expected well under two minutes; no heavy record.

## Design

### D1. No library change

Rule L, the F-bar Hex8, the reactions, the element force and tangent and the eigen solvers exist. F-719 adds one test file.

### D2. The distorted meshes and the generator

`U` is SplitMix64 (the `Rng` of `si_eas_hg4.rs`, copied), one generator per mesh seeded with the registered seed of arithmetic 1,
drawn in node order, three draws (x, y, z) per interior node, none for a boundary node. The same seed for both amplitudes: the
`0.25` mesh is the `0.1` mesh with every node move multiplied by `2.5`. The seeds:

| mesh | seed |
|---|---|
| block `4^3` | `0xF719_0004` |
| block `8^3` | `0xF719_0008` |
| cantilever `10 x 2 x 2` | `0xF719_1010` |
| cantilever `20 x 4 x 4` | `0xF719_1020` |

The mesh is rebuilt with its node, element and surface sets (the `rebuild` of `si_eas_hg4.rs`); the punch and corner sets are
built from the coordinates of the regular mesh's `zmax` nodes before the move (they do not move).

### D3. The test file `crates/rcfd-fem/tests/si_eas_hg5.rs`

Helpers copied from `si_eas_hg4.rs` (existing tests are not edited): `Checks`, `least`, `Rng`, `rebuild`, `with_formulation`,
`boundary_nodes`, the material, the kinds `Fbar`, `L`, `Full`, `Inc`, `newton`, `nodes_where`, `reaction_sum`, the dense SL.

- `db_punch_block_distorted_against_fbar`: for `a` in `0.1, 0.25` and `n` in `4, 8`: M0, M1; F-bar, rule L, Full solved; DB,
  N1, SL (rule L, and SLx on `4^3`), FR1 at the F-bar state; F-bar's SL printed; the plain element on `4^3` printed; the
  distortion change `R(a) / R(0) - 1` and `w(a) / w(0) - 1` of F-bar and rule L printed (the regular runs made in the same test,
  report only).
- `dc_cantilever_distorted_against_fbar`: the same for the cantilever (`10 x 2 x 2`, `20 x 4 x 4`).
- `fr_fbar_reference_controls`: FR2 on the regular coarse meshes, NF1 on the coarse block at `a = 0.25`.

A solve that fails is a non-finite measure (a miss) and is printed with its error.

## Checks

Every measure is required finite before it is compared (a non-finite value or a failed solve misses).

| Id | Clause | Measure | Limit | Estimate |
|---|---|---|---|---|
| M0 | (added) | smallest corner `det J` / centre `det J`, each of the eight distorted meshes | `> 0` | arithmetic 1 (`0.076` smallest) |
| M1 | c3 | moved nodes: block `4^3`, `8^3`, cantilever `10 x 2 x 2`, `20 x 4 x 4` | `27`, `343`, `9`, `171` exactly | arithmetic 1 |
| DB1 | c1, c3, c4 | block `4^3`, `a = 0.1`: `e_R`, `e_w` | `<= 0.15` each | `0.1202`, `0.0406` |
| DB2 | c1, c3, c4 | block `4^3`, `a = 0.25`: `e_R`, `e_w` | `<= 0.15` each | `0.1278`, `0.0870` |
| DB3 | c1, c3, c4 | block `8^3`, `a = 0.1`: `e_R`, `e_w` | `<= 0.10` each | `0.0664`, `0.0349` |
| DB4 | c1, c3, c4 | block `8^3`, `a = 0.25`: `e_R`, `e_w` | `<= 0.10` each | `0.0750`, `0.0287` |
| DC1 | c2, c3, c4 | cantilever `10 x 2 x 2`, `a = 0.1`: `e_P`, `e_d` | `<= 0.15` each | `0.0580`, `0.0058` |
| DC2 | c2, c3, c4 | cantilever `10 x 2 x 2`, `a = 0.25`: `e_P`, `e_d` | `<= 0.15` each | `0.0694`, `0.0045` |
| DC3 | c2, c3, c4 | cantilever `20 x 4 x 4`, `a = 0.1`: `e_P`, `e_d` | `<= 0.10` each | `0.0203`, `0.0027` |
| DC4 | c2, c3, c4 | cantilever `20 x 4 x 4`, `a = 0.25`: `e_P`, `e_d` | `<= 0.10` each | `0.0297`, `0.0028` (P2) |
| N1 | c4 | the Full Hex8 reaction against F-bar on each of the eight meshes | `>= 0.15` coarse, `>= 0.10` refined | `0.791`, `0.821`, `0.395`, `0.405` (block); `0.741`, `0.767`, `0.345`, `0.362` (cantilever) |
| SL | c5 | rule L at the loaded state, each of the eight meshes: negative eigenvalues of `K_ff`; lowest eigenvalue / largest diagonal | `0`; `> 0` | 0; arithmetic 4 |
| SLx | c5 | the coarse meshes: `|lowest_sparse / lowest_dense - 1|` | `<= 1e-6` | rounding (both converge to `1e-10`) |
| FR1 | c6 | F-bar at its loaded state, each of the eight meshes: worst element `max |K - K_fd| / max |K|`, step `1e-7` | `<= 1e-6` | `~1e-9` (arithmetic 5) |
| FR2 | c6 | F-bar, regular coarse meshes: `|R / 0.385977 - 1|`, `|w_c / 0.072210 - 1|`; `|P / 6.1658e-4 - 1|`, `|d / 0.052429 - 1|` | `<= 1e-5`, `1e-5`; `<= 2e-5`, `2e-5` | `1.0e-6`, `7e-7`, `5.5e-6`, `8.5e-6` |
| NF1 | c6 | Full tangent against F-bar force differences, coarse block `a = 0.25`, smallest over the elements | `>= 0.1` | `0.561` |
| B5 | c7 | `git diff d40cd665 -- crates/*/src` | empty | empty |
| B6 | c7 | `si_eas_hg3_bits.rs`, `si_eas_hg_default_bits.rs`, `si_eas_default_bits.rs`, `si_fbar_default_bits.rs` | pass | |
| S5 | c7 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | |
| S6 | | `cargo fmt -p rcfd-fem --check`, `cargo clippy -p rcfd-fem --all-targets -- -D warnings` | clean | |

Reported without a limit (c5 and context): F-bar's SL on each mesh; the plain element on the coarse meshes (`e_R`, `e_w`,
`e_P`, `e_d`, or its failure); the distortion change of each measure of F-bar and rule L against the regular mesh; the Newton
increments of every solve; the corner displacement of the Full Hex8 (N2 of ADR 0636).

### Negative controls

| Id | Control | Expected |
|---|---|---|
| N1 | the Full Hex8 at `nu = 0.49` on each distorted mesh (checked above) | outside the tolerances: the comparison sees locking on a distorted mesh |
| NF1 | the Full tangent offered as the F-bar tangent (checked above) | outside FR1's limit by five orders: FR1 sees a wrong tangent of the reference |
| FR2 | the reference's regular-mesh values (checked above) | a change of the reference that stays self-consistent is seen |

### Planned mutations (on a scratch copy, each run once against the default tests of `si_eas_hg5.rs` and `si_eas_hg3.rs`)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU21 | rule L made `Off` (`rule_coefficient` returns 0 for `LinearStress`, its derivative factors 0; ADR 0636 MU21) | HF3 of `si_eas_hg3.rs`; in this file only if a plain solve fails (ADR 0636 N3), no claim |
| MU22 | rule L's coefficient and derivatives times 4 (ADR 0636 MU22) | DB1 and DB2 `e_R` (probe `0.172`, `0.179`) |
| MU24 | the F-bar ratio `a2m1` forced to 0 in `fbar_kinematics` (ADR 0636 MU24) | **FR1** and **FR2** of this file (the c6 control) |
| MU25 | the mean part of the F-bar tangent dropped (`kv[i * n + j] += mean_coefficient * m;` removed; a tangent-only mutation, the equilibrium unchanged) | **FR1** of this file; not FR2 or the load checks (same equilibrium, a solve may need more iterations) |

## Decision rule

- **done**: M0, M1, DB1 to DB4, DC1 to DC4, N1, SL, SLx, FR1, FR2, NF1, B5, B6, S5, S6 met in their single registered run;
  every planned mutation caught by at least one of the tests it is run against, and MU24 and MU25 by `si_eas_hg5.rs`; no
  amendment.
- **partial**: B5, B6, S5 met, FR1, FR2, NF1 met, but a DB or DC limit missed (recorded as `*_known_miss`, diagnosed), or N1,
  SL or SLx missed, or a mutation not caught, or a check met only after an amendment.
- **todo**: B5, B6 or S5 fails, or FR1, FR2 or NF1 missed (the reference or its control not established), or a rule L or F-bar
  solve cannot be run on a distorted mesh.

## Limits (not covered)

- One material law (neo-Hookean), one Poisson ratio (`0.49`), one load level per case, one random mesh per mesh level and seed
  (two amplitudes of the same move field); no statistics over seeds.
- The boundary nodes are not moved (the faces stay plane); on the coarse cantilever only the 9 centre-line nodes move, so the
  coarse cantilever is mildly distorted at either amplitude.
- The comparison is element against element on a mesh, not against the continuum (ADR 0636 limits); F-bar on a distorted mesh is
  a reference of the same kind as on the regular mesh, not a converged one.
- FR1 and FR2 guard the reference's consistency and its regular-mesh values; a change of the reference that is both consistent
  and leaves the regular-mesh values unchanged to `1e-5` is not seen.
- The ADR 0628 sweep and the ADR 0608 checks are not repeated (they are ADR 0636's; the code is unchanged).

## Open questions (taken or open)

1. **The tolerances of ADR 0636 kept on the distorted meshes** (reading c4): taken on the maintainer's standing instruction of
   2026-11-11 (take the recommendation).
2. **Boundary nodes fixed, interior nodes only** (reading c3): taken on the same instruction, the literal reading of the
   acceptance and the definition of ADR 0614 c1.
3. **The F-bar control by consistency and a recorded value** (reading c6) rather than by a second reference element: taken on
   the same instruction.
4. **Default.** Every rule stays opt-in; open for the maintainer, as ADR 0614, 0628, 0630, 0636.

## Amendment 1 (2026-11-16, written after the single registered run and before any further run)

The registered run (test commit 3026f1d0, log `run1-default.log` in the scratch folder) met every check except four rows of one
mesh: on the refined block `8^3` at `a = 0.25` the solve of rule L **fails**: `element 476: det F = -8.05e-2 at point 0 of an
Incompatible Hex8 element (inverted); load step 7 of 10 (load factor 0.675 to 0.678125) after 5 halvings`. DB4 `e_R`, `e_w` and
SL (both rows) on that mesh are non-finite and missed. The probe (arithmetic 2) converged on the same mesh (`0.367871`, 13
extra increments); the probe updates the modes by the linearised condensation and has no inversion test at the Newton
iterates of the modes, rcfd's driver rejects any inverted point. The plain element on the coarse block at `a = 0.25` failed the
same way (report only: element 53, load factor 0.675). Element 476 of the `8^3` mesh is `i = 4, j = 3, k = 7` (top layer,
`x in [0.5, 0.625]`, `y in [0.375, 0.5]`), element 53 of the `4^3` mesh is `i = 1, j = 1, k = 3`: both touch the corner of the
punch edge, where the strain is singular.

Records, as the house rules require: the test `db_punch_block_distorted_against_fbar` is kept unchanged as the ignored record
`db_punch_block_distorted_against_fbar_known_miss` (the limits intact, the measured values in its ignore text); no limit is
changed.

### A1.1 The met rows as the default test

`db_punch_block_distorted_met_rows`: the same code and limits for every row of the registered test except the four rows of rule
L on `8^3` at `a = 0.25` (DB4 `e_R`, `e_w`, SL negative count, SL ratio), which it prints instead of checking. It is a
regression test of rows already met (as `pt1_to_pt4_ps1_met_rows` of ADR 0506); it is not a new registered run and not counted
as one.

### A1.2 Diagnosis (registered now, no limit, run once, ignored record `a1_diagnosis_db4_rule_l`)

Question: is the failure a loss of stability of rule L under the punch edge on this mesh (a tangent with a non-positive
eigenvalue on the path), or a failure of the load stepping (an iterate that inverts the near-degenerate element from a stable
state)? Runs, each once, rule L on the `8^3` mesh at `a = 0.25`:

- **D1**: the punch moved to `0.0675` (the last converged load factor, `0.675 delta`) in 10 increments: R, `w_c`, and SL (the
  negative count and the ratio) at that state. A ratio near the registered coarse values (`~1e-3`) and no negative eigenvalue
  says the state before the failure is stable; a negative count or a ratio near 0 says it is not.
- **D2**: the punch at `0.1` in 40 increments (the driver's defaults otherwise): if it converges, R and `w_c` and their `e_R`,
  `e_w` against the F-bar values of the registered run (`0.34220247`, `0.06389947`), and SL. Expectation from the probe:
  `e_R ~ 0.075`, `e_w ~ 0.029`. This is a diagnosis only; DB4 stays a known miss whatever D2 gives (more increments are a
  different set-up, chosen after the run).
- **D3**: the corner ratio (M0's measure) of element 476 and the smallest over the mesh, printed.

Interpretation registered: D1 stable and D2 converging within the tolerances means the miss is the load stepping of the driver
on a near-degenerate element at the singular punch edge, not the element's stability; D1 unstable means rule L loses stability
under the punch edge on this mesh. The decision rule is unchanged: a DB limit missed is **partial**.
