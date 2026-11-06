# ADR 0194: F-254 registration: particle wall interaction (erosion and rough-wall rebound)

Status: **REGISTERED 2026-11-06**, written before any code of F-254. Not yet accepted by the maintainer. The only code
run before this text was the two record tests of check E (R3, R4): they use the code of main as it is and were run on it
to make the records below. Results go in the addendum at the end.

Parents: [ADR 0028](0028-wave18c-particles.md) (F-056, the tracker), [ADR 0057](0057-wave29c-lagrangian-extensions.md)
(F-128, the extensions), [ADR 0059](0059-wave30a-wiring-particles-mixing-plane.md) (two-way coupling in the solver loops),
[ADR 0192](0192-f253-registration-particle-wall-film.md) (F-253, the wall film, its records R1 and R2 and its question
Q4), [ADR 0155](0155-job48u-rotate-the-mesh-instead-of-the-flow.md) (optional objects in the case tree, the record R0 of
`case_hashes_w48u.rs`), [ADR 0156](0156-job48v-adaptive-pressure-correctors.md) D6 (`hash_omit_default`),
[ADR 0137](0137-linux-gate-platform-values.md) (platform values). Roadmap item F-254, split from F-245 by the user on
2026-10-22.

Read at main 54ec840. Number: 0192 is the highest on main; 0191 is on the branch `f259-layering-ale-wiring` (F-259) and
0193 is reserved for F-246. No branch holds a 0194.

House rules ([ADR 0130](0130-wave48a-overview-and-run-record-contract.md), ADR 0028, ADR 0192):

- limits are fixed here, before the code and the runs;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact, and the cause is diagnosed, not tuned away;
- a defective set-up gets a new corrected test, and the old one stays as an ignored record;
- every new code path is opt-in, and runs without the new models are unchanged bit for bit;
- a new case key uses `hash_omit_default`, and `case_hashes_w48u.rs` H1 stays green;
- neither `crates/rcfd-api/tests/baseline_hashes.rs` nor the record R0 is re-recorded;
- no new third-party crate, no commit trailers.

Clean-room: the erosion correlation and the rough-wall model are taken from the published sources listed under "Sources",
and everything else is derived below. Status words, as in ADR 0028 and ADR 0192: "read" means read in the source itself;
"verified (secondary)" means found with the same content in published secondary sources read during this work; "derived"
means derived here; "registered here" marks a choice of this text that no source read during this work states. No vendor
code, text or install was used.

## What F-254 asks

The roadmap entry: for a uniform parcel stream hitting a plate at three angles, the reported erosion rate equals the
closed-form impact-flux expression of the chosen correlation (constants read from the source) to 1e-10 relative; the
rough-wall rebound model's sampled rebound angles match its prescribed distribution (Kolmogorov distance below the 1
percent critical value for the sample size). Out of scope: comparison with measured data, particle reactions,
discrete-element coupling.

Dependencies. F-128 is done. F-056 is partial only because it has no cyclone separation case, which F-254 does not use;
this text reads the partial F-056 as no block, as ADR 0192 did. That is a reading of the dependency, not a decision.

Splash and rebound of parcels that reach the wall film (ADR 0192 Q4, whose recommendation was to add them to F-254) are not
in the F-254 acceptance text, and no source for impact regimes was read. They are not delivered here (Q5).

## What the code read shows (main 54ec840, nothing run)

1. **Wall hits.** `Tracker::advance` finds a boundary hit by bisection on the integrated path; `v_hit` is the parcel
   velocity at the hit. The action of the face (`WallAction::{Reflect(e), Stick, Escape, Film}`) is applied after the hit
   point, time and velocity are set. `Reflect(e)` uses `drag::reflect(v, n, e) = v - (1 + e)(v . n) n` with `n` the unit
   area vector of the face, out of the domain; each reflection counts against `max_bounces`. With the exchange record on,
   the wall momentum `m (v_after - v_hit)` is added to `wall_impulse`.
2. **Random numbers.** `SplitMix64` has `uniform` and `gauss` (Box-Muller, one of the pair per call). Each injector and the
   dispersion model have their own generator, so a new generator leaves their streams untouched.
3. **Optional parts.** The film (ADR 0192) is switched on by `Tracker::set_film`, appended to `Tracker::fingerprint` only
   when set, and reported by the run path only when set. The case tree reads it from the optional object `particles.film`
   (`.optional().hash_omit_default()`), absent from the data until a value inside it is set.
4. **Run path.** `ParticleRun::report` writes `particle_report.json`; `write_outputs` writes the other particle files.
5. **The mesh.** `box_mesh` and `box_2d` name their walls `xmin`, `xmax`, `ymin`, `ymax` (and `zmin`, `zmax` or
   `frontAndBack`). Boundary face area vectors point out of the domain.

## Sources

