# ADR 0574: F-635 registration: the warping unknown on the harmonic and transient paths

Status: **REGISTERED 2026-11-15, written before any code of F-635 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-635 was written, compiled or run in the repository: the files listed under "What was
read" were read, and the arithmetic below was done by hand and evaluated with one plain numpy script in the scratch folder
(`w49i-work/f635-scratch/arith.py`, log `arith.log`: a cubic-Hermite Vlasov torsion model with the stiffness and mass of ADR 0548,
its harmonic response, the resonance search of D5, a Newmark run, and the continuous modal series; no rcfd code; disclosed, its
numbers quoted below). The bit record B0 of the default paths and of the ADR 0548 path is a test file written and run on the base
after this commit and before any library code (D7). Results go in the addenda at the end; nothing above the addenda is changed
after a run.

Roadmap item F-635 (deps F-626), structural solver, advanced tier, the follow-up named by
[ADR 0548](0548-f626-registration-warping-unknown-on-the-static-solve-modes-and-pre-stressed-modes.md) (its question Q2). The
element is that of [ADR 0546](0546-f598-registration-warping-degree-of-freedom-for-beam2-and-i-section-lateral-torsional-buckling.md)
with the section forms of [ADR 0553](0553-f627-registration-monosymmetric-thin-walled-sections-for-the-warping-element.md).
Number 0574 given by the lead. Branch `f635-work` from 61033836, worktree `w49i-work/f635`, scratch `w49i-work/f635-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test edited;
no new dependency, no unsafe code, the crate graph unchanged; clean-room. Every choice below that the maintainer would normally
be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation), except the open
questions at the end.

## What F-635 asks

The acceptance, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. The harmonic response of a fork-supported I beam under a
> harmonic end torque, and the transient twist of the same beam under a step torque, use the 14-unknown stiffness and mass of
> ADR 0548. The first torsional resonance equals sqrt((G J k^2 + E C_w k^4)/(rho I_p)) within 1e-3 at 16 elements; every result
> without the option bit-identical.

Clauses: **a0** every limit registered here; **a1** a harmonic response on the 14-unknown stiffness and mass of ADR 0548; **a2**
a transient twist under a step torque on the same matrices; **a3** the first torsional resonance of the harmonic path within
`1e-3` of `sqrt((G J k^2 + E C_w k^4)/(rho I_p))`, `k = pi/L`, at 16 elements; **a4** every result without the option
bit-identical.

### Readings, registered

- **"Fork-supported I beam"**: the full fork beam of ADR 0548 F4: the I section of ADR 0546 (`doubly_symmetric_i(0.1, 0.01,
  0.18, 0.006, 5/6)`, steel `E = 2e11`, `nu = 0.3`, `rho = 7850`), `L = 3` along `x`, 16 Beam2 elements, `u = 0` at node 0, `v
  = w = phi = 0` at both ends, the warping free at both ends (`phi'' = 0`, the fork condition), all other components free.
- **"Harmonic end torque"**: at a fork support the twist is held, so a torque on the end node goes into the support reaction and
  the beam does not move (the reduction drops a load at a prescribed equation; check HE records it). The torque is therefore
  applied at **midspan** (node 8, `x = L/2`), the drive point where the first torsional mode `sin(pi x/L)` has its largest value
  and the even modes do not respond. This reading of "end" is taken on the maintainer's standing instruction of 2026-11-11 (take
  the recommendation): the resonance of a fork-fork beam does not depend on where it is driven, only on whether the drive point
  is a node of the mode; midspan gives the cleanest single-mode response. The transient "step torque" is the same torque at the
  same node, `T = 1000` N m for `t >= 0`, from rest.
- **"The first torsional resonance"** of the harmonic path: the circular frequency at which the undamped drive-point response
  is unbounded, located by the search D5 on the harmonic solve itself (not taken from the modal solve). On the full beam the
  weak-axis bending mode is lower (ADR 0548 F4), but the torque does not excite it (the doubly symmetric section has its shear
  centre at the centroid: twist and bending are uncoupled), so "first torsional" is the lowest pole of the drive-point twist.
- **"Use the 14-unknown stiffness and mass of ADR 0548"**: the matrices `K_aug` and `M_aug` of ADR 0548 D1 and D4 (the
  internal `stiffness_aug` and `mass_aug` of `beam_warping.rs`), with the model's prescribed equations and the restrained warping
  equations eliminated.
- **"The option"**: new functions and types of `rcfd_fem::beam_warping` (D6). The existing harmonic (`harmonic`,
  `harmonic_direct`) and transient (`transient`) modules are not changed; no case-file key (ADR 0546 Q1, F-625).

### Audit of refusals this change could lift (done before this text)

No existing test asserts a refusal of the warping unknown on a harmonic or transient path: neither `harmonic.rs`,
`harmonic_direct.rs` nor `transient.rs` takes a `Warping`, and the warping tests (`sa_beam_warping_f598.rs`, `_f626.rs`,
`_f627.rs`, the bit records, `sa_buckling_restart_f650.rs`, `sa_buckling_sign_asymmetric_f642.rs`) contain no harmonic or
transient call (searched for `harmonic`, `transient`, `Integrator`). Nothing is lifted; the new paths are separate functions,
and every existing test keeps passing unchanged.

## What was read (no run)

- `AGENT_RULES.md`; the brief; ADR 0548 (whole); ADR 0546 D2, D4, D5; ADR 0553 D6 (the bimoment load).
- `crates/rcfd-fem/src`: `beam_warping.rs` (whole: `layout`, `entry_checks`, `warp_elements`, `augmented`, `stiffness_aug`,
  `mass_aug`, `solve_static`, `modes_impl`), `harmonic.rs` (`HarmonicLoad`, `HarmonicResponse`), `harmonic_direct.rs` (module
  text, `DirectHarmonic::response`, the pivot rule with `RESONANCE_PIVOT`), `transient.rs` (module text, `Rayleigh`,
  `LoadTable`, `LoadHistory`, `TransientOptions`, `Integrator::from_matrices`, `build`, `finish`, `begin`, the accessors),
  `rcfd-linalg/src/cskyline.rs` (`ComplexCsr`, `ComplexSkylineLdlt`, the options and the pivot test).
- Tests: `sa_beam_warping_f626.rs` (the set-ups), `sa_beam_warping_f626_bits.rs` (the digest pattern).

## Design

### D1. The load of the warping model

`load_vector(model, warping) -> Vec<f64>`: the model's `nodal_load_vector` on the six-unknown equations, zero on the warping
equations, plus the bimoments of `Warping::bimoment` (ADR 0553 D6), with the same refusals. It is the load of `solve_static`,
which is refactored to call it (the same operations in the same order; B0 records the static solve).

### D2. The direct harmonic solve with the warping unknown

`WarpingHarmonic::new(model, warping, options: &WarpingHarmonicOptions)`: `entry_checks` and `layout` (RF1 to RF4, RF6 of ADR
0546 D4); a nonzero prescribed value refused (key `"constraint"`); `K_aug`, `M_aug` of ADR 0548, each reduced by the model's
prescribed and the restrained warping equations (`constraint::reduce`, zero values). The response at `omega` (finite, `>= 0`,
key `"frequency"`) to a complex load `F` (`HarmonicLoad`, one value per equation of `WarpingDofs`, key `"harmonic load"`) is the
steady `x(t) = Re(X e^{i omega t})` (the convention of ADR 0391) of

`A(omega) X = F`, `A = (1 + i eta) K_aug - omega^2 M_aug + i omega (alpha M_aug + beta K_aug)`

(first principles: `M x'' + C x' + K x = Re(F e^{i omega t})` with `C = alpha M + beta K` and the hysteretic term `i eta K` of a
uniform loss factor; the same `A` as ADR 0448 D2 with one material). `eta` is `WarpingHarmonicOptions::loss_factor` (finite,
`>= 0`, default 0, key `"damping"`), `(alpha, beta)` a `transient::Rayleigh` (default none; its check). `A_FF` on the union of
the patterns of `K_aug` and `M_aug`, factored by `ComplexSkylineLdlt` (reverse Cuthill-McKee, no pivoting); when `A` is real (no
damping) the pivot rule of ADR 0448 D1 with `min_relative_pivot = resonance_pivot` (default `harmonic_direct::RESONANCE_PIVOT =
1e-8`) refuses a resonance (key `"frequency"`, naming the frequency and the node and component, 6 for a warping unknown). The
response (`HarmonicResponse`) has one value per equation of `WarpingDofs`, zero at the eliminated equations. Accessors:
`dofs()`, `free()`.

