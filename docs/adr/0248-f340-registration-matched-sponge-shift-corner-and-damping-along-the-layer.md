# ADR 0248: F-340 registration: the matched sponge layer with a frequency shift (memory decay), a corner of two layers and a damping along the layer

Status: **REGISTERED 2026-11-12**, written before any code of F-340 and before any run of its checks. Results go in an
addendum; nothing above the addendum is changed after a run. Not yet accepted by the maintainer.

Parents: [ADR 0237](0237-f331-registration-sponge-layer-of-the-compressible-solver.md) (F-331: the plain and the matched
sponge, amendment 1, questions 2 and 3), [ADR 0243](0243-f332-registration-mass-spring-damper-wall.md) (the newest state
of `compressible.rs`; a wall together with a sponge stays refused).

Number: the lead gave 0248. Branch `f340-matched-sponge` from main e22ab101. Worktree `E:/projects/cfd/w49i-work/f340`.

## What was run and read before this text (disclosed)

- **Read:** the F-340 and F-331 entries of `docs/roadmap.json`; ADR 0237 in full (registration, amendment 1, addendum 1);
  `crates/rcfd-fv/src/sponge.rs` and `crates/rcfd-fv/tests/sponge_f331.rs` in full; the sponge hooks, `step`, `max_dt`
  and the wall refusals of `crates/rcfd-fv/src/compressible.rs`; the D6 lines of ADR 0243; the sponge section of
  `docs/acoustics.md`; the names of the mesh generators in `crates/rcfd-mesh/src/gen.rs`.
- **Built, not run:** `cargo test --release -p rcfd-fv --no-run` on the unchanged tree (to warm the target directory). No
  test was run.
- **Arithmetic, scratch Python (numpy), no rcfd code, not committed** (`E:/projects/cfd/w49i-work/f340-scratch`): two
  models of linear acoustics, described in "Design arithmetic" with every equation they solve. M3 is the frequency-domain
  model M2 of ADR 0237 with the new terms. M4 is new: a time-domain model in two dimensions (staggered grid, second-order
  central differences, fourth-order Runge-Kutta), which repeats the set-ups of this ADR and of ADR 0237 on the same grids.
  Neither models the solver's HLLC/MUSCL scheme. M4 was first compared with the *published* numbers of ADR 0237 (set-up D,
  the controls); it was then used to try layer variants and to choose the parameters fixed below. This is a search in a
  model before the registration; no solver run was made and none is used. Every variant tried is listed with its number,
  including the ones that were dropped.
- **By hand:** the derivations DR9 to DR15 and the set-up numbers of the check tables.

No library or test code of F-340 existed when this text was written and no check below was run.

House rules: limits are fixed here and are not changed after a run; a missed limit becomes an ignored `*_known_miss` test
with the limit intact and the measured number recorded, and is diagnosed, not tuned away; a defective set-up gets a
corrected test by an amendment committed before the corrected run, the old test staying as an ignored record; each
registered run is made once; the layer's new parameters are fixed here and are not searched for after a run; existing
tests and records are not edited; defaults stay bit-identical; `baseline_hashes.rs`, the hash records and the goldens are
not touched; no new third-party crate, no unsafe code; unsigned commits, no trailers, no push.

## What F-340 asks

Acceptance (binding), five clauses, for the matched form of ADR 0237:

1. "The set-up D record of ADR 0237 (a wave train started inside the layer with zero memory in a rigid duct) falls below
   1 percent within its registered window";
2. "a constant pressure offset with a gradient along the layer gives a bounded tangential momentum over 100 transit
   times of the layer";
3. "a pulse leaving through the corner of two layers reflects less than 1 percent";
4. "an isolated plane pulse at 60 degrees reflects less than 1 percent";
5. "the checks A0, A30b, A45b, A60b, X1 and X45 of ADR 0237 still meet 1 percent".

**What this registration finds beforehand** (derived below, so that it is not a surprise of the addendum):

- Clauses 3, 4 and 5 can be met, and clause 2 is met by a layer with a frequency shift, with one precision: in a run
  the F-331 form is expected to be bounded as well (DR13). What the shift bounds is the memory and the force under a
  pressure that is held.
- **Clause 1 is expected to be missed, by every layer of this thickness, with or without the new options** (DR12). The
  ringing of set-up D sits at the cut-off frequencies of the duct. No matched layer damps it (the field is an exact
  solution of the layer for any profile), and a damping along the layer that removes it within the registered window
  costs a reflection far above 1 percent at 45 and 60 degrees. The model M4 gives 2 to 4 percent for the best trade-off
  against the limit of 1 percent. The clause is registered and run with its limit; the job does not claim it.

## Notation

As ADR 0237: ambient `rho0`, `p0`, `c`, at rest; depth `x` along the unit normal `n` of a layer, `y` along it; rate
`sigma(x)`; time factor `e^{-i omega t}`, `s0 = -i omega`; `k = omega/c`, `k_y = k sin(theta)`; `I(x)` the integral of the
rate from the front. New: the shift `a >= 0` [1/s]; the stretching factor `alpha = 1 + sigma/(a + s0)`; the fraction
`beta >= 0` of the damping along the layer; for two layers the normals `n1`, `n2` (orthogonal), rates `sigma1`, `sigma2`,
`e3 = n1 x n2`.

## Derivations

**DR9. The matched layer as a stretched coordinate, and the two-layer form.** The layer of ADR 0237 DR4 is linear
acoustics in which `d/dx` is replaced by `(1/alpha) d/dx`, `alpha = 1 + sigma/s0` (the complex coordinate stretching of
Chew and Weedon 1994, from memory; the substitution is checked below against DR4). For two layers with orthogonal normals
the same replacement is made along both, with `alpha1(x1)` and `alpha2(x2)`:

    s0 p' + rho0 c^2 (d1 u1/alpha1 + d2 u2/alpha2 + d3 u3) = 0,    rho0 s0 u_j = -d_j p'/alpha_j   (alpha3 = 1).

With the scaled velocities `U1 = alpha2 u1`, `U2 = alpha1 u2`, `U3 = alpha1 alpha2 u3` (the variables the code carries;
they equal the velocity where the rates are zero) and because `alpha1` does not depend on `x2`, `x3` and `alpha2` not on
`x1`, `x3`:

    s0 alpha1 alpha2 p' + rho0 c^2 div U = 0
    rho0 s0 alpha1 U1 = -d1 (alpha2 p'),   rho0 s0 alpha2 U2 = -d2 (alpha1 p'),   rho0 s0 U3 = -d3 (alpha1 alpha2 p').

With `a = 0` and the memories `Q = p'/s0`, `R = Q/s0` (in time: `Q_t = p'`, `R_t = Q`): `s0 alpha1 alpha2 = s0 + sigma1 +
sigma2 + sigma1 sigma2/s0`, `alpha2 p' = p' + sigma2 Q`, `alpha1 alpha2 p' = p' + (sigma1 + sigma2) Q + sigma1 sigma2 R`, so

    p'_t + rho0 c^2 div U = -(sigma1 + sigma2) p' - sigma1 sigma2 Q
    rho0 U1_t + d1 p' = -sigma1 rho0 U1 - sigma2 d1 Q
    rho0 U2_t + d2 p' = -sigma2 rho0 U2 - sigma1 d2 Q
    rho0 U3_t + d3 p' = -(sigma1 + sigma2) d3 Q - sigma1 sigma2 d3 R.

With `sigma2 = 0` this is DR4 of ADR 0237 (normal momentum damped, tangential momentum forced by `-sigma1 grad_t Q`). The
corner needs no new variable in two dimensions: the same `Q`, one more term `-sigma1 sigma2 Q` in the pressure equation
and the force of each layer on the momentum along the *other* layer's normal. In three dimensions the edge needs the
second memory `R` for the momentum along the edge. Matching: every plane wave is the analytic continuation of the
undamped one in both coordinates, so the corner reflects nothing in the differential equations, and a wave returns from
the end of layer `j` with `R_b exp(-2 cos(theta_j) I_j/c)`. (The corner of a perfectly matched layer: Berenger 1994 in the
split form; the unsplit form with products of the stretching factors is standard, cited from memory and derived above.)

**DR10. Why the layer of ADR 0237 has no bound at zero frequency.** `alpha = 1 + i sigma/omega` has a pole at `omega = 0`.
In the time domain: `Q` is the time integral of `p' = p - p_ref` without decay. Under a pressure that is held away from
the reference, `Q = p' t`, the force on the tangential momentum is `-sigma t grad_t p'`, and the tangential momentum grows
as `t^2` where the fluid without a layer would give `t`. The same pole makes the start of set-up D inconsistent for all
time: the missing memory of a wave that "has always been there" is a pattern `dQ(x, y)` that never decays by itself.

**DR11. The published remedy: the complex frequency shift.** `alpha = 1 + sigma/(a + s0)`, `a > 0` (Kuzuoglu and Mittra
1996; the convolutional form of Roden and Gedney 2000; Berenger 2002 for waveguides; all from memory, the equations are
derived here). It is still a coordinate stretching, so the layer stays matched at every angle and frequency. With `Q =
p'/(a + s0)` (in time `Q_t = p' - a Q`), `R = Q/(a + s0)` and, for the damped momentum components, `M = m/(a + s0)` (`M_t = m -
a M`, `m = rho0 U`), and `s0/(a + s0) = 1 - a/(a + s0)`, the equations of DR9 become

    p'_t + rho0 c^2 div U = -(sigma1 + sigma2)(p' - a Q) - sigma1 sigma2 (Q - a R)
    m1_t + d1 p' = -sigma1 (m1 - a M1) - sigma2 d1 Q            (and 1 <-> 2)
    m3_t + d3 p' = -(sigma1 + sigma2) d3 Q - sigma1 sigma2 d3 R
    Q_t = p' - a Q,   R_t = Q - a R,   M_t = m - a M.

