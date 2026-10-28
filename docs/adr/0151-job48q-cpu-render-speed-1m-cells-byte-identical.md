# ADR 0151: Job 48Q, a faster CPU contour render at 1M cells, with byte-identical images

Status: **registered** 2026-10-28, in two parts.
* Part 1 (this section and the next two): the control and profile set-up and the byte-identity checks. Registered before any
  run of this job.
* Part 2: the timing limits, registered after the control runs and before any after run. The maintainer's brief orders
  "profile, register the limit, then optimise".
* Results are in the addendum: every registered limit is met, with no miss and no amendment. Not yet accepted by the
  maintainer.

House rules of [ADR 0130](0130-wave48a-overview-and-run-record-contract.md):
* limits are fixed before the registered runs;
* a missed limit becomes an ignored `*_known_miss` test with the limit intact;
* a defective set-up gets an amendment before the corrected run;
* defaults and outputs stay bit-identical. Here that means the rendered pixels and the PNG bytes of every existing render
  test;
* `crates/rcfd-api/tests/baseline_hashes.rs` is not re-recorded.

No unsafe code, no new crate, and the crate graph stays acyclic.

## Problem

Full gate run 40745275594 (`windows-latest`, commit 7eb8b79, `scripts/gate_test.py` with `jobs=3`) failed two release timing
limits. Both pass on the maintainer's machine and passed in earlier gates.

| Test | CI | Limit | Earlier local figures |
|---|---|---|---|
| `rcfd-post` `render_pixels::r2_one_million_cells_render_in_under_two_seconds` | render 2.316 s; PNG encode 0.097 s (188138 bytes) | render < 2 s | render 0.903 s, encode 0.053 s (`docs/audit/partials-group-3.md`) |
| `rcfd-cli` `render_cli_w39a::r4_cli_renders_a_one_million_cell_contour_image_in_under_two_seconds` | `rcfd call render.png` 2.086 s. `run.initialize` 22.3 s, not limited | < 2 s | 1.253 s smoke, 1.555 s registered ([ADR 0092](0092-wave39a-partials-closeout-rules-fluent-render.md)) |

## The maintainer's decision (2026-10-28)

Investigate a real speedup of the 1M-cell CPU render path, both the `rcfd-post` render and the CLI call, so that it meets
2 s with margin on a slow, shared runner with 2-4 cores.
1. First profile on this machine (release), with a controlled measurement, where the time goes: rasterisation, colour
   mapping, allocation, threading, PNG encode and base64.
2. Register the limit, for example render <= 1.2 s on this machine as the margin proxy, with the existing 2 s limits
   unchanged.
3. Register the byte-identity check: pixel-exact on the existing r1 and r3 tests, and a hash of the 1M-cell image.
4. Then optimise. Candidates:
   * tiling or scanline rasterisation;
   * no per-cell allocation;
   * deterministic threads (`std::thread::scope`, no new crate);
   * a cached colour-map lookup table.

   A cheaper PNG compression level is excluded, because its bytes would change.
5. Report timing under limited cores too.
6. If no safe speedup meets the margin, say so with the measurements. No limit is loosened.

## Context (read at main 6867e40)

* **Render path.** `render_scalar` in `crates/rcfd-post/src/render.rs` runs on one thread, in these steps:
  1. `view`: the bounding box over all points (about 2.0M for the 1M-cell cavity), the view axis and the slice.
  2. `frame`, then `Image::new`: a white RGBA image, filled pixel by pixel.
  3. `cell_polygon` for every cell:
     * it pushes the projections of the points of all the cell's faces (24 for a hex) into a new `Vec`;
     * it takes the cell's extent along the view axis for the slice test;
     * `convex_hull` then sorts the 24 points (stable sort), runs `dedup_by` within 1e-14 px, and builds a monotone chain with
       two boxed iterators.

     The hulls are kept as 1M separate `Vec`s.
  4. The min and max of the finite values of the drawn cells, in cell order.
  5. For every drawn cell in cell order: `colormap`, then `fill_polygon`, which makes a new `Vec` of crossings per scanline.
     A later cell overwrites an earlier one on a shared pixel.
  6. The colour bar and its two labels.

  A 1M-cell render thus makes several heap allocations per cell and frees 1M hulls at the end.
* **CLI path (r4).** `rcfd call` is a new process. It connects to `rcfd serve`, whose `render.png` handler
  (`crates/rcfd-api/src/service.rs`) does the following:
  * computes the field, then calls `render_scalar`, `Image::to_png` and `base64`, and builds the JSON reply;
  * `serve_stream` writes the reply with `writeln!` straight to the `TcpStream`, unbuffered.

  The client reads the line, parses it and prints it.
* **PNG.** png 0.18 at its default settings. Its bytes depend on the compression level and the filters, so the encoding
  stays as it is.
* **Colour map.** `colormap` is a continuous function of `t`: 9 stops, piecewise linear, rounded. A lookup table indexed by
  a quantised `t` would change pixels, so that candidate is out unless it is exact.
* **Signed zeros in the folds.** `f64::min` and `f64::max` may return either zero on a tie, and `label(vmin)` prints
  `-0.000` for -0. Any fold whose result reaches the image therefore keeps its order:
  * the bounding box;
  * the value range;
  * a polygon's y extent.
* **Gate.** `scripts/gate_test.py` runs one process per test, 3 at once (`RUST_TEST_THREADS=3`,
  [ADR 0127](0127-test-gate-efficiency.md)). The runner's core count is not in the run's log (UNVERIFIED). GitHub documents
  2 vCPUs for a private repository's standard Windows runner (UNVERIFIED here).
