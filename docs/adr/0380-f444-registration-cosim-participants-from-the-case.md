# ADR 0380: F-444 registration: co-simulation participants from the case, hosted by `rcfd serve`

Status: **REGISTERED 2026-11-13**, written before any code of F-444 and before any run of its checks. Not yet accepted by the
maintainer. Before this text nothing of F-444 was compiled or run (a release build of the unchanged tree was started to warm
the target directory; no test was run): the code and documents under "What was read" were read, and no arithmetic was
done beyond what is written out below. Results go in the addendum at the end; nothing above the addendum is changed after
a run.

Parents: [ADR 0345](0345-f360-registration-cosimulation-over-the-service.md) (F-360: `rcfd_api::cosim_service`, the
protocol `rcfd-cosim/1` over the service, the proxies, the mock structure in a second process; its questions Q3, Q4 and
Q5, from which this item comes; the recommendations were taken on the maintainer's standing instruction of 2026-11-11),
[ADR 0256](0256-f139-registration-cosimulation-participant.md) (F-139: `rcfd_fv::cosim`, the model problem CWs, checks A1
and A2), [ADR 0263](0263-f362-registration-interface-mapping.md) (F-362: the map builders `interface_rows`,
`InterfaceMap::build`, which take the structure's elements). Roadmap item F-444 (depends on F-360). Number 0380 given by the
lead. Branch `f444-cosim-serve` from main 285eaf22, worktree `w49i-work/f444`.

House rules (`w49i-work/AGENT_RULES.md`):

- limits are fixed here, before the code and the runs, and are not changed after a run; a missed limit becomes an ignored
  `*_known_miss` test with the limit intact and the number recorded, diagnosed, not tuned away; a defective set-up gets a
  corrected test by an amendment committed before the corrected run, the old one stays as an ignored record; each
  registered run is made once;
- defaults stay bit-identical: a case without the new table, a service started without the new flag and a structure that
  gives no elements behave exactly as before; `crates/rcfd-api/tests/baseline_hashes.rs`, the hash tables
  (`case_hashes_w48u.rs`, `w48v_case_hashes.rs`) and the goldens (`crates/rcfd-mcp/tests/golden`) are not edited; no
  existing test is edited;
- no new third-party crate, no unsafe code, the crate graph unchanged, no commit trailers, no push.

Clean-room. Nothing here is an algorithm from the literature: the item is plumbing between parts that exist (the case
tree, the simulation builder, the flow participant `FlowWall`, the smoothing, the service, the proxies). The model problem
and its closed form are ADR 0256's; nothing else was consulted.

## What F-444 asks

The acceptance text of `docs/roadmap.json`, binding:

> A case key names a moving wall patch as a co-simulation fluid participant and rcfd serve hosts it under protocol
> rcfd-cosim/1; a structural participant in its own process (the mock of ADR 0345) and the coupler of rcfd_fv::cosim drive
> it over TCP and reproduce ADR 0256 A1 and A2 with the displacements of the in-process run to 1e-12; describe of a
> structure may carry its elements so that the map builders of ADR 0263 run on the side of the coupler; service::Client
> writes a request with one call; without the key every case hash is unchanged and the schema, CLI help and MCP goldens
> change only by the approved additions.

Clauses: (c1) a case key names a moving wall patch as a co-simulation fluid participant; (c2) `rcfd serve` hosts it under
`rcfd-cosim/1`; (c3) the mock structure of ADR 0345 in its own process and the coupler `coupled_step` drive it over TCP and
reproduce A1 and A2; (c4) with the displacements of the in-process run to `1e-12`; (c5) `describe` of a structure may carry
its elements, and the map builders run on the coupler's side; (c6) `service::Client` writes a request with one call; (c7)
without the key every case hash is unchanged; (c8) the schema, CLI help and MCP goldens change only by the approved
additions.

## What was read (no run)

1. ADR 0345 in full (registration, amendments 1 and 2, addendum and its questions Q1 to Q5), ADR 0256's checks A1 and A2 as
   ADR 0345 restates them, ADR 0263's builder API (`MapSettings`, `interface_rows`, `InterfaceMap::build`), ADR 0266 read 6
   (an optional table stays out of the data, the two texts and the hash of every case that does not write it, and the
   goldens of `crates/rcfd-mcp/tests/golden` hold no case-schema key) and ADR 0352's T (the same reading used for
   `[solver.rhie_chow]`).
2. `crates/rcfd-fv/src/cosim.rs`: the traits; `FlowWall::new(flow, patch, reference, rho, nu, motion)` (the name is always
   `fluid`; the wall points are the patch points in ascending mesh index; a set `corrector_control` is refused);
   `SprungWall`. `crates/rcfd-fv/src/cosim/mapping.rs`: `interface_rows(nodes, elements, points, settings)`.
3. `crates/rcfd-api/src/cosim_service.rs` (hosting on a thread of its own, `cosim.describe` of a structure returns
   `{kind, name, nodes}`, the proxies), `crates/rcfd-api/tests/cosim_service_f360.rs` (the set-up CWs, the child process,
   the comparisons; P1 asserts `METHODS.len() == 69` and that the last eight are the `cosim.*` methods, so no method can
   be appended to `METHODS`), `crates/rcfd-api/tests/data_service_f447.rs` (pins the `service.info` reply).
4. `crates/rcfd-api/src/service.rs`: `Client::call` writes the request with `writeln!(self.writer, "{req}")` on the
   unbuffered socket, which hands it over in several `write` calls (ADR 0345 amendment 1 measured 2 ms per call for the same
   pattern in the proxy).
5. `crates/rcfd-api/src/sim.rs`: `Simulation::new` = `build_mesh` + `Simulation::from_geometry(case, geom)` (crate-private,
   takes any geometry); the transient branch builds `PimpleSettings::piso(nu)` and overrides it from the case:
   `n_correctors`, `non_orthogonal_correctors`, `alpha_u` (schema default 0.9), `alpha_p`, `p_ref`, `ddt_correction`,
   `component_diagonal`, `viscous_stress`, `divergence_courant_limit` (0 keeps the default), the linear controls
   (`momentum_rel_tol`, `momentum_max_iter`, `pressure_rel_tol`, `pressure_abs_tol`, `pressure_max_iter`; there is **no**
   key for the momentum absolute tolerance, which stays `1e-14`), the time scheme, the lag check (only with one corrector),
   `amg_pressure = n_cells > 2000`. Boundary conditions: every patch needs a `boundaries.<patch>` entry; `zero_gradient`,
   `symmetry`, pressure `fixed`. The generator `box_3d` without a section is `gen::box_mesh([nx, ny, nz], [lx, ly, lz],
   Uniform)`, the mesh of CWs.
6. `crates/rcfd-api/src/dynmesh.rs`: the smoothing classes (moving patches `Prescribed`, fixed patches `Fixed`, every other
   patch `Slip`), the method keys, and the control `SolveControl {tol_abs: 1e-16, tol_rel: tolerance.max(1e-14), max_iter:
   5000}`, which for a tolerance of `1e-13` is exactly CWs' `tight(5000)`.
7. `crates/rcfd-cli/src/main.rs`: `cmd_serve` (`--addr`, `--root`, `--stdio`; prints `rcfd service listening on <addr>`
   with the bound address) and the help line `serve [--addr <host:port>] [--root <dir>] [--stdio]`. No file of the
   repository pins the whole help text (`crates/rcfd-cli/tests/cli.rs` checks that it names each command); no file pins the
   JSON schema as a whole.

## Design

### D1. The case key: an optional table `[cosim]`

An optional top-level table (`Schema::optional`, as `[mesh.adapt]` and `[dynamic_mesh.compressible]`), so that by read 1
it stays out of the data, the written texts and the hash of every case that does not write it:

| Key | Kind, default | Meaning |
| --- | --- | --- |
| `patch` | string, `""` | the moving wall patch: the fluid participant's wall (required when the table is present) |
| `participant` | string, `"fluid"` | the name under which `rcfd serve` hosts it (`cosim.open {participant}`) |
| `fixed_patches` | list of strings, `[]` | smoothing: patches whose points do not move; every other patch but `patch` slides |
| `smoothing_method` | `laplacian` / `inverse_distance` / `inverse_volume` | the diffusivity of the point-displacement equation, as `dynamic_mesh.smoothing.method` |
| `smoothing_exponent` | float, 1 | as `dynamic_mesh.smoothing.exponent` |
| `smoothing_tolerance` | float, `1e-12` | relative tolerance of the displacement solve (the control of read 6) |
| `initial_displacement` | list of floats, `[]` | the wall's displacement from the reference mesh at time 0, three numbers per wall point in the order of `describe` (ascending mesh point index) [m]; empty = 0 |

The reference configuration is the case's mesh (`build_mesh`); the flow starts at rest on that mesh moved by the initial
displacement (the smoothing), as CWs does (ADR 0256 amendment 1: the flow at rest on the mesh of the initial wall). The
initial displacement is a list, not an expression, because CWs' initial wall is the map `H q_0` of the structure's
initial state, which only the coupler knows; the coupler writes it into the case.

**Static checks** (located errors, only when the table is present): `cosim.patch` empty or not a mesh patch;
`cosim.fixed_patches` naming an unknown patch or the moving one; `cosim.initial_displacement` not empty and not three times
the number of wall points, or not finite; `solver.algorithm` not `piso` or `pimple`; `solver.corrector_control =
"adaptive"` (refused by `FlowWall`); `dynamic_mesh.enabled = true` (the participant moves the mesh itself);
`[mesh.adapt]`. Everything the simulation builder would advance outside the flow solver is refused after the build, naming
it (turbulence, energy, scalars, species, chemistry, zone sources, conjugate heat transfer, electric, phase change,
particles, mixing plane, rheology, sources, lumped components, radiation, property laws, boundary catalogue, expression or
user-function boundary values, Marangoni walls, user functions): the participant advances the flow solver alone, and a
model it would silently skip must not look supported. `Simulation::new` (the `rcfd run` path) refuses a case with the table:
"a co-simulation fluid participant is hosted by rcfd serve --cosim, not run".

**The build** (`rcfd_api::cosim_case::fluid_from_case(&Case) -> Result<FlowWall, Vec<Diagnostic>>`): `build_mesh` (the
reference), the wall points (`FlowWall::point_ids`), the smoothing (classes and control of read 6, from the reference
geometry), the mesh at the initial displacement, `Simulation::from_geometry(case, geometry)`, the transient flow solver
taken out of it (a new crate-private `Simulation::into_cosim_flow`, which refuses as above), and `FlowWall::new(flow,
patch, reference, physics.rho, physics.nu, motion)` with the smoothing as the mesh motion. Nothing of this runs for a case
without the table.

### D2. `rcfd serve` hosts it

`rcfd serve --cosim <case-file>` loads the case (relative paths against its directory), builds the participant on its own
thread (`Service::host_fluid`, ADR 0345) under the name `cosim.participant`, prints the diagnostics and exits with failure
when the build fails, and otherwise serves as before (TCP or `--stdio`). A line on standard error names the participant;
standard output is unchanged (`rcfd service listening on <addr>`). No service method is added: `METHODS` and
`service.info` are pinned by the tests of read 3, and the hosted participant answers the eight methods of `rcfd-cosim/1`
that exist. Library: `rcfd_api::cosim_case::host(&Service, &Case) -> Result<String, Vec<Diagnostic>>` (the name hosted).

Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation): a flag of `rcfd serve`, not a
service method `cosim.host` (which would grow `METHODS`, pinned by F-360's P1 and F-447's D1, existing tests). The help
line of `serve` gains `[--cosim <case>]`: the one CLI-help addition (c8), reported for approval.

