# ADR 0544: F-611 registration: the energy-momentum step with F-425's hyperelastic materials

Status: **REGISTERED 2026-11-15, written before any code of F-611 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-611 was written, compiled or run in the repository. The files under "What was read"
were read; the arithmetic under "Arithmetic before this registration" was done by hand and by a plain Python/numpy probe in the
scratch folder `w49i-work/f611-scratch` (`probe.py`, `probe.log`; the formulas of `dg.rs` and `mat_hyper.rs` transcribed into
numpy, no rcfd code; disclosed, its numbers quoted below). The default-bits record B0 is a test file written and run on the base
f69eb128 after this commit and before any library line (D7). Results go in the addenda at the end; nothing above the addenda is
changed after a run.

Roadmap item F-611 (deps F-607, F-425), the follow-up of ADR 0523 (its "For the lead" follow-up and open question 1). Number
0544 given by the lead. Branch `f611-work` at f69eb128 (main with F-607, ADR 0523, and F-425, ADR 0541), worktree
`w49i-work/f611`, scratch `w49i-work/f611-scratch`. House rules `w49i-work/AGENT_RULES.md`: limits fixed here, a miss becomes an
ignored `*_known_miss` test with the limit intact and the number recorded, each registered run made once, defaults
bit-identical, no edit of `baseline_hashes.rs`, the hash tables or any golden file, no new dependency, no unsafe code,
clean-room. The brief overrides rule 9: the F-611 entries of `docs/ROADMAP.md` and `docs/roadmap.json` are updated in a
separate last commit (the svgs are not regenerated, no item added); the proposed text is also in the addendum. Files of other
agents not touched: `contact.rs` (F-601), the Newton driver's factor choice in `nonlinear.rs` (F-618: `nonlinear.rs` is not
edited at all), the Shell3 paths (F-541: `shell_corot.rs` and the shell branches of `nl_transient.rs` are not edited).

## What F-611 asks

The acceptance, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. With F-425's neo-Hookean and Mooney-Rivlin energies in
> the discrete gradient of ADR 0523, the neo-Hookean spinning bar over ten turns keeps linear momentum within 1e-10, angular
> momentum within 1e-6 and energy within 1e-6 relative, and the large-strain neo-Hookean and Mooney-Rivlin cubes of ADR 0523
> over 500 steps keep energy within 1e-7 relative; the step-local energy of ADR 0523 is removed or equals F-425's within 1e-12
> at a point.

Clauses: (c0) every limit registered here; (c1) the discrete gradient of ADR 0523 D3 with the point functions (`W`, `S`, the
tangent) of `mat_hyper` (ADR 0541 D1) for the neo-Hookean and the Mooney-Rivlin energy; (c2) the neo-Hookean spinning bar of ADR
0523 D7 (the bar of ADR 0479 D5 with `mu = E/(2(1 + nu))`, `kappa = E/(3(1 - 2 nu))`, `E = 2e9`, `nu = 0.3`, `rho = 7850`, the
steady St. Venant-Kirchhoff start, `dt = 0.01`, 1000 steps, the Newton options of ADR 0514 arithmetic 2) with (c1):
`max_n |P_n - P_0|/|P_0| <= 1e-10`, `max_n |J_n - J_0|/|J_0| <= 1e-6`, `max_n |E_n - E_0|/|E_0| <= 1e-6`, `E = T + W` with `W`
F-425's energy summed by `Integrator::strain_energy`; (c3) the large-strain cube of ADR 0523 arithmetic 3 and D7 (`[2, 2, 2]`
Hex8 unit cube, free, `rho = 1000`, `u_0 = 0`, the rotation-translation-dilatation-shear `v_0`, `dt = 0.004`, 500 steps,
`NewtonOptions::new(1)`) with F-425's neo-Hookean energy of `mu = 1e5`, `kappa = 1e6` and with a Mooney-Rivlin energy (D5) of the
same small-strain moduli, each `max_n |E_n - E_0|/|E_0| <= 1e-7`; (c4) the step-local energy of ADR 0523 D2 removed, or equal to
F-425's within `1e-12` at a point. Every aggregated value is asserted finite before it enters a maximum.

## What was read (base f69eb128, read only)

ADR 0523 (whole: D1 to D9, the checks, mutations, amendments 1 and 2, the addendum and its open questions 1 and 5), ADR 0541
(what F-425 asks, arithmetic 1, D1 to D4, D8, D9, the checks including DG1, amendment 1, the addendum), `nl_transient/dg.rs`
(whole), `nl_transient/emc.rs` (`check_model`, `algorithmic_stress`, `internal_force`, the end of the Newton loop),
`nl_transient.rs` (`NlTransientOptions`, the `Integrator` fields, `Integrator::new`, `start`, `strain_energy`), `mat_hyper.rs`
(whole), `tests/sa_dg.rs` (whole) and `tests/sa_dg_default_bits.rs` (the digest scheme), the F-611 entry of
`docs/roadmap.json`. `AGENT_RULES.md` and the brief.

