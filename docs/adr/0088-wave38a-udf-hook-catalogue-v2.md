# ADR 0088: Wave 38A, user-function hook catalogue v2 (F-239), first slice

Status: registered before any hook code of this wave was written. Limits are not changed, widened or deleted after a run; a miss becomes an
ignored `*_known_miss` test with the limit intact, a defective set-up gets a NEW corrected test and the old one stays ignored. Baseline hashes are
never re-recorded; every new hook is opt-in (a case without the new settings builds and runs exactly as before). Clean-room: first principles only.
Written at HEAD fbbd487 (scratch clone `<scratch dir>`, target `<target dir>`).

Parent: F-122 (ADR-less, `docs/udf-abi.md`): sandboxed WebAssembly user functions, ABI version 1, four hooks (`momentum_source`,
`velocity_profile`, `kinematic_viscosity`, `monitor`). Roadmap F-239 acceptance: "Each hook, set to a constant or analytic function, reproduces the
matching built-in setting or closed-form solution (property hook equals constant-property run; heat-source hook equals the slab profile)".

## Decision: hooks delivered in this slice

All new hooks are additional optional exports of the same module mechanism (`crates/rcfd-api/src/udf.rs`): same load-time checks (no imports, ABI
export, fuel per call renewed, 16 MiB memory cap, finite results), same failure path (a failing call stops the run with state `failed`, never a
panic). The ABI stays version 1: the new exports are optional and additive, so every existing module keeps working. Steady time `t` is 0.

| # | Catalogue item | Export (all reals f64) | Case setting | Scope |
| --- | --- | --- | --- | --- |
| H1 | scalar boundary profile | `temperature_profile(x, y, z, t) -> K` | `boundaries.<p>.temperature_udf` (with `temperature = "fixed"`) | all incompressible algorithms with energy; evaluated per face once at build (t = 0), like `temperature_expression`; an explicit `temperature_expression` wins |
| H2 | wall-heat-flux | `wall_heat_flux(x, y, z, t) -> W/m^2` (into the domain) | `boundaries.<p>.heat_flux_udf` (with `temperature = "heat_flux"`) | as H1 (also through the CHT heat-flux path); `heat_flux_expression` wins |
| H3 | material property | `thermal_conductivity(x, y, z, t, T) -> W/(m K)`, positive | `udf.<n>.conductivity = true` (at most one module) | steady SIMPLE/SIMPLEC with energy; face value from the face-interpolated temperature; refused with `properties.conductivity`, CHT, radiation and `heat_flux` boundaries (which use `physics.conductivity`) |
| H4 | energy source | `energy_source(x, y, z, t, T) -> W/m^3` | `udf.<n>.energy_source = true` (at most one) | steady SIMPLE/SIMPLEC with energy, divided by rho cp like a zone source and added to the other sources of the iteration; refused with CHT and radiation |
| H5 | species/scalar source | `scalar_source(x, y, z, t, value) -> value/s` | `scalars.<s>.source_udf = "<module>"` | passive scalars in steady SIMPLE/SIMPLEC runs; added to `source`, `source_expression`, zone sources |
| H6 | initialise | `initial_value(x, y, z, comp: i32) -> f64`, comp 0..2 velocity, 3 kinematic pressure, 4 temperature | `udf.<n>.initialise = true` (at most one) | every incompressible algorithm; applied after the uniform values and `initial.velocity_expression_*`, before register patches (which still win); refused with `initial.method` other than `uniform` |
| H7 | execute-at-end | `execute_at_end` with the `monitor` signature | `udf.<n>.at_end = true` | evaluated once, in the step whose summary ends the run (converged, iteration limit or end time); the result `sum(value * V)` is the monitor `udf_end_<n>`. A run stopped from outside the step (a `run(n)` cap, run-control stop, wall-clock limit, divergence guard) does not call it |
| (existing) | vector boundary profile | `velocity_profile` (F-122) | `boundaries.<p>.velocity_udf` | unchanged; already covered by the F-122 test |

H3, H4, H5 in transient, coupled and frozen-flux (CHT) runs are refused with a diagnostic, because those engines do not pass through the steady path
that applies them (silently ignoring them would be worse).

