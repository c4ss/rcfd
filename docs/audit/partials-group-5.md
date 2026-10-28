# Partials audit, group 5

Read-only audit of ten roadmap items with status `partial` (docs/roadmap.json at 272eb89). Grades follow the acceptance text as
written, from tests re-run by the auditor (release build, own target directory) unless marked "read only". Earlier notes were
not trusted; where a note and a measurement disagree, the measurement is recorded.

Grades: DONE = acceptance met by a passing, non-ignored test with numeric limits; PARTIAL = a tested slice exists, a named clause
is unmet; TODO = nothing usable; OVERSTATED = the note claims more than the tests show.

| id | title (short) | grade | evidence run or read | residual gap against the acceptance | size | blockers | one wave | user decision |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| F-027 | aggregation AMG | PARTIAL (note sub-claim OVERSTATED) | `rcfd-linalg --lib amg --include-ignored` (5 pass, 1M test 3.3 s); auditor scratch crate measuring V vs W cycles 4k..1M | stand-alone V-cycles at 1M: 24 to 1e-6, 32 to 1e-8 (not under 15, rate 0.34 -> 0.56, not mesh independent); 1M test is `#[ignore]`d | S (reword + un-ignore) or M (make V-cycles mesh independent) | none | yes | yes (reword acceptance to AMG-PCG or W-cycles) |
| F-051 | conjugate heat transfer | DONE | `rcfd-fv --test cht`, `rcfd-api --test cht_case --test coupled_cht` | none against the acceptance; note's "missing" list is scope beyond it | - | - | - | yes (close it; move extra scope to F-220/F-215) |
| F-092 | Claude Code panel (optional) | PARTIAL | `claude plugin validate` (pass, icon warning) and `claude plugin test clients/claude-panel` (5 pass) | no test of an edit round trip against a running `rcfd serve`; never checked in a live host | S | `claude` CLI and `rcfd` on PATH | yes | yes (item is demoted: close as won't-do or keep) |
| F-111 | material property library | PARTIAL | `rcfd-core --lib fluids iapws --include-ignored` (15 pass, 1 known miss) | air viscosity: Sutherland misses 1 % at 400 K (-1.08 %) and 500 K (-1.46 %); only the in-sample cubic fit passes | S | none | yes | no |
| F-125 | public-data validation pack | PARTIAL | `rcfd-io --test datapack` (4 pass); manifest read | `hannover-compressor` has no fetch file, no checksum, licence "unresolved" | S once a dataset is chosen | user decision; network for a new upstream | yes, after the decision | yes (replace or drop Hannover) |
| F-136 | FW-H far field | DONE (note stale) | `rcfd-post --test fwh --test fwh_impermeable`, `rcfd-cli --test fwh_cli` | none against the acceptance (rotating point force W3, ADR 0066, now covers the rotating dipole) | - | - | - | no (update the note) |
| F-205 | initialisation methods | PARTIAL | `rcfd-api --test init_methods` (I1, I3 pass), `rcfd-fv --test init_potential --include-ignored` (I2 miss reproduced) | backward-facing-step clause: potential 258 vs uniform 253 iterations; coarse start never tried on the step | S | none | yes | no |
| F-215 | interface physics | DONE | `rcfd-fv --test cht` (E1, E2), `--test interface_w21a` (G3), `rcfd-api --test thermal_extras_case` | none against the acceptance; title scope (layer capacity, in-plane shell conduction, mapped interfaces, switching) is untested | - | - | - | yes (close, or move title scope to a new item) |
| F-227 | wave generation and absorption | PARTIAL | `rcfd-fv --test waves --include-ignored` (15 pass, 3 known misses); `rcfd-fv --test vof_waves` (6 pass, 706 s) | Stokes elevation and dispersion shown in the library only; the VOF tank runs Airy waves only and does not measure wavelength | M | runtime (tank tests take over 10 minutes) | yes | no |
| F-237 | format registry, structural loads | PARTIAL | `rcfd-post --test w27c_structural`, `rcfd-io --test w27c_registry` (all pass) | public CGNS round trip; heat-rate export; CGNS/HDF5 are listed but have no conformance case | M-L | HDF5/CGNS dependency (user decision), public CGNS file (network) | no | yes |

Counts: DONE 3 (F-051, F-136, F-215), PARTIAL 7, TODO 0. One OVERSTATED sub-claim (F-027 V-cycle rate).

## F-027 algebraic multigrid

Re-run: `poisson_one_million_cells` (ignored) passes in 3.3 s total: 5 levels, operator complexity 1.68, 14 W(1,1)-cycles to 1e-6
(rate 0.368), AMG-CG 11 iterations to 1e-8. The acceptance asks for V-cycles. The auditor measured them with a scratch crate outside
the repository (path dependency on rcfd-linalg, same Laplacian and right-hand side as the in-tree test; it reproduced the in-tree W
numbers exactly). 3-D Poisson, stand-alone cycles to 1e-6 / 1e-8:

| unknowns | V (cycle_index 1) | W (cycle_index 2) | AMG-CG with V / with W, to 1e-8 |
| --- | --- | --- | --- |
| 4 096 | 13 / 18 (rate 0.34) | 13 / 17 | 10 / 10 |
| 32 768 | 15 / 20 (0.38) | 12 / 16 | 11 / 10 |
| 262 144 | 19 / 25 (0.48) | 14 / 18 | 12 / 10 |
| 1 000 000 | 24 / 32 (0.56) | 14 / 19 | 14 / 11 |

So plain V-cycles are neither under 15 nor mesh independent. The note's "rate 0.19 at 4k to 0.45 at 1M, mild" understates this
(measured 0.34 to 0.56): OVERSTATED. The in-tree `mesh_independent_convergence_2d_and_3d` builds with `cycle_index: 2` but prints
and asserts "V-cycles" (16 to 18 to 1e-8, asserted at <= 20); this mislabel probably caused the error in the note. W-cycles reach 1e-6
in 14 at 1M (rate flat 0.31 to 0.37). AMG-preconditioned CG with W-cycles is flat at 10 to 11 iterations to 1e-8.
Gap: either (a) reword the acceptance to "AMG-preconditioned CG (one W-cycle per iteration) reduces the residual by 1e-8 on a 1M-cell
3-D Poisson problem in at most 15 iterations, with the count growing by at most 2 from 4k to 1M", then un-ignore the 1M test (3 s in
release) and fix the label; or (b) keep the text and make the V-cycle mesh independent (better aggregation or smoothing), size M with an
uncertain outcome. Also unmeasured: thread-parallel setup and smoother. User decision: which acceptance.