### D3. Elements in `describe`

`Structure` gains a provided method `fn elements(&self) -> Vec<Vec<usize>> { Vec::new() }` (1 to 4 node indices each, the
convention of `interface_rows`); every existing structure keeps the default. `cosim.describe` of a structure adds the
member `elements` **only when the list is not empty**, so the reply for every structure of ADR 0345 is unchanged;
`RemoteStructure` keeps the member (absent = empty; a member that is not a list of lists of integers fails `connect`) and
returns it from `elements()`. The coupler then calls `InterfaceMap::build(&s.node_coordinates(), &s.elements(),
&f.point_coordinates(), f.face_points(), &settings)` on the proxies: the builders run on its side. The identifier stays
`rcfd-cosim/1` (an added member of a reply needs no new identifier, `docs/cosimulation-protocol.md` "Versions").

### D4. `service::Client` writes a request with one call

`Client::call` formats the request line into one buffer and hands it to the socket with one `write_all` (a private
function `write_request<W: Write>(w, id, method, params)`, so that a unit test can count the writes); the bytes are those
`writeln!` wrote. ADR 0345 Q4.

## Set-up

**CWs as a case** (`CWc`): `mesh.generator = "box_3d"`, `nx, ny, nz = 20, 8, 2`, `lx, ly, lz = 1, 0.4, 0.1`; `physics.nu =
1e-4`, `physics.rho = 1`; boundaries: `xmin` velocity `zero_gradient`, `xmax` velocity `zero_gradient` and pressure `fixed`
0, `ymin`, `ymax` velocity `symmetry`, `zmin`, `zmax` velocity `zero_gradient` (pressure `zero_gradient` elsewhere); solver
`piso`, `convection = "central"`, `n_correctors = 2`, `alpha_u = alpha_p = 1`, `momentum_rel_tol = pressure_rel_tol =
1e-13`, `pressure_abs_tol = 1e-16`, `momentum_max_iter = pressure_max_iter = 5000`; `[cosim] patch = "xmin"`,
`fixed_patches = ["xmax"]`, `smoothing_tolerance = 1e-13`, `initial_displacement` = `H q_0` of CWs (the piecewise-linear
rows of the six nodes at the 27 wall points, `9 x 3` points of the `8 x 2` faces of `xmin`; 81 numbers), written by the
test as JSON numbers (they cross bit for bit, ADR 0345 D3). This is CWs of `cosim_service_f360.rs` except the momentum
absolute tolerance (read 5: `1e-14` here, `1e-16` there), which no key sets; the structure, the map (`piecewise_linear_rows`
of the node and wall-point `y`), the coupling settings C3, `dt = 0.01` and the 650 steps are CWs' unchanged.

