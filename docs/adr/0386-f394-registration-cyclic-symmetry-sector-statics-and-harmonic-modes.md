# ADR 0386: F-394 registration: cyclic symmetry, one sector of a rotor for statics and for modal analysis by harmonic index

Status: **REGISTERED 2026-11-13, written before any code of F-394 and before any run of its checks**. Not yet accepted by
the maintainer. Results go in the addendum at the end; nothing above the addendum is changed after a run.

Parent: [ADR 0297](0297-cyclic-symmetry-static-and-modal.md) (the plan of work package SI-7). Builds on [ADR
0296](0296-linear-multi-point-constraints-rigid-sets-elastic-supports.md) (the transformation `x = T q + g` in
`rcfd_fem::mpc`, F-393) and [ADR 0384](0384-f486-registration-modal-analysis-on-the-transformation.md) (modal analysis on
the transformation, F-486). Dependencies F-375 (loads: the centrifugal load `rho omega^2 r_perp`, done) and F-383 (mass
matrix and modal analysis, partial: what it provides, `mass::consistent_with`, `modal::modal_analysis`, the subspace
iteration of ADR 0283 with its Sturm check, is all this item uses; its open parts concern accuracy records of the basic
tier, not an interface used here). Number 0386 given by the lead. Branch `f394-cyclic-symmetry`, worktree
`w49i-work/f394`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here before the code and the runs and not changed after a run (a miss
becomes an ignored `*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical (every
model without a `Cyclic` constraint runs exactly the code it ran before); no edit of `baseline_hashes.rs`, the hash tables or
a golden file; no new dependency, no unsafe code; no existing test edited except as the maintainer's decision of 2026-11-13
permits (below); clean-room.

## What F-394 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. Equivalence: for a ring and for a disc with N = 12
> sectors, the frequencies of the sector for every harmonic index 0 to 6 equal the frequencies of the full 360 degree model
> built from the same sector mesh within 1e-8 relative, with the same count below a registered cut-off (single for k = 0 and
> 6, double otherwise); the expanded mode shapes satisfy the full model's eigen-equation within 1e-7. Closed forms: in-plane
> bending of a thin ring (thickness/radius 0.05, nu = 0), omega_n^2 = E I n^2 (n^2 - 1)^2/(rho A R^4 (n^2 + 1)) for n = 2 and
> 3 found at harmonic index 2 and 3 within 2 percent; a thin annular disc b/a = 4 spinning about its axis (statics, k = 0):
> rim displacement rho omega^2 b ((3 + nu) a^2 + (1 - nu) b^2)/(4 E) within 1 percent. Boundaries that do not match under the
> rotation within the registered tolerance are refused with the first unmatched node; N < 2 and a harmonic index above N/2
> are refused.

Clauses: (c1) equivalence of the frequencies, ring and disc, N = 12, k = 0 .. 6, within 1e-8 relative; (c2) the same count
below a registered cut-off, single for k = 0 and 6, double otherwise; (c3) the expanded modes satisfy the full model's
eigen-equation within 1e-7; (c4) thin ring, n = 2 and 3 at k = 2 and 3, within 2 percent; (c5) spinning annular disc b/a =
4, statics k = 0, rim displacement within 1 percent; (c6) unmatched boundaries refused with the first unmatched node; (c7) N <
2 refused; (c8) a harmonic index above N/2 refused.

## What was read (branch base 4a95365e, read only)

1. `AGENT_RULES.md`; ADR 0297 (plan); ADR 0296 (the elimination, D1 to D8); ADR 0384 (all, including its addendum: the
   Auto-shift zero-mode known miss, the `si_scaffold` pattern); the roadmap entries F-375, F-383, F-394, F-486, F-488.
2. Code: `crates/rcfd-fem/src/mpc.rs` (all), `cyclic.rs` (the declaration), `constraint.rs` (all), `modal.rs` (all),
   `model.rs` (`Constraint::Cyclic`, its validation in `add_constraint`: an empty axis and `sectors < 2` are refused there),
   `mesh.rs` (sets, faces, `surface_set_nodes`), `hex20.rs` (`upgrade_to_hex20_mapped`), `builders.rs` (`hex_box`), `loads.rs`
   (`nodal_load_vector`, the centrifugal load), `solve.rs` (the dispatch to `solve_static_mpc`), `crates/rcfd-linalg/src/eigen.rs`
   (`lowest_eigenpairs`, `Shift`, `ResidualTest`, `SubspaceOptions`). Tests: `si_scaffold.rs` (rows R7 and the `_after_`
   copies), `si_mpc.rs` (`n5_cyclic_still_refused`), `si_modal_mpc.rs` (helpers).
3. **Not read:** any finite-element code or manual outside this repository. Literature is cited from memory where marked,
   and every formula used is derived below.

Arithmetic done beforehand (disclosed): a short Python script evaluated the two closed forms for the registered data and
checked the rotating-disc solution below by finite differences (`sigma_r(a)`, `sigma_r(b)` of order 1e-4 Pa against 1.8e7 Pa
inside); it also evaluated the thin-ring correction estimate below. No rcfd code was compiled or run. Values: ring (`E = 2e11`,
`rho = 7850`, `t = 0.05`, `R = 1`) `omega_2^2 = 38216.56`, `omega_3^2 = 305732.48` (rad/s)^2; disc (`a = 0.25`, `b = 1`, `nu =
0.3`, `omega = 100` rad/s) `u(b) = 8.892578e-5` m.

## Method and derivation

### Cyclic symmetry by harmonic index (Bloch-Floquet on the sector)

A rotor of N identical sectors: sector `j` (`j = 0 .. N-1`) is sector 0 rotated by `j alpha`, `alpha = 2 pi / N`, about the
axis (unit direction `n`, through `o`; right-hand sense). The boundary of sector 0 has a low face L and a high face H = R L,
`R = R(alpha)` the rotation (Rodrigues: `R = I + sin(alpha) [n]x + (1 - cos(alpha)) [n]x^2`), and the high face of sector `j`
is the low face of sector `j + 1` (indices mod N). Write the displacement of sector `j` in sector-local components, `v_j =
R^(-j) u_j` (each sector then has the same stiffness `K` and mass `M` as sector 0, because the element matrices of a rotated
element are `Q K_e Q^T` with `Q = blockdiag(R^j)`). The full stiffness is block-circulant in these local components, and
the discrete Fourier transform over `j` block-diagonalises it (Thomas, Int. J. Numer. Meth. Engng 14 (1979) 81, cited from
memory; the statement follows from the circulant structure and is derived here): every full mode is a combination of modes of
the form `v_j = Re(w e^(i j k alpha))` with a complex sector amplitude `w` and a harmonic index `k`, and `k` and `N - k`
give complex-conjugate problems, so `k = 0 .. floor(N/2)` covers the spectrum.

**Inter-sector condition.** The node of H in sector `j` is the node of L in sector `j + 1`. In global components that node
moves `u = R^j v_j(H) = R^(j+1) v_(j+1)(L)`, so `v_j(H) = R v_(j+1)(L)`; with `v_j = Re(w e^(i j theta))`, `theta = k alpha`:
**`w_H = e^(i theta) R w_L`**. With `w = a + i b` (`a` the cosine part, `b` the sine part, both real sector vectors) the real
rows are

```
a_H = R (cos(theta) a_L - sin(theta) b_L)
b_H = R (sin(theta) a_L + cos(theta) b_L)
```

**Real problem of twice the size.** The Hermitian sector problem `K w = lambda M w` on the admissible set is equivalent to the
real problem on `x = (a, b)` with stiffness `blockdiag(K, K)`, mass `blockdiag(M, M)` and the rows above (the energy `w^* K w =
a^T K a + b^T K b` for real symmetric `K`; the rows are the real and imaginary parts of the complex rows). The rows are linear
and homogeneous, so the transformation of ADR 0296 applies unchanged: designated dependents `a_H`, `b_H`, `x = T q`, `K_r =
T^T blockdiag(K, K) T`, `M_r = T^T blockdiag(M, M) T` (Rayleigh-Ritz on `range(T)`, ADR 0384). If `(a, b)` is an eigenvector,
so is `(-b, a)` (multiplication of `w` by `i`, which maps the admissible set into itself): every eigenvalue of the real problem
appears **twice** for `0 < k < N/2`, and the pair is the pair of full modes of harmonic `k` and `N - k` (a standing cosine and
sine pattern, double in the full model). For `k = 0` (`theta = 0`) and `k = N/2` (`theta = pi`, N even) the rows decouple
into two identical copies `a_H = +/- R a_L`, `b_H = +/- R b_L`; the full mode is real and **single**, so the real problem of
the single size (`a` only) is solved. The doubled problem is never solved for `k = 0` or `N/2`.

**Expansion to the full rotor.** Mode `x = (a, b)` of harmonic `k` gives the full displacement `u_j(p) = s R^j (cos(j theta)
a_p - sin(j theta) b_p)` at node `p` of sector `j` (`b = 0` for a single mode). At a shared node the two sectors agree by the
rows (substituting `a_H`, `b_H` gives `R^(j+1)(cos((j+1) theta) a_L - sin((j+1) theta) b_L)`). Scale `s`: the full kinetic
energy is the sum over the sectors, `sum_j (cos a - sin b)^T M (cos a - sin b)`; with `sum_j cos^2(j theta) = sum_j
sin^2(j theta) = N/2` and `sum_j sin cos = 0` for `0 < k < N/2`, the full modal mass is `(N/2)(a^T M a + b^T M b) = N/2` for
an `M_r`-normalised `q`; for `k = 0` and `N/2` it is `N a^T M a = N`. So **`s = sqrt(2/N)` for a doubled harmonic and
`sqrt(1/N)` for a single one** makes the expanded modes M-orthonormal in the full model. The two eigenvectors of a pair expand
to two orthogonal full modes. Each node is written once, from the sector in which it is not a high node.

**Statics (`k = 0`).** A load that repeats in every sector (in sector-local components) has a solution with `v_j` the same in
every sector: the rows `u_H = R u_L` are real, and they are added to the rows of ADR 0296 so that the static solve on the
transformation (`mpc::solve_static_mpc`, any constraint values, rigid sets, supports) takes them with everything else.

### Thin ring, in-plane inextensional bending (clause c4)

Ring of centre-line radius `R`, section `A`, in-plane second moment `I`, `nu = 0` (only `E` enters). Inextensional motion:
radial `w`, tangential `v`, `w = -dv/dphi` (no centre-line stretch). Mode `v = V sin(n phi)`, `w = -n V cos(n phi)`.
Curvature change `kappa = (w + d^2 w/dphi^2)/R^2 = (n^3 - n) V cos(n phi)/R^2` (in magnitude). Rayleigh: strain energy `U = (1/2)
int E I kappa^2 R dphi = (pi/2) E I n^2 (n^2 - 1)^2 V^2 / R^3`; kinetic energy `T = (1/2) rho A omega^2 int (v^2 + w^2) R dphi
= (pi/2) rho A omega^2 R (1 + n^2) V^2`. `U = T` gives **`omega_n^2 = E I n^2 (n^2 - 1)^2 / (rho A R^4 (n^2 + 1))`**, the formula
of the acceptance (Hoppe 1871; Timoshenko, Vibration Problems in Engineering; both from memory, re-derived here). For a
rectangular section of radial thickness `t`: `I/A = t^2/12`, so `omega_n^2 = E t^2 n^2 (n^2 - 1)^2 / (12 rho R^4 (n^2 + 1))`.

Expected departure of the 3-D model from it: the formula leaves out shear and rotary inertia, centre-line stretch and the
curvature of the section; for a beam of wave number `n/R` the first-order Timoshenko correction of `omega^2` is about `-(n/R)^2
(t^2/12)(1 + E/(kappa_s G))`, `kappa_s = 5/6`, `G = E/2` for `nu = 0`: **0.28 percent at n = 2, 0.64 percent at n = 3**. The
centre-line stretch and the thick-curvature terms are of order `(t/R)^2 = 0.25 percent` and partly of the opposite sign. The
Hex20 discretisation (below, 72 elements around the ring, quadratic geometry on the arcs) adds a stiffening of order 0.1
percent or less (estimate; membrane locking of curved quadratic elements scales with `(L_e^2/(R t))^2 = 0.02`). Expected total
under 1 percent; limit 2 percent (the acceptance's).

The axial width `w = 2 t` is chosen so that the in-plane mode is the lowest at harmonic `n`: the out-of-plane bending-torsion
mode of the same ring has (thin-ring formula, from memory, used only for this choice) `omega^2 ~ E I_z n^2 (n^2-1)^2/(rho A R^4
(n^2 + E I_z/(G J)))` with `I_z = t w^3/12`; for `w = 2 t`, `J ~ 0.229 w t^3`, it is about 2.9 (n = 2) and 3.4 (n = 3) times
the in-plane `omega^2`. For a square section the two would be within 4 percent. The identification is guarded by a check (C3).

### Spinning annular disc in plane stress (clause c5): derived, agrees with the roadmap

Annulus `a <= r <= b` (`a` inner, `b` outer, free edges), plane stress, rotating at `omega`, body force `rho omega^2 r`
radially. Equilibrium `d sigma_r/dr + (sigma_r - sigma_theta)/r + rho omega^2 r = 0`; `eps_r = u'`, `eps_theta = u/r`;
`sigma_r = E/(1 - nu^2)(eps_r + nu eps_theta)`, `sigma_theta = E/(1 - nu^2)(eps_theta + nu eps_r)`. Navier: `u'' + u'/r -
u/r^2 = -(1 - nu^2) rho omega^2 r / E`. Particular `u_p = -(1 - nu^2) rho omega^2 r^3/(8 E)`; general `u = C1 r + C2/r + u_p`.
Then `sigma_r = A - B/r^2 - (3 + nu) rho omega^2 r^2/8` with `A = E C1/(1 - nu)`, `B = E C2/(1 + nu)` (using `u_p' + nu u_p/r
= -(1 - nu^2)(3 + nu) rho omega^2 r^2/(8 E)`). `sigma_r(a) = sigma_r(b) = 0` gives `B = (3 + nu) rho omega^2 a^2 b^2/8`, `A =
(3 + nu) rho omega^2 (a^2 + b^2)/8`. At the rim:

```
u(b) = rho omega^2 b/(8 E) [ (1 - nu)(3 + nu)(a^2 + b^2) + (1 + nu)(3 + nu) a^2 - (1 - nu^2) b^2 ]
     = rho omega^2 b ((3 + nu) a^2 + (1 - nu) b^2) / (4 E)
```

(the `a^2` coefficient is `(3 + nu)(2)`, the `b^2` coefficient `(1 - nu)(3 + nu - 1 - nu) = 2 (1 - nu)`). **This is the
roadmap's formula with `a` the inner and `b` the outer radius**; no open question. (Timoshenko and Goodier, Theory of
Elasticity, from memory, has the same solution.) Departure of a 3-D disc of thickness `h` from plane stress: of order `nu (h/b)^2`,
4e-5 for `h/b = 0.02`; the Hex20 mesh below (quadratic in `r`, the cubic `u(r)` over 8 elements) a few 1e-5. Limit 1 percent
(the acceptance's).

## Design

### D1. `cyclic.rs` (filled; `PENDING = None`)

- `MATCH_TOLERANCE = 1e-8` (relative). `boundary(model, constraint) -> Result<CyclicBoundary>`: the nodes of the surface sets
  `low` and `high` (`surface_set_nodes`); a node in both is refused naming it ("node 5 is on both boundaries ... (a node on the
  axis or a set given twice)"); the length `L` = the largest distance from the axis over the nodes of both sets; for each low
  node in ascending order the image `o + R (x - o)` must have a high node within `MATCH_TOLERANCE * L` (the nearest; a search
  over the high nodes sorted by one coordinate), else `InvalidValue { key: "cyclic" }` naming that **low node**, its image and
  the nearest high node with its distance; a high node taken twice is refused naming both low nodes; after all low nodes, the
  first high node left without a partner is refused naming it. Result: the pairs `(low, high)` in ascending low order, `R`,
  the unit axis, the origin, `N`, `L`.
- `HarmonicModes` and `modal_analysis_harmonic(model, extra, harmonic, options) -> Result<HarmonicModes>`: exactly one
  `Cyclic` constraint (none, or two, refused); `harmonic > N/2` (integer division, so for odd N above `(N-1)/2`) refused naming
  `k` and `N`; `harmonic = 0` runs `mpc::modal_analysis_mpc` (the `k = 0` rows are in the transformation, D2); otherwise the
  transformation of D2 for `k` (single for `2k = N`, doubled else), `K` with the supports, `M`, `blockdiag` for the doubled
  problem, `Transformation::reduce`, the mass check and `lowest_eigenpairs` with `options.modes * multiplicity` pairs (the
  subspace default of the eigensolver for that count), the default shift `Shift::Auto` as on the transformation path (ADR 0384).
  Zero values only (a non-zero prescribed or Linear value refused naming the row, the message of ADR 0384); a `RigidSet` with
  `harmonic > 0` refused (limit). Result: `harmonic`, `sectors`, `multiplicity` (1 or 2), `eigenvalues` (every eigenvalue of
  the real problem, ascending), `values` (one per sector frequency: a pair merged by its mean), `frequencies`, `pair_splits`
  (`|lambda_2i+1 - lambda_2i| / mean`, 0 for single), `modes` (one `SectorMode { cos, sin }` per eigenvalue of the real
  problem, over the model's `DofMap`; `sin` zero for single), residuals, iterations, shift, Sturm check, unknowns,
  `rigid_free`, the reduced matrices and the transformation.
- `FullRotor` and `full_rotor(model) -> Result<FullRotor>`: the sector mesh copied N times (sector `j` rotated by `j alpha`),
  each low node of sector `j >= 1` identified with the matched high node of sector `j - 1` and each high node of sector `N - 1`
  with the matched low node of sector 0; blocks keep their index, kind and property (copy `j` of element `e` of block `b` is
  element `start_b + j n_b + (e - start_b^sector)`); node, element and surface sets are the unions of their copies. Only solid
  element kinds (beam and shell are pending anyway). `FullRotor::node(j, p)`, `FullRotor::expand(sector_dofs, full_dofs,
  harmonic, mode) -> Vec<f64>` (the expansion above with the scale `s`).

### D2. `mpc.rs` (shared file, additions only)

- `uses_transformation` also true for a `Cyclic` constraint (statics and modal analysis of such a model take the
  transformation; before this package they were refused by `prescribed_equations` through `cyclic::PENDING`).
- `RowOrigin::Cyclic { constraint, node, component }` (the high node), described "constraint 2 (Cyclic 'low'/'high', high
  node 17 component 1)"; `RowOrigin::constraint` covers it.
- In `transformation`, the `Cyclic` arm: for each pair and component `c`, the row `u_(H,c) - sum_j R_cj u_(L,j) = 0`
  (terms with `R_cj = 0` omitted), designated `u_(H,c)`, after the rows of earlier constraints (constraint order, as Linear and
  RigidSet).
- `harmonic_transformation(model, prescribed, harmonic) -> Result<Transformation>` (new, public): the unknowns `a` (the mesh
  unknowns) and, for a doubled harmonic, `b` (offset `n`); rows: the prescribed rows on `a` and on `b`; each Linear row on `a`
  and on `b`; the cyclic rows above with `cos(theta)`, `sin(theta)` (for `2k = N`: `-1` and `0` exactly; `theta` evaluated as
  `2 pi ((k j) mod N)/N` is not needed in the rows, only `theta = 2 pi k/N`); a RigidSet refused. Same elimination (the tail of
  `transformation` moved into a private function `eliminate`, code motion), with the unknown names "node 4 component 1 (sine
  part)" for `b`. `Transformation::n_mesh` is the size of the (doubled) mesh vector.
