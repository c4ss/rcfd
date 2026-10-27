# Partials audit, group 7

This is a read-only audit of ten roadmap items with status `partial`. It started at HEAD 4be2f61 on 2026-10-21. Each item is graded
against its `acceptance` field in docs/roadmap.json only. Wider scope from the title or from a "SCOPE (merged review)" addition is listed
as "scope beyond acceptance". That scope does not lower the grade.

Grades: DONE = the acceptance text, as written, is met by a passing non-ignored test with numeric limits. PARTIAL = a real tested slice
exists, and the unmet clause is named. TODO = nothing usable. OVERSTATED = the roadmap note claims more than the tests show.

How the runs were made: Windows 11, `cargo test --release -p <crate> --test <name> -- --include-ignored` with a private target directory,
on a machine shared with other agents' cargo jobs, so wall times are noisy. The F-094 eval ran in a debug build, as its criterion C8
specifies. "Known misses fail as recorded" means that the `#[ignore]`d known-miss or superseded tests fail, as their notes say. Not re-run:
any full workspace build or test, any Linux run, and any hosted CI.

## Summary table

| id | title (short) | grade | evidence run or read | residual gap (against acceptance) | size | blockers | one wave | user decision |
|---|---|---|---|---|---|---|---|---|
| F-032 | Block-coupled p-U solver with AMG | DONE | ran rcfd-fv `coupled` (gate test, plus ignored `coupled_variants_report` with output), `coupled_open` 10 pass plus 3 known misses fail as recorded, `coupled_solid` 2/2; rcfd-api `coupled` 3/3 | none as written. The comparison is at unequal relaxation: coupled at alpha_u 0.9 settles at 165, SIMPLE at 0.7 at 610. At equal alpha_u 0.7 both settle at 610. | - (S for a SIMPLE relaxation sweep) | none | - | yes (accept, or reword to "best stable relaxation of each") |
| F-054 | Finite-rate / eddy-dissipation combustion | PARTIAL | ran rcfd-api `chemistry` 8 pass plus 1 known miss, rcfd-cli `chemistry_run` 7 pass plus 1 known miss; read ADR 0027, ADR 0037 | methane one-step jet flame never run; chemistry is constant-density only; no published flame band chosen | L | variable-density reacting flow; missing reference data (published flame band); user decision on which flame | no | yes (choose the reference flame and band) |
| F-094 | Assistant-guided workflows (mock eval) | DONE | ran rcfd-assistant `eval` (debug): C1 to C8 all PASS, suite 8.72 s (limit 60 s) | none; the real or local model run is optional in the acceptance text | - | - | - | no |
| F-113 | Lumped zone components | DONE | ran rcfd-api `lumped` 7 pass plus ignored (Z4 nominal known miss fails; Z3 known miss now PASSES), `w35b_hx_corrected` 2/2 | none as written; the note's Z3 miss and Z2 percentages are out of date | - | - | - | yes (re-examine the Z3 ignore; split the SCOPE list) |
| F-127 | Euler-Euler / mixture multiphase, cavitation | PARTIAL | ran rcfd-fv `cavitation` 4/4, `mixture_drift` 10 pass plus 4 superseded fail, `cavitating_venturi` (331 s: N1 and corrected N2 to N6 pass, registered and known-miss fail); rcfd-api `mixture_cavitation_tree` 3 pass plus 2 known misses | no bubble-column void profile and no cavitation sheet-length trend compared with literature (not attempted); no multi-fluid Euler-Euler | L | missing reference data (both datasets); Euler-Euler with lift and dispersion for the bubble column | no | yes (pick the datasets) |
| F-144 | Monitors panel | PARTIAL | ran rcfd-gui `monitors_plot` 6/6 (m5: worst poll 5.0 ms, limit 1 s); read `logic.rs`, `smoke.rs`, ADR 0025 | no test asserts that the plot series grows, or that a frame draws, while the run is in progress | S | none | yes | no |
| F-207 | LES and hybrid RANS-LES catalogue | PARTIAL (0 of 3 clauses) | ran rcfd-turb `des` 6/6, rcfd-api `des_tree` 4/4; read ADR 0032; grep for synthetic inflow, Vreman, sigma, dynamic models (none) | synthetic inflow statistics, channel LES log law and decaying isotropic turbulence spectrum: none attempted | L | F-045 (partial); missing reference data (decay spectrum) | no (yes for the synthetic-inflow clause alone) | yes (split the item) |
| F-217 | Heat-exchanger domain model, fan and vent boundaries | PARTIAL | ran rcfd-api `heat_exchanger` 5 pass plus 2 known misses, rcfd-fv `heat_exchanger` 12/12, rcfd-api `bc_catalogue` `c3_vent_loss_law` (pass); read ADR 0039, ADR 0022 | fan boundary checked at one operating point; vent only at abs(u) = 1, where the quadratic law is not distinguished from a linear one | S | none | yes | no |
| F-229 | Six-DOF rigid-body dynamics | DONE | ran rcfd-fv `sixdof` 7 pass plus 1 known miss, `sixdof_coupled` 3/3; read ADR 0033 | none as written. ADR 0033 reads two of the clauses as flow-coupled and says they are not delivered. | - (M if flow coupling is required) | - | - | yes (wording) |
| F-239 | User-function hook catalogue v2 | PARTIAL | ran rcfd-api `udf_hooks_w38a` 10/10 including the ignored C1 record; read ADR 0088 | turbulence source, species source, adjust, exit, reaction-rate, dynamic-zone motion, cp/density and particle hooks are not delivered | M (non-particle hooks); L with particles | particle hooks need F-056 and F-128 (partial); reaction rate needs F-054 | yes for the non-particle hooks | yes (split the particle hooks) |

