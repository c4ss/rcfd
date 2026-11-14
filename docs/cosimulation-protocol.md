# Co-simulation with a structural solver: the participant protocol (F-139; ADR 0256)

rcfd can exchange the displacement of a wall and the fluid force on it with a structural solver, step by step, and iterate
the two to agreement within each time step. Status: a library interface (`rcfd_fv::cosim`) whose calls can also be sent
over the solver service (`rcfd_api::cosim_service`, below), so a participant can run in another process. There is no case
key and no command-line or MCP tool yet, and the only structural participant in the repository is a mock (a wall of sprung
nodes). The decision records and the measured checks are in
`docs/adr/0256-f139-registration-cosimulation-participant.md` (the protocol) and
`docs/adr/0345-f360-registration-cosimulation-over-the-service.md` (the transport).

**Protocol version: `rcfd-cosim/1`** (the calls of this page as carried by the solver service; see
[Versions](#versions)).

This page specifies the protocol: who sends what, in which units and on which mesh, how a coupling step runs, when it is
converged, and what happens when a participant fails. It is for people who want to connect a structural solver or write a
participant in another program. For the moving mesh the fluid side runs on see [dynamic-mesh.md](dynamic-mesh.md); for the
one-way export of loads to a structural deck see [interchange-formats.md](interchange-formats.md); for the JSON-RPC service
that carries the calls see [service-protocol.md](service-protocol.md).

## Participants and coupler

Two participants and a coupler:

* the **fluid** participant owns the flow and its moving mesh. Its interface mesh is the wall patch: wall points and wall
  faces. In the library this is `cosim::FlowWall` (a `PimpleSolver` on a moving mesh);
* the **structure** participant owns the structure. Its interface mesh is a set of nodes. The mock is `cosim::SprungWall`;
  a finite-element solid is `rcfd_post::solid_coupling::StaticSolid` (ADR 0286): the nodes of one surface set of a
  `rcfd_fem` model, `advance` a static solve under the nodal forces, `checkpoint` and `restore` of its time and
  displacement bit for bit. It has no mass, so against an incompressible flow the added-mass ratio is unbounded: it is for
  mock fluids and quasi-static use, not for a flow solver. `SolidInterface::build_map` builds the map from the set's faces,
  and `solve_face_forces` is the same route without a coupled step (face forces of a fluid solved once, in process).
  A solid with mass is `rcfd_post::solid_dynamic::DynamicSolid` (ADR 0410): the same interface, `advance` the transient
  integrator of `rcfd_fem::transient` (Newmark or generalised-alpha, on either path) over one or more sub-steps with the
  nodal forces held (`Integrator::run_with_force`), `checkpoint` and `restore` of the integrator's state and the held
  forces bit for bit, and `elements` for the map builders. It is checked against mock fluids (an added mass on a piston
  wall, a spring) and against the flow (F-401, ADR 0434, under "Limits and gaps"); with Newmark's method a converged coupled step is the implicit step of the coupled system. A
  fluid whose added mass acts per face excites interface patterns that are not uniform, with slopes of the fixed-point
  map of several hundred on that bar; components of the force below the convergence test are then loosely coupled and
  grow from step to step (ADR 0410 amendment 1). On a non-matching interface the transpose of the map loads the solid
  face with weights `w` that differ from the consistent pattern by a self-equilibrated part; the coupled frequency then
  falls as if the added mass hung on a spring of compliance `w^T K^{-1} w - L/(E A)` in series with the tip (the extra
  static compliance; the cross term with the uniform field vanishes exactly). This predicts the measured change within
  `2.4e-3` of itself for faces of 2 x 2 to 8 x 8 elements. At mass ratio 4 the change is `6.3e-3`, `1.8e-3`, `5.4e-4`
  with the projection and `1.9e-3`, `2.5e-4`, `2.8e-5` with the radial basis of support `3 h` (ADR 0412). The
  projection's weights are not symmetric under the square's axis mirrors: each quadrilateral is split into the triangles
  `(0, 1, 2)` and `(0, 2, 3)` of its corner order, and on a Hex8 face all the diagonals are parallel;
* the **coupler** (`cosim::coupled_step`) owns the mapping between the two interface meshes and the iteration. Neither
  participant sees the other's mesh.

Every call carries plain numbers and text. No call hands a reference into one participant to the other, so each call can
be sent as one request and one reply.

## Calls

| Call | To | Carries | Returns |
| --- | --- | --- | --- |
| describe | both, once | | structure: node coordinates. Fluid: wall point coordinates and the points of every wall face. Both in the reference configuration |
| time | both | | the participant's time |
| checkpoint | both | | nothing; the participant keeps its full state at the current time |
| restore | both | | nothing; the participant is at its checkpoint again, bit for bit |
| forces | fluid | | the force of the fluid on every wall face at the participant's time |
| advance | structure | the step `dt`; one force vector per node, held over the step | the displacement of every node at the new time |
| advance | fluid | the step `dt`; one displacement vector per wall point at the new time | the force of the fluid on every wall face at the new time |

In the library these are the traits `Participant` (`name`, `time`, `checkpoint`, `restore`), `Structure`
(`node_coordinates`, `advance`) and `Fluid` (`point_coordinates`, `face_points`, `forces`, `advance`).

Conventions:

* SI units: metres, seconds, newtons. Vectors are in the global axes of the case.
* A **force** is the force of the fluid on the structure: a total per wall face or per node, not a pressure.
* A **displacement** is measured from the reference configuration that `describe` returned, not from the previous step.
* A checkpoint stays valid until the next `checkpoint`. `restore` may be called any number of times.
* After a failed `advance` a participant may be in any state; the coupler restores it.

## Mapping between the two meshes

The meshes need not match. The coupler holds weights `H` from the nodes to the wall points (`cosim::InterfaceMap`):

* **displacement, structure to fluid:** wall point `p` gets `d_p = sum_i H_pi q_i`. Every row of `H` sums to 1, so a rigid
  translation arrives unchanged (a consistent map). A point whose weights do not sum to 1 is refused when the map is
  built: there is no extrapolation and no default for an unmapped point;
* **force, fluid to structure:** each face force goes in equal shares to the face's points, and from each point to the
  nodes with the point's weights. This is the transpose of the displacement map taken to the faces (a face's displacement
  being the mean of its points), so the resultant force and the work are the same on both meshes: `sum_i f_i . q_i = sum_f
  F_f . d_f` for any displacement, to rounding (a conservative map).

`cosim::piecewise_linear_rows` builds `H` for nodes on a line with a linear displacement between them; with one node the
wall is a rigid segment. Any other weights can be passed to `InterfaceMap::new`.

### Weights built from the two interface meshes (F-362; ADR 0263)

Opt-in, `cosim::InterfaceMap::build(nodes, elements, points, face_points, &settings)` (or `cosim::interface_rows` for the
rows alone) builds `H` from the structure's interface mesh (its nodes and **elements**: 1 to 4 node indices each, a point,
a segment, a triangle or a quadrilateral, which is split into two triangles) and the fluid's (wall points and faces). For a
transport this means that `describe` of the structure also returns its elements: `Structure::elements` (a provided method,
empty by default; ADR 0380) is carried by `cosim.describe` and returned by `RemoteStructure::elements`, so the coupler
builds the map from the proxies. Three methods, `cosim::MapMethod`:

* **nearest-element projection** (`MapSettings::projection(max_distance)`): a wall point gets the barycentric weights of
  its nearest point on the nearest structural element. On a curved wall the two meshes are different facetted surfaces and
  a wall point lies off the structural facets (by up to the sagitta `h^2/(8 r)`); the default offset correction then adds
  the smallest correction on the ring of elements around the nearest one that makes the row reproduce the point's own
  coordinates. Some weights become slightly negative. `Projection { correct_offset: false }` keeps the plain weights
  (non-negative, one element), which reproduce translation but not rotation on a curved wall;
* **radial-basis interpolation** (`MapSettings::radial_basis(support, max_distance)`): Wendland's compactly supported
  function `(1 - r/rho)^4 (4 r/rho + 1)` with a polynomial of degree 1 in the directions the nodes span (on a planar
  structure the two in-plane ones, on a line one), solved densely by Cholesky factorisations (no linear-algebra crate). A
  support of three times the longest element edge is the documented choice; the report gives an estimate of the condition
  number of the kernel matrix (about 10 to 100 at that support). Rows are dense (one weight per node): meant for interface
  meshes of up to a few thousand nodes.
* **bilinear projection** (`MapSettings::bilinear_projection(max_distance)`, `MapMethod::BilinearProjection`; F-504,
  ADR 0414): the projection with the same nearest element, offset correction and unmapped policy, but on a quadrilateral
  the nearest point of its bilinear patch (Newton's method from the centre, or the nearest edge point) and the bilinear
  shape functions there instead of the split into two triangles. Its weights keep all eight symmetries of a square face
  (to `1e-15`), equal the bilinear evaluation of ADR 0412 to `1e-15` on flat faces, and give 0.34 to 0.53 of the
  projection's extra compliance on the 3:2 walls of F-502 (frequency change `2.2e-3`, `8.2e-4`, `2.8e-4` for faces of
  2 x 2, 4 x 4, 8 x 8 elements, within `6.6e-4` of the static-compliance prediction). The default projection is unchanged.

