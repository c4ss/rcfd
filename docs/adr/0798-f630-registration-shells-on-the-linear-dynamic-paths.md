# ADR 0798: F-630 registration: Shell4 and Shell3 on the linear dynamic paths, by an opt-in per path

Status: **REGISTERED 2026-11-25, written before any code of F-630, before any test of it and before any run of its checks.**
Before this text no file of the repository was changed. Done beforehand: the reading below, searches of the tree, one
estimate (a script that does not use rcfd; "Arithmetic"), and a build (no run) of the test `docs_site`, which is run on this
text before its commit for its links, as the brief asks. Results go in addenda at the end; nothing above them is changed.

It answers the brief `docs/adr/briefs/f630-shells-linear-dynamics.md` ([the file](briefs/f630-shells-linear-dynamics.md)) and
follows [docs/process/agent-rules.md](../process/agent-rules.md), version 5. Branch `f630-shells-linear-dynamics` from main
2d125d55, scratch `w49i-work/f630-shells-linear-dynamics-scratch`. Model: Claude Opus 5.5.

## What is asked

The acceptance of F-630 in `docs/roadmap.json`: "Every limit not stated here is registered in the ADR before the run. The
linear transient, the damped modes, the modal and the direct harmonic solve take Shell4 and Shell3: a cantilever strip of each
against the closed-form Euler-Bernoulli response (the free-vibration frequency, the harmonic tip response off resonance)
within registered limits, the direct harmonic against the mode superposition with all modes within 1e-8; models without
shells keep their bits."

Clauses: **a0** every limit registered; **a1** the linear transient, **a2** the damped modes, **a3** the modal solve and the mode
superposition, **a4** the direct harmonic take Shell4 and Shell3; **a5** the free-vibration frequency against Euler-Bernoulli;
**a6** the harmonic tip response off resonance against Euler-Bernoulli; **a7** direct harmonic against the superposition with all
modes; **a8** models without shells keep their bits. "The modal and the direct harmonic solve" is read as the harmonic by modes
and the direct one, and the modal analysis itself is checked too. The brief adds **b1** the transformation path where a path
has one, **b2** transient against modal solution, **b3** damping against a closed form, **b4** mixed models, layups, warped Shell4 and
the drilling mode taken or refused by name, **b5** the refusals that stay, **b6** shells without the option keep their bits
(Windows and Linux x86_64). Decided by the lead and not reopened here: an opt-in per path; the default refuses with the same
error; a path that takes shells already gets its check and no option.

## What was read

In full: `AGENTS.md`, `docs/process/agent-rules.md` (version 5), the brief,
[ADR 0545](0545-f541-registration-shell3-on-the-remaining-paths.md),
[ADR 0556](0556-f628-registration-shells-in-the-cyclic-constraint.md), the
[tolerance table](../process/tolerances.md); of [ADR 0744](0744-f672-registration-beam2-in-the-cyclic-constraint-by-default.md) the
design; the items F-630, F-541, F-628, F-629 (by a script); lines 2125 and 2126 of `docs/reference-data.d/0000-base.part`. Of
`crates/rcfd-fem/src`: `transient.rs`, `harmonic.rs`, `modal.rs`, `mass.rs`, `assemble.rs` in full; of `harmonic_direct.rs`,
`damped.rs`, `model.rs`, `mpc.rs`, `cyclic.rs`, `shell.rs`, `shell3.rs`, `recover.rs`, `geometric.rs`, `random.rs`, `hex8x.rs`, `mat_ortho.rs`
the functions on these paths; of the tests `sa_shell3_paths.rs`, `sa_cyclic_shells.rs` and the bit records in part. Reached by
search: every caller of `check_supported` and of the options.

## What is refused now (at 2d125d55)

| Path | Entry check now | Shells now |
|---|---|---|
| linear transient, plain (`Integrator::new`, `eliminated`) | `check_supported_solids_with_materials` | refused, `NotYetSupported` naming `shell::FOLLOW_UP` / `shell3::FOLLOW_UP` |
| linear transient on the transformation and with motion histories (`on_transformation`) | the same | refused |
| damped modes (`damped_modes`; plain only, the transformation is refused for every model) | `check_supported` | refused |
| modal analysis, plain and on the transformation (`modal_analysis`, `modal_analysis_mpc`) | `check_supported_with_materials` | **taken** (ADR 0432, 0435, 0454) |
| mode superposition: `ModalBasis::from_result`, `HarmonicSolver` with `static_correction(false)`, `participation` | none | **taken** (no model check on the path) |
| mode superposition: `ModalBasis::all_modes` (plain only) | `check_supported` | refused |
| mode superposition: the static correction (the default), plain, free body (inertia relief), transformation | `check_supported` | refused |
| direct harmonic, plain and on the transformation (`DirectHarmonic::new`) | `check_supported` (viscoelastic: `check_supported_nonlinear`) | refused |

