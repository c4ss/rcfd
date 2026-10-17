# ADR 0058: Wave 29D, transport-agnostic distributed-memory layer (F-100, F-016)

Status: accepted. Everything under "Decisions" and "Registered checks" was written before any code of this wave and before any run
of this wave. Limits are not changed, widened, replaced or deleted after a run. A miss is reported as measured and the assertion
moves to an ignored `*_known_miss` test with the registered limit intact; the cause is diagnosed, not tuned away. A test set-up
defect gets a NEW corrected test and the old one stays as a record. Addenda (exact numbers, deviations) may be appended at the end.

Clean-room: first principles and published literature only (Saad, "Iterative Methods for Sparse Linear Systems", for CG, BiCGStab
and block-Jacobi/additive-Schwarz preconditioning; Higham, "Accuracy and Stability of Numerical Algorithms", for the dot-product and
matrix-vector round-off bounds). No vendor code, text or installs, nothing from vendor material. Waves 29A, 29B, 29C work
in the same tree: this wave adds a new crate `rcfd-dist` and one-line hunks in the root `Cargo.toml`, `Cargo.lock`,
`crates/rcfd-cli/tests/workspace_graph.rs`, `docs/architecture.md`, `docs/reference-data.md`, `docs/roadmap.json`, committed through
a temporary index. No existing solver path is edited, so the baseline hashes and all existing tests are unchanged by construction
(re-verified at the end).

## Audit (HEAD 67dc5d3)

* `rcfd_mesh::partition` (F-016) already has the `Partitioner` trait, `Rcb`, `Bisection`, `Sectors`, `GraphPartitioner`, and
  `Decomposition` with per-part `SubDomain` (owned cells ascending by global id, ghost cells grouped by owning peer in ascending peer
  order, `send`/`recv` local-id lists, one face-neighbour layer). `Decomposition::exchange` is a single-address-space copy that mirrors
  what a message layer would do; it has no rank, no transport, no reductions and no consistency validator (`from_assignment` checks
  only length and range).
* `rcfd-linalg` (layer 1) has `CsrMatrix`, `LduMatrix`, `LinearOperator`, `Preconditioner` (`Identity`, `Jacobi`, `Ic0`, `Ilu0`, `Chebyshev`),
  `cg`, `bicgstab`, `gmres` and the deterministic fixed-chunk (4096) rayon `dot`. `cg` and `bicgstab` hard-wire the local `dot`/`norm2`,
  so a `LinearOperator` implementation alone cannot make them distributed. `rcfd-linalg::backend` (wave 22, ADR 0038) is the precedent
  for a backend seam with a CPU fallback that leaves the default path unchanged.
* Layering: `rcfd-mesh` and `rcfd-linalg` are both layer 1 and cannot see each other's types. A layer that needs `Decomposition` and
  `CsrMatrix` together must sit above both. `workspace_graph.rs::layering_matches_architecture_doc` panics on an unknown crate and
  `cargo metadata` lists dev-dependencies as dependencies, so a dev-dependency of `rcfd-dist` on `rcfd-fv` would be rejected; tests that
  need `rcfd-fv` live in a higher crate.

## Decisions

D1 New crate `rcfd-dist`, layer 2 (same layer as `rcfd-gpu`), depending on `rcfd-core`, `rcfd-mesh`, `rcfd-linalg`, `rayon`. The layer
list in `workspace_graph.rs` (`"rcfd-gpu" | "rcfd-dist" => 2`) and `docs/architecture.md` are updated consistently. No change to
`rcfd-mesh` or `rcfd-linalg`.

D2 Communicator (`trait Communicator: Send + Sync`). Point-to-point is the only required transport primitive, so that an MPI or socket
backend is a later drop-in: `rank()`, `size()`, `send(dest, tag, &[f64])`, `recv(src, tag) -> Vec<f64>`, `barrier()`; everything else
(`neighbour_exchange`, `allreduce_sum`, `allreduce_max`, `allgather`) is a provided method built on those, so a backend may override
them with native collectives. Messages between one ordered pair arrive FIFO; a tag mismatch, a closed peer or a receive timeout
(default 60 s) is a located `CommError` (rank, peer, tag, reason), never a hang. Implementations:
* `SerialComm` (rank 0 of 1): exchange is a no-op, `allreduce_sum(x)` returns `x` itself (not `0.0 + x`), so the serial fallback is the
  unchanged code path.