## The choice: the step-local energy kept and shown equal (registered)

The brief prefers removal (`dg.rs` calls `mat_hyper`) if F-607's results keep their bits or the change is confined to rounding,
and otherwise registering that and keeping the comparison. Read before deciding:

1. **The existing test `sa_dg.rs` `f1h_refusals` requires `AlgorithmicStress::DiscreteGradient` to refuse a Mooney-Rivlin
   material** (key `"nl_transient"`, reason containing `ADR 0523`). Taking Mooney-Rivlin through `DiscreteGradient` (needed by
   c1 and c3 if the step-local energy is removed) would fail that test, and existing tests are not edited (rule 2).
2. **Removal changes the bits of F-607's neo-Hookean runs, and not only by the rounding of the point functions.** The two
   evaluations of the same energy differ in rounding (arithmetic 1: `S` within `1e-15`), but F-425's evaluation of the
   isochoric energy, `3 (J^(-2/3) - 1) + 2 tr E J^(-2/3)`, cancels two terms of order `|E|` to a result of order `|E|^2`, so
   its rounding is `~eps |tr E|` against the step-local form's `~eps |E|^2` (arithmetic 1: `|W_D2 - W_425|/|W|` = `1.6e-10` at
   strains of `1e-6`). The rounding bound `beta` of ADR 0523 D3 must then be the magnitude of F-425's terms (D2 below), which
   changes which branch of D3 a point-step takes at small strain: an algorithmic change, not rounding. No digest of ADR 0523
   records a neo-Hookean run (its B0 records the F-570 path only), but the measured results of A0 to A3, K0 to K2, N1h, R1h
   and T1h would all change bits.

**Registered (taken on the maintainer's standing instruction of 2026-11-11, take the recommendation; open question 1):** the
step-local energy of ADR 0523 D2 is **kept** on `AlgorithmicStress::DiscreteGradient` and `Average` (F-607's bits unchanged,
including its Mooney-Rivlin refusal), and a new opt-in variant `AlgorithmicStress::MaterialDiscreteGradient` takes the discrete
gradient of ADR 0523 D3 with F-425's point functions for both energies (D1). The acceptance's clause c4 is met by the
comparison (check E1): the step-local energy equals F-425's within `1e-12` at a point. Folding the two variants into one (and
removing D2's code) needs the F1h row edited: the maintainer's call (open question 1).

## Arithmetic before this registration (disclosed)

1. **The two energies at a point** (`probe.py`, `probe.log`; 200 random strains per scale, components normal with the scale as
   standard deviation, the bar's moduli and the cube's; neo-Hookean, `c10 = mu/2`, `c01 = 0`):

   | scale | `|W_D2 - W_425| / |W_425|` | `|W_D2 - W_425| / m_425` | `|S_D2 - S_425|_inf / |S_425|_inf` |
   |---|---|---|---|
   | 1e-6 | 1.61e-10 | 2.11e-16 | 3.87e-16 |
   | 1e-4 | 1.66e-12 | 2.44e-16 | 4.93e-16 |
   | 1e-3 | 1.55e-13 | 2.43e-16 | 5.68e-16 |
   | 1e-2 | 1.66e-14 | 2.22e-16 | 5.66e-16 |
   | 1e-1 | 3.08e-15 | 4.21e-16 | 7.46e-16 |
   | 3e-1 | 7.21e-16 | 4.22e-16 | 9.53e-16 |

   `m_425` is the magnitude of F-425's terms (D2). The relative energy difference falls as `~1.6e-16 / scale`: the cancellation
   of F-425's evaluation (the choice, item 2), not a difference of the energies. Normalised by `m_425` the difference is a few
   `eps` at every scale. The tangent was not probed (the same closed form term by term, ADR 0541 arithmetic 1; its rounding is
   of order `eps` of its largest entry, no cancellation of order-one terms in either form).
2. **The discrete gradient with F-425's point functions** (`probe.py`, the rule of ADR 0523 D3 with `m = m_425`; NH and MR at
   the cube moduli `(c10, c01, kappa) = (5e4, 0, 1e6)`, `(3.5e4, 1.5e4, 1e6)` and at the bar moduli, MR with `c10 = 0.35 mu`,
   `c01 = 0.15 mu`): the identity `|S_alg . de - dW| / (sum |S_alg,i de_i| + m_0 + m_1)` `<= 1.41e-14` (the midpoint branch's
   bound `64 eps` is `1.42e-14`; 682 to 744 of about 800 pairs corrected); at `|dE| = 1e-8` on base strains `0.1` the midpoint
   branch at 100 of 100 pairs for each energy; the second-order ratio of ADR 0523 P3c, `max |S_alg - S(E_m)|_inf / |S(E_m)|_inf
   / (|dE|^2/|E_m|)`, `3.18` (NH cube), `3.33` (MR cube), `0.63` (NH bar), `2.50` (MR bar). At bar-like points (base strain
   `1e-5`, `|dE| ~ 1e-10`) both rules take the midpoint branch and the two `S_alg` differ by `<= 2.5e-16` relative.
