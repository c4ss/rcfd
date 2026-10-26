# ADR 0120: Wave 46C, rotor dynamics of a small fan rotor on two bearings (fan breakdown E1, E2, the E3 interface and an E4 hook)

Status: **ACCEPTED (registered 2026-10-26)**. The limits and decision rules below were fixed before any registered test ran. The
house rules of ADRs 0110-0117 apply:
* a missed limit becomes an ignored `*_known_miss` test with the limit intact;
* a defective set-up gets a new corrected test, registered here as an amendment before it runs;
* defaults stay bit-identical, and `crates/rcfd-api/tests/baseline_hashes.rs` is not re-recorded.

Written at main 8bd609a (worktree `<scratch dir>`, branch `w46c`, target `<target dir>`). Clean-room: first
principles (rigid-body mechanics, harmonic analysis) and our own ADRs only. Parent plan: [fan-project-breakdown.md](../fan-project-breakdown.md)
parts E1, E2, E3 (interface only) and E4 (F-237, [ADR 0051](0051-wave27c-viscoelastic-structural-export.md)).

## What I already knew when registering (disclosed)

* The code was written before this registration, because the rule-d smoke needs it:
  * `crates/rcfd-post/src/rotordyn.rs` (new module);
  * `FORCE` and `MOMENT` cards in `crates/rcfd-io/src/nastran.rs` (`Deck::forces`, `Deck::moments`), with `..Default::default()`
    added to the two existing `Deck` literals (`rcfd-io/src/registry.rs`, `rcfd-io/tests/w27c_registry.rs`);
  * the test file `crates/rcfd-post/tests/w46c_rotordyn.rs`.
* One thing was run before registration: the ignored `smoke_w46c`, which only constructs the six set-ups and reports root counts,
  critical-speed counts and whether the response solve returns `Ok` (results below). It prints no comparison with any closed form.
* No registered test (J1-J4, R1, R2, E, B, U, X, I, W) was run before this ADR was committed.
* Linear algebra already in the workspace (searched by name and by `dense`, `LU`, `direct`, `complex`):
  * `rcfd_reactor::ode::lu_factor` / `lu_solve` (public, real, dense, `crates/rcfd-reactor/src/ode.rs` 121, 155);
  * private real dense LU copies in `rcfd-api/src/design/adjoint.rs` 310 and `rcfd-linalg/src/amg.rs` 61 (`DenseLu`, the AMG coarse
    solve);
  * sparse real CSR, Krylov and AMG in `rcfd-linalg`;
  * no complex number type anywhere.

## Context

The fan plan needs the vibration side (bearing loads and housing-mount loads from imbalance and blade-passing forces) without a
structural solver: E5 (blade and housing FEM, FSI) is deferred by decision (F-139, F-242). A computer-fan rotor (40 to 140 mm,
1 000 to 5 000 rpm) is a short hub with a magnet ring on a shaft in two small bearings; its own bending frequencies are far above
the running range (assumed here, not checked), so a rigid rotor on flexible, damped bearings is the standard first model.

## Decisions

* **D1, where it lives.** A new module `rcfd_post::rotordyn`. `rcfd-post` already holds the F-237 load export
  (`rcfd_post::structural`) and is the one crate that sees both `rcfd_io::nastran::Deck` and post-processing; `rcfd-core` cannot see
  `Deck`. No new crate, no new dependency.