* `ThreadComm`: in-process ranks, one unbounded `std::sync::mpsc` channel per ordered (src, dst) pair (sends never block; a panicking
  rank drops its senders so peers error rather than hang); `run_ranks(p, f)` runs the ranks on `std::thread::scope` threads (never
  rayon tasks: a blocking receive inside a rayon worker could deadlock the pool that `vecops` uses).
* NOT delivered: MPI and socket transports. They are a later user decision (library choice, launcher, `--hostfile`); the trait is the
  seam. The roadmap F-100 stays `todo` for the cluster feature.

D3 Reduction ordering policy (deterministic for fixed P and fixed partition): each rank's partial is the existing fixed-chunk
`rcfd_linalg::vecops::dot` over its OWNED entries only (ghosts excluded; independent of the rayon thread count); every rank then
obtains all P partials and sums them sequentially in ASCENDING RANK ORDER, starting from partial 0 (no `0.0 +`). Every rank therefore
holds the bit-identical scalar, which is required for identical alpha/beta/stop decisions on every rank. P = 1 returns the single
partial itself, which is exactly the serial `dot`. The sum order differs between different P (so results for different P agree only
to round-off), and the policy is O(P) messages per reduction (an all-to-all of one value); a tree reduction is a later optimisation
that would change the stated order and is not done.

D4 Halo plan. `HaloPlan` is the `SubDomain` send/recv lists of one rank. `exchange(comm, plan, &mut local)` packs the owned values per
peer, sends to all peers, then receives from all peers in ascending peer order and fills the ghost slots; owner values are copied
bit-exactly. `validate_decomposition(mesh, &Decomposition)` is a new validator (the mesh crate is untouched) returning a located
`rcfd_core::Error::Mesh` naming part, peer and global cell for: a cell owned by no part or by two parts; `part_of_cell` disagreeing
with the owned lists; a ghost not owned by its stated peer; a ghost that is not a face neighbour of an owned cell; a missing ghost
(a cut face whose off-part cell is not in the ghost list); `send`/`recv` length or symmetry mismatch between peers; send ids that are not
owned or recv ids that are not ghosts; duplicate ghosts.

D5 Distributed vector and matrix. A rank vector has `n_owned + n_ghost` entries in the `SubDomain` local numbering (owned ascending by
global id, then ghosts by peer). `DistCsr::from_global(a, decomposition, rank)` extracts the owned rows of the global CSR matrix and
remaps columns to local ids PRESERVING the global column order within each row (so the per-row summation order equals the serial one);
a column outside owned + ghost is a located error (part, global row, global column). `matvec` = halo exchange of `x` then the local
product of the owned rows. `DistOperator` (rank, comm, matrix) implements `rcfd_linalg::LinearOperator` for owned-length vectors (a
Mutex-guarded scratch holds the local vector with ghosts, since `LinearOperator::apply` takes `&self` and is `Sync`).

D6 Krylov. `cg` and `bicgstab` in `rcfd-dist::krylov` are the `rcfd-linalg` routines copied operation for operation (same sequential
loops, same `axpy`/`xpby`), with `dot`/`norm2` replaced by the D3 global reduction and with a `Preconditioner` argument (the existing
trait, owned-length vectors). They return `Result<SolveStats, CommError>` and, because every rank takes the same decisions from the same
scalars, all ranks return identical stats. With `SerialComm` or `P = 1` the arithmetic is literally the serial routine's.

D7 Preconditioners (all behind `rcfd_linalg::Preconditioner`, communication-free): (a) point Jacobi (the existing `Jacobi` on the owned
diagonal; mathematically partition-independent), (b) `BlockJacobi`: `Ilu0` of the owned-owned diagonal block of each rank (ghost
columns dropped), the standard non-overlapping additive-Schwarz/block-Jacobi preconditioner of Saad Sec. 14.3. For P = 1 the block is the
whole matrix, so it equals the serial `Ilu0` preconditioner. Overlapping additive Schwarz (an overlap layer plus a halo-exchanged
restriction) is NOT delivered; block-Jacobi was chosen because it is communication-free and keeps the preconditioner a plain
`Preconditioner`. There is no coarse space, so iteration counts grow with the number of parts (D12 registers the allowance).

D8 Convenience driver `solve_in_process(a_global, b_global, &Decomposition, method, precond, control)` scatters, runs P ranks with
`ThreadComm`, gathers the global solution and the per-rank stats. With `P = 1` it uses `SerialComm`-equivalent code.

D9 Case tree: nothing is added to the schema. No `parallel` key and no `--hostfile` wiring (rejected as before, exit 2): wiring a
distributed solve into `rcfd run` requires distributed assembly of the whole SIMPLE/PISO stack, which is not attempted.