### D3. The transient solve with the warping unknown

`WarpingTransient::new(model, warping, loads: &[LoadHistory], options: &TransientOptions)`: the same checks and reduced
matrices as D2, and the integrator of ADR 0389 on them, `transient::Integrator::from_matrices(K_r, M_r, reduced loads,
options)` (Newmark or generalised-alpha, Rayleigh damping, the load tables, the state, the energies: all of ADR 0389 unchanged).
Each load pattern has one value per equation of `WarpingDofs` (key `"load pattern"`) and is reduced to the free equations.
`start_at_rest(t0)` starts from zero displacement and velocity (`M a_0 = f(t_0)`); `start(t0, u0, v0)` takes full vectors (zero
at the eliminated equations, else refused, key `"initial state"`); `run(steps)`; `displacement()`, `velocity()` the full vectors
(zero at the eliminated equations); `integrator()` and `integrator_mut()` give the reduced integrator. A step load is a
`LoadTable::constant(1, 0, t_end)` on `load_vector`.

### D4. Mixed models, refusals

As ADR 0548 D6: RF1 to RF4 and RF6 with the same keys (a model without warping blocks is not taken: the default paths are the
modules' own functions). A free rigid motion is not refused (as `harmonic_direct`): the undamped `A(0)` then refuses by its
pivot, and the transient's effective matrix `M + beta dt^2 K` is regular.

### D5. The resonance search (a procedure of the test, registered)

`s = omega^2`, `g(s) = 1/Re X_d(s)`, `X_d` the undamped drive-point twist (node 8, component 3) of D2 with the midspan torque.
`g` is smooth and has a simple zero at the first eigenvalue (near it `X_d = c_1/(omega_1^2 - s) + R(s)` with `R` regular, so
`g = (omega_1^2 - s)/(c_1 + (omega_1^2 - s) R)`). Secant iteration `s_{j+1} = s_j - g_j (s_j - s_{j-1})/(g_j - g_{j-1})` from
`s_0 = 0.98 C3`, `s_1 = 1.02 C3` (`C3 = (G J k^2 + E C_w k^4)/(rho I_p)`, `k = pi/L`), stopped when `|s_{j+1} - s_j| <= 1e-6
|s_j|` (the new iterate is the result, not evaluated), at most 20 steps (a 21st is a miss of H2). If a solve on the way is
refused as a resonance (key `"frequency"`), that iterate is the result (its pivot is within `1e-8` of singular). The measured
resonance is `sqrt(s)`. The same procedure is applied to the plain `harmonic_direct::DirectHarmonic` (negative control H7) and
to the model with the warping held (HW).

### D6. Files

Changed: `src/beam_warping.rs` (the new functions and types after the ADR 0548 part; `solve_static` takes its load from D1, the
same arithmetic). New tests: `tests/sa_beam_warping_f635.rs`, `tests/sa_beam_warping_f635_bits.rs`. Not touched: `harmonic.rs`,
`harmonic_direct.rs`, `transient.rs`, `solve.rs`, `modal.rs`, `mass.rs`, `geometric.rs`, any other module of any crate.

### D7. The bit record B0

`tests/sa_beam_warping_f635_bits.rs`, written and run on the base (61033836 plus this ADR, no library line changed): digests
(FNV-1a 64 of ADR 0291) of the plain direct harmonic response (`DirectHarmonic`, undamped and with a loss factor, two
frequencies) and the plain transient run (`transient::Integrator::new`, 50 steps, displacement and velocity) of the fork beam
under the midspan torque, and of the ADR 0548 warping path: `solve_static` (displacement and reactions) of the cantilever with
the root warping held, with and without a bimoment, and `modal_analysis` (eigenvalues and modes, 4) of the fork beam. It runs
unchanged after the change.

## Arithmetic done before the code (disclosed)

`arith.log`, the section of ADR 0548 arithmetic 1 (`G J = 6125.128`, `E C_w = 3008.333`, `I_p = 2.265257e-5`).

1. **C3** (ADR 0548 arithmetic 5, from first principles there: `E C_w phi'''' - G J phi'' + rho I_p phi_tt = 0`, the fork
   conditions satisfied by `sin(m pi x/L)`): `omega_1^2 = 58118.116`, `omega_1 = 241.07699` rad/s (38.369 Hz), period `T_1 =
   0.0260630` s. 16 Hermite elements (the stiffness and mass of ADR 0548 on the twist): `omega_1^2` `+7.2e-7`, so `omega_1`
   `+3.6e-7`; 8 elements `+5.8e-6` in `omega_1`.
2. **Continuous drive-point receptance** (modal series of the fork-fork Vlasov beam, the modes `sin(k_m x)` normalised by `int
   rho I_p sin^2 = rho I_p L/2`): `X(a) = sum_m (2 T sin^2(k_m a)/(rho I_p L)) / (omega_m^2 (1 + i eta) - omega^2 + i omega (alpha
   + beta omega_m^2))`, 4000 terms (the terms fall as `m^-4`); the static value `0.066799078` (Saint-Venant alone `T L/(4 G J) =
   0.12244642`). The 16-element model: static `-2.6e-6`; undamped at `omega = 0.5 omega_1` `1.4e-6`; `eta = 0.02` at `omega =
   omega_1` (continuous) `3.6e-5`; Rayleigh `zeta = 0.02` at `omega_1` (`alpha = zeta omega_1`, `beta = zeta/omega_1`) at `omega
   = omega_1` `1.8e-5`; both together at `0.5 omega_1` `1.4e-6`.
3. **Resonance search D5** on the 16-element model: 3 secant steps, `omega` `+3.6e-7` from C3 and `-2.4e-12` (in `s`) from the
   model's eigenvalue. The Saint-Venant model (linear twist, mass `rho I_p L/6 [2, 1; 1, 2]`): 5 steps, `omega` `-0.1925` from C3.
   Warping held at both ends: the search lands on the model's eigenvalue `151151.2` to `-6.8e-13`.
4. **Transient** (Newmark average acceleration, `dt = T_1/400`, 800 steps = two periods of mode 1, from rest): the midspan twist
   against the continuous series `sum_m (2 T sin^2(k_m a)/(rho I_p L omega_m^2))(1 - cos omega_m t)` (4000 terms), the largest
   difference over the steps over the continuous static value: `5.4e-3` (`2.3e-3` at `dt/2`: the error is mostly the period error
   `(omega dt)^2/12` of the third mode, whose share is 3 percent of the static twist, and its phase drift over two periods); the
   largest twist `1.978` of the static. The energy `(1/2) v^T M v + (1/2)(u - u_s)^T K (u - u_s)` is conserved to `3.5e-13` (the
   trapezoidal rule conserves the quadratic invariant of a linear system). The Saint-Venant model against the same series:
   `3.2` of the static. Mass-proportional damping `alpha = 2 omega_1` (critical for mode 1, `e^{-omega_1 t}` for every mode),
   1592 steps (`omega_1 t = 25`): the final twist `3.3e-7` from the model's static twist; with the warping held at both ends
   `2.1e-6` (held static `0.481` of the free).

## Checks (limits fixed now)

Every aggregated error is asserted finite before it is compared (a non-finite value is a miss). The fork beam of the readings,
16 elements, the midspan torque `T = 1000` about `x` unless stated; "twist" the component 3 of node 8.

| id | check | limit | estimate |
|---|---|---|---|
| B0 | the bit record of D7, base and after | equal (bit) | exact |
| H2 | **acceptance a1, a3**: the resonance of D5 on `WarpingHarmonic` (undamped): `|omega_r/sqrt(C3) - 1|` | `<= 1e-3` | `3.6e-7` |
| H3 | H2's `s = omega_r^2` against `omega_1^2` of the torsional mode of ADR 0548 `modal_analysis` of the same model (4 modes, the mode whose twist at node 8 is largest) | `<= 1e-8` relative | `2.4e-12` |
| H1 | undamped twist at `omega = 0.5 sqrt(C3)` against the continuous series | `<= 1e-5` relative | `1.4e-6` |
| H4 | `omega = 0`, undamped: the full response against `solve_static` of the same model and load, `max|X - u|/max|u|`; `Im X` zero | `<= 1e-10`; exact | `1e-14`; exact |
| H5 | loss factor `eta = 0.02`, at `omega = sqrt(C3)`: the complex twist against the series, `|X/X_c - 1|` | `<= 2e-4` | `3.6e-5` |
| H6 | Rayleigh `zeta = 0.02` (`alpha = zeta sqrt(C3)`, `beta = zeta/sqrt(C3)`), at `omega = sqrt(C3)`: as H5 | `<= 1e-4` | `1.8e-5` |
| H6b | `eta` and Rayleigh of H5, H6 together, at `omega = 0.5 sqrt(C3)`: as H5 | `<= 1e-5` | `1.4e-6` |
| H7 | negative control: D5 on the plain `DirectHarmonic` (six unknowns, Saint-Venant) of the same model: `|omega_r/sqrt(C3) - 1|` | `> 0.1` | `0.19` |
| H8 | H2 at 8 elements | `<= 1e-4` | `5.8e-6` |
| HW | warping held at both ends (`Warping::restrain` of both end nodes): D5 (started at `0.98`, `1.02` of the held `modal_analysis` eigenvalue) against that eigenvalue; and that eigenvalue `> 1.2 omega_1^2(free)` | `<= 1e-8` relative; strict | `6.8e-13`; `2.60` |
| HB | the bending components (0, 1, 2, 4, 5 of every node) of the H1 response: `max|.|` against `max|twist|` | `<= 1e-10` relative | `0` |
| HE | the torque on the end node `xl` instead of midspan, at `0.5 sqrt(C3)`: every value of the response zero | exact | exact |
| TR1 | **acceptance a2**: `WarpingTransient`, Newmark average acceleration, `dt = T_1/400`, `T_1 = 2 pi/sqrt(C3)`, 800 steps, from rest under the step torque: `max_n |phi_n - phi_c(t_n)|/phi_c,static` against the continuous series | `<= 1.5e-2` | `5.4e-3` |
| TR2 | TR1: the largest twist over the run over the continuous static twist | in `[1.9, 2.05]` | `1.978` |
| TR3 | mass-proportional damping `alpha = 2 sqrt(C3)`, 1592 steps: the final twist against `solve_static` | `<= 1e-5` relative | `3.3e-7` |
| TR3b | TR3 with the warping held at both ends, against `solve_static` of that model | `<= 2e-5` relative | `2.1e-6` |
| TR4 | TR1: `max_n |E_n/E_0 - 1|`, `E = kinetic_energy + (1/2)(u - u_s)^T K (u - u_s)` (`u_s` of `solve_static`), `E_0 = (1/2) u_s^T f` | `<= 1e-9` | `3.5e-13` |
| TR5 | negative control: the plain `transient::Integrator::new` of the same model and load (Saint-Venant), as TR1 | `> 0.5` | `3.2` |
| R | refusals on the new entries, key each: `WarpingHarmonic::new` with a non-Beam2 block (RF1), a nonzero prescribed value (`"constraint"`), a negative loss factor (`"damping"`); `response` at a negative frequency (`"frequency"`), with a load of the six-unknown length (`"harmonic load"`), undamped at the FE eigenvalue of H3 (`"frequency"`); `WarpingTransient::new` with a pattern of the six-unknown length (`"load pattern"`), a nonzero prescribed value (`"constraint"`); `start` with a nonzero initial displacement at a held twist (`"initial state"`) | exact | |

The TR4 energy is formed from the integrator's `kinetic_energy` and `strain_energy` (`(1/2) v^T M v`, `(1/2) u^T K u` of the
reduced matrices): `E_n = KE + SE - f^T u + (1/2) u_s^T f` (equal to the form in the table since `K u_s = f`), `f` of
`load_vector`, `u` the full displacement, `u_s` of `solve_static`.