**The in-process run** (the reference of c4): `fluid_from_case(CWc)` in the test process, the `SprungWall` of CWs, the map,
`coupled_step`, recording every node's displacement after every step.

**The served run**: `rcfd serve --addr 127.0.0.1:0 --cosim cwc.json` (the binary of the same build,
`CARGO_BIN_EXE_rcfd`, started by the test, the address read from its first line within 120 s, killed when the test
ends); the structure in the test binary started a second time (the helper of ADR 0345, role `structure`; role
`structure-stiff` for the control); the coupler in the test process with `RemoteFluid` and `RemoteStructure`, the map built
from the remote fluid's `describe`. Every socket on `127.0.0.1`, port 0; time-outs of ADR 0345 (5 s connect, 60 s call).

**Duration.** ADR 0345 amendment 2: 650 steps of CWs take two to three minutes on this machine under load. By rule 7 the
650-step checks (A1s, A2s, Ds) are ignored heavy records, run once by me; D100 (100 steps) is the fast test of the same
path.

## Registered checks

Files: `crates/rcfd-api/tests/cosim_case_f444.rs` (in-process), `crates/rcfd-cli/tests/cosim_serve_f444.rs` (the `rcfd
serve` process), a unit test in `crates/rcfd-api/src/service.rs`. Release. "Bits differ" counts values whose 64 bits
differ. `X = 0.002` m.