* **D2, model and solver.**
  * Rigid rotor: mass `m`, transverse inertia `It` about the centre of mass, polar inertia `Ip`, constant spin `Omega` about `z`.
    Two **isotropic** bearings at axial positions `z_j` from the centre of mass: radial stiffness `k_j`, viscous damping `c_j`,
    optional hysteretic loss factor `eta_j`. Rigid, grounded housing. Four lateral degrees of freedom, small motions.
  * With isotropic bearings the equations are written in complex coordinates `w = x + i y`, `psi = theta_y - i theta_x`. A
    harmonic `exp(i lam t)` gives a 2x2 complex system `D(lam) [W, Psi] = [F, M]`:

    ```text
    D11 = kt - m lam^2        D12 = D21 = kc        D22 = kr - It lam^2 + Ip Omega lam
    kt = sum d_j,  kc = sum z_j d_j,  kr = sum z_j^2 d_j,  d_j(lam) = k_j (1 + i eta_j sgn(lam)) + i lam c_j
    ```

    `lam > 0` is forward whirl, `lam < 0` backward. A stationary-frame lateral harmonic at `omega` is split into its forward
    (`+omega`) and backward (`-omega`) circular parts, each solved by **Cramer's rule**. That is the "small, dependency-free complex
    frequency-domain solver". Reusing the real LU would need a new `rcfd-post -> rcfd-reactor` edge and a 4x4 real embedding of a
    2x2 complex system, for no gain. The hysteretic term carries `sgn(lam)`: it is an equivalent viscous damper `eta k / |lam|`
    and must dissipate for both whirl directions (without `sgn`, backward components would generate energy).
  * Singularity: the solve refuses `|det D| <= 1e-12 S`, where `S` bounds the cancelling terms of `det D`; the error names the
    frequency ("undamped resonance").
  * Natural frequencies at speed `Omega`: the roots of the polynomial `det D(lam)` (degree 4, or less when `It = 0`), viscous
    damping included, hysteretic ignored, by Durand-Kerner iteration plus three Newton polishing steps. Accuracy falls to about
    `sqrt(eps)` near a double root (coincident frequencies); this is a stated shortcut, and no registered set-up has one.
  * Critical speeds (undamped, synchronous, forward): the positive roots `s = Omega^2` of `(kt - m s)(kr - (It - Ip) s) = kc^2`,
    by the cancellation-free quadratic formula. A mode with `Ip >= It` (disc-like) has no synchronous crossing; the list then holds
    fewer values and is never padded with NaN.
* **D3, E1 imbalance.** `unbalance_load` sums point unbalances (mass, radius, angle, axial position): rotating force
  `F = sum m_i r_i Omega^2 exp(i phi_i)` and its moment about the centre of mass. A static unbalance is one in the centre-of-mass
  plane; a couple unbalance is two equal unbalances 180 degrees apart at different `z`. The specific unbalance is `e = U / m`.
* **D4, balance grade (definition, first principles).** `G = e Omega` in mm/s, with `e` the centre-of-mass offset and `Omega` the
  maximum service speed in rad/s. The permissible residual unbalance is `U_per = G m / Omega`. In units: `U_per [g mm] = 1000 G [mm/s]
  m [kg] / Omega [rad/s]`. The grade **scale** of ISO 1940-1 (G 0.4, 1, 2.5, 6.3, 16, 40, 100, 250, 630, 1600, 4000; consecutive
  grades a factor 2.5 apart) is recalled and **UNVERIFIED**; so are the usual assignments "fans: G 6.3" and "small electric
  armatures: G 2.5", and the supersession of ISO 1940-1 by ISO 21940-11. The code exposes the scale as
  `ISO1940_GRADES_UNVERIFIED` and depends on it nowhere.
