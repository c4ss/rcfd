# ADR 0768: F-882 registration: the loss of total pressure of the simulated FAN-01 rotor, where it arises and what a finer mesh makes of it

Status: **REGISTERED 2026-11-22, written before any code of F-882 and before any run of its checks.** Before this text no
test of F-882 was written, no solver was run, no mesh of this work was built and no cargo command was run. What was done:
reading (below), one reading script over `docs/roadmap.json`, and the arithmetic on numbers already recorded in the
repository (section "Arithmetic done beforehand", script `pre.py` in the scratch folder). Results go in addenda at the end;
nothing above them is changed after a run.

It answers the brief `docs/adr/briefs/f882-fan01-rotor-loss.md` and follows `docs/process/agent-rules.md`, version 5.
Branch `f882-fan01-rotor-loss` from main 21054b68; scratch `w49i-work/f882-fan01-rotor-loss-scratch`. No solver source is
changed by this work: it adds one test file, its mutation patch, text and the proposal file.

Size (agent): partial; 0; 4; 1; 350000

The expected outcome is partial, and the registration says why before the run: the loss of the recorded family falls
with an observed order of 1.43 (B3, B2, B1; arithmetic below), under the range that the tolerance table allows for an
extrapolation of a formally second-order scheme, and nothing known gives a reason for the finer mesh to bring the
sequence into that range. The four heavy runs are B0 (the finer mesh, new) and B1, B2, B3 again, because the fields of the
recorded solves were not kept and the division needs the field.

## What is asked

Roadmap item F-882 (read by script from `docs/roadmap.json`). Its acceptance: "Every limit not stated here is registered
in the ADR before the run. The loss of total pressure of the FAN-01 design-point solve between the inlet and the measured
plane of ADR 0765 (62.87 Pa on B1, 78.75 Pa on B2, 104.34 Pa on B3) is divided, by a rule registered before the run, into
the parts that arise at the blade surfaces, at the hub and casing, in the tip gap and in the interior of the passage, and
is recomputed on one mesh finer than B1 that also refines the wall-normal direction; the report states the loss the
family extrapolates to with its band and what that leaves of the missing pressure rise of F-870 for the installation by
the identity of ADR 0765. A run that does not decide is recorded as not deciding."

The brief's four tasks: divide the loss by a registered rule and show that the parts sum to the whole within a
registered tolerance; one mesh finer than B1 that also refines the wall-normal direction, within the mesh gates of ADR
0762, run at the design point; extrapolate the loss over the family with a band by a registered rule (observed order,
conditions of validity) and state what the identity of ADR 0765 then leaves for the installation; hold it with a fast
test on the gate and the heavy runs as records.

## What was read

Whole: the brief; `AGENTS.md`; `docs/process/agent-rules.md` (version 5);
[ADR 0765](0765-f878-registration-fan01-lda-profiles-euler-work-and-the-division-of-the-missing-pressure-rise.md);
[ADR 0762](0762-f870-registration-fan01-design-point-state-band-and-ranked-gap.md);
`crates/rcfd-api/tests/f878_fan01_lda_profiles.rs`; `docs/process/tolerances.md`; `docs/adr/briefs/README.md` lines 1
to 25; `docs/roadmap/proposals/0765-F-878.json`. Of `docs/roadmap.json`, by script: F-870, F-878, F-880, F-882 and F-883.
Of [ADR 0763](0763-f869-registration-the-mesh-converged-mean-drag-of-the-cylinder-at-re-100.md): lines 36 to 160 (its
Richardson procedure for unequal ratios and its conditions of validity, which this registration takes over). Of the
investigation folder of ADR 0765 (`docs/investigations/f878-fan01-lda-profiles/`): the result lines and the lines
`F878H` of `B1.log`, `B2.log`, `B3.log`. Of the source, to learn what a test can read from a solve and from a mesh and no
more: the list of public functions of `crates/rcfd-api/src/sim.rs` and the bodies of `velocity`, `pressure`, `flux`,
`momentum_face_viscosity` and `field`; the fields of `FvGeometry` (`crates/rcfd-fv/src/geometry.rs` lines 12 to 45, 106
to 113, 162 to 169); the use of `nu_face` and `lap_coeff` in the momentum predictor of `crates/rcfd-fv/src/simple.rs`
(lines 780 to 831); the public functions of `crates/rcfd-mesh/src/mesh.rs` and the types `Patch`, `PeriodicRotation`;
lines 1 to 160 of `crates/rcfd-mesh/src/fan_cgrid.rs` (the block structure, the core block in the tip gap); the module
text of `crates/rcfd-turb/src/sst.rs` lines 20 to 40 (the wall treatment); `y_plus_lam` in `crates/rcfd-turb/src/wall.rs`;
`gci_fine` in `crates/rcfd-api/tests/validation_common/mod.rs`.