Counts: DONE 4, PARTIAL 6, OVERSTATED 0, TODO 0.

## Items not DONE, and the DONE items with a caveat

**F-032 (DONE, caveat).** Re-run gate output: coupled alpha_u 0.9 settles at 165 outer iterations (270 total, 3060 GMRES), SIMPLEC 0.9 at
167, SIMPLE 0.7 at 610. The Ghia errors are u 0.00277 and v 0.00855 (limit 0.02). The re-run variants report matches the note: coupled 0.95
settles at 83 and 0.98 at 38; SIMPLEC 0.95 at 97 and 0.98 at 249; the Jacobi Schur block takes 14181 GMRES iterations against 3060. Two
caveats. First, coupled at alpha_u 0.7 also settles at exactly 610. The win over SIMPLE comes from the stronger relaxation that coupling
tolerates, and SIMPLE above 0.7 was never measured. Second, the non-ignored test also asserts coupled < SIMPLEC, with a margin of 165
against 167 iterations. The acceptance does not need that, and a two-iteration margin is fragile. Suggested rewording: "at each solver's best
stable alpha_u from a registered sweep". Adding the SIMPLE sweep is S. The open-boundary misses (A2, A3) and the missing transient,
turbulence and periodic support are scope beyond the acceptance. AMG (F-027) is still partial.

**F-054 (PARTIAL).** The infrastructure tests pass: the adiabatic flame temperature equals the hand value, element conservation holds, the
plug flow and splitting orders hold, and the CLI and service runs are bit-identical. Only the K-series chemistry tests and the CLI run were
counted. The flamelet and progress-variable tests in rcfd-reactor `combustion.rs` belong to other items and are not counted. The acceptance
clause (methane one-step jet flame temperature within a published band) was never attempted. `chemistry.density` must equal
`physics.rho`, so no flame with heat-release expansion can run. Closing it needs four things: variable-density species and energy coupling
to the chemistry, a jet geometry, a published one-step reference, and the band. The acceptance is unmeasurable as written because no
reference or band is named. Suggested rewording: "peak temperature of the <named> one-step methane laminar jet flame within <X> K of the
<named source> value". Citations: eddy-dissipation A = 4 and B = 0.5 are UNVERIFIED. The EDC C_xi and C_tau come from a secondary
source, and C_D1 and C_D2 are recalled from memory (docs/reference-data.md).

**F-113 (DONE, note out of date).** Z1 worst error is 1.0e-6 (limit 1e-3). Z2: Q -0.0003 %, fan rise -0.0001 % (limits 0.5 and 0.1). Z4
stream enthalpy balance is 3.1e-12 (fixed duty). The note is stale, not overstated. `z3_network_pressure_known_miss` now passes under
`--ignored` (patch pressure equals R(Q) to +0.00000 %), against the recorded 8.5e-4 miss. Z2 now reads -0.0003/-0.0001 % against the note's
+0.061/+0.091 %. Someone should find which wave changed it, then lift or keep the ignore under the ADR rules. The SCOPE list (intake
fan, exhaust fan, vent, general 1D network) is scope beyond the acceptance. It should become its own item, or the acceptance should grow.

**F-127 (PARTIAL).** Every slice the note lists reproduces: C1 to C4; D1 to D6 (the corrected set-ups pass, the registered ones fail as
recorded); M1 to M3 and M4b; N1; N2 to N6 corrected; W1 to W3. The N3/N4/N5 and bitwise known misses fail as recorded. Neither acceptance
clause was attempted. A bubble-column void profile needs at least lift and turbulent dispersion, so it needs a multi-fluid model or a
justified mixture closure. A venturi or hydrofoil sheet-length trend needs a published sweep of cavitation number against sheet length.
Both datasets are missing, and all references are UNVERIFIED (docs/reference-data.md, wave 29B section). This takes several waves. The
user should name the datasets. The closure taxonomy in the SCOPE note should be split into sub-items.

**F-144 (PARTIAL).** Human viewing is not the gap: the acceptance never asks for it, although the note and docs/audit-report.md frame the
gap that way. The real gap is the first clause, "plots update while a cavity run is in progress". m5 polls a running case and checks that
one poll saw `running` with fewer than 60 iterations, and that the final `res_u` series has exactly 60 samples. It never asserts that the
monitor series grows between polls while the state is `running`, and no frame is drawn mid-run. The 60-iteration run finishes in well under
0.1 s, so the "mid-run" window is tiny. "UI thread never blocks" is measured on `Controller::poll` (worst 5.0 ms here, 22 ms recorded),
not on a full egui frame. To close it: a run of a few thousand iterations; the series length strictly increasing over at least three polls
in state `running`; a frame drawn in that state with its time bounded. Size S, no blockers.