* **D5, E3 interface.** `response(rotor, spin, loads)` takes harmonic loads in **stationary housing axes**, one complex amplitude
  per component and per frequency (for example `n x BPF` from unsteady blade loads of a CFD run or another agent's reduced model):
  `[Fx, Fy, Fz]` and `[Mx, My, Mz]` about the centre of mass. It returns, per frequency, the rotor motion, the lateral force on
  the housing at each bearing, and the **mount** force and moment: the resultant on the rigid housing, moments about the centre of
  mass. The axial force `Fz` and the torque `Mz` are **not** carried by the lateral model; they pass to the mount unchanged (rigid
  axial and torsional path at constant speed). They are not dropped, because axial thrust is often the largest BPF load.
  `ellipse_major` gives the peak of an elliptical orbit or force.
* **D6, E4 hook (F-237).** `load_deck` writes the housing loads as an F-237 `Deck`: grids 1 and 2 at the bearing seats `(0, 0,
  z_j)`, two static load sets per harmonic `n` (`sid0 + 2n` = real part, `sid0 + 2n + 1` = imaginary part, with time load `L(t) =
  L_re cos(omega t) - L_im sin(omega t)`), lateral `FORCE` cards at both bearings, the axial force and a `MOMENT` card for the
  torque at a named thrust bearing. `FORCE, SID, G, CID, F, N1, N2, N3` and `MOMENT` (same fields) are new cards of the deck, written
  with `CID = 0` and `F = 1.0`; their field layout is recalled and **UNVERIFIED**, like the existing `GRID` card (ADR 0051 D8).
  Frequency-response cards are out of scope. The writer emits nothing new for an empty list, so every existing deck is
  byte-identical.

## What this does NOT model

Blade flexibility and blade modes; shaft bending (the rotor is rigid); housing and mount flexibility (the housing is rigid and
grounded); anisotropic or cross-coupled bearings (fluid-film or sleeve bearings, ball-bearing anisotropy); bearing nonlinearity,
clearance, preload change and ball-pass excitation; rotor internal damping (and its whirl instability); thermal effects (stiffness
change, growth); speed fluctuation and motor electromagnetic forces; axial and torsional dynamics (rigid pass-through, D5);
backward critical speeds (they need anisotropy). Every number is a stated proxy until checked against measured data.

## Registered checks (`crates/rcfd-post/tests/w46c_rotordyn.rs`)

Every check runs the general two-bearing solver, never a closed-form helper inside the module. "rel" is `|a - b| / |b|`. Set-ups:

* **Jeffcott**: `Rotor::jeffcott(m, k, c)`, a point mass between two equal bearings at `z = -1, +1` m sharing `k` and `c`, `It = Ip
  = 0`, so the translation decouples exactly: `m = 0.035` kg, `k = 4e5` N/m, `zeta = c / (2 sqrt(k m)) = 0.05` (0 for J1),
  `e = 1e-5` m (a 1 g unbalance at radius `m e / 1e-3`, in the centre-of-mass plane). `omega_n = sqrt(k / m)`.
* **asym**: `m = 0.035` kg, `It = 1.2e-5`, `Ip = 2e-5` kg m^2, bearings `z = -0.012, +0.006` m, `k = 2e5, 3e5` N/m, `c = 0` (R1) or
  `5` N s/m each (J4). Translation and tilt frequencies are distinct.
* **disc**: `m = 0.035`, `It = 1e-5`, `Ip = 2e-5` (thin disc, `Ip = 2 It`), bearings at `z = -0.01, +0.01` m with `k = 2e5` each,
  undamped, spin 1500 rad/s (so `kc = 0`: tilt and translation decouple).
* **energy**: the asym rotor with `c = 6, 3` N s/m and `eta = 0.03, 0.05`, spin 300 rad/s.

| Id | Check | Limit | Reason for the limit | Negative control (must miss the limit by at least 10x) |
|---|---|---|---|---|
| J1 | Jeffcott critical speed and natural frequencies `+-omega_n` (undamped) | rel <= 1e-9 | closed form; quadratic and root iteration carry round-off of order 1e-15, so 1e-9 leaves six orders for it | only one bearing's stiffness counted (`k_2 = 1e-30`): `sqrt(k / 2m)`, rel 0.29 expected |
| J2 | resonance amplitude `|X| / e = 1 / (2 zeta)` at spin = `omega_n` exactly | rel <= 1e-9 | `X / e = r^2 / (1 - r^2 + 2 i zeta r)`, exactly `1 / (2 i zeta)` at `r = 1`. The curve's maximum `1 / (2 zeta sqrt(1 - zeta^2))` at `r = 1 / sqrt(1 - 2 zeta^2)` differs by about `zeta^2 / 2 = 1.3e-3`, so the check is at `r = 1`, not at the maximum | damping of one bearing only: `1 / zeta`, rel 1 |
| J3 | supercritical self-centring: (a) `|X| / e - 1` at `r = 1e5`; (b) centre-of-mass orbit `|X + e| / e` against `|1 + 2 i zeta r| / |1 - r^2 + 2 i zeta r|` at `r = 1e3` | (a) `<= 1e-9` absolute; (b) rel <= 1e-9 | (a) the analytic deviation is `(1 - 2 zeta^2) / r^2 = 1e-10`, a tenth of the limit; (b) the difference `X + e` cancels about four digits at `r = 1e3` (amplification `r / (2 zeta) ~ 1e4`), leaving round-off near 1e-12 | quasi-static model `X = U Omega^2 / k` (no inertia): `|X| / e = r^2 = 1e10` |
| J4 | low-speed transmitted load equals the static unbalance load: mount force and mount moment against the applied unbalance force and moment (asym rotor, `c = 5`, two unbalances at `z = 0.004, -0.009` m), spin `= 1e-5 x` lowest critical speed | rel <= 1e-9 for force and for moment | transmissibility `I + (Omega^2 M - Omega^2 Ip G) D^-1`: the deviation is of order `r^2 = 1e-10`; bearing damping enters only through `D`, not to first order | force on the rotor reported instead of on the housing (sign): rel 2 |
| R1 | rigid rotor on two bearings, `Omega = 0`, undamped: the four roots against `+-sqrt` of the eigenvalues of `M^-1 K`, `omega^2 = ((a + d) -+ sqrt((a - d)^2 + 4 kc^2 / (m It))) / 2`, `a = kt / m`, `d = kr / It` | rel <= 1e-9 for every root; exactly four roots | closed-form 2x2 eigenproblem; simple, well-separated roots | coupling `kc` dropped: `sqrt(a)`, `sqrt(d)` |
| R2 | thin disc gyroscopic splitting: forward and backward tilt whirl `lam = (Ip Omega +- sqrt(Ip^2 Omega^2 + 4 It kr)) / (2 It)` and the translation pair `+-sqrt(kt / m)`; critical speeds: exactly one, `sqrt(kt / m)` (no tilt critical for `Ip > It`) | rel <= 1e-9 each | closed form for `kc = 0` | gyroscopic coupling omitted (`Ip = 0`): forward tilt root `sqrt(kr / It)` |
| E | energy balance: work input per cycle by the load equals the dissipation in the bearings (viscous plus hysteretic), integrated in time with 64 trapezoid points over one period from the real-coordinate phasors (exact for these trigonometric polynomials); two loads: a general load at 700 rad/s with forward and backward parts, and an unbalance at the spin | rel <= 1e-9 | conservation; the gyroscopic term does no work. The integral does not use `D(lam)` or the complex coordinates, so it checks the solve and the forward/backward split | the solver omits the hysteretic damping (`eta = 0` in the solve, the true `eta` in the dissipation) |
| B | balance grade conversion exact: `G(e = 2^-10 m, Omega = 256) == 250` mm/s and `U_per(250, 2 kg, 256) == 2^-9` kg m bit for bit; round trip `G -> U_per -> e -> G` at G 6.3, 35 g, 3000 rpm | bitwise; round trip rel <= 4 eps | binary-exact inputs; the round trip is four roundings | speed in rpm instead of rad/s (factor `60 / 2 pi`) |
| U | static unbalance: `|Fx| = |Fy| = m r Omega^2`, moment zero; couple unbalance (two equal masses 180 degrees apart at `+-a`): force zero, `|Mx| = |My| = 2 a m r Omega^2`; direction: `Fx = F exp(i phi)`, `Fy = F exp(i (phi - pi / 2))` (forward rotation) | `<= 1e-12` relative to `m r Omega^2` (`m r Omega^2 a` for moments) | a handful of floating-point operations | `Omega` to the first power instead of squared |
| X | E3 to E4: three harmonics at `7, 14, 21 x` spin with lateral, axial and torque parts on the energy rotor; the deck round-trips bit-identically through `to_string_checked` and `parse`; the resultant force and moment (about the centre of mass) of each load set equals the real or imaginary part of the mount force and moment | round trip exact; resultant rel <= 1e-12 of the largest component | the deck writes full double precision (`{:?}`); a resultant is a sum of two or three terms | an export that drops the passed-through axial force |
| I | invalid inputs are located: the error message names the field (`mass`, `it`, `bearings[1].k`, `bearings[0].c`, `bearings[1].eta`, `bearings[0].z and bearings[1].z`, `spin`, `loads[1].omega`, `unbalance[0]`, `grade`, `thrust_bearing`), the undamped resonance (`singular at omega`), and a deck `FORCE` card with `CID = 2` (`line 3`) | every message contains its name | specification | the valid inputs are accepted (the validator does not reject everything) |
| W | worked example, 120 mm fan: **REPORTED, no limit** (below) | none | | |

### Worked example W (REPORTED; every value is ASSUMED, not measured)

35 g impeller with magnet ring, `It = 1.2e-5`, `Ip = 2.0e-5` kg m^2 (disc-like); two ball bearings at `z = -10, +6` mm in a plastic
bearing tube, effective radial stiffness `2e5` N/m each (tube included), `c = 4` N s/m, `eta = 0.02`; 7 blades; 2000 rpm. Residual
unbalance: 1.0 g mm static in the centre-of-mass plane plus a 0.5 g mm couple at `+-10` mm (at 1 rad from the static one). Blade-passing loads: 5 mN lateral (fixed `x` direction) and 20 mN axial at BPF, 2 mN and 8 mN at 2 BPF. Reported:
`U_per` at G 6.3, the grade of the static unbalance, critical speeds, natural frequencies at 2000 rpm, the unbalance response
(orbit, bearing forces, mount force and moment) at 600, 1000, 2000, 3000, 4800 rpm, and the bearing and mount spectra at BPF and
2 BPF. These are the numbers an optimiser would see.

### Status decision rule (registered now)

* A check passes only if every listed quantity is within its limit and its negative control misses by at least 10x (the test asserts
  both). A non-discriminating control makes its check non-discriminating; it is reported, not re-tuned.
* A missed check becomes an ignored `*_known_miss` test with the limit intact. A defective set-up gets a new test, registered as an
  amendment before it runs.

## Smoke results before registration (rule d)

`cargo test -p rcfd-post --test w46c_rotordyn smoke_w46c -- --ignored` (debug), natural frequencies at spin 300 rad/s, unbalance
response at 1000 rad/s:

| Set-up | roots | all finite | critical speeds | response |
|---|---|---|---|---|
| Jeffcott, undamped | 2 | yes | 1 | Ok |
| Jeffcott, zeta 0.05 | 3 | yes | 1 | Ok |
| asym, c = 0 | 4 | yes | 1 | Ok |
| asym, c = 5 | 4 | yes | 1 | Ok |
| disc | 4 | yes | 1 | Ok |
| energy | 4 | yes | 1 | Ok |

The Jeffcott set-up has 2 roots undamped (`It = 0`, the tilt degree of freedom is massless) and 3 with damping (the extra root is
the purely decaying massless tilt, `lam = i kr / cr`). The asym and energy rotors have `Ip > It`, hence one critical speed.

## Out of scope

E3's source of the harmonics (unsteady blade loads need C1/C2 and a converged fan: the FAN-01 stall is open); E5; a case-tree or
assistant-tool key (proposed as a roadmap item in the hand-back); frequency-response deck cards; any comparison with measured fan
vibration (no data).

## Files

* `crates/rcfd-post/src/rotordyn.rs`, `crates/rcfd-post/src/lib.rs` (one line)
* `crates/rcfd-io/src/nastran.rs` (FORCE, MOMENT), `crates/rcfd-io/src/registry.rs` and `crates/rcfd-io/tests/w27c_registry.rs`
  (`..Default::default()` only)
* `crates/rcfd-post/tests/w46c_rotordyn.rs`
* `docs/interchange-formats.md` (the two cards)

## Amendment 1 (written after the one registered run of all checks, before the corrected J1 test runs)

* What happened: `w46c_j1_jeffcott_critical_speed` panicked in its **negative control**, after its check had printed critical
  speed rel 0 and natural frequencies rel 0 / 0 (known to me when writing this). The control set `k_2 = 1e-30`, which leaves the
  point-mass rotor on a single bearing at `z = -1`. Then `kt kr - kc^2` cancels to round-off, the static stiffness is singular,
  `critical_speeds` returns an empty list and `[0]` panics. The set-up is defective: the wrong model was meant to be "one bearing's
  stiffness counted", not "a rotor on one bearing".
* The old test stays as `w46c_j1_jeffcott_critical_speed_defective_control`, ignored, unchanged except the attribute.
* New test `w46c_j1b_jeffcott_critical_speed`: the same check and limit (critical speed and `+-omega_n`, rel <= 1e-9). Corrected
  control: both bearings with `k / 4`, so the total is `k / 2` (the stiffness of one of the two bearings) and `kc` stays 0; expected
  `sqrt(k / 2m)`, rel 0.29; must miss 1e-9 by at least 10x.

## Addendum: measured values (one registered run each, debug build, `target-w46c`, commit f235b7d)

Every check passed with its negative control discriminating; J1 passed in its check and failed in its control set-up
(amendment 1), and its corrected test J1b passed. No known miss.

| Id | Measured | Limit | Negative control (measured) |
|---|---|---|---|
| J1 (defective control) | critical speed rel 0, natural `+-omega_n` rel 0 / 0; control panicked (empty critical-speed list) | 1e-9 | defective, test ignored |
| J1b | critical speed 3380.617 rad/s, rel 0; natural `+-omega_n` rel 0 / 0 | 1e-9 | rel 0.293 |
| J2 | `|X| / e = 9.999999999999996` against 10, rel 3.6e-16 | 1e-9 | rel 1.00 |
| J3 | (a) `|X| / e - 1 = 9.95e-11` at `r = 1e5` (analytic `(1 - 2 zeta^2) / r^2 = 9.95e-11`); (b) centre of mass rel 3.2e-14 | 1e-9 | `|X| / e = 1.0e10` |
| J4 | force rel 9.8e-11, moment rel 2.8e-10 | 1e-9 | rel 2.0 |
| R1 | roots `+-1795.1387`, `+-3789.8801` rad/s, worst rel 1.3e-16 | 1e-9 | rel 1.10 |
| R2 | forward tilt 4000.000 rad/s rel 2e-90 (exact to print), backward -1000.000 rel 1.1e-16, translation rel 1.3e-16 / 7e-98; critical speeds `[3380.617]` only | 1e-9 | forward rel 0.155 |
| E | general load rel 5.5e-16 (work 9.5977e-8 J per cycle); unbalance rel 0 (6.5272e-9 J) | 1e-9 | rel 2.03 |
| B | `G = 250` and `U_per = 2^-9` bitwise; round trip rel 1.4e-16 | bitwise; 4 eps = 8.9e-16 | rel 8.55 |
| U | static 1.2e-16, couple 1.2e-16, direction 1.2e-16 | 1e-12 | rel 0.997 (`1 - 1 / Omega` at 300 rad/s) |
| X | deck round trip identical; resultant worst rel 0 | exact; 1e-12 | the dropped axial force is caught (asserted) |
| I | all 14 messages name their field, frequency or line | contains | valid inputs accepted |

J4's moment deviation (2.8e-10) is the largest relative number and is the analytic `O(r^2)` term at `r = 1e-5`, as registered. All
other checks sit at round-off.

### Worked example W (REPORTED; assumed rotor, see above)

* `U_per` at G 6.3, 35 g, 2000 rpm: **1.053 g mm**. The assumed 1.0 g mm static unbalance is grade **G 5.98** at 2000 rpm.
* Critical speeds: **one**, 32 062 rpm (translation). The conical mode has no synchronous crossing (`Ip > It`).
* Natural frequencies at 2000 rpm: backward -541.1 Hz and -205.1 Hz, forward 259.7 Hz and 542.1 Hz. The conical pair splits by
  gyroscopic coupling around about 230 Hz.
* Unbalance response (static plus couple):

| rpm | centre-of-mass orbit [um] | bearing 1 force [N] | bearing 2 force [N] | mount force [N] | mount moment [N m] |
|---|---|---|---|---|---|
| 600 | 0.0124 | 0.00208 | 0.00433 | 0.00395 | 3.94e-5 |
| 1000 | 0.0345 | 0.00576 | 0.0120 | 0.0110 | 1.09e-4 |
| 2000 | 0.138 | 0.0228 | 0.0479 | 0.0441 | 4.32e-4 |
| 3000 | 0.311 | 0.0506 | 0.107 | 0.0998 | 9.54e-4 |
| 4800 | 0.800 | 0.125 | 0.269 | 0.260 | 2.32e-3 |

* The whole range is subcritical. The mount force is 0.5 % above the static unbalance force `U Omega^2` at 2000 rpm (0.0441 against
  0.0439 N) and 2.8 % above it at 4800 rpm (0.260 against 0.253 N). The couple adds the mount moment.
* Blade-passing spectrum at 2000 rpm (7 blades):
  * BPF 233.3 Hz: bearing forces 3.16 and 3.70 mN, mount lateral 6.18 mN for 5 mN applied (x1.24), axial 20 mN passed through.
  * 2 BPF 466.7 Hz: bearing forces 3.95 and 3.33 mN, mount lateral 7.28 mN for 2 mN applied (x3.6), axial 8 mN.
* What an optimiser would read from this: with these assumed bearing stiffnesses, BPF at 2000 rpm falls between the backward
  (205 Hz) and forward (260 Hz) conical modes, and 2 BPF lies 14 % below the 542 Hz translation mode. Blade count and speed range
  move the tonal vibration more than the balance grade does. This depends entirely on the assumed `k`, `c`, `It` and `Ip`; nothing
  here is measured.

## References

* Rigid-rotor and Jeffcott equations, gyroscopic moment `Ip Omega theta'`, hysteretic damping as `eta k / omega`: derived from
  rigid-body mechanics in this ADR (D2), not copied from a text.
* ISO 1940-1 balance-quality grades: definition `G = e Omega` used; the grade table and its assignments UNVERIFIED (D4).
* Nastran `FORCE` / `MOMENT` field layout: recalled, UNVERIFIED (D6).