Why the polynomial and the correction: a rigid motion displaces a point by an affine function of its position, and a map
with one weight per (point, node) reproduces every affine function, so every rigid translation and rotation, exactly when
each row sums to 1 **and** reproduces the point's coordinates (`sum_i H_pi x_i = x_p`). The same two properties make the
transposed force map conserve the resultant and the moment (about any point, with each face force acting at the mean of
its points); the work is conserved by the transpose for any weights. Measured (ADR 0263) on a planar and a cylindrical
wall with non-matching meshes: rigid motion and the balances to `1e-15` or better with both methods; without the offset
correction the cylinder's rotation is off by `5e-3` and the moment by `1e-4`.

**Unmapped points.** A wall point farther than `max_distance` (metres, no default: above the largest gap between the two
discretisations of one surface, below the distance to another wall) from the structure is outside it. With
`Unmapped::Refuse` (the default) the build fails with a message naming the point, its coordinates and its distance; with
`Unmapped::Nearest` the point moves with its nearest structural point (translation and resultant still exact, the moment
changes by the force on such points times their offset). `cosim::MapReport` lists every point's distance, the outside
points, the largest coordinate defect (non-zero only where the structure cannot represent a point's offset, for example a
line of nodes and wall points beside it) and the condition estimate.

On the model problem of ADR 0256 the projection gives the piecewise-linear rows (to `1e-16`) and the same frequencies;
the radial basis shifts them by `-5.2e-4` (mode 0) and `+1.9e-4` (mode 1): its displacement between the nodes is smoother.

## One coupling step

Both participants cover the same window `[t, t + dt]`; `dt` is the coupling time step and the coupler's argument. A
participant may take smaller steps of its own inside the window.

1. The coupler checks its settings and that both participants are at the same time, then sends `checkpoint` to both.
2. Predictor: `forces` from the fluid, mapped to the nodes: `f`.
3. Iteration `k`: (from the second on, `restore` both) the structure advances with `f` and returns the node
   displacements; they are mapped to the wall points; the fluid advances with them and returns the face forces; they are
   mapped to the nodes: `ft`. The residual is `r = ft - f`.
4. Converged when `|r| <= tolerance x max(|ft|, force_scale)`, with Euclidean norms over all components of all nodes.
   `force_scale` is a floor in newtons so that a force passing through zero does not demand an impossible relative
   accuracy. The step then stands as it is: the structure moved under `f`, the flow followed that motion, and the flow's
   force is `f` within the tolerance.
5. Otherwise `f` becomes `f + w r` and the iteration repeats, up to `max_iterations`. `w` is a fixed factor or Aitken's
   dynamic factor, `w_k = -w_{k-1} r_{k-1} . (r_k - r_{k-1})/|r_k - r_{k-1}|^2`, restarted every step from its `initial`
   value.