## F-092 Claude Code panel

Optional and demoted (not on the critical path). The plugin validates (one warning: no icon) and its 5 tests pass. They test only helpers
and that the pane draws run controls and the empty monitors view. Nothing tests "edits round-trip to the service" against a local
`rcfd serve`, and the plugin was never checked in a live host session. Closing it means one integration test that starts `rcfd serve`,
sets a value through the panel model and reads it back (S). User decision: close as won't-do, or keep as partial with no work planned.

## F-111 material property library

Re-run: IAPWS-IF97 regions 1 and 2, B23, saturation, and the viscosity and conductivity verification tables pass. Mixture rules
(mole/mass fractions, ideal-volume, Wilke limits) are unit-tested. Air cp is within 0.12 % and Sutherland conductivity within 0.69 %
(300 to 500 K). Air viscosity: Sutherland gives 18.459, 22.852, 26.704 uPa s against 18.5, 23.1, 27.1 (-0.22, -1.08, -1.46 %), so the
1 % clause fails at 400 and 500 K (known-miss test reproduced). The cubic fit passes, but it was fitted to the same three-digit rows,
so that pass is circular. The passing test asserts Sutherland viscosity at 1.5 %, a guard set after the measurement. Gap: an air
viscosity law that is not fitted to the check table and meets 1 % from 300 to 500 K. Example: a published air correlation, coded
from the paper and checked against Huber. The auditor suggests Lemmon and Jacobsen (2004) for air, but recalled that citation from
memory (UNVERIFIED). Alternatively, register a held-out check of the fit against a
second, independent table. Size S. Unverified: mixture-rule formulas, PR/RK constants and atomic weights are recalled from memory
(docs/reference-data.md says so). The Huber rows were read from a rendered NIST page. The auditor recalls (UNVERIFIED) other textbook
tables giving about 23.0 uPa s at 400 K, so the 400 K miss is sensitive to the table chosen; the 500 K miss (over 1 %) is not.
Licensed-data clause: `git ls-files` finds no file matching refprop, .fld, .hmx or .mix (checked at 272eb89). That clause is met.

