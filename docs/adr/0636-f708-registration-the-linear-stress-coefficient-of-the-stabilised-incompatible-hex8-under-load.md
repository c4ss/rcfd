# ADR 0636: F-708 registration: the linear-stress coefficient of the stabilised incompatible-modes Hex8 under load (a confined punch block at nu = 0.49 and a bending cantilever against the F-bar Hex8)

Status: **REGISTERED 2026-11-16, written before any code of F-708 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-708 was written, compiled or run in the repository: the files listed
under "What was read" were read, and the arithmetic below was done by hand and evaluated with a plain numpy prototype in the
scratch folder (`w49i-work/f708-scratch`; no rcfd code; disclosed, its numbers quoted below): `probe_load.py` (the two load
cases of D2 and D3 for the Hex8 Full, F-bar, plain incompatible and stabilised incompatible elements, total Lagrangian,
neo-Hookean, Newton with the element Hessians by central differences of the exact element gradient, the modes condensed per
element, increments halved on failure; logs `pb_n4_d0.1.log`, `pb_n4_d0.001.log`, `pb_n8_d0.1.log`, `pc_lin_*.log`,
`pc_v0.3_10x2.log`, `pc_v0.3_20x4.log`, `pm_mu22_mu21.log`, `psl.log`, `pc_full_20x4_and_off_n8.log`) and `check_grad.py` (the
probe's element gradients against central differences of the element energies: `4.2e-10` F-bar, `2.6e-11` rule L, `9.3e-11`
rule R, `1.1e-10` Full). `probe_hg.py` is copied unchanged from `w49i-work/f703-scratch` (not used by the load probe beyond its
shape functions, which `probe_load.py` repeats). Results go in the addenda at the end; nothing above the addenda is changed after
a run.

