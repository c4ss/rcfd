# ADR 0093: Wave 39B, partials close-out for F-233, F-211 and F-131

Status: registered before the registered tests were run. Limits are fixed; a miss becomes an `#[ignore]`d `*_known_miss` test
with the limit unchanged. No library code changes: every addition is test code, so defaults and the baseline hashes are
untouched. Clean-room: closed forms derived below from first principles; no literature value enters a gate.

The residual gaps are those stated in docs/audit/partials-group-1.md. I re-checked each claim against the code before writing
this ADR: `q2a_sliver_hexahedron` asserts only edge ratio, angles and inverse orthogonal quality; no tetrahedron test asserts the
four pre-existing measures; `o1c_known_miss_compressive_convergence` is the only test that measures compressive(1.5) and
compressive(2); `p1_pipeline_cylinder_in_box` builds its surface in code and only prints the tetrahedral quality.

## F-233: closed forms on the reference cells (test file crates/rcfd-mesh/tests/measures.rs)

Acceptance (unchanged): "All measures evaluated on a reference tetrahedron, hexahedron and a degenerate cell match closed-form
values". "All measures" is read as the seven ADR 0018 measures (warp, face angle min and max, ortho-skew, inverse orthogonal
quality, size change, face edge-length ratio) plus the four pre-existing ones of `check_mesh` (maximum and mean
non-orthogonality, maximum skewness, maximum aspect ratio, maximum volume ratio). Tolerances are the ADR 0018 ones: 1e-12
absolute for dimensionless values, 1e-9 degrees for angles, 1e-9 relative for the ratio 1000.

Closed forms for the pre-existing measures. Every cell below is a single cell, so every face is a boundary face, the
non-orthogonality of a face is the angle between `d = face centre - cell centre` and the face normal, the skewness is
`|face centre - (cell centre + d (d.n)/(d.n))| / |d| = 0` and the volume ratio is 1 (no internal face; the report starts at 1).

* Unit cube (q1a, n = 1 and n = 2, the latter with internal faces between equal cubes): non-orthogonality max and mean 0,
  skewness 0, aspect ratio 1, volume ratio 1.
* Right-corner tetrahedron (q1b), centre (1/4, 1/4, 1/4). Each of the three coordinate faces has centre e.g. (0, 1/3, 1/3),
  `d = (-1/4, 1/12, 1/12)`, normal (-1, 0, 0): angle `atan2(sqrt(2)/12, 1/4) = atan(sqrt(2)/3) = 25.2394 deg`. The slanted face
  has `d = (1/12, 1/12, 1/12)` parallel to its normal: 0. Max `atan(sqrt(2)/3)`, mean `3 atan(sqrt(2)/3) / 4`. Skewness 0,
  aspect ratio sqrt(2) (edges 1 and sqrt(2)), volume ratio 1.
* Regular tetrahedron (q1c): by symmetry every `d` is along the face normal: non-orthogonality max and mean 0, skewness 0,
  aspect ratio 1, volume ratio 1. Also the missing ADR 0018 measure on this cell: size change 1 (no neighbour).
* Sliver hexahedron 1 x 1 x 0.001 (q2a): all faces planar rectangles, so warp 0 on every face and in the cell; boundary
  `d` along each normal, so ortho-skew 0; size change 1 (no neighbour); non-orthogonality max and mean 0; skewness 0; aspect
  ratio 1000 (1e-9 relative); volume ratio 1.

Smoke: the cells are the ones the existing passing tests q1a, q1b, q1c and q2a already construct; no new set-up.

## F-211: order of compressive(1.5) and compressive(2) on the limit cycle (crates/rcfd-fv/tests/schemes_wave13.rs)

The O1c record shows that the deferred correction of these two schemes ends in a limit cycle (per-sweep change stalls at 7e-8 /
1.4e-8 at n = 256, identical after 600 and 60000 sweeps), so a convergence rule "change below 1e-13 of the range" can never be
met. The existing record stays ignored and unchanged. New test O1d `o1d_compressive_order_on_limit_cycle`, same corrected
set-up as O1c (u = 1, phi = 1 + sin(pi x / 2), Dirichlet inlet, zero-gradient outlet, n = 64, 128, 256), 20000 sweeps per run
(the O1c budget), the L2 error recorded after every sweep. Registered rules:

1. Stationarity: the [min, max] error intervals over the 200 sweeps ending at sweep 10000 and at sweep 20000 overlap, for each
   scheme and n. This separates a limit cycle from a slow drift.
2. Order: with `[lo_n, hi_n]` the final-window interval, both worst-case orders `log2(lo_n / hi_2n)` and `log2(hi_n / lo_2n)`
   lie in the registered O1 band for second-order schemes, [1.85, 2.3], for both refinement pairs. Every member of the cycle
   therefore gives the nominal order.
3. The run must not stop early by meeting the O1c convergence rule (the trace must have 20000 entries); if it did, the
   limit-cycle rule would not apply and the miss would be a set-up defect.

Smoke (ignored `o1d_smoke`, n = 64 and 128, 2000 sweeps, windows ending at 1000 and 2000): the windows are identical at both
budgets; compressive(1.5) error 1.5574e-4..1.5579e-4 (n = 64) and 3.9441e-5..3.9444e-5 (n = 128); compressive(2) 1.5576e-4..
1.5577e-4 and 3.9442e-5..3.9443e-5; 0.4 to 0.7 s per run. What I knew when registering: those smoke numbers (worst-case order
about 1.98 for the 64/128 pair) and the O1c record orders 1.98 / 1.99.

With O1d passing, all three clauses of the F-211 acceptance are met by passing non-ignored tests (B1, O1c plus O1d, W1b).

## F-131: closed STL file through the cylinder-in-box pipeline (new file crates/rcfd-io/tests/stl_cylinder_in_box.rs)

The STL reader and writer live in rcfd-io (ADR 0011), so the test lives there. Geometry: the `cyl_in_box` surface of
crates/rcfd-mesh/tests/tetmesh.rs (box [-2,2] x [-2,2] x [-1,1], inward cylinder r = 0.5, h = 1). It is written as two ASCII
solids (`box`, `cylinder`) in one file under `CARGO_TARGET_TMPDIR`, read back with `stl::read_file`, each solid welded with
`TriSurface::from_soup(.., 0.0)` (bitwise), the cylinder tagged 1. Then the p1 pipeline unchanged: curvature remesh
(theta 0.5, h_min 0.1, h_max 1.0, default options), `tetrahedralize` with default options. Registered checks:

* S1 (closed STL): two solids; closed manifold; Euler characteristic 4 in 2 components; area and enclosed volume equal to
  the in-code surface within 1e-12 relative; tag-1 triangle count equals the cylinder's.
* S2 (watertightness and solver quality): remeshed surface closed manifold; volume mesh volume equals the remeshed surface
  volume within 1e-10 relative (the ADR 0030 C limit); 2 patches; every cell volume positive; `check_mesh(..).is_ok()`;
  minimum dihedral angle at least 3.0 degrees (the ADR 0030 T5 limit, set in wave 19B before this case was measured).
* S3 (tetrahedral shape quality): fraction of cells with radius-edge ratio at most 2 at least 0.99 (the ADR 0030 T5 limit).

The acceptance "passes watertightness and quality checks" is read strictly: "quality checks" means S2 and S3, i.e. the
project's mesh check plus the full ADR 0030 T5 tetrahedral quality set. No new limit is invented for this case.

What I knew when registering: wave 19B measured on the in-code surface (audit): min dihedral 3.34 deg, max 174.0 deg,
83 bad tetrahedra, radius-edge fraction 0.927. S2's dihedral limit therefore has a small margin and S3 is expected to miss.
If S3 misses, F-131 stays partial; meeting it needs quality refinement of the tetrahedral mesher (F-132 scope), which this
item does not attempt.

Smoke (ignored `s0_smoke`, construction and read-back only): 148 points, 288 triangles, Euler 4, 2 components, no free,
non-manifold or misoriented edges, enclosed volume 31.23463313526981 vs 31.23463313526989 in code (2.6e-15 relative).

## Amendment (written after the registered F-131 run, before the run it governs)

