# ADR 0611: F-683 registration: the free-sector pre-stress on solid and shell sectors, and its scope without stress stiffening

Status: **REGISTERED 2026-11-15, written before any code of F-683 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-683 was written and nothing was run; the arithmetic below was done by
hand (no script; disclosed). Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-683 (dep F-680), the follow-up proposed by
[ADR 0606](0606-f680-registration-the-centrifugal-pre-stress-of-a-free-cyclic-sector.md) (its deviation 1, the mutation M9 that
escaped NC3, and its remaining question 3). Number 0611 given by the lead. Branch `f683-work` from the local main tip 2bc0141c
(which holds F-680 / ADR 0606), worktree `w49i-work/f683`, scratch `w49i-work/f683-scratch`. This is a test-only item unless a
run shows a defect of the library (then the fix is an amendment written before the corrected run).

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored `*_known_miss`
test with the limit intact and the measured number); each registered run made once; existing tests not edited (in particular
`sa_free_sector_prestress_f680.rs` and `sa_ring_whirl_beams_f679.rs`); defaults bit-identical; no edit of `baseline_hashes.rs`,
of the case and mesh hash tables or of any golden file; no new dependency, no unsafe code; clean-room. The roadmap files are not
edited (the entries go in "For the lead"). Every choice below that the maintainer would normally be asked about is taken on the
maintainer's standing instruction of 2026-11-11 (take the recommendation).

## What F-683 asks

The acceptance, binding:

> Every limit not stated here is registered in the ADR before the run. RotatingSector::new with free_sector_prestress on a free solid sector and a free shell sector gives the pre-stress of a constrained-copy route within 1e-10 and a residual at the interior nodes within 1e-10 of the load; with stress stiffening off, the opt-in leaves unit_prestress, the whirl frequencies and the refusals bit-identical to the run without it; the defaults bit-identical.

Clauses: **a0** every limit registered here; **a1** `RotatingSector::new` with `free_sector_prestress` on a free solid sector
and a free shell sector gives the pre-stress of a constrained-copy route within `1e-10` (FS); **a2** and a residual at the
interior nodes within `1e-10` of the load (RX); **a3** with stress stiffening off, the opt-in leaves `unit_prestress`, the whirl
frequencies and the refusals bit-identical to the run without it (NS1, NS2, NS3); **a4** the defaults bit-identical (DF, NC0s,
B2, B3).

## What was read

`AGENT_RULES.md`; the brief; ADR 0606 (all: D1 the two cyclic rigid motions, D2 the orthogonality of the load and `lambda = 0`,
D3 the opt-in and the gauge node, its addendum: the M9 diagnosis), ADR 0602 D4 (the constrained copy), ADR 0561 D3 (the shell
frustum sector, for the shell model). Sources at 2bc0141c: `rotating.rs` (`RotatingOptions`, `RotatingPart::new`,
`with_unit_prestress`, `unit_centrifugal_model`, `check_model`, `free_sector_unit_static`, `RotatingSector::new`), the head of
`rotating/whirl.rs` (`RotatingSector::whirl_modes`, its `"harmonic"` refusal), `loads.rs` (module note: the consistent body
loads of solids and shells), `shell.rs` (`body_load_vector_at`: forces at the projected facet carried to the nodes with their
moments `r_i x F_i`; the drilling penalty of ADR 0432 tying `theta_z` to the membrane rotation), `geometric.rs` (`Prestress`,
`of_element`, `uniform`), `builders.rs` (`hex_box`, its node sets), `hex20.rs` (`upgrade_to_hex20_mapped`), the tests
`sa_free_sector_prestress_f680.rs`, `si_rotating_cyclic.rs` (the solid disc sector D), `sa_rotating_cyclic_shells.rs` (the
frustum sector: `point`, `shell_mesh`, `sector`; helpers copied where used, not edited).

## Design

### D1. Solids and shells under ADR 0606 D1 and D2 (derived)

ADR 0606 D1 holds for any element set whose cyclic constraint maps translations by `R` (and, for shells and beams, rotations by
`R`): the cyclically symmetric rigid motions are `r_1 = (u = a, theta = 0)` and `r_2 = (u = a x (x - O), theta = a)`.