Roadmap item F-708 (deps F-703), the follow-up proposed by [ADR 0630](0630-f703-registration-a-deviatoric-coefficient-for-the-stabilised-incompatible-hex8-near-incompressibility.md)
(remaining question 1 there, option (b), "without the bulk modulus squared", taken on the maintainer's standing instruction of
2026-11-11 (take the recommendation)). The element is the incompatible-modes Hex8 at finite strain of
[ADR 0608](0608-f596-registration-incompatible-modes-hex8-at-finite-strain.md) with the opt-in stabilisation of
[ADR 0614](0614-f685-registration-hourglass-stability-of-the-incompatible-hex8-in-compression.md) D1 and the rule
`IncompatibleStabilisation::LinearStress` of ADR 0630 D1 (rule L, `c_L = (s : s) / (G0 + |s|)`); the reference is the F-bar
Hex8 of [ADR 0506](0506-f587-registration-b-bar-and-f-bar-hex8-in-the-nonlinear-driver.md) (`SolidFormulation::BBar` in the
nonlinear driver). Number 0636 given by the lead. Branch `f708-work` from fe2e58b0 (the F-703 tip), worktree `w49i-work/f708`,
scratch `w49i-work/f708-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test edited;
no new dependency, no unsafe code, the crate graph unchanged; clean-room. Every choice below that the maintainer would normally
be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation), except the open
questions at the end.

## What F-708 asks

The acceptance, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. The LinearStress rule of ADR 0630 measured under a load against the F-bar Hex8 of ADR 0506: a nearly incompressible (nu = 0.49) block in confined compression and a bending cantilever, displacement and reaction within a registered tolerance of the F-bar result on the same and on a refined mesh; the lowest eigenvalue of the ADR 0628 sweep kept above a registered positive limit in all six cases; the ADR 0608 checks within their limits; the default bits unchanged.

Clauses: **c0** every limit registered here; **c1** rule L under a load against F-bar: a nearly incompressible (`nu = 0.49`)
block in confined compression; **c2** a bending cantilever; **c3** displacement and reaction within a registered tolerance of
the F-bar result on the same mesh; **c4** and on a refined mesh; **c5** the lowest eigenvalue of the ADR 0628 sweep (rule L) above
a registered positive limit in all six cases; **c6** the ADR 0608 checks within their limits (rule L); **c7** the default bits
unchanged. The brief adds: every rule stays opt-in.

### Readings, registered

- **c1: why a punch.** A block compressed homogeneously in a rigid die (every face on rollers, the top moved down uniformly) has
  the homogeneous solution `u = (F - I) X`, which every one of the elements reproduces exactly (the patch tests of ADR 0608 and
  ADR 0506, H3c of ADR 0630 for rule L): displacement and reaction would agree to rounding whatever the element, so the check
  could not fail (arithmetic 1). Reading registered: the block in the die is compressed by a **rigid frictionless punch on part
  of its top**, the classical inhomogeneous confined compression test of locking (a punch on a quarter of the top face of a
  block with symmetry, in the family of the 3D compression test of S. Reese, P. Wriggers and B. D. Reddy, Comput. Struct. 75
  (2000) 291-304; cited from memory, the set-up below is the repository's and nothing is taken from the paper's numbers). The
  unit cube is the quarter of a `2 x 2 x 1` block: rollers on `x = 0, 1` (`u_x = 0`), on `y = 0, 1` (`u_y = 0`), on `z = 0`
  (`u_z = 0`); the top nodes with `x <= 1/2, y <= 1/2` get `u_z = -delta`, `delta = 0.1` (the punch, free laterally); the rest of
  the top is free. Near incompressibility the volume pushed down by the punch must bulge up through the free part of the top,
  so the reaction measures the volumetric stiffness (the Full Hex8 locks: arithmetic 2) and the bulge is the displacement.
  Measures: the reaction `R = -sum_punch r_z / mu` (the reactions of the driver, `f_int - f` at the prescribed equations, over
  the punch nodes; positive in compression) and the displacement `w_c = u_z(1, 1, 1)` (the far top corner).
- **c2: the cantilever, displacement-controlled.** Under a dead tip load the reaction equals the load (statics) and could not
  fail; the reading registered is a **displacement-controlled** cantilever: the beam `[0, 1] x [0, 0.2] x [0, 0.2]` clamped at
  `x = 0` (all three components), every node of the tip face `x = 1` given `u_y = v = 0.3` (30 per cent of the length; `u_x`,
  `u_z` free), `nu = 0.49` (the material of c1, so that the comparison with F-bar is in the regime F-bar is for). Measures: the
  reaction `P = sum_tip r_y / mu` and the displacement `d = -mean_tip u_x` (the shortening of the projection, a purely
  geometrically nonlinear response). The tip face constraint holds the face's `u_y` uniform, which stiffens the response at large
  rotation (arithmetic 3); it is the same boundary value problem for both elements.
- **c3, c4: the same and the refined mesh.** Regular meshes (`hex_box`): block `4^3` (coarse) and `8^3` (refined), cantilever
  `10 x 2 x 2` and `20 x 4 x 4` (cubic elements, every direction halved). The comparison is element against element on the same
  mesh: `e_R = |R_L / R_Fbar - 1|`, `e_w = |w_L / w_Fbar - 1|`, `e_P = |P_L / P_Fbar - 1|`, `e_d = |d_L / d_Fbar - 1|`, each on
  the coarse and on the refined mesh. **The tolerance** (arithmetic 4): `0.15` on the coarse and `0.10` on the refined mesh, for
  each of the four measures. Rationale, fixed before the run: two locking-free elements may differ on a mesh by about the
  discretisation error of that mesh, which the F-bar's own change between the two meshes measures (`0.122` (R), `0.133` (w_c)
  on the block, `0.078` (P) on the cantilever); the coarse tolerance is that change with a small margin, the refined one assumes
  the difference shrinks by at least a third under halving (the punch edge is singular, so convergence is slower than first
  order). The Full Hex8 is the locking control (N1): its reaction is `0.79` (block, coarse), `0.40` (refined), `0.73`
  (cantilever, coarse) and `0.34` (refined) above F-bar, well outside the tolerances, so the check discriminates locking from discretisation.
  Reported, no limit: `R_L` on the coarse mesh against `R_Fbar` on the refined one (a reading against the better reference).
- **c5: the sweep.** ADR 0630 D6 with the rule L pencil only: the 6 x 6 x 6 distorted patch of ADR 0614 c1 (seed
  `0xF685_0614`), every boundary node prescribed, the homogeneous state at `F = diag(t, t, lambda)`, the six cases U30, C30, C45,
  C49, U45, U49, the 51 grid stretches `lambda_j = 1 - 0.01 j`, the pencil `(K_ff, L_ff)` with `L` the Full Hex8 vector
  Laplacian, eigenvalues over `mu`. **Limit `m_L >= 0.5` at every grid stretch of every case** (the limit of ADR 0614 S1, ADR
  0628 S1, ADR 0630 S1L). The same measure on the same code was run once in ADR 0630 (S1L, smallest `0.5590` at U30 0.5; rule L's
  code is unchanged since fe2e58b0), so the outcome is known in advance and disclosed; this item registers it as its own check
  and runs it once on its own branch (G5).
- **c6: the ADR 0608 checks.** The R-* copies with rule L of ADR 0630 (`si_eas_hg3_rl.rs`: SS1 to SS3, PT1 to PT4, PM, PS, T1
  to T4, X1, Q1, CB1 to CB3, CE, NT1, the limits of ADR 0608), run unchanged on this branch.
- **c7: default bits.** F-708 changes no library file (D1): `git diff fe2e58b0 -- crates/*/src` empty is the primary record; the
  bit records `si_eas_hg3_bits.rs` (ADR 0630 B3), `si_eas_hg_default_bits.rs` (ADR 0614 B1), `si_eas_default_bits.rs` (ADR 0608
  B0), `si_fbar_default_bits.rs` (ADR 0506 B0) and `cargo test -p rcfd-api --release --test baseline_hashes` are run.
- **Stability under the load** (added, not in the acceptance): at the converged loaded state of rule L on the coarse meshes, the
  lowest eigenvalue of the free block of the driver's tangent over its largest diagonal entry, `> 0` (SL).

## What was read

- `AGENT_RULES.md`; the brief; ADR 0630 (whole, including Addendum 1 and its roadmap section); ADR 0628 (head, the form of the
  sections); ADR 0506 (head, the readings); ADR 0572, 0573, 0630 (the form of the roadmap section).
- `crates/rcfd-fem/src/nl_incompatible.rs` (the rule enum, `rule_coefficient`, `penalty`), `nonlinear.rs` (`NewtonOptions`,
  `solve`, `NonlinearResult`, the F-bar kinematics), `model.rs` (`SolidResult::reactions`, `displacement`).
- Tests: `si_eas_hg3.rs` (whole), `f703/r_body.rs` (the cantilever helpers), `si_nonlinear.rs` (A3: reactions of a
  displacement-controlled solve), `sa_contact.rs` (reaction sums).

## Arithmetic done before the code (disclosed)

1. **Homogeneous confined compression is a patch test** (first principles): with every face on rollers and the top moved
   uniformly the state `u = (F - I) X`, `F = diag(1, 1, lambda)`, satisfies every boundary condition and is in equilibrium
   without body force; every element of the comparison reproduces it (the incompatible modes are zero at a homogeneous state:
   ADR 0628 arithmetic 3; F-bar: `J_bar = det F` at every point). Hence the punch (reading c1).
2. **The block** (`probe_load.py block`, `delta = 0.1`, 10 increments, `mu = 1`). `R / mu`, `w_c`:

   | mesh | F-bar | rule L | Full | rule R | plain Inc | rule L x 4 (MU22) |
   |---|---|---|---|---|---|---|
   | `4^3`, `delta = 0.001` (linear) | `0.003912`, `0.000746` | `0.004194`, `0.000719` | `0.006469`, `0.000629` | = L | = L | |
   | `4^3` | `0.385977`, `0.072210` | `0.432843`, `0.073175` | `0.690762`, `0.065300` | `0.442972`, `0.074846` | `0.407252`, `0.069365` | `0.454276`, `0.076169` |
   | `8^3` | `0.344038`, `0.063721` | `0.366596`, `0.066317` | `0.480858`, `0.070854` | not run | `0.348550`, `0.063653` | not run |

   Rule L against F-bar: `e_R = 0.1214`, `e_w = 0.0134` (coarse), `e_R = 0.0656`, `e_w = 0.0407` (refined). The F-bar's own
   change between the meshes: `0.122` (R), `0.133` (w_c). The Full Hex8: `e_R = 0.790` (coarse), `0.398` (refined) - the
   locking; its corner displacement is within `0.10` and `0.112` of F-bar's, so **`w_c` is not a locking discriminator, the
   reaction is** (reported in the limits). On the coarse mesh the plain element is closer to F-bar (`e_R = 0.055`) than rule L
   (`0.121`): about half of rule L's difference is the stiffness the stabilisation adds at this load. Rule R gives `0.148`. On the refined mesh the plain element is within `0.013` of F-bar.
3. **The cantilever** (`probe_load.py cant`, `v = 0.3`, 10 increments). `P / mu`, `d`:

   | mesh | F-bar | rule L | Full | rule R | plain Inc | rule L x 4 |
   |---|---|---|---|---|---|---|
   | `10 x 2 x 2`, `v = 1e-4` (linear) | `1.2531e-7` | `1.2840e-7` | `2.1328e-7` | = L | = L | |
   | `10 x 2 x 2` | `6.1658e-4`, `0.052429` | `6.5152e-4`, `0.052754` | `1.0677e-3`, `0.052491` | `6.5203e-4`, `0.052758` | `6.4860e-4`, `0.052744` | `6.5800e-4`, `0.052775` |
   | `20 x 4 x 4` | `5.7223e-4`, `0.052350` | `5.8279e-4`, `0.052478` | `7.6773e-4`, `0.052760` | not run | not run | not run |

   Rule L against F-bar: `e_P = 0.0567`, `e_d = 0.0062` (coarse), `e_P = 0.0185`, `e_d = 0.0024` (refined); the F-bar's own
   change `0.078` (P). The Full Hex8 `e_P = 0.732` (coarse), `0.342` (refined). The linear response (`3 E I v / L^3 = 1.19e-7` for the
   Euler-Bernoulli beam, `E = 2 mu (1 + nu)`) is matched within 5 per cent; the nonlinear `P` is about 1.6 times the linear one
   at `v = 0.3`, more than the elastica's 1.10 for a free tip: the uniform `u_y` of the tip face holds the face against the
   `(1 - cos theta) h / 2` motion of its edges at the tip rotation `theta ~ 0.4`, and the nearly incompressible tip elements
   resist that strain. It is the same boundary value problem for both elements (reading c2).
4. **The tolerance** (reading c3, c4): the largest probed difference of rule L is `0.121` (block R, coarse) against `0.15`, and
   `0.066` (block R, refined) against `0.10`. The probe's element is the repository's formulation (ADR 0630 arithmetic 1: the
   F-703 probe reproduced rcfd's eigenvalues to their printed digits; the gradients here are checked, `check_grad.py`), its
   Newton tolerance `1e-8` relative; so rcfd is expected to reproduce these numbers to about `1e-6`, and the margins are
   `0.029` and `0.034` absolute. The margin of PB1 R is small; a miss there would be recorded as a known miss.
5. **Stability** (`psl.log`): the lowest eigenvalue of the free condensed tangent over its largest diagonal at the loaded state,
   rule L: `6.47e-3` (block, coarse), `1.60e-5` (cantilever, coarse: the soft bending modes of a slender free part); F-bar
   `7.57e-3`, `1.79e-5`.
6. **Cost.** In rcfd release the largest solve is the `8^3` block (2187 unknowns, 512 elements, 10 increments): seconds. The
   sweep G5 is 6 x 51 dense 375 x 375 pencils, about a second each: about five minutes, a heavy record (`#[ignore]`, run once);
   its fast companion is the existing `hf3_fast_companion_on_a_4_cube_patch` of `si_eas_hg3.rs` (the same `lowest10` path,
   `m_L >= 0.5` on the 4^3 patch at three stretches of each case).

## Design

### D1. No library change

Rule L, the F-bar Hex8 and the reactions exist. F-708 adds tests only.

### D2. The test file `crates/rcfd-fem/tests/si_eas_hg4.rs`

Helpers copied from `si_eas_hg3.rs` (existing tests are not edited): `Checks`, `worst`, `least`, `Rng`, `rebuild`,
`with_formulation`, `boundary_nodes`, the material (`neo_hookean(mu = 1e6, kappa(nu))`, density 1000), the patch, `lateral`,
`homogeneous`, `interior`, `dense_free`, `laplacian`. The models of a load case for `X = Fbar (BBar, Off), L (Incompatible,
LinearStress), Full (Full, Off), R (Incompatible, RestStress), Inc (Incompatible, Off)`, each solved by `nonlinear::solve` with
`NewtonOptions::new(10).threads(4)` (the driver's defaults otherwise: residual and energy `1e-10`, 20 iterations, 5 halvings). A
solve that fails is a non-finite measure for a check (a miss) and is printed for a report-only row.

- `pb_punch_block_against_fbar`: the block of reading c1 on `4^3` and `8^3` (punch node set built from the `zmax` set by
  coordinates, tolerance `1e-9`); checks PB1, PB2, N1 (block), SL (block); R and Inc reported.
- `pc_cantilever_against_fbar`: the cantilever of reading c2 on `10 x 2 x 2` and `20 x 4 x 4`; checks PC1, PC2, N1
  (cantilever), SL (cantilever); R and Inc reported on the coarse mesh.
- `g5_sweep_linear_stress_heavy` (ignored, run once): the sweep of reading c5 for rule L, table `case lambda t m_L` printed.

## Checks

Every measure is required finite before it is compared (a non-finite value or a failed solve misses).

| Id | Clause | Measure | Limit | Estimate |
|---|---|---|---|---|
| PB1 | c1, c3 | block `4^3`: `e_R`, `e_w` | `<= 0.15` each | `0.1214`, `0.0134` |
| PB2 | c1, c4 | block `8^3`: `e_R`, `e_w` | `<= 0.10` each | `0.0656`, `0.0407` |
| PC1 | c2, c3 | cantilever `10 x 2 x 2`: `e_P`, `e_d` | `<= 0.15` each | `0.0567`, `0.0062` |
| PC2 | c2, c4 | cantilever `20 x 4 x 4`: `e_P`, `e_d` | `<= 0.10` each | `0.0185`, `0.0024` |
| X1 | c4 (report) | `R_L(4^3) / R_Fbar(8^3) - 1`, `P_L(10 x 2 x 2) / P_Fbar(20 x 4 x 4) - 1` | report | `+0.258`, `+0.139` |
| N1 | c3, c4 | the Full Hex8 reaction against F-bar: block `4^3`, block `8^3`, cantilever `10 x 2 x 2`, cantilever `20 x 4 x 4` | `>= 0.15`, `>= 0.10`, `>= 0.15`, `>= 0.10` | `0.790`, `0.398`, `0.732`, `0.342` |
| SL | (added) | lowest eigenvalue of the free tangent / largest diagonal, rule L at the loaded state, block `4^3`, cantilever `10 x 2 x 2` | `> 0` each | `6.5e-3`, `1.6e-5` |
| G5 | c5 | `m_L` at every grid stretch, per case (six cases, 6^3) | `>= 0.5` | `0.5590` (U30 0.5) smallest; ADR 0630 S1L |
| HF3 | c5 | the existing fast companion in `si_eas_hg3.rs` | passes | passes (ADR 0630) |
| R-*L | c6 | `si_eas_hg3_rl.rs` | passes (ADR 0608 limits) | passes (ADR 0630) |
| B5 | c7 | `git diff fe2e58b0 -- crates/*/src` | empty | empty |
| B6 | c7 | `si_eas_hg3_bits.rs`, `si_eas_hg_default_bits.rs`, `si_eas_default_bits.rs`, `si_fbar_default_bits.rs` | pass | |
| S5 | c7 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | |
| S6 | | `cargo fmt -p rcfd-fem --check`, `cargo clippy -p rcfd-fem --all-targets -- -D warnings` | clean | |

Reported without a limit: R and Inc on the coarse block and the coarse cantilever (`e_R`, `e_P`), Inc on the refined block, the
Newton increments of every solve.

### Negative controls

| Id | Control | Expected |
|---|---|---|
| N1 | the Full Hex8 at `nu = 0.49` (checked above) | outside the tolerances: volumetric locking is what the check is to see |
| N2 | `w_c` of the Full Hex8 (printed) | inside `0.15` (arithmetic 2): the corner displacement alone would not see locking |
| N3 | the plain element (Off) under the loads (printed) | converges in the probe (with halvings); within `0.055` of F-bar on the coarse block |

### Planned mutations (on a scratch copy, each run once against `si_eas_hg4.rs` (default tests), the default tests of `si_eas_hg3.rs` and `si_eas_hg3_rl.rs`)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU21 | rule L made `Off` (`rule_coefficient` returns 0 for `LinearStress`, its derivative factors 0) | HF3 (the plain element is negative at C49 0.8); **not** by PB/PC (probe: the plain element within `0.055` of F-bar), recorded as such |
| MU22 | rule L's coefficient and its derivatives times 4 | PB1 `e_R` (probe `0.177`); not by PC (`0.067`) |
| MU23 | rule L's `b (s : ds_a)(s : ds_b)` tangent term dropped | R-T1 L (ADR 0630 MU15); **not** by PB/PC (the equilibrium does not depend on the tangent) |
| MU24 | the F-bar ratio `a2m1` forced to 0 in `fbar_kinematics` (the reference made inconsistent) | PB1 or PB2 (the reference moves or fails); no number claimed |

## Decision rule

- **done**: PB1, PB2, PC1, PC2, N1, SL, G5, HF3, R-*L, B5, B6, S5, S6 met in their single registered run; every planned mutation
  caught by at least one of the tests it is run against; no amendment.
- **partial**: B5, B6, S5 and R-*L met, G5 met, but a PB or PC limit missed (recorded as `*_known_miss`, diagnosed), or N1 or SL
  missed, or a mutation not caught, or a check met only after an amendment.
- **todo**: B5, B6 or S5 fails, or R-*L or G5 missed, or a rule L solve cannot be run.

## Limits (not covered)

- One material law (neo-Hookean), one Poisson ratio (`0.49`), one load level per case (`delta = 0.1`, `v = 0.3`), regular meshes
  only (the sweep's patch is distorted, the load cases are not), two mesh levels; no tension, no shear load, no contact punch
  (the punch is a prescribed displacement), no history material.
- The comparison is element against element on a mesh, not against the continuum: F-bar is not a converged reference (its own
  change between the meshes is 8 to 13 per cent, arithmetic 2, 3); the punch edge is singular.
- The corner displacement does not discriminate locking (N2); the reactions do.
- The tip face constraint of the cantilever stiffens its nonlinear response (arithmetic 3); not an elastica comparison (that is
  R-CB of ADR 0608, in c6).
- The stabilisation's own stiffness under load is visible (arithmetic 2: about half of rule L's difference on the coarse block)
  and reported, not limited separately.

## Open questions (taken or open)

1. **The reading of "without the bulk modulus"** (ADR 0630 remaining question 1): option (b), rule L as the candidate, taken on
   the maintainer's standing instruction of 2026-11-11 (take the recommendation), as the brief states.
2. **The confined block** loaded by a punch rather than homogeneously (arithmetic 1), and **the cantilever displacement-controlled**
   (reading c2): taken on the same instruction.
3. **Regular meshes for the load cases**: taken on the same instruction (the distortion is covered by the sweep and by R-*L).
4. **Default.** Every rule stays opt-in; open for the maintainer, as ADR 0614, 0628, 0630.

## Addendum 1: results (2026-11-16)

Every registered check was run once, on the test commit 00d3351e (logs in the item's scratch directory: `run1-default.log`,
`run1-heavy.log`, `run1-b6-rl-hf3.log`, `s5-baseline.log`, `s6-clippy.log`, `s6-fmt.log`, `mutations.log`, `mut-MU*.log`). No
limit, set-up, seed or measure was changed after a run; no amendment was written. rcfd reproduced the probe of arithmetic 2 and 3
to four or more digits in every rule L, F-bar and Full row.

### Results

| Id | Limit | Measured | Met |
|---|---|---|---|
| PB1 `e_R`, `e_w` (block `4^3`) | `<= 0.15` | `0.1214`, `0.0134` (`R_L = 0.43284`, `R_Fbar = 0.38598`) | yes |
| PB2 `e_R`, `e_w` (block `8^3`) | `<= 0.10` | `0.0656`, `0.0407` (`R_L = 0.36660`, `R_Fbar = 0.34404`) | yes |
| PC1 `e_P`, `e_d` (cantilever `10 x 2 x 2`) | `<= 0.15` | `0.0567`, `0.0062` | yes |
| PC2 `e_P`, `e_d` (cantilever `20 x 4 x 4`) | `<= 0.10` | `0.0185`, `0.0024` | yes |
| X1 (report) | report | block `R_L(4^3) / R_Fbar(8^3) - 1 = +0.258`; cantilever `+0.139` | reported |
| N1 Full against F-bar | `>= 0.15`, `0.10`, `0.15`, `0.10` | `0.790`, `0.398` (block); `0.732`, `0.342` (cantilever) | yes |
| SL block `4^3`, cantilever `10 x 2 x 2` | `> 0` | `6.467e-3`, `1.601e-5` | yes |
| G5 `m_L`, six cases | `>= 0.5` | U30 `0.5590` (at 0.50), C30 `0.7272` (0.50), C45 `1.0051` (1.00), C49 `1.0555` (1.00), U45 `1.0032` (0.74), U49 `1.0553` (0.96) | yes, all six |
| HF3 | passes | passes | yes |
| R-*L (`si_eas_hg3_rl.rs`) | ADR 0608 limits | 6 of 6 tests pass | yes |
| B5 | empty | `git diff fe2e58b0 -- crates/*/src`: 0 lines | yes |
| B6 | pass | `si_eas_hg3_bits`, `si_eas_hg_default_bits`, `si_eas_default_bits`, `si_fbar_default_bits` pass | yes |
| S5 | pass | 3 passed, 1 ignored | yes |
| S6 | clean | `cargo fmt -p rcfd-fem --check` and `cargo clippy -p rcfd-fem --all-targets -- -D warnings` clean | yes |

Reported (no limit): on the coarse block rule R `e_R = 0.148`, `e_w = 0.036`, the plain element `0.055`, `0.039`; on the
coarse cantilever rule R `e_P = 0.0575`, plain `0.0519`. N2: `w_c` of the Full Hex8 within `0.096` (coarse) and `0.112`
(refined) of F-bar, as registered (the corner displacement does not see locking; the reaction does).

### Deviations

- **N3 (the plain element under the load).** Registered expectation "converges in the probe". In rcfd the plain element on the
  refined block **fails**: at load factor `0.93` the driver's factor finds a negative pivot (`-1.13e6` against a diagonal
  `3.62e6`) after five halvings, so the tangent of the plain element loses positive definiteness under the punch: the
  hourglass instability of ADR 0614 and 0628 under a real load. The probe's dense solve has no pivot test, which is why it
  reported a number (`0.34855`, `pc_full_20x4_and_off_n8.log`). Rule L on the same mesh converges in 10 increments with no
  halving. Recorded as a deviation of an expectation, not a check.
- **MU21** was registered as "not caught by PB/PC"; it is also caught by PB2 (the same failure as N3).

### Mutations (scratch copy, the default tests of `si_eas_hg4.rs`, `si_eas_hg3.rs`, `si_eas_hg3_rl.rs`, each run once)

| Id | Caught | By |
|---|---|---|
| MU21 (rule L made `Off`) | yes | HF3 (`m_L` down to `-139` at C49 0.5), S3b L, E1L, and PB2 (the refined block solve fails, as N3) |
| MU22 (rule L times 4) | yes | PB1 `e_R = 0.1770` (probe `0.177`), HF3 O2 (`m_L - m_R` up to `4.2e-2`), S3b L, E1L; not PC (as registered) |
| MU23 (`b` term dropped) | yes | R-T1 L (b) `6.1e-5`, (e) `4.8e-5`, R-T3 L, R-Q1 L (7 of 14); not PB/PC (as registered) |
| MU24 (F-bar `a2m1 = 0`) | **no** | PB1 `e_R` `0.1214 -> 0.1414`, PB2 `0.0656 -> 0.0804`, PC within `0.002`: every limit still met |

Three of four caught. Diagnosis of MU24: with `a2m1 = 0` the F-bar point strain is the compatible one, but the force and the
tangent keep the mean-dilatation operator `dbar` and the weights `omega` of ADR 0506, so the mutated element still relaxes the
volumetric constraint and stays near F-bar (its reaction moves by 1.7 per cent on the coarse block, 1.4 per cent on the refined
one), inside tolerances set at the scale of discretisation differences. The load checks compare elements to about ten per cent;
they are not a test of the reference's implementation. Run once more as a diagnosis, outside the registration, against the ADR
0506 tests (`si_fbar.rs`, `mut-MU24-diag-si_fbar.log`): SS2 (`1.9e-1` against `1e-5`), T1 (a) (`8.4e-4` against `1e-6`) and
five other tests fail, so the reference is guarded by its own checks.

### Not run

Nothing registered was skipped. The heavy record G5 was run once (294 s) and not re-run for the mutations (HF3 is its fast
companion).

### Decision

**partial**, by the registered rule: every check met in its single run (PB1, PB2, PC1, PC2, N1, SL, G5, HF3, R-*L, B5, B6, S5,
S6), but one planned mutation (MU24) not caught by the tests it was run against. Every clause of the acceptance is met: rule L
under a load is within `0.121` (coarse) and `0.066` (refined) of the F-bar reaction on the confined punch block at `nu = 0.49`
and within `0.057` and `0.019` on the bending cantilever, where the Full Hex8 is `0.34` to `0.79` off; its lowest eigenvalue in
the ADR 0628 sweep stays `>= 0.559 mu` in all six cases; the ADR 0608 checks hold; no library file changed. On the coarse block
about half of rule L's difference from F-bar is the stiffness the stabilisation adds (the plain element is within `0.055`); on
the refined block the plain element cannot be solved (N3).

### Remaining questions

1. **MU24 not caught.** Options: (a) accept: the reference is guarded by its own ADR 0506 checks (shown above) and the load
   checks are about the element under test; (b) add a check of the F-bar reference against a registered value of its own in
   this test file. Recommendation: (a); a frozen reference value would be a regression record of F-bar, which the ADR 0506
   checks already are. If the lead reads the decision rule's mutation clause as satisfied by the diagnosis, the item is done.
2. **Rule L's own stiffness under load** (coarse block `0.121` against the plain element's `0.055`). Options: (a) accept, it
   halves under refinement (`0.066`); (b) a follow-up scaling of the coefficient. Recommendation: (a).
3. **Default.** Every rule stays opt-in (as ADR 0614, 0628, 0630). Recommendation: keep; a default change would change default
   results and is the maintainer's.

## For the lead: roadmap entries

> Proposed status: partial.

> Structural solver, intermediate tier. Follow-up of F-703 (ADR 0630). PARTIAL. ADR 0636, 2026-11-16: the LinearStress rule of ADR 0630 (opt-in, unchanged; tests only, no library file changed, the bit records of ADR 0506, 0608, 0614, 0630 and the rcfd-api baseline hashes pass) measured under load against the F-bar Hex8 of ADR 0506 on the same mesh. A confined block at nu = 0.49 in a die, compressed by a rigid punch on a quarter of its top (homogeneous confined compression is a patch test and could not fail): punch reaction within 12.1 per cent of F-bar on 4^3 and 6.6 per cent on 8^3 (tolerances 15 and 10 per cent), corner bulge within 1.3 and 4.1 per cent; the Full Hex8 reaction 79 and 40 per cent off (the locking control); the plain incompatible element loses positive definiteness under the punch on 8^3 and cannot be solved, rule L converges. A displacement-controlled cantilever at nu = 0.49 (tip 30 per cent of the length): tip reaction within 5.7 per cent of F-bar on 10 x 2 x 2 and 1.8 per cent on 20 x 4 x 4, shortening within 0.6 and 0.2 per cent; Full 73 and 34 per cent off. The ADR 0628 sweep of rule L keeps the lowest eigenvalue >= 0.559 mu in all six cases (limit 0.5 mu); the ADR 0608 checks hold. Three of four mutations caught; the one missed (the F-bar ratio zeroed in the reference) moves the reference by under 2 per cent and is caught by the ADR 0506 tests. Not covered: distorted meshes under load, other materials and Poisson ratios, tension and shear loads, a contact punch.

> F-703 (note, ADR 0636, 2026-11-16): F-708 measures the LinearStress rule under load: within 12 per cent (coarse) and 7 per cent (refined) of the F-bar reaction on a confined punch block at nu = 0.49, where the plain element fails on the refined mesh.

> Follow-up proposed: The stabilised incompatible-modes Hex8 (LinearStress) under load on distorted meshes. Deps F-708. Acceptance: Every limit not stated here is registered in the ADR before the run. The confined punch block and the displacement-controlled cantilever of ADR 0636 on meshes with the interior nodes moved by 0.1 h (2U - 1) and by 0.25 h (2U - 1), rule L against the F-bar Hex8 of ADR 0506 on the same meshes, reaction and displacement within registered tolerances; the stability of the loaded state reported; a negative control of the F-bar reference within the item's own tests; the default bits unchanged.