| Item | Status | What was read |
| --- | --- | --- |
| Erosion relation `E = m_p K U_p^n F(alpha)` (eq. 7.1, printed with `~`), the angle function of eq. 7.2/7.3, `F(alpha) = sum_{i=1..8} (-1)^(i+1) A_i (alpha pi/180)^i` with `alpha` in degrees, constants of Table 7-1: `A_1..A_8 = 9.370, 42.295, 110.864, 175.804, 170.137, 98.398, 31.211, 4.170`; the angle as defined by Figure 7-1 (between the particle path and the surface); the depth rate of eq. 8.1, `E_L = m_p K U_p^n F(alpha) / (rho_t A_t) C_unit` with `C_unit = 3.15E10` for mm/year | read | DNV RP O501 "Erosive wear in piping systems", Revision 4.2 - 2007, amended January 2011 (Det Norske Veritas), Chapter 7 and 8.1, pages 10 to 14. Read on 2026-11-06 from a copy hosted at wiki.pengtools.com; the document itself says the electronic version found through dnv.com is the binding one, and that version was not fetched. The pages were rendered and read as images, and the constants checked against the text layer. DNV attributes F to Haugen et al. (1994), which was not read |
| Material constants of Table 7-2: steel grades `K = 2.0e-9 (m/s)^-n`, `n = 2.6`, 7800 kg/m^3; titanium grades `2.0e-9`, 2.6, 4500; GRP/epoxy `0.3e-9`, 3.6, 1800; GRP/vinyl ester `0.6e-9`, 3.6, 1800 | read | the same pages. DNV says K and n "have to be determined by experimental investigations" and calls Table 7-2 recommended values |
| Rough wall: the impact angle on a rough wall is `theta' = theta + Delta_gamma xi`, `xi` Gaussian with mean 0 and standard deviation 1, `Delta_gamma` from experiment (5.3 degrees quoted for 100 um glass beads on steel); a virtual wall tilted by the random angle (their Figure 2); the shadow effect named and its handling by Sommerfeld and Huber (1999) described in words only ("shifts the distribution function of Delta_gamma xi towards the positive side and avoids the unphysical situation that particles hit the roughness structure with a negative angle"); rebound with restitution `e_n`, friction `mu_d` and particle rotation (eqs. 1 to 4), which for `mu_d = 0` and no rotation leaves `v_t = u_t`, `v_n = -e_n u_n` | verified (secondary) | Z. F. Tian, K. Inthavong, J. Y. Tu and G. Yeoh, "Numerical simulation of gas-particle flows over an in-line tube bank", ANZIAM J. 48 (CTAC2006), C509 to C526, 2007, pages C514 and C515, rendered and read. They cite Sommerfeld (1992) for the model and Sommerfeld and Huber (1999) for the shadow effect; neither original was accessed |
| The same Gaussian `p(gamma) = exp(-gamma^2/(2 sigma^2))/sqrt(2 pi sigma^2)` and `alpha' = alpha + Delta_gamma xi`; measured `Delta_gamma` from about 1 to 15 degrees, depending on the particle size and the wall | verified (secondary) | S. Lain and M. Sommerfeld, "Erosion prediction in a horizontal to vertical elbow by the Euler-Lagrange approach", slides of the 14th Workshop on Two-Phase Flow Predictions, Halle, September 2015 (text layer and plots read) |
| The virtual-wall normal in the plane of incidence `n_gamma = -sin(gamma) t + cos(gamma) j` (eq. 7.21) and the Gaussian of `gamma` on `[-pi/2, pi/2]` (eq. 7.27); the shadow effect moves the density of `gamma` towards positive values at small impact angles (page 70) | verified (secondary) | D. R. Radenkovic, doctoral dissertation (in Serbian), University of Belgrade, Faculty of Mechanical Engineering, 2019, pages 69 and 70 rendered and read. The effective distribution of Sommerfeld and Huber is used there but not written out |
| One-sample Kolmogorov-Smirnov test: `D_n = sup_x abs(F_n(x) - F(x))`; for a continuous `F`, `sqrt(n) D_n` converges to the Kolmogorov distribution `Pr(K <= x) = 1 - 2 sum_{k>=1} (-1)^(k-1) exp(-2 k^2 x^2)`; reject at level `alpha` when `sqrt(n) D_n > K_alpha`, `Pr(K <= K_alpha) = 1 - alpha` (asymptotically valid as `n -> infinity`); the table value `c(0.01) = 1.628` | verified (secondary) | the Wikipedia page "Kolmogorov-Smirnov test", read 2026-11-06; Kolmogorov (1933) and Smirnov (1948) not accessed |
| Not accessed: Sommerfeld (1992), Sommerfeld and Huber (1999) (the effective distribution of the shadow effect), Haugen et al. (1994), Finnie (1960), Oka et al. (2005), Konan, Kannengieser and Simonin (2009), Radenkovic and Simonin (2018) | not used | A search-engine summary of Radenkovic and Simonin (2018), not the paper, says that the shadow-effect model draws the virtual-wall angle from a truncated Gaussian and repeats the draw when the particle does not return to the flow. It is recorded here and not used as a source |