`a = 0` gives DR9 (the terms with `a` vanish and `M` is not used). Properties:

- *Zero frequency.* `alpha -> 1 + sigma/a`, real and finite: a static field sees a real stretching. Under a held `p'` the
  memory tends to `p'/a` and the force to `-(sigma/a) grad_t p'`: bounded. `p' - a Q = Q_t` tends to zero, so the layer
  no longer relaxes a static offset to the reference: a static offset is left alone. In a closed volume `p' + (sigma1 +
  sigma2) Q + sigma1 sigma2 R` is conserved; a uniform offset `p'` in a box whose layer has the mean rate `sbar` (over the
  whole box) ends at `p'/(1 + sbar/a)`: at zero frequency the layer acts as a volume `1 + sigma/a` times its own.
- *What it costs.* In the layer a plane wave is `exp(i k cos(theta) (x + I(x)/(a + s0)))`; the attenuation exponent is
  multiplied by `omega^2/(a^2 + omega^2)`:

      R(theta, omega) = R_b(theta) R0^(cos(theta) omega^2/(a^2 + omega^2)).

  Components below `omega = a` pass the layer and meet the boundary behind it. For a Gaussian pulse of width `w` the part
  of the peak carried by `|omega| < a` is about `a w/(c sqrt(pi))`.
- *Choice of `a`.* It must be small against the lowest frequency that is to be absorbed, and its decay time `1/a` should
  be short against the run so that the memory forgets. Registered: **`a = 0.25/s`** for the layer of ADR 0237 D7 (`c = 1`,
  `L = 0.2`): `a = 0.05 c/L`, a decay time of 20 transit times of the layer; `a w/(c sqrt(pi)) = 0.0056`, times the
  reflection of the Thompson boundary (0.33 at 60 degrees): `1.9e-3` at most; the lowest harmonic of the trains of set-up D
  (`omega = 9.8`) is attenuated with the exponent factor 0.99935. Model M3: the isolated pulse at 60 degrees returns
  `5.2e-4` with `a = 0.25` against `4.6e-4` without; `1.1e-3` with `a = 1`, `4.1e-3` with `a = 2`, `1.4e-2` with `a = 5`.

**DR12. Waves along the layer: what a layer can and cannot do (clause 1).**

(i) *No matched layer damps them.* A field that does not depend on `x` solves the stretched equations for any `alpha(x,
omega)`: the stretching acts on `d/dx` only. In a duct with rigid walls at `y = 0, H` these fields are the transverse
standing waves `cos(n pi y/H)` at the cut-off frequencies `omega_n = n pi c/H`. Near cut-off the group velocity along the
duct, `c k_x/k`, tends to zero and the attenuation of the layer, `exp(-k_x I/omega)`, tends to one. This holds for the
shifted form as well. (Check in M4, set-up D at 45 degrees, the F-331 form: the difference at the probe column is
dominated by the mode `n = 1` with an amplitude of 0.047 of `eps`, a period of 0.91 = `2H/c`, and no decay up to `t = 3`.)

(ii) *A damping along the layer is a mismatch, and its reflection does not depend on the frequency.* Add `-nu(x) m_t` to
the tangential momentum equation (`m_t` the tangential momentum). Then `k_x^2 = alpha^2 (k^2 - k_y^2/(1 + i nu/omega))` and
the impedance in front is no longer met: `Z/Z0 = cos(theta)/sqrt(1 - sin^2(theta)/(1 + i nu/omega))`. For `nu << omega` the
change of `k_x^2` is `alpha^2 k_y^2 i nu/omega`, and the first Born approximation of the reflection of a layer that is thin
against the wavelength is

    R ~ -(tan(theta) sin(theta)/(2 c)) * integral of nu(x) A(x)^2 dx,     A(x)^2 = exp(-2 cos(theta) I(x)/c)

(the factor `omega` cancels: the mismatch is `nu/omega`, the Born integral carries `k`). With `nu = beta sigma` the
integral is `beta c (1 - R0^cos(theta))/(2 cos(theta))`, so `|R| ~ beta tan(theta) sin(theta)/(4 cos(theta))`: `0.75 beta` at
60 degrees, `0.25 beta` at 45 degrees, `0.08 beta` at 30 degrees. M3 (table below; linear in `beta` up to 0.02) gives for
the largest value of the reflected isolated Gaussian pulse `1.3 beta`, `0.34 beta`, `0.10 beta`, and for the trains of
set-up D `0.54 beta`, `0.13 beta`, `0.034 beta`.

(iii) *What the registered window needs.* Set-up D measured 3.8, 6.2 and 8.7 percent from `t_s` = 0.84, 1.05, 1.58 on.
To be below 1 percent from `t_s` on, the ringing has to fall by a factor of 4 to 9 (more with a margin) within about one
time unit, that is one to two periods of the cut-off modes. A damping `nu` that acts on the tangential momentum only
inside the layer damps a standing wave there at the amplitude rate `nu/2`; the field in front of the layer, where the
probe column is, is emptied only by spreading into the layer. Even for a field that sat wholly inside the layer the factor
needs `integral of nu dx >= 2 ln(8) L c/(c t_s) ~ 0.8 c` (mean `nu` about 4/s, `beta ~ 0.12`), and by (ii) that costs 16
percent of an isolated pulse at 60 degrees. The budget of clause 4 (well below 1 percent at 60 degrees) allows `beta <=
0.004`, a mean `nu` of 0.14/s: no effect within the window.

(iv) *Hiding the damping behind the attenuation* (a damping zone at the back of the layer) needs `A^2 << 1` at 60 degrees
where the zone starts; the D7 layer has `A^2 = 1e-3` only at its very end. M3 and M4, below: a zone over the last 20
percent costs 0.4 to 0.6 percent at 60 degrees and leaves set-up D at 5 to 6 percent.

(v) *The memory cannot be started right from the state alone.* The consistent memory is `-rho0` times the velocity
potential, and the scaled tangential velocity contains its time integral; both depend on the history of the wave. A
ramp of the rates in time would start the layer quietly, but the window of set-up D begins `0.1` after the first transit
and leaves no time for a ramp that is slow against the cut-off period.