* **Thread count on Windows.** In std 1.99, `available_parallelism` on Windows is
  `GetSystemInfo().dwNumberOfProcessors`; it ignores the process affinity mask (read from the toolchain's
  `library/std/src/sys/thread/windows.rs`). A process pinned to 2 logical CPUs here still sees 16.
* **This host.**
  * AMD Ryzen 7 9800X3D: 8 cores, 16 logical CPUs. The SMT siblings are logical CPUs (2k, 2k+1), from
    `GetLogicalProcessorInformation`.
  * 93.6 GB of RAM; rustc 1.99.0.
  * Other agents share the host: the CPU was 28-40 % busy when this was written, with about 22 GB of RAM free.

## Registered set-up, part 1: control and profile runs

**Builds.**
* Release builds, in the worktree's own target folder.
* The control binaries were built at 6867e40, before any change, and copied before any rebuild:
  * `rcfd.exe` (SHA-256 starting `20B2DFE4`);
  * the `render_pixels` test binary (`4A026728`);
  * the `render_cli_w39a` test binary (`46B0AC40`).
* The control `render_cli_w39a` reaches `rcfd.exe` through an absolute path fixed at compile time. Its runs therefore happen
  before any rebuild of `rcfd.exe`.
* The new CLI harness takes each `rcfd.exe` path as a parameter.

**Conditions.** Each run records its start time, the CPU load over the 2 s before it, and the free RAM.

| Id | Affinity mask | Other load | Models |
|---|---|---|---|
| U | none | none from this job | this machine, as for the earlier local figures |
| S2 | 0xC000 (logical 14 and 15: one core, two SMT threads) | none from this job | a 2-vCPU VM whose vCPUs are SMT siblings |
| P2 | 0x5000 (logical 12 and 14: two cores) | none from this job | a 2-vCPU VM on separate cores |
| S2L | 0xC000 | two busy loops (`python -c "while True: pass"`), each pinned to 0xC000 | the gate's 3 concurrent test processes on a 2-vCPU runner |

* The mask is set at process creation, with `cmd /c start "" /affinity <mask> /b /wait <exe> ...`. Children inherit it.
  Probe: a Python process started this way, and its child, both showed 0xC000.
* The busy loops are stopped in a `finally`.

**Control runs, on the base code (6867e40).**
* **C-r2.** The control `render_pixels` binary, `r2_one_million_cells_render_in_under_two_seconds --exact --test-threads 1
  --nocapture`. 5 rounds; each round runs U, S2, P2 and S2L, in that order. Recorded: the render and encode times.
* **C-r4.** The control `render_cli_w39a` binary, r4 with `--exact --test-threads 1 --nocapture`. 3 rounds over U, S2, P2
  and S2L. Each r4 run takes about 20 s and up to a few GB, mostly the session's initialisation. Recorded: the
  `render.png` call time.
* **C-profile (Q0).** `crates/rcfd-post/tests/render_speed_w48q.rs`, `q0_profile_control_phases`, ignored and run by hand.
  * It uses the base renderer's verbatim copy (below) with phase marks: timer calls between the phases, with no change to the
    arithmetic.
  * Workload: r2's mesh, field and options. 5 repetitions in one process, under U and under S2L.
  * Phases timed:
    * `view`;
    * `frame` plus the image;
    * the cell polygons (projection, hull, the 1M `Vec`s);
    * the value range;
    * colour plus fill;
    * the colour bar;
    * freeing the hulls;
    * the PNG encode.
  * Probes on the same data, outside the control's code path:
    * the polygon phase with reused buffers (the allocation share);
    * the polygon phase with the cell's point indices deduplicated before the sort (the sort share);
    * the fill with a reused crossing buffer;
    * `colormap` alone.

  Each probe's hulls are checked equal to the control's.
