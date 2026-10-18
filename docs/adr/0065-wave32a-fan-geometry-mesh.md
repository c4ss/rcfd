# ADR 0065: Wave 32A, parametric axial-fan geometry and a body-fitted periodic-sector mesh

Status: registered before any implementation (criteria below are fixed; a miss becomes an ignored `*_known_miss` test with the
limit unchanged, a set-up defect becomes a new corrected test with the old one kept). Serves `docs/milestone-fan.md`
(capability links "parametric blade/hub geometry" and "volume mesh of a periodic blade passage"). Branch main.

## Scope statement (what this is NOT)

A lower-fidelity, hex-only, axially sheared H-type single-passage mesh of a thin blade with BLUNT (axially truncated) leading
and trailing edges. It is a geometry/meshing deliverable. It is NOT validated for accurate fan performance prediction; no
comparison with a published fan was made. Validated scope is only what the checks below measure.

## Geometry (library `rcfd-mesh::fan`)

`AxialFan`: `n_blades`, `r_hub`, `r_tip` (blade tip), `tip_clearance` (shroud radius = `r_tip + tip_clearance`), `z_inlet = 0`,
`axial_extent` (outlet at `z = axial_extent`), `z_le_root` (axial position of the hub leading edge), a section family and a
table of radial stations (first at `r_hub`, last at `r_tip`, strictly increasing), each with `chord`, `stagger` (angle of the chord
from the axis, positive = trailing edge toward -theta, i.e. the blade moves toward +theta when the rotor turns about +z),
`camber` m (fraction of chord), `camber_pos` p, `thickness` t (fraction of chord), `sweep` (axial shift of the leading edge, m),
`lean` (tangential shift of the leading edge along the cylinder, m). Piecewise-linear interpolation in r. Sections lie on
cylinders and are described in the unrolled plane (z, s = r theta); the chord frame is e_c = (cos g, -sin g), e_v = (-sin g, -cos g)
(v > 0 is the suction side, at smaller theta). Validation returns a list of errors with a path (`stations[2].chord`) and a message.

Section families (published forms):
* NACA 4-digit (checked on the NACA-airfoil Wikipedia page, fetched 2026-10-18, which cites Moran 2003 p. 7; the originals, Jacobs, Ward and Pinkerton NACA TR-460 (1933), were not read):
  `yt = 5 t (0.2969 sqrt x - 0.1260 x - 0.3516 x^2 + 0.2843 x^3 - 0.1036 x^4)` (closed trailing edge), camber
  `yc = m/p^2 (2 p x - x^2)` for x < p, `m/(1-p)^2 ((1-2p) + 2 p x - x^2)` for x >= p, `theta = atan(yc')`,
  `x_U = x - yt sin(theta), y_U = yc + yt cos(theta)`, `x_L = x + yt sin(theta), y_L = yc - yt cos(theta)`.
* Circular arc (double-circular-arc / biconvex, constructed here from elementary geometry, c = 1): camber a circular arc through
  (0,0),(1/2,m),(1,0): `yc = sqrt(R^2-(x-1/2)^2) - (R-m)`, `R = (1/4+m^2)/(2m)` (m = 0 gives a straight line); thickness an
  arc through (0,0),(1/2,t/2),(1,0): `yt = sqrt(Rt^2-(x-1/2)^2) - (Rt - t/2)`, `Rt = (1/4+(t/2)^2)/t`; same normal-offset construction as NACA.

Blade surface (`BladeSurface`): closed, oriented, watertight triangulation (section loop x spanwise strips plus root and tip caps
as strips between the two surfaces), points `(r cos(s/r), r sin(s/r), z)`.

## Mesh approach and justification

Chosen: structured hex multi-block in one index space (i axial column, j pitchwise, k radial), "H-grid with a slit": per radial
node layer a 2-D grid in the unrolled plane, cells of the solid footprint deleted (i in the chord range, j in the middle band, k below
the blade tip). Blade faces are the faces of kept cells next to deleted ones. Rejected: tet/prism from `tetmesh` (no layer
extrusion that survives a sharp periodic seam with exact node matching in the current crate), octree cut cell (periodic exactness
and wall-normal layers are not available), full O/C-H topology (best quality but exceeds the time box). Columns are z = const
lines; the seams follow the camber line inside the chord range (lower seam `s = s_c(z) - P/2`, upper = lower + P, P = 2 pi r/N) and
are axial outside it, so the upper seam is the lower seam rotated by exactly 2 pi/N and nodes match to round-off. The blade is
truncated by planes z = const at `trunc_le` and `trunc_te` (chord fractions, along the camber line) giving a blunt flat leading
and trailing face resolved by `n_mid` cells. The tip gap is `n_gap` radial layers without a slit. Cell zone `rotor` = all cells.
Patches: `inlet`, `outlet`, `hub`, `shroud`, `blade` (pressure, suction, both end faces, tip cap), `seam_lo`/`seam_hi`
(`set_periodic_rotation(seam_lo, seam_hi, 0, +z, 2 pi/N)`).

Boundary-layer stack: first-cell height `h1` either given or from a target y+: `h1 = 2 y+ nu / u_tau` (cell centre at y+),
`u_tau = U sqrt(Cf/2)`, `Cf = 0.0576 Re_c^(-1/5)` (flat-plate turbulent correlation, Re_c = U c / nu; an estimate only). `n_bl`
layers with exact geometric growth `g` on hub, shroud, tip cap (radial), the leading/trailing end faces (axial), and the blade sides
(pitchwise offset `h1/(e_s . n)` so that the wall-normal distance, measured in the unrolled section plane from the wall node along its
central-difference normal, is h1). After the exact layers the spacing continues by a geometric law solved to fit the length.