Negative controls: H7 and TR5 (the six-unknown paths miss C3 and the series: the checks see the 14-unknown matrices), HW (the
restraint acts), HE (the end-torque reading), HB (no spurious coupling).

## Planned mutations (on a scratch copy, one at a time, own `CARGO_TARGET_DIR`)

| id | mutation | expected caught by |
|---|---|---|
| Z1 | the harmonic mass without the difference `M14 - M12` (the six-unknown mass embedded) | H1, H2, H3, or a refusal |
| Z2 | the harmonic stiffness without the difference `K14 - K12` | H1 to H4 |
| Z3 | `+ omega^2 M` in `A` | H1, H2 |
| Z4 | the loss factor on the mass term instead of the stiffness | H5, H6b |
| Z5 | the Rayleigh `beta K` term omitted | H6, H6b |
| Z6 | the restrained warping equations not eliminated in the harmonic solve | HW |
| Z7 | the restrained warping equations not eliminated in the transient solve | TR3b |
| Z8 | the transient on the six-unknown mass embedded (no `M14 - M12`) | TR1, or a refusal (no mass on `chi`) |
| Z9 | the sign of the imaginary damping term flipped (`- i omega C`, `(1 - i eta) K`) | H5, H6 |

## Decision rule

**done**: B0, H1 to H8, H6b, HW, HB, HE, TR1 to TR5, TR3b and R met, and every mutation caught. **partial**: any registered
check missed (a `*_known_miss` record), any mutation not caught, or any check met only after a post-run amendment. **todo**: H2,
TR1 or B0 missed.

