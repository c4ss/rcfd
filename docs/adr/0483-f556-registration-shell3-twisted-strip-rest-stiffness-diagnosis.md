# ADR 0483: F-556 registration: diagnosis of the Shell3 rest stiffness of the twisted strip against Hex20 (the first torsion mode of ADR 0473, located by registered element and mesh variations)

Status: **REGISTERED 2026-11-14, written before any code of F-556 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-556 was written, compiled or run in the repository: the files listed under "What was
read" were read, and the arithmetic below was done by hand and with one plain Python script in the scratch folder
(`w49i-work/f556-scratch/shares.py`, closed forms only, no rcfd code; disclosed, its numbers quoted below). The bit record B1
is a test file written and run on the base after this commit and before any other code (D4). Results go in the addenda at the
end; nothing above the addenda is changed after a run.

Roadmap item F-556 (deps F-420), structural solver, advanced tier ([ADR 0310](0310-structural-solver-advanced-tier-plan.md)),
the follow-up proposed by [ADR 0473](0473-f420-registration-stress-stiffening-of-beams-and-shells.md) (F-420, its E2 known
miss). Number 0483 given by the lead. Branch `f556-work` from 44b6c812, worktree `w49i-work/f556`, scratch
`w49i-work/f556-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a missed limit becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; existing tests not
edited; no new dependency, no unsafe code, the crate graph unchanged; clean-room. Every choice below that the maintainer would
normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).

## What F-556 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. The 4.05 percent excess of the Shell3 rest frequency of the third mode of the twisted strip of ADR 0473 over Hex20 (Shell4 0.53 percent) is located in the Shell3 membrane-bending coupling on warped triangles (CST or ANDES membrane, MITC3 shear) by registered element and mesh variations against the Hex20 strip, before any change to Shell3; any fix is opt-in and the default Shell3 results stay bit-identical.

Clauses: **a1** the excess (4.05e-2 Shell3, 5.3e-3 Shell4, the rest frequency of the third track) is reproduced; **a2** it is
located in the Shell3 parts named (the membrane, CST or ANDES, the MITC3 shear, their coupling through the warped geometry) by
registered element and mesh variations against the Hex20 strip; **a3** before any change to Shell3; **a4** any fix opt-in,
default Shell3 results bit-identical. The brief of the lead adds: register a variation matrix (membrane choice, diagonal split
direction and the angle split of F-540, mesh refinement, twist angle including 0 degrees, thickness) and what each outcome would
mean; a fix, if any, opt-in, with a default-bits test at the base before any library change; if a fix is too large, stop at the
diagnosis and propose it as a follow-up.

## What was read (44b6c812, read only)

ADR 0473 whole (arithmetic 7 the twisted strip, D2 the shell geometric stiffness, E1 to E3, addendum 1 with the E2 known miss
and the set-up correction of the Hex20 map), its run log `w49i-work/f420-scratch/run_e1.txt` (the rest frequencies quoted in
arithmetic 1 below); ADR 0454 (Shell3: facet, CST membrane, MITC3 tying, the stabilisation, the drilling penalty), ADR 0466
(F-539, the ANDES OPT and Allman membranes of Shell3, `BlockProperty::shell_with_membrane`), ADR 0470 (F-549, the ANDES
membrane of Shell4), ADR 0461 (F-540, the angle-weighted pressure); the sources `shell3.rs` (whole), `shell.rs` (the options,
`SectionStiffness`, `homogeneous_section`, `element_stiffness_section`, the constants), `mesh.rs` (`BlockProperty::shell*`),
`modal.rs` (`modal_analysis`, `ModalResult::dofs`), `rcfd-linalg/src/eigen.rs` (`lowest_eigenpairs`, `SubspaceOptions`),
`rcfd-linalg/src/matrix.rs` (`CsrMatrix::from_triplets`); the tests `sa_geometric_struct.rs` (the strip builders, `table_of`,
`compare`, `e3`, `e2_twisted_strip_shell3_known_miss`), `sa_geometric_struct_bits.rs` and `sa_shell3_membrane_bits.rs` (the
digest pattern).

Published sources, cited from memory, **not read** for this work (the formulas used are derived below; the sources name them):
D. J. Allman, Comput. Struct. 19 (1984) 1 (the triangle with vertex rotations); C. A. Felippa, Comput. Methods Appl. Mech.
Engrg. 192 (2003) 2125, "A study of optimal membrane triangles with drilling freedoms" (ANDES OPT, its constant-strain parasitic
shear in in-plane bending); P. S. Lee and K. J. Bathe, Comput. Struct. 82 (2004) 945, "Development of MITC isotropic triangular
shell finite elements" (MITC3); M. Lyly, R. Stenberg and T. Vihinen, Comput. Mech. 13 (1993) 92 (the stabilisation); A. Rosen,
Appl. Mech. Rev. 44 (1991) 483, "Structural and dynamic behavior of pretwisted rods and beams" (the torsional stiffening of a
pretwisted strip, `E t c^5 theta'^2 / 180`), with the Saint-Venant strip `G c t^3 / 3` of any elasticity text.