* **C-cli (Q5).** `crates/rcfd-cli/tests/render_cli_speed_w48q.rs`, `q5_cli_render_timing`, ignored and run by hand.
  * One `rcfd serve` per `rcfd.exe` listed in `RCFD_W48Q_EXES` (default: the build's own), each with r4's session, created
    and initialised.
  * Then 7 rounds, alternating between the binaries:
    * `rcfd call service.info`: the process and connection floor;
    * `rcfd call render.png`, with r4's parameters.

    The first `render.png` call after the initialisation is reported apart, as r4 times it.
  * The stdout of `render.png` must be byte-identical across the binaries.
* **C-cli in process (Q6).** In the same file, `q6_cli_handler_breakdown`, ignored. r4's session in an in-process
  `Service`; `call("render.png")` is timed, and so are `base64` and the serialisation of the reply's JSON line. 5
  repetitions, condition U.

Medians are reported, together with the minimum and the maximum.

## Registered byte-identity checks (exact limits)

The oracle is the base code. `crates/rcfd-post/tests/render_control_w48q/mod.rs` is a verbatim copy of the base
`render_scalar` and the private functions it calls. Only these changes are made:
* `Image::new` and `Image::put` become free functions;
* phase marks are added;
* the hulls are dropped explicitly before the return.

| Id | Check | Limit |
|---|---|---|
| Q1 | `render_speed_w48q::q1_matches_the_base_renderer_byte_for_byte`: the product `render_scalar` against the copy, over the matrix below. It runs in debug and release, and in the gate | Equal in every case: RGBA bytes, width, height, the bits of `vmin` and `vmax`, `cells_drawn` and the PNG bytes |
| Q2 | `q2_r1_and_r3_images_are_as_before`: r1's and r3's images, each from its own set-up. It runs in the gate | The SHA-256 of the RGBA bytes and of the PNG bytes equal the values recorded from the base code |
| Q3 | `q3_one_million_cell_image_is_as_before`: the 1M-cell image of r2. Ignored, release only, run by hand: the mesh takes 2.7 GB, too much to add to the gate next to r2 | The RGBA and PNG SHA-256, `vmin`, `vmax` and `cells_drawn` equal the base values, and the image equals the copy's, rendered in the same process |

**Matrix of Q1.** Cell values are pseudo-random unless stated, so that every cell has its own colour and an order error shows.

| # | Mesh | Options and values | What it exercises |
|---|---|---|---|
| 1 | `cavity_2d(8)` | default options (colour bar) | the colour bar and labels |
| 2 | `cavity_2d_rect(32, 32)`, width 64, no colour bar | | cell edges at 8 + 1.5 i px. Every other edge passes through pixel centres, which are drawn by both cells, so cell order decides |
| 3 | `cavity_2d_rect(3, 5, 1, 2)`, width 2048 | | coarse cells spanning hundreds of rows each |
| 4 | `warp` of `cavity_2d_rect(17, 13)` by a smooth non-affine map | width 777, height 500 | skewed quads; crossings on sloped edges; explicit height |
| 5 | `cylinder_ogrid(24, 8, 0.5, 2.0)` | width 513 | curved cells around a hole: the background shows |
| 6 | `backward_step` | width 300 | a non-rectangular domain |
| 7 | `box_mesh` 4 x 5 x 6, graded | axis 0, 1 and 2; a slice inside a layer, a slice on a layer boundary (two layers pass the slice test, so order decides), and a slice outside (nothing drawn, range (0, 1)) | 3D slices |
| 8 | the 2D cavity of #2 | values with NaN, +inf and -inf | non-finite values and the range fold |
| 9 | the same | `range` given: increasing; equal ends; reversed | the range options |
| 10 | the same | all values equal | `span = 1` |
| 11 | the same | width 10 (clamped to 64), height 5 (clamped to 32) | the clamps |
| 12 | `box_mesh` 3 x 3 x 3 with z length 1e-20, viewed along x | the extreme values on these cells | degenerate hulls (fewer than 3 points after the dedup): not filled, yet counted in `cells_drawn` and in the range |
| 13 | `annulus_3d`, default axis and slice | | a curved 3D mesh |
| 14 | `cavity_2d_rect(256, 256)`, r2's field, width 1024 (release; also in debug) | | a medium mesh at r2's width |

**Order of work.**
1. This registration.
2. The tests: the copy, Q0-Q6, and the recorded base hashes of Q2 and Q3. They use only APIs that exist at 6867e40, and
   they are run on the base code before `render.rs` is changed. Q1 then proves that the copy matches the product on the
   matrix.
3. The control runs.
4. Part 2.
5. The change.
6. The after runs.

The constants of Q2 and Q3 are taken from the base product, not from the copy.

## What I already knew when registering

* The code reading in Context, the gate log of run 40745275594, and the earlier local figures quoted in Problem.
* No measurement of this job has been made. The only probe run is the Python affinity probe in the set-up, which is not a
  check.

## Part 2 (registered 2026-10-28, after the control runs and before any after run)

### Control results (base code 6867e40)

**Tests on the base code (commit 822c30c).**
* Q1: 24 renders compared, 0 differ. The copy matches the product on the whole matrix.
* Q1's negative control: with the cell order reversed, the image differs in all 4 order-sensitive cases (763, 184320,
  221184 and 177024 pixels).
* Q2 and Q3: their constants were recorded from the base product. Q3 also passed against the copy. The 1M-cell image is
  1024 x 954, its PNG 188138 bytes, the same size as on CI.

**C-r2**, 5 rounds (22:02-22:08). The host load before the runs was 20-72 %.

| Condition | r2 render, median (min-max) | PNG encode, median | r2 failed |
|---|---|---|---|
| U | 0.729 s (0.661-0.745) | 0.041 s | 0 of 5 |
| S2 | 0.614 s (0.595-0.698) | 0.039 s | 0 of 5 |
| P2 | 0.733 s (0.712-0.812) | 0.040 s | 0 of 5 |
| S2L | 2.179 s (1.111-2.347) | 0.093 s | 4 of 5 |

**C-r4**, 3 rounds (22:08-22:13).

| Condition | `rcfd call render.png`, median (min-max) | `run.initialize`, median | r4 failed |
|---|---|---|---|
| U | 0.850 s (0.784-1.005) | 12.2 s | 0 of 3 |
| S2 | 0.776 s (0.722-0.842) | 12.0 s | 0 of 3 |
| P2 | 0.752 s (0.695-0.831) | 11.7 s | 0 of 3 |
| S2L | 2.502 s (2.290-2.527) | 30.3 s | 3 of 3 |

**S2L reproduces both CI failures.**
* r2's render: 2.179 s here, 2.316 s on CI.
* r4's call: 2.502 s here, 2.086 s on CI.

Its contention is not steady, though: one r2 round gave 1.111 s, and Q0 under S2L (below) rendered the base in 1.03 s.
Whole r2 processes took 23-30 s under S2L, against 10-12 s under U.

**C-profile (Q0)**, r2's workload, 5 repetitions per condition.
* The host was 64 % (U) and 100 % (S2L) busy at the start. The absolute times are therefore inflated: the base render took
  1.6 s under U here, against 0.729 s in C-r2.
* What counts is the shares and the probe deltas.

