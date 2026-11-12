# Co-simulation with a structural solver: the participant protocol (F-139; ADR 0256)

rcfd can exchange the displacement of a wall and the fluid force on it with a structural solver, step by step, and iterate
the two to agreement within each time step. Status: a library interface, in-process (`rcfd_fv::cosim`). There is no network
transport, no case key and no command-line or MCP tool yet, and the only structural participant in the repository is a mock
(a wall of sprung nodes). The decision record and the measured checks are in
`docs/adr/0256-f139-registration-cosimulation-participant.md`.

This page specifies the protocol: who sends what, in which units and on which mesh, how a coupling step runs, when it is
converged, and what happens when a participant fails. It is for people who want to connect a structural solver or write a
transport for the calls. For the moving mesh the fluid side runs on see [dynamic-mesh.md](dynamic-mesh.md); for the one-way
export of loads to a structural deck see [interchange-formats.md](interchange-formats.md); for the JSON-RPC service that a
transport could build on see [service-protocol.md](service-protocol.md).

## Participants and coupler

Two participants and a coupler:

* the **fluid** participant owns the flow and its moving mesh. Its interface mesh is the wall patch: wall points and wall
  faces. In the library this is `cosim::FlowWall` (a `PimpleSolver` on a moving mesh);
* the **structure** participant owns the structure. Its interface mesh is a set of nodes. The mock is `cosim::SprungWall`;
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
wall is a rigid segment. Any other weights can be passed to `InterfaceMap::new`. A geometric search and radial-basis
weights are not in the library.

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

* In-process only. A transport (the calls above over the JSON-RPC service) is not written.
* One wall and two participants. No simultaneous update of several participants.
* The structure in the repository is a mock: lumped nodes on springs, one direction of motion, an explicit integrator.
  No structural solver has been connected.
* Mapping by given weights only: no geometric search, no radial-basis functions.
* Fixed, Aitken and least-squares updates only (no reuse of earlier steps): no ramping of the load, no extrapolated predictor (the
  predictor is the flow's force at the start of the step).
* The fluid side is the incompressible PISO/PIMPLE step with implicit Euler on a moving mesh, with its limits
  ([dynamic-mesh.md](dynamic-mesh.md)). Side walls that a transverse flow must not cross need the `Symmetry` velocity
  condition; a zero-gradient velocity condition leaves their flux free.
* Tested for a wall moving along its normal with rectangular faces. Heat transfer across the interface is not exchanged.

## Where to go next

* [dynamic-mesh.md](dynamic-mesh.md): the moving-mesh step and the rigid-body coupling.
* [interchange-formats.md](interchange-formats.md): the one-way export of pressure, temperature and heat-flux loads.
* [service-protocol.md](service-protocol.md): the solver service a transport would extend.