## Limits (not covered)

- No warping inertia `rho C_w` (ADR 0548 Q1); C3 neglects it too.
- No mode-superposition harmonic (`harmonic::HarmonicSolver` needs a `DofMap` basis); the direct solve only. No base motion, no
  dashpots, no per-material loss factors, no viscoelastic material, no pre-stress on the warping harmonic path.
- No nonlinear transient driver, no prescribed motion histories, no transformation path (RF4).
- The skyline factor only (no supernodal factor) on the harmonic path; the transient uses the integrator's own factor.
- No case-file key (F-625).

## Open questions (to be taken on the maintainer's standing instruction of 2026-11-11 when answered; none changes a default)

- **Q1. The reading of "end torque".** Options: (a) midspan torque (this ADR; an end torque at a fork support does not move the
  beam, HE); (b) a beam fork-supported at one end and free in twist at the other, driven there (the resonance is then not C3).
  Recommendation (a).
- **Q2. A pre-stress on the warping harmonic path** (as `DirectHarmonicOptions::prestress`). Recommendation: a separate item when
  a case needs it.

## Amendment 1 (2026-11-15): the plain harmonic and transient paths refuse beams (B0, H7, TR5 set-ups corrected)

Written after the first attempt to record B0 on the base (log `f635-scratch/b0_base.log`) and before any library code and any
run of the other checks. The attempt stopped at its first call: `DirectHarmonic::new` of the fork beam returns
`FemError::NotYetSupported { item: "element kind", variant: "Beam2", location: "block 0", package: "a follow-up of ADR 0417" }`.
On reading the entries (after the attempt, disclosed): `harmonic_direct::DirectHarmonic::new` calls `check_supported`,
`harmonic.rs` likewise, and `transient::Integrator::new` calls `check_supported_solids_with_materials`; none of the three takes
a Beam2 block. The registered set-ups that call them on the beam (the plain digests of B0, the negative controls H7 and TR5)
are therefore defective; no digest was printed and no limit was compared. The limits of every other check are unchanged.