## Arithmetic before this registration (disclosed)

1. **The base numbers (ADR 0473 run log, not a run of F-556).** Third track at rest: Hex20 442.797470 Hz, Shell4 445.161813 Hz
   (`delta_4 = 5.3396e-3`), Shell3 460.730207 Hz (`delta_3 = 4.0499e-2`); the other tracks of Shell3 within 3.8e-3.
2. **The torsion of a pretwisted strip (derived).** A section of chord coordinate `s` turned by `phi(r)` about the span axis
   moves the point `s e_c(r)` by `phi s e_n(r)`; with `e_c' = theta' e_n`, `e_n' = -theta' e_c`, the derivative along the
   fibre tangent `t = e_r + s theta' e_n` gives the fibre strain `t . d/dr (phi s e_n) = theta' phi' s^2`. A free end relieves
   the mean `c^2/12`; the energy per length `(1/2) E t int (theta' phi')^2 (s^2 - c^2/12)^2 ds = (1/2) E t theta'^2 c^5/180
   phi'^2` (the integral `c^5 (1/80 - 1/72 + 1/144) = c^5/180`). So `GJ_eff = G c t^3/3 + E t c^5 theta'^2/180`, and the
   clamped-free first torsion `omega = (pi/2L) sqrt(GJ_eff/(rho c^3 t/12))` (the warping restraint of the root neglected). The
   second term is carried by **membrane** strain, the first by bending; its share `m = (E t c^5 theta'^2/180)/GJ_eff`:

   | set-up | Saint-Venant N m^2 | pretwist N m^2 | membrane share `m` | closed form Hz |
   |---|---|---|---|---|
   | twist 0, `t = 1 mm` | 1.2821 | 0 | 0 | 208.7 |
   | twist 15 deg, `t = 1 mm` | 1.2821 | 1.0577 | 0.4521 | 281.9 |
   | twist 30 deg, `t = 1 mm` (base) | 1.2821 | 4.2308 | 0.7674 | 432.8 |
   | twist 30 deg, `t = 0.5 mm` | 0.1603 | 2.1154 | 0.9296 | 393.2 |
   | twist 30 deg, `t = 2 mm` | 10.2564 | 8.4616 | 0.4521 | 563.9 |

   The base closed form 432.8 Hz is 2.3 percent below Hex20 (442.8): the third track is the first torsion, and three quarters of
   its stiffness is membrane strain coupled to the torsion by the twisted geometry. On flat facets that coupling exists only
   through the angles between facets (the triangles are flat, the surface is warped).
3. **The candidate locations and their signatures.** Let `Delta_3 = delta_3 - delta_4` (the Shell3-specific excess; both
   against the same Hex20), `e_m` the relative membrane-energy excess of the Shell3 membrane on the coupled field.
   - **H1, the membrane interpolation (CST).** The coupled field has the axial strain `theta' phi'(r)(s^2 - c^2/12)`, varying
     across the chord; the constant-strain triangle cannot vary its strain inside an element without parasitic in-plane shear
     (the in-plane bending defect of the CST, ADR 0466: 0.543 of beam theory on 8 x 2). Then `Delta_3 ~ (1/2) m e_m`, `e_m`
     independent of `t` and of the twist (linear problem, same field shape), `O(h^2)`. From arithmetic 1, `e_m = 2 Delta_3/m =
     2 (4.0499e-2 - 5.340e-3)/0.7674 = 9.16e-2`. Predicted: twist 0 `Delta_3 ~ 0`; twist 15 and `t = 2 mm` `Delta_3 ~ 2.07e-2`;
     `t = 0.5 mm` `4.26e-2`; 24 x 72 a quarter of 12 x 36; ANDES (exact in linear in-plane strain variation of rectangle pairs)
     removes most of it; the three diagonal patterns alike; the drilling and shear factors immaterial.
   - **H2, the facet geometry of the split pattern.** The flat facets approximate the helicoid differently along the two
     diagonals; a geometric error would stay with ANDES (same facets) and change with the split pattern.
   - **H3, the bending and the MITC3 shear.** An excess in the Saint-Venant part: present at twist 0, sensitive to the
     stabilisation, growing with `1 - m`.
   - **H4, the drilling penalty on warped facets.** The torsion rotation has a drilling component `phi n . e_r = -phi s theta'`
     (up to 0.087 `phi` at the edges) that the penalty ties to the membrane rotation; sensitive to the drilling factor, not
     present with ANDES (no penalty), with a drilling-energy share above a few percent.