So: the matched layer plus a damping along it trades the ringing against the oblique reflection, and at the thickness of
D7 no point of the trade meets both 1 percent limits. M4 (set-up D, strict measure over the registered window):

| layer (`a = 0.25` unless said) | 30 deg | 45 deg | 60 deg |
|---|---|---|---|
| F-331 form (`a = 0`); solver measured 0.0385, 0.0618, 0.0870 | 0.034 | 0.053 | 0.072 |
| shift only, `a` = 0.25 / 1 / 5 | 0.033 / - / - | 0.052 / 0.050 / 0.043 | 0.071 / 0.070 / 0.064 |
| `beta` = 0.02 | 0.027 | 0.043 | 0.056 |
| `beta` = 0.05 | 0.019 | 0.032 | 0.036 |
| `beta` = 0.10 | 0.013 | 0.020 | 0.060 |
| `beta` = 0.30 | 0.013 | 0.042 | 0.134 |
| (`a = 0`) damping zone over the last 20 percent, peak 100 / 400 per s | - | 0.051 / 0.050 | 0.063 / 0.057 |
| (`a = 0`) damping zone over the last 50 percent, peak 100 per s | - | 0.029 | 0.047 |
| (`a = 0`) shift in the tangential force only, `a` = 1 / 5 / 20 | - | 0.039 / 0.037 / 0.083 | 0.061 / 0.121 / 0.225 |

At large `beta` the number is the reflection of the train itself (ii), at small `beta` the ringing. The smallest value
of the worst angle is near `beta = 0.05` (3.6 percent). No variant is below 1 percent at any angle.

**DR13. Clause 2 in a run.** The linear system of the F-331 form in a field that does not depend on `x`, for one transverse
wavenumber `k_y`, has the characteristic polynomial `(s + sigma)(s^2 + c^2 k_y^2)`: one decaying root and the two neutral
roots of (i), no growing one and no double root. With a steady source in the pressure equation the tangential momentum
tends to a constant. So in a closed run the F-331 form does not grow without bound either: the fluid moves until the
pressure gradient along the layer is gone. The statement of ADR 0237 ("drives the tangential momentum without bound")
holds for a pressure that is *held*, which a run does not do. The clause is therefore checked twice: Z1, a run over 100
transit times with a bound and a no-growth condition, in which both forms are expected to pass; and F4, the held
pressure, where the forms differ by construction (DR10, DR11). The shifted form in Z1 is expected to keep less of the
offset (the layer's volume at zero frequency, DR11).

**DR14. The conservative form of the source** (as ADR 0237 D1, for the nonlinear solver; `U_ref` at rest, `c_ref^2 =
gamma p_ref/rho_ref`, `Q` the memory of `p - p_ref`, `M` of the momentum, `m` the momentum, `u` the velocity):

    mass:      -(s1 + s2)(rho - rho_ref - a Q/c_ref^2) - s1 s2 (Q - a R)/c_ref^2
    momentum:  -s1 (m.n1 - a M.n1) n1 - s2 (m.n2 - a M.n2) n2 + F
    F        = -s2 (n1.grad Q) n1 - s1 (n2.grad Q) n2 - [(s1 + s2)(e3.grad Q) + s1 s2 (e3.grad R)] e3
               - beta [ s1 (m - (m.n1) n1) + s2 (m - (m.n2) n2) ]
    energy:    -(s1 + s2)(E - E_ref - a Q/(gamma - 1)) - s1 s2 (Q - a R)/(gamma - 1) + u.F
    Q_t = p - p_ref - a Q,   R_t = Q - a R,   M_t = m - a M        (in the cells with s1 + s2 > 0).

For an acoustic perturbation `rho' = p'/c^2` and `E' = p'/(gamma - 1)`, which is where `Q/c_ref^2` and `Q/(gamma - 1)` come
from; a density perturbation without a pressure perturbation relaxes with the plain rate. With one layer (`s2 = 0`) the
force is written `-s1 (grad Q - n1 (n1.grad Q)) - beta s1 (m - (m.n1) n1)`, the statement of ADR 0237 plus the damping,
so that with `a = 0` and `beta = 0` the new form performs the arithmetic of `SpongeForm::Matched` (check B3). `beta` is the
damping along the layer of DR12 (ii): an opt-in with a known price, not part of the recommended configuration.

**DR15. The isolated pulse at 60 degrees at an affordable size.** The half-space set-up of ADR 0237 (X1, X45) is kept:
same cell size, pulse width, layer, probe and measure. Two of its margins are tightened, each with its reason:

- *The tails.* X45 keeps `5 w` (a tail of `e^-25 = 1.4e-11`) between the pulse and the layer at `t = 0` and after the
  reflected peak. `3.5 w` leaves `e^-12.25 = 4.8e-6` of the peak, three orders below the limit. The transverse extent
  follows from `Y (1 - sin(theta)) >= 2 x 3.5 w + (2 L + 0.1025) cos(theta)`: `Y = 4.05` instead of 4.9.
- *The reference grid.* ADR 0237 puts the end of the reference grid as far behind the probe as the run is long. That is
  sufficient, not necessary. The pulse reaches the end `x_e` of the reference grid at the height `y` at the time `x_e cos -
  y sin - xi0`, and whatever the end sends back needs at least the distance to the probe. The minimum over `y` of the
  arrival time is the specular one, `t_1 + (2 (x_e - X_i) + 0.1025) cos(theta)`, `t_1` the time the peak reaches the layer
  front at the wall. For the front of the returning pulse (`3.5 w` ahead of its peak) to arrive after the end of the run,
  `x_e - X_i >= 0.481`; registered `0.6`. The corner of the reference grid at `ymax` is `Y` away, as `ymax` itself.

Size: test `870 x 810` (705 000 cells), reference `950 x 810` (770 000 cells), 4040 steps: about 1.5 million cells instead
of the 3 million of the conservative rule (and of the 2 million that ADR 0237 guessed). The speed of the ADR 0237 runs
(about 0.7 million cell steps per second and thread) gives 70 to 80 minutes with one thread per run. It is run in full:
the clause is not reduced. M4 on the same grids, with the short and with the conservative reference length: the same number in 12 digits (table of M4 below).

## Design arithmetic (the two scratch models)

Both: `rho0 = c = 1`, `dx = 0.005`, Gaussian pulses of width `w = 0.04`, the D7 layer (40 cells, exponent 2, `R0 = 1e-6`).

- **M3 (frequency domain).** M2 of ADR 0237 with `coef = s0 alpha + k_y^2 alpha/(s0 + nu)` at the cell centres and `s0
  alpha` at the faces, `alpha = 1 + sigma/(a + s0)`; end `u = p` (Thompson-like) or `u = 0`; the reflected pulse is the
  inverse transform of `R(k)` times the spectrum, its largest value against the incident peak. Without the new terms it
  returns the numbers of ADR 0237 (`1.0e-6`, `6.4e-6`, `5.7e-5`, `1.0e-3` in front of a rigid end).

  | layer, Thompson-like end | pulse 0 deg | 30 deg | 45 deg | 60 deg | train 30 | 45 | 60 deg |
  |---|---|---|---|---|---|---|---|
  | F-331 form | 5.6e-7 | 1.5e-6 | 1.8e-5 | 4.6e-4 | 1.3e-6 | 1.6e-5 | 4.1e-4 |
  | K1: `a = 0.25` | 5.7e-7 | 8.2e-6 | 3.6e-5 | 5.2e-4 | 1.3e-6 | 1.6e-5 | 4.1e-4 |
  | K1 in front of a rigid end | 9.2e-5 | 9.8e-5 | 1.5e-4 | 1.2e-3 | 5.7e-6 | 5.1e-5 | 8.9e-4 |
  | K2: `a = 0.25`, `beta = 0.05` | 5.7e-7 | 4.6e-3 | 1.5e-2 | 4.9e-2 | 1.7e-3 | 6.5e-3 | 2.7e-2 |
  | `beta` = 0.005 / 0.01 / 0.02 (`a = 0`), pulse at 60 deg | | | | 6.9e-3 / 1.3e-2 / 2.3e-2 | | | |
  | damping zone, last 20 percent, peak 100 / 400; last 10 percent, peak 400 (pulse at 60 deg) | | | | 4.4e-3 / 6.2e-3; 1.7e-3 | | | |
  | shift in the tangential force only, `a` = 0.25 / 1 / 5 (pulse at 60 deg) | | | | 1.3e-2 / 4.4e-2 / 1.4e-1 | | | |

- **M4 (time domain, two dimensions).** Unknowns `p` at the cell centres, `u`, `v` on the faces, `Q`, `R` at the centres,
  the momentum memories on the faces; the equations of DR11 (linear, `rho0 = c = 1`) with `beta sigma` on the tangential
  velocity; rigid walls `u = 0` or `v = 0`; open ends `u = +-p`; `dt = 0.5 dx`. The measure is that of ADR 0237 (largest
  difference to a run on an extended grid against the incident peak at the probes).

  | set-up | M4 | the solver, ADR 0237 |
  |---|---|---|
  | set-up D without a layer, 30 / 45 / 60 deg | 0.0815 / 0.179 / 0.344 | 0.0814 / 0.196 / 0.372 |
  | set-up D, plain form | 0.052 / 0.138 / 0.299 | - / 0.150 / - |
  | set-up D, F-331 form | 0.034 / 0.053 / 0.072 | 0.0385 / 0.0618 / 0.0870 |
  | corner C1 (below): no layer / one layer K1 / two layers K1 | 0.089 / 0.049 / 1.8e-6 | |
  | Z1 (below), largest `rho0 c |m_t|/delta` in the layer per quarter of the run, F-331 form | 3.25, 3.26, 3.07, 3.03 | |
  | Z1, K1 | 3.07, 3.07, 2.98, 2.97 | |
  | X45 of ADR 0237 as registered there, F-331 form / K1 | 1.8e-5 / 2.0e-5 | 1.45e-4 |
  | X60 of DR15, F-331 form / K1 | 4.60e-4 / 4.94e-4 | |
  | X60 with the reference grid of the conservative rule (1680 cells long), F-331 form / K1 | 4.60e-4 / 4.94e-4 (equal to the short reference in 12 digits) | |

  M4 is within 20 percent of the solver where both exist for set-up D. For the small numbers the solver's own scheme
  error dominates (ADR 0237: the solver measured 3 to 100 times the model), so the estimates of the small checks below
  are the ADR 0237 measurements, not the model's.

## Decisions

- **E1. One new variant, the existing ones untouched.** `SpongeForm::MatchedShifted(MatchedOptions)`, `MatchedOptions {
  normal, shift, along, second: Option<SpongeLayer> }`: `normal` the unit normal of the layer whose rates are
  `Sponge::rate` (as `Matched`), `shift` = `a`, `along` = `beta`, `second` a second planar layer whose normal is orthogonal
  to `normal` (its rates are evaluated at the cell centres when the sponge is set). `Plain` and `Matched { normal }` keep
  their code path, statement for statement; the existing tests construct them by name and are not edited.
- **E2. The source** is DR14, added to the residual of every Runge-Kutta stage; the memories `Q`, `R` and `M` advance with
  the stage weights (ADR 0237 D2). `grad Q` and `grad R`: Green-Gauss with the mean on a face between two cells of a
  layer (`s1 + s2 > 0` in both) and the cell's own value elsewhere (ADR 0237 D2).
- **E3. Solver.** `set_sponge` accepts the new variant after validation (unit `normal`, finite `shift >= 0` and `along >=
  0`, a valid second layer with `|n1.n2| <= 1e-12`, a reference at rest). `sponge_memory()` and `set_sponge_memory()` keep
  their meaning (`Q`); new `sponge_memories()` and `set_sponge_memories()` for `Q`, `R`, `M` together (a restart and the
  start of set-up D2 need `M`). `step` branches to the new path only for the new variant. `max_dt` is not changed.
  `step_ale`, `replace_mesh` and a mass-spring-damper wall stay refused with any sponge (ADR 0237 D5, ADR 0243 D6).
- **E4. Helpers.** `Sponge::planar_shifted(geom, reference, &layer, shift, along)` and `Sponge::corner(geom, reference,
  &layer1, &layer2, shift, along)`.
- **E5. Registered configurations**, fixed now from DR11 and DR12, on the D7 layer of ADR 0237 (thickness 0.2, 40 cells,
  exponent 2, `R0 = 1e-6`, reference = the ambient state):
  - **K1 (the configuration of the acceptance): `shift = 0.25/s`, `along = 0`.**
  - **K2 (the trade of DR12, for clause 1 only): `shift = 0.25/s`, `along = 0.05`.**
  Clauses 2 to 5 are decided with K1. Clause 1 is run with K1 and with K2; it counts as met only if K1 meets it (a
  configuration that fails clause 5 does not meet the acceptance).
- **E6. No case-schema wiring** (it goes with F-330; no file of `rcfd-api` is edited). Not on moving meshes.
- **E7. Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation):** the full frequency
  shift on all equations (not the cheaper decay of `Q` in the tangential force only, which M3 shows to reflect `1.3e-2` at
  60 degrees already at `a = 0.25`); the damping along the layer as an option that is off in K1; the corner as the
  product of two stretchings with the one memory `Q` (plus `R` for an edge in three dimensions). No default changes.