Behind these checks the assembled stiffness, mass and load vector already take both shells. One place does not: with more than
one material the direct harmonic, and with loss factors the damped modes, assemble `K_m` by `hex8x::element_stiffness_for`, which
for a shell reaches the refusing placeholder element.

## Design

**D1 (the opt-in).** A field `shells: bool`, default `false`, with a builder `shells(on)`, on `TransientOptions`, `DampedOptions`,
`HarmonicOptions` and `DirectHarmonicOptions`; and `ModalBasis::all_modes_with_shells(model, threads)` beside `all_modes`, which
has no options. With the option the entry check is one of two new functions of `model.rs`,

- `check_supported_shells` (`check_elements` taking shells, not beams, then `shell::check_shell_model`, then the supports): damped
  modes, `all_modes_with_shells`, static correction, direct harmonic;
- `check_supported_shells_with_materials` (the same, and the per-point materials on solid elements with `mat_ortho::check_model`,
  as the transient takes them now): transient.

Without the option the call is the one above, in the same place, so nothing that runs for a model changes (a8, b6). With it, a
model without shells passes the same element checks and runs the same lines.

**D2 (the stiffness of one material).** `assemble.rs` gains `element_stiffness_of(model, ctx)`: Beam2 (not reached), Shell4 and Shell3 by
their `model_element_stiffness`, any other kind by the line of today (`linear_elasticity_matrix`, `element_stiffness_for`). Used by the
per-material assembly of `harmonic_direct.rs` and of `damped.rs` (loss factors). Not by the viscoelastic assemblies.

**D3 (what each path does with it).** Transient: both branches; on the transformation the check of `mpc::check_transformation_model`
is called as well, which refuses a cyclic constraint with shells unless the constraint has its own `shells` option (F-628); with
it the rotation rows are those of ADR 0556. Damped modes: the plain path; a model with shells and a viscoelastic material stays refused
(the storage model is checked by `check_supported` as now). Direct harmonic and static correction: as now with the new check; a
cyclic constraint stays refused there. A pre-stress of shells (ADR 0473) is taken by the static correction and the direct solve: the
geometric stiffness dispatches shells already, and it is checked (P1, P2).

**D4 (layups).** Shell4 layups are taken (stiffness and mass of ADR 0496); a Shell3 layup is refused by `check_shell_model` naming
`laminate::FOLLOW_UP`. New: a `LossFactor::Table` on a set that holds an element of a Layup block is refused (key `loss factor`), because
`s = E'/E` refers to the material of the set and not to those of the plies; the damped modes refuse tables for every model. A
constant loss factor scales the stiffness of the set's elements and is taken.

**D5 (refused by name).** With the option on: Beam2 (also beside a shell), `beam::FOLLOW_UP`; a shell node shared with a solid, key `node`
naming it; a warp above `WARP_LIMIT`, key `warp`; a viscoelastic material with shells; a per-point material on a shell element; the
stress at integration points of a response of a shell (`HarmonicResponse::stress`, `DirectHarmonic::stress`: the placeholder
refuses; `shell::stresses` is the function for shells). `RandomSolver` takes `HarmonicOptions` and is not this item: with `shells` set it
refuses a model with shells, `NotYetSupported` naming "a follow-up of ADR 0798" (without the option it is as now).
Unchanged: `DirectHarmonic::about_state`, the rotating paths, the energy-momentum step, `solve_in_time`, the damped modes on the
transformation, `all_modes` on the transformation, the cyclic constraint without its option.

**D6 (drilling).** Both elements give the rotation about the normal a mass (`rho t^3/12`, as the bending rotations) and a stiffness (the
penalty, or the membrane of Allman or ANDES). It is an unknown like the others: taken, with the modes it adds, and the rotational
inertia about the normal it adds is `rho t^3 A/12` (checked in MP).