D10 Stretch (only if D1-D8 are complete and green): distributed assembly of ONE steady laminar problem inside `rcfd-dist::assemble`:
steady scalar convection-diffusion on the partitioned 2-D cavity with a prescribed divergence-free rotating velocity field
(`u = (-(y-0.5), (x-0.5))`, diffusivity Gamma), Green-Gauss cell gradient with linear face interpolation using ghost values from a
halo exchange, upwind convection with a deferred linear-upwind correction (gradient of the owned and the ghost cells, the ghost
gradients obtained by a second exchange), owned-row assembly from the global mesh faces that touch owned cells, solved by the D6
BiCGStab. This is NOT distributed SIMPLE: momentum, pressure-velocity coupling, turbulence, boundary-condition catalogue and the
case driver are not distributed. "A converged laminar flow field matches the serial one" is therefore claimed only for this scalar
problem, and the report says so.

D11 Honesty about performance: the in-process transport runs rank threads on one machine sharing memory bandwidth and the rayon pool.
Timing versus P is REPORTED (printed by an ignored/`--nocapture` test and recorded in the addendum), never asserted, and is not a
cluster or network measurement.

## Registered checks (limits fixed before any run)

Test meshes: `cavity_2d_rect(48, 48, 1, 1)` (2304 cells, `lid`/`walls` Dirichlet, `frontAndBack` empty) and `box_mesh([8,8,8])`.
Partitioners: `Rcb` for the solves; `Rcb`, `Bisection`, `GraphPartitioner` for structure checks. P in {1, 2, 4, 8} unless stated.
Poisson/diffusion matrix: finite-volume `-div(grad phi) = f`, internal face coefficient `|S_f| / d_f` (orthogonal), boundary patch
faces `|S_f| / (d_b)` with `d_b` the cell-centre-to-face distance (homogeneous Dirichlet on every non-empty patch), `f = 1`. The matrix
is SPD with condition number `kappa ~ cot^2(pi / (2 (n+1)))` for the 48 x 48 grid, `kappa_est = 1000` (exact 973).

B1 Partition structure (cavity, box; P in {1,2,4,8}; the three partitioners): the union of `owned` over parts is every cell exactly once
(count = n_cells, no duplicate); `part_of_cell` agrees; every ghost is owned by its stated peer; every ghost is a face neighbour of an
owned cell; every off-part neighbour of an owned cell is in the ghost list (no missing, no extra ghost: ghost set equals the exact
neighbour set computed independently from the face list); `recv[q]` of part p has the same length as `send[p]` of part q and the
global ids line up element by element; `validate_decomposition` returns Ok.

B2 Located errors: five corrupted decompositions (public fields mutated) are each rejected by `validate_decomposition`, and each error
text contains the part number, the peer number and the global cell id of the defect: (i) a ghost cell dropped, (ii) an owned cell
duplicated into another part, (iii) `send` list shortened (send/recv length mismatch), (iv) a ghost attributed to a wrong peer,
(v) an owned cell removed (lost). A global matrix with a column outside owned + ghost for a row (an entry coupling to a non-neighbour
cell) is a located error naming part, global row and global column. An invalid `Decomposition` is never accepted silently.

B3 Halo update (synthetic fields `g(c) = c` as f64, and `g(c) = sin(1.7 x_c) + y_c z_c + 3` from the cell centres; cavity and box;
P in {2,4,8}; the three partitioners): after the in-process exchange every ghost value is bit-equal (`==` on `to_bits`) to the owner's
value, and the owned values are unchanged; the result also equals the single-address-space `Decomposition::exchange`.

B4 Matrix-vector product (cavity; P in {1,2,4,8}; `x` a fixed LCG random vector in [-1, 1]): distributed product gathered to the global
numbering versus the serial `CsrMatrix::matvec`: for P = 1 bit-identical; for every P per row `|y_d - y_s| <= 2 gamma_k (|A||x|)_i`
with `gamma_k = k u / (1 - k u)`, `u = 2^-53`, `k` the row nonzero count (Higham Thm 3.5/Sec. 3.5 style bound). Whether the product is
in fact bit-identical for all P (the column order is preserved) is reported.

B5 Reductions: `allreduce_sum` of rank-dependent values equals the sequential rank-order sum bit-exactly on every rank; all ranks
return bit-identical scalars; the distributed dot of two random vectors differs from the serial `dot` by at most
`gamma_n sum |a_i b_i|` with `n = 2304`, and for P = 1 is bit-identical; run-to-run: 20 repeats at fixed P give a bit-identical value
and a bit-identical CG solution (including the iteration count), and the same under a rayon global pool of 1 thread and a local
4-thread pool (`ThreadPoolBuilder::install`).