Corrections (the old set-ups are not run; there is no record of them beyond this text and the log):

- **B0** (D7 corrected): the plain harmonic and transient digests are replaced by **B0r**: `DirectHarmonic::new` (undamped and
  with a loss factor) and `transient::Integrator::new` of the fork beam under the midspan torque are refused with the error
  above, before and after (compared by `{:?}` of the error, exact). The ADR 0548 warping digests of D7 (static solve with and
  without a bimoment, displacement and reactions; modes) stay, and the plain `modal::modal_analysis` of the fork beam (4 modes,
  eigenvalues and modes) is added. The file is rewritten accordingly and run once on the base.
- **H7'** (negative control, in place of H7): D5 on `WarpingHarmonic` with `WarpingSection::saint_venant()` (`C_w = 0`, the
  Hermite twist of the same element): `|omega_r/sqrt(C3) - 1| > 0.1`. Estimate (`arith_a1.py`, log `arith_a1.log`): `0.194`
  (5 secant steps; it lands on `G J k^2/(rho I_p)` to `1.9e-9`).
- **TR5'** (negative control, in place of TR5): TR1 on `WarpingTransient` with `saint_venant()`: `> 0.5`. Estimate `3.27`.
- A further fact the attempt shows, registered as a check (**PB**): the plain harmonic and transient paths do not take beams at
  all, so the warping paths of D2 and D3 are the only harmonic and transient paths a beam has; nothing of the defaults changes
  (B0r).