Files: `crates/rcfd-fem/src/{transient,damped,harmonic,harmonic_direct,model,assemble,mpc,random}.rs`; new
`crates/rcfd-fem/tests/sa_shells_linear_dynamics_f630.rs` and `sa_shells_linear_dynamics_bits_f630.rs`, each with a
`.mutation.patch`, and the records `sa_shells_linear_dynamics_bits_f630.tsv` and `.linux.tsv`. No other file of another crate.

## Closed forms (derived here)

**C1, free vibration.** `E I W'''' = rho A omega^2 W`, `W(0) = W'(0) = W''(L) = W'''(L) = 0` gives `1 + cos s cosh s = 0`, `s = beta L`, `beta^4 =
rho A omega^2 / (E I)`; `s_1 = 1.875104068711961`, `omega_1 = s_1^2 sqrt(E I/(rho A))/L^2` (`0000-base.part`, line 2126, derived in ADR 0522). Shape
`W = cosh bx - cos bx - c (sinh bx - sin bx)`, `c = (cosh s + cos s)/(sinh s + sin s)` (from `W''(L) = 0`).

**C2, tip force `P e^{i omega t}`.** With `E I* W'''' = rho A* omega^2 W`, `W = a (cos - cosh) + b (sin - sinh)` (the two clamped conditions);
`W''(L) = 0`: `a (cos s + cosh s) = -b (sin s + sinh s)`; `E I* W'''(L) = -P`. Eliminating, `W(L) = P L^3/(E I* s^3) (sin s cosh s - cos s sinh s) /
(1 + cos s cosh s)`; its limit `s -> 0` is `P L^3/(3 E I)`. Damping makes `s` complex: loss factor, `E I* = E I (1 + i eta)`, `A* = A`; Rayleigh, `E I* =
E I (1 + i omega beta_R)`, `rho A* omega^2 = rho A (omega^2 - i alpha_R omega)`; `s^4 = rho A* omega^2 L^4/(E I*)` (any root: the expression is even in the
fourth roots).

**C3, uniform load `q e^{i omega t}` per length.** Particular solution `-w_0`, `w_0 = q/(E I beta^4)`; `W = a cos + b sin + (w_0 - a) cosh - b sinh - w_0`
meets the clamped end; the free end gives, with `p = cos s + cosh s`, `r = sin s + sinh s`, `m = sin s - sinh s`, `a p + b r = w_0 cosh s` and `a m - b p = -w_0 sinh
s`, so `a = w_0 (p cosh s - r sinh s)/(p^2 + r m)`, `b = w_0 (p sinh s + m cosh s)/(p^2 + r m)`. Limit `s -> 0`: `q L^4/(8 E I)`.

**C4, one mode under the average-acceleration rule.** The rule is `x_1 = x_0 + (h/2)(x'_0 + x'_1)` on `x = (q, v)`: a root `lambda` of
`lambda^2 + (alpha + beta omega^2) lambda + omega^2 = 0` is multiplied per step by `mu = (1 + lambda h/2)/(1 - lambda h/2)`. From `q = a`, `v = 0`: `q_n = 2 Re(C mu^n)`, `C =
-a conj(lambda)/(lambda - conj(lambda))` (underdamped; two real roots likewise). A constant load of modal value `f` from rest gives `q_n = (f/omega^2)(1 -
q_n|_{a=1})`. Undamped: `mu = e^{i theta}`, `tan(theta/2) = omega h/2`, the period factor of `0000-base.part`, line 2125.

**C5, damped roots.** Rayleigh: `lambda_r = -c + i sqrt(omega_r^2 - c^2)`, `c = (alpha + beta omega_r^2)/2`. One loss factor on every element: `lambda_r^2 =
-omega_r^2 (1 + i eta)`. Both exact for the discrete model with `omega_r` its own frequencies.

**C6, rigid inertia of a flat `a x b x t` plate of either element, axes in and normal to it.** `I_x = m (b^2 + t^2)/12`, `I_y = m (a^2 + t^2)/12`, `I_z = m (a^2 +
b^2)/12 + m t^2/12`; the last term is D6's. Rigid motions are in the element spaces, so it is exact.

## The models

**Strip S(n)** (slender, for Euler-Bernoulli): `L = 1`, `b = 0.125`, `t = 0.01` m, `E = 2e11`, `nu = 0`, `rho = 7850`, `n x n/8` square cells, `n = 16, 32, 64`;
Shell3 is each cell `(a, b, c, d)` cut into `(a, b, c)`, `(a, c, d)`. Root edge clamped in six components. Tip force: shared over the tip edge in
the ratio 1/2, 1, ..., 1, 1/2. With `nu = 0` the strip is a beam of rectangular section.