## F-125 public-data validation pack

Re-run: manifest validation, in-repo checksums, tamper and proprietary-source rejection, and the provenance report pass. 30 datasets
are in the manifest. Every one has a licence record and SHA-256 sums except `hannover-compressor` (status unresolved, no files, no
licence; the reason recorded is that the only located upstream is CC-BY-NC). The acceptance is "each dataset has a fetch script,
checksum and licence record". That holds for every dataset except Hannover. The note's other gaps (Rotor 37 numbers not transcribed,
NACA 0012 not compared) are beyond the acceptance as written. The Zenodo fan entry `w34d-fan01-fau-eaa-axial-fan` stores two
in-repo derived CSV files. Each has a SHA-256 checksum and a `source_url` (a DOI), but no `url` field that fetch.py downloads. That is
why the three-host allow-list still passes. It has a licence and checksums; whether in-repo derived tables count as "a fetch script"
is the same convention as the other transcribed-table entries. Gap: Hannover only. User decision: (a) drop Hannover
from the item, (b) replace it with an open compressor or fan dataset (the CC-BY 4.0 FAU axial fan entry `w34d-fan01-fau-eaa-axial-fan`
is a candidate), or (c) accept a CC-BY-NC fetch-only entry. Any of these is S with network access.

## F-205 initialisation methods