S2 missed on its last assertion (minimum dihedral 1.84 deg vs 3.0); every S2 assertion before it passed in that run. S2 becomes
the ignored `s2_known_miss_pipeline_watertight_and_quality` with its limit unchanged. I add `s2b_pipeline_watertight_and_check_mesh`,
which repeats the S2 assertions without the dihedral limit, so the watertightness and `check_mesh` results stay a running
check. It is not a registered test of its own and does not change the F-131 verdict. What I knew when writing this: the full
registered results below.

## Results (release build, HEAD f002a3c plus the test files)

### F-233: pass

`measures`: 17 passed, 1 ignored (the old q2d record). q1a, q1b, q1c and q2a pass with the added assertions at the registered
tolerances: unit cube non-orthogonality 0, skewness 0, aspect ratio 1, volume ratio 1; right-corner tetrahedron maximum
non-orthogonality atan(sqrt(2)/3) = 25.2394 deg and mean 0.75 of it, aspect ratio sqrt(2); regular tetrahedron size change 1
and the four pre-existing measures; sliver hexahedron warp 0 on the cell and every face, ortho-skew 0, size change 1,
non-orthogonality 0, skewness 0, aspect ratio 1000, volume ratio 1. Every ADR 0018 measure and every pre-existing measure is
now asserted on a tetrahedron, a hexahedron and the degenerate cell.

### F-211: pass

`o1d_compressive_order_on_limit_cycle` passes (37 s for both schemes). The runs did not converge in the O1c sense (20000-entry
traces). The windows at sweeps 10000 and 20000 are identical apart from the last digits (compressive(1.5) n = 256: lower bound
9.922224969034015e-6 vs 9.922224969033713e-6), so the cycle is stationary. Error intervals: compressive(1.5) 1.55737e-4..
1.55789e-4, 3.94408e-5..3.94440e-5, 9.92222e-6..9.92243e-6; compressive(2) 1.55762e-4..1.55772e-4, 3.94424e-5..3.94430e-5,
9.92232e-6..9.92236e-6. Worst-case orders: compressive(1.5) 1.9812..1.9818 (64/128) and 1.9909..1.9911 (128/256);
compressive(2) 1.9815..1.9816 and 1.9910..1.9910. Limit band [1.85, 2.3]: met.

### F-131: S1 pass, S2 miss, S3 miss

* S1 passes (closed manifold, Euler 4, 2 components, area and volume within 1e-12).
* S2: closed manifold remeshed surface (372 triangles), 1115 cells, volume error within 1e-10, 2 patches, positive volumes,
  `check_mesh` without errors: all pass. Minimum dihedral 1.84 deg vs the registered 3.0: MISS. I re-ran
  `p1_pipeline_cylinder_in_box` (in-code surface, same options) after the gate: 1061 cells, minimum dihedral 3.3395 deg,
  maximum 174.02 deg, radius-edge fraction 0.9274, 83 bad left after 4 rounds, so the wave 19B record still holds. The STL
  surface has the same coordinates (bit exact ASCII round trip) but a different vertex numbering from the welding. The
  difference is consistent with the remesh and the refinement depending on the numbering, but I did not isolate that cause
  and did not try other numberings: that would be picking a favourable set-up. Note that even p1's 3.34 deg passes the
  3.0 limit only narrowly.
* S3: radius-edge fraction 0.936 vs 0.99: MISS (as expected; p1 gave 0.927). Maximum dihedral 176.8 deg, maximum radius-edge
  27.9, 68 bad tetrahedra left after 3 refinement rounds.

`s2b` is a post-hoc subset of S2 (see the amendment) and does not count toward the verdict. Test-count delta of this ADR:
+3 passed (O1d, S1, S2b), +4 ignored (O1d smoke, S0 smoke, S2 and S3 known misses). Gate on a fresh archive of 828d1aa:
fmt and clippy clean, 1914 passed / 0 failed / 540 ignored.

F-131 stays partial: the STL clause and watertightness are met; the tetrahedral quality is not. Closing it needs sliver removal
and quality refinement in the tetrahedral mesher (F-132 scope), not more test work.