Known limits (declared in advance): cells next to the blade are sheared parallelograms, so the non-orthogonality is about the local
stagger angle; the blade is cut by axial planes (blunt edges, flat end faces of height about 2 yt(trunc)); sections must be monotone in z
(located error otherwise); the wall-normal height is exact in the unrolled section plane, the 3-D normal differs by the twist/lean rate;
the blade must fit between its seams (located error); radial spacing across the span is not normal-to-blade; the tip gap needs room for
the two boundary-layer stacks.

## Pre-registered checks (tests `crates/rcfd-mesh/tests/fan.rs`, `crates/rcfd-api/tests/fan_sector.rs`)

Test blade B: N = 5, r_hub 0.10, r_tip 0.30, clearance 0.01, NACA 4412-like linear in r: chord 0.10 (hub) to 0.07 (tip), stagger
35 deg to 20 deg, camber 0.04, p 0.4, t 0.10 to 0.06, sweep 0 to 0.01, lean 0 to 0.005. Unless stated the mesh is `trunc_le = 0.02`,
`trunc_te = 0.01`.

* G1 section: |TE - LE| = c within 1e-13 relative (NACA 0012, 4412, circular arc); polygon (shoelace) area from 400 cosine points per side vs
  Gauss-Legendre of `int 2 yt sqrt(1+yc'^2) dx` (analytic integrand, substitution x = (1-cos b)/2) within 1e-5 relative;
  NACA 00xx closed form `A = 10 t c^2 (0.2969*2/3 - 0.1260/2 - 0.3516/3 + 0.2843/4 - 0.1036/5)` within 1e-5; maximum thickness within 1 % of t
  (NACA), within 1e-12 (circular arc, x = 1/2).
* G2 surface: closed triangulated blade B surface: area vs quadrature of `|X_u x X_r|` (tensor Gauss on the exact parametrisation) and enclosed volume vs
  `int A(r) dr` (exact for sections on cylinders: dV = dr ds dz) -- volume within 1e-3, area within 2e-3 relative at 80 points per side and 24 spanwise strips,
  and the error strictly decreasing over (80,24), (160,48), (320,96) with observed order >= 1.5 between the last two.
* G3 watertight: every directed edge appears exactly once and its reverse exactly once, Euler characteristic 2, volume > 0.
* M1 volume: with V_env from the same mesh built without the blade, `|V_passage + V_solid - V_env| <= 1e-11 V_env` where V_solid is the divergence-theorem volume of
  the blade patch plus the root footprint (independent of the cell volumes); V_env against the analytic `pi/N (Rs^2 - Rh^2) L` within 5e-3 and strictly decreasing for n_pitch side 8, 16, 32;
  mesh blade solid volume vs quadrature of the z-truncated analytic sections (`int A_trunc(r) dr`, sections clipped by z-planes, dense polygons) within 2e-3 and strictly decreasing for n_chord 12, 24, 48.
* M2 topology: each internal face has two distinct cells (owner < neighbour), all cell volumes > 0, `|sum S_f| <= 1e-13 sum |S_f|` per cell, `Mesh::validate` ok.
* M3 periodicity: rotating every `seam_lo` node by +2 pi/N about z lands on a `seam_hi` node within 1e-12 m (bijection), face counts equal, rotated face centres
  match within 1e-12 m, and `FvGeometry::new` accepts the mesh.
* M4 quality (set-ups A: y+ = 30, n_bl 5; B: y+ = 1, n_bl 8; blade B, sizes in the test): max non-orthogonality <= 70 deg, max skewness <= 2.0, max aspect ratio <= 300 (A) / 3000 (B); the full distribution (max, mean, 99th percentile, counts above 50 and 60 deg) is reported.
* M5 first cell: hub, shroud, tip-cap radial spacing, leading/trailing end-face axial spacing: first spacing = h1 and ratios = g within 1e-12 relative; blade side wall-normal distance of the first node = h1
  and next ones h1 g^m within 1e-12 relative at every blade wall node; `h1` from y+ matches the formula within 1e-14; measured centre y+ with the formula's u_tau equals the target.
* S1 empty passage (no blade, same mesh), Omega = 0, all walls moving at the inflow (0,0,U), inlet fixed, outlet pressure: converged velocity equals (0,0,U) within 1e-8 U.
* S2 thin blade: NACA 00xx, t = 0.06, 0.03, 0.015, stagger 0, same walls and inflow: volume-weighted L2 of `(u - U)/U` strictly decreasing with t and <= 0.08 at t = 0.015.
* S3 laminar rotating passage (relative-frame `rotating_frame` source, small mesh, hub/blade no-slip in the frame, shroud stationary in the absolute frame, inlet axial, outlet pressure): the
  steady solver reaches its residual target within the iteration cap, and `|m_in + m_out| <= 1e-6 |m_in|`.
* S4 documented limit: the `mrf` (absolute formulation) source on the sector is refused with the located zone diagnostic (blade and seam faces are not surfaces of revolution); this is the
  present behaviour, asserted so that a later solver change is noticed. Hence S3 uses the existing relative-frame formulation.

Baseline hashes and all existing tests stay bit-identical; no solver code is changed.
