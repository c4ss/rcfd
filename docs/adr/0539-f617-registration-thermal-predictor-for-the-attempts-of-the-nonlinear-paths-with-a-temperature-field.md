# ADR 0539: F-617 registration: a thermal predictor for the attempts of the nonlinear paths with a temperature field

Status: **REGISTERED 2026-11-15, written before any code of F-617 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-617 was written, compiled or run: the files listed under "What was read" were read, and
the arithmetic below was done by hand (no script, no rcfd run; disclosed). The default-bits record B2 (D8) is a test file written
and run on the unchanged base 8ebc67e8 after this commit and before any library code. Results go in the addenda at the end;
nothing above the addenda is changed after a run.

Roadmap item F-617 (deps F-613), the follow-up proposed by [ADR 0534](0534-f613-registration-nodal-temperature-field-with-thermal-strain-on-the-nonlinear-paths.md)
(its remaining question 1, the known miss FE1c). Number 0539 given by the lead. Branch `f617-work` from 8ebc67e8 (which contains
F-613), worktree `w49i-work/f617`, scratch `w49i-work/f617-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored `*_known_miss`
test with the limit intact and the measured number); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no unsafe code; no existing test
edited; clean-room. The brief asks for the F-617 entry of `docs/ROADMAP.md` and `docs/roadmap.json` to be updated by this branch
in a separate last commit (the svgs not regenerated). Areas of other agents not touched: the contact driver (F-615), the Shell3
model check (F-574), the radiation impedance (F-579). Every choice below that the maintainer would normally be asked about is
taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).

## What F-617 asks

The acceptance, binding:

> Every limit not stated here is registered in the ADR before the run. With an opt-in thermal predictor each attempt with a temperature field starts at the committed displacement plus the free thermal expansion increment of the field's change; the free expansion of ADR 0534 FE1 (a step of alpha dT = 1e-2 from u = 0) converges without halving and equals alpha dT within 1e-12, and runs without the predictor keep the bits of ADR 0534.

The brief adds: register how the free-expansion increment is defined for a nonuniform field (the free expansion of a uniform
field exact), how prescribed displacements are kept, whether the predictor acts on both paths (the quasi-static driver and the
nonlinear transient integrator; "the nonlinear paths"); opt-in; a default-bits record first; `baseline_hashes` unchanged; no
case-file key; mutations the checks catch; every aggregated test error finite.

Clauses: **c0** every limit registered here; **c1** an opt-in predictor on the quasi-static driver; **c2** on the nonlinear
transient integrator; **c3** the start of an attempt is the committed displacement plus the free expansion increment of the
field's change; **c4** FE1 with the predictor: no halving, `alpha dT` within `1e-12`; **c5** runs without the predictor keep the
bits of ADR 0534.

## What was read (no run)

`AGENT_RULES.md`; the brief; ADR 0534 whole (D1 to D8, the arithmetic of the multiplicative thermal stretch `F_th = theta I`,
`theta = 1 + alpha (T - T_ref)`, checks FE1, FE1s, FE1c, FE2, HB1, UB1, UB2, FEi, V3T, addendum 1 with the diagnosis of FE1c);
`thermal.rs` (`TemperatureFieldHistory`, `FieldAt`, `check_field_model`); `nonlinear.rs` (`solve_from_field`, the attempt loop
`attempt_with`, `solve_in_time_with`, `Histories::at`); `nl_transient.rs` (`Integrator::advance`, `substep`, `start`);
`tests/sa_temp_field.rs` and `tests/sa_temp_field_default_bits.rs` (the helpers and the form of the default-bits record).

Facts used from the code, read not run: (a) `solve_from_field` requires the start to carry the prescribed values bit for bit
(it refuses otherwise) and sets the prescribed entries at the first correction; (b) the Newton loop factors the tangent at the
start (iteration 0) and returns only from `k >= 1`, so every attempt costs at least one correction and the pivot test applies to
the tangent at the start: this is why FE1c fails (ADR 0534 addendum 1: pivot `-2.17e10` at `u = 0` under `a = 1e-2`); (c) the
driver's initial equilibrium at `t = 0` is one attempt from `u = 0` with no halving; (d) the integrator's attempt starts from the
committed displacement of the sub-step (`u` of the sub-step before), with the field at the sub-step's end.

## Design

**D1. Opt-in.** `TemperatureFieldHistory::with_thermal_predictor(self, on: bool) -> Self` and the accessor
`thermal_predictor(&self) -> bool`; default `false` (`new` and `constant`). The flag is a property of the field that both
`TimeSteppingOptions::temperature_field` and `NlTransientOptions::temperature_field` already take, so no field of a public
options struct is added and both paths are covered by one switch. No case-file key (ADR 0534 D7 stands). The flag takes part in
`PartialEq`/`Clone`; nothing else reads it.

**D2. The free expansion at fixed elastic deformation (derivation).** With the multiplicative split `F = F_e F_th`,
`F_th = theta I` (ADR 0534 arithmetic 1), a change of the stretch from `theta_o` to `theta_n` at unchanged `F_e` changes `F` to
`F theta_n/theta_o`. For a homogeneous body that moves every point about any fixed point `O`:
`x_new - O = (theta_n/theta_o) (x_c - O)`, where `x_c = X + u_c` is the committed position. The increment of the displacement at
a node `b` is therefore

    d_b = (s_b - 1) (x_c,b - O),   s_b = theta_n,b / theta_o,b,   theta = 1 + alpha_b (T_b - T_ref),

with `T_b` the nodal temperature of the committed time (`theta_o`) and of the attempt's end time (`theta_n`). The elastic
strain of a stress-free expanded body is unchanged (zero), so a uniform field gives the exact solution `u_n = (theta_n - 1)
(X - X_A)` of ADR 0534 arithmetic 5 from `u_c = (theta_o - 1)(X - X_A)`: `x_c - X_A = theta_o (X - X_A)`, times `theta_n/theta_o`,
minus `X - X_A`. The additive form `(theta_n - theta_o)(X_b - O)` agrees only if `u_c` is the free expansion; for a stretched
committed state it differs (P5 measures it, `1e-4` for `e = a = 1e-2`).

For a nonuniform field the same formula with the nodal values is the increment of a locally free-expanding body; it is a start,
not the solution (the thermal strain of a nonuniform field is incompatible), and the converged solution does not depend on it
beyond the Newton tolerance (CONV, INTn).

**D3. The nodal expansion coefficient.** `alpha_b` is the mean of `material.expansion()` over the elements that contain node `b`,
summed in element order; if all of them have the same bits the bits are used (no rounding of `k alpha / k`). The materials are
single finite coefficients by `check_field_model`.

**D4. The reference point and the rigid translation.** `O` is the mean of the committed positions of all nodes with equations
(in node order). The increments `d_b` are completed by one rigid translation `c` (three numbers), fitted per component to the
fixed equations: `c_i = mean over the fixed equations of component i of ( -d_b,i )`, the fixed equations being those of
`prescribed_equations(model, prescribed)` (constraints and prescribed motions); a component without a fixed equation has
`c_i = 0`. Reason: for a body held statically determinately (3-2-1, a pinned node) the fit makes the fixed nodes' increments
vanish where the free expansion allows it, so the start of FE1 is exact whatever `O` is (hand arithmetic: the cube `[0,1]^3`,
`s - 1 = a`, `u_c = 0`, `O = 0.5 (1,1,1)`, fixed A all components `-a (0 - 0.5) = a/2`, B y and z `a/2`, C z `a/2`, so
`c = (a/2) (1,1,1)` and a free node gets `a (X - 0.5) + a/2 = a X`). For a body held on faces or with prescribed motions the fit
is a least-squares translation and the start is not the free expansion (the held cube HB1 of ADR 0534: its exact solution is
`u = 0`, and on the 2 x 2 x 2 mesh the start is exactly `u_c`, by the symmetric sums `+a/2 - a/2`).

**D5. Prescribed displacements are kept.** The start is `u_c` with `d_b + c` added at the free equations only; every fixed
equation keeps the bits of `u_c` (which carry the prescribed values, as `solve_from_field` requires). If every `s_b` is exactly
1 (the field did not change, or `alpha = 0`) the start is `u_c` itself, bit for bit (no `+ 0.0` that could turn a `-0.0`).
A non-finite value in the start is refused (key `"temperature"`).

**D6. The committed field.** The quasi-static driver: the attempt `t -> t1` takes `theta_o` from the field at `t` (the committed
time, the sub-step start after a halving) and `theta_n` from the field at `t1`; the initial equilibrium at `t = 0` is an attempt
from the undeformed reference state, `T = T_ref` everywhere (`theta_o = 1`, `u_c` the prescribed values), so a table that starts
hot is solved from the free expansion (FE0p). The integrator: the sub-step `t_a -> t_b` takes `theta_o` from the field at `t_a`
and `theta_n` from the field at `t_b`; its committed displacement `u` of the sub-step before is assumed to correspond to the
field at `t_a` (the state of `start` is taken as given, as ADR 0534 D5 does for the internal force at `t0`).

**D7. The entry point.** `pub fn thermal::free_expansion_start(model, prescribed, committed, old, new, reference) ->
Result<Vec<f64>>` (the nodal temperatures `old` and `new`, one per node, in K) returns the start of D2 to D5; the driver and the
integrator call it before `solve_from_field` when the flag is on. It checks lengths (`FieldLength`) and finiteness. The Newton
iteration, the load vector, the tolerances and the thermal strain are unchanged; only the start differs. With the flag off no new
code is entered (D8).

**D8. Default bits.** Check B2 records the digests of the runs with a field and no predictor (FE1 with its halved path, FE2, HB1
of the driver, a nonuniform driver run, the integrator runs FEi and V3T style) on the unchanged base 8ebc67e8, before any library
code; they must be equal after. N1 of ADR 0534 (`sa_temp_field_default_bits.rs`) and `baseline_hashes` stay.

## Arithmetic done beforehand (disclosed)

1. FE1: `a = 1e-4 x 100 = 1e-2`, `theta_n = 1.01`; the start of D2/D4 is `a X` to rounding of products of `1e-2` numbers (relative
   `1e-16`), i.e. `~1e-18` absolute; the residual of the start is `E 1e-18` and the first correction (every attempt takes at least
   one, fact b) brings `u` to `~1e-17`. So FE1 limit `1e-12` (acceptance), corrections per entry `<= 2` (one expected), halvings 0.
2. Step 2 and 3 of FE1 (the field held): `s_b = 1` exactly at every node, the start is `u_c` (D5), as without the predictor.
3. FE0p (the table hot from `t = 0`): the initial attempt from `u = 0` at `a = 1e-2` is the case of the diagnosis of ADR 0534
   (pivot `-2.17e10` at node 5 component 2, diagonal `4.49e10`); without the predictor it cannot halve (fact c) and is refused.
   With the predictor the start is the stress-free `a X`, where the tangent is the positive definite material tangent.
4. P5: committed `u_c = e (X - X_A)`, `e = 1e-2`, field step `a = 1e-2`: D2 gives the start `u_c + a (1 + e)(X - X_A)`; the additive
   start is `u_c + a (X - X_A)`; the two differ by `a e = 1e-4` at the far corner.
5. INT1: the free cube with the consistent initial velocity `v_0 = a_dot (X - X_A)` and the ramp `a(t) = a t / (1 s)` has the exact
   solution `u = a(t)(X - X_A)`, `v = v_0`, acceleration 0, stress-free (`F_e = I`): the Newmark average-acceleration step is exact for
   constant velocity, so the integrator reproduces it to the Newton tolerance. The start of D2 equals the solution; without it the
   start is `u_c`, `a dt = 5e-4` away, which the Newton needs at least one correction to remove and with a residual tolerance
   `1e-13` most likely two. With the predictor one correction per step (fact b).
6. The limits of CONV and INTn: the converged `u` of two runs of the same problem with different starts differ by at most the
   Newton error `~ (tolerance 1e-13 relative) / (K / |f|)`, amplified by at most the number of steps (40): `~ 1e-11` relative;
   limit `1e-9` (margin about 100).

## Checks

All tests are in `crates/rcfd-fem/tests/sa_temp_predictor.rs` unless stated; every aggregated error is required finite (the
`Checks` helper of `sa_temp_field.rs`: a NaN is a miss). `a = alpha dT = 1e-2`, the 3-2-1 cube of `sa_temp_field.rs`
(`E = 2e11`, `nu = 0.3`, `alpha = 1e-4`, `T_ref = 300`, `dT = 100`, `tight()` Newton).

| id | clause | what | limit | estimate |
|---|---|---|---|---|
| B2 | c5 | digests of 6 field runs without the predictor (driver: FE1 with its halved path, FE2, HB1, a nonuniform bar; integrator: FEi, V3T style, each values and bytes), recorded on the base before any library code (`sa_temp_predictor_default_bits.rs`) | digests equal | bits |
| N1 | c5 | `sa_temp_field_default_bits.rs` (12 digests of ADR 0534) | pass | bits |
| N4 | c5 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | bits |
| N5 | c5 | `cargo test -p rcfd-fem --release` (all existing tests, including `sa_temp_field.rs` without the predictor and its known miss ignored) | pass | |
| P1 | c3 | `free_expansion_start`, cube 3-2-1, `u_c = 0`, uniform `T_ref -> T_ref + dT`: `max abs(start - a X)` (free and fixed entries) | `<= 1e-14` | `~1e-18` |
| P2 | c3 | prescribed kept: the cube with the extra prescribed motion `u_x(B) = 3e-3` carried by `u_c`; every fixed entry of the start has the bits of `u_c`; every free entry finite | bits equal (0 differing) | exact by D5 |
| P3 | c5 | `old == new` (uniform and nonuniform field) and the `alpha = 0` ramp: the start has the bits of `u_c` at every entry | 0 differing bits | exact by D5 |
| P4 | c3 | nonuniform fields (`T_b = T_ref + A r_b` at the committed and a second `r` at the end, `A = 50 K`, `alpha = 1e-4`) on three supports: 3-2-1, one pinned node, the x component of one node only: the start against the formula of D2 to D4 written out independently in the test (explicit loops over the fixed equations), `max abs(difference)` | `<= 1e-14` | `~1e-17` |
| P5 | c3 | the stretched committed state `u_c = e (X - X_A)`, `e = 1e-2`, uniform field step: start against `u_c + a (1 + e)(X - X_A)`; and the distance to the additive start | `<= 1e-14`; the distance `>= 1e-5` (`1e-4`, arithmetic 4) | |
| FE1p | c1, c4 | FE1 of ADR 0534 with the predictor on the driver: steps 1 to 3; `max abs(u - a (X - X_A)) / L` over the step ends | `<= 1e-12` | `~1e-17` |
| FE1c-p | c4 | the same run: halvings of every entry; entries; `max abs(u(0))/(a L)`; corrections of every entry | halvings `== 0`; 4 entries; `<= 1e-12`; corrections `<= 2` | arithmetic 1 |
| FE1s-p | c4 | the same run: `max abs(f_int) / (E a L^2)` | `<= 1e-10` | `~1e-15` |
| FE2p | c1 | the ramp FE2 with the predictor: `abs(u - a_n (X - X_A))` over the step ends; halvings | `<= 1e-12`; 0 | |
| FE0p | c1, c4 | a table hot from `t = 0` (`(0, T_ref + dT), (2, T_ref + dT)`), step ends 1, 2, with the predictor: `abs(u - a (X - X_A))` at the 3 entries including `t = 0`; halvings | `<= 1e-12`; 0 | arithmetic 3 |
| NC0 | control | the same run without the predictor | refused (an error), by the diagnosis of ADR 0534 | pivot test |
| HB1p | c1 | the held cube HB1 with the predictor: face force against the closed form (relative); `max abs(u) / (a L)` | `<= 1e-10`; `<= 1e-12` | start exact (D4) |
| CONV | c3 | the 3-2-1 beam `[2, 0.5, 0.5]` of `8 x 2 x 2` Hex8, `alpha = 1e-5`, `T_b = T_ref + 50 r_b`, a ramp over 3 steps, driver with and without the predictor: `max abs(u_p - u_n) / max abs(u_n)` over the step ends; both runs succeed | `<= 1e-9` | arithmetic 6 |
| CONVp | c3, D5 | the cube with the prescribed `u_x(B) = 1e-3` (`alpha = 1e-5`, `dT = 100`) ramp, with and without: as CONV | `<= 1e-9` | arithmetic 6 |
| INT1 | c2 | the integrator, free cube, `v_0 = a_dot (X - X_A)`, ramp over 1 s, `dt = 0.05`, 20 steps, predictor on: `max abs(u - a(t_n)(X - X_A))`; halvings; corrections per step | `<= 1e-12`; 0; `<= 1` | arithmetic 5 |
| INT2 | c2 | the same without the predictor: the total corrections; with the predictor the total is strictly smaller | total(with) < total(without) | arithmetic 5 |
| INTn | c2 | the V3T bar, nonuniform fields `F1 -> F2` ramp, 40 steps, with and without: `max abs(u_p - u_n) / max abs(u_n)` over the steps | `<= 1e-9` | arithmetic 6 |
| UBp | c5 | `alpha = 0` ramp (driver: bits of `flat_history`; integrator: bits and state bytes): predictor on equals predictor off; and the reference-constant field `T = T_ref` | equal bits | exact by D5 |
| NCp | control | the same non-trivial runs (FE1p, INT1): predictor on differs from off in the bits of the first entry/step (the predictor does act) | differing | |
| RFp | | the predictor flag on a field the model refuses (a Shell4 model; a wrong length): refused with key `"temperature"` / `FieldLength`; default `thermal_predictor() == false`; `with_thermal_predictor(true) != default` by `PartialEq` | each as stated | |

## Negative controls and mutations

Negative controls: NC0 (without the predictor the hot start is refused), NCp (the predictor acts), P5's additive distance.

Mutations, each on a scratch copy of the tip with its own `CARGO_TARGET_DIR`, one at a time; expected catcher in the last column,
all counted:

| id | mutation | expected |
|---|---|---|
| MU1 | the driver ignores the flag (start unchanged) | FE1c-p (halvings), FE0p, NCp |
| MU2 | the integrator ignores the flag | INT2, NCp (INT1 corrections) |
| MU3 | `theta_o = 1` always (the increment from the reference, not from the committed field) | P3, P4 |
| MU4 | the predicted increment also added at the fixed equations | P2 (and the run refused by `solve_from_field`) |
| MU5 | the translation `c` dropped | P1 (`5e-3`) |
| MU6 | the additive increment `(theta_n - theta_o)(X - O)` instead of the ratio | P5 (`1e-4`) |
| MU7 | `alpha` taken as 1 | P1 |
| MU8 | the committed field of the attempt at `t = 0` taken as `field(0)` instead of the reference | FE0p |
| MU9 | the predictor active with the flag off (driver and integrator) | B2, UBp |
| MU10 | the sign of the increment reversed | P1 |
| MU11 | `O` the origin and `c` set 0 in the components without a fixed equation replaced by the fit over all equations (free expansion about the wrong point) | P4 (the x-only support) |

A mutation that no check catches is reported as a gap, not hidden.

## Decision rule

**done**: B2, N1, N4, N5 met; P1 to P5, FE1p, FE1c-p, FE1s-p, FE2p, FE0p, NC0, HB1p, CONV, CONVp, INT1, INT2, INTn, UBp, NCp, RFp
met; every counted mutation caught; fmt and clippy clean. **partial**: the default-bits files unchanged and FE1p and FE1c-p met,
with a miss of any other check recorded as an ignored `*_known_miss` test with its measured number. **todo**: a default-bits
file changed, or FE1p or FE1c-p missed. A missed check is never re-run with another limit.

## Limits (not covered)

The predictor is a start of the Newton iteration: it changes no converged result beyond the tolerance and does not make a
singular or indefinite tangent at the true solution (a held body whose stress state is indefinite after the expansion) converge.
The start of a held or prescribed body is a least-squares translation of the free expansion, not an elasticity solution. The
committed displacement of the integrator is assumed to belong to the field at `t_a`. Models the field refuses (shells, other
formulations, the energy-momentum step) stay refused. A thermal predictor for the linear thermal load, for the creep-strain
increment limit or for the contact driver is not offered. No case-file key.