Code: `crates/rcfd-fv/src/sponge.rs` (the variant, the options, validation, the source, unit tests),
`crates/rcfd-fv/src/compressible.rs` (the memory fields, the two accessors, the branch of `step` and the new step path),
new `crates/rcfd-fv/tests/sponge_f340.rs`. No existing test file is edited.

## Checks and limits (registered)

Common: as ADR 0237 (`gamma = 1.4`, `rho0 = 1`, `p0 = 1/1.4`, `c = 1`, solver defaults, `eps = 1e-4 p0`, `w = 0.04`, `dx = dy
= 0.005`, the D7 layer), the measure `R` of ADR 0237 (largest difference to a reference run on an extended grid without a
sponge, against the incident peak at the probes, runs in lockstep with the smallest `max_dt()`). The new test file
carries its own copy of the helpers of `sponge_f331.rs` (that file is not edited); its measure steps every run in its own
thread (the same arithmetic).

### Clause 1: the set-up D record (heavy)

Set-up D of ADR 0237 exactly (grids, the train started as simple waves in the whole domain, memory zero, boundaries,
probe column, window `t_s <= t <= T`, measure), with the layer in the new form.

| id | run | limit | estimate (M4, DR12) |
|---|---|---|---|
| D30s, D45s, D60s | set-up D, K1 | `R <= 0.01` each | **0.033, 0.052, 0.071: expected to miss** |
| D30d, D45d, D60d | set-up D, K2 | `R <= 0.01` each | **0.019, 0.032, 0.036: expected to miss** |

Also printed, not limited: the largest difference over the last 0.2 of the window (M4: K1 0.030, 0.047, 0.031; K2 0.018,
0.021, 0.027).

### Clause 2: a pressure offset with a gradient along the layer

**Z1 (run, heavy by its length).** `box_2d(100, 64, 0.5, 0.32)`, all four boundaries `Slip` (a closed box, so that only
the layer can change the offset), layer `[0.3, 0.5]` with the normal `+x`, reference = ambient. Start at rest with `p - p0 =
delta (1 + 0.5 cos(pi y/H))`, `delta = eps`, `H = 0.32`, isentropic density, memories zero. Run until `t >= 20` (100 transit
times `L/c`). `B_k` = the largest `c |m_t|/delta` (`m_t` the momentum along `y`) over the cells of the layer and the steps
of the quarter `k` of the run.