The open-access copy of Finnie's work found during the search is D. McFadden's M.S. thesis "Erosion of ductile metals by
solid particles" (LBL-6279, 1977); its equations are not legible in the scanned text, so the Finnie model is not used.

## The erosion model

**E1. Impact.** At a hit on boundary face `f`, `n` is the unit area vector of `f` (out of the domain) and `v` the parcel
velocity at the hit (`v_hit`), before any wall action. Impact speed `U = abs(v)`; normal part `v_n = v . n`; tangential
speed `v_t = abs(v - v_n n)`; impact angle `alpha = atan2(max(v_n, 0), v_t)`, in `[0, pi/2]` (Figure 7-1 of DNV: the
angle between the particle path and the surface). `U = 0` gives `alpha = 0`.

**E2. The correlation.** Each hit erodes the mass `dE = m K U^n F(alpha)` [kg] (DNV eq. 7.1 with `m_p` the mass of the
parcel), with `m = n_p rho_p pi d^3 / 6` (`n_p` real particles per parcel) and `F` of DNV eq. 7.2 and Table 7-1, evaluated
with `alpha` in radians (`alpha_deg pi/180 = alpha`). `K` [(m/s)^-n] and `n` are inputs with no default (Q3); Table 7-2 is
quoted in the documentation. `F` is the function DNV gives for steel grades (ductile); the brittle curve of DNV Figure
7-2 has no formula there and is not delivered. On a 0.1-degree grid of `[0, 90]` degrees, `F` is `>= 0`, 0 at 0, largest
1.0019 at 36.4 degrees and 0.5685 at 90 degrees (computed from eq. 7.2 with a Python script, not in the repository).

**E3. Booking.** `dE`, the hit count and the parcel mass `m` are added to face `f` at every hit of a face of an erosion
patch, before the wall action, whatever that action is (reflect, rough reflect, stick, escape). With the rough wall on, the
angle and speed are those of E1 against the face, not against the virtual wall of W2. Erosion is an observation: it never
changes a parcel, the flow or the exchange record. An erosion patch cannot carry the wall film (refused): the film absorbs
liquid parcels, and erosion of the wall under a film is not defined here.

**E4. Reported values.** Over the tracked time `T` (the tracker's time since its start), for each face: the eroded mass
`E_f` [kg], the hits, the impact mass `M_f` [kg], the erosion rate `ER_f = E_f / (A_f T)` [kg/(m^2 s)], the impact mass
flux `G_f = M_f / (A_f T)` [kg/(m^2 s)] and the depth rate `ER_f / rho_t` [m/s], with `rho_t` the target density (an input,
no default). These are SI forms of DNV eq. 8.1 without its unit factor `C_unit`; the second form printed in eq. 8.1 (with
`10^3`) is not used. `T = 0` reports zero rates.

**E5. The closed form of the acceptance (derived).** When `N` parcels of mass `m`, each at the same speed `U` and angle
`alpha`, hit face `f` during `[0, T]`, `E_f = N m K U^n F(alpha)` and

    ER_f = G K U^n F(alpha),   G = N m / (A_f T)

which is the impact-flux expression of the roadmap.

## The rough-wall model

**W1. Roughness angle (Tian et al. eq. 5, Lain and Sommerfeld, Radenkovic eq. 7.27).** At a hit on a face of a rough
patch, a roughness angle `gamma = Delta_gamma xi` is drawn, `xi` standard normal, `Delta_gamma` [rad] the input
`angle_std`.

**W2. The virtual wall (derived from the same sources).** Write `v = U (cos(alpha) t + sin(alpha) n)` with `t` the unit
tangential direction of `v`. The virtual wall normal, tilted by `gamma` in the plane of incidence, is

    n' = cos(gamma) n + sin(gamma) t

(the form of Radenkovic eq. 7.21 with `j = -n`, up to the sign convention of `gamma`), so `v . n' = U sin(alpha + gamma)`: the effective impact angle is `alpha + gamma` (Tian
et al. eq. 5). At normal incidence (`abs(v - v_n n) <= 1e-12 U`) the plane of incidence is undefined, and `t = cos(phi) p +
sin(phi) q` with `phi = 2 pi u`, `u` one uniform draw, and the fixed pair `p = unit(n x e_k)` (`e_k` the coordinate axis of
the smallest `abs(n_k)`, the first of equal ones), `q = n x p` (registered here).

**W3. Rebound.** `v' = v - (1 + e)(v . n') n'`: the tracker's reflection law about `n'`, with the patch restitution `e`.
This is Tian et al. eq. 3 with `mu_d = 0` and no particle rotation (the tracker has none). `e` must be in `(0, 1]` on a
rough patch; `e = 0` makes the rebound slide along the virtual wall and is refused.

**W4. Realizability (registered here).** The draw is accepted when `v . n' > 0` (the parcel reaches the virtual wall from
the fluid side, `alpha + gamma > 0`), `v' . n < 0` (the rebound leaves the real wall) and `abs(gamma) < pi/2`; otherwise
`xi` is drawn again. This truncates the Gaussian of W1 to the realizable angles. It is not the effective distribution of
Sommerfeld and Huber (1999), which was not read (Q2). After 1000 rejected draws in one hit the smooth law with `n` applies
and a fallback is counted. When `v . n <= 0` or `U = 0` at the hit (possible only at a grazing hit resolved by the
bisection), the smooth law with `n` applies, nothing is drawn, and a grazing hit is counted.