Re-run: I1 cylinder potential flow: surface error 9.74e-4 of 2A on M2, order 2.02 (limit 3 %), met. I3 cavity coarse start: 112 vs
165 iterations. I2 backward-facing step: uniform 253, zero 258, potential 258, potential velocity only 256: the registered clause fails
(test ignored, reproduced; 4.5 s). ADR 0019 explains why: potential flow cannot represent the recirculation that sets the slow SIMPLE
mode. The acceptance reads "an initialised backward-facing-step case", not "a potential-initialised" one, and `initial.method = coarse`
was never tried on the step. Gap: a pre-registered coarse-start step run (same set-up as I2; the limit "fewer fine iterations than
uniform" fixed before running, and whether coarse work is counted stated up front). Size S, about 5 s per run. The acceptance is
reasonable as written. If the coarse start also misses, the honest outcome is a known miss plus a user decision on rewording.

## F-227 wave generation and absorption

Re-run: library tests `waves.rs` (15 pass). Finite-depth dispersion residual 2e-15. The Stokes second-order formulas satisfy both
free-surface conditions to second order. The 1-D toy beach gives R = 2.6e-4. Known misses W3, W7d and W8-4 were reproduced (all
documented set-up or premise defects). VOF tank `vof_waves.rs` re-run (6 pass, 2 ignored known misses, 706 s with 2 test
threads), matching ADR 0052: Airy wave height -10.5 % at 0.5 lambda and -18.0 % at 1.5 lambda, periods 0.9997 and 0.9951 s, beach
K_r = 0.0142 in the 20-period run (meets the 5 % reflection clause), control wall 0.524, volume identity 6e-14, inlet elevation
1.4e-16 m.
Against the acceptance: "reflected-wave fraction below 5 percent with the beach" is met in a 2-D VOF tank at one resolution (lambda/32).
"Linear dispersion relation and Stokes wave elevation reproduced" is met only by the library formulas checked against themselves
(dispersion residual, free-surface conditions). The tank runs Airy waves only, measures period and height but not wavelength or phase
speed, and loses 10 to 18 % of the height to numerical damping. Gap: a tank run that measures the wavelength (two-gauge phase) against
the dispersion relation, and a Stokes-2 run that compares the second-harmonic elevation, each with a pre-registered limit and a
refinement pair. Size M; the tank tests already take over 10 minutes, so new runs need an `#[ignore]` long-test tier or a smaller tank.
Unverified: the Stokes-2, JONSWAP and blending citations (docs/reference-data.md).

## F-237 format registry and structural load export

Re-run: 12 supported formats pass their conformance cases, tampering is detected, the deck round trip is bit exact, and the plane
force is exact. Tube force: the test asserts |dF|/S = 1.55e-4, where S is the integral of |p| dA. The auditor computed |dF|/|F| from the
printed vectors (7852.58 vs 7846.98 N): 7.1e-4. That is below the 0.1 % acceptance, so the force clause is met, but the test's
normalisation is about 4.6 times looser than the acceptance, and the margin is not asserted. The test should also assert |dF|/|F| <= 1e-3.
Not met: (1) the public CGNS example round trip (no CGNS reader or writer); (2) the heat-rate export and its 0.1 % clause (only nodal
temperature, exact for linear fields, is exported); (3) "each listed format has a pass/fail conformance case": `cgns` and `hdf5` are
listed in the registry as NotSupported with no case. Blockers: a CGNS implementation needs an HDF5 dependency (or the legacy ADF
container) and a public sample file fetched over the network. This is a user decision on the dependency. Heat-rate export is S to M
on its own and closable in one wave. Unverified: the deck card layouts other than PLOAD4 and TEMP, and the PLOAD4 sign convention (ADR
0051). Suggested rewording if CGNS is out of scope: limit the conformance clause to supported formats, and move CGNS to its own item.

## DONE items (short)

- F-051: the acceptance "1-D slab + fluid analytic CHT within 1 percent" is met. The prescribed-flow duct error is 1.5e-4 -> 9.6e-6
  (order 2.0, `d1_d2_..._central`). With a solved flow (coupled algorithm), Nu is 8.2367 vs 140/17, error 1.7e-4 (`coupled_cht d2`).
  The convection-BC slab is at 8e-15. The ignored upwind D1/D3 and solid-form D4 tests are documented set-up misses, not acceptance
  gaps. Every "missing" item in the note (segregated solved-flow CHT, orthotropic or temperature-dependent solids, shell capacity,
  radiation) is scope beyond the acceptance.
- F-136: monopole 4.1e-6 to 4.5e-6, rotating monopole 4.6e-6, and the rotating point force against an in-repo Gutin-type
  far-field formula, 1.2e-7 to 4.0e-5 relative, the largest at theta = 135 degrees where thrust and torque terms partly cancel
  (limit 1e-2; ADR 0066 W3). The sampled-file path is the JSON round trip (A7) plus
  the CLI equal to the library to 0 (S3). W3 itself runs through the library, not a file, so the file clause holds by these two steps
  combined. The Gutin formula was derived in ADR 0066, not checked against Gutin (1936) or Lowson (1965): UNVERIFIED citation. The
  W5 convective misses belong to F-243. The note is stale: it still says the rotating dipole is only a monopole pair.
- F-215: E1 contact-resistance jump within 2.3e-14 relative (limit 1 %). For the layered wall, G3 (layered interface equals resolved
  layers, 1.1e-14) and E2 (thin wall equals resolved layers, 1.9e-14) together with B1 (resolved composite wall equals the analytic
  solution, 3.4e-13) give the composite-wall match by these steps combined. Tree checks G1, G2 and G4 pass. G5 at dt 0.01 is a
  documented round-off known miss. The title promises more than the acceptance tests (layer capacity, in-plane shell conduction,
  solution-dependent switching, mapped interfaces).

## Not re-run

Every test named above was re-run at 272eb89, including the ignored known-miss tests, except the two ignored `vof_waves` known misses
(w2, w4). Those failures are documented in ADR 0052, and their corrected versions passed. Not checked: the F-092 live-host behaviour.
The F-027 V-cycle numbers come from an auditor scratch crate, not from a repository test.
