# ADR 0528: F-597 registration: the B-bar creep cylinder growth bound re-registered at a p/E where the finite-strain drift is below it, and a Full-solve, BBar-recovery locking control

Status: **REGISTERED 2026-11-14, written before any code of F-597 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-597 was written, compiled or run. The only numbers used below are those already
recorded in [ADR 0506](0506-f587-registration-b-bar-and-f-bar-hex8-in-the-nonlinear-driver.md) addendum 1 (disclosed where
quoted). Results go in an addendum at the end; nothing above the addendum is changed after a run.

Roadmap item F-597 (deps F-587), follow-up of ADR 0506 (C4's known miss and the mutation MU5). Number 0528 given by the lead.
Branch `f597-work` from 1f142196, worktree `w49i-work/f597`, scratch `w49i-work/f597-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here, not changed after a run; a miss becomes an ignored
`*_known_miss` test with the limit intact; each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, the hash tables or goldens; no existing test edited (new tests are appended to `si_fbar.rs`, which
carries the helpers of the cylinders). Clean-room: only the closed forms already in `si_fbar.rs` (Lame, the stationary creep
field of a power law, both textbook closed forms) are used.

## What was read

ADR 0506 (D6, the checks C1 to C4 and CL, mutation MU5, addendum 1 with the diagnostic of C4), and the tests of `si_fbar.rs`
(`cylinder`, `creep_run`, `polar_points`, `creep_error`, `c4_diagnosis_tenth_pressure`).

## The choice of alternative

The acceptance allows (A) a finite-strain reference for C4, or (B) C4 re-registered at a p/E small enough that the total
Lagrangian drift stays below the bound.

**B is taken (taken on the maintainer's standing instruction of 2026-11-11, take the recommendation).** Reasons. (A) needs a
closed form or a reference solution of the thick cylinder in creep at finite strain, with a compressible elastic part and a
creep strain near `1e-2`: none is in the repository or derivable here soundly (the large-strain stationary field of a Norton law
has a closed form only for incompressible, purely viscous material, and the transient from the elastic start to it is not
closed); a reference computed by the same code would not be independent. (B) keeps the registered limit `2e-3` and the closed
forms of C2/C3 unchanged, and the diagnostic of ADR 0506 says why it is sound: the drift is a geometry effect that scales with
the strain (the worst point stays in the interior at `p/10`, growth `1.06e-4` against `6.02e-3` at `p`, a factor 57 for a
factor 10 in strain, as a second-order geometric effect is).

Disclosure: the number `1.06e-4` at `p/10` is already known from the diagnostic of ADR 0506 (`c4_diagnosis_tenth_pressure`).
The registered run below is the same run with a limit attached; it is deterministic and will repeat those bits. The limit
`2e-3` is ADR 0506's own, kept, not derived from this number; the margin is 19.

## Design

The creep cylinder of ADR 0506 D6 unchanged (mesh `48 x 24 x 1`, material `creep(E = 2e9, nu = 0.3, A = 1/(E q^3), n = 4)`,
the 70 times of `creep_times`, `TimeSteppingOptions::new(5e-4)`), at `q = p/10 = 2e4`, so `t* = 1 s` and every stress over `q`
equals the small-strain value, every strain ten times smaller. Stress recovery of the BBar solve with BBar (`polar_points`).
Two solves, each made once: a BBar solve (`creep_run(BBAR, q)`) and a Full solve (`creep_run(FULL, q)`); both are held in one
test.

The locking control (clause 2). The mutation MU5 (a BBar element evaluated as Full by the driver) was not seen by C2 because
the BBar recovery of a Full-solved field removes most of the pressure error in the stress. The stress is the wrong witness of
the solve for this combination. The control therefore uses quantities that follow the solve and do not pass through the
recovery:

- LC-u: the radial displacement of the bore nodes, `u_r = (x u_x + y u_y) / r` at the nodes of the set `xmin` (`r = a`); the
  relative difference of the Full solve and the BBar solve, `max_nodes |u_r^F - u_r^B| / max_nodes |u_r^B|`, at `5 t*` and
  `20 t*`.
- LC-e: the equivalent creep strain of the point states (state value 6, as in the diagnostic), relative difference
  `max_pts |e^F - e^B| / max_pts e^B` over all integration points (same mesh, same ordering), at `20 t*`.
- LC-s (reported, no limit): the point-wise stress error against the stationary field of the Full solve recovered with BBar
  (the combination MU5 produces) and recovered with Full (CL), at `5 t*` and `20 t*`.

Why the Full solve differs: with the creep strain incompressible the Full Hex8 locks (ADR 0506 CL: `0.103` at `5 t*`, `0.364`
at `20 t*` in the stress), so its displacement is stiffened. Estimate of LC-u: the relative displacement error of a locked
linear element on this mesh is of the order of the relative stress error, a few times `1e-1`; the limit is set far below, at
`3e-2`. No probe was made of this quantity: the estimate is by analogy and its margin is believed, not shown; a miss is
recorded, not tuned.

## Checks

Every measure is required finite before it is compared. All at `q = p/10`.

| Id | Clause | Measure | Limit | Estimate |
|---|---|---|---|---|
| C1' | 1 | BBar, `t = 0`, point-wise against Lame over `q` | `<= 1e-2` | ADR 0506 C1 `5.4e-3` at `p`; same stresses over `q` |
| C2' | 1 | BBar, `5 t*`, point-wise against the stationary field | `<= 2e-2` | `9.5e-3` at `p` |
| C3' | 1 | BBar, `20 t*`, the same | `<= 2e-2` | `1.6e-2` at `p`, less at `q` |
| C4' | 1 | `abs(C3' - C2')` | `<= 2e-3` (ADR 0506's limit, unchanged) | `1.06e-4` (diagnostic of ADR 0506) |
| SC | 1 | the strain is small: largest equivalent creep strain of the BBar run at `20 t*` | `<= 2e-3` | `9.65e-4` (diagnostic) |
| NC1 | 2 | Full and BBar solves at `t = 0` (elastic, `nu = 0.3`, no locking): `max_nodes |u_r^F - u_r^B| / max |u_r^B|` | `<= 1e-2` | discretisation difference only, `~1e-3` believed |
| LC-u5 | 2 | LC-u at `5 t*` | `>= 3e-2` | a few `1e-1` believed |
| LC-u20 | 2 | LC-u at `20 t*` | `>= 3e-2` | larger than at `5 t*` (CL grows) |
| LC-e20 | 2 | LC-e at `20 t*` | `>= 5e-2` | believed `>= 1e-1` (creep strain follows the stress) |
| LC-s | 2 | the stress errors of the Full solve (BBar and Full recovery) | reported only | shows what C2 does not see |

Negative controls: NC1 (the difference LC-u measures is made by the creep, not by the set-up: it is below `1e-2` at `t = 0`);
the C4 at `p` of ADR 0506 stays as the ignored known miss `c1_to_c4_cl_creep_cylinder_known_miss` (`6.02e-3` against `2e-3`),
so C4' is the same measure at a smaller strain, not a loosened one; SC (the premise of the choice is that the strain is small).

## Planned mutations (on a scratch copy with its own CARGO_TARGET_DIR)

| Id | Mutation | Expected catcher |
|---|---|---|
| MU5 | the dispatch of ADR 0506 D2 dropped (the `BBar` arm of `evaluate_formulation` takes `evaluate(&Hex8, ..)`) | LC-u5, LC-u20, LC-e20 (the BBar solve equals the Full solve, difference 0); C2' expected not to catch (ADR 0506) |
| MU-R | the recovery of C1' to C4' taken with Full instead of BBar | reported; C2' or C3' if it shows |

## Decision rule

- **done**: every check C1' to C4', SC, NC1, LC-u5, LC-u20, LC-e20 met in its single run, and MU5 caught by an LC check.
- **partial**: a limit missed (an ignored `*_known_miss` test with the number), or MU5 not caught by any new check.
- **todo**: nothing met.

## Limits (what is not covered)

No finite-strain reference (alternative A): C4 at the strain of `p` stays a known miss of ADR 0506. No library change; no
case-file key; defaults untouched. The estimates of LC-u and LC-e are unprobed, set by analogy to CL.

## Amendment 1 (2026-11-14, written after run 1 and before any corrected run)

Run 1 (test `f597_c1_to_c4_prime_and_the_full_solve_bbar_recovery_control`, single run, 28 s) gave:

| Check | Measured | Limit | Result |
|---|---|---|---|
| C1' | `5.41e-3` | `<= 1e-2` | met |
| C2' | `9.48e-3` | `<= 2e-2` | met |
| C3' | `9.58e-3` | `<= 2e-2` | met |
| C4' | `1.063e-4` | `<= 2e-3` | met (margin 19, the bits of the diagnostic of ADR 0506) |
| SC | `9.65e-4` | `<= 2e-3` | met |
| NC1 | `3.8e-5` | `<= 1e-2` | met |
| LC-u5 | `1.13e-3` | `>= 3e-2` | **missed** |
| LC-u20 | `4.14e-3` | `>= 3e-2` | **missed** |
| LC-e20 | `4.16e-3` | `>= 5e-2` | **missed** |
| LC-s (reported) | Full solve with BBar recovery `9.9e-3` (5 t*), `1.13e-2` (20 t*); with Full recovery `0.103`, `0.362` | | |

The three LC limits are missed and stay as registered: the test is kept as the ignored record
`f597_run1_known_miss_displacement_and_creep_strain_witnesses`. Diagnosis: the locking of the Full Hex8 on the creep cylinder is
a pressure (volumetric stress) oscillation; the displacement and the creep strain of the Full solve differ from the BBar solve
by only `1e-3` to `4e-3`. The estimate "of the order of the stress error" was wrong, and the witnesses LC-u and LC-e are not
sensitive to what MU5 changes. Also the LC-s row confirms ADR 0506's finding: the BBar recovery of a Full solve (`1.1e-2`) is
inside C2/C3's `2e-2`, so no stress measure can tell the MU5 combination from a BBar solve.

The set-up of the control, not the limit, was defective (the wrong witness). A corrected witness, fixed here before its run, from
the physics and without a probe: the recovery-free internal force. If `u_B` solves the F-bar equations, then
`f_B = internal_force(BBar model, u_B, s_B)` is the external load; the same field and states through the Full element
(`f_F = internal_force(Full model, u_B, s_B)`, `creep_run(FULL)` gives the Full model on the same mesh and dof map) differ
by the volumetric stress the Full element cannot represent. When the solve is Full (MU5) both are the same element and
`f_F = f_B` to the bits.

| Id | Clause | Measure | Limit | Estimate |
|---|---|---|---|---|
| LC-f5 | 2 | `max |f_F - f_B| / max |f_B|` at `5 t*` | `>= 1e-2` | the stress difference is `0.10 q` (LC-s) at points, integrated against `B` the cancellation of an oscillating pressure leaves a fraction; `~1e-2` to `1e-1` believed, not probed |
| LC-f20 | 2 | the same at `20 t*` | `>= 3e-2` | LC-s grows by 3.5 from `5 t*` to `20 t*` |
| NC2 | 2 | the same at `t = 0` for the elastic start (`nu = 0.3`) | `<= 1e-2` | NC1 shows the elements agree to `4e-5` there |

C1' to C4', SC and NC1 continue unchanged in the corrected test `f597_c4_prime_and_the_force_witness_control` (they repeat
the bits of run 1, they are not re-run to pick a result). MU5 is expected to be caught by LC-f5 and LC-f20 (difference zero).
The decision rule is unchanged, with LC-f5, LC-f20 and NC2 in place of LC-u5, LC-u20 and LC-e20; because the three
displacement/creep-strain checks missed, the best status reachable is **partial** unless the lead rules the amendment a
set-up correction only: it is reported as partial either way.