3. **The bar with F-425's energy.** By item 2 the bar's point-steps take the midpoint branch (as with the step-local energy, ADR
   0523 arithmetic 4); the energy error of that branch per point-step is `|nu| <= beta = 64 eps (m_0 + m_1 + ...)`, now with
   `m_425 ~ mu |E|` instead of `mu |E|^2`: at `|E| ~ 1e-5`, `mu = 7.7e8`, the bar volume `4e-4 m^3`, at most `~4e-14 J` per step
   against `E_0 ~ 5.7 J` (kinetic, the spin `0.5 I omega^2` with `I ~ m L^2/12 = 0.26 kg m^2` plus the translation): `~8e-15`
   per step, `8e-12` over 1000 steps of one sign. The residual bound of ADR 0514 arithmetic 3 dominates as before; ADR 0523
   measured `2.1e-13` (A3), `4.1e-13` (A1), `7.5e-13` (A2) with the step-local energy.
4. **The cubes.** The neo-Hookean cube with F-425's energy differs from ADR 0523's run by rounding of the point functions and
   their branch (item 1, 2; the cube's strains are `0.01 .. 0.1`, where the two `W` differ by `<= 1.7e-14` relative): estimate
   the same `~3e-9` (ADR 0523 K1: `2.8e-9`; bound per step `~3e-11` at the residual `1e-10`, 500 steps of one sign `1.5e-8`). The
   Mooney-Rivlin cube (D5) has the same small-strain moduli and so the same mass, `K_0`, frequencies and strain range; the step
   is the same; estimate the same order, `~3e-9`, the bound `1.5e-8`.
5. **The model-level comparison (E2).** At a state of the neo-Hookean cube (strains `<= 0.2`), `sum_q w W` of the two energies
   differs by the point difference of item 1, `<= 2e-14` relative.

## Design

### D1. The variant `AlgorithmicStress::MaterialDiscreteGradient` (opt-in)

`nl_transient::AlgorithmicStress` (`#[non_exhaustive]`) gains `MaterialDiscreteGradient`, read by the energy-momentum step only
(the `Method` step ignores it, as the others):

- the isotropic elastic material: the same as `DiscreteGradient` (ADR 0523 D3 with `S = D E`, the same code: `elastic_discrete_
  gradient`);
- a `Material::Hyperelastic` of either energy: ADR 0523 D3 with `W = mat_hyper::energy`, `S = mat_hyper::stress`, `m(W) =
  mat_hyper::energy_magnitude` (D2) and the tangent `mat_hyper::stress_and_tangent(E_m)` (ADR 0523 D3's tangent `CC(E_m)/2` of
  the step, the derivative of the correction left out as before).

`Auto`, `DiscreteGradient` and `Average` are unchanged (the same code, the same bits; `Auto` still refuses a hyperelastic
material, ADR 0523 amendment 1 and its open question 5; `DiscreteGradient` and `Average` still take the step-local neo-Hookean
and refuse Mooney-Rivlin). The option is not in the state bytes (ADR 0523 D4).

### D2. `mat_hyper::energy_magnitude` (additive)

A new public function in `mat_hyper.rs` (no existing function changed): the sum of the magnitudes of the terms of
`energy_parts`'s evaluation, `|c10| (3 |a - 1| + 2 |t| a) + |c01| (3 |b - 1| + b (4 |t| + 2 t^2 + 2 q)) + (kappa/2)(J - 1)^2`
(`a = J^(-2/3)`, `b = J^(-4/3)`, `t = tr E`, `q = tr E^2`), the bound of the rounding of `W` that ADR 0523 D3's test needs
(`m >= |W|` by the triangle inequality, check Q4). The same refusals as `energy` (a non-finite strain, `det C <= 0`).

### D3. The integrator

`Integrator::new` with the energy-momentum step and `MaterialDiscreteGradient`: the surrogate of ADR 0523 D5 with each
hyperelastic material (either energy) replaced by the isotropic elastic material of `mat_hyper::small_strain_moduli` (`G`,
`K`; `E = 9 K G/(3 K + G)`, `nu = (3 K - 2 G)/(2 (3 K + G))`); for the neo-Hookean energy these are `(mu, kappa)`, the same as
ADR 0523's. For every other stress the surrogate is ADR 0523's code unchanged (Mooney-Rivlin refused). The internal force of the
state (start and end of each step, ADR 0523 D5) is the element routine with `u_0 = u_1` in the mode of the integrator when it is
`MaterialDiscreteGradient` (the midpoint branch at `dE = 0`: `S = mat_hyper::stress(E)`), in `Auto` otherwise (as now).
`strain_energy` sums `mat_hyper::energy` for a hyperelastic material when the mode is `MaterialDiscreteGradient`, the step-local
energy otherwise (as now). `start`'s empty states of the surrogate's layout (ADR 0523 amendment 2) hold for either energy
(`state_size` 0).

### D4. Files

`crates/rcfd-fem/src/mat_hyper.rs` (D2, additive), `crates/rcfd-fem/src/nl_transient/dg.rs` (the variant, the F-425 point
functions and their dispatch, the surrogate's branch, unit tests Q1 to Q4 and E1), `crates/rcfd-fem/src/nl_transient/emc.rs`
(the dispatch in `algorithmic_stress`, the mode of `internal_force`), `crates/rcfd-fem/src/nl_transient.rs` (the surrogate's
mode argument, the mode of the two internal-force calls, the branch of `strain_energy`; a few lines each), `crates/rcfd-fem/
tests/sa_dgm.rs` (new: the model checks), `crates/rcfd-fem/tests/sa_dgm_default_bits.rs` (new: B0). No case-file key, no
golden, `nonlinear.rs` and `contact.rs` untouched.

### D5. The Mooney-Rivlin cube

The cube of (c3) with `HyperelasticEnergy::mooney_rivlin(c10 = 3.5e4, c01 = 1.5e4, kappa = 1e6)`: `G = 2 (c10 + c01) = 1e5 =
mu`, `K = kappa`, the split `c01/(c10 + c01) = 0.3` of ADR 0541 D7's Mooney-Rivlin (`3.5e5`, `1.5e5`). The neo-Hookean cube of
(c3): `HyperelasticEnergy::neo_hookean(1e5, 1e6)`, the cube of ADR 0523.

### D6. Runs

- **Bar (A0 to A3)**: ADR 0523 D7's bar (its `spin`, `bar_options` with `MaterialDiscreteGradient`), 1000 steps.
- **NH cube (K0n to K2n), MR cube (K0r to K2r)**: ADR 0523 D7's cube (its `cube`, `cube_options` with
  `MaterialDiscreteGradient`), 500 steps each.
- **E2**: the NH cube with `DiscreteGradient` 100 steps; its state restored into a `MaterialDiscreteGradient` integrator of the
  same model; the two `strain_energy()`.
- **X1r, X2r (records)**: the NH bar (1000 steps) and the NH cube (500 steps) with `DiscreteGradient` and with
  `MaterialDiscreteGradient` from the same start: `max_n |u_a - u_b|_inf / |u_a|_inf`.
- **I3**: the isotropic (St. Venant-Kirchhoff) bar of ADR 0523 I1, 50 steps with `DiscreteGradient` and with
  `MaterialDiscreteGradient`: equal state bytes.

### D7. The default-bits record B0 (`crates/rcfd-fem/tests/sa_dgm_default_bits.rs`)

Written after this commit and run on the unchanged base f69eb128 before any library line; its FNV-1a 64 digests (state bytes
after every 10 steps, the kinetic and strain energies and momenta at the end) written into the file then. Content, every path
that D1 to D3 touch outside the new variant: the NH bar with `DiscreteGradient` (30 steps), the NH cube with `DiscreteGradient`
(30 steps), the NH cube with `Average` (20 steps), the isotropic cube with `DiscreteGradient` (20 steps), the isotropic bar with
`Auto` (the F-570 step, 20 steps), the strain energy and the start's internal force through a split run of the NH cube (10 +
bytes + restore + 10, `DiscreteGradient`). `sa_dg.rs`, `sa_dg_default_bits.rs`, `sa_emc.rs`, `sa_emc_default_bits.rs`,
`sa_nl_transient.rs`, `sa_nl_transient_default_bits.rs`, `sa_hyper.rs` and `sa_hyper_default_bits.rs` stay as they are and must
pass.