Not read: the other FAN-01 ADRs and tests (known through the tables of ADR 0762 and ADR 0765); `docs/milestone-fan.md`;
the meshing page beyond what ADR 0762 records of its gates; any paper. No published value is used that is not already
recorded in the repository: the measured terms are those of ADR 0765, the gates those of ADR 0762.

## Arithmetic done beforehand (recorded numbers only)

From the recorded lines of the three solves of ADR 0765 (pointwise reading, x = 90 mm, 8000 iterations), the loss
`L = e - q - Pi` over the measured span (`L_s`) and over the whole annulus (`L_ann`, midpoint rule on 126 rings):

| | B3 (4 920 cells) | B2 (13 627) | B1 (38 184) |
| --- | --- | --- | --- |
| `L_s` [Pa] | 104.342 | 78.755 | 62.872 |
| `L_ann` [Pa] | 102.083 | 75.954 | 63.600 |

With `h` proportional to `N^(-1/3)` the ratios are 1.40436 (B3 to B2) and 1.40981 (B2 to B1). The observed order of the
triplet by the rule below is **1.430** for `L_s` (Richardson value 37.83 Pa; 24.12 Pa at order 1, 46.79 Pa at order 2)
and 2.228 for `L_ann` (52.85 Pa). The two orders differ by 0.8 on two readings of one loss: the triplet is not in an
asymptotic range that I could show, and B3 is coarse. These numbers are known to me as I write and decide nothing; they
are why the expected outcome is partial and why the extrapolation below uses the three finest meshes.

The identity of ADR 0765 with its recorded terms: `D = 126.46 - 73.9881 = 52.4719 Pa`; `e_m - q_m = 220.015 - 99.196 =
120.819 Pa` (pointwise reading of the measurement; the profile reading gives 124.089 Pa, so `u_data = 3.270 Pa`);
`Pi_s` of B1 = 61.7094 Pa; the band terms `S + T + u_cov` of `Pi_s` of B1 sum to 0.4030 Pa (ADR 0765, addendum 1).

## The design, from first principles

**The loss as a drop of rothalpy.** The solve is steady and incompressible in the frame that turns with the rotor, with
the relative velocity `w` as unknown, the kinematic static pressure `p` (the centrifugal term is part of the frame
source, ADR 0765), Coriolis and centrifugal forces as sources. Write `I = p + |w|^2 / 2 - omega^2 r^2 / 2` (kinematic). With
`c = w + omega e_z x r`, `|w|^2 = |c|^2 - 2 omega r c_t + omega^2 r^2`, so `I = p + |c|^2 / 2 - omega r c_t`: the total
pressure less the Euler term. At the inlet `c_t = 0` and `I = p0_in`. The flux-weighted mean of `rho (p0_in - I)` over a
plane is therefore `e - (p0 - p0_in)`, which is ADR 0765's `L = e - q - Pi` (`e` the Euler head, `q` the kinetic energy,
`Pi` the static pressure against `p0_in`, all per volume flow). So **the loss up to a plane is the drop of the flux of
rothalpy from the inlet to that plane.**