- Nothing else in `mpc.rs` changes; a model without a `Cyclic` constraint builds exactly the same rows in the same order.

### D3. Files

`crates/rcfd-fem/src/cyclic.rs` (D1), `crates/rcfd-fem/src/mpc.rs` (D2), new test file `crates/rcfd-fem/tests/si_cyclic.rs`,
the docs `docs/` topic page if one describes the structural constraints (only adding a paragraph), and the permitted edits
below. Not touched: `constraint.rs` (its `Cyclic` arm already does nothing once `PENDING` is `None`), `solve.rs`, `modal.rs`
(both dispatch on `uses_transformation`), `model.rs`, `error.rs`, `loads.rs`, any golden or hash table. `rcfd-api` does not
depend on `rcfd-fem` (checked: only `rcfd-post` does), so `baseline_hashes` cannot change; `rcfd-post` is checked once
with `cargo check -p rcfd-post --tests` (it must still compile against the additions).

### D4. Permitted edits of existing tests (maintainer decision 2026-11-13)

This package retires the refusal row R7 (`Cyclic`, "ADR 0297") of `si_scaffold.rs` and the `Some("ADR 0297")` assertion.
Marked `#[ignore]` with a reason naming ADR 0386 as a record, bodies untouched: `x1_the_scaffold_exists_after_landed`,
`r1_to_r9_static_solve_refuses_each_new_variant_after_landed`, `r10_refused_on_every_static_path_after_landed`; copies
`..._after_cyclic_landed` are added without the retired row (`cyclic::PENDING` is `None`; R7 added to the landed list; the
remaining rows R2, R3, R4, R4b unchanged). In `si_mpc.rs`, `n5_cyclic_still_refused` asserts the same refusal; it is marked
`#[ignore]` as a record naming ADR 0386 (its only row is retired, so no copy). R7's model itself (a box with `ymin`/`ymax` as
low/high about the `x` axis, 12 sectors) is now refused by the matching (its high face is not the low face rotated by 30
degrees): that is checked as a negative control here (R1d).