**W5. The prescribed distribution (derived).** The rebound angle against the real wall, in the plane of incidence, is
`alpha_r = atan2(-v' . n, v' . t)`, in `(0, pi)` after W4. With `beta = alpha + gamma`,

    alpha_r(gamma) = atan2(e sin(beta), cos(beta)) + gamma,   d alpha_r / d gamma = e / (cos^2 beta + e^2 sin^2 beta) + 1 > 0

so on `gamma` in `(-alpha, pi/2)` the map is strictly increasing, the accepted set of W4 is an interval `(gamma_lo,
gamma_hi)` and, with `Phi` the standard normal distribution function and `g` the inverse of `alpha_r(gamma)`,

    P(alpha_r <= theta) = (Phi(g(theta)/Delta_gamma) - Phi(gamma_lo/Delta_gamma)) / (Phi(gamma_hi/Delta_gamma) - Phi(gamma_lo/Delta_gamma))

For `e = 1`: `alpha_r = alpha + 2 gamma`, `gamma_lo = -alpha/2`, `gamma_hi = (pi - alpha)/2`, `g(theta) = (theta -
alpha)/2`. The rebound stays in the plane of incidence (`v' . (n x t) = 0`) and `abs(v') = U sqrt(cos^2 beta + e^2 sin^2
beta)`. At normal incidence the polar angle `psi = abs(alpha_r - pi/2)` of the rebound about `-n` has, for `e = 1`,
`P(psi <= x) = (Phi(x/(2 Delta_gamma)) - Phi(-x/(2 Delta_gamma))) / (Phi(pi/(4 Delta_gamma)) - Phi(-pi/(4
Delta_gamma)))`, and the azimuth of its tangential part is uniform on `[0, 2 pi)`.

**W6. Generator.** One `SplitMix64` for all rough faces of a tracker, seeded with the input `seed`; draws are made in the
order of the hits, which the serial tracker fixes, so a run is reproducible bit for bit. No other generator changes.

**W7. With the other models.** A rough reflection counts against `max_bounces` and books `m (v' - v_hit)` in
`wall_impulse`, as a smooth one does; with `two_way` the carrier therefore sees the rough walls through the parcels. The
erosion of a hit on a rough face uses E1 (W2 does not enter it). Range of `angle_std`: `(0, 30]` degrees, so at most 0.3 percent of
the Gaussian lies beyond 90 degrees (registered here; the sources quote 1 to 15 degrees).

Not modelled: wall friction and particle rotation (Tian et al. eqs. 1 to 4), roughness out of the plane of incidence, a
roughness that depends on the particle size, several collisions inside one roughness structure, the effective distribution
of Sommerfeld and Huber.

## Settings and code

**S1. Library (`rcfd-particles`).**

- Module `erosion`: `ErosionSpec {patches, k, n, density}`; `Erosion` with the per-face eroded mass, hits and impact mass
  and read access to faces, areas, centres, rates and the CSV text; the constants `DNV_A`, the functions
  `dnv_angle_function(alpha)` (E2) and `impact_angle(v, n)` (E1).
- Module `rough`: `RoughSpec {patches, angle_std, seed}`; `RoughWall` with the generator and the counts (collisions,
  redraws, fallbacks, normal incidences, grazing hits); the function `rough_rebound` (W2 to W4).
- `WallAction::Rough(e)` is added. `Tracker::set_erosion(spec)` and `Tracker::set_rough_wall(spec)` switch the models on,
  `Tracker::erosion()` and `Tracker::rough_wall()` read them. `set_erosion` refuses a second call, an empty or repeated
  patch list, an unknown patch, a patch that is not a wall, a film face, `k` or `density` not above 0 and `n` not above 0.
  `set_rough_wall` refuses a second call, an empty or repeated list, an unknown or non-wall patch, a face whose action is
  not `Reflect(e)` with `e > 0`, and `angle_std` outside `(0, 30]` degrees. `set_film` refuses a face of an erosion patch
  or a rough face.