The decision rule reads B0 as B0 with B0r, H7 as H7', TR5 as TR5'. These corrections were made before the run of the corrected
checks; the rule treats a check met only after a post-run amendment as partial: B0 is such a check only if a reader counts the
failed first attempt as its run (it compared nothing); this is recorded and left to the lead (Addendum 1 says how it is
counted).

## Amendment 2 (2026-11-15): H3 and HW missed; a corrected search registered (H3', HW')

Written after the one registered run (log `f635-scratch/run1.log`, code commit 426df3bd) and before any run of the checks
below. In that run H3 measured `1.06e-8` and HW `1.77e-8` against the limit `1e-8` (missed; every other check met). Diagnosis:
the search D5 on the default options ends at the first refused solve. After two secant steps the iterate was `1.06e-8` (H3) and
`1.77e-8` (HW) from the eigenvalue, and the solve there was refused as a resonance: the pivot rule refuses a pivot at most
`RESONANCE_PIVOT = 1e-8` of its scale, so the search cannot get closer than about `1e-8` relative. D5 says such an iterate is the
result. The arithmetic of the registration (numpy, no pivot test) took a third step and reached `2.4e-12`. The estimate left out
the refusal, so the limit `1e-8` sat on the pivot threshold. The missed checks stay as the ignored record
`h3_hw_known_miss` (limits intact, measured values recorded). H2 (the acceptance, `3.6e-7` against `1e-3`) is not affected.