### Acceptance

| Id | Clause | Check | Limit | Estimate behind it |
| --- | --- | --- | --- | --- |
| K1 | c1 | CWc parses; `fluid_from_case` builds; its `point_coordinates`, `face_points` against the hand-built CWs `FlowWall`; the located errors of D1: `cosim.patch` (empty; `nope`), `cosim.fixed_patches` (`nope`; `xmin`), `cosim.initial_displacement` (80 numbers; a NaN cannot be written in JSON, so `1e400` read as infinity is not tried: an 80-number list only), `solver.algorithm` (`simplec`), `dynamic_mesh.enabled`, `turbulence.model = "k_omega_sst"` with `solver.algorithm = "pimple"` (refused by name after the build), `Simulation::new` of CWc (path `cosim`) | parses, builds; 27 points, 16 faces, 0 bits differ; each error at its path | D1 |
| K2 (set-up) | c1 | the case-built participant against the hand-built CWs one before any step: the mesh points of the initial mesh, `u`, `p`, `phi`; the `Debug` texts of the two `PimpleSettings` after setting the case-built `momentum_solver.tol_abs` to `1e-16` | 0 bits differ; equal | read 5, read 6: the same smoothing with the same control on the same bits; the settings differ only in the field no key sets |
| K3 (set-up) | c3 | 100 steps of the in-process run of CWc against 100 steps of the hand-built CWs (ADR 0345's reference): `max abs(dq)`; steps whose iteration count differs (printed) | `<= 1e-6 X` | the momentum solves stop at `1e-14` instead of `1e-16` absolute; the velocity changes by about that residual over a diagonal `V/dt ~ 1e-2`, `1e-12` m/s against a wall speed `omega X ~ 1e-2` m/s, `1e-10` relative, so about `1e-10 X`; the limit is four decades above |
| H1 | c2 | `rcfd serve --cosim cwc.json`, through `service::Client`: `cosim.list`; `cosim.open fluid`; `cosim.describe` against the in-process `fluid_from_case` | `protocol` `rcfd-cosim/1`, one participant `{participant: fluid, kind: fluid, name: fluid}`; `protocol` `rcfd-cosim/1`, lease 1, time 0; 0 bits differ in points, faces equal | D2 |
| H2 | c2 | `rcfd serve --cosim` with `cosim.patch = "nope"`: the exit status and standard error within 120 s; `rcfd --help` | non-zero, names `cosim.patch`; contains `--cosim` | D1, D2 |
| D100 | c3, c4 | the served run, 100 steps, against the in-process run of CWc: `max abs(q_served - q_in_process)` over steps and nodes; steps whose iteration count differs; every step converged | `<= 1e-12 X`; 0; yes | 0 exactly: the same build, the same case bytes, the numbers cross bit for bit (ADR 0345 D, D2 measured 0) |
| A1s (heavy record) | c3 | the served run, 650 steps: every step converged; iterations per step, largest and mean; the largest last test quantity; the pid of `rcfd serve` and of the structure differ from the test's and from each other | all 650; `<= 16` and `<= 12`; `<= 1e-8`; yes | ADR 0256 A1 (CWs: 10, 8.766, `9.995e-9`); K3 says CWc is CWs to `1e-10 X` |
| A2s (heavy record) | c3 | the same run: `omega_m/omega_0` of `Q_0`, `omega_m/omega_1` of `Q_1`; upward crossings | `abs(. - 1) <= 0.005`; `<= 0.03`; `>= 6` and `>= 11` | ADR 0256 A2 (0.998618, 1.011453; 6 and 12) |
| Ds (heavy record) | c4 | the same run against the in-process run of CWc, 650 steps | `<= 1e-12 X`; 0 steps with another count | 0 exactly |
| E1 | c5 | a `SprungWall` of CWs wrapped with the five segments `[i, i+1]` as elements, hosted in a service of the test process, `RemoteStructure` over TCP: `elements()`; `InterfaceMap::build` with `MapSettings::projection(1e-9)` from the proxies (the CWc fluid hosted the same way) against the build from the in-process objects: `displacement_to_points` of `q_0` and `forces_to_nodes` of the fluid's initial `forces()`, bits | equal; 0 bits differ | D3; the same builder on the same bits |
| E2 | c5 | the plain `SprungWall` hosted: the `cosim.describe` reply (raw, through `service::Client`) has no member `elements`; `RemoteStructure::elements()` is empty | yes; yes | D3, the reply of ADR 0345 unchanged |
| C1 | c6 | unit test: `write_request` into a writer that counts its `write` calls, for a request with a 200-vector parameter | 1 call; the bytes are the request's JSON and `\n` | D4 |
| C2 | c6 | 2000 calls of `cosim.list` through `service::Client` to a service of the test process on the loopback interface: mean time per call | `<= 1 ms` | ADR 0345 T1: 0.05 ms after one write per line; 2 ms before |
| B1 | c7, c8 | unedited, release: `rcfd-api` `case_hashes_w48u`, `w48v_case_hashes`, `baseline_hashes`, `mesh_transform_w48u`, `physics_schema`, `rules_w19c`, `w48v_adaptive_correctors`, `registers`, `dynamic_mesh`, `service_tcp`, `data_service_f447`, `cosim_service_f360` (its default tests), the library tests of `service`; `rcfd-fv` `cosim_f139` and the suite of F-362 (`cosim_f362*`); `rcfd-cli` `cli`; `rcfd-mcp` `default_f1`, `project_list_f270`, `tool_list_f275` (the goldens) | all pass | D1 (an absent optional table), D3 (a provided method), D4 (the same bytes) |
| G | c8 | the differences of the schema text (`rcfd schema`) and of `rcfd --help` against main, listed in the addendum | only the table `cosim` with its seven keys; only `[--cosim <case>]` and its line | D1, D2 |

### Negative controls

| Id | Check | Limit | Estimate |
| --- | --- | --- | --- |
| NC1 (D100 can fail) | the served run with the structure role `structure-stiff` (springs times `1 + 1e-9`), 100 steps, against the in-process run of CWc | `>= 1e-10 X` | ADR 0345 NC1 measured `4.15e-9 X` for the same perturbation |
| NC2 (K2 can fail) | K2's mesh comparison with `smoothing_tolerance = 1e-12` instead of `1e-13` | `> 0` bits differ | another stopping point of the displacement solve (`1e-12` against `1e-13` relative) |

### Mutations (scratch copy, not committed)

Each applied alone to a `git archive` copy of the final tree by an exact replacement that occurs once; the named tests are
run once and must fail; the file is restored and compared byte for byte.

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU1 | `cosim.describe` of a structure leaves out `elements` | E1 |
| MU2 | `RemoteStructure::elements` returns an empty list | E1 |
| MU3 | `Client::call` writes with `writeln!` again | C1 |
| MU4 | `fluid_from_case` ignores `initial_displacement` | K2 |
| MU5 | the smoothing classes treat `fixed_patches` as sliding | K2 |
| MU6 | `into_cosim_flow` does not refuse a turbulence model | K1 |
| MU7 | `rcfd serve` ignores `--cosim` | H1 |
| MU8 | the `cosim` table is not optional | `case_hashes_w48u` |

`scripts/limit_audit.py` is run on the two new test files and every hit is read.

### After the code

`cargo test --release -p rcfd-api --test cosim_case_f444`, `cargo test --release -p rcfd-cli --test cosim_serve_f444`
(the heavy records once with `--ignored`), the suites of B1; `cargo fmt -p <crate> --check` and `cargo clippy -p rcfd-fv -p
rcfd-api -p rcfd-cli --all-targets -- -D warnings`. The whole-workspace suite is not run.

## Predictions (mine, before any run)

D100 and Ds exactly 0; K2 0 bits; K3 a few `1e-11 X` or less, possibly 0 if the momentum solves never stop on the absolute
floor; A1s and A2s the numbers of CWs to the digits printed. The check I trust least is K2's settings comparison (a field of
`PimpleSettings` the case sets differently from `piso()` that I missed in read 5); second, H2 on Windows (the exit status of
a process that fails before listening).