- `Tracker::fingerprint` appends the erosion per-face values only when erosion is set, and the rough counts only when the
  rough wall is set. No field is added to `Particle`, `Flow`, `Options` or `InjectorSpec`.

**S2. Case tree.** Two optional objects (`.optional().hash_omit_default()`), absent from the data until set:

| Key | Default | Meaning |
| --- | --- | --- |
| `particles.erosion.enabled` | `false` | book the erosion of every parcel hit on the erosion patches |
| `particles.erosion.patches` | `[]` | wall patches where hits are booked (required, each once) |
| `particles.erosion.k` | `-1` | `K` of DNV eq. 7.1 [(m/s)^-n], must be set (> 0) |
| `particles.erosion.n` | `-1` | velocity exponent `n`, must be set (> 0) |
| `particles.erosion.density` | `-1` | target material density [kg/m^3] for the depth rate, must be set (> 0) |
| `particles.rough_wall.enabled` | `false` | reflect on the rough patches about a randomly tilted virtual wall |
| `particles.rough_wall.patches` | `[]` | reflecting wall patches that are rough (required, each once) |
| `particles.rough_wall.angle_std` | `-1` | standard deviation `Delta_gamma` of the roughness angle [degrees], in `(0, 30]` |
| `particles.rough_wall.seed` | `1` | seed of the roughness random numbers (>= 0) |