**Not delivered in this slice** (stated in the roadmap note): turbulence source hooks (the turbulence models live inside rcfd-fv and expose no source
input), species (multicomponent) source hooks (only passive scalars), adjust hook (modifying fields between iterations), exit hook as distinct from
execute-at-end, reaction-rate hook, dynamic-zone motion hook, specific-heat and density property hooks, any hook in the compressible, variable-density
or VOF solvers (still refused for all user functions), and transient/coupled use of H3-H5. **Particle hooks** stay gated on the particle model: the
Lagrangian tracking features F-056 and F-128 are `partial`, and no particle hook (injection, drag law, wall interaction, scalar update) is offered.

## Registered checks (`crates/rcfd-api/tests/udf_hooks_w38a.rs`)

Modules are written in WAT. "rel diff" of a field = max over cells |a - b| / S, S = (max - min) of the built-in field for temperature and the
maximum |value| for the other fields. The equivalence limit 1e-9 is the F-122 limit: the hook and the built-in set-up assemble the same discrete
equations, so only round-off of the per-face/per-cell values may differ.

Common slab ("SLAB", copy of `tests/zone_sources.rs`): box_2d nx x 2 cells on 0.1 x 0.02 m, rho 1, cp 1000, k 2, no flow (all velocities fixed 0),
energy relax 1, xmin/xmax T = 300 K, SIMPLEC 400 iterations.

* **C1 (H1)**: SLAB nx 32, xmin `temperature_udf` returning 300 + 1000 y K against `temperature_expression = "300 [K] + 1000 [K/m] * y"`:
  rel diff of T < 1e-9; and the run differs from the uniform-300 xmin run (rel diff > 1e-3).
* **C2 (H2)**: SLAB nx 32, xmin `temperature = "heat_flux"`, `heat_flux_udf` returning 5000 W/m^2 against `heat_flux_value = 5000`: rel diff of T
  < 1e-9; and the hook run equals the closed form T = 300 + q (L - x) / k to max |error| / (q L / k) < 1e-9 (a linear profile is exact for the
  scheme).
* **C3 (H3)**: SLAB nx 32 with a uniform zone source 1e4 W/m^3, `physics.conductivity = 5` and a `thermal_conductivity` module returning 2, against
  `physics.conductivity = 2` without the module: rel diff of T < 1e-9; the k = 5 run without the module differs (rel diff > 1e-3).
* **C4 (H4)**: (a) SLAB nx 64, `energy_source` returning 1e4 against `zone_sources` value 1e4: rel diff of T < 1e-9. (b) The same hook run against
  the slab profile: the discrete identity T_h = 300 + q x (L - x) / (2 k) + q h^2 / (8 k) holds to 1e-6 of the maximum rise q L^2 / (8 k) (the E1a
  limit of `tests/zone_sources.rs`). (c) SLAB nx 64, `energy_source` returning 2e4 x / L against the closed form T = 300 + q0 x (L^2 - x^2) / (6 k L):
  max error / max rise <= 6e-4 (the E2 limit of `tests/zone_sources.rs` at nx 64, registered there with its 8-64 refinement study).
* **C5 (H5)**: SLAB nx 64 with a passive scalar c (diffusivity 0.01, c = 0 on xmin and xmax), `source_udf` returning 5 against `source = 5`: rel
  diff of c < 1e-9; and the hook run satisfies the same discrete identity c_h = S x (L - x) / (2 D) + S h^2 / (8 D) to 1e-6 of S L^2 / (8 D).
* **C6 (H6)**: channel_2d 16 x 10 (2 x 1 m) with energy, `initial_value` returning u = (6 y (1 - y), 0, 0), p = 0.25, T = 310, against
  `initial.velocity_expression_x = "6 [m/s] * y / 1 [m] * (1 - y / 1 [m])"`, `initial.pressure = 0.25`, `initial.temperature = 310` with
  `initial.velocity = [1, 0, 0]` in both: after construction the fields ux, uy, p, T have rel diff < 1e-12; after 50 SIMPLEC iterations < 1e-9.
* **C7 (H7)**: the F-122 channel run (16 x 10, SIMPLEC, 300 iterations) with an `execute_at_end` module returning ux: the monitor `udf_end_<n>` has
  exactly one value, recorded at the last step, equal to sum(ux V) of the final fields to 1e-12 relative.