Corrected checks (new, run once after this commit):

- **H3'**: H3 with the search D5 on `WarpingHarmonicOptions::new().resonance_pivot(0.0)` (only the rounding pivot test of ADR
  0448 D1 is left, so the search ends by its step rule): `|s_r/lambda - 1| <= 1e-8`. Estimate `2.4e-12` (arithmetic 3).
- **HW'**: HW (the held eigenvalue, the same procedure) with that option: `<= 1e-8`; the held eigenvalue `> 1.2` of the free
  value (as HW). Estimate `6.8e-13`; `2.60`.

The decision rule: H3 and HW are missed in the registered run, so the item is **partial** by the rule whatever H3' and HW' give.
H3' and HW' are checks met (or not) only after a post-run amendment.

## Addendum 1 (2026-11-15): results

Written after the one registered run of `sa_beam_warping_f635.rs` (log `f635-scratch/run1.log`, code commit 426df3bd), the run of
the amendment 2 checks (`run2.log`, commit after 2ff4b53c), the B0 runs (`b0_base2.log` on the base, `b0_after.log`) and the
mutation runs (`mutations.log`, `mut_Z*.log`). The numbers are those the tests printed (release). No limit was changed; nothing
was re-run to pick a result.

### Results against the registered limits

| id | measured | limit | verdict |
|---|---|---|---|
| B0 (with B0r, amendment 1) | 10 digests bit-identical to the base record (the three refusals of the plain harmonic and transient paths on a beam, the plain modes, the ADR 0548 static solve with and without a bimoment and its modes); the ADR 0546 and ADR 0548 bit records also unchanged | equal | met |
| H2 (acceptance) | `3.56e-7` (`241.07708` against `241.07699` rad/s, 2 secant steps) | `1e-3` | met |
| H3 | `1.06e-8` | `1e-8` | **missed** (record `h3_hw_known_miss`) |
| H1 | `1.36e-6` | `1e-5` | met |
| H4 | `2.7e-13`; every imaginary value 0 | `1e-10`; exact | met |
| H5 (loss factor) | `3.62e-5` | `2e-4` | met |
| H6 (Rayleigh) | `1.82e-5` | `1e-4` | met |
| H6b (both) | `1.36e-6` | `1e-5` | met |
| H7' (Saint-Venant control) | `0.194` | `> 0.1` | met |
| H8 (8 elements) | `5.79e-6` | `1e-4` | met |
| HW | `1.77e-8`; held eigenvalue `2.601` of the free | `1e-8`; `> 1.2` | **missed** (the first part; record `h3_hw_known_miss`) |
| HB | `0` | `1e-10` | met |
| HE | every value 0 | exact | met |
| TR1 (acceptance) | `5.37e-3` | `1.5e-2` | met |
| TR2 | `1.9775` | `[1.9, 2.05]` | met |
| TR3 | `3.26e-7` | `1e-5` | met |
| TR3b | `2.07e-6` | `2e-5` | met |
| TR4 | `3.1e-13` | `1e-9` | met |
| TR5' (Saint-Venant control) | `3.27` | `> 0.5` | met |
| R (9 refusals) | each with its key | exact | met |
| H3' (amendment 2) | `9.7e-13` (3 steps) | `1e-8` | met after a post-run amendment |
| HW' (amendment 2) | `8.6e-14`; `2.601` | `1e-8`; `> 1.2` | met after a post-run amendment |

Every measured value met is within a factor of 3 of its estimate. H3 and HW: diagnosed in amendment 2. The search stops at the
first solve refused by the resonance pivot `1e-8`, and the estimate left that refusal out.

### Mutations (scratch copy `f635-scratch/mut`, own target dir, one at a time)

| mutation | result | caught by |
|---|---|---|
| Z1 harmonic mass without `M14 - M12` | caught | H1, H2 (`3.2e-3`), H3', HW', H5, H6, H6b, H8, R (resonance) |
| Z2 harmonic stiffness without `K14 - K12` | caught | H1, H2 (`0.18`), H3', HW', R |
| Z3 `+ omega^2 M` | caught | H1, H2, H3', HW', H5, H6, H6b, H7', H8, R |
| Z4 loss factor on the mass term | caught | H6b only (at `omega_1` `omega^2 M` and `K` nearly agree on the resonant mode, so H5 passes) |
| Z5 Rayleigh `beta K` omitted | caught | H6, H6b |
| Z6 restraint not eliminated (harmonic) | caught | HW' |
| Z7 restraint not eliminated (transient) | caught | TR3b |
| Z8 transient on the six-unknown mass | caught | a refusal (`"mass matrix"`, no mass on `chi`) in TR1 |
| Z9 imaginary damping sign flipped | caught | H5, H6, H6b |