**Block B** (stubby, for the two-route rows): `1 x 0.2`, `t = 0.1`, `nu = 0.3`, `6 x 1` cells, root clamped (72 free unknowns), the load `f_j = sin(1.3 j + 0.2)` N
or N m on every free unknown (the drilling ones too). Variants: **V1** Shell4; **V2** Shell3; **V3** Shell4 with the ANDES membrane; **V4** Shell3 with the ANDES
membrane; **V5** Shell4 warped, `z = 0.08 (x/L)(2 y/b - 1)`, `t = 0.1` (warp ratio about 0.037); **V6** Shell4 layup of two isotropic plies (steel 0.06 m under `E = 7e10, nu = 0.33, rho =
2700` 0.04 m, so the mass and the stiffness couple); **V7** cells 0 to 2 Shell4, 3 to 5 Shell3 cut.

**Mixed X**: a Hex8 block `0.2 x 0.2 x 0.2`, 1 x 1 x 1, its far face clamped, joined to the root edge of B (V1, V2; root not clamped) by one
rigid set of the block's near face and the shell's edge; two materials.

**Frustum**: the sector of ADR 0556 D3 (`N = 6`, 2 x 3 cells, root clamped, `CyclicOptions::shells`).

## Arithmetic (before the run, no rcfd)

`estimate.py` of the scratch folder (numpy): the two-node Timoshenko beam with linear `w` and `theta`, one-point shear (factor 5/6), consistent mass
`rho A`, `rho I`. For `nu = 0` a Shell4 strip has this beam as its solutions constant across the width. Shell3: the same beam with the shear divided by
`1 + 0.1 l^2/t^2`, `l = sqrt(2) h` (its stabilisation); an approximation, the triangle couples more. Relative to Euler-Bernoulli (modelling error: shear and
rotary inertia of `t/L = 0.01`, about -6e-5 of `f_1`; mesh: second order):

| | f_1 at n = 16, 32, 64 | tip force, largest of 0.5 and 2.5 omega_1, loss factor 0 and 0.05 | uniform load, the same |
|---|---|---|---|
| Shell4 | 4.1e-4, 5.3e-5, -3.6e-5 | 5.0e-3, 1.1e-3, 7.9e-5 | 3.3e-4, 8.5e-5, 1.0e-4 |
| Shell3 (approximate) | 4.6e-5, -3.7e-5, -5.9e-5 | 3.3e-3, 6.5e-4, 7.0e-5 | 6.5e-4, 2.6e-4, 1.6e-4 |

Ratio of the successive differences of `f_1`: 3.98 (Shell4), 3.93 (Shell3), order 2.0. The errors change sign with the mesh, so each
mesh has its limit and the order is taken from the differences. Time step `T_1/200`: period factor `1 - 8.2e-5`, damping factor `1 - (omega h)^2/6 = 1 -
1.6e-4`; a Rayleigh ratio fixed at `omega_1` and `6.27 omega_1` changes by `-0.72` times the relative change of the frequency.

Two-route rows: the dense eigensolution has the absolute error `eps lambda_max`; for B `lambda_max/lambda_1` is about 7e4 (membrane `(E/rho)(12/h^2 + 12/b^2)` against `12.36
(E/rho) t^2/(12 L^4)`), so `1.6e-11` of `lambda_1`; over two periods of 40 steps the phase error is about `1e-9` with a constant of ten. For the strip S it
is 5e8 (a relative error of `1e-7`), so the superposition with all modes is not compared on S.

## Checks (limits fixed now)