## Registered checks

All in `crates/rcfd-fem/tests/si_cyclic.rs`, release, run once (`cargo test -p rcfd-fem --release --test si_cyclic --
--test-threads=1 --nocapture`). Material `E = 2e11` Pa, `rho = 7850` kg/m^3, `nu` as stated. `alpha = 2 pi/12`.

Meshes (Hex20 by `upgrade_to_hex20_mapped` of `hex_box` with the polar map `(r, phi, z) -> (r cos phi, r sin phi, z)`, so
every node, mid-edge nodes included, lies on the exact circles; `ymin` is the low face `phi = 0`, `ymax` the high face `phi =
alpha`, axis `(0, 0, 1)` through the origin, 12 sectors):

- **RF (free ring, closed form)**: `hex_box([R - t/2, 0, -w/2], [t, alpha, w], [1, 6, 1])`, `R = 1`, `t = 0.05`, `w = 0.1`, `nu
  = 0`; no constraint but the cyclic one.
- **RS (supported ring, equivalence)**: `[1, 4, 1]` divisions, otherwise RF; the nodes of the inner face (`r = R - t/2`) at `phi
  = alpha/2` (two corners and one mid-edge node) clamped (three `Displacement` on a node set `spoke`).
- **DS (disc, equivalence)**: `hex_box([0.25, 0, -0.1], [0.75, alpha, 0.2], [3, 1, 1])`, `nu = 0.3`, inner face `xmin` clamped.
- **DP (spinning disc, statics)**: `hex_box([0.25, 0, -0.01], [0.75, alpha, 0.02], [8, 2, 1])` (`a = 0.25`, `b = 1`, `h = 0.02`),
  `nu = 0.3`, `Load::centrifugal([0,0,1], [0,0,0], 100)`, `Displacement` `u_z = 0` on the node set `mid` (nodes with `z = 0`,
  the mid-edge nodes of the axial edges) and `u_y = 0` on the node `(a, 0, 0)` (holds the rotation about the axis, the one
  rigid motion the `k = 0` rows and `mid` leave).