| id | run | limit | estimate |
|---|---|---|---|
| Z1 | K1 | `max_k B_k <= 6` and `B_4 <= 1.05 B_1` | M4: 3.07, 3.07, 2.98, 2.97 |
| Z1o | the F-331 form (`Matched`), reported; the same two limits, **expected to hold as well** (DR13) | as Z1 | M4: 3.25, 3.26, 3.07, 3.03 |

Also printed: the mean of `p - p0` in the box at the end against `delta` (DR11: K1 `1/(1 + 13.8/0.25) = 0.018` at
equilibrium, M4 0.008 at `t = 20`; F-331 form 0).

**F4 (the held pressure, unit test of the source).** Uniform grid, rate 3 in every cell, layer normal `x`, the state held
at `p - p_ref = d (1 + g y)`, at rest; the memory is advanced with the returned `Q_t` by 20 000 explicit Euler steps of
0.001 (to `t = 20`). K1 (`a = 0.25`): `Q a/p'` against `1 - exp(-a t)` within `1e-3` in every cell, and the tangential
force of the interior cells against `-(rate/a) d g (1 - exp(-a t))` within `1e-3` (bounded). Control, the F-331 form: `Q =
p' t` within `1e-9` relative (it grows linearly, the force with it).

### Clause 3: the corner (C1; a few minutes)

Test `box_2d(160, 160, 0.8, 0.8)`, reference `box_2d(250, 250, 1.25, 1.25)`. `xmin`, `ymin` `Slip` (two mirror planes),
`xmax`, `ymax` Thompson. Start at rest with `p - p0 = eps exp(-(x^2 + y^2)/w^2)` (a quarter of a cylindrical pulse about the
corner of the two mirror planes), isentropic density. Two D7 layers, fronts at `x = 0.6` and `y = 0.6`, overlapping in
`[0.6, 0.8]^2`. Probes: the cells with the centres `(0.4975, 0.4975)` (on the diagonal, in front of the corner), `(0.4975,
0.3975)`, `(0.3975, 0.4975)`, `(0.4975, 0.0125)`. Run until `t >= 1.8`, window: all steps. `R` = the largest of the four
ratios (each against the incident peak at its own probe). Timing: the pulse reaches the corner of the fronts at `t =
0.85`, the far corner at 1.13; the return from there passes the diagonal probe at 1.56 plus the tail; the reference's ends
return to a probe after 2.0. The waves meet the layers at every angle from 0 to about 50 degrees and the corner region
along the diagonal.

| id | run | limit | estimate |
|---|---|---|---|
| C1 | two layers, K1 (`Sponge::corner`) | `R <= 0.01` | M4 `1.8e-6`; the solver's scheme, as X45: `1e-4` to `1e-3` |
| NCC1 | no sponge | `R >= 0.04` | M4 0.089 |
| NCC2 | one layer K1 at `x = 0.6` only | `R >= 0.02` | M4 0.049 (the Thompson boundary at `ymax` reflects) |

### Clause 4: the isolated pulse at 60 degrees (X60; heavy, about 75 minutes)

DR15. Test `box_2d(870, 810, 4.35, 4.05)`, reference `box_2d(950, 810, 4.75, 4.05)`; `ymin` `Slip`, `ymax` and `xmin`
`Extrapolate`, `xmax` Thompson; initial field `eps [g(x cos + y sin - xi0) + g(x cos - y sin - xi0)]` as simple waves, `xi0 =
-1.5724` (`= 4.15 cos - 4.05 sin - 0.14`); layer front `X_i = 4.15`, memories zero; probe: the cell with the centre
`(4.0475, 0.0025)`; run until `t >= 4.04`, window: all steps. Timing: the peak reaches the layer front at the wall at `t_1 =
3.6474`; the reflection from the end of the layer with a tail of `3.5 w` has passed the probe at 4.0387; at `t = 0` the
pulse is `3.5 w` in front of the layer at `y = 4.05`; `ymax` and `xmin` are 4.0475 from the probe; the front of what the
reference's end returns reaches the probe at 4.158.

| id | run | limit | estimate |
|---|---|---|---|
| X60 | K1 | `R <= 0.01` | M3 `5.2e-4`; the solver measured `1.24e-3` for the train at 60 degrees (M3 `4.1e-4`): `1e-3` to `3e-3` |
| X60o | the F-331 form in the same run (ADR 0237 question 2), reported with the same limit | `R <= 0.01` | M3 `4.6e-4`: `1e-3` to `3e-3` |

### Clause 5: the checks of ADR 0237 with K1

| id | run | limit | estimate |
|---|---|---|---|
| A0c | the tube of A0, K1, Thompson behind; A0sc with `Slip` behind | `R <= 0.01` each | ADR 0237 measured `9.2e-7` and `1.1e-6`; M3 with the shift `5.7e-7` and `9.2e-5` (the part below `omega = a` returns from the wall) |
| A30c, A45c, A60c | set-up D2 of ADR 0237 amendment 1 exactly, K1. The start is the state and the `Q` of DR8 for the unshifted layer, and `M = n cos(theta) Q/c` (the time integral of the normal momentum of DR8) | `R <= 0.01` each | ADR 0237 measured `1.2e-4`, `2.8e-4`, `1.2e-3`; the start is off by `a/omega <= 2.6` percent of the memory, which adds about 2.6 percent of the set-up D values: `1e-3` to `2e-3`. Expected below `4e-3` |
| X1c | X1 of ADR 0237, K1 | `R <= 0.01` | measured `2.4e-5` there; M3 with the shift 5 times its value without: below `2e-4` |
| X45c | X45 of ADR 0237, K1 | `R <= 0.01` | measured `1.45e-4` there; M3 twice: below `5e-4` |
| NCB60 | set-up D2 at 60 degrees, K2 (the price of the damping along the layer) | `R >= 0.01` | M3 `2.7e-2` |

### U, F, S, B: algebra, refusals, bit identity (fast)

| id | check | limit |
|---|---|---|
| S3 | refusals of the new variant: `shift` and `along` negative, NaN, infinite; `normal` not a unit vector; a second layer with a zero normal, a non-positive thickness or a normal not orthogonal to `normal`; a reference with a velocity; `set_sponge_memories` with a wrong length or a NaN; `step_ale`, `replace_mesh` and `set_msd_wall` with the new variant set; clearing accepted | error / ok |
| F2 | DR14 with one layer on a prescribed field (the set-up of F1 of ADR 0237: rate 3, `Q = b y`, a state with a velocity along and across the layer), `a = 0.25`, `beta = 0.05`, `M` given: the five sources and `Q_t`, `R_t`, `M_t` of the interior cells against DR14 | `1e-12` relative each |
| F3 | DR14 with two layers on `box_mesh` in three dimensions (uniform rates 3 and 2, normals `x` and `y`, `Q` and `R` linear in `x`, `y`, `z`, `a = 0.25`, `beta = 0.05`): the five sources of the interior cells | `1e-12` relative each |
| F4 | above (clause 2) | `1e-3`; control `1e-9` |
| U3 | U1m of ADR 0237 with two layers K1 (fronts at `x = 0.5` and `y = 0.25` of the `1 x 0.5` box, `rate_max = 50`), 200 steps: drift of the state; the memories | `1e-12`; exactly zero |
| U4 | U2 of ADR 0237 with K-like values (`a = 0.5`, rate 2 in every cell, one layer), until `t >= 1`: with `z0 = p'(0)`, `q = sigma + a`: `p'(t) = z0 (a + sigma e^{-q t})/q`, `Q(t) = z0 (1 - e^{-q t})/q`, `rho'(t) = rho'(0) e^{-sigma t} + (sigma a z0/(c_ref^2 q)) [(1 - e^{-sigma t})/sigma - (e^{-sigma t} - e^{-q t})/a]`; momentum | relative `1e-5` (pressure, density), `1e-4` (`Q`); `1e-12` |
| B3 | bit identity. (i) the existing tests pass unedited: `--lib compressible`, `--lib sponge`, `--test compressible_bc`, `--test compressible_ale_f258`, `--test compressible_ale_f258_records`, `--test impedance_f243`, `--test sponge_f331` (fast), `--test msd_wall_f332` (fast). (ii) new: the set-up of B2 of ADR 0237 (30 steps, the D7 layer on the last 40 columns): `MatchedShifted` with `shift = 0`, `along = 0`, no second layer gives the bits of `Matched` in every cell and the same `Q`; with K1 the state differs (control) | pass; bit-identical; differs |