## Decision rule

- **done**: K1 to K3, H1, H2, D100, A1s, A2s, Ds, E1, E2, C1, C2, B1 met, NC1 and NC2 hold, every mutation fails its tests,
  and G shows only the additions of D1 and D2, **and** those additions are approved by the maintainer. The acceptance stays
  the maintainer's.
- **partial**: the participant is built from the case and served by `rcfd serve`, D100 is met, and one of the other checks
  is missed (kept as `*_known_miss`, diagnosed) or a mutation is not caught; or every check is met and the approval of the
  additions of G is still outstanding.
- **todo**: the participant is not built from the case, or `rcfd serve` does not host it, or the served run does not
  complete.

A set-up check (K2, K3) or a negative control that does not hold is reported as a defective set-up, corrected by an
amendment before any re-run, never counted as a pass.

## Open choices taken

Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation); none changes a default result, a
golden or a hash table.

- **T1. An optional top-level table `[cosim]`** (D1). Options: a value `motion = "cosim"` of `[dynamic_mesh]` (an enum of
  the schema changes and the dynamic-mesh driver would have to refuse it everywhere); a key inside an existing table (a
  hash-table edit, ADR 0266 read 6).
- **T2. The initial wall as a list in the case** (D1). Options: an expression of position (cannot express `H q_0`
  exactly); a protocol call that sets the initial wall (a change of `rcfd-cosim/1`).
- **T3. A flag `rcfd serve --cosim`, not a service method** (D2): `METHODS` is pinned by existing tests.
- **T4. The participant's own name stays `fluid`** (`FlowWall` sets it; `rcfd-fv` would need a setter); the hosted name is
  `cosim.participant`.