- The full models **RS-full**, **DS-full**, **DP-full**: `full_rotor` of the sector, material on `all`, the same
  `Displacement` constraints on the replicated node sets (a clamp and `u_z` are invariant under the rotation about `z`); for
  DP-full the `u_y = 0` on the image of the node in sector 0 only.

Eigen options: sector problems of RS and DS `ModalOptions::new(4)` (4 sector frequencies per harmonic) with tolerance `1e-9`
(relative residual), `max_iterations` 400; their full models `ModalOptions::new(n_c + 8)` (`n_c` the sector count below the
cut-off) with the same tolerance and cap. RF: `ModalOptions::new(2)` with `ResidualTest::Backward`, tolerance `1e-12` (the free
thin ring has `lambda_max/lambda_1 ~ 4e7`, whose relative-residual floor `~ eps lambda_max/lambda_1 ~ 4e-9` is above 1e-10;
ADR 0354). The supported ring and the thick disc have `lambda_max/lambda_1 ~ 5e4` and `~ 2e5` (estimates: smallest node
spacing a half element, `lambda_max ~ 50 c^2/h_min^2`), relative-residual floors near 1e-11, under 1e-9.

**Cut-off (clause c2).** `lambda_c = 0.999 min_k lambda_(4),k`, `lambda_(4),k` the largest of the four sector values of
harmonic `k` (so every sector eigenvalue below `lambda_c` of every harmonic has been computed). Sector count `n_c = sum_k
m_k #{values_k < lambda_c}`, `m_k = 1` for `k = 0, 6` and 2 otherwise. The full model is solved for `n_c + 8` modes; its largest
computed eigenvalue must exceed `lambda_c` (else the set-up is defective, an amendment). If any sector or full eigenvalue lies
within `1e-7` relative of `lambda_c` the count would be ambiguous: also a defective set-up (amendment), not a miss.