Estimates: F round-off; U4 as U2 of ADR 0237 (`6.8e-7`, `3.3e-5`); B3 (ii) by construction of DR14 (the terms with `a` and
`beta` multiply by zero and add nothing).

## Mutations (registered; scratch copies, not committed)

Each is applied to a copy of `sponge.rs` or `compressible.rs` in the scratch folder with its own target directory; then
`cargo test --release -p rcfd-fv --lib sponge` and `cargo test --release -p rcfd-fv --test sponge_f340` (the fast tests)
are run once; MV4 and MV5 are run against C1 as well.

| id | mutation | expected to be caught by |
|---|---|---|
| MV1 | the memory without decay (`Q_t = p'`) | F2, F4, U4 |
| MV2 | mass and energy relax with `p'` instead of `p' - a Q` (the shift in the force only) | F2, U4 |
| MV3 | the momentum memory not advanced (`M_t = 0`) | F2 |
| MV4 | the product term `s1 s2 Q` dropped | F3; C1 (reported) |
| MV5 | the cross forces exchanged (`s1` on the gradient along `n1`) | F3; C1 (expected to fail) |
| MV6 | the damping along the layer applied to the normal momentum as well | F2 |
| MV7 | `R` not advanced (`R_t = 0`) | F2 (it compares the returned `R_t`; F3 takes `R` as given and does not see it) |
| MV8 | the single-layer force of the new variant written without the projection (`-s1 grad Q`) | F2, B3 |

A mutation that no check catches is reported as surviving.

## Decision rule (registered)

- **done** (pending acceptance by the maintainer) only if all five clauses are met with K1: D30s, D45s, D60s; Z1 and F4;
  C1; X60; A0c, A30c, A45c, A60c, X1c, X45c; and B3, S3, F2, F3, U3, U4 hold, the controls NCC1, NCC2 hold and every
  mutation is caught.
- **partial** if the shifted form is built, B3 holds, and at least clauses 2 to 5 are met with K1 while clause 1 is
  missed (the outcome this registration expects), or if one of the heavy clauses was not run. The miss is named with its
  measured values and the diagnosis of DR12; the tests D..s and D..d become ignored `*_known_miss` records.
- **todo** stays if the shifted form is not usable (unstable in A0c or Z1), if B3 fails, or if clause 5 is lost.
- Z1o, X60o, NCB60 and the "last 0.2 of the window" numbers do not decide; a miss of a control is kept as a
  `*_known_miss` test and named.
- If K1 misses clause 1 and K2 meets it, the acceptance is still not met (K2 fails clause 5 by DR12); it is reported.

## Limits

- Linear acoustics about a state at rest; one planar layer or two with orthogonal normals; the edge of three layers (a
  corner in three dimensions) is not built. The three-dimensional edge of two layers is built (DR9, `R`) and checked by
  the algebra of F3 only, not by a run.
- The shifted layer does not absorb below `omega ~ a` and does not hold a static pressure at the reference; the plain
  form or the boundary behind the layer has to do that.
- Waves along the layer stay undamped in K1 (DR12); the damping along the layer costs `1.3 beta` of an isolated pulse at
  60 degrees.
- Explicit source (`rate dt` well below 2.5; `max_dt` does not know it); not on moving meshes; library only.
- Cartesian grids; verification against reference runs and closed forms; no measurement is compared.

## Sources

- J.-P. Berenger, J. Comput. Phys. 114 (1994) 185-200 (the layer and its corners, split form); IEEE Microwave Wireless
  Compon. Lett. 12 (2002) 218-220 (the frequency-shifted layer for evanescent waves in waveguides).
- W. C. Chew, W. H. Weedon, Microwave Opt. Technol. Lett. 7 (1994) 599-604 (the layer as a complex coordinate stretching).
- M. Kuzuoglu, R. Mittra, IEEE Microwave Guided Wave Lett. 6 (1996) 447-449 (the complex frequency shift).
- J. A. Roden, S. D. Gedney, Microwave Opt. Technol. Lett. 27 (2000) 334-339 (the shifted layer with memory variables).
- F. Q. Hu, J. Comput. Phys. 173 (2001) 455-480 (unsplit layer for the Euler equations).
- E. Becache, S. Fauqueux, P. Joly, J. Comput. Phys. 188 (2003) 399-433 (stability).
- K. C. Meza-Fajardo, A. S. Papageorgiou, Bull. Seismol. Soc. Am. 98 (2008) 1811-1836 (a damping along the layer, the
  "multiaxial" layer; that it is not matched is DR12 (ii)).

All are cited from memory of their content; none was opened for this job and no equation number is quoted. Every formula
used is derived above from the linear acoustic equations. No vendor code or documentation was used.

## Open questions (with recommendations)

- Q-A. Whether clause 1 is kept as written. DR12 says it cannot be met at this thickness together with clauses 4 and 5.
  Options after the results: (a) keep it and leave the item partial; (b) replace it by what a layer can do (the D2 start,
  or a duct whose transverse modes are damped by its own walls or by a plain zone); (c) a thicker layer with a damping
  zone behind an attenuation of `1e-3` at 60 degrees. Recommendation in the addendum, with the measured numbers.
- Q-B. A default for `shift` when the layer is wired to a case file (F-330). Not taken here: it would be a default.
  Recommendation: `0.05 c/L`, to be confirmed with the results.

## Addendum 1: results (2026-11-12; the registered text above is unchanged)

Commits: registration 4ee9cd0a; library and tests 3760d184; this addendum with the three `*_known_miss` records and
`docs/acoustics.md`. Every run was made in the **release** profile, each registered heavy run once. No limit was changed
and no amendment was needed. The three checks of clause 1 missed their limit at all three angles, as the registration
expected (DR12); they are kept as ignored `*_known_miss` records with the limit intact.

### Checks