6. At the cap without convergence: with `AtCap::Stop` (the default) both participants are restored and the step is an
   error; with `AtCap::Keep` the last iterate is kept and reported with `converged = false`.

To repeat a finished step (with another `dt`, for instance), send `restore` to both and step again.

With `max_iterations = 1` the step is the loose (explicit) coupling: one pass, the structure advanced with the flow's
force of the previous time.

## Failure

A participant fails when its call returns an error, an array of the wrong length or a non-finite value, or when it
reports a time other than `t + dt` after `advance` (within `1e-9 dt`). The coupler then sends `restore` to both and returns
`cosim::CouplingError` with the participant's name (or `coupler`), the time at the start of the step, the iteration and
the participant's message, for example:

```text
co-simulation: 'structure' at t = 0, iteration 2: injected failure of the structural solver
co-simulation: 'coupler' at t = 0, iteration 5: the coupled iteration did not converge in 5 iterations (...)
```

After an error both participants are at the start of the step, bit for bit, and the caller may retry or stop. Invalid
settings, a non-positive `dt`, a map that does not fit the participants and participants at different times are refused
before anything is touched.

## Over the solver service (`rcfd-cosim/1`)

The calls above can be sent to a participant in another process through the JSON-RPC service
([service-protocol.md](service-protocol.md), version 1.1; ADR 0345). Opt-in: nothing is hosted unless a program asks.

**Hosting.** The program that owns a participant hands it to a `Service` and lets the service listen:

```rust
use rcfd_api::service::{Service, ServiceConfig};

let service = Service::new(ServiceConfig::default());
service.host_structure("structure", move || Ok(Box::new(sprung_wall)))?;   // or host_fluid("fluid", ...)
let server = service.serve_tcp("127.0.0.1:0")?;                            // server.addr is the address
```

The closure builds the participant on a thread of its own, where every call for it is applied, one at a time in the order
the requests arrive (so a participant need not be `Send`; `FlowWall` is not).

**From a case** (F-444, ADR 0380). A case with the optional table `[cosim]` names a moving wall patch as the fluid
participant, and `rcfd serve --cosim <case>` hosts it (under the name `cosim.participant`, default `fluid`) before it
listens:

    rcfd serve --addr 127.0.0.1:7878 --cosim wall.toml

The case is an incompressible `piso` or `pimple` case with nothing else to advance (no turbulence, energy, scalars,
dynamic mesh and so on: each is refused by name). Its mesh is the reference configuration; the flow starts at rest on that
mesh moved by `cosim.initial_displacement` (three numbers per wall point, in the order of `describe`, which only the
coupler can compute when the initial wall comes from the structure, as `H q_0`), and every `advance` moves the wall,
smooths the interior (`cosim.patch` prescribed, `cosim.fixed_patches` fixed, every other patch sliding;
`smoothing_method`, `smoothing_exponent`, `smoothing_tolerance` as `dynamic_mesh.smoothing`) and makes one flow step. The
keys are listed in [case-schema.md](case-schema.md). Without the table nothing changes; `rcfd run` refuses a case with it.
In a program the same is `rcfd_api::cosim_case::host(&service, &case)`, or `fluid_from_case(&case)` for the participant
in-process. On the model problem of ADR 0256 the participant served this way, driven by the mock structure in a third
process, gives the displacements of the in-process run bit for bit over 650 steps.

**Driving.** `rcfd_api::cosim_service::RemoteStructure` and `RemoteFluid` implement the traits `Structure` and `Fluid` by
sending each call as one request, so the coupler is the same `cosim::coupled_step`, with one or both participants remote:

```rust
use rcfd_api::cosim_service::{RemoteSettings, RemoteStructure};

let mut structure = RemoteStructure::connect(address, "structure", RemoteSettings::default())?;
let step = coupled_step(&mut fluid, &mut structure, &map, dt, &settings)?;
```

**Methods.** One JSON object per line (JSON-RPC 2.0), one reply per request. `participant` is the name the participant is
hosted under, `lease` the number that `cosim.open` returned.