| Id | Clause | Check | Limit | Estimate behind it |
| --- | --- | --- | --- | --- |
| E1 | c1, c2 | RS: count of the full eigenvalues below `lambda_c` equals `n_c`; the sorted sector eigenvalues below `lambda_c` (each pair as its two eigenvalues; single ones once) against the sorted full ones: `max |f_s/f_f - 1|` | equal; `<= 1e-8` | the same discrete problem (block-circulant reduction exact); rounding of the element matrices in rotated coordinates `~ eps lambda_max/lambda` with random signs, below 1e-11; eigenvalue error of the solves `~` (residual)^2, below 1e-14 |
| E2 | c1, c2 | DS: as E1 | equal; `<= 1e-8` | as E1 (`lambda_max/lambda_1 ~ 2e5`) |
| E3 | c1 | RS and DS: per harmonic, `m_k #{values_k < lambda_c}` equals the number of full eigenvalues that match a value of `k` (pairing in the sorted order of E1); and for `0 < k < 6` every pair split `pair_splits <= 1e-8`; for `k = 0, 6` the multiplicity is 1 | as stated | the double eigenvalue is exact in the real problem; split by rounding `~ eps lambda_max/lambda` |
| E4 | c3 | RS and DS: every sector mode below `lambda_c` expanded by `FullRotor::expand`: `||(K phi - lambda M phi)_F|| / ||(K phi)_F||` over the free equations of the full model, `lambda` the sector eigenvalue | `<= 1e-7` | the full residual is the expanded sector residual: the relative residual of the sector solve, `<= 1e-9` |
| E5 | (expansion) | RS and DS: the expanded modes below `lambda_c` of all harmonics, `max_ij |phi_i^T M phi_j - delta_ij|` with the full consistent `M` | `<= 1e-8` | the scale `s` (derivation); orthogonality across harmonics exact, within a pair by the Ritz vectors; rounding ~1e-14 |
| E6 | c8 | RS and DS: `k = 6` is solved single (`multiplicity == 1`) and `k = 1 .. 5` doubled (`multiplicity == 2`, `eigenvalues.len() == 2 values.len()`) | as stated | D1 |
| C1 | c4 | RF, `k = 2`: lowest value `lambda` against `omega_2^2` of the formula (`I/A = t^2/12`): `|lambda/omega_2^2 - 1|` | `<= 0.02` | under 1 percent (above) |
| C2 | c4 | RF, `k = 3`: as C1 with `omega_3^2` | `<= 0.02` | under 1 percent |
| C3 | c4 | RF, `k = 2, 3`: the selected mode is in-plane: the axial share `sum_(i in z equations) x_i (M_2 x)_i / x^T M_2 x` of the sector mode `x = (a, b)` (`M_2 = blockdiag(M, M)` of the sector) | `<= 1e-2` | with `nu = 0` and a section symmetric about `z = 0`, the in-plane mode has `u_z` from the curvature only, `~0` |
| S1 | c5 | DP: mean of `u . e_r` over the nodes of `xmax` (`r = b`) of the sector against `rho omega^2 b ((3 + nu) a^2 + (1 - nu) b^2)/(4 E)`: `|mean/u_b - 1|` | `<= 0.01` | `~1e-4` (above) |
| S2 | (statics) | DP against DP-full (direct solves): `max_p |u_sector(p) - u_full(node(0, p))|_inf / max |u_full|_inf` over every sector node `p` (high nodes included) | `<= 1e-8` | the same discrete problem; rounding ~1e-13 |
| X1 | D1 | `cyclic::PENDING == None`; `uses_transformation` of a model with a Cyclic constraint is true and of RS without it false | as stated | D2 |

