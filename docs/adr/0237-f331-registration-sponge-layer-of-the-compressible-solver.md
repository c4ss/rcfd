# ADR 0237: F-331 registration: sponge (absorbing) layer of the compressible solver, plain and matched form, against 1 percent at 0, 30, 45 and 60 degrees

Status: **REGISTERED 2026-11-12**, written before any code of F-331 and before any run of its checks. Results go in an
addendum; nothing above the addendum is changed after a run. Not yet accepted by the maintainer.

Parents: [ADR 0233](0233-f243-registration-impedance-and-quiet-boundary.md) (F-243: the impedance wall, the two quiet
boundaries, their oblique reflection, question 4), [ADR 0006](0006-compressible-boundary-conditions.md) (F-062).

Number: the lead gave 0237. Branch `f331-sponge-layer` from main 7b65c5f4. Worktree `E:/projects/cfd/w49i-work/f331`.

## What was run and read before this text (disclosed)

- **Read:** the F-331 and F-243 entries of `docs/roadmap.json`; ADR 0233 in full (registration and addendum);
  `crates/rcfd-fv/src/compressible.rs` in full; `crates/rcfd-fv/tests/impedance_f243.rs` in full; the F-243 section of
  `docs/acoustics.md`; the signatures of `box_2d` and `box_2d_coords` in `crates/rcfd-mesh/src/gen.rs`.
- **Searched:** `compressible.rs` for "periodic" and "cyclic": no hit. The compressible solver treats a cyclic patch as a
  boundary patch, so a test cannot use a periodic direction.
- **Built, not run:** `cargo test --release -p rcfd-fv --no-run` on the unchanged tree (to warm the target directory). No
  test was run.
- **Arithmetic, two scratch Python scripts (numpy), no rcfd code, not committed:** two frequency-domain models of linear
  acoustics with a damping layer, described in "Design arithmetic" below with every equation they solve. Their numbers
  fixed the form of the layer and its parameters. They model the differential equations (M1) and a staggered second-order
  difference form of them (M2); they do not model the solver's HLLC/MUSCL scheme, and nothing of them was fitted to a
  solver run (there was none).
- **By hand:** the derivations DR1 to DR7 and the set-up numbers of the check tables.

No library or test code of F-331 existed when this text was written and no check below was run.

House rules: limits are fixed here and are not changed after a run; a missed limit becomes an ignored `*_known_miss` test
with the limit intact and the measured number recorded, and is diagnosed, not tuned away; a defective set-up gets a
corrected test by an amendment committed before the corrected run, the old test staying as an ignored record; each
registered run is made once; the layer's parameters are fixed here from the derivation and are not searched for after a
run; defaults stay bit-identical; `baseline_hashes.rs`, the hash records and the goldens are not touched; no new
third-party crate, no unsafe code; unsigned commits, no trailers, no push.

## What F-331 asks

Acceptance (binding): "A plane pulse leaving through a sponge layer at 0, 30, 45 and 60 degrees reflects less than 1 percent;
a case without a sponge is bit-identical." Background (ADR 0233): both quiet boundaries reflect `(1 - cos)/(1 + cos)` of an
oblique plane wave: 7 percent at 30 degrees, 17 percent at 45 degrees (measured 0.060 to 0.081 and 0.157 to 0.178), and 33
percent at 60 degrees by the same formula.

## Derivations

Notation: ambient state `rho0`, `p0`, `c`, at rest; layer in `0 <= x <= L` with the depth `x` measured along the unit normal
`n` of the layer (pointing from the interior into the layer); `y` along the layer; damping rate `sigma(x) >= 0` in 1/s;
perturbations `p'`, `u'` (normal), `v'` (tangential); time factor `e^{-i omega t}`, trace wavenumber `k_y = k sin(theta)`,
`k = omega/c`; `s = sigma - i omega`; `alpha = s/(-i omega) = 1 + i sigma/omega`.

