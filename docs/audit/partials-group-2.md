# Partials audit, group 2

Read-only audit of ten roadmap items with status `partial` (HEAD 07ae2db, 2026-10-21). Every grade below rests on tests re-run
for this audit in release mode (`cargo test --release ... -- --include-ignored --nocapture`, own target directory), unless the row
says "read". Ignored `*_known_miss` tests were run too; all of them still fail, which confirms the recorded misses. Paths are given
as plain text.

Grades: DONE = the acceptance text as written is met by passing, non-ignored tests with numeric limits; PARTIAL = a real tested
slice exists, named clauses unmet; TODO = nothing usable; OVERSTATED = the note claims more than the tests show.

Result: 3 DONE (F-072, F-121, F-202), 7 PARTIAL, 0 TODO, 0 OVERSTATED. Two notes are stale in the other direction (they claim
less than exists): F-132 and F-121.

| id | title (short) | grade | evidence run or read | residual gap | size | blockers | one wave | user decision |
|---|---|---|---|---|---|---|---|---|
| F-002 | CI, lint, licence, clean-room doc | PARTIAL | run: `cargo fmt --all -- --check` exit 0, `cargo clippy --workspace --all-targets -- -D warnings` (result below); read: ci.yml, LICENSE-MIT, LICENSE-APACHE, CONTRIBUTING.md; GitHub Actions API for c4ss/rcfd returned 404 unauthenticated | no evidence of a green hosted CI run | S | user action (repository visibility / Actions enabled / run URL) | no | yes |
| F-045 | LES and hybrids | PARTIAL | run: rcfd-turb sgs_closed_forms 7/7, sgs_mesh 3/3, sgs_taylor_green 7/7 (+2 known misses fail as recorded), viscous_stress_full 4/4 | both acceptance clauses unmet: decaying isotropic turbulence spectrum, channel LES sanity check | L | 3D periodic LES run time; no reference or numeric limit for the spectrum clause | no (channel slice possibly) | yes (reword spectrum clause) |
| F-072 | Derived fields and sampling | DONE | run: rcfd-post derived 7/7, rcfd-api postprocessing 10/10 | title-only: wall shear and y+ laminar only, no lambda2, `rcfd sample` re-runs the case (F-238 holds the rest) | - | - | - | no |
| F-103 | CGNS/EnSight/HDF5 | PARTIAL | run: rcfd-io w26c_ensight 7/7; checked locally: no ParaView, VisIt, h5dump, Python vtk, h5py, pyvista or meshio | acceptance ("open in third-party viewers") untested; CGNS and HDF5 not delivered | S (EnSight check) + L (CGNS/HDF5) | external third-party reader not installed; no HDF5 library | EnSight yes once a reader exists; CGNS no | yes (install a reader, HDF5 route) |
| F-121 | Headless run control | DONE | run: rcfd-cli run_control 5/5, w19c_run_control 7/7, rcfd-api runctl 6/6, rcfd-mcp lib `workflow_and_headless_tools_run_a_case_and_read_the_status_stream`; read: docs/run-control.md exit codes 0-8. Note stale: R1 now reproduces exit 6 ("diverged") through the binary; exit 4 is still not reproduced through the binary, and both R1 tests pass either way | title-only: hostfile (F-100), single precision, control socket, signal handlers, affinity, scheduled commands, checkpoints for compressible and turbulence runs | - | F-100 for hostfile | - | no |
| F-132 | Tet/poly volume meshing | PARTIAL | run: rcfd-mesh tetmesh 10/10 (+torus known miss fails), polydual 3/3, rcfd-fv w19b_meshing_solve 2/2 | boundary layers absent; no STL-file input test; quality limits met on cube and sphere only | M-L (L if F-108 inflation cannot be reused) | none hard (overlaps F-108) | yes, with risk | no (thresholds need numbers) |
| F-202 | User-defined transport equations | DONE | run: rcfd-api user_scalars 6/6 | title-only: vector variant, coupling of user variables into properties and other equations | - | - | - | no |
| F-212 | Enthalpy-porosity solidification | PARTIAL | run: rcfd-fv solidification 15/15 (+8 known misses fail as recorded), rcfd-api phase_change_case 3/3 | gallium-cavity clause: no melt-flow coupling in the case tree, no reference data; pull velocity absent | L | reference data (gallium experiment) not obtained; melt-flow coupling | no | yes (obtain data, reword "trend") |
| F-224 | Chemistry input formats | PARTIAL | run: rcfd-reactor slices k12a/k12b, reactor `gri` filter (r3_gri_reads 53 species / 325 reactions; r3 NASA continuity known miss fails as recorded); counted gri30.yaml: 53 species, 325 reactions | no second-format reader (YAML); no three-condition hydrogen-air equilibrium comparison | M | none (data present locally) | yes | yes (YAML crate vs subset parser) |
| F-234 | Octree fill with prism layers | PARTIAL | run: rcfd-mesh octree 9/9 (+O5 known miss fails: 77.41 deg), rcfd-fv octree_cavity 1/1 | sphere quality miss (non-orthogonality); sphere drag Re 100 not run; flat-plate y+/Cf not attempted | L | drag reference not in repo | no | no |