**Negative controls (clauses c6 to c8; each must hold):**

| Id | Control | Expected |
| --- | --- | --- |
| R1 | RS with the 3rd node of the high boundary (ascending order) moved radially by `1e-6 L` (`L = R + t/2`, 100 times the tolerance) | `Err(InvalidValue { key: "cyclic" })`, the reason names "low node P" with `P` the low partner of the moved node (the first unmatched low node) |
| R1b | the same move by `1e-10 L` (below the tolerance) | `boundary` succeeds with the same pairs |
| R1c | RS with an extra face in `high` (a face of the element next to `ymax`, so the high set has nodes that are no image) | refused naming the first unmatched node (low or high, whichever the order of D1 meets first; the reason names a node and "no node" / "no low node") |
| R1d | R7's model of `si_scaffold.rs` (box, `ymin`/`ymax` about `x`, 12 sectors), `solve_static` | `Err(InvalidValue { key: "cyclic" })` naming low node 0's image |
| R2 | `Constraint::cyclic(..., 1)` and `(..., 0)` added to RS's model | `Err(InvalidValue { key: "sectors" })` (c7; existing validation) |
| R3 | RS, `modal_analysis_harmonic` with `k = 7`; a 5-sector rotor (DS geometry with `alpha = 2 pi/5`) with `k = 3` | `Err(InvalidValue { key: "harmonic" })` naming `k` and `N` (c8); `k = 2` of the 5-sector rotor is accepted and doubled |
| R4 | low and high the same set (`ymin`, `ymin`) | refused naming node 0 of the set as on both boundaries |
| R5 | `modal_analysis_harmonic` of a model without a Cyclic constraint; with two | refused (`key: "cyclic"`) |
| R6 | RS plus a RigidSet at `k = 2`; RS with a prescribed value `1e-3` at `k = 2` | refused (`RigidSet` named; "zero values only") |