**DR1. The plain sponge.** Every conserved variable relaxes to a reference state: `dU/dt = ... - sigma (U - U_ref)` (Israeli
and Orszag 1981; Bodony 2006). Linearised about `U_ref` at rest: `p'_t + rho0 c^2 (u'_x + v'_y) = -sigma p'`,
`rho0 u'_t + p'_x = -sigma rho0 u'`, `rho0 v'_t + p'_y = -sigma rho0 v'`. A uniform state equal to `U_ref` has zero source.
A uniform state different from it decays as `e^{-sigma t}` in every conserved variable.

**DR2. The plain sponge is matched at normal incidence.** With `k_y = 0` the equations are `w+-_t +- c w+-_x = -sigma(x)
w+-` for `w+- = p' +- rho0 c u'`: the two characteristics do not couple for any profile `sigma(x)`, so the layer itself
reflects nothing, and a wave that crosses the layer, is reflected at its end with the coefficient `R_b` and crosses it again
leaves with

    R = R_b exp(-2 I / c),     I = integral of sigma over the layer.

**DR3. The plain sponge is not matched at oblique incidence.** Eliminating `u'`, `v'`: `k_x^2 = (omega + i sigma)^2/c^2 -
k_y^2` and the wave impedance in the normal direction is `Z = p'/u' = rho0 c Omega / sqrt(Omega^2 - omega^2 sin^2)`, `Omega =
omega + i sigma`. In front of the layer `Z0 = rho0 c / cos(theta)`. For `sigma << omega`: `Z/Z0 = 1 - i (sigma/omega) tan^2
(theta)`; for `sigma >> omega`: `Z -> rho0 c`, so a component whose wavelength is long against the layer sees the layer as
the matched wall of ADR 0233 and is reflected by `(cos - 1)/(cos + 1)`. A pulse with a non-zero mean (a Gaussian) has most
of its spectrum at long wavelengths. Model M1 below gives the size: at 60 degrees a plain layer needs a thickness of about
7 (175 pulse widths, 1400 cells of the ADR 0233 grid) for 1 percent. The plain form therefore cannot meet the acceptance
at 45 and 60 degrees in a layer of practical thickness. This is a property of the equations, not of a profile.

**DR4. The matched form.** Let the three equations have the rates `s_p`, `s_u`, `s_v` in place of `-i omega`: `s_p p' =
-rho0 c^2 (u'_x + i k_y v')`, `rho0 s_u u' = -p'_x`, `rho0 s_v v' = -i k_y p'`. For constant rates `k_x^2 = -s_u s_p/c^2 -
k_y^2 s_u/s_v` and `Z = i rho0 s_u/k_x`. `Z = Z0` for every angle needs `k_x = alpha k cos(theta)` with `s_u = -i omega
alpha`, hence `s_p = -i omega alpha` and `s_v = -i omega/alpha`. With `alpha = 1 + i sigma/omega`: `s_p = s_u = sigma - i
omega` (the plain damping of the pressure and of the normal velocity), and the tangential equation becomes `-i omega rho0
v' = -alpha p'_y = -(p' + sigma Q)_y` with `-i omega Q = p'`. In the time domain:

    p'_t + rho0 c^2 div u' = -sigma p'
    rho0 u'_t + p'_x       = -sigma rho0 u'            (normal momentum: damped)
    rho0 v'_t + p'_y       = -sigma Q_y                (tangential momentum: not damped, forced)
    Q_t                    = p'                        (one scalar of memory per cell)

Substituting `dX = alpha dx` turns these into the undamped equations in `X`, for any profile `sigma(x)`: the solution in the
layer is `exp(i k cos(theta) (x + i I(x)/omega))`, `I(x)` the integral of `sigma` from the front to `x`. So the layer itself
reflects nothing at any angle and any frequency, and

    R(theta) = R_b(theta) exp(-2 cos(theta) I / c).