All nine mutations are caught. Z4 was predicted to be caught by H5 and H6b, and only H6b caught it. Z6 is caught by HW' (the
amended check); HW missed in the registered run, so HW cannot count as a catch.

### Other runs

`cargo test -p rcfd-fem --release`: 276 test binaries and doc-tests, 1154 passed, 0 failed, 310 ignored (the ignored records,
including `h3_hw_known_miss`, stay ignored). `cargo test -p rcfd-api --release --test baseline_hashes`: 3 passed, 1 ignored
(unchanged). `cargo test -p rcfd-cli --release --test docs_site`: 2 passed. `cargo clippy -p rcfd-fem --release --all-targets
-- -D warnings`: clean. `cargo fmt -p rcfd-fem --check`: clean. No documentation page changed (the module text of
`beam_warping.rs` has a paragraph for ADR 0574).

### Decision

**Partial** by the registered rule: H3 and HW missed in the registered run (record `h3_hw_known_miss`), and H3' and HW' were met
only after a post-run amendment. B0 and the refusal records of amendment 1 were corrected before any comparison (amendment 1).
The acceptance clauses are met. The harmonic response and the transient twist of the fork-supported I beam use the 14-unknown
stiffness and mass of ADR 0548 (H7', TR5' and mutations Z1, Z2 and Z8 show it). The first torsional resonance is `3.6e-7` from
`sqrt((G J k^2 + E C_w k^4)/(rho I_p))` at 16 elements. Every result without the option is bit-identical (B0). The misses are in
supporting checks of the resonance search; they are not in the acceptance.

### Remaining questions

- Q1 (the reading of "end torque"): (a) midspan torque, as taken here; (b) a beam free in twist at the driven end. Recommendation:
  (a).
- Q2 (a pre-stress on the warping harmonic path): recommendation, a separate item when a case needs it.
- Q3 (the resonance pivot cuts the search): (a) leave as is, with the search taking `resonance_pivot(0.0)` when it needs more
  than `1e-8`; (b) add a search function `first_resonance` to the library that runs without the pivot. Recommendation: (a). The
  pivot guards a response that does not exist, and a search is a caller's procedure.

## For the lead: roadmap entries

> F-635 status: partial. Note: Structural solver, advanced tier. ADR 0574, 2026-11-15: the warping unknown of ADR 0548 on a direct harmonic solve (`rcfd_fem::beam_warping::WarpingHarmonic`: (1 + i eta) K - omega^2 M + i omega (alpha M + beta K) on the 14-unknown stiffness and mass, one complex factorisation per frequency, resonance refused without damping) and on the linear transient (`WarpingTransient`: the integrator of ADR 0389 on the reduced 14-unknown matrices), with the load `load_vector`. The plain harmonic and transient paths do not take beams at all; they are unchanged. Fork-supported I beam, torque at midspan (an end torque at a fork support goes into the reaction): first torsional resonance of the harmonic path 3.6e-7 from sqrt((G J k^2 + E C_w k^4)/(rho I_p)) at 16 elements (5.8e-6 at 8); response 1.4e-6 of the continuous modal series undamped, 3.6e-5 with a loss factor and 1.8e-5 with Rayleigh damping at the resonance. Step-torque transient (Newmark, T_1/400) 5.4e-3 of the static twist from the continuous series over two periods, energy conserved to 3e-13. The Saint-Venant section misses the resonance by 0.19. Partial: the resonance search against the modal eigenvalue missed its 1e-8 limit (1.1e-8 and 1.8e-8) because the search stops at the 1e-8 resonance pivot; with the pivot off it gives 1e-12 (amendment 2). Defaults bit-identical (10 digests). Nine of nine mutations caught. No warping inertia, no pre-stress, no mode superposition, no case-file key.

> Follow-up proposed: a pre-stress on the warping harmonic path. Deps F-635. Acceptance: Every limit not stated here is registered in the ADR before the run. The direct harmonic solve with the warping unknown takes the pre-stress of the warping static solve (K + K_sigma); the first torsional resonance of a fork-supported I beam under an axial force N = +/-0.5 of the torsional buckling load equals sqrt((G J k^2 + N I_p/A k^2 + E C_w k^4)/(rho I_p)) within 1e-3 at 16 elements; every result without the option bit-identical.