B6 Solve (cavity Poisson, `tol_rel = 1e-10`, `tol_abs = 1e-30`, `max_iter = 2000`, `x0 = 0`; CG and BiCGStab; preconditioners none,
point Jacobi, block-Jacobi; P in {1,2,4,8}): every run converges; the TRUE residual `||b - A x||_2 <= 1.01e-10 ||b||_2` (recomputed
with the serial matrix); relative L2 difference to the serial solution (serial `cg`/`bicgstab` with the corresponding global
preconditioner, `Ilu0` of the global matrix for block-Jacobi, same tolerance) `||x_P - x_s||_2 / ||x_s||_2 <= 2 kappa_est tol_rel =
2e-7` (a rigorous norm-wise bound: each solution satisfies `||e||/||x|| <= kappa ||r||/||b||` with `kappa = 973`, and `x_0 = 0`
gives `||r_0|| = ||b||`). The limit is 2e-7 = 2 kappa_est x the solver tolerance, i.e. 2000 times the solver tolerance.

B7 P = 1 bit-identity: for every method and preconditioner above, the in-process P = 1 solve and the `SerialComm` solve return a solution
whose bits equal the serial `rcfd-linalg` `cg`/`bicgstab` (same preconditioner), with equal iteration count and equal
`final_residual` bits.

B8 Iteration counts (CG, cavity): point Jacobi: `|iters(P) - iters(serial Jacobi)| <= 2` for P in {2,4,8} (mathematically
partition-independent; only round-off may shift the count). Block-Jacobi (non-overlapping ILU0 blocks, no coarse space) versus the
serial global `Ilu0` preconditioner: `iters(P) <= 2.5 x iters(serial ILU0)` for P in {2,4,8}, and block-Jacobi at P = 8 is not worse
than point Jacobi (`iters_blockjacobi(8) <= iters_jacobi`). All counts are printed. The same 2.5 ratio is registered for BiCGStab.
Rationale for 2.5: ILU(0) block preconditioners without overlap degrade with the number of subdomains because the coupling across
subdomain interfaces is dropped; for 8 strips/blocks of a 48 x 48 grid the interface fraction is moderate, so a factor between 1 and 2.5
is expected, and a larger factor would signal an implementation error rather than the known degradation.

B9 Strong scaling (REPORTED, never asserted): cavity 192 x 192 (36864 cells) Poisson, CG with block-Jacobi, `tol_rel = 1e-8`,
P in {1,2,4,8}, 3 repeats, median wall time of the solve loop, with speed-up and parallel efficiency, iteration counts and the host
logical-core count. The statement printed with it: in-process threads share one machine's memory bandwidth and rayon pool, so this is
not a cluster measurement and says nothing about network scaling.

B10 (stretch, only if D10 is done) Distributed gradient: for a linear field `phi = a + b.x` the Green-Gauss gradient equals `b` to
`1e-12 |b|` in every cell whose faces are all interior; for a smooth field, the distributed gradient (P in {2,4,8}) versus the serial
(P = 1) gradient: `max |g_P - g_1| <= 1e-13 max |g_1|`. Distributed steady convection-diffusion (Gamma = 0.05, Dirichlet 0 and 1 on
two patch groups, 32 x 32 cavity, deferred-correction iterations until `||phi^{k+1} - phi^k||_inf <= 1e-12`, linear tol_rel 1e-12):
converged field versus the P = 1 result `max |phi_P - phi_1| <= 1e-9 (max phi - min phi)`; the P = 1 result also matches the same
assembly solved with serial `bicgstab` bit-for-bit; global boundedness (upwind part) is reported.

B11 Baselines and existing tests unchanged: `crates/rcfd-api/tests/baseline_hashes.rs` passes untouched, `workspace_graph.rs` passes
with the one layer-map edit, no existing source file on a solver path is modified (git diff of the commits lists only the files in the
header), checked at the end.

## Not delivered (named up front)

MPI and socket transports (the user decides library, launcher and `--hostfile`); overlapping additive Schwarz; coarse-space or
distributed AMG; distributed assembly of momentum, pressure-velocity coupling, turbulence, energy and the boundary-condition catalogue;
`rcfd run` wiring and any case-schema key; parallel mesh reading/partitioned I/O; dynamic load balancing; multi-device GPU; tree
(logarithmic) reductions; non-blocking communication overlap with computation; a cluster performance claim.