Clippy result (run for F-002 at HEAD 07ae2db): exit 0. Recommended status flips: F-072, F-121 and F-202 to done, with their
title-only remainders moved to new items or to F-238 / F-100.

## F-002 (PARTIAL)

Measured: `cargo fmt --all -- --check` passes at HEAD; `cargo clippy --workspace --all-targets -- -D warnings` passes (exit 0, no warnings); the full workspace test suite was NOT
re-run for this audit. The
licence files and the clean-room and sourcing rules in CONTRIBUTING.md are present. The unmet clause is "CI runs": the GitHub
Actions API for the origin repository returns 404 without authentication, so whether a hosted run ever happened (origin/main is at
5e300f9, wave 32) cannot be shown either way from here. Two things will differ from the local gate when it does run: ci.yml runs
`cargo test --workspace` in debug mode on the stable toolchain (the local gate uses `--release`; MSRV 1.75 is not tested), and
`validation-data/data/` is gitignored, so every test that reads the downloaded data pack returns early on CI. Closing it needs the
user to make the run visible (or authenticate `gh`). Proposed rewording: "ci.yml runs fmt, clippy -D warnings and tests; one green
hosted run is linked, or the identical commands are recorded as a clean-checkout gate in an ADR".

## F-045 (PARTIAL; no acceptance clause is covered)

The delivered slice is real: Smagorinsky and WALE in PISO/PIMPLE, closed forms, linear-field checks and Taylor-Green checks. With
`solver.viscous_stress = full` the registered TG1 assertion passes (measured Smagorinsky extra dissipation ratio 0.9173, limit
1 +- 0.15), the WALE closed form passes (ratio 1.1448, marginal), the ordering holds (K_W/K_S 1.0587) and the energy never rises.
The Laplacian-form known misses still fail as recorded. Neither acceptance clause has any test. Decaying isotropic turbulence: no
3D periodic box test, no reference spectrum in the repository (docs/reference-data.md: "not verified, not run, no claim made");
a 3D box can be built from `gen::box_mesh` plus `Mesh::set_periodic` on three pairs, so the blocker is the reference and run time,
not the mesher. Channel LES: verified mean-profile data for Re_tau 178 (Moser, Kim and Mansour, chan180, "verified (file read)")
is already in docs/reference-data.md, so reference data is not the blocker; the blockers are a 3D periodic channel with a driving
pressure gradient in PIMPLE (not checked whether PIMPLE supports one) and hours of run time; the full viscous-stress option also
lacks boundary-face contributions at walls (ADR 0032). DES lives under F-207 (rcfd-turb des.rs and rcfd-api des_tree.rs exist; not run for
this audit). Unverified: Cs 0.17, Cw 0.325, A+ 25 (from memory). Proposed rewording: "Taylor-Green
(2D) dissipation checks; channel LES at Re_tau 180 on a coarse mesh gives U_b+ within 10 percent of the DNS and a log-layer slope
within 15 percent; decaying-turbulence clause replaced by a 3D Taylor-Green or decaying box with a numeric spectrum-slope limit
over a stated wavenumber band, or dropped".

## F-103 (PARTIAL)

EnSight Gold writer passes its own tests (hand-written reader in the test file, and the ADR 0048 addendum records an independent
stdlib-Python reader). The acceptance clause is that exports open in third-party viewers, and no third-party reader exists on
this machine (checked: no ParaView, VisIt, h5dump, Python vtk, h5py, pyvista or meshio). The facts a viewer would catch, hexa8
node order, nfaced face orientation and the 4-byte binary widths, are from memory and unverified (ADR 0048). Installing an
open-source reader (for example the VTK EnSight Gold reader from the `vtk` Python wheel) would make the EnSight half an S-size
check; that is a user decision about an external tool. CGNS and HDF5 are not delivered at all (no HDF5 library, ADR 0048 D7), size
L, and need a user decision on a dependency or a pure-Rust HDF5 writer. Proposed rewording: name the reader and its version, and
require counts, cell volumes and one field to match after reading.