| Method | Parameters | Result |
| --- | --- | --- |
| `cosim.list` | | `{protocol, participants: [{participant, kind, name}]}` |
| `cosim.open` | `participant` | `{protocol, lease, kind, name, time}` |
| `cosim.describe` | `participant`, `lease` | a structure: `{kind, name, nodes}` and `elements` when it gives some; a fluid: `{kind, name, points, faces}` |
| `cosim.time` | `participant`, `lease` | `{time}` |
| `cosim.checkpoint` | `participant`, `lease` | `{time}` |
| `cosim.restore` | `participant`, `lease` | `{time}` |
| `cosim.forces` | `participant`, `lease` (a fluid) | `{forces, time}` |
| `cosim.advance` | `participant`, `lease`, `dt` and, for a structure, `forces`; for a fluid, `displacements` | a structure: `{displacements, time}`; a fluid: `{forces, time}` |

`nodes`, `points`, `forces` and `displacements` are arrays of `[x, y, z]`, `elements` an array of arrays of 1 to 4 node
indices (from 0, into `nodes`), `faces` an array of arrays of point indices
(from 0, into `points`), `time` and `dt` numbers; units and conventions as above. `kind` is `fluid` or `structure`; `name`
is the participant's own name, the one a coupling error carries. `protocol` is `rcfd-cosim/1`. For example:

```text
{"jsonrpc":"2.0","id":7,"method":"cosim.advance","params":{"participant":"structure","lease":1,"dt":0.01,"forces":[[0.012,0,0],[0.024,0,0]]}}
{"jsonrpc":"2.0","id":7,"result":{"displacements":[[0.0029,0,0],[0.0026,0,0]],"time":0.01}}
```

**Numbers.** A real number is a JSON number. rcfd writes the shortest decimal text that reads back as the same binary64
value and reads with a correctly rounding parser, so every finite value crosses bit for bit, and a run with a participant
in another process gives the same bits as the in-process run (ADR 0345). A participant written with another JSON library
keeps this only if that library round-trips binary64. JSON has no text for a non-finite number: a participant that
returns one is answered with the error 1005.

**Errors** (codes of the service): `-32602` a missing or malformed parameter; `1006` no participant of that name; `1002` a
lease that is not the current one, or a call that does not belong to the participant's kind (`cosim.forces` to a
structure); `1005` the participant returned an error, `error.message` being its message unchanged. After a failed
`advance` the service does not restore the participant; the coupler does.

**The lease.** `cosim.open` returns a number that grows with every `open` of that participant, and every other call must
carry the current one. The participant's state and its checkpoint live in the service, not in the connection, so a new
connection finds both; the lease makes sure that a request of an old connection, still waiting in a buffer, is refused
rather than applied after the participant has been restored over the new one. It also means one coupler per participant:
a second `open` takes the participant over.

**A dropped connection.** A proxy whose call meets an input-output error, an end of stream, a time-out
(`RemoteSettings::call_timeout`, default 120 s) or a reply that does not answer the request marks its connection as lost
and returns an error naming the method and the address. From then on every call fails at once, except `restore`, which
connects again once (`connect_timeout`, default 5 s), takes a new lease and restores the participant. Since the coupler
answers a failed call inside the iteration with `restore` on both participants, a connection dropped in the middle of a
step ends the step with the usual located error and both participants at the start of the step, whether the lost request
was never received, was applied, or is still being computed (the `restore` waits behind it):

```text
co-simulation: 'structure' at t = 0, iteration 2: the connection to 127.0.0.1:52144 was lost during cosim.advance: the service closed the connection
```

The caller may then step again over the new connection. If the connection drops between two steps, the next step fails
before its iteration (iteration 0) with nothing advanced and nothing to restore; `reconnect()` on the proxy opens a new
connection. If the participant's process is gone, the reconnection fails and the error says so ("... and the restore
failed: could not connect to ..."); the participant that is still there is restored.

## Why relaxation is needed, and how to choose it