**F-207 (PARTIAL, 0 of 3 clauses).** The delivered slice is SA-based DES and DDES: length scale, shielding, RANS limit, LES-limit fixed
point and time derivative. B1 to B7 pass. None of the three acceptance clauses has any code or test. There is no synthetic inflow
generator, no channel LES run, and no isotropic-turbulence case. The repo's `sgs_taylor_green.rs` is a 2-D Taylor-Green vortex for F-045,
not a spectrum-decay comparison. Suggested split:
- The synthetic inflow statistical test: M, closable in one wave, no reference data needed.
- Decaying isotropic turbulence against a published spectrum: M to L. It needs a 3-D periodic box, a spectral initialisation, spectrum
  post-processing and a verified dataset, which is missing.
- Channel LES log law: L, compute-heavy. It depends on the inflow generator and on F-045.

The subgrid catalogue (dynamic Smagorinsky, Vreman, sigma, WMLES, IDDES) is scope beyond the acceptance. C_DES 0.65 is from a secondary
source only.

**F-217 (PARTIAL).** The counterflow clause is met: X1 duty matches eps-NTU (identical by construction, rel 0) and an independent marched
solution to 9.8e-15 (limit 2e-2). That checks the algebraic macro model, not a resolved exchanger. The fan and vent clause ("reproduce
their prescribed pressure-flow curves") is shown at single points:
- `fan_boundary` X4: one operating point, Q within 6.35e-4 and patch pressure on the curve to 1.8e-13.
- Vent: only borrowed evidence from F-200's `bc_catalogue` C3. It runs at abs(u) = 1 for K = 0, 2.5 and 10, forward and reversed,
  within 7.3e-11. At abs(u) = 1, K u abs(u)/2 equals K u/2, so the quadratic law is not tested. C3 runs on a post-hoc set-up (pseudo
  dt 0.02) because the registered plain SIMPLEC set-up diverged, and the reversed K = 0 case is a known miss.

To close it: a registered sweep of at least three flow rates per boundary type (for example 0.5, 1 and 2 times nominal), on one set-up.
Size S. The shell-and-tube relation is recalled from memory, UNVERIFIED and not part of the acceptance.

**F-229 (DONE, wording conflict).** R1 free fall with linear drag gives a relative error of 1.13e-10 in v (limit 1e-8) at t = 5 tau. There,
v/v_t = 1 - e^-5 = 0.993, which is within 2 % of the terminal velocity. R3 damped oscillator (zeta 0.2): the trajectory deviates by at most
2.4e-7 from the exact damped solution over three periods (limit 1e-5), so the damped frequency is far inside 2 %. R5 pendulum period:
8.9e-5 small amplitude, 4.3e-10 at theta0 = 1 (RK4). R6 locked axes: exactly fixed. Without a fluid, all four clauses pass. ADR 0033,
however, reads the spring-mounted body and the drag fall as flow-coupled, and states that this is not delivered. If the user agrees with
that reading, the item is PARTIAL with an M gap: a spring-mounted cylinder or plate in a resolved flow, and a body falling through a
fluid. Citation remark: the roadmap note's "terminal velocity with drag 2.4e-7 (1e-5)" is the R3 damped-oscillator number. ADR 0033 row
"R3 terminal velocity" carries the same mislabel. The free fall is R1, at 1.1e-10 against 1e-8.

**F-239 (PARTIAL).** All seven delivered hooks reproduce their built-in counterparts:
- C1 to C3, C5, C6, C9 and C4a: rel diff 0 (limit 1e-9).
- C4b slab identity: 1.08e-10 (limit 1e-6).
- C4c linear source against the closed form: 4.72e-4 (limit 6e-4, a narrow but deterministic margin).
- C2 closed form: 3.5e-13.
- C5 identity: 9.9e-15.
- C7 at-end: exact.
- C8 refusals pass.

"Each hook" in the acceptance covers the whole title catalogue. Not delivered: turbulence source, species (multicomponent) source, adjust,
exit (distinct from at-end), reaction rate, dynamic-zone motion, cp and density hooks, and all particle hooks. Most non-particle hooks fit one
M wave, because each has a built-in counterpart to equal. Reaction rate depends on F-054. Particle hooks depend on F-056 and F-128, both
partial. Suggested split: move the particle hooks to F-128.

## What was not re-run or not checked

- The optional real or local model run of F-094.
- Any GUI frame drawn during a run (F-144).
- The transient (ADR 0045) heat-exchanger and fan tests of F-217, and `transient_sources.rs`.
- The F-127 slices beyond the tests listed.
- Any test in a hosted CI or on Linux.

The F-094 eval journals were not inspected by hand; C6 checks them.