Dotting the momentum equation of the relative frame with `w`: the Coriolis force does no work (`(Omega x w) . w = 0`) and
the centrifugal force is the gradient of `omega^2 r^2 / 2`, so `div(w I) = div(tau . w) - Phi` with `tau` the effective
viscous stress and `Phi >= 0` its dissipation. Integrated over any region, the drop of the convected rothalpy plus the
work of the stress on the region's boundary is what is lost in it. Blade and hub are at rest in this frame (`w = 0`), so
their stress does no work; the casing moves (`w = -omega r e_t`) and its work enters the region next to it. That is the
reason a part of the loss can be negative near the casing: in this frame the casing drives the fluid, and its friction
torque is already out of the Euler head `e` at the plane (ADR 0765, moment of momentum).

**The discrete budget (fixed now).** The control volume `V` is the set of cells whose centre has `z < z_P`, with `z_P =
0.16855 m` the plane x = 90 mm of ADR 0765 (`z_mesh = 0.1 m + (x - 21.45 mm)`). Its boundary is the inlet, the walls, the
seams and the faces between a cell of `V` and a cell outside it (the "plane faces"; a staircase that follows the cells).
For every face `f` with outward flux `phi_f` (the solver's face flux, `Simulation::flux`) and its two cells P, N:

* the face rothalpy `I_f = lambda_f I_P + (1 - lambda_f) I_N`, with `lambda_f` the interpolation weight of
  `FvGeometry::weights`, `I_c = p_c + |w_c|^2 / 2 - omega^2 r_c^2 / 2 - p0_in` per cell (`p0_in = p_in + u_in^2 / 2` as
  ADR 0762 defines the inlet total pressure; the offset makes the inlet term near zero and removes the arbitrary level of
  `p` from the parts); on a boundary face the boundary values of `p` (`pressure_face_values`) and `w`;
* the work of the stress across an internal face, from P to N, `W_f = nu_f a_f (w_N - w_P) . w_f`, with `nu_f` the face
  viscosity of the momentum equation (`momentum_face_viscosity`, laminar plus turbulent), `a_f` the coefficient
  `lap_coeff` of the face (the orthogonal part of the solver's diffusion flux) and `w_f` interpolated like `I_f`;
* a seam face of `seam_lo` and its partner (`FvGeometry::periodic_partner`) are one face between the two owners: the flux
  is that of the `seam_lo` face, the partner's vectors are turned into the `seam_lo` frame (`periodic_rot`), `a_f =
  |S_f| / (dn_lo + dn_hi)`. The largest `abs(phi_lo + phi_hi)` of the solver is reported.

The part of the loss of cell P is `lambda_P = rho sum_f (-phi_f I_f + W_f)` over its faces, where `W_f` counts only on
faces between two cells of `V` (on a wall, at the inlet and on the plane faces only the convective term counts). Summed
over any union of cells the internal faces cancel; summed over `V` what is left is `rho` times the drop of the rothalpy
flux through the boundary of `V`, the discrete form of `L` times the volume flow. **The whole** is `L_V = sum_V lambda_P /
Q_in` with `Q_in` the inlet volume flow of the sector, in Pa, and **the parts** are the sums over the four regions below,
divided by the same `Q_in`. That the parts sum to the whole is then an identity of the discrete budget, and the check
(B1 of the table) is that the sum over the regions equals the sum over the boundary faces of `V`, computed by the two
routes separately: it catches a cell counted twice or not at all and a face term of the wrong sign.

The whole of the budget and the sampled loss of ADR 0765 are two readings of the loss up to the same place: one through
the staircase of cell faces with the face values above, the other through the plane at the cell values of the points.
The difference is reported for every mesh (check B2); I know no source for a limit on it and set none.

**The regions (fixed now).** Every cell of `V` is in exactly one, taken in this order:

1. **tip gap**: the centre is above the blade tip radius (`r_c > 0.2475 m`) and the point at `r = 0.2475 m - 1 mm` with the
   same `theta` and `z` lies in the blade, that is in no fluid cell (the test of ADR 0765: the smallest largest signed
   distance from a cell's face planes above 0.1 mm). This is the gap between the blade tip and the casing as meshed,
   the core block of the C-grid and the cells over the blade that the radial grading puts there;
2. **blade surfaces**: the distance of the centre from the blade, `d_b`, is at most `delta` and at most the distance from
   hub and casing. `d_b` is the exact distance from the centre to the faceted blade surface (each face of the patch
   `blade` split into triangles from its centre), taken over the blade of the sector and its two images turned by plus
   and minus one pitch;
3. **hub and casing**: the smaller of `r_c - 0.124 m` and `0.250 m - r_c` is at most `delta`;
4. **passage interior**: every other cell of `V`, the inlet duct included.

`delta = 2.5 mm`, the tip clearance: the one length of the geometry that is the size of the layers next to walls, so
that the casing layer and the gap have one thickness. On every mesh of the family it holds the two exact stack cells
of the wall (the first cell 0.565 mm on B1 by the y+ rule, ADR 0762). The division is reported also at `delta = 1.25 mm`
and `5 mm` (half and twice), so that its dependence on the choice is seen; the parts at 2.5 mm are the result.
Reported inside the parts, without a rule of their own: hub and casing apart; the interior upstream of `z = 0.1 m` (the
inlet duct ahead of every leading edge) apart; and for each part the convective and the stress-work shares.

**The finer mesh, B0 (fixed now).** The family of ADR 0086 refines in the plane and in the span by about 1.41 per step
(B2 to B1: 1.40981 in `N^(1/3)`) and keeps the wall layers: the first cell by `y+ = 30`, `n_bl = 2`, growth 1.2 on every
mesh. B0 is B1 refined once more by `r_0 = 1.41` in every count and, as the acceptance asks, in the wall-normal
direction too:

* counts `(n_up, n_chord, n_down, n_side, n_nose, n_span, n_gap) = (17, 34, 17, 21, 8, 30, 14)`, each `1.41` times B1's
  (12, 24, 12, 15, 6, 21, 10) rounded to the nearest integer; `n_core = 1` and `front_gap = 0.3` as B1;
* the wall layers: first cell at `y+ = 30 / 1.41 = 21.277`, `n_bl = 3` (`2 x 1.41 = 2.82`, rounded), growth `1.2^(1 /
  1.41) = 1.13797`, so that the first cell is smaller by 1.41 and the exact stack keeps about its thickness (2.43 against
  2.20 first-cell heights of B1). The wall treatment of the SST model in this repository is Spalding's law, valid from
  `y+` about 1 to the log layer (`crates/rcfd-turb/src/sst.rs`, module text lines 25 to 29), so the smaller first cell
  stays inside the model's stated range;
* everything else of the case text as B1 (`f870_fan01_design_point.rs`, copied without a change of value through
  `f878_fan01_lda_profiles.rs`): 1486 rpm, 1.4032 m^3/s, `rho = 1.2` assumed, SST, SIMPLEC, limited `linear_upwind`,
  tolerances 1e-5, 8000 iterations, no wall-clock stop, the stops of ADR 0762.

Expected about 107 000 cells. If the generator rejects this specification, an amendment written before any run corrects
it; the counts are not searched for.

**The mesh gates** of ADR 0762 (largest non-orthogonality <= 70 deg, skewness <= 2.0, aspect ratio <= 300, by
`rcfd_mesh::quality::check_mesh`) are evaluated on B0 and recorded with their verdicts. B1 misses the first two (83.37
deg, 3.765), and the non-orthogonality has risen with each refinement of the family (78.4, 79.5, 83.4 deg); the
construction is the same, so I expect B0 to miss them too. The brief's task 2 says "within the mesh gates of ADR 0762";
this generator cannot meet them (F-133), and I read it as "evaluated against the gates and recorded", taken on the
maintainer's standing instruction of 2026-11-11 (take the recommendation). A miss of a gate is recorded as a miss; the
decision rule below is the brief's and does not contain the gates.

**The sampled loss.** On every mesh `L_s` and `L_ann` are formed exactly as ADR 0765 forms them (its points, its
locator, the cell's absolute velocity of its amendment 1, the trapezoid rule on the 26 radii, the midpoint rule on 126
rings), at x = 88.7, 90 and 91.3 mm and at the iterations 7000, 7250, 7500, 7750 and 8000. The code is copied from
`f878_fan01_lda_profiles.rs` without a change.

**The extrapolation (fixed now).** The quantity extrapolated for the decision is `L_s`, the loss over the measured span
at x = 90 mm, because it is the `L` of the identity of ADR 0765. The three finest meshes are used: `f1 = L_s(B2)`, `f2 =
L_s(B1)`, `f3 = L_s(B0)` at 8000 iterations, `h = N^(-1/3)`, `r21 = (N_B1 / N_B2)^(1/3) = 1.40981`, `r32 = (N_B0 /
N_B1)^(1/3)` from the cell count of B0. With `f_i = f_0 + C h_i^p`, `f2 - f3 = C h3^p (r32^p - 1)` and `f1 - f2 = C h3^p
r32^p (r21^p - 1)`, so the observed order `p` is the root of `g(p) = r32^p (r21^p - 1) / (r32^p - 1) = (f1 - f2) / (f2 -
f3)`, found by bisection on [0.01, 20], and the extrapolated loss is `L_0 = f3 + (f3 - f2) / (r32^p - 1)` (ADR 0763, its
section "The Richardson procedure"). Its band:

`U_L = 1.25 abs(f3 - f2) / (r32^p_b - 1) + S + T`, `p_b = min(max(p, 1), 2)`,

the first term Roache's index of the fine value with the factor of safety 1.25 and the order limited to [1, 2] as
`gci_fine` of `validation_common` and ADR 0060 lines 196 to 198 define it, in Pa; `S` half the range of `L_s(B0)` over
the five iterations, `T` half its range over the three planes, as ADR 0765 defines them. Stated: the loss at mesh
convergence is `L_0`, and lies in `[f3 - U_L, f3 + U_L]`.

The extrapolation is **valid** only if both hold:

* (a) the sequence is monotone, `(f1 - f2)(f2 - f3) > 0`, and the bisection has a root;
* (b) the observed order is in **[1.5, 2.5]**, within 0.5 of the formal order 2 of the scheme (row "Observed order of
  accuracy" of the [tolerance table](../process/tolerances.md)). The formal order is that of limited `linear_upwind`
  with central diffusion away from the limiter; the limiter and the wall functions can lower it, and that is what (b)
  tests.

If it is not valid, the loss at mesh convergence is **not shown**: the number the formula gives is printed and marked
not valid, and beside it the values at order 1 and order 2, `f3 + (f3 - f2) / (r32 - 1)` and `f3 + (f3 - f2) / (r32^2 -
1)`; none of them is then a result. The order range is not widened after the run. Reported beside, without a rule: the
same procedure for `L_ann`, for the whole of the budget and for each of its four parts, and for the triplet B3, B2, B1.

**What it leaves for the installation (fixed now).** By the identity of ADR 0765, `dp_ts = Pi + Rec` and `Pi = e - q -
L`. If the measured rotor loses what the simulated rotor loses at mesh convergence (`L_m = L_0`, a condition nothing in
the record tests), then the rotor delivers `Pi_m = (e_m - q_m) - L_0` at the plane, and

* the rig must recover behind the plane `Rec_m = 126.46 - Pi_m = L_0 + 5.641 Pa`;
* the rotor's share of the missing pressure rise of F-870 is `R_0 = Pi_m - Pi_s(B1) = 59.110 Pa - L_0`, and **the
  installation's share is `I_0 = D - R_0 = L_0 - 6.638 Pa`**, with `D` and `Pi_s` of B1 as ADR 0765 writes the identity;
* the band `U_I = U_L + 3.270 Pa (u_data) + 0.403 Pa (S + T + u_cov of Pi_s of B1)`, the terms of ADR 0765.

Reported beside: the same with `D` and `Pi_s` of B0 in the place of B1's, and `Rec_m` against `q_m = 99.196 Pa`, the most
the rig can recover.

## The runs (fixed now)

Four solves, each made once to 8000 iterations: B0 in one process, by the heavy tests `f882_b0_loss_state_record` and
`f882_acceptance` sharing one solve; B3, B2 and B1 one after the other in a second process, by `f882_family_run`. The two
processes run at the same time. Each solve forms the budget and the sampled loss at the five iterations and prints them.
No limit on wall-clock time; I expect three to five hours for B0 on the shared machine and about 1.7 hours for the three
others.

## The test that holds it (fixed now)

One new file, `crates/rcfd-api/tests/f882_fan01_rotor_loss.rs` (header: ADR 0768, F-882), with one path
`run_design_point` that every solving test calls. The files `f870_fan01_design_point.rs` and
`f878_fan01_lda_profiles.rs` are not changed.

* Run by the gate:
  * `f882_rules_closed_forms`: on the B3 mesh, without a solve: the distance from a point to a triangle against closed
    forms (above the interior, beyond an edge, beyond a corner); the regions partition `V` (every cell of `V` in exactly
    one, the tip gap not empty and above the tip radius); a uniform state (constant `I`, the flux of a uniform axial
    velocity `phi_f = U . S_f`, uniform `w`) gives every part zero; the two routes of the budget agree on a field of
    random values; the observed order and the Richardson value of an exact power law on unequal ratios are recovered.
  * `f882_b0_mesh_gates_as_recorded`: builds B0; its cell count equal and its three quality numbers within 0.5 % of the
    recorded ones (those of its first run), and each gate's verdict as recorded.
  * `f882_b3_short_run_holds_the_budget`: B3 for 300 iterations on the path of the heavy runs; the two routes agree
    within 1e-8; `L_s`, the whole and the four parts within 1 % of the recorded whole of the values of its first run.
  * `f882_recorded_family_gives_the_recorded_verdicts`: from the recorded values the observed order, the validity, `L_0`,
    `U_L`, `I_0` and `U_I` are formed again and equal the recorded ones.
* Heavy records (`#[ignore]`):
  * `f882_b0_loss_state_record`: B0 to 8000 iterations; torque, pressure rise, `L_s`, the whole and the four parts
    within the tolerance of the state of the recorded ones. Its first run is the registered run and fails, as planned,
    while nothing is recorded; the values are then recorded from it.
  * `f882_acceptance`: the decision rule below as a test, on the B0 solve with the recorded B2 and B1 values of ADR 0765.
    If it misses it is renamed `..._known_miss` with the numbers in its ignore reason (rule 2).
  * `f882_family_run`: one of B1, B2, B3 by the environment variable `F882_RUN`, with check R1.

**The tolerance of the state**: 1 % of the recorded value (ADR 0762 and ADR 0765: twice the 0.5 % to which the cap rule
defines a functional of this case); for a part of the loss, 1 % of the recorded whole, because a part can be small or
change sign while it is a share of the whole.

## Table of checks

Every clause of the acceptance first. "Estimate" is what I expect; where I have no basis for a number I say so.

| Id | Check | Limit | Source of the limit | Estimate | Margin | Kind |
| --- | --- | --- | --- | --- | --- | --- |
| A1 | the division: the loss up to x = 90 mm divided by the region rule into blade surfaces, hub and casing, tip gap, passage interior, on B3, B2, B1 and B0 | the parts sum to the whole by the two routes within a relative `1e-8` on each mesh (B1 below) | acceptance of F-882 ("divided, by a rule registered before the run"); the tolerance table, row "Same discrete problem by two routes" | 1e-14 | 1e6 | solution verification |
| A2 | recomputed on B0, the mesh finer than B1 in every direction and at the wall | the solve reaches 8000 iterations without a stop of ADR 0762 | acceptance of F-882; the stops of ADR 0762 | reached (B1 reached it) | n/a | solution verification |
| A3 | the extrapolation of `L_s` over B2, B1, B0 | valid iff (a) monotone with a root and (b) `p` in [1.5, 2.5] | the tolerance table, row "Observed order of accuracy"; the rule of ADR 0763 | `p` about 1.4 (the recorded triplet gives 1.43) | below 1: expected MISS of (b) | solution verification |
| A4 | the loss the family extrapolates to with its band, and what the identity leaves for the installation: `L_0`, `U_L`, `Rec_m`, `I_0`, `U_I` | the rule above; given as a result only if A3 is valid | acceptance of F-882 ("states the loss the family extrapolates to with its band and what that leaves") | `L_0` 25 to 45 Pa, `I_0` 18 to 38 Pa, if valid | n/a | validation |
| A5 | a run that does not decide is recorded as not deciding: `f882_acceptance` fails on a missed clause and is kept as a known miss with its numbers | as stated | acceptance of F-882 (last sentence); rule 2 | as stated | n/a | validation |
| B1 | the parts sum to the whole: the sum over the regions against the sum over the boundary faces of `V` | relative difference `<= 1e-8` on every mesh and every sampled iteration | the tolerance table, row "Same discrete problem by two routes" | 1e-14 | 1e6 | code verification |
| B2 | the whole of the budget `L_V` against the sampled `L_ann` at x = 90 mm, on each mesh | none (report): no row of the tolerance table covers it and no source I read does | - | within 5 Pa on B1 | n/a | solution verification |
| G1 | the mesh gates of ADR 0762 on B0: largest non-orthogonality, skewness, aspect ratio | `<= 70 deg`, `<= 2.0`, `<= 300` | ADR 0762, its section "Mesh quality gates" | about 85 deg, about 4, about 260 | below 1, below 1, above 1: expected MISS of two | solution verification |
| R1 | B3, B2, B1 are the recorded states: torque and pressure rise against ADR 0762, `L_s` against ADR 0765 (104.342, 78.755, 62.872 Pa) | 1 % each | ADR 0762 and ADR 0765, the tolerance of the state | 0 | n/a | unchanged default output |
| R2 | the parts on the four meshes at `delta` 2.5, 1.25 and 5 mm; their spread `S` over the cycle and `T` over the planes; hub and casing apart; the inlet duct apart; convective and stress-work shares; the seam flux; the extrapolation of `L_ann`, of `L_V` and of each part, and of the triplet B3, B2, B1 | none (report) | - | the interior and blade parts fall most with the mesh; no number | n/a | solution verification |
| V1 | distance from a point to a triangle: three closed forms | absolute error `<= 1e-12` m | the tolerance table, row "Agreement with a closed form" (exact for the discrete problem) | 1e-17 | 1e5 | code verification |
| V2 | the regions partition `V` on B3: each cell of `V` in one region; the tip gap has cells, all with `r_c > 0.2475 m` | exact | the region rule (a definition) | exact | n/a | code verification |
| V3 | a uniform state gives every part zero | `abs(part) <= 1e-9 rho Q abs(I)` | the tolerance table, row "Agreement with a closed form" | 1e-15 | 1e6 | code verification |
| V4 | observed order and Richardson value of `f = 2 + 3 h^1.7` on the ratios 1.40981 and 1.41 | relative error `<= 1e-9` | the same row | 1e-12 | 1e3 | code verification |
| S1 | `f882_b3_short_run_holds_the_budget` | closure `1e-8`; values 1 % as above | B1; the tolerance of the state | 0 | n/a | unchanged default output |
| S2 | `f882_recorded_family_gives_the_recorded_verdicts` | equal verdicts; numbers within 5e-3 Pa | this registration (printed digits) | equal | n/a | code verification |
| S3 | `f882_b0_mesh_gates_as_recorded` | cell count equal; each number within 0.5 %; verdicts as recorded | the form of S3 of ADR 0762 | 0 | n/a | unchanged default output |
| D1 | defaults: no file under `crates/*/src` is changed; `f870_fan01_design_point.rs` and `f878_fan01_lda_profiles.rs` are not changed | 0 files | rule 3; the brief | 0 | n/a | unchanged default output |
| P | `cargo fmt -p rcfd-api --check`; `cargo clippy -p rcfd-api --all-targets -- -D warnings`; `cargo test -p rcfd-cli --release --test docs_site`; `python scripts/roadmap_proposals.py check`; `python scripts/process_report.py --check` | clean; passes; no failure but a generated file not current | rule 7 | as stated | n/a | code verification |

A4 and A5 are the rows of kind validation: the measured Euler head and kinetic energy of the record enter A4 through
the identity. No row compares a simulated loss with a measured one, because the record measures none (ADR 0765).

## Negative controls and planned mutations

On a scratch copy of the tree (`git archive` of the branch, its own target folder), each against the gate tests; no
heavy record is rerun for a mutation.

| Id | Mutation | Must be caught by |
| --- | --- | --- |
| M1 | solver: the Coriolis factor of `RotatingFrame::accel` (`crates/rcfd-fv/src/sources.rs`) from -2.0 to -1.9 | `f882_b3_short_run_holds_the_budget` |
| M2 | test: the centrifugal term left out of the rothalpy (`I = p + abs(w)^2 / 2`) | `f882_b3_short_run_holds_the_budget` |
| M3 | test: the sign of the stress work `W_f` | `f882_b3_short_run_holds_the_budget` (the parts move, the whole does not) |
| M4 | test: a tip-gap cell also counted in the blade region | `f882_rules_closed_forms` (partition and two routes) |
| M5 | test: `r21` and `r32` exchanged in the observed order | `f882_rules_closed_forms` (V4) |
| M6 | test: the range of the observed order widened to [0.5, 2.5] | `f882_recorded_family_gives_the_recorded_verdicts`, if the recorded verdict is "not valid" by (b); if it is valid, the range narrowed to [2.4, 2.5] instead |
| K1 | negative control: the B3 short run made twice | equal to the last printed digit |

M2 is committed as `crates/rcfd-api/tests/f882_fan01_rotor_loss.mutation.patch` with the first line
`# expect-fail: cargo test -p rcfd-api --release --test f882_fan01_rotor_loss f882_b3_short_run_holds_the_budget`.

## Decision rule

* **Done** only if the loss is divided by the registered rule on all four meshes with B1 met on each (A1, B1), B0 is run
  (A2), and the extrapolation of `L_s` over B2, B1, B0 is valid (A3); then `L_0`, `U_L`, `I_0` and `U_I` are the result
  (A4).
* **Partial** if the division is made and the run does not decide: the extrapolation is not valid, or the B0 solve
  stops. The numbers, the not-valid values with their reason, and what it would take are reported.
* **Stopped and reported** if a defect of the solver appears, or no specification of B0 by the rule above can be built
  after one amendment.

## What is not covered

* A loss of the measured rotor: the record measures none (ADR 0765). `I_0` holds under the stated condition `L_m = L_0`.
* The division of the loss over the measured span: the budget divides the loss of the whole annulus up to the
  staircase of `V`. Which fluid of the span passes which region is not traced; `L_s` is extrapolated as a whole.
* A converged solve: the limit cycle of ADR 0762 and ADR 0110 stays; `S` says what it does here.
* A refinement study of the wall-normal direction alone: B0 refines it together with the rest, so its effect is not
  separated from that of the in-plane and span refinement, and the step from B1 to B0 is not of the same kind as the
  step from B2 to B1. A valid observed order would not show the family to be systematic in that direction.
* The turbulent kinetic energy and its transport: the budget is that of the mean flow. The shear production of `k` is a
  loss of the mean flow and is in the parts.
* The dissipation resolved by the cell gradients against the numerical one: not separated.
* The mesh gates (expected missed; F-133), the outlet as on the rig (F-880), the wall-pressure data (F-883, another
  agent's work), the density and the uncertainty of the measurement (F-877).