**Mutations** (on a `git archive` copy of the final tree in `w49i-work/f394-scratch`, own target directory; each applied
alone by exact text replacement, `si_cyclic` run once, the file restored and compared byte for byte):

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU1 | the sign of `sin(theta)` in the rows of the sine part (`b_H = R(-sin a_L + cos b_L)`) | E1 or E2 |
| MU2 | the rows use `R^T` (rotation of the displacements by `-alpha`) while the positions match with `R` | S1 or S2 |
| MU3 | expansion scale `sqrt(1/N)` for a doubled harmonic | E5 |
| MU4 | `k = N/2` solved doubled | E6 (and the counts of E1, E2) |
| MU5 | the matching tolerance test removed (the nearest high node always accepted) | R1 |
| MU6 | expansion `cos a + sin b` instead of `cos a - sin b` | E4 |
| MU7 | the pairs not merged (`values` = every eigenvalue, multiplicity 1 reported) | E3 or E6 |
| MU8 | the node-on-both-boundaries refusal removed | R4 |

## Decision rule

**Done** if every check meets its limit, every negative control holds, MU1 to MU8 are caught on the first run, and no
existing test fails other than the ones made ignored records under D4; **partial** if a check misses or is not run, a
negative control fails, a mutation survives, or another existing test fails; **todo** if the harmonic problem cannot be built
or a model without a Cyclic constraint changes its result.

