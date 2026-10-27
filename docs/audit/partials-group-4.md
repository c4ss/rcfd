# Partials audit, group 4

Read-only audit of ten roadmap items with status `partial` (docs/roadmap.json at 07ae2db). Grades follow the acceptance text as
written, from tests re-run by the auditor (release build, own target directory, Windows, two test threads) unless marked "read
only". The earlier notes were not trusted; where a note and a measurement disagree, the measurement is recorded.

Grades: DONE = acceptance met by a passing, non-ignored test with numeric limits; PARTIAL = a tested slice exists, a named clause
is unmet; TODO = nothing usable; OVERSTATED = the note claims more than the tests show.

| id | title (short) | grade | evidence run or read | residual gap against the acceptance | size | blockers | one wave | user decision |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| F-014 | mesh readers (polyMesh, CGNS, Fluent) | PARTIAL (note stale on samples) | `rcfd-io --lib openfoam` (4 pass), `--test fluent_public` (public elbow.msh read, passes), `--test w27c_registry` (4 pass) | CGNS reader absent (registry: NotSupported, no HDF5/ADF layer); OpenFOAM reader only on hand-built and self-written meshes | L | HDF5/CGNS dependency or own HDF5 subset reader (user decision); a public CGNS sample (network) | no | yes |
| F-050 | energy, Boussinesq, ideal gas | DONE | `rcfd-fv --test natural_convection` (5 pass, 28 s) | none against the acceptance: Nu +0.02, +0.08, +0.44 percent at Ra 1e4, 1e5, 1e6 (limit 2 percent); test reference constants equal the Table V values read from the paper scan | - | - | - | yes (close; note's "not done" list is beyond the title) |
| F-083 | Python bindings (PyO3) | PARTIAL | `rcfd-cli --test python_w20c` (harness ran the 11 Python tests, "Ran 11 tests" asserted) | no wheel or pyproject; no PyO3 extension; numpy path never tested (the harness runs `python -I`, where numpy is not importable, and the tests pass `as_numpy=False`); cavity run compared to the CLI only, not to a reference | S (pure-Python wheel + numpy test) / L (PyO3) | PyO3: `forbid(unsafe_code)`, pyo3 not in offline cache (user decision) | yes for S, no for L | yes |
| F-108 | boundary-layer inflation | PARTIAL (note OVERSTATED on the y+ check) | `rcfd-mesh --test inflate` (8 pass), `rcfd-api --test inflate_report` (4 pass), `rcfd-turb --test flat_plate_sst -- --ignored --exact sst_flat_plate_cf` (pass, 131 s) | solved first-cell y+ 0.48 to 0.54 for target 0.5 (within 10 percent) is printed only, never asserted, and the test is ignored; no solved case on an inflated unstructured wall patch | S (assert y+) / L (unstructured clause) | unstructured clause: F-234 volume fill | yes for S, no for the rest | no |
| F-124 | cell registers | PARTIAL | `rcfd-post --test registers` (9 pass, 1 known miss), `rcfd-api --test registers` (5 pass) | no register type in the settings tree thresholds a solution field (tree `expression` evaluates x, y, z only); the tree patch and source tests use a box register; no test chains field threshold, patch and source | S-M | none | yes | no |
| F-135 | viscoelastic models | PARTIAL | `rcfd-fv --test w27c_viscoelastic_channel` (5 pass, 1 ignored, 80 s), `--test w27c_viscoelastic_homog` (9 pass, 2 ignored) | planar contraction not attempted; no case-tree wiring; acceptance asks for an analytic Oldroyd-B contraction solution, which does not exist | M-L | acceptance rewording (user decision); benchmark reference data | no | yes |
| F-204 | wall motion and shear | DONE (caveat) | `rcfd-api --test wall_motion` (9 pass, 5 ignored, 540 s) | none against the acceptance: Couette 1.1e-13, thermocapillary 3.25e-5 (limit 7.5e-4), Taylor-Couette torque -0.43 percent at n_r 128 (limit 1 percent); the 128 run reports `converged false` (stopped at the iteration cap without converging) | - | - | - | yes (close; turbulence and MRF to other items) |
| F-214 | MHD Lorentz force | DONE | `rcfd-api --test mhd` (5 pass, 56 s), `rcfd-fv --test lorentz` (4 pass) | none against the acceptance: profile error 1.6e-3, 6.5e-3, 6.5e-3 at Ha 2, 10, 20 (limit 2e-2) | - | - | - | yes (close; the note's missing list is scope beyond the title) |
| F-226 | interfacial mass transfer | PARTIAL | `rcfd-fv --test masstransfer` (9 pass), `--test vof_masstransfer` (5 pass) | both acceptance clauses unmet: no 1D Stefan evaporation run, no CFD film condensation compared to Nusselt (library closed form only) | L | energy equation in the VOF host | no | no |
| F-236 | partitioning and reordering | DONE (held partial by user decision) | `rcfd-mesh --test partition_strategies` (9 pass), `rcfd-fv --test reorder` (3 pass), `rcfd-linalg --test reorder`, `rcfd-api --test decomposition` (all pass) | none against the acceptance text; the user's reason (the solver does not use partitions) is not in the acceptance | depends on decision (L if partitions must drive a parallel solve) | user decision; parallel solver (rcfd-dist) | no | yes (reword the acceptance) |

Counts: DONE 4 (F-050, F-204, F-214, F-236), PARTIAL 6 (F-014, F-083, F-108, F-124, F-135, F-226), TODO 0. OVERSTATED
sub-claim: F-108 (y+ "within 10 percent" has no assertion). Stale notes (claiming less than is tested): F-014 ("no third-party
sample files": the public Fluent elbow mesh is read and checked) and F-050 (the cavity reference values are flagged "transcribed
from memory", but the constants in `natural_convection.rs` equal `devahldavis1983_tableV.csv`, which wave 30C transcribed from a
page scan of the 1983 paper, see docs/reference-data.md).

Not re-run: `validation_natural_convection` (all of it), the variable-density and Le Quere tests behind F-050's title scope,
every ignored or known-miss test except `sst_flat_plate_cf`, the F-204 convergence diagnostic and the F-214 transient known misses.

## F-014 mesh readers

Measured: the public `elbow.msh` (fetched data set `fluent-elbow-openfoam`, SHA-256 checked) reads as 918 cells, passes
`check_mesh` (F-011), and its extruded volume equals thickness times the shoelace area of the file's boundary edges to 5.4e-16.
OpenFOAM polyMesh write/read round trip is exact on hand-built meshes (lib tests `round_trip_is_exact`, `cyclic_patches_round_trip`,
`reads_hand_written_sample_with_comments`). Unmet: the CGNS reader (registry status NotSupported, ADR 0048 D7) and any polyMesh written
by OpenFOAM itself (the fetched `dambreak-openfoam` set contains only dictionaries, no polyMesh). CGNS needs either an HDF5
dependency or a hand-written HDF5 subset reader; that is a user decision and a large job. Acceptance rewording proposal: "round
trip" is undefined for the read-only Fluent reader; say "each reader loads a public sample mesh, passes F-011 and the counts and
volume match the file; read-write formats round trip exactly".

## F-083 Python bindings

The pure-Python package (subprocess to the `rcfd` binary) works and its 11 tests run from cargo. Three gaps against the text: (1)
no `pyproject.toml`, so nothing is pip-installable; a pure-Python wheel built with `pip wheel` and installed into a temporary
venv in a test is size S. (2) numpy arrays are never exercised: the harness uses `python -I`, which hides the user-site numpy
2.0.2 on this machine, and the two field reads pass `as_numpy=False`. (3) "reproduces cavity" is shown only as byte equality
with the CLI, not against Ghia et al. The PyO3 extension named in the title is blocked by the workspace `forbid(unsafe_code)` rule
and the offline crate cache. User decision: accept the subprocess package plus a pure-Python wheel as meeting F-083, or keep PyO3.

## F-108 boundary-layer inflation

Re-run of the ignored `sst_flat_plate_cf` (131 s, so it is cheap enough to un-ignore or move to a gated test): target y+ 0.5 at
x = 2; solved first-cell y+ 0.54, 0.53, 0.51, 0.50, 0.48 at x = 0.4, 0.6, 1.0, 1.4, 1.99, all within 10 percent, but the test
asserts only Cf, so the clause has no numeric limit in a test (closable in under a day: add the assertion). Layer metrics are
reported (`rcfd inflate --json`, `mesh.inflate`, `inflate_report` tests). The "unstructured wall patches" clause is not met as a
solved case: `inflate_patch` builds an outward shell that is not merged into a fluid domain, which waits on F-234.

## F-124 cell registers

Library: a value threshold on a cell field (R2), patching (R6) and statistics pass. Settings tree: registers are named objects
and a box register restricts a porous sink to exactly 100 cells and initial patches change exactly those cells. Unmet: the tree's
only threshold form is an expression of the cell centre, so "a region defined by a field threshold" cannot be written in a case;
a `field` register type (threshold on an initial or restarted field, or re-evaluated each outer iteration) and one test that
patches it and restricts a source would close the acceptance. The R5 face-grow known miss is a wording issue in ADR 0011, not a
defect. Adaption in the title is marks only.

## F-135 viscoelastic models

The Poiseuille start-up clause is met (conformation and log-conformation errors 6.57e-3 against 2e-2 at ny 40, modal-series
check 1.0e-4), plus steady Oldroyd-B equals Newtonian total viscosity to 6.5e-3. The planar contraction was not attempted and has
no case wiring. The acceptance is unmeasurable as written: there is no analytic Oldroyd-B solution for a planar contraction.
Proposed wording: "Poiseuille start-up matches the analytic Oldroyd-B series; a 4:1 planar contraction or confined cylinder matches
a published benchmark quantity (corner-vortex length or drag coefficient) within a registered band". The benchmark values would
have to be read from the papers (for example the Alves et al. contraction and cylinder studies; citation from memory, UNVERIFIED).
FENE-P normalisation is marked unverified in the note and stays so.

## F-204 wall motion and shear

Acceptance met. Caveat: the 1 percent Taylor-Couette pass is taken at n_r = 128 where the run stops at the iteration cap (`converged false`); the errors -1.60, -0.82, -0.43 percent at n_r 32, 64, 128 fall at order 0.93, so the cap does not look
decisive, but the test does not assert convergence. The specified-shear run (D2) also reports `converged false` while meeting its
profile limit. Closing it means moving turbulence and MRF combinations to their own items.

## F-214 MHD

Acceptance met. The Ha 10 and Ha 20 profile errors are identical (6.532e-3) because the meshes keep the same cells per Hartmann
length (ny/Ha = 8); the flow-rate errors differ. Out-of-title scope (coupled solver, Joule heating, varying B, 3-D duct) belongs in
new items.

## F-226 interfacial mass transfer

Only library closed forms and the VOF wiring are tested (M1-M9 and the VOF M1-M5 checks pass). Neither acceptance case exists:
the 1D Stefan evaporation problem needs an energy equation with latent-heat feedback in the VOF host, and the film-condensation
clause needs a wall-condensation CFD run compared to Nusselt within 10 percent. Note that the enthalpy-method Stefan melting of
F-212 (`rcfd-api --test phase_change_case`) is a different problem (solid-liquid) and does not count. The Lee, ZGB and Schrage
factors remain UNVERIFIED (docs/reference-data.md).

## F-236 partitioning and reordering

Measured: worst unit-weight imbalance 0.030 and weighted 0.0296 (limit 0.05), graph cut never above the RCB cut, RCM bandwidth 72
against 320 (40x8x8) and 7 against 40 (40x6x1). Every acceptance clause is met by non-ignored tests. Wording nit: the "stretched
mesh" of the RCM test is an elongated index space with uniform cells; bandwidth depends only on connectivity, so this is
equivalent. The item is partial only because the user downgraded it (2026-10-16: the solver does not use partitions). Proposed
acceptance: add "a decomposed parallel run (rcfd-dist) uses the partition and reproduces the serial result to a registered
tolerance", or close the item at library level and move solver use to the distributed-solver item.
