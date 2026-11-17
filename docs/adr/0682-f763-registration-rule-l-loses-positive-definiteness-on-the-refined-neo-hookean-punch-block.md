# ADR 0682: F-763 registration: rule L loses positive definiteness on the refined neo-Hookean punch block (the failing solve of ADR 0678 reproduced, the lowest eigenvalue of the free tangent and of the ADR 0628 pencil at the last converged state located by element, S, W and finer increments, a registered criterion for the cause)

Status: **REGISTERED 2026-11-17, written before any code of F-763 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-763 was committed and none of its checks was run. One timing probe
was run before this text (disclosed under "Arithmetic", item 5): an uncommitted, since deleted test file that solved the
`16^3` block with rule L and the neo-Hookean material to the punch displacement `-0.1 x 0.9625` in ten equal steps (not the
registered path), assembled and factored its free tangent and ran the two eigensolvers; it printed only wall times, the
iteration counts of the eigensolvers, whether each call returned `Ok`, the free equation count and the coordinates of node
4768; no eigenvalue, inertia, pivot or measure of this item was printed or seen. Results go in the addenda at the end; nothing
above the addenda is changed after a run.

Roadmap item F-763 (deps F-747), the follow-up proposed by
[ADR 0678](0678-f747-registration-rule-l-against-f-bar-with-a-j2-material-on-a-refined-reference.md) (its remaining question
2, option (a)). The element is the incompatible-modes Hex8 at finite strain of ADR 0608 with the rule
`IncompatibleStabilisation::LinearStress` of ADR 0630 D1 (rule L); the material the neo-Hookean material of
[ADR 0636](0636-f708-registration-the-linear-stress-coefficient-of-the-stabilised-incompatible-hex8-under-load.md) (`mu =
1e6`, `nu = 0.49`); the set-up the punch block of ADR 0636 as solved by ADR 0678 (`run_block` of `si_eas_hg11.rs`: `[0, 1]^3`
on `16^3`, the die constraints, the punch `u_z = -0.1` on the `zmax` nodes with `x, y <= 0.5`, ten increments, four threads,
the supernodal factor). Number 0682 given by the lead. Branch `f763-work` from 41181a84 (F-742 / ADR 0666, F-747 / ADR 0678
and F-748 / ADR 0668 merged), worktree `w49i-work/f763`, scratch `w49i-work/f763-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test
edited; no new dependency, no unsafe code, the crate graph unchanged; clean-room. **Test-only**: no library file is changed by
this item. Every choice below that the maintainer would normally be asked about is taken on the maintainer's standing
instruction of 2026-11-11 (take the recommendation), except the open questions at the end.

## What F-763 asks

The acceptance, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. The failing solve of ADR 0678 (rule L, the neo-Hookean material of ADR 0636, the punch block on 16^3, the default options) reproduced first; the lowest eigenvalue of the ADR 0628 sweep and of the free tangent at the last converged state located by element; the same solve with S and W of ADR 0658 and ADR 0660 and with finer increments; the cause identified or bounded against a registered criterion; the default bits unchanged.

Clauses: **c0** every limit registered here; **c1** the failing solve of ADR 0678 reproduced first; **c2** the lowest
eigenvalue of the ADR 0628 sweep at the last converged state, located by element; **c3** the lowest eigenvalue of the free
tangent at the last converged state, located by element; **c4** the same solve with S (ADR 0658) and W (ADR 0660); **c5** and
with finer increments; **c6** the cause identified or bounded against a registered criterion; **c7** the default bits
unchanged.

### Readings, registered

- **c1: the failing solve.** Exactly the EC1 solve of ADR 0678: `solve_block(block_mesh(16), "block 16^3", K_L, DEFAULT,
  Mat::NeoHookean, SupernodalNestedDissection)` of `si_eas_hg11.rs`, copied unchanged. Its published outcome (ADR 0678
  `run3-heavy.log`, line 23) is the error text `solver newton: load step 10 of 10 (load factor 9.625e-1 to
  9.656250000000001e-1) after 5 halvings: singular tangent: pivot -4.651826504388123e6 at node 4768 component 0 (diagonal
  2.8253460477227545e6)`. **Reproduced** means the same text, character for character (the driver is deterministic for any
  thread count, ADR 0542). Node 4768 of `hex_box` on `16^3` is `(0.5, 0.5, 1)` (probe, arithmetic 5): the corner of the punch,
  where the two edge rows meet.
- **The attempt log.** The driver keeps no state of a failed solve. The same solve is run a second time through
  `solve_with` with a step-control hook (ADR 0440 D7) that returns the driver's proposed load factor unchanged and records
  every attempt (`committed`, `attempted`, converged or failed). The hook changes no arithmetic (it returns `proposed`); the
  check is that it gives the same error text. The **last converged load factor** `lambda_c` is the `committed` value of the
  last failed attempt (expected `0.9625` from the text).
- **The last converged state** `u_c`. Rebuilt by the replicated path P: from `u = 0`, for each converged load factor `l` of the
  log in order, one Newton attempt by `solve_from` (ADR 0479 D2) on the model with the punch value `l x (-0.1)`, started from
  the previous converged `u` with the punch equations set to that value (a lifted start, where the driver linearises the
  increment of the prescribed values: the iterates differ, the converged equilibrium agrees to the Newton tolerance), the
  default Newton options of c1 (`max_iterations` 20, residual and energy `1e-10`, the supernodal factor, four threads). If an
  attempt of P fails it is retried once from the same state in two equal halves (disclosed in the results). The driver's own
  first factor at `u_c` is rebuilt as the driver builds it (`constraint::reduce` of `nonlinear::tangent`, `SupernodalAnalysis::
  nested_dissection`, `SupernodalLdlt::factor_analysed`, the pivot test `d <= 1e-10 a_ii`): if the driver failed at the first
  iteration of every halving from `u_c` (each attempt starts by factoring the tangent at `u_c`), this factor fails with the
  pivot of the text (RP2). RP2 tells which: met, the tangent at the converged state itself is not positive definite; missed,
  the failure is at a later iterate.
- **c3: the free tangent.** `K_ff`: `nonlinear::tangent` at `u_c` with the converged states, reduced to the free equations by
  `constraint::reduce` (the driver's reduction), symmetrised `(K + K^T) / 2` after its antisymmetric part is measured (SY;
  the condensed tangent of a hyperelastic element is symmetric, rounding apart). Its four lowest eigenpairs of `K_ff x =
  lambda x` by `eigen::lowest_eigenpairs` (subspace iteration with a Sturm check, ADR 0283; `Shift::Auto`, the supernodal
  factor with the node coordinates, tolerance `1e-8`, at most 300 iterations): **the shift-invert method** the brief asks for
  (the factor of `K + sigma I` applied at every iteration). `lambda_1` is reported over `mu h` (`h = 1/16`). The inertia
  `nu` = the number of negative eigenvalues of `K_ff` by the signs of the supernodal `L D L^T` with the `NonZero` pivot rule
  (Sylvester's law of inertia; ADR 0283 D8).
- **c2: the ADR 0628 sweep.** Its measure, the lowest eigenvalue `m_1` of the pencil `(K_ff, L_ff)` over `mu`, `L` the vector
  Laplacian of the Full Hex8 (2 x 2 x 2 rule) on the free block (`laplacian` of `si_eas_hg3.rs`, assembled sparse), at the
  same `u_c`, by the same solver. ADR 0628 swept homogeneous states of a patch with every boundary node fixed; here it is the
  loaded block's state, with the block's own free equations (the brief: "at the last converged state"). By Sylvester's law
  the pencil has exactly `nu` negative eigenvalues (`L` is positive definite), a check (E2). The ADR 0628 element row K0 is
  also read: the smallest eigenvalue of the modes block `K_aa` of every element (the stabilised extended element at its
  converged modes, the block the condensation inverts) over `mu h^3`, with its element (KA).
- **Located by element.** For an eigenvector `x` (full length, zero at the fixed equations) the element energies `e_e = x_e^T
  K_e x_e`, `K_e` the condensed tangent of element `e` at `u_c` (`nl_incompatible::condensed_force_and_tangent` with rule L,
  the default mode update); `sum_e e_e = x^T K x` (LC1). Weights `w_e = max(-e_e, 0)` when the eigenvalue is negative, `|e_e|`
  otherwise; `W = sum_e w_e`. Reported:
  - the **located set** `A`: the fewest elements, by decreasing `w_e`, whose weights reach `0.5 W`; its size and the grid
    position `(i, j, k)` of its first element;
  - the **edge share** `s_E = sum_{e in E} w_e / W`, `E` the punch-edge elements: the top-layer elements with a node of the
    edge row (ADR 0678 c6: the punch nodes with `|x - 0.5| < 1e-9` or `|y - 0.5| < 1e-9`); `|E| = 32` on `16^3`, `16` on `8^3`
    (arithmetic 2). The punch-edge row is where ADR 0678 found F-bar's error; "**at the punch edge**" when `s_E >= 0.5`;
  - the **hourglass share** `H = sum_{e in A} w_e h_e / sum_{e in A} w_e`, `h_e = |x_e - P_lin x_e|^2 / |x_e - P_0 x_e|^2`
    (per component, summed; `0` when the denominator is `0`): `P_lin` the projection of the eight nodal values on the
    trilinear basis `1, xi, eta, zeta` of the element (the fields a Hex8 represents with constant gradient), `P_0` on `1`. On a
    regular element the eight vectors `1, xi, eta, zeta, xi eta, eta zeta, zeta xi, xi eta zeta` are orthogonal, so `h_e` is the
    share of the non-translational nodal field carried by the four hourglass vectors. A smooth field gives `h_e = O((h/2)^2
    |grad grad u|^2 / |grad u|^2)` (arithmetic 3); a mesh-scale hourglass pattern `h_e = 1`. "**Hourglass form**" when `H >=
    0.5`, "smooth" when `H <= 0.2`, between "mixed";
  - the smallest principal stretch at the centre of the first element of `A` (the compatible gradient), against the range
    `[0.5, 1]` of ADR 0628.
- **The reference element at the same state (the physical reading).** Two numbers decide whether the reference element of
  ADR 0636 and ADR 0678 (the F-bar Hex8 of ADR 0506) sees the same instability:
  - `rho_F = x_1^T K_F(u_c) x_1 / x_1^T x_1`, the Rayleigh quotient of rule L's lowest mode under the F-bar tangent **at rule
    L's displacement** `u_c` (`nonlinear::tangent` of the F-bar model, reduced the same way): the energy F-bar gives the same
    displacement mode at the same configuration;
  - `lambda_F`, the lowest eigenvalue of F-bar's own free tangent at **its own** converged state at `lambda_c` (the
    replicated path P for F-bar on the same log), by the same solver.
  Reported beside: `rho_Full` (the Full Hex8 at `u_c`, which locks and so over-estimates every energy, reported only) and
  `lambda_1` of rule L on `8^3` at `lambda_c` (the path P on `8^3`; ADR 0678 solved it to full load).
- **c6: the criterion, registered.**
  - **I-S, identified: a spurious instability of rule L.** `lambda_1 < 0` at `u_c` (equivalently `nu >= 1`), `rho_F > 0` and
    `lambda_F > 0`: rule L has a negative-energy mode that the reference element, at the same configuration and at its own
    equilibrium, does not have. Since the neo-Hookean energy is strongly elliptic (ADR 0628 arithmetic 2: `q > 0` on its
    range; polyconvex energies are rank-one convex, cited from memory) and F-bar is stable at the same load, the mode is not
    a material or geometric instability of the continuum; it is the element's. Its form by `H` (hourglass form, mixed or
    smooth) and its place by `s_E` (at the punch edge or not), reported as part of the identification.
  - **I-P, identified: an instability of the discretised body.** `lambda_1 < 0`, `rho_F <= 0` and `lambda_F <= 0`: the
    reference element has it too (a bifurcation of the punch problem on this mesh).
  - **I-T, identified: a Newton iterate.** `lambda_1 > 0` at `u_c` (the converged state is stable) and at least one of the
    finer-increment solves (c5) reaches load factor 1: the failure is that of an increment, not of the path.
  - **Bounded** otherwise (for instance `rho_F` and `lambda_F` of different signs, or `lambda_1 > 0` and no solve passes):
    reported with `lambda_1`, `nu`, `m_1`, `rho_F`, `lambda_F`, `H`, `s_E` and the onset bracket.
  - The **onset bracket**: `nu` at every converged state of P (the counts along the path); the last state with `nu = 0` and
    the first with `nu >= 1` bracket the load factor where `lambda_1` crosses zero.
  - The pencil mode is located the same way (`H_m`, `s_E,m`); the identification requires the pencil mode to fall on the same
    side of the form thresholds as the free-tangent mode (both `>= 0.5`, both `<= 0.2` or both between); otherwise the form
    is reported as undetermined (the identification of I-S or I-P itself rests on the signs).
- **c4: S and W.** `solve_block` of c1 with the options S (`backtracking`, `decrease`, cold) and W (S with the warm start of
  ADR 0660), each once, the attempt log recorded as in c1. **Reading**: at a converged state the modes of S and W solve the
  same local equations as the default (ADR 0658, ADR 0660: they change the path of the local iteration, not its solution), so
  the tangent at a converged state is the same; under I-S or I-P both are expected to fail with the last converged load
  factor in `[lambda_c - 0.025, lambda_c + 0.025]`. Reported: the outcome, the last converged factor, the error text, or `R`
  and `w_c` if a solve passes.
- **c5: finer increments.** `solve_block` of c1 (default options) with `NewtonOptions::new(20)` and `new(40)` (increments
  `0.05` and `0.025`, five halvings each as the default), each once, the log recorded. **Reading**: "a property of the path"
  when both fail with their last converged factors and `lambda_c` within `0.025` of each other (one increment of the finest
  run); "increment-dependent" when one passes or the spread is larger. A pass enters I-T.
- **c7: default bits.** No library file changes (the diff is checked to touch only `crates/rcfd-fem/tests/` and `docs/`).
  Records run anyway: `si_eas_hg_default_bits.rs`, `si_eas_hg3_bits.rs`, `si_eas_default_bits.rs` (B6); `baseline_hashes` is
  not required (no shared solver file touched) and not run.

## What was read

- `AGENT_RULES.md`; the brief; ADR 0678 (whole, and `f747-scratch/run3-heavy.log` for the error text and the timings); ADR
  0628 (readings c2 to c6, the sweep, K0); ADR 0636 (c1, N3: the plain element's negative pivot on `8^3`); ADR 0614 (the
  literature section, H6); the headers of ADR 0658 and ADR 0660.
- `crates/rcfd-fem/tests/si_eas_hg11.rs` (helpers, `solve_block`), `si_eas_hg3.rs` (`dense_free`, `laplacian`, `kaa_min`,
  `lowest10`, `element_u`).
- `crates/rcfd-fem/src/nonlinear.rs` (`solve`, `solve_with`, `StepControl`, `DriverExtensions`, `Problem::attempt_with`,
  `factor_solve`, `solve_from`, `tangent`), `constraint.rs` (`reduce`, `prescribed_equations`), `nl_incompatible.rs` (the public
  element functions), `builders.rs` (`hex_box` numbering); `crates/rcfd-linalg/src/eigen.rs` (`lowest_eigenpairs`,
  `SubspaceOptions`, `Shift`, `count_below`), `sparse_chol.rs` (`SupernodalLdlt`, `SupernodalAnalysis`, `inertia`).

## Arithmetic done before the code (disclosed)

1. **What the text of ADR 0678 already says.** Five halvings of the tenth increment `0.1` end at `0.1 / 32 = 0.003125`, from
   `0.9625` to `0.965625`. A sequence consistent with the driver's rule (halve on failure, keep the halved increment after a
   success): `0.9 -> 1.0` fails, `0.95` converges, `1.0` fails, `0.975` fails, `0.9625` converges, `0.975`, `0.96875`,
   `0.965625` fail. Three failures from the same `u_c` with ever smaller increments are what a tangent that is already not
   positive definite at `u_c` produces (every attempt factors it first); this is the hypothesis RP2 tests. The pivot is at the
   punch corner, `x` component: a negative pivot of an elimination without pivoting is not an eigenvector, so this locates
   nothing by itself (the order of the nested dissection decides where the deficit appears).
2. **The punch-edge elements.** On `n^3` (`n` even) the edge row is `x = 0.5`, `y <= 0.5` (`n/2 + 1` nodes) and `y = 0.5`, `x
   <= 0.5` (as many), `n + 1` nodes together. Top-layer elements with such a node: columns `i in {n/2 - 1, n/2}`, rows `j in
   0..=n/2`: `2 (n/2 + 1)`; the same for the other row; `4` counted twice: `|E| = 2 n`, so `16` on `8^3`, `32` on `16^3`.
3. **The hourglass share of a smooth field.** For `u = f(x)` on an element of size `h` the nodal values are, about the centre,
   `f + (h/2)(f_x xi + f_y eta + f_z zeta) + (h/2)^2 (f_xy xi eta + ...) + ...` (the `xi^2` terms are constant at the nodes).
   So `h_e ~ (h/2)^2 sum f_xy^2 / sum f_x^2`; for `f = sin(pi x) sin(pi y) sin(pi z)` the averages give `(h/2)^2 pi^2`: `0.0096`
   on `16^3`, `0.039` on `8^3`. NC-H (iii) uses `16^3` with the limit `0.05` (factor 5). A field varying in one direction only
   (any wavelength) is linear on every element (`h_e = 0`): `H` reads two- and three-dimensional mesh-scale patterns, the form
   of the hourglass modes of the Hex8, and not a one-directional zigzag (a limit of the measure, stated).
4. **What to expect of the plain element (PC1).** ADR 0636 N3: the plain incompatible element (`Off`) on `8^3` fails at load
   factor `0.93` with a negative pivot (`-1.13e6`) after five halvings, the hourglass instability of ADR 0614 and ADR 0628 under
   load. It is the positive control of the criterion: I-S is expected for it (F-bar stable on `8^3`). ADR 0614 H6 measured a
   low checkerboard content (`0.107`) for the plain element's patch mode, so no limit is set on its `H` (reported: it
   calibrates the hourglass threshold).
5. **Cost (the timing probe, disclosed).** The `16^3` rule L neo-Hookean solve to `-0.1 x 0.9625` in ten steps: `59.7 s`
   (ADR 0678: the full solve `55.3 s`); `13213` free equations; tangent and reduction `0.2 s`; the supernodal `NonZero` factor
   `0.7 s`; four lowest pairs of `(K_ff, I)` `5.5 s` (25 iterations); the sparse Laplacian `0.5 s`; four lowest of the pencil
   `28.8 s` (167 iterations); the 4096 element tangents `0.3 s`. The node of the pivot: `(0.5, 0.5, 1.0)`. So the heavy
   records: (A) the reproduction, the hooked solve, P on `16^3` for rule L and F-bar and on `8^3`, the eigen analyses: about
   `5` minutes; (B) S, W, 20 and 40 increments: about `8` to `12` minutes. Each is `#[ignore]`d and run once; the fast test
   (`8^3`, the synthetic controls, PC1) about `30 s`.