This is the perfectly matched layer of Berenger (1994) in the unsplit form with an auxiliary variable (for the Euler
equations: Hu 1996, 2001), written here for a medium at rest and derived above; the papers are cited from memory.
Stability of the constant-coefficient problem: the dispersion relation is `(omega + i sigma)^2 (omega^2 - c^2 k_y^2) = c^2
k_x^2 omega^2`; the literature result (Becache, Fauqueux and Joly 2003, from memory) is that the layer of an isotropic wave
equation has no growing mode. It is not re-derived here; the checks run it.

**DR5. Profile and peak rate.** `sigma(x) = sigma_max (x/L)^m`, so `I = sigma_max L/(m + 1)`. For a wanted two-way
attenuation `R0` at normal incidence (DR2, DR4 with `R_b = 1`):

    sigma_max = (m + 1) c ln(1/R0) / (2 L)

(the polynomial grading commonly used for such layers, for example Collino and Tsogka 2001, from memory; the formula is
DR2 solved for `sigma_max`). At the angle `theta` the matched form attenuates by `R0^cos(theta)`. `m = 2`: the rate and
its slope vanish at the front, where the wave is strongest.

**DR6. Why the profile does not reflect more than the target.** In the differential equations the matched form reflects
nothing (DR4). What is left is the discretisation: the layer must be resolved. Model M2 (second-order staggered
differences on the test grid) gives a reflection that follows `R0^cos(theta)` down to about `1e-6` for 20 to 60 cells with
`m = 2` or `3`, so the difference form of the profile adds nothing visible at 40 cells. The solver's scheme (collocated, upwind,
limited) is not that scheme; its own layer error is not known beforehand and is what the checks measure. Thickness against
wavelength: the matched form has no wavelength condition in the equations; the plain form needs `L` of many wavelengths
at oblique incidence (DR3) and none at normal incidence (DR2). The time step: the source is explicit, `sigma_max dt` must
stay well below the stability limit of the Runge-Kutta scheme on the negative real axis (2.5 for SSP-RK3).

**DR7. The boundary behind the layer.** Any boundary works: the layer multiplies its reflection by `R0^cos(theta)` (matched)
or `R0` at normal incidence (plain). With a fully reflecting end and `R0 = 1e-6`: `1e-6`, `6.4e-6`, `5.7e-5`, `1.0e-3` at 0,
30, 45, 60 degrees. With the Thompson boundary (`NonReflecting`, `sigma = 0`) behind, these are multiplied by about `(1 -
cos)/(1 + cos)` (0, 0.07, 0.17, 0.33). The reference state of the sponge holds the ambient pressure, which the Thompson
boundary alone does not (ADR 0233 NC3).

## Design arithmetic (the two scratch models)

Both: `rho0 = c = 1`, Gaussian pulse `exp(-(xi/w)^2)`, `w = 0.04` (ADR 0233 set-up O), spectrum `w/(2 sqrt(pi))
exp(-k^2 w^2/4)`; the reflected pulse is the inverse transform of `R(k)` times the spectrum; the number reported is its
largest absolute value against the incident peak 1.

- **M1 (differential equations, plain form).** Riccati equation for `Z = p'/u'`: `Z' = -s + Z^2 (s + k_y^2/s)`, integrated
  from the end of the layer (Thompson: `Z = 1/(1 + k_y^2/s^2)`; matched wall: `Z = 1`) to the front with RK4, `R = (Z -
  Z0)/(Z + Z0)`. Isolated pulse, `m = 2`, `I = 3` (the weakest layer tried, which reflects least):

  | `L` | 0 deg | 30 deg | 45 deg | 60 deg |
  |---|---|---|---|---|
  | 0.2 | 0.0000 | 0.044 | 0.117 | 0.259 |
  | 0.8 | 0.0000 | 0.012 | 0.034 | 0.085 |
  | 3.2 | 0.0000 | 0.0031 | 0.0087 | 0.022 |

  The reflection falls as `1/L`; `m = 3`, `I = 6` and `12` and the matched wall behind give the same picture (within 50
  percent). No plain layer of practical thickness reaches 1 percent at 45 or 60 degrees for an isolated Gaussian.