4. **The diagonal congruence.** `S(r, -s) = R_x(pi) S(r, s)` exactly (negation of `y, z`, exact in floating point); the index
   map `i -> NCH - i` sends the split `a-c` of every cell to the split `b-d`. The two meshes are congruent by a rigid rotation
   (the clamp included): their frequencies are equal up to rounding. The direction of the diagonal alone therefore cannot
   matter on this strip; the alternating pattern is the one different split (a control C3 checks the congruence).
5. **The angle split of F-540** (ADR 0461) shares a pressure load by the interior angles; a rest-frequency problem has no load,
   so it does not enter: recorded here, no run.
6. **Sizes.** Shell 12 x 36: 481 nodes, 2886 unknowns; 24 x 72: 1825 nodes; 48 x 144: 7105 nodes, 42630 unknowns. Hex20
   12 x 36 x 1: 3267 nodes; 24 x 72 x 1: about 12.6e3 nodes, 38e3 unknowns. Free RAM 18.9 GB at registration.

## Design

### D1. No library change

F-556 is a diagnosis. It adds two test files and no library code: every variation below is reachable through the existing API
(`BlockProperty::shell_with_membrane`, `shell4_with_membrane`, `shell3::element_stiffness_with` with `Shell3Options`, which
reaches the stabilisation and the drilling factor that a model cannot). If H1 holds, the remedy is the existing opt-in ANDES
membrane of ADR 0466 and check F1 measures it on the registered E2 rule; any other remedy is proposed as a follow-up, not
written here (taken on the maintainer's standing instruction of 2026-11-11: the brief allows stopping at the diagnosis).

### D2. Set-ups (fixed now)

`G(nch, nsp, twist, t, split)`: chord `c = 0.05`, span `L = 0.15`, root on the axis, `theta(r) = twist r/L`, nodes on the
mid-surface `S(r, s) = (r, s cos theta, s sin theta)` at `r = L j/nsp`, `s = -c/2 + c i/nch`; steel `E = 2e11`, `nu = 0.3`,
`rho = 7850` (as ADR 0473); root nodes clamped (6 components shells, 3 components solids). Splits of the cell `(a, b, c, d) =
(i,j), (i+1,j), (i+1,j+1), (i,j+1)`: **AC** `[a,b,c], [a,c,d]` (ADR 0473), **BD** `[a,b,d], [b,c,d]`, **ALT** AC when `i + j`
is even, BD otherwise. Models: Shell3 with the membrane CST (default), ANDES OPT or Allman; Shell4 bilinear (default) or ANDES;
Hex20 `nch x nsp x 1` by the mapped upgrade of ADR 0473 (the thickness coordinate `-p[2]`, its addendum's correction).
Rest frequencies: `modal_analysis` with 6 modes, `ResidualTest::Backward`, tolerance `1e-10` (the options of ADR 0473 E1).

**The own assembly (OA).** For the variations a model cannot reach: the Shell3 global `K` from
`shell3::element_stiffness_with(coords, t, material, options)` and `M` from `shell3::element_mass`, unknowns `6 n + c`, the
root rows and columns dropped, `CsrMatrix::from_triplets`, `lowest_eigenpairs` with 6 pairs, `Shift::Value(0)`, tolerance
`1e-10`, `ResidualTest::Backward`, the other options default.

**The torsion mode.** Tip section `r = L`: `e_c = (0, cos theta_L, sin theta_L)`, `e_n = (0, -sin theta_L, cos theta_L)`; at the
leading (`s = -c/2`) and trailing (`s = c/2`) tip points (shell nodes; the Hex20 mid-edge nodes of the thickness edges, which sit
on `S(L, -+c/2)`, found by position within `1e-9`) the displacements give `w_L, w_T` (along `e_n`) and `c_L, c_T` (along `e_c`);
`tau = |w_L - w_T| / (|w_L + w_T| + |w_L - w_T| + |c_L + c_T|)`. The torsion mode of a run is the mode of the six with the
largest `tau`; its frequency `f_T`. `delta = f_T(shell)/f_T(Hex20 of the same twist, thickness and the reference mesh) - 1`.

**The energy partition.** For a mode `x` (M-orthonormal) and each element, the parts `(1/2) x_e^T K_p x_e` with
`shell3::element_stiffness_section` (or `shell::element_stiffness_section` for Shell4) on partial sections of
`homogeneous_section`: membrane (`A` only), bending (`D` only), shear (`H` only), drilling (`G` of `drilling` only; `t` kept,
since the stabilisation reads it); every other part zero. The stiffness is linear in the section, so the parts sum to the whole.

### D3. The runs (each made once)

| group | runs | reference |
|---|---|---|
| base (12 x 36, 30 deg, 1 mm) | Hex20; Shell4; Shell4 ANDES; Shell3 CST AC, BD, ALT; Shell3 ANDES AC, ALT; Shell3 Allman AC | Hex20 12 x 36 |
| OA (base, CST AC) | default options; stabilisation `0`; drilling `1e-4`, `1e-2` (default `1e-3`) | Hex20 12 x 36 |
| OA twist 0 | default options; stabilisation `0` | Hex20 twist 0 |
| twist | 0 and 15 deg: Hex20, Shell4, Shell3 CST AC, Shell3 ANDES AC | Hex20 of the twist |
| thickness | 0.5 and 2 mm at 30 deg: Hex20, Shell4, Shell3 CST AC, Shell3 ANDES AC | Hex20 of the thickness |
| refinement | Hex20 24 x 72 x 1; Shell4, Shell3 CST AC, Shell3 ANDES AC at 24 x 72 and 48 x 144 | Hex20 24 x 72 |
| energy | the torsion modes of Shell3 CST AC, Shell3 ANDES AC, Shell4 at the base | — |
| F1 | `RotatingPart::campbell` of Shell3 ANDES AC and of Hex20, the E1/E2 rule of ADR 0473 | Hex20 table |

### D4. The bit record (B1)

`crates/rcfd-fem/tests/sa_shell3_twist_bits.rs`, using only the API of 44b6c812, run once on the base with this ADR committed
and no other line changed; its printed digests (FNV-1a 64 of the count and the `to_bits` of the values, ADR 0291) become its
constants. Digests: the default Shell3 strip (base) stiffness, mass and six rest frequencies; the ANDES Shell3 strip stiffness
and frequencies; the Shell4 strip stiffness and frequencies; the Hex20 strip stiffness; one twisted Shell3 element by
`element_stiffness` and by `element_stiffness_with` for each membrane. Since F-556 changes no library line, B1 guards the
claim a4 and any later fix.

## Checks (limits fixed now)

Hard checks (a miss is a `*_known_miss` test, the limit kept):

| id | clause | quantity | limit | estimate |
|---|---|---|---|---|
| C1 | a1 | base torsion `f_T` of Hex20, Shell4, Shell3 CST AC against 442.797470, 445.161813, 460.730207 Hz, relative | `<= 1e-7` each | `< 1e-9` (6-decimal record) |
| C1b | a1 | base `delta_3`, `delta_4` against 4.0499e-2, 5.3396e-3 | `<= 1e-5` abs each | rounding |
| C2 | a2 | every run: `tau` of the torsion mode | `>= 0.8` | `~ 1` |
| C2b | a2 | base: the torsion mode is the third (index 2) for Hex20, Shell4, Shell3 CST AC | each | ADR 0473 |
| C3 | a2 | AC against BD (CST), six frequencies, relative | `<= 1e-8` | rounding (arithmetic 4) |
| C4 | a2 | OA default against the model Shell3 CST AC, six frequencies, relative | `<= 1e-8` | rounding |
| C5 | a2 | energy: `|sum of parts / whole - 1|`; `|whole / (lambda/2) - 1|` | `<= 1e-11`; `<= 1e-8` | `1e-14`; `1e-12` |
| C6 | a2 | Hex20 24 x 72 against 12 x 36 at the base, torsion, relative | `<= 5e-3` | `~ 1e-3` |
| C7 | a2 | Hex20 torsion against the closed form of arithmetic 2, the five set-ups, relative | `<= 6e-2` each | base 2.3e-2 |
| F1 | a4 | E2 of ADR 0473 with the ANDES Shell3 (tracks 1 to 4, speeds 0, 120, 240, 360 rad/s): `max |omega_shell/omega_solid - 1|`; and its E3 (no loss, MAC `> 0.9`) | `<= 4e-2`; each | `< 1e-2` |
| B1 | a3, a4 | every digest of `sa_shell3_twist_bits.rs` equal to the base's | equal | equal |

Diagnostic predictions (evaluated by the decision rule; printed and classified by the test, not asserted; `Delta = delta_3 -
delta_4` of the same set-up; `Delta_b` the base CST AC value of this run, `e_m = 2 Delta_b/0.7674`):

| id | hypothesis | prediction | estimate |
|---|---|---|---|
| P1 | H1 | twist 0: `|Delta| <= 5e-3` | `~ 0` |
| P2 | H1 | ANDES AC at the base: `|Delta_ANDES| <= 0.25 Delta_b` | small |
| P3 | H1 | twist 15, `t = 0.5`, `t = 2 mm`: `Delta / ((1/2) m e_m)` in `[0.6, 1.4]` each (`m` of arithmetic 2) | `1` |
| P4 | H1 | refinement (against Hex20 24 x 72): `Delta(24 x 72)/Delta(12 x 36)` and `Delta(48 x 144)/Delta(24 x 72)` in `[0.15, 0.40]` | `0.25` |
| P5 | H1 | CST ALT: `Delta_ALT / Delta_b` in `[0.5, 1.5]` | `~ 1` |
| P6 | H1 | OA at the base: stabilisation `0`, drilling `1e-4` and `1e-2`: `|f_T/f_T(default) - 1| <= 5e-3` each | `< 1e-3` |
| P7 | H1 | energy of the CST torsion mode: membrane share in `[0.65, 0.90]`, drilling share `<= 2e-2` | `0.78`; small |
| Q2 | H2 | ANDES AC `Delta_ANDES > 0.5 Delta_b` and `Delta_ALT/Delta_b` outside `[0.5, 1.5]` | — |
| Q3 | H3 | twist 0 `|Delta| > 1e-2`, or OA stabilisation `0` shifts the base `f_T` by `> 1e-2`, or the twist-0 OA stabilisation `0` shift `> 1e-2` | — |
| Q4 | H4 | OA drilling `1e-4` or `1e-2` shifts the base `f_T` by `> 1e-2`, or the drilling share of P7 `> 2e-2` | — |

What each outcome means: **H1** (P1 to P7 hold; Q2 to Q4 do not): the excess is the constant-strain membrane's parasitic shear
on the chordwise-varying axial strain that the warped geometry couples to the torsion; the location is the Shell3 CST membrane
acting through the membrane-bending coupling of the warped facets, and the remedy is the ANDES membrane (F1). **H2**: the flat
facet geometry of the split pattern; a remedy would need curved or averaged facets (a follow-up). **H3**: the bending or the
MITC3 shear of the triangle, present on flat strips; a remedy in the shear (follow-up). **H4**: the drilling penalty on warped
facets; a remedy by the drilling treatment (ANDES has none). Mixed outcomes: inconclusive, recorded with the numbers.

## Negative controls and mutations

| id | control | expected |
|---|---|---|
| N1 | base Shell3 CST: the second-largest `tau` among the six modes | `< 0.5` (the identification discriminates) |
| N2 | energy partition with the membrane part left out against the whole | `|sum/whole - 1| > 0.5` |
| N3 | Hex20 torsion twist 30 against twist 0, ratio against the closed-form ratio 2.074 | ratio `> 1.5` and within `1e-1` of 2.074 (the pretwist stiffening is resolved by the reference) |

Mutations (scratch copy `f556-scratch/mut`, one at a time, the named test run): MU1 `tau` with the sum `w_L + w_T` in place of
the difference (C2); MU2 the OA default with stabilisation `0` (C4); MU3 the energy partition without the drilling part (C5);
MU4 the BD split as `[a,b,d], [a,c,d]` (C3); MU5 the twist in degrees taken as radians (C1); MU6 the library CST membrane shear
row with `0.5 nx` (B1, C1); MU7 the library `ANDES_ALPHA_B` 1.5 to 1.4 (B1, F1); MU8 the library `SHEAR_STABILISATION` 0.1 to
0.2 (B1, C1).

## Decision rule

- **done:** C1 to C7, F1, B1 and N1 to N3 met, and one hypothesis conclusive (all its predictions held, those of the others not)
  among H1, H2, H4 (the membrane-bending coupling on warped triangles named by the acceptance), or H3 with its excess shown to
  arise only through the twisted geometry; the remedy shown opt-in (F1) or proposed as a follow-up.
- **partial:** the hard checks met except possibly F1 and the diagnosis inconclusive or located outside the parts the acceptance
  names (H3 on flat strips); or a hard check missed with the diagnosis made.
- **todo:** the diagnosis runs not made.

## Limits (not covered)

- The first torsion mode only (the other tracks of ADR 0473 E2 agree within 3.8e-3); one material; a linear twist.
- The Hex20 reference with one element through the thickness (C6 bounds its in-plane mesh error, not the through-thickness one).
- No library change: a remedy other than the existing ANDES membrane is a follow-up.
- No Shell3 default changes (a change of default is a maintainer decision that would move results and hashes).

## Addendum 1: results of the first run (2026-11-14)

Run once at 723a5e19, release, `cargo test -p rcfd-fem --release --test sa_shell3_twist_diagnosis -- --nocapture
--test-threads=3` (log `f556-scratch/run1.log`, 15 s); B1 recorded at dcf83a9f on the base (`f556-scratch/b1-base.log`).
`d2_refinement` and `f1_andes_shell3_twisted_strip_campbell` passed; `d1_variation_matrix_base_twist_thickness_energy` failed on
four checks (below).

| check | measured | limit | |
|---|---|---|---|
| C1 Hex20, Shell4, Shell3 torsion against the F-420 log | 8.3e-10, 4.4e-10, 4.6e-10 | `1e-7` | met |
| C1b `delta_3`, `delta_4` | 4.049873e-2 (2.7e-7 off), 5.339560e-3 (4.0e-8 off) | `1e-5` | met |
| C2 `tau` of the chosen mode, every run | lowest 0.98803 | `>= 0.8` | met |
| C2b third mode at the base (Hex20, Shell4, Shell3) | 2, 2, 2 | each | met |
| C3 AC against BD, six frequencies | 3.69e-8 (modes 5 and 6; modes 1 to 4 equal to the printed 6 decimals) | `1e-8` | **missed** |
| C4 own assembly against the model | 0 (bit-equal frequencies) | `1e-8` | met |
| C5 sum of parts; whole against `lambda/2` (CST, ANDES, Shell4) | 1.5e-12, 1.9e-12, 2.8e-12; 4.9e-13, 3.2e-14, 3.0e-12 | `1e-11`; `1e-8` | met |
| C6 Hex20 24 x 72 against 12 x 36 | 1.17e-3 | `5e-3` | met |
| C7 Hex20 against the closed form: base, twist 15, 0.5 mm, 2 mm | 2.32e-2, 5.71e-2, 1.43e-3, 4.77e-2 | `6e-2` | met |
| C7 twist 0 | 9.29e-2 (228.084 against 208.690 Hz) | `6e-2` | **missed** |
| F1 ANDES Shell3 Campbell against Hex20, tracks 1 to 4, four speeds | 2.98e-3; no loss, lowest MAC 0.999708 | `4e-2`; `> 0.9` | met |
| N1 second-largest `tau` at the base | 0.98395 | `< 0.5` | **missed** |
| N2 without the membrane part (CST, ANDES, Shell4) | 0.751, 0.730, 0.732 | `> 0.5` | as expected |
| N3 Hex20 twist 30 / twist 0 against the closed-form ratio 2.0736 | 1.9414 (0.132 off) | `> 1.5`, within `0.1` | **missed** |

**N1: a defect of the identification rule (set-up).** Every torsion mode is antisymmetric at the tip, so the second torsion has
`tau ~ 1` as well; "the largest `tau`" does not separate the first torsion from the second. At the base and in every run of
the twisted strips (15 and 30 degrees, every thickness and mesh) the rule took the third mode, the first torsion (its frequency
continuous with the base: 442.80, 298.02, 393.77, 590.75 Hz for Hex20); at twist 0 it took the first torsion for Hex20 and Shell4
(index 1, 228.08 and 228.01 Hz) but the second torsion for Shell3 CST and ANDES and the own assembly (index 4, 716.06 Hz), so
P1 (twist 0, `Delta = 2.14`) and the twist-0 part of Q3 (`7.47e-3`) were measured on the wrong mode. Corrected by amendment 1.

**C3: the limit ignored the eigensolver tolerance.** The two meshes are congruent (arithmetic 4); the first four frequencies
agree to the printed 6 decimals (the torsion 460.730207 Hz on both), the fifth and sixth differ by 3.7e-8 and 1.8e-8. The
pairs are converged to the backward error `1e-10`, which for the higher pairs allows an eigenvalue difference of order
`1e-10 ||K|| / (lambda ||M||)`, not rounding. Kept as a known miss (`c3_ac_bd_congruence_known_miss`), the limit intact.

**C7 at twist 0 and N3: the closed form neglects the root restraint.** The clamped root sets `phi'(0) = 0` as well as `phi(0)
= 0` (the slope `w_r = phi' s` vanishes along the clamp): a plate strip has the warping-like stiffness `D c^3/12` of the
chordwise-linear `w_rr`, with the decay length `sqrt(D c^3/(12 G J)) = 0.128 m`, comparable with `L = 0.15`. At twist 0 this
raises the torsion by 9.3 percent over the free-warping closed form (Shell4 228.010 Hz agrees with Hex20 228.084 Hz within
3.2e-4, so the reference is not in question); at 30 degrees the pretwist term dominates and the effect is 2.3 percent. The
registration estimated the base only. Kept as a known miss (`c7_n3_twist0_closed_form_known_miss`), the limits intact.

**The diagnostic predictions of the first run** (P1 and the twist-0 Q3 on the wrong mode; the others on the first torsion):

| id | measured | range | |
|---|---|---|---|
| P2 ANDES `|Delta|/Delta_b` (`Delta_b = 3.5159e-2`) | 0.161 | `[0, 0.25]` | holds |
| P3 twist 15, 0.5 mm, 2 mm: `Delta/((1/2) m e_m)` (`e_m = 9.163e-2`) | 0.927, 1.044, 0.969 | `[0.6, 1.4]` | holds |
| P4 `Delta(24 x 72)/Delta(12 x 36)`, `Delta(48 x 144)/Delta(24 x 72)` | 0.257, 0.264 | `[0.15, 0.40]` | holds |
| P5 ALT `Delta/Delta_b` | 0.652 | `[0.5, 1.5]` | holds |
| P6 OA stabilisation 0, drilling `1e-4`, `1e-2` | 1.94e-3, 2.76e-4, 6.44e-5 | `<= 5e-3` | holds |
| P7 CST membrane share; drilling share | 0.7508; 8.1e-5 | `[0.65, 0.90]`; `<= 2e-2` | holds |
| Q2; Q4 | false; false | — | not indicated |

## Amendment 1 (2026-11-14, written and committed before the corrected run)

1. **The first torsion** of a run is the lowest-frequency mode of the six with `tau >= 0.8` (the registered `tau` of D2
   unchanged). C2 becomes: such a mode exists in every run. C2b unchanged (index 2 at the base).
2. **N1 corrected:** every mode below the first torsion has `tau < 0.5`, in every run (the identification separates the
   torsion from the flap and edge modes below it). Its limit `0.5` unchanged.
3. The corrected test `d1b_variation_matrix_first_torsion` repeats the runs of `d1` (deterministic; the same bits) with the
   corrected identification and asserts C1, C1b, C2, C2b, C4, C5, C6 is in `d2`, C7 except at twist 0, N1 (corrected), N2; the
   predictions P1 to P7 and Q2 to Q4 are printed and classified with the registered ranges. C3, C7 at twist 0 and N3 are
   printed, not asserted, and asserted with their limits in the ignored known-miss tests. `d1` becomes an ignored record of the
   first run. `d2` and F1 are not re-run: their choices were the third mode in every run, the first torsion (addendum 1).
4. No limit, range, mesh or option changes.