## Limits (what is not covered)

- One Cyclic constraint per model; solid elements only; the boundaries must match node for node (no non-matching
  interpolation); a node on the axis is refused (an annulus only).
- Statics at `k = 0` only (a load that repeats in every sector); no harmonic loads (`k > 0` statics) and no harmonic
  response.
- Modal analysis by harmonic: zero constraint values only; `Linear` constraints apply to the cosine and sine parts alike (the
  same constraint in every sector); a `RigidSet` at `k > 0` is refused (at `k = 0` it works through ADR 0296); elastic supports
  are added to both parts (a support on a boundary node acts on the shared node once: one put on both boundaries counts
  twice, the user's responsibility, documented).
- The `Displacement` constraints of a sector act in every sector in that sector's frame; only components that are invariant
  under the rotation (all three, or along the axis) mean the same in the full model. The full-rotor builder copies the mesh
  and its sets; constraints and materials of the full model are the caller's.
- No pre-stress, spin softening or Coriolis terms (ADR 0299, the demonstration ADR 0309); no complex arithmetic (the real
  cos/sin pair is the route).
- The matching search is `O(n log n)` by sorting one coordinate; a boundary with very many nodes at the same value of that
  coordinate degrades to `O(n^2)`.
- The free-ring zero modes at `k = 0, 1` (Auto shift) are not measured here (F-488 covers the zero-mode measure).

## Amendment 1 (2026-11-13, before the test file was compiled or any check run)

1. **DP-full (check S2) was defective as registered.** With `u_z = 0` on `mid` and `u_y = 0` at the image of the pin node in
   sector 0 only, the full disc keeps two in-plane rigid motions free (the translations `x` and `y` combined with the rotation
   about the axis; the `k = 0` rows exclude them from the sector, the full model has no such rows), so its static solve is
   refused by the rigid-motion check. Corrected: DP-full holds `u_y = 0` at the images of the pin node `(a, 0, 0)` in
   sectors 0 and 6 (`phi = 0` and 180 degrees) and `u_x = 0` at its image in sector 3 (`phi = 90` degrees). The exact
   axisymmetric solution has `u_theta = 0`, so each held component is zero in it (`u_y = u_theta cos(phi) = 0` at 0 and 180
   degrees, `u_x = -u_theta sin(phi) = 0` at 90 degrees); the three rows hold `t_x`, `t_y` and the rotation about `z`. The
   limit of S2 is unchanged.
2. **R3, 5-sector rotor at `k = 2`:** the eigen options were not fixed; they are those of the equivalence runs
   (`ModalOptions::new(2)` with tolerance `1e-9`, cap 400).