- **M2 (staggered second-order differences in `x`, `dx = 0.005`, exact in `y` and `t`).** `coef_j p_j + (u_{j+1/2} -
  u_{j-1/2})/dx = 0`, `s_{j+1/2} u_{j+1/2} + (p_{j+1} - p_j)/dx = 0`, with `coef = s cos^2(theta)` (matched form) or `s +
  k_y^2/s` (plain form), a rigid end (`u = 0`) or a Thompson-like end (`u = p`), and the discrete wavenumber in front.
  Matched form, rigid end, isolated pulse:

  | cells, `I`, `m` | 0 deg | 30 deg | 45 deg | 60 deg | `exp(-2 I cos)` |
  |---|---|---|---|---|---|
  | 20, 6, 2 | 7.0e-6 | 3.3e-5 | 2.2e-4 | 2.5e-3 | 6.1e-6, 3.1e-5, 2.1e-4, 2.5e-3 |
  | 40, 6, 2 | 6.1e-6 | 3.1e-5 | 2.1e-4 | 2.5e-3 | the same |
  | 40, 8, 2 | 6.5e-7 | 9.9e-7 | 1.2e-5 | 3.4e-4 | 1.1e-7, 9.6e-7, 1.2e-5, 3.4e-4 |

  Plain form, 40 cells, `I = 6`, `m = 2`, Thompson-like end: isolated pulse `9e-7`, `0.053`, `0.136`, `0.290`; the pulse
  trains of set-up D below `0.044`, `0.116`, `0.252` at 30, 45, 60 degrees.

## Decisions

- **D1. One opt-in sponge with two forms.** `Sponge { reference: Prim, rate: Vec<f64>, form: SpongeForm }`, `rate` one
  damping rate per cell. `SpongeForm::Plain`: DR1, `S = -rate (U - U_ref)` on the five conserved variables; any reference
  state, any set of cells. `SpongeForm::Matched { normal }`: DR4 for one planar layer with the unit normal `normal`: mass
  and total energy relax as in the plain form, the momentum relaxes only along `normal`, the tangential momentum receives
  the force `-rate (grad Q - normal (normal . grad Q))` and the energy the work of that force; `Q_t = p - p_ref` in the
  cells with a positive rate. The matched form is derived for a reference at rest and is refused for a reference with a
  velocity. Why two: the plain form is the robust one for a flow that leaves through the layer (it damps everything,
  vortices included) and is exact at normal incidence; only the matched form can meet the acceptance at 45 and 60 degrees
  (DR3). Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation); the alternative, the plain
  form alone with a registered miss at 45 and 60 degrees, was rejected because the derivation says beforehand that it
  misses.
- **D2. Discretisation.** The source is added to the residual of every Runge-Kutta stage (explicit). `grad Q` is the
  Green-Gauss gradient with the face value `(Q_c + Q_nb)/2` on an internal face whose two cells both have a positive
  rate and `Q_c` on every other face (zero normal gradient at walls and at the front of the layer; on a grid aligned with
  the layer the tangential part does not use the neighbours across the front). `Q` is advanced with the same stage
  weights as `U`. Cells with rate zero are skipped without any arithmetic.
- **D3. Geometry helper.** `SpongeLayer { origin, normal, thickness, rate_max, exponent }` gives `rate = rate_max
  (d/thickness)^exponent` for the depth `d = (x_c - origin) . normal` in `[0, thickness]`, `rate_max` beyond, zero in
  front; `rate_max_for_reflection(thickness, c, R0, exponent)` is DR5; `Sponge::planar(geom, reference, layer, matched)`.
- **D4. Solver.** `CompressibleSolver::set_sponge(Option<Sponge>)` (validated: one finite rate `>= 0` per cell, positive
  finite reference density and pressure, a unit normal and a reference at rest for the matched form), `sponge()`,
  `sponge_memory()` (`Q`). `step` branches to a separate sponged path when a sponge is set; without one it runs the
  existing statements unchanged, so the default is bit-identical by construction. `max_dt` is not changed.