## Checks (limits fixed now)

| Id | Clause | What | Limit | Estimate behind the limit |
|---|---|---|---|---|
| A0 | c2 (set-up) | the steady St. Venant-Kirchhoff solve converges; the NH bar integrator with `MaterialDiscreteGradient` is built; 1000 steps converge, all measures finite | `Ok` | ADR 0523 A0 |
| A1 | c2 | NH bar `max_n |P_n - P_0|/|P_0|` | `<= 1e-10` | ADR 0523: `4.1e-13` |
| A2 | c2 | NH bar `max_n |J_n - J_0|/|J_0|` | `<= 1e-6` | ADR 0523: `7.5e-13` |
| A3 | c2 | NH bar `max_n |E_n - E_0|/|E_0|`, `W = mat_hyper::energy` | `<= 1e-6` | arithmetic 3; ADR 0523: `2.1e-13` |
| K0n | c3 (set-up) | NH cube: 500 steps converge, all measures finite | `Ok` | ADR 0523 K0 |
| K1n | c3 | NH cube `max_n |E_n - E_0|/|E_0|` | `<= 1e-7` | arithmetic 4: `~3e-9`, bound `1.5e-8` |
| K2n | companion | NH cube `max |P - P_0|/|P_0|`, `max |J - J_0|/|J_0|` | `<= 1e-10`, `<= 1e-8` | ADR 0523 K2: `2.1e-13`, `4.0e-13` |
| K0r | c3 (set-up) | MR cube (D5): 500 steps converge, all measures finite | `Ok` | arithmetic 4 |
| K1r | c3 | MR cube `max_n |E_n - E_0|/|E_0|` | `<= 1e-7` | arithmetic 4: `~3e-9`, bound `1.5e-8` |
| K2r | companion | MR cube momenta as K2n | `<= 1e-10`, `<= 1e-8` | the momentum argument of ADR 0523 (symmetric `S_alg`) |
| E1a | c4 | at 200 random strains per scale `1e-3, 1e-2, 1e-1, 3e-1` (bar and cube moduli; a draw with `det C <= 0` drawn again): `|W_D2 - W_425|/|W_425|`, `|S_D2 - S_425|_inf/|S_425|_inf`, `|D_D2 - D_425|_max/|D_425|_max` (the step-local `NeoHookean::point`, `tangent` against `mat_hyper::energy`, `stress`, `stress_and_tangent`) | each `<= 1e-12` | arithmetic 1: `<= 1.6e-13` (`W`), `<= 1e-15` (`S`) |
| E1b | c4 (companion) | the same at the scales `1e-6, 1e-4`, `W` normalised by `m_425`: `|W_D2 - W_425|/m_425`; `S`, `D` as E1a | each `<= 1e-12` | arithmetic 1: `<= 2.5e-16`, `<= 5e-16` |
| E1r | record | `|W_D2 - W_425|/|W_425|` at the scales `1e-6, 1e-4` | record | arithmetic 1: `1.6e-10`, `1.7e-12` (the cancellation of F-425's evaluation) |
| E2 | c4 (model) | the NH cube after 100 steps (`DiscreteGradient`): `strain_energy()` of the `MaterialDiscreteGradient` integrator restored at that state against the `DiscreteGradient` one, relative | `<= 1e-12` | arithmetic 5: `<= 2e-14` |
| Q1 | c1 (point) | `MaterialDiscreteGradient` point functions, NH and MR at the bar and the cube moduli (arithmetic 2), 200 random pairs at each scale `1e-6, 1e-4, 1e-2, 1e-1` (change `0.3` of it): `|S_alg . de - (W_1 - W_0)| / (sum_i |S_alg,i de_i| + m_0 + m_1)` | `<= 1e-12` | arithmetic 2: `1.41e-14` |
| Q2 | c1 (branch) | NH and MR (cube moduli): `de = 0` and `|dE| = 1e-13`: `S_alg` equals `S(E_m)` bit for bit (20 pairs each, base `0.1`); `|dE| = 1e-8` at base `0.1`: bit for bit at 100 pairs | exact | ADR 0523 D3; arithmetic 2: 100/100 |
| Q3 | c1 (second order) | NH and MR (cube moduli), the P3c sizes of ADR 0523 (`{1e-5, 1e-4, 1e-3}` at base `0.1`, `{1e-6, 1e-5, 1e-4}` at `1e-3`, 100 pairs each): the ratio of ADR 0523 P3c | `<= 5` | arithmetic 2: `3.18`, `3.33` |
| Q4 | D2 | `m_425 >= |W_425|` and finite at every strain of Q1 (both ends) | all | triangle inequality |
| I3 | D1 | isotropic bar, 50 steps: `MaterialDiscreteGradient` and `DiscreteGradient` equal state bytes | exact | the same code (D1) |
| X1r | record | NH bar, 1000 steps: `max_n |u_MDG - u_DG|_inf / |u_DG|_inf` | record | arithmetic 3 |
| X2r | record | NH cube, 500 steps: the same | record | arithmetic 4 |
| R1m | state | MR cube, 60 steps against 30 + bytes + restore + 30: equal state bytes | exact | no new state |
| T1m | threads | MR cube, 50 steps with 1 and 4 threads: equal state bytes | exact | the assembly of ADR 0514 D2 |
| F1m | refusals | with the energy-momentum step and `MaterialDiscreteGradient`: the MR and NH bar models accepted; NH in a BBar Hex8 block refused (key `"nl_transient"`, `ADR 0514`); with `DiscreteGradient` and with `Average`: MR refused (key `"nl_transient"`, `ADR 0523`); the NH model with the `Method` step and `MaterialDiscreteGradient` refused (`NotYetSupported`); an inverted strain in `mat_hyper::energy_magnitude` refused with `nonlinear::INVERSION_KEY` | all | D1 to D3 |
| B1 | defaults | B0: every digest of base f69eb128; the eight test files of D7 pass (in the `rcfd-fem` release suite) | exact; pass | D7 |
| B2 | defaults | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | no output of a case changed |
| S1 | build | `cargo test -p rcfd-fem --release` 0 failed; `cargo test -p rcfd-cli --release --test docs_site` pass; `cargo fmt -p rcfd-fem --check`; `cargo clippy -p rcfd-fem --all-targets -- -D warnings` | clean | |

A check that would take more than about two minutes in release is an ignored heavy record run once, with a fast test on the same
code path (as ADR 0523's tests, which run unignored).

## Negative controls and mutations

There is no F-570 average for Mooney-Rivlin (the `Average` mode keeps refusing it, D1), so no new negative-control run is
registered; ADR 0523's N1h (the average on the NH cube, `4.7e-3` against `2.8e-9`) stays and must pass (B1), and the mutation
MV1 is the negative control of the new path (the correction dropped must show in K1n and K1r). Planned mutations, on a scratch
copy (git archive of the code commit) with its own target, one at a time, `dg.rs` unit tests and `sa_dgm.rs` run for each:

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MV1 | the correction dropped on the F-425 path (`S_alg = S(E_m)` always) | K1n, K1r, Q1 |
| MV2 | `energy_magnitude` returns 0 (`beta = 0`) | Q2; recorded, not required (as ADR 0523 MU10) |
| MV3 | `strain_energy` of the new mode evaluates the MR energy with `c01 = 0` | K1r |
| MV4 | the midpoint stress of the new mode for MR from the NH energy of the same small-strain moduli | Q3 or Q1; recorded, not required (the step conserves the energy of `W` whatever `S` is, ADR 0523 MU7) |
| MV5 | the isotropic material in the new mode takes the F-570 average | I3 |
| MV6 | the MR material refused in the new mode | F1m, K0r |
| MV7 | the step-local volumetric energy scaled by `1 + 1e-9` | E1a, E2 |
| MV8 | `energy_magnitude` without the volumetric part | Q4 |
| MV9 | the new mode's internal force at the state from the step-local NH stress multiplied by `1 + 1e-6` (an inconsistent start force) | K1n or A3; recorded, not required |

## Decision rule

- **done**: every check of the table met (E1r, X1r, X2r are records), MV1, MV3, MV5, MV6, MV7, MV8 caught, fmt and clippy clean,
  B1 and B2 unchanged.
- **partial**: B1 and B2 unchanged, A1 to A3, K1n, K1r and E1a met, and a miss of a companion check diagnosed and recorded as a
  `*_known_miss` test, or a required mutation uncaught; also partial if the lead rules that keeping the step-local energy on
  `DiscreteGradient` beside the new variant does not meet "removed or equals F-425's" (open question 1).
- **todo**: otherwise (in particular any change of the default bits, of B0 or of ADR 0523's tests).

## Limits (not covered)

- `DiscreteGradient` keeps the step-local energy and its Mooney-Rivlin refusal (the existing F1h); the two variants are not
  folded (open question 1).
- `Auto` refuses the hyperelastic material as before (ADR 0523 open question 5; F1ec of ADR 0514 would need updating).
- No `Average` for Mooney-Rivlin; no other hyperelastic energy; no BBar/F-bar (mean-dilatation) block in the energy-momentum
  step (ADR 0514's refusal stays); no shells; the Method step keeps refusing the hyperelastic material (ADR 0541 D4).
- The tangent leaves out the derivative of the correction (as ADR 0523).
- No case-file key.

## Open questions (taken on the maintainer's standing instruction of 2026-11-11, take the recommendation; none changes a default)

1. **Removal against comparison.** Options: (a) a new variant with F-425's functions, the step-local energy kept on
   `DiscreteGradient` and shown equal (E1), no existing test edited; (b) `DiscreteGradient` switched to F-425's functions, the
   step-local code removed, the Mooney-Rivlin row of `sa_dg.rs` F1h edited and ADR 0523's runs re-recorded (an existing test
   edited: the maintainer's approval). Recommendation (a) now, taken; (b) as a later clean-up once the maintainer allows the F1h
   edit, with `MaterialDiscreteGradient` kept as an alias.
2. **F-425's energy at small strain.** F-425's isochoric energy loses digits as `eps/|E|` at small strain (arithmetic 1). Options:
   (a) leave it (the energy is consistent with its stress to that rounding and the step's rounding test uses its term
   magnitude); (b) a follow-up evaluating it in the step-local form without the cancellation (changes F-425's bits of
   `mat_hyper::energy`). Recommendation (a), recorded for the lead.
3. **Case-file key.** Not added (brief). Recommendation as ADR 0523 open question 2, with `"material_discrete_gradient"`.

## Addendum 1: results (2026-11-15)

Runs: B0 recorded on the base f69eb128 (commit `62d608de`, `f611-scratch/b0-base.log`) before any library line. The library and
the tests in commit `a7b58e10`; the registered runs on it, each once: the unit checks E1, Q1 to Q4 (`run1_unit.log`, one
thread), the model checks of `sa_dgm.rs` (`run1.log`). Then `cargo test -p rcfd-fem --release` (`fem_all.log`, exit 0: 1012
passed, 0 failed, 274 ignored; it ran `sa_dgm` and the unit checks once more as part of the suite, not a registered run, its
numbers not used), `baseline_hashes` (`baseline.log`), `docs_site` (`docs_site.log`), clippy (`clippy.log`). No amendment; no
limit changed; nothing re-run to pick a result.

| Id | Measured | Limit | Result |
|---|---|---|---|
| A0 | SVK steady solve converged; NH bar integrator built; 1000 of 1000 steps, no halving, largest final residual `9.94e-8` | `Ok` | met |
| A1 | `3.805e-13` | `<= 1e-10` | met |
| A2 | `1.002e-12` | `<= 1e-6` | met |
| A3 | `2.322e-13` | `<= 1e-6` | met |
| K0n | 500 of 500 steps, no halving, largest final residual `9.85e-11` | `Ok` | met |
| K1n | `2.832e-9` | `<= 1e-7` | met |
| K2n | `dP = 1.458e-13`, `dJ = 3.291e-13` | `<= 1e-10`, `<= 1e-8` | met |
| K0r | 500 of 500 steps, no halving, largest final residual `9.87e-11` | `Ok` | met |
| K1r | `2.832e-9` | `<= 1e-7` | met |
| K2r | `dP = 1.423e-13`, `dJ = 2.948e-13` | `<= 1e-10`, `<= 1e-8` | met |
| E1a | `W 1.69e-13`, `S 1.42e-15`, `D 7.84e-16` | `<= 1e-12` each | met |
| E1b | `W/m_425 2.27e-16`, `S 5.46e-16`, `D 6.16e-16` | `<= 1e-12` each | met |
| E1r (record) | `1.30e-10` at `1e-6`, `1.49e-12` at `1e-4` | record | - |
| E2 | `2.21e-16` (`W = 513.9648747943739` against `...740`) | `<= 1e-12` | met |
| Q1 | `1.417e-14` (corrected 2888 of 3200 pairs) | `<= 1e-12` | met |
| Q2 | bit for bit at `dE = 0`, `1e-13` and `1e-8` | exact | met |
| Q3 | NH `1.29`, MR `1.45` | `<= 5` | met |
| Q4 | `m_425 >= |W_425|`, finite, at every end | all | met |
| I3 | equal state bytes | exact | met |
| X1r (record) | `2.25e-13` | record | - |
| X2r (record) | `1.67e-13` | record | - |
| R1m | equal state bytes | exact | met |
| T1m | equal state bytes | exact | met |
| F1m | MR and NH accepted; MR refused with `DiscreteGradient` and `Average` (ADR 0523); NH with the Method step refused (`NotYetSupported`, the follow-up of ADR 0541); NH in a BBar block refused (ADR 0514); `energy_magnitude` of an inverted strain refused (`deformation gradient`) | all | met |
| B1 | the six B0 digests of f69eb128; `sa_dg` (9 passed), `sa_dg_default_bits`, `sa_emc` (10 passed, 2 ignored), `sa_emc_default_bits`, `sa_nl_transient` (7 passed, 6 ignored), `sa_nl_transient_default_bits`, `sa_hyper` (11 passed, 3 ignored), `sa_hyper_default_bits` pass | exact; pass | met |
| B2 | `baseline_hashes`: 3 passed, 1 ignored | pass | met |
| S1 | `rcfd-fem` release 0 failed; `docs_site` 2 passed; `cargo fmt -p rcfd-fem -- --check` clean; `cargo clippy -p rcfd-fem --release --all-targets -- -D warnings` clean | clean | met |

**Mutations** (`f611-scratch/mutations.log`, `mutate.py`; a git-archive copy of `a7b58e10` with its own target
`target-f611-mut`; the unit checks and `sa_dgm` run for each):

| Id | Caught by |
|---|---|
| MV1 correction dropped | K1n `2.39e-3`, K1r `2.39e-3` (not Q1: Q1 tests `discrete_gradient` itself, the mutation was in `material_stress`) |
| MV2 `beta = 0` (recorded, not required) | caught: Q2, Q4, E1b, and A0 (the bar's first step does not converge: the correction taken on the rounding of F-425's `W` divided by `|dE| ~ 1e-10`) |
| MV3 MR strain energy without `c01` | K1r `3.1e-2` |
| MV4 MR midpoint stress from the NH energy (recorded, not required) | caught: Q3 (MR `1.9e8`), Q2; `sa_dgm` passed, as registered |
| MV5 isotropic material on the F-570 average | I3 |
| MV6 MR refused in the new mode | F1m, K0r, R1m, T1m |
| MV7 step-local volumetric energy `* (1 + 1e-9)` | E1a (`W 1.9e-9`), E1b, E2 (`6.1e-10`) |
| MV8 `energy_magnitude` without the volumetric part | Q4 |
| MV9 state internal force `* (1 + 1e-6)` (recorded, not required) | not caught (A3, K1n, K1r unchanged in every digit shown). Reading, not a measured decomposition: the cube starts at `u = 0` (zero internal force) and the step's residual is formed from the midpoint stress of each step, so the stored internal force does not enter the energy. Also a deviation: registered as "from the step-local NH stress", applied as F-425's internal force scaled (the step-local stress cannot evaluate Mooney-Rivlin). |

All six required mutations (MV1, MV3, MV5, MV6, MV7, MV8) caught.

**Diagnosis worth recording.** (i) K1n and K1r agree to four digits (`2.831984e-9`, `2.831973e-9`) and with ADR 0523's K1
(`2.832e-9`): the energy error of the cube is set by the Newton residual tolerance at the same steps, not by the energy (MV3
shows that the MR energy is the one measured). (ii) MV2 confirms the reason for D2: with F-425's evaluation of `W`, whose
rounding is `eps |tr E|`, a rounding bound built on `|E|^2` would take the correction on noise at the bar's strain changes and
the step would not converge. (iii) E1r: the step-local and F-425's energies differ by `1.3e-10` relative at strains of `1e-6`,
all of it the cancellation in F-425's evaluation (E1b: `2.3e-16` of its term magnitude); the clause's "within 1e-12 at a
point" holds at every strain of `1e-3` and above (E1a) and in the sense of rounding at every scale (E1b).

**Deviations.** (1) MV9 applied in a form other than registered (above); it is not required. (2) MV1 not caught by Q1, as
explained; it is caught by the registered K1n and K1r. (3) The step-local energy is kept (the registered choice), not removed.

**Decision: done** by the registered rule (every check met, the records recorded, MV1, MV3, MV5, MV6, MV7, MV8 caught, fmt and
clippy clean, B1 and B2 unchanged). **Proposed status: done**, unless the lead rules (open question 1) that keeping the
step-local energy on `DiscreteGradient` beside the new variant falls short of the acceptance; then partial, as the decision
rule says.

Remaining questions (taken on the maintainer's standing instruction of 2026-11-11; none changes a default):

1. Folding `DiscreteGradient` and `MaterialDiscreteGradient`: options (a) keep both (now); (b) switch `DiscreteGradient` to
   F-425's functions and remove the step-local code, which needs the Mooney-Rivlin row of `sa_dg.rs` F1h edited and changes the
   bits of ADR 0523's neo-Hookean runs (X1r, X2r: `~2e-13` of `u`). Recommendation (b) as a maintainer-approved clean-up.
2. F-425's energy at small strain (`eps/|E|` relative): options (a) leave; (b) evaluate `mat_hyper::energy_parts` in the
   cancellation-free form of ADR 0523 D2 (changes the bits of `mat_hyper::energy`, not of its stress). Recommendation (a) now,
   (b) if a small-strain energy check ever needs it.
3. `Auto` and the hyperelastic material (ADR 0523 open question 5): unchanged; recommendation (b) of ADR 0523 (`Auto` takes
   `MaterialDiscreteGradient` for a hyperelastic material) once the maintainer allows F1ec of ADR 0514 to be updated.
4. Case-file key: recommendation `algorithmic_stress = "material_discrete_gradient"` with ADR 0523 open question 2.

## For the lead: roadmap entries

Proposed status of F-611: done (by the registered rule; partial if the lead rules against keeping the step-local energy beside
the new variant, open question 1).

> Structural solver, advanced tier. ADR 0544, 2026-11-15: nl_transient gains AlgorithmicStress::MaterialDiscreteGradient, the discrete gradient of ADR 0523 with F-425's point functions (mat_hyper energy, stress, tangent at E_m, and the new mat_hyper::energy_magnitude as the rounding bound), taking the neo-Hookean and the Mooney-Rivlin material; DiscreteGradient and Average keep the step-local neo-Hookean of ADR 0523 (bits unchanged, Mooney-Rivlin refused there), Auto unchanged. Neo-Hookean spinning bar over ten turns: linear momentum 3.8e-13 (1e-10), angular momentum 1.0e-12 (1e-6), energy 2.3e-13 (1e-6). Large-strain cubes over 500 steps: neo-Hookean energy 2.8e-9, Mooney-Rivlin (c10 3.5e4, c01 1.5e4, kappa 1e6) energy 2.8e-9 (1e-7), momenta at most 3.3e-13. The step-local energy equals F-425's at a point within 1.7e-13 for strains from 1e-3 (1e-12), stress 1.4e-15, tangent 7.8e-16; at strains of 1e-6 the two differ by 1.3e-10 relative, the cancellation in F-425's evaluation (2.3e-16 of its term magnitude). Six of six required mutations caught (and two of three recorded ones); B0 digests of the base, baseline_hashes and the F-607, F-570 and F-425 tests unchanged. No case-file key. Folding DiscreteGradient into the new variant needs the Mooney-Rivlin row of the F-607 refusal test edited (maintainer's call).

> Proposed follow-up (no new item added): fold AlgorithmicStress::DiscreteGradient into MaterialDiscreteGradient and remove the step-local neo-Hookean of ADR 0523 once the maintainer allows the Mooney-Rivlin row of sa_dg.rs f1h_refusals to be edited (ADR 0544, 2026-11-15, remaining question 1).