| Phase of the base code | U, median | share of the render | S2L, median |
|---|---|---|---|
| `view` (bounding box) | 2.6 ms | 0.2 % | 3.2 ms |
| `frame` and the image | 1.6 ms | 0.1 % | 1.4 ms |
| cell polygons | 1156.8 ms | 72 % | 815.4 ms |
| value range | 1.7 ms | 0.1 % | 1.7 ms |
| colour and fill | 219.2 ms | 14 % | 142.2 ms |
| colour bar | 0.0 ms | 0 % | 0.0 ms |
| freeing the 1M hulls | 144.3 ms | 9 % | 69.8 ms |
| render total | 1615.6 ms | | 1029.3 ms |
| PNG encode (outside r2's render time) | 67.6 ms | | 51.5 ms |

The probes ran in the same process, on the same data. Every probe's hulls equal the base's bit for bit, and probe C's image
equals the base fill.

| Probe | U | S2L | Reading |
|---|---|---|---|
| E: the base polygons alone | 1118.5 ms | 784.5 ms | |
| A: E with reused buffers | 460.1 ms | 315.1 ms | per-cell allocation is about 59 % of the polygon phase |
| B: A with each cell's point indices deduplicated before the sort | 226.1 ms | 157.0 ms | the 24-point sort is about half of what A leaves |
| G: the base colour and fill | 195.3 ms | 135.6 ms | |
| C: G with a reused crossing buffer | 53.0 ms | 45.5 ms | per-scanline allocation is about 73 % of the fill |
| D: `colormap` alone | 17.2 ms | 17.1 ms | colour mapping is about 1 % of the render |
| F: freeing E's hulls | 150.2 ms | 69.4 ms | |

**C-cli (Q5)**, under U at 22:14, load 32 %, with the control `rcfd.exe`.
* The first `render.png` took 1.201 s.
* Over 7 rounds, `render.png` had a median of 1.460 s (1.227-1.764), and `service.info` 0.022 s (0.020-0.144).
* The stdout, 359859 bytes, was identical in all 8 calls.

**Q6**, in process, under U at 100 % load. Medians:

| Step | Median (range) |
|---|---|
| `render.png` handler | 1000 ms (845-1391) |
| the reply line serialised | 0.23 ms |
| `base64` of the 269795-byte PNG | 0.20 ms |
| transport, as `serve_stream` writes (`writeln!` unbuffered) | 1.63 ms |
| transport, one `write_all` | 0.55 ms |
| client parse | 0.11 ms |
| client clone and print text | 0.26 ms |

### Where the time goes

1. **The render is the time**, in r2 and in r4: the polygons 72 %, colour and fill 14 %, freeing 9 %, everything else
   under 1 %.
2. **Allocation is the largest single cost**, about 58 % of the base render in all:
   * per-cell `Vec`s in the polygons, about 0.66 s of 1.12 s;
   * per-scanline `Vec`s in the fill, about 0.14 s of 0.20 s;
   * freeing the 1M hulls, about 0.14 s.
3. **The 24-point sort** of each hex is about half of the polygon phase once allocation is gone.
4. **Colour mapping** is about 1 %. A lookup table would gain nothing, and an exact one is impossible for a continuous `t`
   (Context). Not done.
5. **PNG encode**: 40-70 ms under U and about 0.1 s under S2L. Unchanged by design.
6. **The CLI's extras** each take under 2 ms: base64, serialisation, transport, the client's parse and print. Process start
   and connection take about 22 ms. `service.rs` and the CLI are therefore not changed.
7. **Threads.** The base uses one thread. The polygon and fill phases, about 95 % of the render, can be split by cells and by
   rows. `view`, the range and the colour bar stay serial: about 6 ms together.

### Design decisions

**D1. Polygons.**
* The new `cell_hulls` has reused scratch buffers. It deduplicates each cell's point indices before the projection and the
  sort.
* `map_point`, the sort, the dedup and the monotone chain are those of the base. No floating-point expression changes.
* The hulls are kept in flat arrays per range of cells, together with the image rows each fill covers.

Why the result is identical:
* The cell's distinct points form the same set as all its faces' points, so the extent along the axis is the same min and
  max. A signed zero cannot change the slice test, `slice < zlo - 1e-12`.
* The sort's output is fixed: elements that `total_cmp` calls equal are bitwise equal.
* An exact copy of a finite point is always removed by the dedup, whether its first instance was kept or merged into the
  point before it.
* A copy of a non-finite point is not removed, because `NaN < 1e-14` is false. A cell with any non-finite projection
  therefore takes every face point, in the listed order, as the base did.

**D2. Fill.** The crossings of each scanline go into a reused buffer. The arithmetic is unchanged, and the writes are
clipped to the image as `put` clipped them.

**D3. Threads**, with `std::thread::scope`.
1. T threads find the hulls of contiguous ranges of cells (D1). The results are joined in cell order.
2. The value range is folded on one thread, in cell order, as before.
3. T threads fill bands of rows. Each band walks all drawn cells in cell order and draws their rows inside the band. A
   pixel therefore gets the colour of the last cell that covers it, as with one thread.
4. The colour bar is drawn after the bands.

**D4. Thread policy.** `RenderOptions` gets a new field `threads`, with default 0, meaning automatic.
* Automatic is `available_parallelism`, at most 8, and one thread below 16384 cells.
* Explicit values are used as given.
* Reasons:
  * the serial phases take about 6 ms;
  * at probe B's 0.2 µs a cell, 16384 cells take about 4 ms on one thread, a few times the cost of starting 8 threads;
  * on a 2-vCPU runner `available_parallelism` is 2 (UNVERIFIED, GitHub's documented size), so r2 and r4 would use 2
    threads there.

**D5. The image.** `Image::new` fills the image with `bg.repeat(w * h)`.

**D6. Not changed:**
* `colormap`, `view`, `frame`, the colour bar, the labels and `to_png`;
* `overlay_vectors`. It keeps the base `cell_polygon`, and it is not on the timed path (a question);
* `service.rs` and the CLI (finding 6).

### Registered limits, part 2

The conditions are those of part 1. All after runs are release builds of the change commit, and each run records its load and
RAM.

**Q4.** A new test, `render_speed_w48q::q4_margin_proxy_render_times`, ignored and run by hand.
* It renders r2's workload in one process. Each round renders four variants: the base copy, then the product with 1 thread,
  with 2 threads and with the default threads. The order rotates every round.
* 7 rounds. Every product image is checked equal to the base image.
* It runs under U and under S2L. The test reads the condition from `RCFD_W48Q_COND`.

| Id | Limit | Negative control |
|---|---|---|
| Q4-r1 (primary) | median of the product with 1 thread / median of the base copy <= **0.43**, under U and under S2L | the base copy against itself gives 1 |
| Q4-r2 | the same ratio with 2 threads <= 0.43, under U and S2L | as Q4-r1 |
| Q4-a1 | under S2L, the median of the product with 1 thread <= **1.0 s** | C-r2: the base under S2L, 2.179 s |
| Q4-a2 | under S2L, the median of the product with 2 threads <= 1.0 s | as Q4-a1 |
| Q4-v (validity of an S2L run) | the median of the base copy is >= 1.6 s in the same run | |

Why these numbers:
* 0.43: CI's 2.316 s x 0.43 = 1.0 s, half the 2 s limit, provided the speed-up carries over to the runner.
* 1.0 s under S2L: S2L stands for CI (2.179 s here, 2.316 s there), and 1.0 s is half the limit. It is stricter than the
  brief's example of 1.2 s.
* The brief's example under U is already met by the base (0.729 s), so it cannot tell the change apart. It is kept below as
  R2-u.
* The 1-thread ratio is the primary limit because it does not depend on how much contention a given S2L run delivers.

**Q5**, the existing harness, run under U and under S2L.
* `RCFD_W48Q_EXES` lists the control `rcfd.exe`, then the after `rcfd.exe`. 7 rounds.
* It asserts the limits below, reading the condition from `RCFD_W48Q_COND`.

| Id | Limit | Negative control |
|---|---|---|
| Q5-i | the stdout of every `render.png` call is identical across the binaries and the calls | |
| Q5-a | under S2L, the after binary's `render.png` median <= 1.0 s | the control's median in the same run |
| Q5-r | after median / control median <= 0.5, under U and S2L | the control against itself gives 1 |
| Q5-v (validity of an S2L run) | the control's `render.png` median is >= 1.6 s in the same run | |

Q5 under S2L is optimistic. The after server takes the automatic thread count: 8 here, 2 on a 2-vCPU runner. The figure for
r4 that is faithful to CI is therefore a reported estimate, not a limit:
* estimate = Q5's after median (S2L) - Q4's default-threads median (S2L) + Q4's 2-thread median (S2L);
* the PNG encode is the same in both, so it cancels.

**The existing tests**, after binaries:

| Id | Check | Limit | Negative control |
|---|---|---|---|
| R2 | r2 under U, S2, P2 and S2L, 5 rounds, alternating with the control `render_pixels` binary (ABAB) | every after run renders in < 2 s, the existing limit | the control's S2L runs fail (C-r2: 4 of 5) |
| R2-u | the brief's example: r2's median render under U | <= 1.2 s | met by the base (0.729 s), so not discriminating; reported |
| R4 | r4 under U, S2, P2 and S2L, 3 rounds | every run < 2 s | C-r4: 3 of 3 S2L runs fail |

r4 cannot alternate with its control at the process level: both test binaries reach `target\release\rcfd.exe` by a path
fixed at compile time. The CLI A/B is Q5.

**Byte identity.** Q1, Q2 and Q3 as registered in part 1, with Q1 extended:
* Every row is rendered by the product with the default threads and with 1, 2, 3, 5, 8 and 16 threads.
  * Row 3's 15 cells are fewer than 16 threads.
  * Row 14 runs every variant.
* The PNG bytes are compared once per row, for the default variant.
* **Row 15:** `warp` of `cavity_2d_rect(12, 12)` that sends points on x = 1 to x = NaN, and points on y = 1 to y = NaN. It
  reaches D1's non-finite path.
  * If the constructor refuses the mesh, the row reports that this path cannot be reached through the public constructors,
    and the row is dropped.
  * A probe without the fallback reports whether this row would tell the two paths apart. The limit does not depend on it.

**Re-runs.**
* `render_pixels` r1-r3;
* the `rcfd-post` unit tests;
* the `rcfd-api` service unit tests (`render.png`);
* `rcfd-mcp` `project_w48f` (`render_field`);
* `rcfd-render` `tests/render.rs`, the CPU reference (it may skip without a GPU);
* `rcfd-studio` `viewport_live`;
* `rcfd-gui` `viewport_raster`;
* `rcfd-cli` `workspace_graph`;
* fmt, and `clippy -D warnings` on the workspace.

**An S2L run that fails its validity check** (Q4-v or Q5-v) is void. An amendment records it, and the run is repeated at
most twice more. If none of the three attempts is valid, the S2L absolute limits (Q4-a1, Q4-a2 and Q5-a) are reported as
not established, neither passed nor missed, and the ratios stand.

**A missed limit.** The limit moves into its own ignored test, named `<name>_known_miss`, with the limit intact. The other
limits stay in Q4 or Q5.

### What I knew when registering part 2

* The control and profile results above.
* The change's code is drafted outside the tree. It has not been built or run, and no after run of any kind has been made.

## Addendum: results against the registered limits (2026-10-28)

**Verdict: every registered limit is met.**
* No miss, no `*_known_miss` test, no amendment.
* The image is byte-identical to the base on every check: Q1-Q3, and Q5's CLI stdout.
* The 1M-cell render on this host:
  * 0.73 s -> 0.05 s unrestricted (C-r2 against R2 U medians);
  * 2.18 s -> 0.21 s in the CI-like S2L condition.
* The single-thread speed-up alone is 5-6x (Q4-r1).
* The existing 2 s limits of r2 and r4 are unchanged and passed in every run, under every condition.

Commits:
* 005a68c: registration part 1.
* 822c30c: the tests on the base code, with no product code changed.
* bd79a74: registration part 2.
* dbb293f: the change (`crates/rcfd-post/src/render.rs`), the Q1 thread variants and row 15, Q4 and the Q5 limits. All after
  runs used its build.
* This addendum.

Set-up:
* This host: 16 logical CPUs, 93.6 GB, rustc 1.99.0. Release builds in the worktree's own target folder.
* Before any rebuild, the base and change binaries were copied to a scratch folder, with their SHA-256:
  * base `rcfd.exe` 20B2DFE4;
  * change `rcfd.exe` A6A077BE.
* No new crate, `Cargo.lock` unchanged, no unsafe code. No floating-point expression changed (D1, D2).
* Other agents' builds and solver runs shared the host throughout. The CPU was 20-100 % busy before the runs; each run's
  figure is in the run records.

### Byte identity

| Id | Limit | Result |
|---|---|---|
| Q1 | every row equal to the base copy: RGBA, size, `vmin`/`vmax` bits, `cells_drawn`; PNG once per row | **pass**: 175 renders (25 rows x 7 thread settings), 0 differ, in release and in debug |
| Q1 negative control | reversed cell order differs where order decides | 763, 184320, 221184 and 177024 pixels: as required |
| Q1 row 15 | built, or reported unreachable | built: `warp` accepts NaN points. 23 cells have NaN points, all 144 cells are drawn. The probe without the non-finite path gives 0 differing pixels, so this row does not tell the two paths apart (reported, as registered) |
| Q2 | r1's and r3's RGBA and PNG SHA-256 as recorded | **pass** |
| Q3 | the 1M-cell RGBA and PNG SHA-256, range bits and `cells_drawn` as recorded; equal to the copy | **pass**: RGBA 9ac74c08..., PNG 59638fce... (188138 bytes), range bits bfeffbe708600597..3feffbe40acae0c8, 1000000 cells |
| Q5-i | the CLI's `render.png` stdout identical across binaries and calls | **pass**: 359859 bytes, identical in all 16 calls, under U and under S2L |

### Timing: the margin proxy (Q4) and the CLI (Q5)

**Q4**, `render_speed_w48q::q4_margin_proxy_render_times`, 7 rounds with the order rotating. Medians, with the range in
brackets:

| Variant | U (22:28, host 84 % busy) | S2L (22:29, host 94 % busy) |
|---|---|---|
| base copy | 1.698 s (1.663-1.808) | 1.934 s (1.615-4.975) |
| product, 1 thread | 0.288 s (0.278-0.295) | 0.381 s (0.262-1.259) |
| product, 2 threads | 0.170 s (0.158-0.203) | 0.619 s (0.363-1.099) |
| product, default threads (8 here) | 0.084 s (0.079-0.104) | 0.394 s (0.233-1.690) |

| Id | Limit | U | S2L | Result |
|---|---|---|---|---|
| Q4-r1 (primary) | 1-thread ratio <= 0.43 | 0.169 | 0.197 | **pass** |
| Q4-r2 | 2-thread ratio <= 0.43 | 0.100 | 0.320 | **pass** |
| Q4-v | S2L: base median >= 1.6 s | | 1.934 s | valid |
| Q4-a1 | S2L: 1 thread <= 1.0 s | | 0.381 s | **pass** |
| Q4-a2 | S2L: 2 threads <= 1.0 s | | 0.619 s | **pass** |

Predicted for CI from the primary ratio: 2.316 s x 0.197 = 0.46 s with one thread, and 2.316 s x 0.320 = 0.74 s with two
threads (a 2-vCPU runner's automatic count). That is 2.7 to 4.4 times below the 2 s limit.

**Q5**, `render_cli_speed_w48q::q5_cli_render_timing`, with the control and then the change `rcfd.exe`, 7 rounds,
alternating.

| | U (22:31, host 96 % busy) | S2L (22:32, host 83 % busy) |
|---|---|---|
| control `render.png`: first; median (min-max) | 1.797 s; 1.565 s (1.544-1.699) | 4.271 s; 3.423 s (3.294-4.449) |
| change `render.png`: first; median (min-max) | 0.193 s; 0.175 s (0.168-0.204) | 0.787 s; 0.873 s (0.632-0.922) |
| `service.info`, the process floor (control; change) | 0.027 s; 0.027 s | 0.212 s; 0.255 s |

| Id | Limit | U | S2L | Result |
|---|---|---|---|---|
| Q5-r | after/control <= 0.5 | 0.112 | 0.255 | **pass** |
| Q5-v | S2L: control median >= 1.6 s | | 3.423 s | valid |
| Q5-a | S2L: after median <= 1.0 s | | 0.873 s | **pass** |

The CI-faithful r4 estimate, as registered (reported, not a limit), is Q5 after (S2L) - Q4 default (S2L) + Q4 2 threads (S2L):
0.873 - 0.394 + 0.619 = **1.098 s**.
* This session's S2L was heavier than CI: the control `render.png` took 3.423 s here, against CI's 2.086 s for the same
  call.
* Scaled by that same-run control, the estimate becomes 2.086 x 1.098 / 3.423 = **0.67 s** on CI.

### Timing: the existing tests (R2, R2-u, R4)

**R2**, `render_pixels::r2`, 5 rounds (22:35-22:50). In each round the control and change binaries ran back to back
under every condition, the control first in odd rounds. Render medians, with the range in brackets:

| Condition | Control | Change | Change runs >= 2 s | Control runs that failed | Host busy |
|---|---|---|---|---|---|
| U | 1.109 s (0.921-1.190) | 0.047 s (0.039-0.054) | 0 of 5 | 0 of 5 | 29-79 % |
| S2 | 1.096 s (0.652-2.444) | 0.144 s (0.119-0.392) | 0 of 5 | 1 of 5 | 28-100 % |
| P2 | 1.190 s (1.032-2.585) | 0.108 s (0.082-0.127) | 0 of 5 | 1 of 5 | 33-100 % |
| S2L | 2.003 s (1.102-2.280) | 0.213 s (0.174-0.375) | 0 of 5 | 3 of 5 | 29-100 % |

* **R2: pass.** Every change run rendered in under 2 s; the largest was 0.392 s. The control failed 5 of its 20 runs in the
  same session, two of them outside S2L while the host was 100 % busy.
* **R2-u** (the brief's example, the median under U <= 1.2 s): **pass**, 0.047 s. The base meets it too (0.729 s in C-r2,
  1.109 s here), as registered.
* The PNG encode is unchanged in kind: medians of 0.048-0.050 s under U, S2 and P2 for both binaries, and 0.104 s (control)
  and 0.136 s (change) under S2L.

**R4**, `render_cli_w39a::r4` on the change build, 3 rounds (22:50-22:58). Before every run, the `rcfd.exe` that the test
binary reaches was checked to be the change build (A6A077BE).

| Condition | `rcfd call render.png`, median (min-max) | `run.initialize`, median | Runs >= 2 s | Control, C-r4 median | Host busy |
|---|---|---|---|---|---|
| U | 0.147 s (0.141-0.170) | 13.0 s | 0 of 3 | 0.850 s | 54-100 % |
| S2 | 0.225 s (0.213-0.227) | 12.7 s | 0 of 3 | 0.776 s | 45-68 % |
| P2 | 0.244 s (0.215-0.259) | 14.9 s | 0 of 3 | 0.752 s | 38-60 % |
| S2L | 0.422 s (0.409-0.430) | 31.7 s | 0 of 3 | 2.502 s (3 of 3 failed) | 52-82 % |

**R4: pass.** All 12 runs were under 2 s; the largest was 0.430 s.
* Under S2L the server's render takes 8 threads here (optimistic, see Q5); the CI-faithful estimate is in the Q5 section.
* `run.initialize`, which is not limited, is unchanged: 12.7-14.9 s under U, S2 and P2, and 31.7 s under S2L, against
  11.7-12.2 s and 30.3 s for the control.

### Re-runs

All on the change build, 22:58-23:10.

| Re-run | Result |
|---|---|
| `cargo test --release -p rcfd-post`, every test that is not ignored | 168 passed, 0 failed, 17 ignored, over 20 test binaries. They include the 5 `render` unit tests, `render_pixels` r1-r3 and `render_speed_w48q` Q1, Q2 and the row 15 probe |
| Q1 and Q2 in a debug build | 4 passed, 3 ignored; Q1 again 175 renders, 0 differ |
| `rcfd-api` `--lib service::tests::` | 11 passed. Among them `in_process_run_to_convergence_with_monitors_reports_and_render`: `render.png` with vectors, and `render.glb` |
| `rcfd-mcp` `project_w48f` | 10 passed (F9 `render_field`) |
| `rcfd-render` `tests/render.rs` | 11 passed; 2 ignored, its existing known misses (ADR 0125) |
| `rcfd-studio` `viewport_live` | 2 passed |
| `rcfd-gui` `viewport_raster` | 15 passed, 4 ignored (existing) |
| `rcfd-cli` `workspace_graph` | 3 passed. The crate graph is unchanged |
| `cargo clippy --workspace --all-targets -- -D warnings` | clean |
| `cargo fmt -p rcfd-post -- --check` and `-p rcfd-cli` | clean (deviation 1) |

**`rcfd-render` G1 on this host's GPUs.** G1 compares the GPU image with `render_scalar`. It was run once more with
`--nocapture` to see its adapters. On each of the 5 it gave max diff 0, 0 differing pixels and a negative control of 0.992:
* RTX 5080, Vulkan and DX12;
* AMD Radeon iGPU, Vulkan and DX12;
* the Microsoft Basic Render Driver.

**Repository state against main 6867e40.** Only these five files changed: this ADR, `render.rs` and the three new test
files.
* No `Cargo.toml` and no `Cargo.lock` changed.
* No `unsafe` was added.
* `baseline_hashes.rs` is untouched.

**Limit audit** (`python scripts/limit_audit.py`, after the change). 139 hits in the repository, 2 of them in this job's
files. Each was read.

| Hit | Class | Classification |
|---|---|---|
| `crates/rcfd-post/tests/render_speed_w48q.rs:685` `fn q0_profile_control_phases` | (iii) | false positive: "control" names the base renderer that it profiles. It is an ignored measurement, and its asserts check that every probe equals the base |
| `crates/rcfd-post/tests/render_speed_w48q.rs:797` `return;` | (iv) | documented skip: the row 15 probe returns only when the constructor refuses the mesh, and Q1 then reports the row as dropped. The probe reports and has no limit, as registered |

### Deviations and notes

1. **fmt.** `cargo fmt --all -- --check` stops here with "The filename or extension is too long (os error 206)": the worktree's
   path is deep, and the command line holds every file of the workspace. `cargo fmt -p rcfd-post -- --check` and
   `cargo fmt -p rcfd-cli -- --check` were run instead: clean. These are the only packages this job changed. The gate's fmt
   runs at CI's short path.
2. **A shared host.** Other agents' builds and solver runs used the host during every run: 20-100 % CPU before the runs. The
   registered set-up records this but cannot prevent it.
   * The after session was busier than the control session. The control's `render.png` under S2L took 2.502 s in C-r4 and
     3.423 s in Q5's same-run control. The control's r2 under U took 0.729 s in C-r2 and 1.109 s in the R2 session.
   * Every comparison was therefore also made inside one session: Q4 interleaved, Q5 alternating, R2 back to back. The
     absolute limits held in the busier session.
3. **S2L's contention varies from run to run.** The base render was 1.111 s in one C-r2 round, 1.03 s in Q0, and 1.102-2.280
   s in the R2 session's control runs. Both registered S2L runs that carry a validity check (Q4, Q5) were valid, so no
   repeat or amendment was needed.
4. **Row 15 does not tell the two paths apart.** Without the non-finite path, the probe gives the same image (0 pixels
   differ). That path's identity with the base therefore rests on its construction: it hands `convex_hull_into` the base's
   own list of face points. It also rests on row 15 passing on all 7 thread settings. No discriminating test exists.
5. **Two threads under S2L were slower than one** (Q4, 0.619 s against 0.381 s). This is within the limits; see question 1.
6. **Q0's absolute times are inflated** by the host load, as stated in part 2. The probe deltas carry the profile.
7. **The control's r4 runs** (C-r4) used the control test binary. It reaches `target\release\rcfd.exe`, whose hash was checked
   to be the control's (20B2DFE4) before every run. The after runs (R4) checked the change's (A6A077BE).
8. **Processes of other sessions** (`rcfd.exe` runs of other worktrees) ran on the host during some runs; none was touched.
   No busy loop of this job was left running (checked after every series).

### Not run

* The Full gate (`scripts/gate_test.py`) and CI on `windows-latest`. The CI figures above are predictions from S2L and the
  ratios.
* Linux and macOS.
* Q0's profile of the change build. Q4 gives its totals; the phases of the new code were not timed separately.
* `crates/rcfd-api/tests/baseline_hashes.rs`. No solver code changed, and it was not re-recorded.
* r2 and r4 in debug builds: they are ignored there by design.
* The Studio's and `rcfd-mcp`'s render paths by hand. Their automated tests are in Re-runs.
* More than 16 threads, and hosts with more than 64 logical CPUs.

### Files

* This ADR.
* `crates/rcfd-post/src/render.rs`.
* New tests:
  * `crates/rcfd-post/tests/render_speed_w48q.rs`: Q0-Q4 and the row 15 probe;
  * `crates/rcfd-post/tests/render_control_w48q/mod.rs`: the base renderer's verbatim copy;
  * `crates/rcfd-cli/tests/render_cli_speed_w48q.rs`: Q5 and Q6.
* Unchanged:
  * `render_pixels.rs` (r1-r3) and `render_cli_w39a.rs` (r4), with their 2 s limits;
  * `crates/rcfd-api/src/service.rs`;
  * `baseline_hashes.rs`.

### Questions for the maintainer

1. **Two threads can be slower than one under contention.**

   Q4 under S2L gave 0.381 s with 1 thread and 0.619 s with 2. Each band and each cell range waits for the slowest thread,
   and a thread that loses its time slice to the busy loops holds both phases back. On a quiet host, 2 threads beat 1
   (0.170 s against 0.288 s under U). The limits are met either way. Options:
   * (a) keep the automatic policy as registered;
   * (b) automatic = 1 thread when `available_parallelism` <= 2;
   * (c) more ranges and bands than threads, handed out through an atomic counter. The output stays the same, because
     ranges are joined in cell order and bands are disjoint.

   **Recommendation:** (a) now. (c) as a follow-up only if more margin is wanted on contended runners. It needs its own
   registration and re-runs.
2. **`overlay_vectors` still builds a full hull per cell** (the base `cell_polygon`), only to test the slice. A
   `render.png` with `vectors: true` on 1M cells therefore still pays about 1 s for that. Options:
   * (a) leave it;
   * (b) use the slice test alone. The output is identical, since only `is_some()` is used. Check it with a Q1-style
     comparison against the base.

   **Recommendation:** (b), as a small follow-up job.
3. **The 1M-cell hash check (Q3) stays out of the gate.** Its mesh takes 2.7 GB, next to r2's own 2.7 GB, on a runner of
   about 7 GB (UNVERIFIED). Options:
   * (a) keep it ignored and run it by hand when `render.rs` changes;
   * (b) add the hash asserts to r2 itself. That is free, but r2 then fails for a non-timing reason whenever an intentional
     change of the image lands;
   * (c) a gate test on a medium mesh only. Q1 row 14, 65k cells at r2's width, already does this.

   **Recommendation:** (a) together with (c), as done.
4. **New public API:** `RenderOptions::threads`, `PARALLEL_MIN_CELLS` and `MAX_AUTO_THREADS`. Every struct literal in the
   workspace uses `..Default::default()`, so nothing else changed. Options:
   * (a) keep them;
   * (b) make the constants private and keep only the field.

   **Recommendation:** (a). The constants document the automatic policy.
5. **The base renderer's copy** (`tests/render_control_w48q/mod.rs`, about 300 lines) stays in the repository as Q1's
   oracle. Options:
   * (a) keep it;
   * (b) replace it with recorded hashes of the matrix, which is smaller but weaker when the matrix grows.

   **Recommendation:** (a).
6. **Confirm on CI.** Run the Full gate on `windows-latest` after the merge: r2 and r4 are predicted at about 0.5-0.7 s
   there (Q4, Q5). **Recommendation:** yes, with the result appended here.