- **D5. Not on moving meshes.** `step_ale` and `replace_mesh` return an error while a sponge is set (the rates are per
  cell index; an ALE form of the source and of `Q` is not derived here).
- **D6. No case-schema wiring** (the job's instruction; a new key needs an approved edit of a registered hash test). No file
  of `rcfd-api` is edited.
- **D7. Parameters of the registered checks**, from DR5 to DR7, fixed now: thickness `L = 0.2` (40 cells of `dx = 0.005`, 5
  pulse widths; M2 shows no gain beyond 20 cells, 40 leaves a factor 2 for the solver's coarser effective resolution),
  `m = 2`, `R0 = 1e-6`, hence `sigma_max = 3 ln(1e6)/0.4 = 103.616` and `sigma_max dt = 0.10` at `dt = 0.001`; reference =
  the ambient state at rest; Thompson boundary behind the layer unless a check says otherwise.

Code: new `crates/rcfd-fv/src/sponge.rs` (types, profile, validation, unit tests), `crates/rcfd-fv/src/compressible.rs`
(two fields, the methods of D4, the sponged step, the two refusals), `crates/rcfd-fv/src/lib.rs` (the module line), new
`crates/rcfd-fv/tests/sponge_f331.rs`. No existing test file is edited.

## Checks and limits (registered)

Common: `gamma = 1.4`, `rho0 = 1`, `p0 = 1/1.4`, `c = 1`; solver defaults (HLLC, MUSCL van Leer, SSP-RK3, CFL 0.4);
`eps = 1e-4 p0`; pulses `g(xi) = exp(-(xi/w)^2)`, `w = 0.04`, as exact isentropic simple waves (ADR 0233), superposed by
adding the pressure perturbations and the velocities, density isentropic; `dx = dy = 0.005`; the layer of D7.

**The measure (all A, X, P and NC checks).** Two solvers run in lockstep with the same `dt = min` of their `max_dt()`: the
run under test and a reference run on the same grid extended in `x` behind the layer position, without sponge, so far
that nothing returns from its end. `R = max |p_test - p_ref| / max (p_ref - p0)`, both maxima over the probe cells and
over the steps of the window. This is the size of everything the layer and its boundary send back, against the incident
peak; it needs no calibration run and it cancels the numerical loss of the incident pulse.

### A0, P0: normal incidence, isolated pulse in a tube (fast)

Test run `box_2d(240, 1, 1.2, 0.005)`, reference `box_2d(600, 1, 3.0, 0.005)`; pulse `eps g(x - 0.5)` running right; `xmin`
`Fixed(ambient)`; layer front at `x = 1.0` (layer `[1.0, 1.2]`); probe: the cell with centre `0.8975`; run until `t >= 1.6`;
window: all steps. (The pulse passes the probe at `t = 0.4`, a reflection from the end returns by `t = 1.0`; the
reference's end is reached at `t = 2.5`.)

| id | form, boundary behind | limit | estimate |
|---|---|---|---|
| A0 | matched, Thompson | `R <= 0.01` | DR7: below `1e-6`; solver scheme: below `2e-3` |
| A0s | matched, `Slip` (fully reflecting end) | `R <= 0.01` | `R0 = 1e-6` plus the scheme |
| P0 | plain, `Slip` | `R <= 0.01` | as A0s (DR2) |
| NC0 | no sponge, `Slip` | `R >= 0.9` | rigid wall, ADR 0233 NC1 `0.999` |

In one dimension the two forms have the same equations; both are run because their code paths differ.

### A30, A45, A60: oblique incidence, pulse trains in a duct (heavy, `#[ignore]`, release)

Why trains: an isolated oblique plane pulse needs a transverse extent that grows as `1/(1 - sin(theta))` (the front must
not touch the layer or an open boundary within reach of the probe); at 60 degrees that is about a million cells per run. A
duct with two rigid walls carries the oblique plane wave and its mirror image exactly, with no open transverse boundary.
The price: the field in a duct is periodic along the wave direction, so the incident field is a train of pulses, and its
mean cannot travel at the angle (the transverse average of the train is a uniform state, not a wave). The mean is removed.
X1 below runs one isolated pulse.

Set-up D, angle `theta`, duct height `H`: `Lambda = 2 H sin(theta)`; `G(xi) = sum over m of g(xi - m Lambda) - w
sqrt(pi)/Lambda` (periodic, zero mean); initial field `p - p0 = eps [G(x cos + y sin) + G(x cos - y sin)]`, each term a
simple wave along `(cos, +sin)` and `(cos, -sin)`, in the whole domain including the layer. `ymin`, `ymax` `Slip`; `xmin`
`Fixed(ambient)`; `xmax` Thompson. Layer front at `X_i`, layer `[X_i, X_i + 0.2]`. Probe cells: the column with centres `x
= X_i - 0.1025` (every `y`). Window `t_s <= t <= T`, run until `t >= T`.

- `t_s = (2 L + a + 3 w/cos)/cos + 0.1`, `a = 0.1`: the pulses that started inside the layer (not attenuated on the way
  in) have left through the probe column; the energy of the train travels along `x` with `c cos(theta)`.
- `T = t_s + Lambda + 0.05`: one full period of the pattern at a fixed point.
- `X_i >= T + 0.2`: the truncation of the train at `xmin` reaches the probe after `T` (signals travel at `c`).
- reference length `>= X_i + T - 0.05`: its end, where the train is cut from `t = 0`, is `T + 0.05` or more from the probe
  column.

| id | theta | `H` (cells) | `Lambda` | `t_s` | `T` | `X_i` | test grid | reference grid |
|---|---|---|---|---|---|---|---|---|
| A30 | 30 | 0.64 (128) | 0.6400 | 0.84 | 1.53 | 1.75 | `box_2d(390, 128, 1.95, 0.64)` | `box_2d(660, 128, 3.3, 0.64)` |
| A45 | 45 | 0.455 (91) | 0.6435 | 1.05 | 1.75 | 1.95 | `box_2d(430, 91, 2.15, 0.455)` | `box_2d(740, 91, 3.7, 0.455)` |
| A60 | 60 | 0.37 (74) | 0.6409 | 1.58 | 2.28 | 2.50 | `box_2d(540, 74, 2.7, 0.37)` | `box_2d(960, 74, 4.8, 0.37)` |

Neighbouring pulses of a family are 16 widths apart. At a wall the two families coincide, so the incident maximum over
the column is near `2 eps (1 - 0.11)` and a reflection with the coefficient `R` gives the measure `R`.

| id | run | limit | estimate |
|---|---|---|---|
| A30, A45, A60 | matched form, Thompson behind | `R <= 0.01` each | DR7: `4.6e-7`, `9.8e-6`, `3.3e-4`; solver scheme unknown, expected below `3e-3` |
| NC30, NC45, NC60 | the same run without sponge | `R` in `[0.04, 0.10]`, `[0.12, 0.21]`, `[0.25, 0.40]` | `(1 - cos)/(1 + cos)` = 0.0718, 0.1716, 0.3333; ADR 0233 measured 0.060 and 0.157 |
| NCP45 | plain form at 45 degrees, Thompson behind | `R >= 0.05` | M2: 0.116 (the plain form fails the acceptance at 45 degrees, DR3) |

### X1: one isolated oblique pulse, 30 degrees, half space (heavy)

Test `box_2d(420, 360, 2.1, 1.8)`, reference `box_2d(720, 360, 3.6, 1.8)`. `ymin` `Slip` (the mirror plane), `ymax` and
`xmin` `Extrapolate`, `xmax` Thompson. Initial field `p - p0 = eps [g(x cos + y sin - 0.545) + g(x cos - y sin - 0.545)]`
(one pulse and its mirror image). Layer front `X_i = 1.9`, matched form. Probe: the cell with centre `(1.7975, 0.0025)`.
Run until `t >= 1.75`, window: all steps. Timing: the peak reaches the layer front at the wall at `t = 1.100`; the
reflection from the end of the layer with a tail of `5 w` has passed the probe at `t = 1.733`; at `t = 0` the pulse is `5.0
w` in front of the layer at `y = 1.8` and further everywhere below; `ymax`, `xmin` and the reference's end are 1.795 or
more from the probe. Limit `R <= 0.01`. Estimate: as A30. Angles 45 and 60 are not registered in this form (cost).

### U, F, S: algebra and conservation (fast)

| id | check | limit |
|---|---|---|
| U1p | `box_2d(40, 20, 1, 0.5)`, state = reference `rho 1.2, u (0.3, 0.1, 0), p 0.9` everywhere, all four boundaries `Fixed(reference)`, plain layer `x >= 0.5`, `rate_max = 50`, 200 steps: largest `|U - U_ref|` over cells and components against `rho_ref`, `rho_ref a_ref`, `E_ref` | `<= 1e-12` |
| U1m | the same with the reference at rest (`rho 1.2, p 0.9`), `Slip` on all four, matched form; also `max |Q| / (p_ref t)` | `<= 1e-12` each |
| U2p | `box_2d(20, 20, 1, 1)`, `Slip`, plain, rate 2 in every cell, reference `(1, 0, p0)`, start `rho = 1.001`, `p = 1.002 p0`, at rest, until `t >= 1`: `(U - U_ref)/(U(0) - U_ref)` against `exp(-2 t)` for mass and energy in every cell; momentum | relative `<= 1e-5`; `<= 1e-12` |
| U2m | the same in the matched form (normal `x`): mass and energy as U2p; `Q` of a cell against the trapezoid sum over the steps of the recorded `p - p_ref` of that cell | relative `<= 1e-5`; `<= 1e-4` |
| F1 | matched source on a prescribed field (unit test with access to the source routine): interior cells of a uniform grid, `Q = b y`, state with a velocity along and across the layer: tangential force `= -rate b`, normal momentum source `= -rate m_n`, energy source `= -rate (E - E_ref) + u . F`, `Q_t = p - p_ref` | each `<= 1e-12` relative |
| S1 | profile: `sum rate dx` over the 40 cells of the D7 layer against `(c/2) ln(1/R0)`; rate zero in front; rate of the last cell `= rate_max (39.5/40)^2`; `rate_max = 103.6163` | `<= 1e-3`, exactly, `<= 1e-12`, `<= 1e-6` relative |
| S2 | refusals: rate count, negative and NaN rate, non-positive reference, matched form with a reference velocity or a zero normal, non-positive thickness, `R0` outside `(0, 1)`, `step_ale` and `replace_mesh` with a sponge; `set_sponge(None)` accepted | error / ok |

Estimates: U1 round-off (the source is zero or of round-off size); U2 the RK3 error of `y' = -2 y` at `dt = 0.01` is `(0.02)^4
/24` per step, `7e-7` over 100 steps; U2m the trapezoid sum has the error `dt^2` times the second derivative, `4e-5`
relative.

### B. Bit identity (the second sentence of the acceptance)

- B1: `cargo test --release -p rcfd-fv --lib compressible`, `--test compressible_bc`, `--test compressible_ale_f258`,
  `--test compressible_ale_f258_records` (the F-258 bit records), `--test impedance_f243` (fast tests) pass unedited.
- B2 (new, fast): set-up D at 45 degrees on `box_2d(60, 20, 0.3, 0.1)`, 30 steps of `max_dt()`: (i) no sponge ever set,
  (ii) a sponge set and then cleared with `set_sponge(None)`, (iii) a plain and a matched sponge whose rates are all zero:
  the conserved state of every cell is bit-identical (`to_bits`) between (i), (ii) and both of (iii). Control: with the D7
  layer on the last 40 columns the state differs.

## Mutations (registered; scratch copies, not committed)

Each is applied to `sponge.rs` or `compressible.rs`; then `cargo test --release -p rcfd-fv --lib sponge` and `cargo test
--release -p rcfd-fv --test sponge_f331` (the fast tests) are run once; MU2 is run against A45 as well.

| id | mutation | expected to be caught by |
|---|---|---|
| MU1 | sign of the relaxation (`+rate (U - U_ref)`) | U2p, U2m, A0s, P0, F1 |
| MU2 | the tangential force dropped (matched form without `grad Q`) | F1; A45 (expected near the Thompson value or worse) |
| MU3 | the tangential momentum damped in the matched form as well (plain damping plus the force) | F1 |
| MU4 | the reference ignored: the source relaxes to the cell's own state (an inert layer) | U2p, U2m, A0s, P0 |
| MU5 | the exponent ignored (linear profile) | S1 |
| MU6 | `Q` not advanced (`Q_t = 0`) | U2m |

A mutation that no check catches is reported as surviving.

## Decision rule (registered)

- **Acceptance met in the library** when A0, A30, A45, A60 and X1 meet 1 percent, B1 and B2 hold, U, F, S meet their
  limits, NC30, NC45, NC60 hold and every mutation is caught. If A30 to A60 meet the limit and X1 misses, the acceptance is
  "met for pulse trains only" and that is said in the note.
- **Proposed roadmap status.** `done` needs, beyond that, that the sponge can be selected from a case file; by D6 that is
  not part of this job, so the status this job can reach is **partial** (acceptance met in the library, wiring open). If an
  A check misses, the status is **partial** with the miss, its measured value and its diagnosis named and the acceptance
  "not met". **todo** stays if the sponged step is not usable at all (for example unstable in A0).
- A0s, P0, NC0, NCP45 do not decide the acceptance; a miss is kept as a `*_known_miss` test and named.

## Limits

- The matched form: one planar layer, reference at rest, derived from linear acoustics; no corner treatment where two
  layers would meet; no mean flow through the layer. The time integral `Q` has no decay: a pressure that stays away from
  the reference with a tangential gradient inside the layer drives the tangential momentum without bound (the known
  zero-frequency weakness of this layer; the frequency-shifted variant is not built).
- The plain form: reflects `(cos - 1)/(cos + 1)` of the long-wave part of an oblique wave (DR3); quiet only near normal
  incidence or with a thickness of many wavelengths.
- Explicit source: `rate_max dt` must stay well below 2.5; `max_dt` does not know the sponge.
- Not on moving meshes (D5). Library only (D6).
- Oblique acceptance at 45 and 60 degrees is checked with periodic pulse trains without mean, not with an isolated pulse.
- Cartesian grids, gas at rest, verification against a reference run and closed forms; no measurement is compared.

## Sources

- M. Israeli, S. A. Orszag, J. Comput. Phys. 41 (1981) 115-135 (damping layers).
- D. J. Bodony, J. Comput. Phys. 212 (2006) 681-702 (analysis of sponge zones).
- J.-P. Berenger, J. Comput. Phys. 114 (1994) 185-200 (perfectly matched layer).
- F. Q. Hu, J. Comput. Phys. 129 (1996) 201-219; J. Comput. Phys. 173 (2001) 455-480 (matched layer for the Euler
  equations, unsplit form).
- F. Collino, C. Tsogka, Geophysics 66 (2001) 294-307 (polynomial profile and its peak value).
- E. Becache, S. Fauqueux, P. Joly, J. Comput. Phys. 188 (2003) 399-433 (stability of matched layers).
- K. W. Thompson, J. Comput. Phys. 68 (1987) 1-24; ADR 0233 DR3 (the boundary behind the layer).

All are cited from memory of their content; none was opened for this job and no equation number is quoted. Every formula
used is derived above from the linear acoustic equations. No vendor code or documentation was used.

## Open questions (with recommendations)

- Q-A. Case-schema wiring of the sponge (a table of layers per case, or keys on a boundary). Not taken: it needs approval.
  Recommendation: a follow-up item after the results.
- Q-B. Whether pulse trains are accepted as "a plane pulse" at 45 and 60 degrees. Recommendation: yes, with X1 as the
  isolated-pulse check at 30 degrees; the alternative is a run of about a million cells per angle.