* **C8 (diagnostics and refusals)**: each of these is refused with a located message, none panics: a role flag (`conductivity`, `energy_source`,
  `initialise`, `at_end`) or a reference (`temperature_udf`, `heat_flux_udf`, `source_udf`) whose module lacks the export; a reference to an
  undefined module; `temperature_udf` without `temperature = "fixed"` and `heat_flux_udf` without `temperature = "heat_flux"`; two modules with
  `energy_source`; `energy_source`, `conductivity`, `source_udf` with `solver.algorithm = "piso"`; `conductivity` with `properties.conductivity`;
  `conductivity` with a `heat_flux` boundary; `initialise` with `initial.method = "potential"`. A non-positive conductivity and a non-finite energy
  source stop the run with state failed.
* **Defaults**: `crates/rcfd-api/tests/baseline_hashes.rs` and the F-122 tests `tests/udf.rs` pass unchanged.

## Smoke runs of the built-in counterparts (rule d), before this registration

Scratch file `crates/rcfd-api/tests/w38a_smoke.rs` (not committed), release build of fbbd487; no hook code existed.

* S1 (C1 set-up): xmin `temperature_expression` runs; T range 300.156 .. 313.765 K (400 iterations, iteration limit, field converged).
* S2 (C2 set-up): `heat_flux_value = 5000` converges in 4 iterations; max |T - closed form| / (qL/k) = 3.5e-13.
* S3 (C4a/b set-up): the zone-source slab identity deviation is 1.1e-10 of the maximum rise (limit 1e-6). S3b (C4c set-up): linear zone source
  relative error 4.72e-4 at nx 64 (limit 6e-4). S3c (C3 set-up): k = 5 gives T max 302.50 K (k = 2: 306.25 K), so the sanity check has margin.
* S4 (C5 set-up): the passive-scalar slab with `source = 5` meets the identity to 9.9e-15 of the maximum rise.
* S5 (C6 set-up): the expression-initialised channel builds (ux at the first cell centre 0.285 = 6 * 0.05 * 0.95, p 0.25, T 310) and runs 50
  iterations without failure.

## Addendum: measured values (release build, `crates/rcfd-api/tests/udf_hooks_w38a.rs`)

| Check | Limit | Measured | Verdict |
| --- | --- | --- | --- |
| C1 `temperature_profile` vs `temperature_expression`, rel diff of T | < 1e-9 | 0 (bit-identical) | pass |
| C1 sanity, hook run vs uniform-300 K run | > 1e-3 | 1.01 (corrected test, see deviation 1) | pass |
| C2 `wall_heat_flux` vs `heat_flux_value`, rel diff of T | < 1e-9 | 0 | pass |
| C2 hook run vs T = 300 + q (L - x) / k | < 1e-9 | 3.5e-13 | pass |
| C3 `thermal_conductivity` (physics.conductivity 5, hook 2) vs k = 2 run | < 1e-9 | 0 | pass |
| C3 sanity, k = 5 run vs k = 2 run | > 1e-3 | 0.64 | pass |
| C4a `energy_source` 1e4 vs zone source 1e4 | < 1e-9 | 0 | pass |
| C4b slab identity, fraction of q L^2 / (8 k) | <= 1e-6 | 1.08e-10 | pass |
| C4c linear source vs closed form, nx 64 | <= 6e-4 | 4.72e-4 | pass |
| C5 `scalar_source` 5 vs `source = 5`, rel diff of c | < 1e-9 | 0 | pass |
| C5 scalar slab identity | <= 1e-6 | 9.9e-15 | pass |
| C6 `initial_value` vs initial expressions, at build | < 1e-12 | 0 | pass |
| C6 after 50 SIMPLEC iterations | < 1e-9 | 0 | pass |
| C7 `execute_at_end`: one value, at the last step (300), vs independent sum | 1 value, < 1e-12 | 1 value at x = 300, 0 | pass |
| C8 diagnostics and refusals (17 cases) and two run-time stops | all refused / stopped with the named message | all | pass |
| Defaults: `baseline_hashes.rs`, F-122 `udf.rs` (8 tests), `zone_sources.rs`, `scalars.rs`, `properties.rs`, `rules_w19c.rs`, `init_methods.rs` | unchanged | all pass, hashes not re-recorded | pass |

The equivalence results are exactly 0 because the hook and the built-in feed the same numbers into the same assembly
(for example k / (rho cp) = 2 / 1000 on every face either way); the limit 1e-9 was not needed.

Deviations from the registered plan, disclosed:

1. **C1 sanity metric was a defective set-up.** As written, the sanity part scaled the difference by the range of the
   uniform-300 K run, which is 0, so it printed 1.4e301; the check "passed" without meaning anything. The test stays
   as `c1_temperature_profile_equals_the_expression`, ignored with that reason, and the NEW test
   `c1_corrected_sanity_scale` scales by the range of the hook run (measured 1.01). The equivalence part (0) is the
   same in both.
2. **C6 module text did not parse on the first run** (one closing parenthesis too many in the WAT of the test module),
   so C6 measured nothing on that run. The parenthesis was removed; no limit or set-up changed. The second run is the
   one reported.
3. C8 grew beyond the list in the registration only by splitting it into individual cases (four role flags, three
   references, one undefined module, two wrong-condition cases, two providers, three transient refusals, the property
   law, the heat-flux boundary, the initial method, and the two run-time stops).

Not delivered, as registered: turbulence source, species (multicomponent) source, adjust, a separate exit hook,
reaction rate, dynamic-zone motion, specific heat and density hooks, transient/coupled/frozen-flow use of the
source and property hooks, and particle hooks (gated: F-056 and F-128 are partial). F-239 is therefore `partial`:
every hook delivered meets its registered check, but the acceptance names the whole catalogue.

No reference values beyond first principles were used: the closed forms are the 1D conduction solutions with
uniform and linear sources and with a fixed flux, and the discrete identity of the uniform-source slab follows from
the three-point scheme (as in ADR 0019 E1).

## Amendment 1 (written after the addendum above and before the run it governs)

Review of the delivered slice found claims in the docs that no registered check ran. What I knew when writing this: C1-C8 as
measured above; no run so far had an energy-source hook together with another heat source, no run used `heat_flux_udf`
with conjugate heat transfer, and no run used `initialise` or the boundary hooks with an algorithm other than SIMPLEC.

* **C9 (new, registered here, not yet run)**: SLAB nx 64, `energy_source` returning 5e3 W/m^3 together with a uniform
  zone source of 5e3 W/m^3, against the zone source of 1e4 W/m^3 alone: rel diff of T < 1e-9 (the same limit and the
  same reason as C4a: the hook adds its value to the zone source assigned in the same iteration). This exercises the
  add-to-the-other-sources branch, where an accumulation over iterations would show as a large difference.
* Docs are reworded, not tested: the CHT heat-flux path of `heat_flux_udf` is wired but untested; H1, H2 and H6 are
  tested in steady SIMPLEC only; the sentence that a restart overwrites the initialise hook is removed (not tested);
  the combination of `energy_source` with components, chemistry and particle heat sources is wired but only the zone
  source combination is tested (C9).

Rule (c) disclosure for deviations 1 and 2 of the addendum: the corrected test `c1_corrected_sanity_scale` was
written and run before any amendment text existed. At that point I already knew the C1 equivalence (0) and that the
original sanity value (1.4e301) was degenerate; the corrected scale (range of the hook run) was chosen because it is
the non-zero field of the pair, not from its value. The C6 module text was fixed and C6 rerun the same way, before
writing it down; the first run had measured nothing (the module did not parse).

### Amendment 1 results

* C9 (hook 5e3 plus zone source 5e3 against zone source 1e4): rel diff of T = 0 (limit 1e-9). Pass.
* Gate of the first code commit 59f0279 (fresh `git archive`, `<scratch dir>`): `cargo fmt --check` clean,
  `cargo clippy --workspace --all-targets -- -D warnings` clean, `cargo test --workspace --release --no-fail-fast`
  1910 passed / 0 failed / 536 ignored. This HEAD includes waves 38B, 38C and 38D, so the totals are not comparable
  with the wave-37 close (1881 / 531); the 38A delta at 59f0279 was +8 passed, +1 ignored (`udf_hooks_w38a.rs`), and
  C9 adds one more passed test.
* Re-gate of 356112b (adds C9 and doc rewording only): `cargo fmt --check` and `cargo clippy ... -D warnings` clean;
  `cargo test --workspace --release --no-fail-fast` was stopped by the host for low memory after 218 test binaries
  (1160 passed, 0 failed, 297 ignored at that point) and was not restarted. C9 itself passed in the scratch build
  (9 passed, 1 ignored in `udf_hooks_w38a.rs`). The last complete gate is the one of 59f0279 above.