An incompressible fluid answers a wall acceleration at once with a pressure proportional to it: the fluid acts on the
structure as an added mass. If the structure is advanced with a force `f` and the flow then returns `ft`, a change of `f`
changes `ft` by minus half the added-mass to structural-mass ratio (for the pairing used here: a structure advanced with
a constant force, a flow step whose wall velocity is the displacement over the step). So:

* the plain iteration (`w = 1`) diverges when that ratio exceeds 2 for any mode of the wall, whatever the time step. The
  loose coupling grows from step to step for the same reason;
* a fixed factor converges when `w < 2/(1 + ratio/2)` for the largest ratio. `fsi::relaxation_from_added_mass(M, m_a) =
  2M/(2M + m_a)` removes the heaviest mode in one iteration and is a safe start;
* Aitken's factor started from that value adapts to the other modes.
* Opt-in, `fsi::Relaxation::LeastSquares { initial, filter }` (`CouplingSettings::least_squares(initial)`) replaces the
  factor by the least-squares quasi-Newton update of ADR 0261: after a first update relaxed with `initial`, the next force
  is the flow's force corrected by the combination of the step's earlier output differences whose residual differences
  best cancel the current residual (a difference column nearly dependent on newer ones, below `filter` of its norm, is
  dropped; nothing is carried from one step to the next; `fsi::strong_step` refuses it).

Measured on the model problem of ADR 0256 (a sprung wall with six nodes at the end of a channel, mass ratios 4 and 0.5
for its two lowest modes, tolerance `1e-8`): Aitken 7 to 10 iterations per step (mean 8.8); the fixed factor 1/3 takes 30;
the plain iteration multiplies the residual by 2.0 per iteration; the loose coupling passes ten times the initial
amplitude at step 27.

## The fluid participant's force

`FlowWall` returns for every wall face the pressure and viscous load `rho [p_f S_f - nu ((grad u)_f + (grad u)_f^T) .
S_f]` (`sixdof::patch_face_forces`). Its wall pressure is evaluated with the pressure gradient that the moving wall
prescribed to the flow step (`fsi::WallPressure::PrescribedGradient`, ADR 0252), which is second order in the cell size.
The default evaluation of the rigid-body force (`patch_force_moment`) is first order on such a wall (a quarter cell
short); it can be selected with the field `wall_pressure`. The gradient of the last flow step belongs to the
participant's state and is saved and restored with it.

The caller gives `FlowWall` the function that turns wall-point displacements into the positions of all mesh points (the
smoothing). It stays inside the participant.

## Example

```rust
use rcfd_fv::cosim::{coupled_step, piecewise_linear_rows, CouplingSettings, Fluid, FlowWall, InterfaceMap, SprungWall};
use rcfd_fv::fsi::relaxation_from_added_mass;

// flow: a PimpleSolver on the mesh of the initial wall; reference: the undeformed mesh; smooth: wall displacements -> points
let mut fluid = FlowWall::new(flow, "xmin", &reference, rho, nu, Box::new(smooth))?;
let mut structure = SprungWall::new(nodes, [1.0, 0.0, 0.0], masses, springs, initial_displacements)?;
let rows = piecewise_linear_rows(&node_y, &wall_point_y)?;
let map = InterfaceMap::new(rows, node_y.len(), fluid.face_points())?;
let mut settings = CouplingSettings::aitken(relaxation_from_added_mass(wall_mass, added_mass));
settings.force_scale = largest_spring_force;
for _ in 0..steps {
    let step = coupled_step(&mut fluid, &mut structure, &map, dt, &settings)?;
    // step.iterations, step.residuals, step.node_displacements, step.face_forces
}
```

The test file `crates/rcfd-fv/tests/cosim_f139.rs` holds the complete set-up.

## Limits and gaps

* Over the service: one coupler per participant; no authentication or encryption (use the loopback interface or a trusted
  network); only a fluid participant is built from a case (one per `rcfd serve`, the flow alone, the initial wall as a
  list of numbers); a structure is hosted by a program; a call that outlasts the time-out counts as a lost connection; a
  participant process that died is not restarted.