- **Solids** (Hex8, Hex20) have translations only; `r_1 = a`, `r_2 = a x (x - O)` at the nodes. Both are linear in `x`, so they
  are interpolated exactly by the isoparametric functions (`u(x) = sum N_i(xi) r(x_i) = r(sum N_i x_i)`, also on the curved
  Hex20). The body load is `f_i = int N_i b dV` at the element's mass rule, so `r^T f = sum_q w_q b(x_q) . r(x_q)`, and `b(x) . r_1
  = b(x) . r_2 = 0` pointwise (ADR 0606 D2): `r^T f = 0` at any rule.
- **Shells** (Shell4, Shell3) have six unknowns per node, the rotation `theta_z` about the normal tied to the membrane rotation by
  the drilling penalty (ADR 0432) so that the stiffness has no extra mechanism. A rigid motion `(c + w x (x - O), w)` has zero
  membrane, bending, shear and drilling strain, so it is in the null space of the element stiffness; the cyclic pair is `r_1`,
  `r_2` with `theta = a`. The body load is `t int N_i b dA` at the points `x = sum N_j p_j` of the projected facet, carried to the
  nodes by the rigid link `T^T` (forces and the moments `r_i x F_i`; `shell.rs`). The work of `r_2` on it is the work of the rigid
  motion at the points `p_i`: `sum_i F_i . (a x (p_i - O)) = t int b(x) . (a x (x - O)) dA = 0` pointwise, again because the
  motion is linear and the rigid link moves it rigidly.

So the index-0 problem of a free solid or shell sector is singular by exactly `N = span(r_1, r_2)` (if the sector has no other
mechanism, which the meshes of D2 do not have: each is a connected solid or shell body with drilling stiffness), the load is
orthogonal to `N`, and two constraints `C^T u = 0` with `C^T N` invertible carry `lambda = 0` exactly (ADR 0606 D2).

**The constraint dofs.** A solid node has no rotation, so the rotation about the axis cannot be held by a rotational unknown;
it is held by the tangential translation `t_g . u_g = 0` at a node off the axis (`t_g . r_2 = rho_g > 0`). This is what
`free_sector_unit_static` already does for every element kind: the gauge is on the three translations of one node (`c_1 = a`,
`c_2 = t_g`), whatever rotations the node has. For a shell node the alternative `a . theta_g = 0` (`a . r_2 = 1`, `a . r_1 = 0`)
would also be reaction-free by D2; it is not taken (the library's translation gauge is the same for all kinds and needs no
change). Nothing in `free_sector_unit_static` depends on the element kind except the requirement of components 0, 1, 2 at the
gauge node, which solids and shells meet. **No library change is planned**; the item checks the ADR 0606 code on solids and
shells and closes the NC3 gap with checks of the stress, the frequencies and the refusals.

### D2. The models (fixed now)

Frame: the oblique axis `a = (1, 2, 2) / 3` through `O = (0.3, -0.2, 0.5)` of ADR 0602 / 0561, `e_a = (2, -1, 0) / sqrt 5`,
`e_b = a x e_a`; steel `E = 2e11`, `nu = 0.3`, `rho = 7850`.

- **S20**: the solid disc sector of `si_rotating_cyclic.rs` (ADR 0407), `N = 12`: `hex_box([0.25, 0, -0.1], [0.5, alpha, 0.3],
  [3, 2, 1])`, `alpha = 2 pi / N`, upgraded by `upgrade_to_hex20_mapped` with the map `(r, phi, z) -> O + r (cos phi e_a + sin phi
  e_b) + z a` (in place of that test's z-axis frame); cyclic constraint `Constraint::cyclic("ymin", "ymax", a, O, N)`. Free.
- **S8**: the same with Hex8, `hex_box([0.25, 0, -0.1], [0.5, alpha, 0.3], [2, 3, 2])`, its nodes mapped by the same map (the
  connectivity and the node sets `ymin`, `ymax`, `xmin` copied). Free.
- **SH4**, **SH3**: the frustum sector of `sa_rotating_cyclic_shells.rs` (ADR 0561 D3: `N = 6`, `NC = 2` columns of elements per
  sector, `NA = 3` rows, `t = 0.05`, `CyclicOptions::new().shells(true)`), Shell4 and Shell3, **without its clamp**. Free.
- The clamped twins (for the refusals, NS3): S20 with `xmin` (the inner face) held in components 0, 1, 2; SH4 with its `clamp`
  row held in components 0 to 5.

Interior nodes: the nodes of the model in neither cyclic set (`ymin`, `ymax`; `low`, `high`).

**The constrained-copy route** (the test's own, after ADR 0602 D4, with nodes chosen independently of the library): a copy of
the free sector with `Constraint::linear` `a . u_A = 0` at the node `A` = the interior node of the smallest index and `t_B . u_B =
0` at `B` = the interior node of the largest index, `t_B = a x (x_B - O) / |(I - a a^T)(x_B - O)|` from the coordinates, plus
`Load::centrifugal(a, O, 1)`, `solve_static` with the default options and `Prestress::from_static`. `C^T N` is `[[1, a . (a x
..)], [t_B . a, rho_B]] = diag(1, rho_B)`, invertible since every node of D2 is at `rho >= 0.25`. Its gauge differs from the
library's (one node, the farthest from the axis): FS compares two different gauges, so it tests D1 and D2 and not the same
arithmetic twice.

### D3. The scope without stress stiffening (the gap of ADR 0606 NC3)

With `stress_stiffening(false)`, `RotatingSector::new` takes `RotatingPart::new`, whose pre-stress is `Prestress::uniform(model,
[0; 6])`, whatever `free_sector_prestress` is; ADR 0606 D3 fixed that the option is read only with stress stiffening on. Three
things would change if it were read (ADR 0606 M9): `unit_prestress()` would hold the free-sector stress, a sector with supports
or other constraints would be refused with `"free_sector_prestress"`, and an extra static solve would be made. The frequencies
do not change (the operator scales `K_sigma,1` by `s = 0`). NS1 to NS3 compare all three observable effects bit for bit.

## Arithmetic (disclosed, by hand)

1. Scales of S20: the hoop strain at 1 rad/s `rho r^2 / E <= 7850 x 0.5625 / 2e11 = 2.2e-8`; the radial displacement
   `<= 1.7e-8` m. Of SH4: `7850 x 1 / 2e11 = 3.9e-8`.
2. The rounding of the pre-stress of two gauges: the two solutions differ by a rigid motion (ADR 0606 D2) plus the rounding of two
   factorizations of matrices of condition `kappa`. The stress is a strain of the solution, so the rigid part drops out exactly
   in exact arithmetic and to `eps` times the size of the rigid part over the element size in floating point. ADR 0606 measured
   `9e-15` (beam ring, two gauges). For the solid and shell sectors `kappa` is larger (the shell bending-to-membrane ratio
   `(L / t)^2 ~ (0.27 / 0.05)^2 ~ 30`, the solid `~ 10^3`): an estimate of `kappa eps ~ 1e-12` relative at worst. The limit
   `1e-10` (the acceptance) leaves a factor of 100.
3. The residual at the interior nodes: `|K u - f|` in double `~ eps |K| |u| x (row length)`; for S20 `|K| ~ E h ~ 2e11 x 0.17
   = 3e10` N/m, `|u| ~ 1.7e-8`, rows of `<= 81` entries: `~ 2.2e-16 x 3e10 x 1.7e-8 x 81 ~ 1e-11` N absolute; the nodal load
   `~ rho r h^3 ~ 7850 x 0.75 x (0.15)^3 ~ 20` N, ratio `~ 5e-13`. The shells similar. Limit `1e-10`.
4. The whirl frequencies of two gauges (WS): the dense whirl solver on matrices differing at `~1e-12` relative (arithmetic 2)
   in `K_sigma,1`, which is scaled by `Omega^2` and is a fraction `~ (Omega / omega_1)^2 x (stiffening ratio)` of `K_eff`; with
   `Omega = omega_1 / 2` the frequencies move by less than `1e-12`; the solver's own rounding on the lowest modes is `~ eps
   (omega_max / omega_1)^2`, with `omega_max / omega_1 ~ 10^2 .. 10^3`: `<= 2e-10`, but two runs of the same algorithm on inputs
   that agree to `1e-12` differ by far less than this bound (ADR 0606 measured `4e-12`). Limit `1e-8`.
5. The speed of NS2 and WS: `Omega = omega_1 / 2`, `omega_1 = sqrt(lambda_1)` of `modes(2, 0.0)` of the sector built without the
   opt-in. With stress stiffening off, `K_eff = K - Omega^2 M_perp >= K - Omega^2 M` (`M_perp <= M`), positive definite on the
   index-2 space for `Omega < omega_1`: no loss of stiffness at `omega_1 / 2`, so both runs return modes. With stress stiffening
   on, the same holds up to the share of `K_sigma,1`, positive for a centrifugal hoop tension on these sectors.

## Checks (limits fixed now)

Options: `RotatingOptions::new(3).coriolis(true)` (`base`); the opt-in `base.free_sector_prestress(true)`; harmonic `k = 2` for
the whirl checks.

| Id | Clause | Check | Limit | Estimate behind it |
|---|---|---|---|---|
| FS | a1 | S20, S8, SH4, SH3: `RotatingSector::new(free, opt-in).part().unit_prestress()` against the copy route (D2): `max |S_new - S_copy|` over every element, point and component over `max |S_copy|` | `<= 1e-10` each | the acceptance; arithmetic 2: `<= 1e-12` |
| RX | a2 | S20, S8, SH4, SH3: `free_sector_unit_static(free, opt-in)`; `K u - f` (`assemble::stiffness` of the free sector, `nodal_load_vector` of its centrifugal copy) at every equation of every interior node (the gauge node among them), `max |.|` over `max |f|` | `<= 1e-10` each | the acceptance; D1: `lambda = 0`; arithmetic 3: `~ 5e-13` |
| RXc | a2 | the same residual of the copy route's solution at its interior nodes (`A` and `B` among them) | `<= 1e-10` each | as RX |
| OR | D1 | S20, S8, SH4, SH3: `|r_i^T f| / (|r_i| |f|)` for `r_1`, `r_2` of D1 (on the sector's equations; for shells with `theta = a` in `r_2`) | `<= 1e-13` | D1: zero pointwise; rounding `~ sqrt(n) eps` |
| GN | D1 | S20, S8, SH4, SH3: the library's gauge node is interior and its `rho_g` against the largest radius of the model (`0.75` solids, `1.0` shells) | interior; `|rho_g / r_max - 1| <= 1e-12` | ADR 0606 D3 |
| WS | supplementary | S20, SH4: `whirl_modes(2, Omega)` (arithmetic 5) of the opt-in sector against `with_unit_prestress(copy route, base)`: `max |omega_new / omega_copy - 1|` over every returned mode; the same count and directions | `<= 1e-8`; equal | arithmetic 4 |
| NC0s | a4 | S20, SH4 free with `base` (stress stiffening on, no opt-in) | refused, `FemError::SingularStiffness` | the free sector is singular without the opt-in |
| NS1 | a3 | S20, S8, SH4, SH3 free and the two clamped twins, `base.stress_stiffening(false)` with and without the opt-in: `unit_prestress()` element by element, point by point, component by component | bit-identical (`to_bits`) | D3 |
| NS2 | a3 | the same six models: `whirl_modes(2, Omega)` (arithmetic 5, `Omega` from the run without the opt-in): both `Modes`; `frequencies_all` and every mode's `omega` bit-identical, the directions equal | bit-identical | D3 |
| NS3 | a3 | the refusals, `stress_stiffening(false)`, with and without the opt-in: (a) the clamped twins and S20 free with an `ElasticSupport::nodal_spring` (on `xmin`, component 0, `1e6`) are accepted by `RotatingSector::new` both ways; (b) S20 free with `Constraint::displacement("xmin", 0, 1e-3)` (a non-zero value) refused both ways with equal `format!("{:?}")`; (c) `whirl_modes(N / 2 + 1, Omega)` of S20 free refused both ways with equal `format!("{:?}")`; (d) the control that the refusal exists: the clamped S20 twin and the sprung S20 with stress stiffening **on** and the opt-in are refused with `"free_sector_prestress"` | (a) both `Ok`; (b), (c) equal; (d) refused | D3; ADR 0606 NC2 |
| DF | a4 | `RotatingOptions::new(3).free_sector_prestress` | `false` | ADR 0606 D3 |
| B2 | a4 | the existing tests unedited: `sa_free_sector_prestress_f680` (incl. ADR 0606 NC3), `sa_ring_whirl_beams_f679` (incl. ADR 0602 NC0), `cargo test -p rcfd-fem --release --lib rotating` | pass | no library change |
| B3 | a4 | `git diff 2bc0141c -- crates/*/src` | empty (no library change); if an amendment changes a source file, B1 `baseline_hashes` is run | D1 |

## Negative controls

NC0s (without the opt-in the free solid and shell sectors are refused: they are free), NS3 (d) (with stress stiffening on the
opt-in refuses the clamped and sprung sectors, so NS3 (a) tests a real branch), and the mutations.

## Mutations (on a scratch copy, own `CARGO_TARGET_DIR`)

| Id | Mutation (site, `rotating.rs`) | Expected to be caught by |
|---|---|---|
| M1 | the axial gauge constraint `c_1` not added | FS, RX (the opt-in solve refused `SingularStiffness`: the test panics) |
| M2 | the tangential gauge constraint `c_2` not added | as M1 |
| M5 | a third gauge constraint on the radial component of `g` (over-constraint) | FS, RX |
| M6 | the default `free_sector_prestress: true` | DF, NC0s |
| M9 | the branch taken with the option regardless of `stress_stiffening` (ADR 0606 M9 again) | NS1 (the free-sector stress in place of zero), NS3 (a) (the clamped and sprung sectors refused) |

Each is applied by a script to a `git archive` copy of the test commit, each copy with its own `CARGO_TARGET_DIR` (the lesson of
ADR 0606 deviation 2: a shared target linked a stale crate), and the new test file run once per mutation with
`--no-fail-fast`. M9 also runs `sa_free_sector_prestress_f680` to record that ADR 0606 NC3 still misses it (the escape is
closed by the new checks, not by an edit of that test).

## Decision rule

**done**: every check FS to B3 met in the registered run, every mutation caught by at least one of its expected checks (M9 by
NS1 or NS3). **partial**: any registered check missed (recorded as a `*_known_miss`), a check met only after a post-run
amendment, a check not run, or a mutation not caught. **todo**: FS and RX not run.

## Limits (what is not covered)

- Hex8, Hex20, Shell4 and Shell3 only; tetrahedra, wedges, Shell4 ANDES membranes, layups, orthotropic and tabulated materials,
  hyperelastic structures and mixed meshes are not run (D1 holds for them by the same argument where their load is consistent).
- A solid sector touching the axis (axis nodes): the gauge excludes axis nodes, not run.
- Supported or otherwise constrained sectors with the opt-in and stress stiffening on stay refused (ADR 0606 remaining question
  2); `RotatingPart::new` and `RotatingSector::with_unit_prestress` ignore the option.
- The whirl frequencies of the solid and shell sectors are compared only against the copy route (WS), not against an independent
  reference.

## Amendment 1 (2026-11-15, written before the corrected run): the S8 mesh without its surface sets

Run 1 (tests at 0e8bcdd6, log `f683-scratch/run1.log`) is the registered run. Two of its three tests stopped at the set-up,
before any check of theirs was computed: the S8 builder of the test copied the node sets of `hex_box` but not its surface sets,
and a solid sector's cyclic constraint reads its low and high sides from the surface sets
(`cyclic::boundary`, `mesh.surface_set_nodes`), so `Constraint::cyclic("ymin", ..)` on S8 was refused (`UnknownSet { kind:
Surface, name: "ymin" }`). `free_cases()` builds all four models before the first check, so no FS, RX, RXc, OR, GN, WS, NS1,
NS2 or NS3 number was printed. What run 1 did measure: NC0s (S20, SH4 refused `SingularStiffness`: met) and DF (`false`: met);
these stand as measured and are not re-run.

Correction (a defect of the set-up, not of a limit): the S8 builder also copies every surface set of the `hex_box` mesh (the
`FaceRef`s keep their meaning, since the Hex8 block has the same elements in the same order). Nothing else changes: the models,
the copy route, the limits and the speeds are as registered. The two stopped tests stay in the file as ignored records
(`#[ignore]`, their bodies unchanged and moved into the functions `fs_run` and `ns_run`, which they call with the defective
`free_cases()` as before); two new tests, `*_amended`, call the same functions with `free_cases_a1()` (the corrected S8). The
corrected run (run 2) is made once.