## Design (test-only)

### D1. The test file `crates/rcfd-fem/tests/si_eas_hg12.rs`

Helpers copied from `si_eas_hg11.rs` and `si_eas_hg3.rs` (existing tests are not edited). New: the recording hook
`Recorder` (a `StepControl` returning `proposed`); `replicate` (the path P by `solve_from`); `free_tangent` (tangent, reduce,
SY, symmetrise); `inertia` (supernodal, `NonZero`); `driver_factor` (the driver's first factor, RP2); `laplacian_sparse`;
`lowest` (the four pairs); `locate` (`e_e`, `A`, `s_E`, `H`, the stretch); `hourglass_share`; `edge_elements`; `rayleigh`.

- `f763_fast_setup` (default, fast): on `8^3`, rule L and the plain element; the synthetic controls; every row F, NC, PC.
- `f763_nh16_tangent_heavy` (`#[ignore]`, run once): RP0, RP1, RP2, SY, E1, E2, LC1, the location, KA, ON, CF, C8, the
  classification.
- `f763_nh16_variants_heavy` (`#[ignore]`, run once): S, W, 20 and 40 increments.

## Checks

Every measure is required finite before it is compared (a non-finite value or a failed step misses).

| Id | Clause | Measure | Limit | Estimate |
|---|---|---|---|---|
| F1 | c1 | `8^3` rule L, the path P (ten steps by `solve_from`) to load factor 1: `R`, `w_c` against ADR 0678 (`0.36659596`, `0.06631664`), relative | `<= 1e-6` each | the converged equilibrium to the Newton tolerance; the published 8 digits |
| F2 | c3 | `8^3` rule L, `nu` at the converged states `0.1 .. 0.9` of P, and the driver's first factor there passes | `nu = 0`; passes, each | the driver factored these states (they started the next step) |
| F3 | c3 | `8^3` rule L at load factor 1: four lowest pairs of `(K_ff, I)`, the largest residual; and the count of negative values among them equal to `min(nu, 4)` | `<= 1e-8`; equal | the solver's own test |
| F4 | c2 | the same for the pencil `(K_ff, L_ff)` | `<= 1e-8`; equal | as F3; Sylvester |
| F5 (LC1) | c2, c3 | `8^3`: `|sum_e e_e - x^T K_ff x| / sum_e |e_e|` for the lowest mode of F3 and of F4 | `<= 1e-10` each | the same element matrices summed: rounding |
| F6 | c2 | `|E|` on `8^3` and `16^3` | `16`, `32` | arithmetic 2 |
| F7 | c1 | `8^3` rule L: the hooked solve against the plain solve, `R` and `w_c` | bit-equal | the hook returns `proposed` |
| NC-H | c2 | `16^3` mesh, no solve: (i) the pattern `xi eta` in `u_x` on one interior element: `h_e`; (ii) a linear field `u = (0.1 x + 0.2 y, 0.3 z, -0.1 x + 0.4 z)` on every element: largest `h_e`; (iii) the field `sin(pi x) sin(pi y) sin(pi z)` in each component: `H_mesh = sum_e |x_e - P_0 x_e|^2 h_e / sum_e |x_e - P_0 x_e|^2` | (i) `>= 1 - 1e-12`; (ii) `<= 1e-12`; (iii) `<= 0.05` | `1`; `0`; `0.0096` |
| NC-L | c2 | `8^3` at load factor 1: the vector `e_(n0, x)` at the interior node `n0 = (4, 4, 4)`: `A` | `|A| <= 8`, every element of `A` contains `n0` | the energy of one nodal value lives in the elements around it |
| PC1 | c6 | the plain element `Off`, `8^3`, neo-Hookean, default options: the hooked solve fails; at its `u_c` (P on its log) `lambda_1 < 0`, `rho_F > 0`, `lambda_F > 0` (F-bar `8^3` at its `lambda_c`): **I-S** | fails; I-S | ADR 0636 N3 (arithmetic 4); risk: the failure at a later iterate (then `lambda_1 > 0`) |
| RP0 | c1 | the c1 solve: its error text | equal to ADR 0678's | deterministic |
| RP1 | c1 | the hooked solve: its error text; `lambda_c` from the log | equal to RP0's; `0.9625` | the hook changes nothing; arithmetic 1 |
| RP2 | c1, c3 | P on `16^3` reaches `lambda_c`; the driver's first factor at `u_c` fails at node 4768 component 0 with `|d / (-4.651826504388123e6) - 1|` | `<= 1e-3` | the same state within the Newton tolerance; the hypothesis of arithmetic 1 |
| SY | c3 | `max |k_ij - k_ji| / sqrt(|k_ii k_jj|)` of `K_ff` at `u_c` | `<= 1e-10` | rounding of the condensation |
| E1 | c3 | four lowest pairs of `(K_ff, I)` at `u_c`: the largest residual; negative values among them `= min(nu, 4)` | `<= 1e-8`; equal | the probe converged in 25 iterations |
| E2 | c2 | the pencil `(K_ff, L_ff)` at `u_c`: the same | `<= 1e-8`; equal | probe: 167 iterations |
| LC1 | c2, c3 | as F5 for the lowest mode of E1 and of E2 | `<= 1e-10` each | as F5 |
| LOC | c2, c3 | `A`, `s_E`, `H`, the stretch for both modes | finite (reported) | |
| KA | c2 | `min_e lambda_min(K_aa,e) / (mu h^3)` at `u_c` with its element | finite (reported; `> 0` read as a well-posed condensation) | |
| ON | c6 | `nu` at every converged state of P on `16^3` | finite (reported) | |
| CF | c6 | `rho_F`, `lambda_F` (F-bar P to `lambda_c` succeeds, its four pairs with residual `<= 1e-8`); `rho_Full` | finite; `<= 1e-8` | |
| C8 | c6 | rule L `8^3` at `lambda_c`: `lambda_1` | finite (reported) | expected `> 0` |
| CL | c6 | the classification of the criterion | I-S, I-P, I-T or bounded (reported; the decision) | |
| SW | c4 | S and W on `16^3`: outcome (failure with a parsed last converged load factor, or a pass with finite `R`, `w_c`) | each classified (reported against the reading of c4) | |
| FI | c5 | 20 and 40 increments: the same | each classified (reported against the reading of c5) | |
| B6 | c7 | `si_eas_hg_default_bits`, `si_eas_hg3_bits`, `si_eas_default_bits`; the diff only tests and docs | pass; yes | |
| S6 | | `cargo fmt -p rcfd-fem --check`, `cargo clippy -p rcfd-fem --all-targets -- -D warnings` | clean | |
| S7 | | `cargo test -p rcfd-fem --release --lib`; `--test si_eas_hg9`, `si_eas_hg10`, `si_eas_hg11` (the brief) | pass | |
| S8 | | `cargo test -p rcfd-cli --release --test docs_site` (docs/ touched) | pass | |

### Negative and positive controls

| Id | Control | Expected |
|---|---|---|
| NC-H | synthetic fields with known hourglass shares | `1`, `0`, below `0.05` |
| NC-L | a vector of one nodal value | located in the elements around the node |
| F2 | the stable converged states of the `8^3` path | no negative eigenvalue counted |
| PC1 | the plain element, whose loss of positive definiteness under this punch is known (ADR 0636 N3) | I-S |
| C8, CF | rule L on `8^3` and F-bar on `16^3` at the same load (both solved to full load in ADR 0678) | reported, expected stable |

### Planned mutations (on a scratch copy of the test commit, each run once against `f763_fast_setup`)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | `P_lin` replaced by `P_0` in the hourglass share | NC-H (ii) |
| MU2 | the element energies from the plain element's tangent (`Off`) | F5 |
| MU3 | the edge set `E` from every punch node | F6 |
| MU4 | the lifted start without the new punch value (the start keeps the previous one) | F1 |
| MU5 | the inertia counting positive pivots | F2 |
| MU6 | the Laplacian with the first component only | F4 |

The heavy records are not run under the mutations (they use the same functions, checked by the fast test).

## Decision rule

- **done**: F1 to F7, NC-H, NC-L, PC1, RP0, RP1, RP2, SY, E1, E2, LC1, CF, B6, S6, S7, S8 met in their single registered run;
  LOC, KA, ON, C8, SW, FI finite and reported; every mutation caught; the cause **identified** (I-S, I-P or I-T); no
  amendment.
- **partial**: B6 met, but a check missed, or a mutation not caught, or the cause only bounded, or a check met only after an
  amendment.
- **todo**: B6 fails (a default bit changed), or RP0 fails (the failing solve is not reproduced) and no state could be
  analysed.

## Limits (not covered)

- One mesh family (the regular block), one material, one load; the `32^3` mesh is not run.
- The located measures are of the four lowest modes' first one only (the others are reported by value); the eigenvector of a
  repeated or nearly repeated lowest eigenvalue is any vector of its space (reported if the two lowest are within `1e-6`
  relative).
- `H` does not see one-directional mesh-scale patterns (arithmetic 3).
- The last converged state is rebuilt, not the driver's own bits (RP2 bounds the difference by the pivot).
- No remedy is registered here: if the cause is identified, a remedy is a follow-up.
- The default stays as it is (rule L opt-in, cold start; ADR 0660 open question 2).

## Open questions (taken or open)

1. **The design of the criterion** (the reference element at the same configuration and at its own equilibrium, the thresholds
   `0.5` and `0.2` of `H`, `0.5` of `s_E`, the replicated path with a lifted start, the band `0.025` of c4 and c5): taken on
   the maintainer's standing instruction of 2026-11-11 (take the recommendation).
2. **Whether a remedy may change rule L's coefficient**: not taken (it would change the results of every rule L item); open
   for the maintainer, with the numbers of this item.

## Amendment 1 (2026-11-17, written after the registered run of the fast test and before any further run)

The registered run of `f763_fast_setup` (test commit 0c57234a, log `run1-fast.log` in the scratch folder, 50 s) met every row
but one: F6 (`16`, `32`), NC-H (i) `1.0000`, (ii) `2.4e-29`, (iii) `9.64e-3`, F7 (bit-equal), F1 (`2.3e-9`, `6.4e-8`), F2
(`nu = 0` and the driver's factor passing at the nine states), F3 (`6.8e-9`; no negative value, `nu = 0`), F4 (`8.9e-9`; the
same), F5 (`4.8e-15`, `7.6e-15`), NC-L (`|A| = 4`, all around `n0`) and the PC1 row "the hooked solve fails" (load factor
`0.928125` to `0.93125`, pivot `-3.21e3` at node 446). It missed **PC1 "I-S"**: the classification had a missing measure,
because **the path P of the plain element did not reach its `lambda_c`**: every attempt of P for `Off` from `0.1` on failed
(a negative pivot at its first factor), the retries in two halves passed up to `0.6`, and at `0.65` and `0.7` an element
inverted; P stopped at `0.6` of `0.928125`. P of F-bar on the same log also failed every attempt and passed by its retries.

**Diagnosis.** A defect of the set-up of reading c1, not of the item: the **lifted start** sets the punch nodes to the new
value while their neighbours stay where they were, so the first iterate of every attempt carries the whole increment `0.1 x
0.1 = 0.01` as a jump over one element at the punch edge (a shear of `0.08` over `h = 1/8`). The driver never evaluates that
iterate: its first correction is the linear predictor `K_ff du_F = r_F - K_FP dg` at the committed state (`Problem::
attempt_with`, `reduce` with the increment `(lt - lc) v`), after which the prescribed values are `lt v` and the free ones have
moved with them. The tangent at the lifted iterate is what failed (for `Off` and F-bar; rule L passed it, F1). The registered
statement "the iterates differ, the converged equilibrium agrees" holds only when the lifted iteration converges.

**Correction (registered now).** Reading c1, the path P: each attempt from the committed state `(u_c, states_c, lc)` to `l`
takes **the driver's first iteration**: `nonlinear::internal_force` and `nonlinear::tangent` at `u_c` with `states_c`,
`constraint::reduce` with the right side `-f_int` and the increment `(l - lc) v` at the prescribed equations (`v` the model's
value of each, `-0.1` at the punch, `0` elsewhere), the driver's factor (`SupernodalAnalysis::nested_dissection`,
`SupernodalLdlt::factor_analysed`, the pivot test `d <= 1e-10 a_ii`; a refusal fails the attempt), `u[free] += du`, then the
prescribed values set to `l v` (bit for bit the driver's assignment `lt * v`); from that predictor the Newton iteration by
`solve_from` on the model with the punch at `l x (-0.1)` (the same bits). The rest of P (the retry in two halves, the
options) is unchanged. The iterates are then the driver's from its second iteration on; the convergence decision may stop at
another iterate (`solve_from`'s energy test is relative to its own first correction), so the state agrees with the driver's
within the Newton tolerance (RP2 tests it). Every other definition, limit and check stays as registered. In `si_eas_hg12.rs`:
the path as registered is kept as `replicate_lifted` and the registered run as the ignored record
`f763_fast_setup_known_miss` (rows and limits unchanged, the measured value in its ignore text); `f763_fast_setup_corrected`
runs the same rows with the corrected P, once. The heavy records have not been run; they use the corrected P and are run once
after this amendment. The mutations are run against `f763_fast_setup_corrected`; MU4 (the lifted start without the new punch
value) is read on the corrected P as "the predictor without the prescribed increment" (`(l - lc) v` replaced by `0`), caught
by F1 as registered. The registered rule gives **partial** whatever follows (a check met only after an amendment).

## Addendum 1: results (2026-11-17)

The registered fast run once on 0c57234a (`run1-fast.log`, 50 s; amendment 1); the corrected fast run once on abf6ab10
(`run2-fast.log`, 16 s); the two heavy records once each on 000a0d4d, side by side (`run3-heavy-tangent.log`, 247 s;
`run4-heavy-variants.log`, 248 s; the machine shared); the mutations once each on a scratch copy of 000a0d4d against
`f763_fast_setup_corrected_known_miss` (`mutations.log`, `mut-MU*.log`); S6, S7, B6, S8 once (`fmt.log`, `clippy.log`,
`s7-lib.log`, `s7-tests.log`, `b6-bits.log`, `s8-docs.log`). All in `w49i-work/f763-scratch`. No limit, set-up or measure was
changed after a run. The records: `f763_fast_setup_known_miss` (the registered run, amendment 1),
`f763_fast_setup_corrected_known_miss` (PC1), `f763_nh16_tangent_heavy_known_miss` (RP2), `f763_nh16_variants_heavy` (met;
heavy), all ignored with the measured values in their ignore texts; `f763_fast_rows` (default, 7.5 s) runs the met rows of
the corrected fast run (all but PC1) on `8^3`, on the same path as the heavy records.

### What was found

- **The failing solve reproduced** (RP0, RP1): the same error text, character for character, plain (60.0 s) and hooked
  (44.8 s). The attempts of the tenth step: `0.9 -> 1.0` fails, `0.95` converges, `1.0`, `0.975`, `0.9625` fail, `0.95625` and
  `0.9625` converge, `0.96875` and `0.965625` fail (the eighth step also halved once, `0.7 -> 0.75 -> 0.8`). `lambda_c =
  0.9625` exactly.
- **Every converged state is stable.** P (the driver's predictor, then `solve_from`) reached `lambda_c` with no retry (47.0
  s). The inertia is `nu = 0` at all 13 converged states, and **the driver's own first factor passes at `u_c`** (RP2 missed:
  there is no refusal, so no pivot to compare). At `u_c`: `R/mu = 0.32658599`, `w_c = 0.06020454`. So the negative pivot of
  ADR 0678 is **not** a property of a converged state: it appears at a Newton iterate of the attempts from the stable state
  `u_c`, and the hypothesis of arithmetic 1 is refuted.
- **The free tangent at `u_c`** (E1, c3): `lambda_1 / (mu h) = 0.03633` (the four lowest `0.0363`, `0.0489`, `0.0630`,
  `0.0756`); F-bar at its own state `lambda_F / (mu h) = 0.03620` (`0.4` per cent apart); rule L's mode under the F-bar tangent
  at the same displacement `rho_F / (mu h) = 0.03884` (`rho_Full = 0.0540`); rule L on `8^3` at `lambda_c` `0.1170`. The mode is
  global and smooth: `|A| = 652` elements, `s_E = 0.021`, `H = 0.134`; its largest element energy is in element 3720.
- **The ADR 0628 pencil at `u_c`** (E2, c2): `m_1 = 0.0629`, **isolated**: the next three are `0.615`, `0.649`, `0.674`. Its
  mode is **local and of hourglass form**: `|A| = 2` elements, `H = 0.882`, the first element **3720 at `(8, 8, 14)`** (`x, y in
  [0.5, 0.5625]`, `z in [0.875, 0.9375]`: diagonally outside the punch corner, one layer below the top; its upper corner node
  `(0.5, 0.5, 0.9375)` is directly under node 4768, the corner where the two edge rows meet and where the pivot of ADR 0678
  appears), `s_E = 0.133`, the smallest centre stretch there `0.808`. The **smallest modes block** of all 4096 elements (KA)
  is in the same element: `lambda_min(K_aa) / (mu h^3) = 37.3`. The pencil weights gradients, so it isolates this mesh-scale
  mode, which the standard problem hides under the smooth global modes. ADR 0636 kept the sweep of rule L at `>= 0.559 mu`
  on homogeneous states; here a non-homogeneous state at the punch corner brings one mode down to `0.063 mu`, still positive.
- **S and W** (c4): both fail exactly as the default (the same attempts, last converged `0.9625`, the pivot `-4.6518265e6`
  at node 4768 within `2e-9` relative of the default's): the local mode update does not change the failing iterate.
- **Finer increments** (c5): 20 and 40 increments fail **earlier**, from `0.870312` and `0.871094`, by an **inverted element
  3959** (`(7, 7, 15)`: the top-layer element under the punch corner; `det F = -0.157`, `-0.155` at point 4) at an iterate, with
  increments of `0.0016` and `0.0008` after the halvings. By reading c5 the failure is **increment-dependent** (spread
  `0.092` against `0.025`), and no finer solve passes.
- **The plain element** (PC1): its `8^3` path is stable too: at its last converged state `0.928125` `nu = 0`, `lambda_1 /
  (mu h) = 0.1153` against F-bar's `0.1151`; its failure (pivot `-3.21e3`) is also at an iterate. PC1 expected I-S and
  missed: the registered positive control was not one (arithmetic 4 named this risk).

### Results

| Id | Limit | Measured | Met |
|---|---|---|---|
| F1 (registered run; corrected) | `<= 1e-6` | `2.3e-9`, `6.4e-8` (both runs) | yes |
| F2 | `nu = 0`; passes | `nu = 0` at the nine states; the driver's factor passes | yes |
| F3 | `<= 1e-8`; equal | `6.8e-9`; `0 = min(0, 4)` | yes |
| F4 | `<= 1e-8`; equal | `8.9e-9`; `0` | yes |
| F5 (LC1, `8^3`) | `<= 1e-10` | `3.1e-15`, `3.6e-15` | yes |
| F6 | `16`, `32` | `16`, `32` | yes |
| F7 | bit-equal | bit-equal | yes |
| NC-H (i), (ii), (iii) | `>= 1 - 1e-12`; `<= 1e-12`; `<= 0.05` | `1.0`; `2.4e-29`; `9.64e-3` | yes |
| NC-L | `|A| <= 8`, around `n0` | `4`, all around `n0` | yes |
| PC1 (registered run) | fails; I-S | fails; **a missing measure** (the lifted start stopped P at `0.6`) | **no** (amendment 1) |
| PC1 (corrected) | fails; I-S | fails; **`nu = 0`, `lambda_1 / (mu h) = 0.1153`**: "I-T or bounded" | **no** (known miss) |
| RP0 | the text of ADR 0678 | equal | yes |
| RP1 | equal; `lambda_c = 0.9625` | equal; `0.9625` | yes |
| RP2 | node 4768 comp 0; `<= 1e-3` | **the driver's factor passes at `u_c`**: no refusal | **no** (known miss) |
| SY | `<= 1e-10` | `3.9e-16` | yes |
| E1 | `<= 1e-8`; equal | `9.7e-9`; `0 = min(0, 4)` | yes |
| E2 | `<= 1e-8`; equal | `9.994e-9`; `0` | yes |
| LC1 (`16^3`) | `<= 1e-10` | `6.6e-15`, `6.8e-16` | yes |
| LOC, KA, ON | finite | as above | yes |
| CF | finite; `<= 1e-8` | `rho_F 0.0388`, `rho_Full 0.0540`, `lambda_F 0.0362`; `9.0e-9` | yes |
| C8 | finite | `0.1170` | yes |
| SW, FI | classified | S, W fail from `0.9625`; 20, 40 fail from `0.8703`, `0.8711` | yes |
| CL | classified | **bounded** (`lambda_1 > 0`, no finer solve passes) | reported |
| B6 | pass; tests and docs only | the three records pass; `git diff --stat 41181a84` touches `crates/rcfd-fem/tests/si_eas_hg12.rs` and this ADR only | yes |
| S6 | clean | fmt and clippy (`--all-targets -D warnings`) clean | yes |
| S7 | pass | `--lib` 15; `si_eas_hg9` 4, `si_eas_hg10` 2, `si_eas_hg11` 1, `si_eas_hg12` 1 passed | yes |
| S8 | pass | `docs_site` 2 passed | yes |

### c6: the cause, by the registered criterion

**Bounded, not identified.** `lambda_1 > 0` at the last converged state and no finer solve passes: "bounded (stable converged
state, no finer solve passes)". What the measures bound:

- **Not an instability of the body, nor of rule L at a converged state**: every converged state of the path has `nu = 0`; at
  `u_c` rule L's lowest eigenvalue equals F-bar's within `0.4` per cent and F-bar gives rule L's mode a positive energy. I-S and
  I-P (both need `lambda_1 < 0`) are excluded at the converged states.
- **A Newton iterate from a stable state**: the driver's factor at `u_c` passes; the failing pivot is the same to `2e-9` in the
  default, S and W (the mode update does not move it); with finer increments the attempts fail earlier, by an inverted
  element, so smaller increments do not avoid it. Registered reading c5: increment-dependent.
- **Where**: at the **punch corner**, not along the edge rows: the pivot at node 4768 (the corner), the inverted element 3959
  under it, the isolated pencil mode and the smallest `K_aa` in element 3720 under the corner one layer down. The edge shares of
  the two modes are `0.02` and `0.13`: the instability does not live in the edge row where ADR 0678 found F-bar's error.
- **The candidate, not proven**: the isolated, local, hourglass-form pencil mode (`m_1 = 0.063 mu` in two elements, ten times
  below the next) is soft enough that the predictor of an increment from `u_c` (a linear solve with this nearly singular local
  mode) can carry the corner elements past the point where the tangent or the deformation gradient is admissible. This was not
  measured: no iterate of the failing attempt is recorded (the driver keeps none), and the share of the predictor in that mode
  was not registered. The follow-up below registers it.

### Deviations

- Amendment 1: the lifted start of P was a defective set-up (the plain element's path stopped), corrected before the heavy
  records.
- RP2 missed: the hypothesis of arithmetic 1 (the tangent at `u_c` already indefinite) is false; recorded, not tuned.
- PC1 missed: the plain element's failure on `8^3` is also at an iterate, so it was not a positive control of I-S.
- The heavy records ran side by side (their wall times are shared-machine times).
- MU4, the predictor without the prescribed increment, was registered as caught by F1; it is **not caught**: on the `8^3`
  rule L path the lifted iteration converges to the same state (as in the registered run), and its only effect is on PC1's
  path, a row that already misses.

### Mutations (scratch copy of 000a0d4d, `f763_fast_setup_corrected_known_miss`, each run once; PC1 misses in every run)

| Id | Caught | By |
|---|---|---|
| MU1 (`P_lin` replaced by `P_0`) | **yes** | NC-H (ii) (`1.0`), (iii) (`1.0`) |
| MU2 (energies from the plain element's tangent) | **yes** | F5 (`3.3e-2`, `9.8e-2`) |
| MU3 (`E` from every punch node) | **yes** | F6 (`25`, `81`) |
| MU4 (the predictor without the increment) | **no** | F1, F2 met; only PC1's path changes (it stops at `0.6`) |
| MU5 (inertia counting positive pivots) | **yes** | F2 (`1757`), F3, F4 |
| MU6 (the Laplacian of the first component) | **yes** | F4 (NaN: the pencil solve fails), F5 |

Five of six caught.

### Not run

Nothing registered was skipped. Not run by design: a `32^3` mesh, other materials, the iterates of the failing attempt (not
registered), a remedy, `baseline_hashes` (no shared solver file touched). The whole rcfd-fem suite was not run (rule 7).

### Decision

**partial**, by the registered rule: B6 met (no library file changed), the failing solve reproduced, the lowest eigenvalue of
the free tangent and of the ADR 0628 pencil at the last converged state located by element, S, W and two finer increments run;
but the cause is **bounded**, not identified, a check was met only after amendment 1, PC1 and RP2 missed, and MU4 was not
caught.

### Remaining questions

1. **The failing iterate.** Options: (a) a follow-up recording the failing attempt from `u_c` iterate by iterate (the
   predictor's share in the isolated pencil mode, the eigenvalues at each iterate), with the line search of ADR 0607 and a
   registered damping of the predictor tried on it; (b) record the punch-corner failure as a limit of rule L on node-set
   punches. Recommendation: (a), proposed below.
2. **The ADR 0628 sweep on non-homogeneous states.** The homogeneous sweep (ADR 0628, ADR 0636: `>= 0.559 mu`) did not see a
   state that brings one local mode to `0.063 mu`. Options: (a) a separate item extending the sweep to simple shear and to the
   punch-corner element's state; (b) inside the follow-up of question 1. Recommendation: (b).
3. **Whether a remedy may change rule L's coefficient** (open question 2 of the registration): open for the maintainer.
4. **The positive control.** The plain element's `8^3` failure is not an I-S case; a true control needs a state where the
   plain element's converged tangent is indefinite (the ADR 0628 patch below its `lambda_s`). Recommendation: use that patch
   in the follow-up.

## For the lead: roadmap entries

> Proposed status: partial.

> Structural solver, intermediate tier. Follow-up of F-747 (ADR 0678). PARTIAL. ADR 0682, 2026-11-17 (tests only, no library file changed): the failing solve of ADR 0678 (rule L, neo-Hookean, the punch block on 16^3, default options) reproduced to the character, its attempts recorded through an identity step-control hook (last converged load factor 0.9625). The last converged state rebuilt by a replicated path (the driver's predictor, then solve_from; a lifted start was a defective set-up, corrected by amendment): none of the 13 converged states has a negative eigenvalue (supernodal inertia), and the driver's own first factor passes at the last one, so the negative pivot of ADR 0678 belongs to a Newton iterate, not to the equilibrium. The free tangent there: lowest eigenvalue 0.0363 mu h against F-bar's 0.0362 at its own state, F-bar's energy of rule L's mode positive, the mode global and smooth. The ADR 0628 pencil there: lowest 0.063 mu, isolated (next 0.615), in two elements of hourglass form (share 0.88) under the punch corner, where the element with the smallest modes block also sits. S and W fail exactly as the default; 20 and 40 increments fail earlier (0.870, 0.871) by an inverted element under the punch corner. Cause bounded by the registered criterion (a stable converged state, no finer solve passes): not an instability of the body or of rule L at a converged state; a Newton iterate from a stable state at the punch corner, not the edge row; the soft local pencil mode is the candidate, not proven. Missed: the plain element's 8^3 failure, registered as the positive control of a spurious instability, is also at an iterate; the registered hypothesis that the converged tangent is indefinite. Five of six mutations caught. No default bit changed. Not covered: the iterates of the failing attempt, a remedy, a 32^3 mesh.

> F-747 (note, ADR 0682, 2026-11-17): the failed 16^3 neo-Hookean rule L solve of EC1 fails at a Newton iterate from a stable converged state at the punch corner; every converged state is positive definite (F-763).

> Follow-up proposed: The failing Newton iterate of rule L at the punch corner. Deps F-763. Acceptance: Every limit not stated here is registered in the ADR before the run. The failing attempt of ADR 0682 (rule L, neo-Hookean, the punch block on 16^3, from the converged state at load factor 0.9625) recorded iterate by iterate: the predictor's share in the isolated pencil mode of ADR 0682 and the lowest eigenvalue of the free tangent and of the ADR 0628 pencil at every iterate, located by element; the ADR 0628 sweep at the state of the punch-corner element; the line search of ADR 0607 and a registered damping of the predictor on the same attempt, and whether the 16^3 block then solves to full load; a positive control on the ADR 0628 patch below its zero; the default bits unchanged.