| id | check | limit | measured | met |
|---|---|---|---|---|
| **Clause 1** | | | | |
| D30s, D45s, D60s | set-up D of ADR 0237 (train started inside the layer, memory zero), K1 | `R <= 1e-2` each | `3.73e-2`, `6.09e-2`, `8.64e-2` (M4 expected 0.033, 0.052, 0.071; the F-331 form measured 0.0385, 0.0618, 0.0870) | **no (known miss)** |
| D30d, D45d, D60d | the same, K2 (`along = 0.05`) | `R <= 1e-2` each | `2.24e-2`, `3.62e-2`, `4.21e-2` (M4 expected 0.019, 0.032, 0.036) | **no (known miss)** |
| (printed) | the last 0.2 of the window: K1; K2 | none | `3.6e-2`, `5.7e-2`, `4.0e-2`; `2.0e-2`, `2.3e-2`, `3.0e-2` | - |
| **Clause 2** | | | | |
| Z1 | closed box, offset `delta (1 + 0.5 cos(pi y/H))`, 100 transit times, K1: largest `c |m_t|/delta` in the layer per quarter | largest `<= 6`; last `<= 1.05` first | 3.00, 2.98, 2.81, 2.72 (M4 3.07, 3.07, 2.98, 2.97); mean offset left 0.0088 (M4 0.008) | yes |
| Z1o | the same, the F-331 form (reported; expected to hold) | as Z1 | 3.20, 3.12, 2.87, 2.77; mean offset left `-4e-6` | held |
| F4 | held pressure, K1: memory against `(p'/a)(1 - e^{-a t})` at `t = 20`; force along the layer against its bound; control, F-331 form: memory against `p' t` | `1e-3`; `1e-3`; `1e-9` | `4.2e-6`; `4.2e-6`; `4.7e-13` | yes |
| **Clause 3** | | | | |
| C1 | quarter of a cylindrical pulse through two layers and their corner, K1 | `R <= 1e-2` | `5.30e-5` (M4 `1.8e-6`; the solver's scheme as expected) | yes |
| NCC1 | no sponge | `R >= 0.04` | 0.0577 | held |
| NCC2 | one layer only | `R >= 0.02` | 0.0287 | held |
| **Clause 4** | | | | |
| X60 | one isolated plane pulse at 60 degrees, K1 | `R <= 1e-2` | `8.13e-4` (M3 `5.2e-4`, M4 `4.9e-4`) | yes |
| X60o | the same run, the F-331 form (reported) | `R <= 1e-2` | `8.02e-4` (M4 `4.6e-4`) | held |
| **Clause 5** | | | | |
| A0c | tube, K1, Thompson behind | `R <= 1e-2` | `9.37e-7` (F-331 form `9.23e-7`) | yes |
| A0sc | tube, K1, rigid wall behind | `R <= 1e-2` | `1.18e-6` (F-331 form `1.08e-6`) | yes |
| A30c, A45c, A60c | set-up D2, K1 | `R <= 1e-2` each | `3.12e-4`, `3.65e-4`, `1.28e-3` (F-331 form `1.16e-4`, `2.78e-4`, `1.24e-3`) | yes |
| X1c | isolated pulse at 30 degrees, K1 | `R <= 1e-2` | `2.29e-5` (F-331 form `2.37e-5`) | yes |
| X45c | isolated pulse at 45 degrees, K1 | `R <= 1e-2` | `1.43e-4` (F-331 form `1.45e-4`) | yes |
| NCB60 | set-up D2 at 60 degrees, K2: the price of the damping along the layer | `R >= 1e-2` | `4.43e-2` (M3 `2.7e-2`) | held |
| **Algebra, refusals, bits** | | | | |
| S3 | refusals of the new variant (options, second layer, reference, memories, `step_ale`, `replace_mesh`, `set_msd_wall`); clearing accepted | error / ok | as required | yes |
| F2 | DR14 with one layer on a prescribed field, 36 interior cells | `1e-12` | `1.58e-15` | yes |
| F3 | DR14 with two layers in three dimensions, 27 interior cells | `1e-12` | `1.43e-15` | yes |
| U3 | uniform reference in two layers and their corner, 200 steps: drift; memories | `1e-12`; zero | 0; 0 | yes |
| U4 | uniform offset, rate 2, shift 0.5: pressure, density; memory; momentum | `1e-5`; `1e-4`; `1e-12` | `4.1e-7`, `3.6e-7`; `1.5e-7`; 0 | yes |
| B3 (i) | existing tests unedited: `--lib compressible` 17 passed, 1 ignored; `--lib sponge` (S1, S2, F1 of ADR 0237 among 7); `compressible_bc` 2 passed, 4 ignored; `compressible_ale_f258` 8; `compressible_ale_f258_records` 4 (the bit records); `impedance_f243` 7 passed, 2 ignored; `sponge_f331` 5 passed, 8 ignored; `msd_wall_f332` 7 passed, 11 ignored | pass | pass | yes |
| B3 (ii) | `MatchedShifted` without shift, damping and second layer against `Matched`, 1200 cells, 30 steps, state and memory | bit-identical | bit-identical; K1 changes 1200 of 1200 cells | yes |

Commands: `cargo test --release -p rcfd-fv --lib sponge` (F2, F3, F4, the layer part of S3; with S1, S2, F1 of ADR 0237);
`cargo test --release -p rcfd-fv --test sponge_f340` (A0c, U3, U4, S3, B3; about 2 s); the heavy ones from the test
binary with `--ignored`, in two batches at the same time (`x60 x45c x1c`: 5079 s, one thread per run; the nine others
in 1060 s with three test threads).

What the numbers say. The frequency shift costs nothing measurable at the registered value: every check of ADR 0237
is repeated within a factor of three of its old value (the trains at 30 and 45 degrees rise from `1.2e-4` and `2.8e-4` to
`3.1e-4` and `3.6e-4`: the start of set-up D2 is the steady state of the unshifted layer, off by `a/omega`, as
registered). Two layers and their corner return `5e-5` of a cylindrical pulse where the Thompson boundaries return 5.8
percent and one layer 2.9 percent. The model M4 predicted set-up D within 20 percent again, for K1 and for K2.

### Clause 1: the miss and its diagnosis

The diagnosis is DR12, written before the runs; the runs agree with it in each point that they test:

- The shift alone changes nothing in set-up D (`3.73e-2`, `6.09e-2`, `8.64e-2` against `3.85e-2`, `6.18e-2`, `8.70e-2` of
  the F-331 form). The ringing is not a matter of the memory that never decays; it sits at the cut-off frequencies of the
  duct, where a field that does not vary across the layer solves the equations of every matched layer.
- The damping along the layer lowers it (K2: `2.2e-2`, `3.6e-2`, `4.2e-2`) and does not reach the limit, and it has the
  registered price: the steady train at 60 degrees returns `4.4e-2` with it (NCB60) against `1.3e-3` without. At 60
  degrees the K2 value of set-up D is that price, not the ringing.
- No value of `along` meets 1 percent at the three angles in M4 (table of DR12), and M4 has now predicted five set-up D
  runs of the solver within 20 percent.

This is a limit of a matched layer of this thickness in a rigid duct, not a defect of the set-up and not a parameter
that was chosen badly. The clause as written is not met. Nothing was tuned after the run.

### Clause 2: what was shown

Z1 holds, and so does the F-331 form in the same run (Z1o), as DR13 expected: in a run the fluid moves until the
pressure gradient along the layer is gone, and neither form grows. The difference the shift makes is in F4 (under a held
pressure the memory and the force saturate at `p'/a`; without the shift they grow linearly) and in what happens to the
uniform part of the offset: the F-331 form removes it, the shifted form leaves `0.9` percent at `t = 20` because the layer
then acts as a large volume at zero frequency (DR11). The clause is met; the statement of ADR 0237 that the F-331 form
"drives the tangential momentum without bound" is true for a held pressure only.

### Mutations (a `git archive` copy in the scratch folder with its own target directory; the registered commands, once each)

| id | mutation | caught by |
|---|---|---|
| MV1 | memory without decay | F2 (`1.3`), F3 (`41`), F4 (`4.0`), U4 (`0.13`, `0.15`, `0.38`) |
| MV2 | mass and energy relax with `p'` | F2 (`13`), F3 (`0.25`), U4 (`0.50`, `0.59`) |
| MV3 | momentum memory not advanced | F2 (`1.0`) only; A0c rises to `4.7e-3` and stays below its limit |
| MV4 | product term dropped | F3 (`1.1`); C1: `R = 2.2e-2` (limit `1e-2`) |
| MV5 | cross forces exchanged | F3 (`0.35`); C1: `R = 0.26` |
| MV6 | damping along the layer on the normal momentum as well | F2 (`7.0e-2`), F3 (`2.0e-2`) |
| MV7 | `R` not advanced | F2 (`1.0`), F3 (`1.0`: it compares the returned `R_t` as well, other than registered) |
| MV8 | single-layer force without the projection | B3 (bits differ), A0c (`R = 0.21`). **Not by F2, other than registered**: the gradient of the prescribed `Q` of F2 has no component along the normal |

No mutation survived. MV3 is caught by one check. The unmutated copy was run first with the same commands (all pass,
the numbers of the table). After the mutations the copy equals the committed file (`cmp`), and the worktree was never
mutated.

### fmt, clippy, other crates

`cargo fmt -p rcfd-fv --check`: clean. `cargo clippy --release -p rcfd-fv --all-targets -- -D warnings`: clean (one
`too_many_arguments` allowance on the new source routine and one on a test helper). `cargo check -p rcfd-api --tests`:
compiles; its tests were not run.

### Deviations and disclosures

- **A model search before the registration.** The layer variants and the two parameter values were chosen with the
  scratch models M3 and M4 before the registration, as its disclosure says; no solver run preceded it.
- **A misstatement in the registration.** DR13 and the Z1 table call the measured quantity `rho0 c |m_t|/delta` once and
  `c |m_t|/delta` once; `m_t` is the momentum, so the second is right and is what the test computes.
- **A0sc.** The registration's estimate for the rigid wall behind the shifted layer was `9.2e-5` (M3, the part below
  `omega = a` returning); measured `1.2e-6`. The run ends at `t = 1.6`; the slow part has not returned by then. The
  estimate was for a longer window than the check has.