* One wall and two participants. No simultaneous update of several participants.
* The structural solver of the repository (`DynamicSolid`, ADR 0410) has been connected to the flow (F-401, ADR 0434): the
  elastic bar closing a channel of incompressible fluid, `FlowWall` with the prescribed wall gradient, coupled by
  `coupled_step` at mass ratios 0.5 and 4 from a static release. Aitken's factor and the least-squares update converge to
  `1e-6` in 3 iterations per step on the matching wall; on a 3:2 wall with the bilinear projection the least-squares update
  needs 4 to 7 and Aitken's factor 5 to 12, so the least-squares update is the better choice for a non-matching wall; the first
  coupled frequency is the discrete model of the pairing (the Newmark bar with the plug force
  `-m_a (u_{n+1} - 2 u_n + u_{n-1})/dt^2`, a lag of one step) to `4e-7`, and the root of `k L tan(k L) = rho_s L_s/(rho_f
  L_f)` to `1.1e-3` at `T/100`; the fluid's work on the interface is the change of the solid's energy to `5e-6` of it over
  ten periods; the loose coupling grows by about 14 per step at ratio 4. Against a real column the patterns of the force that
  are not uniform have slopes far below 1 (their added mass is that of the local flow, about `rho_f W/pi` per area, not the
  column's), so they stay at rounding on the matching wall (`1e-9` of the force) and decay with the motion on the 3:2 wall
  (at most `9e-4`), unlike the per-face mock of ADR 0410 amendment 1. The fluid mesh of that case follows the axial component of the interface only (`rcfd_post::solid_fsi::ChannelMotion`,
  a straight channel); the coupled pair damps the motion through the lag of the flow step (the energy falls to 0.027 of its
  value in ten periods at ratio 4). The earlier mock structures (`SprungWall`, `StaticSolid`) remain for the protocol's own
  tests.
* Mapping by given weights or by the builders of ADR 0263 (brute-force nearest-element search, dense radial-basis
  rows): no sparse or partitioned radial basis for large meshes, no rotational degrees of freedom of shells or beams, no
  mortar or mass-matrix map; the map is built once on the reference configurations.
* Fixed, Aitken and least-squares updates only (no reuse of earlier steps): no ramping of the load, no extrapolated predictor (the
  predictor is the flow's force at the start of the step).
* The fluid side is the incompressible PISO/PIMPLE step with implicit Euler on a moving mesh, with its limits
  ([dynamic-mesh.md](dynamic-mesh.md)). Side walls that a transverse flow must not cross need the `Symmetry` velocity
  condition; a zero-gradient velocity condition leaves their flux free.
* Tested for a wall moving along its normal with rectangular faces. Heat transfer across the interface is not exchanged.

## Versions

* `rcfd-cosim/1` (ADR 0345, with service protocol 1.1): the calls describe, time, checkpoint, restore, forces and advance
  of ADR 0256 as the methods `cosim.describe`, `cosim.time`, `cosim.checkpoint`, `cosim.restore`, `cosim.forces` and
  `cosim.advance`, with `cosim.list` and `cosim.open` (the lease). Before it the calls existed in-process only
  (`rcfd_fv::cosim`, ADR 0256), unversioned; the in-process traits are unchanged.
* `rcfd-cosim/1`, addition of ADR 0380 (F-444): the member `elements` of the reply of `cosim.describe` for a structure
  that gives elements (`Structure::elements`, a provided method returning none by default), so that the map builders of
  ADR 0263 run on the coupler's side (`InterfaceMap::build` with `RemoteStructure::elements`). A reply without elements is
  unchanged.

A change that an existing participant or coupler would not understand (a renamed member, another unit or sign, a call
with another meaning) gets a new identifier `rcfd-cosim/2`; an added method or an added member of a reply does not.

## Where to go next

* [dynamic-mesh.md](dynamic-mesh.md): the moving-mesh step and the rigid-body coupling.
* [interchange-formats.md](interchange-formats.md): the one-way export of pressure, temperature and heat-flux loads.
* [service-protocol.md](service-protocol.md): the solver service that carries the calls.