## F-132 (PARTIAL; note stale)

Measured: cube and sphere meshes have radius-edge fraction 1.0 and minimum dihedral 29.5 and 55.1 degrees; sphere, cube,
cylinder-in-box and torus volumes match to 1e-10; the generated sphere solves a Laplace problem (w19b_meshing_solve). The torus
known miss still fails (fraction 0.634 against 0.99); the remeshed torus reaches 1.0; the cylinder-in-box pipeline prints fraction
0.927 and minimum dihedral 3.34 degrees but asserts no quality limit. The note says polyhedral dual conversion is not done and
F-102 stays todo; that is stale: rcfd_mesh::polydual (F-102, ADR 0054) exists and `a5_dual_of_tetrahedralised_sphere` checks
that the dual of a generated tet mesh preserves volume to 1e-13 relative, so the "polyhedral conversion preserves volume" clause
is met. Unmet: boundary layers (no test applies prism layers to a tet mesh; whether F-108 inflation can be applied to the tetmesh body
patch was not checked), and "closed STL geometry" (all inputs are procedural surfaces; the STL reader in rcfd-io is never fed to
`tetrahedralize`). The acceptance gives no numeric quality thresholds; propose naming them (radius-edge fraction, minimum
dihedral, non-orthogonality) and the STL file.

## F-212 (PARTIAL)

The Stefan clause is met: one-phase melting at N = 240 relative front error 2.88e-3 (corrected initial state 1.33e-3), two-phase
melting and solidification 6.27e-5 (corrected 1.42e-3), limit 3e-2, and the case tree equals the library (T1). The order-criterion
misses (P1, P2, P8, their v2 variants) and K2 still fail as recorded; they are not part of the acceptance. The gallium-cavity
clause is untouched: Carman-Kozeny damping is library-only (not in the case tree), there is no buoyant melt-flow coupling, and the
experimental front shapes were never obtained (docs/reference-data.md: "not obtained"). Pull velocity is absent. "Front shape
trend agrees" has no numeric limit; propose "front position at three heights at two or three published times within a stated
distance of the digitised experiment", which needs the user to supply or approve the source data.

## F-224 (PARTIAL; closable)

Delivered and passing: NASA nine-coefficient polynomials on species, a lossless JSON interchange (the repository's own format, so
it does not count as a second public format), and CHEMKIN import of GRI-Mech 3.0 (`r3_gri_reads`: 53 species, 325 reactions).
Unmet: a second public format and the three-condition hydrogen-air equilibrium comparison. The data needed is already on disk:
validation-data/data/gri-mech-30/gri30.yaml (Cantera YAML with its licence file) has 53 species and 325 `equation` entries by
count, and `equil::equilibrium_tp` exists. Residual: a YAML reader for the mechanism subset (units block cm/mol/cal, NASA-7
thermo, Arrhenius, three-body, falloff), then compare counts and equilibrium states at three (T, p, phi) points. Size M, one
wave. User decision: add a YAML crate or write a subset parser. Caveat: `validation-data/data/` is gitignored, so the test must
fail loudly or be recorded as data-dependent, not skip silently as the existing `.ok()?` reactor tests do.

## F-234 (PARTIAL)

Measured: sphere in box valid with positive volumes, hex fraction 0.889, void-volume order 1.84; offset rules exact. Quality miss
confirmed: non-orthogonality 77.41 degrees against 70 (all 240 offending faces are layer-to-layer faces of the snapped shell).
Cavity on the generated mesh: max |u - Ghia| 0.00456 and |v - Ghia| 0.00797 against 0.03, which meets the clause as written, but on
an unrefined uniform 40 x 40 x 1 fill (ADR 0047 deviation 3), so no hanging-node face enters the flow solve, and "3 percent" was
read as 3 percent of the lid speed. Not run: sphere drag at Re 100 (no reference in the repository; any correlation recalled from
memory is unverified). Not attempted: flat-plate last-ratio layers (the Blasius reference, Lal and Martin 2022, is verified in
docs/reference-data.md, so this clause is unblocked). Size L overall; the flat-plate clause alone is M.