- **Fast checks run more than once.** The unit tests of `--lib sponge` ran a second time inside B3 (i), and the fast
  checks ran once more, unmutated, on the mutation copy, and the fast tests of `sponge_f340` once more on the final tree
  (after the three clause 1 tests were renamed to records); every number was reproduced to the digits shown.
- **The first start of the mutation runs applied nothing**: the scratch script had a syntax error and stopped before
  any cargo command. It was corrected, each mutation was applied once to check that it changes the file, and the runs
  were then made. No file of the repository was affected.
- **The reduced margins of X60** (tails of 3.5 widths, the reference grid 0.6 behind the layer front) are DR15,
  registered beforehand; M4 gives the same number with the conservative reference length.
- F2, F3, F4 and the layer part of S3 are unit tests in `sponge.rs` (a second test module; the module of ADR 0237 is
  unchanged); the rest is in `sponge_f340.rs`. No existing test file was edited.
- The papers are cited from memory; none was opened. Every formula used is derived in this ADR.

### Not run

The whole workspace test suite; the tests of `rcfd-api` and of every other crate; the debug profile; a corner in three
dimensions as a run (F3 checks its algebra); three layers; angles above 60 degrees; a mean flow; a layer on a
non-Cartesian grid; values of `shift` other than 0.25 in a run of the solver (the cost at other values is M3's).

### Decision by the registered rule

- Clauses 2, 3, 4 and 5 are met with K1 (the isolated pulse at 60 degrees `8.1e-4`, in full size); B3, S3, F2, F3, U3, U4 hold; NCC1, NCC2 and NCB60 hold; all
  eight mutations are caught.
- Clause 1 is missed with K1 and with K2, with the diagnosis of DR12.
- **Proposed status: partial**, by the second bullet of the rule (the outcome the registration expected).

### Remaining questions (options and recommendation)

1. **Clause 1.** (a) keep it and leave F-340 partial for good; (b) close F-340 on clauses 2 to 5 and replace clause 1 by
   a follow-up that asks for what a layer can do: a zone of plain damping behind a matched layer strong enough to hide
   it (an attenuation of `1e-3` at 60 degrees before the zone starts, which needs about twice the rate integral of the
   D7 layer, so about 80 cells), with the set-up D record as its check over a window of several cut-off periods instead
   of the one that starts 0.1 after the first transit; (c) drop the clause: start with a quiet layer or set the
   memories. Recommendation: (b), and only if a ducted case shows the ringing in practice; until then (c) is what
   `docs/acoustics.md` tells the user.
2. **A default for `shift` in the case-file wiring (F-330).** (a) zero (the form of ADR 0237); (b) `0.05 c/L`.
   Recommendation: (b): it cost nothing in any check and bounds the memory. Not taken here: it is a default.
3. **Whether `along` should stay in the library.** It does not meet clause 1 and costs 4 percent at 60 degrees at 0.05.
   (a) keep it as a documented option; (b) remove it. Recommendation: (a): it is the cheap remedy for a duct at near
   normal incidence (its price falls as `tan sin/cos`), and its price is measured.
4. **The corner in three dimensions.** Built and checked by algebra only. Recommendation: a run when a case needs it.

### For the lead: roadmap entries

**F-340, proposed status: `partial`.** Proposed note text:

> From ADR 0237 questions 2 and 3, recommendation taken on the maintainer's standing instruction of 2026-11-11. The job of 2026-11-12 (ADR 0248): four of the five clauses are MET IN THE LIBRARY, clause 1 is MISSED with a diagnosis that the registration gave before the runs. Built (rcfd-fv sponge.rs, opt-in, `Plain` and `Matched` untouched): `SpongeForm::MatchedShifted(MatchedOptions { normal, shift, along, second })`, the matched layer as a complex coordinate stretching with (i) a complex frequency shift `shift` (the memories decay: under a held pressure the memory and the force along the layer saturate at p'/a instead of growing; below omega ~ a the layer does not absorb and it leaves a static offset alone), (ii) a second planar layer with an orthogonal normal for a corner (`Sponge::corner`: the same memory, one product term, the force of each layer on the momentum along the other's normal; a second memory for an edge in three dimensions), (iii) a damping of the momentum along the layer `along` (a mismatch with a measured price); `Sponge::planar_shifted`, `CompressibleSolver::sponge_memories` and `set_sponge_memories`. Measured with shift 0.25/s (0.05 c/L) and along 0 on the 40-cell layer of ADR 0237 (limit 1 percent, largest difference to a run on an extended grid against the incident peak): an isolated plane pulse at 60 degrees 8.1e-4 (the form of ADR 0237 in the same run 8.0e-4; 705 000 and 770 000 cells, the set-up of X45 with tails of 3.5 pulse widths and a reference grid shortened by a causality argument that a model confirmed to 12 digits); a quarter of a cylindrical pulse through two layers and their corner 5.3e-5 (no sponge 0.058, one layer 0.029); the checks of ADR 0237 repeated with the shift: A0 9.4e-7 (1.2e-6 in front of a rigid wall), A30b, A45b, A60b 3.1e-4, 3.6e-4, 1.3e-3, X1 2.3e-5, X45 1.4e-4; a closed box with a pressure offset varying along the layer over 100 transit times: momentum along the layer at most 3.0 delta/c and not growing (the form of ADR 0237 in the same run 3.2 and not growing either: in a run neither form grows, the difference is under a held pressure, checked by algebra to 4e-6). Without the options the new form produces the bits of `Matched`; the existing compressible, F-258, ADR 0233, ADR 0237 and ADR 0243 tests pass unedited. Eight mutations caught. MISSED, clause 1: the set-up D record (a wave train started inside the layer with zero memory in a rigid duct) stays at 3.7e-2, 6.1e-2, 8.6e-2 at 30, 45, 60 degrees with the shift (3.8e-2, 6.2e-2, 8.7e-2 without) and at 2.2e-2, 3.6e-2, 4.2e-2 with along = 0.05, against 1e-2; kept as three ignored known_miss records. Diagnosis (ADR 0248 DR12, a scratch model predicted the five numbers within 20 percent before the runs): the ringing sits at the cut-off frequencies of the duct; a field that does not vary across the layer solves the equations of every matched layer, so no matched layer damps it; a damping along the layer is a mismatch that reflects about 1.3 along of a pulse at 60 degrees whatever its frequency (along = 0.05 measured 4.4e-2 of the steady train at 60 degrees), and the strength needed to remove the ringing within the registered window costs far more than 1 percent. Not a parameter that could be tuned at this thickness. Limits: linear acoustics about a state at rest; one layer or two orthogonal ones (no corner of three layers; the three-dimensional edge is checked by algebra only); explicit source; not on moving meshes; not with a mass-spring-damper wall; library only (the wiring goes with F-330); Cartesian grids. Open questions in ADR 0248 addendum 1 (what to do with clause 1; a default for the shift in the wiring; whether to keep `along`; a corner run in three dimensions).

Proposed follow-up item (number for the lead to assign), only if question 1 is answered with (b):

- **Matched sponge layer: ring-down of a rigid duct.** Deps: F-340. Size M. Acceptance: "With a zone of plain damping
  behind a matched layer, the set-up D record of ADR 0237 (a wave train started inside the layer with zero memory in a
  rigid duct) falls below 1 percent within 5 periods of the lowest transverse mode at 30, 45 and 60 degrees; an isolated
  plane pulse at 60 degrees and the trains of set-up D2 still reflect less than 1 percent; the thickness and the rates
  are fixed from a derivation before the runs."