Located errors, checked without the mesh: either object enabled without `particles.enabled`; a patch list empty, with an
empty name or a repeated name; an erosion or rough patch that is also a film patch (with the film enabled); `k`, `n`,
`density` not above 0; `angle_std` outside `(0, 30]`; a rough patch whose action is not a reflection (its
`particles.walls.<patch>.interaction` is `stick` or `escape`, or it has no entry and `particles.wall_interaction` is
`stick` or `escape`) or whose restitution (the entry's, else `particles.restitution`) is 0. Checked with the mesh, by
`build_tracker`: an unknown patch, a patch that is not a wall. The keys of `particles.walls`, `wall_interaction` and
`restitution` keep their values and meaning; the restitution of a rough patch is its `e`.

**S3. Run path.** Only when erosion is on: `particle_report.json` gains `erosion {faces, hits, impact_mass, eroded,
max_rate, max_depth_rate}` and the run writes `particle_erosion.csv`, one row per erosion face
(`face,patch,x,y,z,area,hits,impact_mass,eroded,rate,depth_rate`, rates over the report's `time`). Only when the rough
wall is on: the report gains `rough_wall {collisions, redraws, fallbacks, normal_incidence, grazing}`. Without them every
output is unchanged (R2, R4).

## Registered checks

All tests run in release. Library checks are in `crates/rcfd-particles/tests/wall_interaction.rs`; case-tree and run-path
checks in `crates/rcfd-api/tests/wall_interaction_f254.rs`. "Relative" is `abs(a - b) / abs(b)`. In every case below the
carrier is at rest, the drag is `none`, gravity is zero, so a parcel flies straight at its injected velocity.

**Geometry P (A, B, C).** `box_mesh([6, 4, 2], [0.06, 0.04, 0.02])`, cells of 10 mm; the plate is the patch `ymin` (normal
`-y`). A stream is a point injector at `x_s = x_t - L (cos(alpha), -sin(alpha), 0)`, `L = 0.03` m, aimed at the point
`x_t` on the plate, with the velocity `U (cos(alpha), -sin(alpha), 0)`, constant diameter, `parcel_count = 1`. The plate
reflects (`Reflect(e)`), every other patch is `Escape`, so each parcel hits the plate once and leaves the box.

**A. Closed forms (library).**

- **A1.** `dnv_angle_function` against eq. 7.2 written in the test as the power sum in `(alpha_deg pi/180)`, at every whole
  degree of `[0, 90]`: absolute difference at most 1e-11 (`F` is of order 1, and the eight terms reach about 1600 at 90
  degrees, so rounding alone allows a few 1e-12). `F(0) = 0` exactly; `F >= 0` on a 0.1-degree grid.
- **A2.** `impact_angle` of `v = U (cos(a) t + sin(a) n)` for `n = unit(1, 2, 3)`, `t` a unit vector normal to it,
  `a = 0, 5, ..., 90` degrees: within 1e-13 rad of `a`.

**B. Erosion acceptance (library).** Three streams on the plate of P, `e = 1`, `U = 20` m/s, `d = 2e-4` m, density 2650
kg/m^3: S1 at 15 degrees on the face centred at `(0.045, 0, 0.005)`, 1000 parcels/s; S2 at 35 degrees on `(0.035, 0,
0.015)`, 2000/s; S3 at 90 degrees on `(0.015, 0, 0.005)`, 500/s; all inject from 0 to 0.5 s; 600 steps of 0.001 s.
Erosion on `ymin` with `K = 2.0e-9`, `n = 2.6`, `rho_t = 7800` (the steel grades of Table 7-2, as inputs). `N_k =
floor(rate_k 0.5)` = 500, 1000, 250; `m = 2650 pi (2e-4)^3 / 6`; `A = 1e-4` m^2; `T` the tracker's time at the end.

- **B1 (acceptance).** For each stream, the reported rate of its face equals `G K U^n F(alpha)` with `G = N_k m / (A T)`,
  `U` and `alpha` as injected and `F` by the power sum of A1: relative 1e-10.
- **B2.** The target faces have `N_k` hits and impact mass `N_k m` (relative 1e-12); the other 9 plate faces have no hit
  and no eroded mass; the depth rate is the rate over 7800 (relative 1e-15).
- **B3.** The same run without erosion gives the same particle part of the fingerprint, bit for bit (erosion is an
  observation).

**C. Rough-wall acceptance (library).** One stream per run on the plate of P aimed at `(0.035, 0, 0.005)`, `U = 10` m/s,
`d = 1e-4` m, density 2500, 20 000 parcels/s from 0 to 1 s (`n = 20 000` parcels), 1100 steps of 0.001 s; the rough wall
on `ymin` with `angle_std = 8` degrees and `seed = 254`. The rebound angle of each parcel is measured from its final
velocity (it escapes with its rebound velocity): `alpha_r = atan2(-v . n, v . t)`, `n = (0, -1, 0)`, `t = (1, 0, 0)`.
`K_0.01` is the root of `2 sum_{k=1..100} (-1)^(k-1) exp(-2 k^2 x^2) = 0.01` (about 1.6276), and the critical value is
`K_0.01 / sqrt(n)` (0.01151 for `n = 20 000`); this is the asymptotic value of the source, used as the value for the sample
size. `Phi` is computed in the test by composite Simpson quadrature of the normal density (2 x 10^4 intervals), not by a
library function.

- **C1 (acceptance).** `e = 1`, `alpha` = 10, 40 and 75 degrees (three runs): every parcel escapes; the Kolmogorov distance
  of the `alpha_r` sample from the distribution of W5 (`e = 1`) is below the critical value.
- **C2.** In the runs of C1: `abs(v_z) <= 1e-12 abs(v)` and `abs(v) = U` to 1e-12 relative for every parcel; the counts
  give `n` collisions, no fallback, no normal incidence, no grazing hit.
- **C3 (power).** At 10 degrees, the Kolmogorov distance of the same sample from the untruncated map (`P(alpha_r <= theta)
  = Phi((theta - alpha)/(2 Delta_gamma))`) is above the critical value (prediction: about `Phi(-0.625) = 0.27`): the test
  can tell W4 from the plain Gaussian.
- **C4 (restitution).** `e = 0.8`, `alpha = 40` degrees, 2000 parcels/s from 0 to 1 s: for every parcel, `gamma =
  g(alpha_r)` (bisection on W5) lies in `(gamma_lo, gamma_hi)`, and `abs(v')` equals `U sqrt(cos^2(alpha + gamma) + e^2
  sin^2(alpha + gamma))` to 1e-10 relative.
- **C5 (normal incidence).** `e = 1`, `alpha = 90` degrees: the Kolmogorov distance of the polar angle `psi` (the angle of
  `v'` from `-n`) from the distribution of W5, and that of the azimuth `atan2(v_z, v_x)` (mapped to `[0, 2 pi)`) from the
  uniform one, are each below the critical value; the counts give `n` normal incidences.

A correct sampler fails each Kolmogorov check with probability 0.01 at the fixed seed: the seed and `n` are fixed here,
and a failure becomes a known miss with its numbers, not a new seed.

**D. Case tree and run path.** Case D: `box_2d`, `nx = 6`, `ny = 4`, `lx = 0.06`, `ly = 0.04` (depth 10 mm); frozen flow at
rest with `energy.enabled` (ADR 0192 amendment 1); `particles.gravity = false`, drag `none`, integration `analytic`,
`density = 2650`, `end_time = 0.6`, `dt = 0.001`; `xmin`, `xmax` and `ymax` escape (`particles.walls`); the streams S1,
S2, S3 of B in the plane `z = 0.005` (their targets at `x` = 0.045, 0.035, 0.015), the velocities written with the
shortest round-trip text; `particles.erosion` on `ymin` with the inputs of B.

- **D1 located errors.** Each invalid input of S2 gives a diagnostic at the stated path, from `Case::semantic_errors` or
  from `Simulation::new`; case D (and case D with the rough wall on `ymin`, `angle_std = 8`) gives none.
- **D2 tree equals library.** Case D with the rough wall on `ymin` (`angle_std = 8`, `seed = 254`): `track_frozen`, the
  run path and a tracker built by hand from the same inputs give the same bits (fingerprint with the erosion values and
  the rough counts).
- **D3 (acceptance in the tree).** Case D through the run path: the `rate` column of `particle_erosion.csv` for the three
  target faces equals the closed form of B1 (with `T` the report's `time`, `A` the face area 1e-4) to 1e-10 relative.
- **D4 outputs.** The `erosion` and `rough_wall` objects of the report equal the tracker's values; the CSV has one row per
  `ymin` face, and its `rate` column reads back `eroded / (area time)` to 1e-15 relative.

**E. Defaults.**

- **E1 R3.** FNV-1a 64 of the fingerprint of a tracker with the wall film of ADR 0192 and no F-254 model (the R3 scenario in
  `wall_interaction.rs`: the incline of ADR 0192 geometry G, the film on `zmin`, a falling stream and a stream thrown at
  the reflecting walls with restitution 0.7 and a sticking `xmin`, Schiller-Naumann drag, RK2, 300 steps; on main it
  injects 2760 parcels and ends with 20 active, 417 deposited and 2323 in the film). Recorded on main 54ec840 (Windows):
  `fe94f34ac9fc5dbd`. The film path goes through the hit code F-254 changes.
- **E2 R4.** FNV-1a 64 of the names and bytes of the particle output files of a steady run with per-patch wall actions and
  no F-254 model (the R4 case in `wall_interaction_f254.rs`: a prescribed shear flow in air, gravity, `ymin` sticking,
  `ymax` reflecting with 0.5, `xmax` escaping, a Rosin-Rammler and a fluid-velocity injection, the track log and numbered
  VTP files; on main 132 injected, 31 active, 29 deposited, 72 escaped). Recorded on main 54ec840 (Windows):
  `5514d6332741d3a4` (over `particle_report.json`, `particle_tracks.csv`, `particles.vtp` and `particles_25.vtp` to
  `particles_100.vtp`).
- **E3.** R1 and R2 of ADR 0192 stay equal; `case_hashes_w48u.rs` H1, `baseline_hashes.rs` and the particle suites
  (`closed_forms`, `lagrangian_ext`, `wall_film`, `particles_case`, `particles_ext_case`, `particle_run`,
  `particles_two_way`, `wall_film_f253`) pass unchanged; no `DEFAULT_KEYS` entry is added.
- **E4.** A case without `particles.erosion` and `particles.rough_wall` has neither key in its data; a case that writes
  either table with `enabled = false` and nothing else has the settings hash of the case without it.

R3 and R4 must stay equal on Windows; they are printed elsewhere, because `exp` may differ in the last bits between
platforms (ADR 0137).

## Not delivered

Erosion correlations other than DNV RP O501 eq. 7.1 to 7.3 (Finnie, Oka, E/CRC and others), the brittle angle function,
erosion that changes the wall shape or the mesh, erosion under the wall film, wall friction and particle rotation at the
wall, roughness out of the plane of incidence, a roughness that depends on the particle size, the shadow-effect
distribution of Sommerfeld and Huber, splash, rebound and breakup of liquid parcels at the wall film (ADR 0192 Q4),
comparison with measured erosion or rebound data, the erosion and roughness state in a restart.

## Open questions and choices for the maintainer

- **Q1. Status of F-254.** If every registered check is met, the acceptance is met on the branch. Marking F-254 done is
  the maintainer's decision; the roadmap says partial until then.
- **Q2. The shadow effect.** W4 truncates the Gaussian to the realizable angles; the effective distribution of Sommerfeld
  and Huber (1999) was not read. Options: (a) keep W4 as the model; (b) obtain the paper and add its distribution as a
  second value of a new key, registered with its own checks. Recommendation: (b) once the paper can be read; (a) until
  then.
- **Q3. K, n and the target density.** Options: (a) inputs with no default, as registered, with Table 7-2 in the docs; (b)
  defaults to the steel grades of Table 7-2. Recommendation: (a): DNV says the constants have to be found by experiment.
- **Q4. The DNV edition.** The copy read is Revision 4.2-2007 (amended 2011) from a third-party host; the current edition
  (DNVGL-RP-O501, 2015 and later) was not read and may change the constants. Recommendation: check the constants against
  the edition the maintainer holds before relying on absolute rates.
- **Q5. Impact regimes at the wall film (ADR 0192 Q4).** Not delivered here. Options: (a) amend F-254 with an acceptance for
  them; (b) a new item. Recommendation: (b), because they need their own sources (impact regime maps) and checks.
- **Q6. Friction and rotation.** Tian et al. eqs. 1 to 4 need a particle rotation state that the tracker does not keep.
  Options: (a) a new item; (b) nothing for now. Recommendation: (b) until a case needs it.

## Addendum 1: results (2026-11-06)

Measured against the limits registered above; no limit was changed and no check was run before its code was complete.
Release builds on Windows: the library checks on commit c0f1868, the case-tree checks on edd0364 (the library checks ran
again there with the same numbers). Each registered check ran once on its code, and the Kolmogorov checks ran with the
registered seed 254 only.

| Check | Limit | Measured | Verdict |
| --- | --- | --- | --- |
| A1 `dnv_angle_function` against eq. 7.2 at whole degrees; `F(0)`; smallest `F` on the 0.1-degree grid | 1e-11; 0; `>= 0` | 4.2e-13; 0; 0 (at 0); `F(36.4 deg)` 1.00186, `F(90 deg)` 0.56851 | met |
| A2 impact angle on the tilted face | 1e-13 rad | 2.2e-16 rad | met |
| B1 reported rate against `G K U^n F(alpha)` at 15, 35, 90 degrees | 1e-10 | 1.1e-14, 1.1e-14, 5.4e-13 | met |
| B2 hits; impact mass; the 9 other faces; depth rate | `N_k`; 1e-12; no hit, no erosion; 1e-15 | 500, 1000, 250; 4.0e-15; none, 0; 0 | met |
| B3 particle fingerprint with and without erosion | bit for bit | equal (22 752 words; 22 788 with erosion) | met |
| C1 Kolmogorov distance at 10, 40, 75 degrees (`n = 20 000`) | below 0.011509 | 0.004746, 0.005930, 0.005821 | met |
| C2 out-of-plane part; speed; counts | 1e-12; 1e-12; `n`, 0, 0, 0 | 0; 7.1e-16; 20 000 collisions, no fallback, normal incidence or grazing hit | met |
| C3 distance from the untruncated map at 10 degrees | above 0.011509 | 0.266 (predicted 0.27) | met |
| C4 `e = 0.8`, 40 degrees: `gamma` inside `(gamma_lo, gamma_hi)`; speed | all; 1e-10 | all 2000 inside (-17.95, 67.95) degrees; 6.3e-16 | met |
| C5 normal incidence: polar angle, azimuth; counts | below 0.011509; `n` | 0.004504, 0.003759; 20 000 | met |
| D1 21 invalid inputs; case D with and without the rough wall | each at its path; none | each at its path; none | met |
| D2 `track_frozen`, run path and tracker by hand (fingerprint with erosion and rough counts) | bit for bit | equal (22 775 words; 1750 parcels, 1750 rough collisions, 118 redraws, 250 normal incidences) | met |
| D3 `rate` column of `particle_erosion.csv` against the closed form | 1e-10 | 1.1e-14, 1.1e-14, 5.4e-13 | met |
| D4 report objects; CSV rows; `rate` read back | equal; 6; 1e-15 | equal; 6; 0 | met |
| E1 R3 | `fe94f34ac9fc5dbd` | equal | met |
| E2 R4 | `5514d6332741d3a4` | equal | met |
| E3 R1, R2 of ADR 0192; H1; `baseline_hashes`; the particle suites | unchanged | equal; pass; pass; pass | met |
| E4 no key in the data; settings hash with an inert table | equal | equal (`4b830c0bb7976b35`, both tables) | met |

The accepted fractions of the draws in C1 were 0.7322, 0.99384 and 1.000000, against 0.7340, 0.99379 and 0.999999 for the
truncated Gaussian of W4: consistent, and not a registered check.

Disclosures:

- The difference of 4.2e-13 in A1 and the 5.4e-13 at 90 degrees in B1 and D3 come from the cancellation in the
  alternating sum of eq. 7.2, whose terms reach about 1600 at 90 degrees for a value of 0.57; the code evaluates it in
  Horner form, the test as printed.
- The Kolmogorov checks use the asymptotic critical value `K_0.01 / sqrt(n)` of the source as the value for the sample
  size; with a correct sampler each fails with probability 0.01 at the fixed seed.
- C1 checks the distribution for `e = 1` only; `e < 1` is checked per parcel by C4, without a statistical test.
- D1 inputs 4 and 12 (an erosion or rough patch that carries the film) also give the film's own error at
  `particles.gravity`, because case D has no gravity; the registered diagnostic is present in both.
- Not exercised by a test: the fallback after 1000 rejected draws and the grazing branch of W4 (no registered case
  reaches them), the library refusals of a second `set_erosion` or `set_rough_wall` call, and the refusals between the film
  and the new models inside the library (the case tree refuses the overlaps first, D1).
- The records changed before this text was registered, all on main before any code of F-254: R3's thrown stream first
  missed the sticking wall (no parcel deposited, its set-up assertion failed) and was aimed at it; R4's case was first a
  steady SIMPLE lid-driven box, which diverged on main at iteration 14 (with `linear_upwind` and with `upwind`), then a
  frozen shear flow in which the parcels were pressed against a reflecting wall until stuck (10 s of run time), and was
  changed to the registered air-like case.
- The tests take about a second each in release (`wall_interaction` 1.3 s, `wall_interaction_f254` 0.2 s).