Every row on each path runs with the option on unless it says otherwise. A NaN misses. "rel" is the largest absolute difference over the unknowns of
one kind (translations; rotations) divided by the largest value of that kind, and the larger of the two. Tolerance rows are cited as
T-bit ("Bit-identity of default output"), T-two ("Same discrete problem by two routes"), T-closed ("Agreement with a closed form"), T-order ("Observed
order of accuracy"). Direct, superposition and damped rows with a frequency name: `w_a = 0.5 omega_1`, `w_b = sqrt((lambda_1 + lambda_2)/2)`, `w_c = sqrt((lambda_4 + lambda_5)/2)`.

| Id | Clause | Check | Limit | Source of the limit | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|---|
| F1 | a3, a5 | modal analysis of S(n), Shell4 and Shell3 (no option): `f_1` against C1, at each n | 1e-3 each | T-closed (continuum) | 4.1e-4 | 2.4 | code verification |
| F2 | a3 | order of `f_1` from n = 16, 32, 64, each element | within 0.5 of 2 | T-order | 2.0 | 0.5 of 0.02 | solution verification |
| F3 | a1, a5 | linear transient of S(n), each element and n: started from `1e-3 L` times the shape of C1 (`u_z = W`, `theta_y = -W'`), `v = 0`, undamped, 200 steps per period, 10 periods: frequency of the tip from its first and last downward zero crossing against C1 | 1e-3 each | T-closed (continuum) | 3.3e-4 | 3 | code verification |
| F4 | a1 | the same runs: largest change of `v^T M v/2 + u^T K u/2` over the run, relative to its start | 1e-8 | T-closed (exact for the discrete problem) | 1e-10 | 100 | code verification |
| F5 | a1, b3 | the runs of F3 with Rayleigh ratio 0.02 at `omega_1` and `6.27 omega_1` of C1: damping ratio from the tip's positive peaks 3 and 10 (parabola through three points) against 0.02 | 1e-3 relative each | T-closed (continuum) | 4.6e-4 | 2.2 | code verification |
| H1 | a4, a6 | direct harmonic of S(n), tip force, 0.5 and 2.5 `omega_1`, loss factor 0 and 0.05: `abs(X - X_C2)/abs(X_C2)` of the mean tip deflection, each element | n = 16: 1e-2, 32: 3e-3, 64: 1e-3 | acceptance of F-630 ("within registered limits"); n = 64 T-closed (continuum) | 5.0e-3, 1.1e-3, 8e-5 | 2, 2.7, 12 | code verification |
| H2 | a4, b3 | as H1 with Rayleigh (the two coefficients of F5) at 0.5 and 2.5 `omega_1` | as H1 | as H1 | as H1 | as H1 | code verification |
| H3 | a4, a6 | as H1 with `HarmonicLoad::from_model` of `ShellPressure` against C3 (`q = p b`, loss factor 0) | as H1 | as H1 | 6.5e-4, 2.6e-4, 1.6e-4 | 15, 11, 6 | code verification |
| H4 | a3, a6 | mode superposition of S(n): six modes of the modal analysis with the static correction, tip force, 0.5 and 2.5 `omega_1`, `Ratio(0)`, against C2 | as H1 | as H1 | as H1 | as H1 | code verification |
| G1 | a7 | direct harmonic against the superposition with every mode (`all_modes_with_shells`, 72 modes, static correction on), V1 to V7, `w_a`, `w_b`, `w_c`, undamped and Rayleigh (ratio 0.02 at `omega_1`, `omega_2`): rel of the complex response | 1e-8 | acceptance of F-630 | 1e-10 | 100 | code verification |
| G2 | a3, b4 | the basis of G1 has as many modes as free unknowns, all eigenvalues finite and positive, `Phi^T M Phi = I` by the assembled mass | exact; 1e-8 | T-two | 1e-12 | 1e4 | code verification |
| G3 | a1, b2 | transient of V1 to V7 from rest under the constant load, `h = T_1/40`, 80 steps, undamped and Rayleigh, against C4 summed over every mode: rel at each step, the largest (denominators over the run) | 1e-8 | T-two | 1e-9 | 10 | code verification |
| G4 | a2, b3 | damped modes of V1 to V7 (basis 8): the six lowest oscillating roots with Rayleigh, and with one loss factor 0.05 on every set, against C5 with `omega_r` of `all_modes_with_shells`: relative in the complex plane | 1e-8 | T-two | 1e-10 | 100 | code verification |
| G5 | a4, b4 | direct harmonic of B in two materials (cells 0 to 2, `E = 2e11`; 3 to 5, `E = 7e10`), both elements, loss factor 0.1 on the second set only: rel against `(K + i eta K_2 - omega^2 M) x = f` solved dense in the test, `K_2 = K(2 E_2) - K(E_2)` of the assembled stiffness | 1e-8 | T-two | 1e-11 | 1000 | code verification |
| G6 | a2, b4 | damped modes of the model of G5 (basis 8): for the six lowest oscillating roots the projected residual `Phi^T (lambda^2 M + K + i eta K_2) x` over the sum of the norms of its three terms | 1e-8 | T-two | 1e-11 | 1000 | code verification |
| G7 | a3 | static correction: basis of 4 modes of V1, V2; the response at `omega = 0` against `solve_static` and against the direct harmonic at 0 | 1e-8 | T-two | 1e-11 | 1000 | code verification |
| G8 | a3 | free body: V1, V2 without clamp, every mode and inertia relief against the direct harmonic, `w_b` of its elastic modes, undamped | 1e-8 | acceptance of F-630 | 1e-9 | 10 | code verification |
| P1 | a4 | direct harmonic with a uniform pre-stress `sigma_xx = 1e8` Pa, V1, V2, `w_a`, `w_b`, against `(K + K_sigma - omega^2 M) x = f` dense | 1e-8 | T-two | 1e-11 | 1000 | code verification |
| P2 | a3 | superposition with the pre-stress, every mode of `(K + K_sigma, M)` and the static correction, against the direct solve of P1 | 1e-8 | acceptance of F-630 | 1e-10 | 100 | code verification |
| R1 | b1 | V1, V2 held by a rigid set on the root edge with the corner node clamped, against the clamped model: direct harmonic (`w_a`, `w_b`), superposition (4 modes, static correction) and transient (20 steps): rel over the mesh | 1e-8 | T-two | 1e-10 | 100 | code verification |
| R2 | b1, a7 | V1, V2 with a nodal spring (translations 1e8, rotations 1e6) on the tip edge: direct harmonic against every mode of the reduced `K_r, M_r` (dense) with the static correction, `w_a`, `w_b`, undamped and Rayleigh | 1e-8 | acceptance of F-630 | 1e-10 | 100 | code verification |
| R3 | b1, b2 | transient of the model of R2 from rest under the load against C4 over every reduced mode, undamped and Rayleigh | 1e-8 | T-two | 1e-9 | 10 | code verification |
| R4 | b1 | base motion: direct harmonic with the root moved by 1 in z, V1, V2, plain, against `r_z + w`, `(K - omega^2 M) w = omega^2 M r_z` dense | 1e-8 | T-two | 1e-11 | 1000 | code verification |
| R5 | b1 | transient with a motion history: all root nodes moved in z by `s(t) = t`, started with the velocity `r_z`: the displacement is `t r_z` at step 40 (`h = T_1/40`), rel | 1e-8 | T-closed (exact for the discrete problem) | 1e-11 | 1000 | code verification |
| X1 | b4, a7 | mixed X (V1, V2): direct harmonic against every reduced mode with the static correction, `w_a`, `w_b`, Rayleigh; and with loss factor 0.1 on the shell set only against the reduced dense solve as G5 | 1e-8 | acceptance of F-630 | 1e-10 | 100 | code verification |
| X2 | b4, b2 | mixed X: transient from rest against C4 over every reduced mode, undamped | 1e-8 | T-two | 1e-9 | 10 | code verification |
| X3 | b4, b5 | with the option on, on each path of D3: a shell and a beam refused naming `beam::FOLLOW_UP`; a shell node shared with a solid refused, key `node`; warp above the limit refused, key `warp`; a Shell3 layup refused naming `laminate::FOLLOW_UP`; a viscoelastic material with a shell refused; a loss table on a layup refused, key `loss factor` (direct); a cyclic constraint without its option refused (transient: `Cyclic`; direct, static correction: as for solids); `RandomSolver` refused; the stress of a response refused | as stated | acceptance of F-630 (a0) | as stated | n/a | code verification |
| X4 | b5 | without the option each refusing path of "What is refused now" refuses V1 and V2, `NotYetSupported` with the element's own package; the paths taken there are taken | as stated | acceptance of F-630 (a8) | as stated | n/a | unchanged default output |
| X5 | b4 | one isotropic ply as a layup against the `Shell` block: displacements of transient, damped modes, superposition with every mode and direct harmonic | equal bits | T-bit | equal | 1 | code verification |
| X6 | b1, b4 | the frustum sector with its option, transient started from mode 1 and from mode 3 of `modal_analysis_mpc`, 80 steps, `h = T/40`, against C4: rel, both elements | 1e-8 | T-two | 1e-9 | 10 | code verification |
| X7 | D6, b4 | `mass_properties` of the flat block B (V1, V2): mass, and the inertia against C6 | 1e-12 relative | T-two (rounding) | 1e-14 | 100 | code verification |
| X8 | b4 | `mass_properties` of V6: the mass against `(0.06 * 7850 + 0.04 * 2700) * 0.2` kg | 1e-12 relative | T-two (rounding) | 1e-14 | 100 | code verification |
| B1 | a8, b6 | `sa_shells_linear_dynamics_bits_f630`: digests of the results on every path of models without shells (Hex8; plain, rigid set, spring, two materials, loss, Rayleigh, dashpot, free body, pre-stress, motion), of the shell paths taken now (modal plain and on a rigid set, superposition without the static correction, participation), and the text of every refusal of X4, equal to the record of main, on Windows x86_64 and Linux x86_64 | equal | T-bit; acceptance of F-630 | equal | 1 | unchanged default output |
| B2 | a8 | the Hex8 models of B1 with the option on against without it | equal bits | T-bit | equal | 1 | unchanged default output |
| B3 | a8 | the named existing tests, unedited: every test file of rcfd-fem whose name holds shell, modal, harmonic, transient or damped, and `cargo test -p rcfd-fem --release --lib` | pass | brief, task 4 | pass | n/a | unchanged default output |
| B4 | a8 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | rule 3 | pass | n/a | unchanged default output |
| N1 | controls | NC1 to NC4 below | as stated, at least 100 times the row's limit | tolerances, the sentence on negative controls | as stated | n/a | code verification |
| Q | process | `cargo fmt -p rcfd-fem -- --check`; clippy of rcfd-fem, all targets, `-D warnings`; `docs_site`; `roadmap_proposals.py check`; `process_report.py --check` | clean; only a generated file not current may fail | rule 7 | clean | n/a | code verification |

No row is of kind validation: the acceptance claims no agreement with a measured case. All new tests are fast (S(64) has 3510 free unknowns; the
longest expected is F3 to F5, under two minutes for all runs together in release; if one exceeds that it becomes an ignored heavy record by an
amendment before any second run).

**The bit record.** `B1` is written first, as the second commit, using the API of main only, and run on the unchanged library (two runs on each platform,
which must agree): the record of main, in `.tsv` (Windows x86_64) and `.linux.tsv` (Linux x86_64, in the image of
[ADR 0150](0150-job48p-linux-example-stores.md), offline, from an export of the commit). A missing line or a changed digest fails. On another platform
the refusals only are asserted. If the container cannot build, no Linux record is registered and the row is a miss as to Linux. These record runs are
set-up runs, disclosed here; everything else is run once on the changed library.

## Negative controls (in the test file)

- **NC1** the closed form C2 with `E I` of `t = 0.011` differs from the measured tip response of H1 (n = 64, 0.5 `omega_1`) by more than 0.1.
- **NC2** the recurrence C4 with the frequencies of the model with half the rotary mass (the entries of `M` between rotation unknowns halved by the test; none would make `M` singular) differs from G3 (V1) by more than 1e-6.
- **NC3** superposition with 71 of the 72 modes and no static correction differs from the direct harmonic of G1 (V1, `w_a`) by more than 1e-6 (the mode of the largest modal force left out).
- **NC4** the direct harmonic of G5 with the loss factor on both sets differs from the dense solve with it on one by more than 1e-3.

## Mutations (scratch copy of the code commit, target folder of their own, each alone; the unchanged copy is run too)

| Id | Change | Must fail |
|---|---|---|
| M1a, M1b | the rotary (and drilling) mass `rho t^3/12` doubled: `shell::local_mass`; `shell3::local_mass` | X7 |
| M2a, M2b | the drilling mass left out (the third rotation's diagonal skipped): `shell::local_mass`; `shell3::local_mass` | G1 or G2 |
| M3a to M3h | the option ignored on one path: transient plain; transient transformation; damped; `all_modes_with_shells`; static correction plain; static correction transformation; direct; the cyclic call of D3 left out | the rows of the path: F3; R3; G4; G1; G7; R2; G1; X3 (the last) |
| M4 | the load vector of a shell left out: `HarmonicLoad::from_model` zero on a model with shells | H3 |
| M5 | the Shell3 dispatch of `mass.rs` removed | F1, G1 |
| M6a, M6b, M6c | the damping matrix without shells: Rayleigh taken as zero when the option is on, in transient, damped, direct | F5 and G3; G4; H2 and G1 |
| M7a, M7b | the per-material assembly of D2 by the old line: direct; damped | G5; G6 |
| M8 | the table refusal on a layup left out | X3 |
| M9 | the refusal of `RandomSolver` left out | X3 |
| M10 | `check_shell_model` left out of the new checks | X3 |
| M11a to M11d | the default of the option `true`: transient, damped, harmonic, direct | X4 and B1 |
| M12 | `Beam2` taken by the new checks | X3 |
| M13 | the Shell4 layup mass replaced by the homogeneous mass of the set's material | X8 (the two-route rows share `M` and do not see it) |

Committed: M7a as `crates/rcfd-fem/tests/sa_shells_linear_dynamics_f630.mutation.patch` (first line `# expect-fail: cargo test --release -p rcfd-fem --test
sa_shells_linear_dynamics_f630`) and M11a as `sa_shells_linear_dynamics_bits_f630.mutation.patch` (the same with the bits file).

Size (agent): done; 0; 0; 1; 500000

## Decision rule

**done** if every registered row holds on both elements and all the paths of the acceptance, each mutation is caught and the named existing tests pass
unedited; **partial** otherwise, with the path named. (The brief's rule. A mutation that names several rows is caught if one of them fails.)

## What is not covered

- Lifting the refusal by default (the open question below); case-file keys; the Studio; `DynamicSolid` of rcfd-post, which hands the caller's `TransientOptions` on.
- Beams on the linear dynamic paths; the rotating paths; the random response of shells; `about_state`; damped modes on the transformation.
- Stress recovery of a harmonic shell response at integration points.
- The accuracy of warped, layered or ANDES shells against a reference: their rows compare two routes on one discrete model.
- Thermal, follower and curvature shell loads in the dynamic paths (the load vector is that of the static solve).

## Open questions for the maintainer

1. Whether the paths take shells by default. (a) Keep the opt-in; (b) lift the refusals by default, as ADR 0744 and ADR 0746 did, which retires or restates the
   existing refusal rows (found by search: `sa_shell3_paths.rs` RC1, RC2; `sa_shell3.rs`; `si_shell.rs`; others to be listed then). Recommendation: (b), as an item
   of its own after this one is merged, because the modal analysis already takes shells and the opt-in carries no choice of method. Not decided here.

## Amendment 1 (2026-11-25): set-up details the table leaves open

Written after the library change and the test file were written and compiled (`cargo check`, `--no-run`) and before either was
committed or any of their checks was run; the only runs so far are the two record runs of B1 per platform on main. No limit,
row, control or mutation changes.

1. **F5, the numbering of the peaks.** A run of ten periods from a deflection at rest starts at a maximum and ends before its
   tenth later maximum (the damped period and the rule's period are longer than `T_1`). The start is peak 1, so peaks 3 and 10 are
   those near `2 T_1` and `9 T_1`, seven periods apart.
2. **R5, the kind without a reference.** The exact rotations are zero, so "rel" of the rotations has no denominator. The largest
   difference over all unknowns (translations and rotations) is divided by `t`, the exact translation.
3. **Modal options** of every row that calls `modal_analysis` (also the basis of the damped modes): `ResidualTest::Backward`, tolerance
   1e-12, `subspace_size` 24, `max_iterations` 1000, as in the record runs of B1. The first record run on main with the default
   size and 200 iterations stopped at "not converged" on block B; that is why they are set (a set-up run on the unchanged library).
4. **Parameters not stated.** S: tip force 10 N, pressure 100 Pa; H3 compares with the sign of the load's z resultant. G5, G6, NC4: the
   two materials differ in `E` only (`nu = 0.3`, `rho = 7850`); the three frequencies of G1. X: solid `E = 7e10, nu = 0.33, rho = 2700`, shell steel.
   P1, P2: the frequencies of `(K + K_sigma, M)`; R2, R3, X1, X2: those of the reduced pencil. NC2: the eigenvalues of the changed
   pencil take the places of the old ones in ascending order, with the old shapes. F3: crossings interpolated linearly.
5. **X3.** Warp is checked with Shell4 only (a triangle is flat). The static correction gets a basis of one vector of ones, since the
   refusal comes before the modes are used. A viscoelastic model is also given to the damped modes with a reference frequency.
   That a table on a `Shell` block is taken is asserted beside the refusal on a layup.
6. **Q, the format check on Windows.** `cargo fmt -p rcfd-fem -- --check` cannot start on this Windows machine ("The filename or extension
   is too long", os error 206: the crate's file list exceeds the command line), as ADR 0793 records. The same command is run in
   the Linux container of B1 on an export of the code commit; both outputs are reported.