- **T5. `elements` only when not empty** (D3), so ADR 0345's replies are unchanged.
- **T6. Refuse what the participant would not advance** (D1) rather than advancing a subset of the simulation's models.

## Limits of this registration

One fluid participant per `rcfd serve` (one `--cosim`). No structure from a case (the structure stays the embedding
program's or the mock's). The participant advances the incompressible PISO/PIMPLE flow alone: no turbulence, energy,
scalars or other models, no moving-mesh options beyond the smoothing of D1, no restart. The initial wall is a list of
numbers in the case. The force is `FlowWall`'s (prescribed wall-pressure gradient), with no key. No authentication (as the
service). The model problem and the mock of ADR 0256 and ADR 0345, with their limits.

## Amendment 1 (2026-11-13, while writing the tests, before any run of a check)

Two set-up errors of the registered table, found by reading while writing the tests; no check had been run.

- **E1's map settings.** The CWs nodes lie on the line `x = 0, z = 0.05`, and the 27 wall points of `xmin` at `z = 0,
  0.05, 0.1`, so a third of the points lie `0.05` m from the structure. `MapSettings::projection(1e-9)` would refuse them
  (ADR 0263's `Unmapped::Refuse`) and E1 could not run. Corrected: `MapSettings::projection(0.06)` (the largest distance
  0.05 plus a fifth). The limits of E1 are unchanged.
- **K1's turbulence case.** `turbulence.model = "k_omega_sst"` may be refused by the simulation builder for reasons of its
  own (inlet and wall conditions) before the participant's refusal is reached, so the check would not see the refusal
  "by name". Corrected: `turbulence.model = "smagorinsky"` with `solver.algorithm = "piso"` (an algebraic transient model
  that the builder accepts on CWc); the error must name "a turbulence model" at the path `cosim`.

MU6 (the refusal of a turbulence model removed) keeps K1 as its test.

## Addendum 1: results (2026-11-13)

### What was built

- `crates/rcfd-api/src/cosim_case.rs` (new): the schema of the optional table `[cosim]` (the seven keys of D1), `present`,
  `check`, `fluid_from_case` (reference mesh, smoothing, the mesh at the initial displacement,
  `Simulation::from_geometry`, `FlowWall`), `host` (`Service::host_fluid` under `cosim.participant`).
- `crates/rcfd-api/src/sim.rs`: lines 1271-1277 (`Simulation::new` refuses a case with `[cosim]`, path `cosim`) and
  5372-5423 (a new `impl Simulation` block with the crate-private `into_cosim_flow`). Nothing else in the file changed.
- `crates/rcfd-api/src/schema.rs` lines 2357-2358 (the field `cosim`); `lib.rs` one line.
- `crates/rcfd-fv/src/cosim.rs` lines 61-65: the provided method `Structure::elements` (empty by default).
- `crates/rcfd-api/src/cosim_service.rs`: `cosim.describe` adds `elements` when the structure gives some;
  `RemoteStructure` keeps and returns them; one parser `index_lists` for `faces` and `elements`.
- `crates/rcfd-api/src/service.rs`: `write_request` (one `write_all` per request), used by `Client::call`; unit test C1.
- `crates/rcfd-cli/src/main.rs`: `rcfd serve --cosim <case>` and its help line.
- Tests: `crates/rcfd-api/tests/cosim_case_f444.rs` (7 tests), `crates/rcfd-cli/tests/cosim_serve_f444.rs` (4 by default,
  3 ignored heavy records, 1 ignored helper). Documents: `docs/cosimulation-protocol.md`, `docs/service-protocol.md`,
  `docs/case-schema.md` (prose, no TOML block).
- Not edited: any existing test, hash table, golden; `Cargo.toml`, `Cargo.lock`; no unsafe code.

### Checks

Run 1 on commit a9a2af18, release, each once: `cargo test --release -p rcfd-api --test cosim_case_f444` (7 passed, 8.7 s),
`--lib c1_f444`, `cargo test --release -p rcfd-cli --test cosim_serve_f444` (4 passed, 7.6 s), the heavy records once with
`--ignored a1s a2s ds` (3 passed, 41 s). The machine was lightly loaded: 650 steps took 21.5 s served, 19.6 s in process.

| Check | Limit | Measured | Met |
| --- | --- | --- | --- |
| K1 (c1) the key names the wall; located errors | 27 points, 16 faces, 0 bits; each error at its path | 27, 16, 0 bits, faces equal; `cosim.patch` (empty; "mesh has no patch 'nope'"), `cosim.fixed_patches` (unknown; "is the moving patch"), `cosim.initial_displacement` ("80 numbers for 27 wall points"), `solver.algorithm`, `dynamic_mesh.enabled`, `cosim` ("... also has a turbulence model"), `Simulation::new` at `cosim` | met |
| K2 (set-up) initial state and settings against hand-built CWs | 0 bits; settings equal after tol_abs | mesh 0 of 1701, fields 0 of 2456 bits; `Debug` texts equal (case-built tol_abs `1e-14`, hand-built `1e-16`) | held |
| K3 (set-up) 100 steps CWc against CWs | `<= 1e-6 X` | exactly 0 (0 values differ in bits, 0 counts): the momentum floor never bound | held |
| H1 (c2) `rcfd serve --cosim` | protocol, one participant, lease 1, time 0, 0 bits | `{"participants":[{"kind":"fluid","name":"fluid","participant":"fluid"}],"protocol":"rcfd-cosim/1"}`; lease 1, time 0; 27 points, 0 bits differ, faces equal | met |
| H2 (c2) bad case; help | non-zero, names `cosim.patch`; `--cosim` in help | exit 1 after 0.1 s, "cosim.patch: mesh has no patch 'nope' ..."; help names it | met |
| D100 (c3, c4) served against in-process, 100 steps | `<= 1e-12 X`; 0; all converged | exactly 0; 0; yes (max 10, mean 9.000 iterations) | met |
| A1s (c3, heavy record) | all 650; `<= 16`, `<= 12`; `<= 1e-8`; three processes | all 650; 10 and 8.766; `9.995e-9`; pids 24744 (rcfd serve), 106116 (structure), 71268 (test) | met |
| A2s (c3, heavy record) | `<= 0.005`, `<= 0.03`; `>= 6`, `>= 11` | 0.998618 (6 crossings), 1.011453 (12 crossings) | met |
| Ds (c4, heavy record) 650 steps | `<= 1e-12 X`; 0 | exactly 0; 0 | met |
| E1 (c5) elements and the coupler-side map | equal; 0 bits | elements `[[0,1],...,[4,5]]`; `displacement_to_points` 0 of 81, `forces_to_nodes` 0 of 18 bits differ; maps equal; largest distance 0.05 | met |
| E2 (c5) no elements, reply as before | no member; empty | members `kind, name, nodes`; empty | met |
| C1 (c6) one write | 1 call; the bytes | 1 call, 4809 bytes equal to the JSON line | met |
| C2 (c6) Client time per call | `<= 1 ms` | 0.047 ms | met |
| B1 (c7, c8) existing suites, unedited | all pass | `case_hashes_w48u` 1, `w48v_case_hashes` 0 (its test is ignored in the tree), `baseline_hashes` 3, `mesh_transform_w48u` 13, `physics_schema` 11, `rules_w19c` 8, `w48v_adaptive_correctors` 5, `registers` 7, `dynamic_mesh` 5, `service_tcp` 2, `data_service_f447` 6, `cosim_service_f360` 13 (V re-run after the document edits: passed), lib `service::` 15; `rcfd-fv` `cosim_f139` 18, `cosim_f361` 9, `cosim_f362` 11; `rcfd-cli` `cli` 8; `rcfd-mcp` `default_f1` 2, `project_list_f270` 1, `tool_list_f275` 7; 0 failed | met |
| G (c8) the additions | only the table and the help line | below | met; approval outstanding |
| NC1 stiffer structure seen | `>= 1e-10 X` | `4.15e-9 X` | held |
| NC2 another smoothing tolerance seen | `> 0` bits | 157 mesh bits differ | held |

Against the predictions: D100, Ds and K2 as predicted; K3 exactly 0. No limit missed.

**G, the additions for approval.** (1) The JSON schema (`rcfd schema`, `settings.schema`) gains the property `cosim` of
the top-level object: `type object`, `x-rcfd-optional: true`, not in the top-level `required`, with the seven properties
`patch` (string, ""), `participant` (string, "fluid"), `fixed_patches` (array of string), `smoothing_method` (enum
laplacian, inverse_distance, inverse_volume), `smoothing_exponent` (number 1, 0 to 10), `smoothing_tolerance` (number
`1e-12`, 0 to 1), `initial_displacement` (array of number). Nothing else in the schema changes (the only edit of
`schema.rs` is the one field). No file of the repository pins the whole schema, so no golden file had to change. (2) `rcfd
--help`: `serve [--addr <host:port>] [--root <dir>] [--stdio]` becomes `serve [--addr <host:port>] [--root <dir>]
[--stdio] [--cosim <case>]`, and the description gains "; --cosim hosts the co-simulation fluid participant of the case's
[cosim] table (protocol rcfd-cosim/1)" (one more line). No file pins the help text. (3) MCP tools list: no change (the three
golden suites pass unedited).

### Mutations

On a `git archive` copy of a9a2af18 with its own target directory; each applied alone by a replacement that occurred once,
run once, restored and compared byte for byte (identical in all eight).

| Id | Must fail | Result |
| --- | --- | --- |
| MU1 `elements` left out of `describe` | E1 | E1 failed |
| MU2 `RemoteStructure::elements` empty | E1 | E1 failed |
| MU3 `writeln!` again | C1 | C1 failed |
| MU4 `initial_displacement` ignored | K2 | K2, K1, K3 failed |
| MU5 `fixed_patches` treated as sliding | K2 | **not caught**: all 7 tests passed |
| MU6 a turbulence model not refused | K1 | K1 failed |
| MU7 `--cosim` ignored | H1 | H1 failed |
| MU8 `[cosim]` not optional | `case_hashes_w48u` | its test H1 failed |

**Diagnosis of MU5.** In CWc the prescribed displacement is along `x` only, and the Laplacian smoothing solves each
component on its own. A sliding (`Slip`) `xmax` keeps the normal component, `x`, at zero, as a `Fixed` one does, and the
tangential components are zero everywhere because nothing drives them; so the two classes give the same mesh bit for bit,
and no check on CWc can tell them apart. The design is not shown wrong; the set-up cannot see the difference. A check that
can: a wall displacement with a component along `xmax` (a `y` part), where a sliding `xmax` moves its points and a fixed
one does not. Not added after the run (rule 2); question Q3.

`scripts/limit_audit.py` on the two new test files: 0 hits.

### After the tests

`cargo fmt -p <crate> --check` for `rcfd-fv`, `rcfd-api`, `rcfd-cli`: clean. `cargo clippy --release -p rcfd-fv -p
rcfd-api -p rcfd-cli --all-targets -- -D warnings`: clean.

### Deviations and not run

- The heavy records took 41 s on a lightly loaded machine; they stay ignored, as registered.
- The library code was committed before the run (a9a2af18) and not changed afterwards; the documents were edited after the
  run (F-360's V re-run passed on them).
- Not run: the whole-workspace suite and the gate (the lead's); the ignored records of `cosim_service_f360`; `--stdio`
  with `--cosim`; the inverse-distance and inverse-volume smoothing; a `pimple` case.

### Decision by the registered rule

Every acceptance check, set-up check and control is met, but MU5 was not caught and the additions of G still need the
maintainer's approval: **partial** by the rule. Delivered: the participant built from the case and served by `rcfd serve
--cosim`, reproducing A1 and A2 with the in-process displacements bit for bit over 650 steps; elements in `describe`; one
write per `Client` request; no case hash, hash table or golden file moved.

### Open questions (each with options and a recommendation)

- **Q1. Approve the additions of G** (the `cosim` table in the schema, the `--cosim` help line). Options: (a) approve as
  built; (b) rename keys or the flag first. Recommendation (a).
- **Q2. The participant's own name** is always `fluid` (`FlowWall` sets it). Options: (a) a setter in `rcfd-fv` so that
  `cosim.participant` also names it in coupling errors; (b) leave. Recommendation (a), in a follow-up.
- **Q3. MU5** is not caught on CWc. Options: (a) a follow-up check with a wall motion along `xmax`; (b) accept.
  Recommendation (a).
- **Q4. A structure from a case** hosted by `rcfd serve`. Options: (a) a later item; (b) leave to embedding programs.
  Recommendation (a), once a real structural participant exists.

### For the lead: roadmap entries

Proposed status of F-444: **partial** (`[~]`).

Note text for F-444:

> PARTIAL (ADR 0380, 2026-11-12; registered before the code, one amendment before any run). An optional case table [cosim] (patch, participant, fixed_patches, smoothing_method, smoothing_exponent, smoothing_tolerance, initial_displacement) names a moving wall patch of an incompressible piso/pimple case as the co-simulation fluid participant (rcfd_api::cosim_case::fluid_from_case: the case's mesh is the reference, the flow starts at rest on it moved by initial_displacement, every advance smooths the interior and makes one flow step; any other model the simulation would advance is refused by name; rcfd run refuses the case), and rcfd serve --cosim <case> hosts it under rcfd-cosim/1 (cosim_case::host). On the model problem of ADR 0256 written as a case, with the mock structure of ADR 0345 in a third process and the unchanged coupled_step driving both over TCP: all 650 steps converged (7 to 10 iterations, mean 8.766, last test 9.995e-9), omega_m/omega 0.998618 and 1.011453, and the displacements of every node at every step equal to the in-process run of the same case bit for bit (limit 1e-12 X; the case also equals the hand-built CWs bit for bit over 100 steps); a structure stiffer by 1e-9 is seen (4.15e-9 X). Structure::elements (a provided method, empty by default) is carried by cosim.describe only when not empty and returned by RemoteStructure::elements, so InterfaceMap::build runs on the coupler's side with the bits of the in-process build. service::Client writes each request with one call (unit-tested; 0.047 ms per call). Without the table every case hash, written case text and baseline hash is unchanged and the MCP goldens pass unedited; the schema gains the optional property cosim and the help of serve gains [--cosim <case>], both awaiting the maintainer's approval (ADR 0380 Q1). Seven of eight mutations caught; treating fixed_patches as sliding is not seen by this model problem (its wall moves along x only, so a sliding and a fixed xmax give the same mesh; Q3). Not delivered: a structure from a case, the participant's own name (always fluid, Q2), more than one participant per rcfd serve, models other than the incompressible flow. Open: ADR 0380 Q1 to Q4.

Note text for F-360 (to append):

> Follow-up F-444 (ADR 0380, 2026-11-12): participants from the case hosted by rcfd serve --cosim, elements in describe, one write per service::Client request (ADR 0345 Q3, Q4, Q5).

Follow-up item (number for the lead to assign). Title: A check that sees fixed against sliding patches of the co-simulation smoothing. Deps: F-444. Acceptance:

> A [cosim] case whose initial wall displacement has a component along a fixed patch gives a different mesh when that patch is declared sliding than when it is declared fixed (the mutation MU5 of ADR 0380 is caught), and the points of the fixed patch do not move (0 bits); without the table every case hash is unchanged.
