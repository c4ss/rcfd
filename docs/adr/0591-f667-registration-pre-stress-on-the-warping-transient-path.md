# ADR 0591: F-667 registration: the pre-stress on the warping transient path

Status: **REGISTERED 2026-11-15, written before any code of F-667 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-667 was written, compiled or run in the repository: the files listed under "What was
read" were read, and the arithmetic below was done by hand and evaluated with one plain numpy script in the scratch folder
(`w49i-work/f667-scratch/arith.py`, log `arith.log`: the cubic-Hermite Vlasov twist model with Wagner's axial term of ADR 0585
`arith.py` re-typed, a Newmark average-acceleration integrator on it, the continuous modal step response with and without Rayleigh
damping, and a pinned Euler-Bernoulli bending series under the axial force; no rcfd code; disclosed, its numbers quoted below).
The bit record B0 of the default warping transient path is a test file written and run on the base after this commit and before
any library code (D6). Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-667 (deps F-661), structural solver, advanced tier, the follow-up proposed by
[ADR 0585](0585-f661-registration-pre-stress-on-the-warping-harmonic-path.md) (its question Q2). The transient path is that of
[ADR 0574](0574-f635-registration-warping-unknown-on-the-harmonic-and-transient-paths.md) D3 (`WarpingTransient`, the
integrator of ADR 0389 on the reduced 14-unknown matrices); the pre-stress is `WarpingPrestress` of
[ADR 0548](0548-f626-registration-warping-unknown-on-the-static-solve-modes-and-pre-stressed-modes.md) D2 and its geometric
stiffness that of `matrices` (ADR 0546 D5, the load points of ADR 0582 and ADR 0588). Number 0591 given by the lead. Branch
`f667-work` from 5e6c3587 (the F-663 branch, which carries F-635, F-643, F-661 and F-663), worktree `w49i-work/f667`, scratch
`w49i-work/f667-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test edited;
no new dependency, no unsafe code, the crate graph unchanged; clean-room. Every choice below that the maintainer would normally
be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation), except the open
questions at the end.

## What F-667 asks

The acceptance, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. WarpingTransient takes the pre-stress of the warping
> static solve (K + K_sigma); the midspan twist of the fork-supported I beam under a step torque and an axial force N = +/-0.5 of
> the torsional buckling load follows the continuous modal series with omega_m^2 = (G J k_m^2 + N I_p/A k_m^2 + E C_w
> k_m^4)/(rho I_p) within 1.5e-2 of the static twist over two periods at 16 elements; every result without the option
> bit-identical.

Clauses: **a0** every limit registered here; **a1** `WarpingTransient` takes the pre-stress of the warping static solve
(`WarpingPrestress::from_static`) and integrates on `K + K_sigma`; **a2** the midspan twist under the step torque with `N = +0.5
|N_T|` and with `N = -0.5 |N_T|` within `1.5e-2` of the static twist (of the series) from the continuous modal series over two
periods at 16 elements; **a3** every result without the option bit-identical.

### The closed form (C5)

C4 of ADR 0585 (derived there from first principles: Vlasov torsion with Wagner's term, `E C_w phi'''' - (G J + N I_p/A) phi'' +
rho I_p phi_tt = 0`, fork supports satisfied by `sin(k_m x)`, `k_m = m pi/L`), `omega_m^2 = (G J k_m^2 + N I_p/A k_m^2 + E C_w
k_m^4)/(rho I_p)`, `N` tension positive (the reading of ADR 0585, the same as the code's). Expanding the step torque `T` at `x = a`
in the modes normalised by `int rho I_p sin^2 dx = rho I_p L/2` gives the modal step response of an undamped oscillator from rest,

**C5**: `phi_c(a, t) = sum_m (2 T sin^2(k_m a)/(rho I_p L omega_m^2)) (1 - cos omega_m t)`,

the static twist `phi_c,s = sum_m 2 T sin^2(k_m a)/(rho I_p L omega_m^2)` (4000 terms; the terms fall as `m^-4`). With Rayleigh
damping `C = alpha M + beta (K + K_sigma)` (D2) each mode has `c_m = alpha + beta omega_m^2` and the response of the damped
oscillator from rest (`1 - e^{-zeta w t}(cos w_d t + zeta/sqrt(1 - zeta^2) sin w_d t)`, `zeta_m = c_m/(2 omega_m)`; the
overdamped form `1 + (r_2 e^{r_1 t} - r_1 e^{r_2 t})/(r_1 - r_2)` for `zeta_m > 1`) (**C5d**). "Two periods" is `2 T_1(N)`, `T_1(N)
= 2 pi/omega_1(N)`.

### Readings, registered

- **"Fork-supported I beam"**, **"axial force N"**, **"the pre-stress of the warping static solve"**: exactly those of ADR 0585
  (the I section `doubly_symmetric_i(0.1, 0.01, 0.18, 0.006, 5/6)`, steel `E = 2e11`, `nu = 0.3`, `rho = 7850`, `L = 3` along
  `x`, 16 Beam2 elements, `u = 0` at node 0, `v = w = phi = 0` at both ends, the warping free at both ends; the pre-stress
  `WarpingPrestress::from_static` of the warping static solve of the same beam with the force `(N, 0, 0)` at `xl` and no other
  load). The torque is not in the pre-stress.
- **"Step torque"**: as ADR 0574 TR1: the torque `T = 1000` about `x` at midspan (node 8), `load_vector` times
  `LoadTable::constant(1, 0, 10)`, started from rest at `t = 0`; Newmark average acceleration (`TransientOptions::new(dt)`),
  `dt = T_1(N)/400`, 800 steps.
- **"Takes the pre-stress ... (K + K_sigma)"**: the integrator is built on the reduced `K + K_sigma` in place of `K` (D1): the
  equation of motion `M a + C v + (K + K_sigma) u = f(t)`. The Rayleigh stiffness term is then `beta (K + K_sigma)` (the
  convention of ADR 0585 D1 and ADR 0543 D5, Q1 of ADR 0585 option (a)); the integrator's `strain_energy` is `(1/2) u^T (K +
  K_sigma) u`, the energy of the linearised pre-stressed system. Taken on the maintainer's standing instruction of 2026-11-11
  (take the recommendation).
- **"The option"**: a new constructor `WarpingTransient::with_prestress(model, warping, loads, options, prestress:
  &WarpingPrestress)`. `TransientOptions` belongs to `transient.rs` (a shared module) and is not touched; a constructor keeps
  `WarpingTransient::new` and everything it runs unchanged. Opt-in; no case-file key (F-625).

### Compression: the full beam is past its weak-axis Euler load (registered handling)

`0.5 |N_T| = 6.407e5` N is `1.749` times the weak-axis Euler load `pi^2 E I_z/L^2 = 3.663e5` N (ADR 0585). Under `N = -0.5 |N_T|`
the full fork beam's `K + K_sigma` has a negative eigenvalue (the first weak bending mode, `omega^2 = -12447` by the pinned Euler
estimate, arithmetic 4): about this static state a transient run is unstable, and any excitation of that mode grows like
`cosh(111.6 t)`, a factor of about 3700 in two torsional periods (`2 T_1 = 0.0737` s). Registered handling, decided before the
run:

1. **The acceptance is checked on the full beam as it asks (TR1-)**, because the step torque at the shear centre of a doubly
   symmetric section does not excite bending: the load has no bending component, and with an axial pre-stress (no moments, no
   shear) the element matrices `K`, `M` and `K_sigma` have exact zero entries between the twist unknowns `(phi, chi)` and the
   bending and axial unknowns (ADR 0546 D2: the Wagner block is on the twist only; ADR 0585 HB- measured exactly 0 on the
   harmonic path). The bending components then stay exactly zero through the factorisation and every step. **HB-** checks this
   on the run: the bending and axial components of every node over every step against the largest twist.
2. **The same check on the torsion-only model (TR1T-)**, which is stable (its `K + K_sigma` is positive definite: `omega_1^2(N)
   = 29059 > 0`), registered beside it, so the clause has a reading that does not rely on the decoupling.
3. **The instability is shown, not hidden (LG+-)**: a separate run of the full beam with a lateral step force `(0, 100, 100)` N
   at midspan (no torque) under the same pre-stress. In compression the lateral displacement grows (the ratio of its largest
   magnitude in the second period to that in the first, LG-, `> 10`, estimate 63); in tension it stays bounded (LG+, `<= 1.5`,
   estimate 1.005). This registers that the path integrates the indefinite system as the linear equations say, and that an
   unsymmetric load on the compressed full beam is not a meaningful step response.
4. **No refusal of an indefinite `K + K_sigma`.** The integrator factors the effective matrix `M + beta dt^2 (K + K_sigma)` (with
   damping `c_m M + c_k (K + K_sigma)`), which stays positive definite while `beta dt^2 |lambda_neg| < 1`: here `0.25 (9.21e-5)^2
   12447 = 2.6e-5`. A time step large enough to make it indefinite is refused by the integrator's own pivot test (key
   `"effective matrix"`), as for any model. A Sturm count reporting the negative eigenvalues is the follow-up proposed by ADR
   0585 (Q3 there); not done here (open question Q2).

### Audit of refusals this change could lift (done before this text)

None. `WarpingTransient` has no pre-stress today and nothing refuses one (no parameter takes it). The plain
`transient::Integrator::new` refuses beams (ADR 0574 amendment 1) and is not touched. Searched the warping tests (`_f598`,
`_f626`, `_f627`, `_f635`, `_f643`, `_f661`, `_f663` and the bit records) for `WarpingTransient` with a pre-stress: no hit.

## What was read (no run)

- `AGENT_RULES.md`; the brief; ADR 0585 (whole); ADR 0574 D2, D3, D5, arithmetic 4, TR1 to TR5, amendment 1 and the addendum;
  ADR 0548 D2 (`WarpingPrestress`); the "For the lead" sections of ADR 0572, 0573 and 0585.
- `crates/rcfd-fem/src/beam_warping.rs`: module text, `reduced_dynamic`, `WarpingHarmonic::with_prestress` (the reduction of the
  geometric stiffness), `WarpingTransient` (whole), `triplets_of`, `augmented`, `WarpingPrestress`.
- `crates/rcfd-fem/src/transient.rs`: `Integrator::from_matrices`, `build`, `finish` (the effective matrix `c_m M + c_k K`),
  `factor` (the pivot test), `kinetic_energy`, `strain_energy`, `combine`; `rcfd-linalg` `CsrMatrix::from_triplets` (a stable
  sort, duplicates summed in order).
- Tests: `sa_beam_warping_f635.rs` (`tr_transient_twist`, `series_step`, the refusals), `sa_beam_warping_f661.rs` (the fork
  beam, `axial_prestress`, `torsion_only`, `n_t`, `c4_parts`), `sa_beam_warping_f661_bits.rs` and `_f635_bits.rs` (the digest).

## Design

### D1. The constructor

`WarpingTransient::with_prestress(model, warping, loads, options, prestress)`:

1. `reduced_dynamic` (the checks of ADR 0574 D3 and the reduced `K`, `M`) unchanged;
2. the reduced geometric stiffness exactly as `WarpingHarmonic` with a pre-stress (ADR 0585 D1 step 2): `prestress.check_for`
   (RF10, key `"warping"`), `matrices_impl` with its `check_torque` (RF8, key `"warping"`; the `Prestress::check_fits`
   refusals), its `geometric` matrix reduced by the same prescribed and restrained equations; a different free set is refused
   (key `"warping"`). This step is moved into one private function shared by both paths (the same operations in the same order,
   so the pre-stressed harmonic path keeps its arithmetic);
3. `K_e = K + K_sigma` as `CsrMatrix::from_triplets(n, triplets(K) ++ triplets(K_sigma))` (each entry `k + g`);
4. the load patterns reduced as in `new`, and `transient::Integrator::from_matrices(K_e, M, reduced loads, options)`.

`WarpingTransient::new` is not changed: it runs the lines of ADR 0574 D3 (a private shared body is allowed only if it runs the
same operations in the same order without the option). Everything after construction (`start`, `start_at_rest`, `run`,
`displacement`, `velocity`, `integrator`) is the same type and code.

### D2. Damping and energy

The Rayleigh damping of `TransientOptions` is `alpha M + beta K_e` (the integrator's own). No loss factor exists on the transient
path. The energies of the integrator are those of `K_e` and `M`.

### D3. What is not changed

`reduced_dynamic`, `WarpingTransient::new`, `WarpingHarmonic::new` without the option, `matrices`, `modes_impl`, `solve_static`,
`WarpingPrestress`, `transient.rs` and every other module.

### D4. Refusals

Those of `WarpingTransient::new` (ADR 0574 D3) and, with the pre-stress, RF8 and RF10 of ADR 0548 D3 and the `check_fits`
refusals of `Prestress` (each with its key), and the integrator's own.

### D5. Files

Changed: `src/beam_warping.rs` (the constructor after `WarpingTransient::new`, the shared reduction of the geometric stiffness,
the module text). New tests: `tests/sa_beam_warping_f667.rs`, `tests/sa_beam_warping_f667_bits.rs`.

### D6. The bit record B0

`tests/sa_beam_warping_f667_bits.rs`, written and run on the base (5e6c3587 plus this ADR, no library line changed): digests
(FNV-1a 64 of ADR 0291) of the full displacement and velocity of `WarpingTransient::new` on the 16-element fork beam under the
midspan step torque after 100 steps of `dt = T_1/400` (`T_1 = 2 pi/sqrt(C3)`), from rest: Newmark undamped; Newmark with
Rayleigh `zeta = 0.02` (`alpha = 0.02 sqrt(C3)`, `beta = 0.02/sqrt(C3)`); Newmark with the warping held at both ends; and
generalised-alpha (`Method::GeneralisedAlpha { rho_inf: 0.8 }`) undamped. It runs unchanged after the change.

## Arithmetic done before the code (disclosed)

`arith.log`. Section as ADR 0585 (`A = 3.08e-3`, `I_p = 2.265257e-5`, `G J = 6125.128`, `E C_w = 3008.333`), `|N_T| =
1.281370e6` N.

1. **C4, C5.** `N = +0.5 |N_T|`: `omega_1 = 295.25781` rad/s, `T_1 = 2.128034e-2` s, `dt = 5.320084e-5` s, `phi_c,s =
   4.5060729e-2`. `N = -0.5 |N_T|`: `omega_1 = 170.46718`, `T_1 = 3.685862e-2`, `dt = 9.214656e-5`, `phi_c,s = 1.3161054e-1`.
   The 16-element Hermite static twist: `-5.7e-6` (tension) and `-2.7e-7` (compression) from `phi_c,s`.
2. **Newmark on the 16-element twist model** (average acceleration, from rest, the step torque, 800 steps), `max_n |phi_n -
   phi_c(t_n)|/phi_c,s`: tension `6.48e-3`, compression `6.52e-3` (`dt/2`: `1.9e-3`, `2.4e-3`: as ADR 0574, mostly the period
   error of the higher modes); the largest twist `1.9946` and `1.9972` of `phi_c,s`; the energy `KE + (1/2) u^T K_e u - f^T u`
   (zero at the start) stays within `6.0e-13` and `2.1e-12` of `(1/2) T phi_c,s`. Without the pre-stress against the series with
   `N`: `2.64` and `1.90`. Mutations: `-K_sigma` `5.58` / `1.91`; `2 K_sigma` `1.44` / `77.4` (compression: at the buckling
   load); `0.5 K_sigma` `1.22` / `1.77`.
3. **Rayleigh** `zeta = 0.02` on mode 1 (`alpha = zeta omega_1(N)`, `beta = zeta/omega_1(N)`), `beta` on `K_e`, against C5d:
   `2.2e-4` / `1.7e-4`; with `beta` on `K` only: `3.2e-2` / `9.0e-2` (not a mutation of this code: the integrator receives one
   stiffness; the check fixes the convention). The damped series differs from the undamped one by `0.24` / `0.23`.
   Mass-proportional `alpha = 2 omega_1(N)`, `ceil(25/(omega_1 dt)) = 1592` steps: the final twist `1.9e-7` / `1.2e-6` from the
   model's static twist and `5.5e-6` / `9.1e-7` from `phi_c,s`.
4. **Lateral control** (pinned Euler-Bernoulli in both planes under `N`, `omega^2 = (E I k_m^4 + N k_m^2)/(rho A)`, odd modes to
   399, Timoshenko and the discretisation ignored; the step force `100` N in `y` and in `z` at midspan): `R = max_{(T_1, 2 T_1]}
   |u_lat| / max_{(0, T_1]} |u_lat|` = `1.0053` (tension), `63.09` (compression; the weak mode `omega^2 = -12447`). The growth is
   exponential in `sqrt(|omega^2|)`: a 5 percent error of the FE weak mode changes the compression `R` by a factor of about
   `e^{0.1}` only.

## Checks (limits fixed now)

Every aggregated error is asserted finite before it is compared (a non-finite value is a miss). The fork beam of the readings,
16 elements, the step torque at midspan, the pre-stress of `N = s 0.5 |N_T|` (`s = +1` tension, `-1` compression),
`dt = T_1(N)/400`, 800 steps, unless stated; "twist" is component 3 of node 8; `phi_c` and `phi_c,s` those of C5 with that `N`;
`+-` means run for both signs.

| id | check | limit | estimate |
|---|---|---|---|
| B0 | the bit record of D6 (the default `WarpingTransient::new`), base and after; the existing bit records (`sa_beam_warping_f598_bits`, `_f626_bits`, `_f635_bits`, `_f661_bits`, `_f663_bits`) unchanged | equal (bit) | exact |
| P0 | `with_prestress` with a zero pre-stress (`Prestress::uniform` zero, `torsion_free`): the displacement and velocity of B0's undamped and Rayleigh cases after 100 steps bit-equal to `new` | equal (bit) | exact (`k + 0 = k`, the same pattern) |
| TR1+- | **acceptance a1, a2**: full beam, `with_prestress`: `max_n |phi_n - phi_c(t_n)|/phi_c,s` over steps 0 to 800 | `<= 1.5e-2` | `6.5e-3`, `6.5e-3` |
| TR1T+- | TR1 on the torsion-only model (the pre-stress from the fork static solve, as ADR 0585 H2T) | `<= 1.5e-2` | `6.5e-3`, `6.5e-3` |
| TR2+- | TR1: the largest twist over the run over `phi_c,s` | in `[1.9, 2.05]` | `1.995`, `1.997` |
| TR4+- | TR1: `max_n |KE + SE - f^T u|/((1/2) T phi_c,s)` (integrator `kinetic_energy`, `strain_energy`; `u` the full displacement, `f` of `load_vector`) | `<= 1e-9` | `6e-13`, `2e-12` |
| TRD+- | Rayleigh `zeta = 0.02` of D2 (`alpha = 0.02 omega_1(N)`, `beta = 0.02/omega_1(N)`): as TR1 against C5d | `<= 2e-3` | `2.2e-4`, `1.7e-4` |
| TR3+- | mass-proportional `alpha = 2 omega_1(N)`, 1592 steps: the final twist against `phi_c,s` | `<= 2e-5` relative | `5.5e-6`, `9.1e-7` |
| TR5+- | negative control: `WarpingTransient::new` (no pre-stress) against C5 with `N`, as TR1 | `> 0.5` | `2.64`, `1.90` |
| HB+- | TR1 on the full beam: the components 0, 1, 2, 4, 5 of every node over every step, `max|.|` over the largest twist | `<= 1e-10` | `0` |
| LG- | the lateral control of the compression handling, item 3, compression: `R` of arithmetic 4 on node 8 (`|u_lat| = hypot` of components 1 and 2), steps 1 to 400 against 401 to 800 | `> 10` | `63` |
| LG+ | the same in tension | `<= 1.5` | `1.005` |
| R | refusals of `with_prestress`, key each: a `torsion_free` pre-stress of a static solve with the midspan torque (RF8, `"warping"`); a `from_static` pre-stress of the `saint_venant()` blocks (RF10, `"warping"`); a `from_static` pre-stress of the 8-element model on the 16-element model (`"warping"`); a load pattern of the six-unknown length (`"load pattern"`) | exact | |

Negative controls: TR5 (the option acts), P0 (a zero pre-stress is the default path), TRD (the Rayleigh convention: `beta K`
only would give `3.2e-2` / `9.0e-2`), LG+- (the indefinite system is integrated as it is), HB (no spurious coupling).

## Planned mutations (on a scratch copy, one at a time, own `CARGO_TARGET_DIR`)

| id | mutation | expected caught by |
|---|---|---|
| Z1 | `K_sigma` not added (the integrator on `K`) | TR1+-, TR1T+-, TRD+-, TR3+, LG- |
| Z2 | `- K_sigma` | TR1+-, TR1T+-, LG+- |
| Z3 | `2 K_sigma` | TR1+-, TR1T+- |
| Z4 | `0.5 K_sigma` | TR1+-, TR1T+- |
| Z5 | `K_sigma` not reduced (the full geometric matrix indexed as the free set) | a refusal, or TR1+- |
| Z6 | `check_for` skipped (`check_torque` true) | R (RF10 cases) |

## Decision rule

**done**: B0, P0, TR1+-, TR1T+-, TR2+-, TR4+-, TRD+-, TR3+-, TR5+-, HB+-, LG+-, R met, and every mutation caught. **partial**:
any registered check missed (a `*_known_miss` record), any mutation not caught, or any check met only after a post-run
amendment. **todo**: TR1+ or TR1- missed with TR1T of the same sign missed, or B0 missed.

## Limits (not covered)

- The linear transient about the pre-stressed state only: no nonlinear (large-twist) transient, no follower or time-varying
  pre-stress, no pre-stress from a transient.
- No warping inertia `rho C_w` (C4 neglects it too).
- The pre-stress with torque and with moments (lateral-torsional coupling) is taken (the same `matrices`) but not checked
  against a closed form; the load points of ADR 0582 and ADR 0588 enter `K_sigma` when the `Warping` has them, not checked here.
- Monosymmetric sections (twist-bending coupling under an axial pre-stress): not checked.
- No warning or refusal of an indefinite `K + K_sigma` (the compression handling, item 4); no case-file key (F-625).

## Open questions (to be taken on the maintainer's standing instruction of 2026-11-11 when answered; none changes a default)

- **Q1. The Rayleigh stiffness.** (a) `beta (K + K_sigma)` (this ADR, the integrator's own on the stiffness it is given; as ADR
  0585); (b) `beta K` only (needs a second stiffness in the integrator, a change of `transient.rs`). Recommendation (a).
- **Q2. An indefinite `K + K_sigma`.** (a) integrate as the linear equations say (this ADR); (b) report the negative-eigenvalue
  count (the follow-up of ADR 0585 Q3) on the transient too. Recommendation (b) with that follow-up.
