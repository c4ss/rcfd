//! Elasticity solves on the ranks of `rcfd-dist` and on the CUDA device (ADR 0431, F-433; plan ADR 0310 decision A5 (a)).
//!
//! Library only: nothing here is reached from a case file, the CLI or the default static solve.
//!
//! * [`solve_static_ranks`]: the reduced stiffness of a [`SolidModel`] split by nodes over in-process ranks
//!   ([`node_partition`], [`RankSystems::build`]) and solved by the distributed CG of `rcfd-dist` with the block Jacobi or
//!   a rank-local incomplete Cholesky of F-381, in the outer loop of ADR 0282 (true residual, refinement restarts). With
//!   one rank the arithmetic is that of `rcfd_fem::solve::solve_static_report`, bit for bit (ADR 0431 arithmetic 2).
//! * [`solve_static_device`]: the reduced system solved by `rcfd-gpu` (Jacobi or Chebyshev) through
//!   `rcfd_gpu::resolve`, with the CPU backend for `cpu`, for `auto` below the threshold and for `auto` without a device.
//!
//! Assembly stays serial in both. Models on the transformation path are refused.

use rcfd_core::vec3::Vec3;
use rcfd_dist::comm::{CommError, Communicator, Tag, DEFAULT_TIMEOUT};
use rcfd_dist::krylov::{self, gnorm2};
use rcfd_dist::{run_ranks_with_timeout, DistCsr, DistOperator, HaloPlan};
use rcfd_fem::assemble::stiffness_with;
use rcfd_fem::constraint::{check_rigid_body_motions, prescribed_equations, reactions, reduce, Prescribed};
use rcfd_fem::loads::{nodal_load_vector, NodalForceAt};
use rcfd_fem::solve::MAX_RESTARTS;
use rcfd_fem::{DofMap, FemError, Result, SolidModel, SolidResult, SolverStats};
use rcfd_linalg::precond::{BlockJacobi, IcOptions, IncompleteCholesky};
use rcfd_linalg::vecops::norm2;
use rcfd_linalg::{
    CpuBackend, CsrMatrix, GpuPrecond, LinearBackend, LinearDevice, LinearOptions, Preconditioner, SolveControl,
};
use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// Solver name of the rank solve in errors.
pub const RANK_SOLVER: &str = "cg-ranks";
/// Solver name of the device solve in errors.
pub const DEVICE_SOLVER: &str = "cg-device";

// ---------------------------------------------------------------------------------------------------------------------
// Common preparation (ADR 0431 D2).

struct Prepared {
    dofs: DofMap,
    f: Vec<f64>,
    fixed: Vec<(usize, f64)>,
    k: CsrMatrix,
    sys: rcfd_fem::constraint::ReducedSystem,
    assembly_seconds: f64,
}

fn prepare(
    model: &SolidModel,
    prescribed: &[Prescribed],
    forces: &[NodalForceAt],
    threads: Option<usize>,
    solver: &'static str,
) -> Result<Prepared> {
    let t0 = Instant::now();
    model.validate()?;
    model.check_supported_with_beams()?;
    if rcfd_fem::mpc::uses_transformation(model) {
        return Err(FemError::Solver {
            solver,
            reason: "the model has linear constraints, rigid sets or elastic supports (the transformation path); \
                     the rank and device solves take the elimination path only (ADR 0431)"
                .to_string(),
        });
    }
    let dofs = model.dof_map();
    let f = nodal_load_vector(model, forces)?;
    let fixed = prescribed_equations(model, prescribed)?;
    check_rigid_body_motions(model, &fixed)?;
    let k = stiffness_with(model, threads)?;
    let sys = reduce(&k, &f, &fixed)?;
    Ok(Prepared {
        dofs,
        f,
        fixed,
        k,
        sys,
        assembly_seconds: t0.elapsed().as_secs_f64(),
    })
}

/// `|b - A x| / |b|` with the plain sums of `rcfd_fem::solve` (`residual_norm`), so that results have its bits.
fn residual_norm(a: &CsrMatrix, b: &[f64], x: &[f64]) -> f64 {
    let bn = b.iter().map(|v| v * v).sum::<f64>().sqrt();
    if bn == 0.0 || x.is_empty() {
        return 0.0;
    }
    let mut r = vec![0.0; b.len()];
    a.residual(b, x, &mut r);
    r.iter().map(|v| v * v).sum::<f64>().sqrt() / bn
}

fn finish(p: &Prepared, u_free: &[f64], name: &str, iterations: usize) -> Result<(SolidResult, f64)> {
    let relative_residual = residual_norm(&p.sys.matrix, &p.sys.rhs, u_free);
    let u = p.sys.expand(u_free);
    let r = reactions(&p.k, &u, &p.f, &p.fixed);
    let result = SolidResult::new(p.dofs.clone(), u)?
        .with_reactions(r)?
        .with_stats(SolverStats::new(
            name,
            p.sys.free().len(),
            iterations,
            relative_residual,
        ));
    Ok((result, relative_residual))
}

// ---------------------------------------------------------------------------------------------------------------------
// The node split (ADR 0431 D3).

/// The rank of every free equation: recursive coordinate bisection of the nodes that have free equations (the rule of
/// `rcfd_mesh::partition::Rcb` on node points: the longest extent, ties by node id, the left part getting
/// `round(count floor(p/2) / p)` nodes). All equations of a node go to the node's rank.
pub fn node_partition(free: &[usize], dofs: &DofMap, nodes: &[Vec3], p: usize) -> Result<Vec<usize>> {
    let node_of: Vec<usize> = free
        .iter()
        .map(|&q| dofs.node_component(q).map(|(n, _)| n))
        .collect::<Option<Vec<usize>>>()
        .ok_or_else(|| FemError::Solver {
            solver: RANK_SOLVER,
            reason: "a free equation has no node".to_string(),
        })?;
    let mut list: Vec<usize> = node_of.clone();
    list.sort_unstable();
    list.dedup();
    if p == 0 || p > list.len() {
        return Err(FemError::Solver {
            solver: RANK_SOLVER,
            reason: format!("cannot split {} nodes with free unknowns over {p} ranks", list.len()),
        });
    }
    let mut part_of_node: HashMap<usize, usize> = HashMap::with_capacity(list.len());
    bisect(nodes, list, 0, p, &mut part_of_node);
    Ok(node_of.iter().map(|n| part_of_node[n]).collect())
}

fn bisect(nodes: &[Vec3], mut list: Vec<usize>, first: usize, p: usize, out: &mut HashMap<usize, usize>) {
    if p == 1 {
        for n in list {
            out.insert(n, first);
        }
        return;
    }
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for &n in &list {
        for d in 0..3 {
            lo[d] = lo[d].min(nodes[n][d]);
            hi[d] = hi[d].max(nodes[n][d]);
        }
    }
    let axis = (0..3)
        .max_by(|&a, &b| (hi[a] - lo[a]).total_cmp(&(hi[b] - lo[b])))
        .unwrap_or(0);
    list.sort_by(|&a, &b| nodes[a][axis].total_cmp(&nodes[b][axis]).then(a.cmp(&b)));
    let left_parts = p / 2;
    let n_left = (list.len() * left_parts + p / 2) / p;
    let n_left = n_left.clamp(left_parts, list.len() - (p - left_parts));
    let right = list.split_off(n_left);
    bisect(nodes, list, first, left_parts, out);
    bisect(nodes, right, first + left_parts, p - left_parts, out);
}

/// The rows of the reduced stiffness of every rank (ADR 0431 D3).
#[derive(Debug, Clone)]
pub struct RankSystems {
    /// One distributed matrix per rank, in rank order.
    pub ranks: Vec<DistCsr>,
    /// Per rank, the local node blocks of its owned rows (block pointer, as `rcfd_fem::solve::node_blocks`).
    pub node_blocks: Vec<Vec<usize>>,
    /// Per rank, the number of nodes it owns.
    pub owned_nodes: Vec<usize>,
}

impl RankSystems {
    /// Split the reduced matrix `a` (rows = the free equations `free`) by `part` (the rank of every free equation). Two
    /// equations of one node on different ranks, a rank out of range or a rank without equations are located errors.
    pub fn build(a: &CsrMatrix, free: &[usize], dofs: &DofMap, part: &[usize], p: usize) -> Result<RankSystems> {
        let err = |reason: String| FemError::Solver {
            solver: RANK_SOLVER,
            reason,
        };
        let n = a.n;
        if part.len() != n || free.len() != n || a.ncols != n {
            return Err(err(format!(
                "{} x {} matrix, {} free equations, {} rank assignments",
                a.n,
                a.ncols,
                free.len(),
                part.len()
            )));
        }
        // The split check, independent of how `part` was made (clause c6).
        let mut seen: HashMap<usize, (usize, usize)> = HashMap::new();
        for (g, (&q, &r)) in free.iter().zip(part).enumerate() {
            if r >= p {
                return Err(err(format!("free equation {g} is assigned to rank {r} of {p}")));
            }
            let (node, comp) = dofs
                .node_component(q)
                .ok_or_else(|| err(format!("free equation {g} has no node")))?;
            match seen.get(&node) {
                Some(&(r0, c0)) if r0 != r => {
                    return Err(err(format!(
                        "node {node} is split: component {c0} on rank {r0}, component {comp} on rank {r}"
                    )))
                }
                Some(_) => {}
                None => {
                    seen.insert(node, (r, comp));
                }
            }
        }
        let mut owned: Vec<Vec<usize>> = vec![Vec::new(); p];
        let mut owned_local = vec![0usize; n];
        for (g, &r) in part.iter().enumerate() {
            owned_local[g] = owned[r].len();
            owned[r].push(g);
        }
        if let Some(r) = owned.iter().position(|o| o.is_empty()) {
            return Err(err(format!("rank {r} of {p} owns no equation")));
        }
        // Ghosts of each rank, grouped by owner (ascending), ascending within an owner.
        let mut ghosts: Vec<BTreeMap<usize, Vec<usize>>> = vec![BTreeMap::new(); p];
        for (r, rows) in owned.iter().enumerate() {
            let mut cols: Vec<usize> = rows
                .iter()
                .flat_map(|&g| a.col[a.row_ptr[g]..a.row_ptr[g + 1]].iter().copied())
                .filter(|&c| part[c] != r)
                .collect();
            cols.sort_unstable();
            cols.dedup();
            for c in cols {
                ghosts[r].entry(part[c]).or_default().push(c);
            }
        }
        let mut send: Vec<Vec<(usize, Vec<usize>)>> = vec![Vec::new(); p];
        let mut recv: Vec<Vec<(usize, Vec<usize>)>> = vec![Vec::new(); p];
        let mut ghost_local: Vec<HashMap<usize, usize>> = vec![HashMap::new(); p];
        for r in 0..p {
            let mut next = owned[r].len();
            for (&q, cols) in &ghosts[r] {
                let ids: Vec<usize> = cols
                    .iter()
                    .map(|&c| {
                        ghost_local[r].insert(c, next);
                        next += 1;
                        next - 1
                    })
                    .collect();
                recv[r].push((q, ids));
                send[q].push((r, cols.iter().map(|&c| owned_local[c]).collect()));
            }
        }
        let mut ranks = Vec::with_capacity(p);
        let mut blocks = Vec::with_capacity(p);
        let mut owned_nodes = Vec::with_capacity(p);
        for r in 0..p {
            let n_owned = owned[r].len();
            let n_local = n_owned + ghost_local[r].len();
            let mut row_ptr = Vec::with_capacity(n_owned + 1);
            row_ptr.push(0);
            let mut col = Vec::new();
            let mut val = Vec::new();
            for &g in &owned[r] {
                for j in a.row_ptr[g]..a.row_ptr[g + 1] {
                    let c = a.col[j];
                    col.push(if part[c] == r {
                        owned_local[c]
                    } else {
                        ghost_local[r][&c]
                    });
                    val.push(a.val[j]);
                }
                row_ptr.push(col.len());
            }
            let eqs: Vec<usize> = owned[r].iter().map(|&g| free[g]).collect();
            let bp = rcfd_fem::solve::node_blocks(&eqs, dofs);
            owned_nodes.push(bp.len().saturating_sub(1));
            blocks.push(bp);
            ranks.push(DistCsr {
                rank: r,
                n_owned,
                n_local,
                local: CsrMatrix::rect(n_owned, n_local, row_ptr, col, val),
                plan: HaloPlan {
                    rank: r,
                    n_owned,
                    n_local,
                    send: std::mem::take(&mut send[r]),
                    recv: std::mem::take(&mut recv[r]),
                },
                owned: owned[r].clone(),
            });
        }
        Ok(RankSystems {
            ranks,
            node_blocks: blocks,
            owned_nodes,
        })
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// The rank solve (ADR 0431 D4, D5).

/// The preconditioner of the rank solve.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum RankPreconditioner {
    /// The block Jacobi of F-381 on node blocks (the same operator for every rank count).
    BlockJacobi,
    /// Incomplete Cholesky with zero fill of each rank's owned block (additive Schwarz without overlap); for one rank the
    /// incomplete Cholesky of F-381.
    IncompleteCholesky(IcOptions),
}

/// A fault injected into one rank's sends (a test hook for the failure path; ADR 0431 D5).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RankFault {
    /// From its `after_sends`-th send on, the rank's sends fail with "injected failure".
    Fail {
        /// The faulty rank.
        rank: usize,
        /// The send that fails first (1-based).
        after_sends: usize,
    },
    /// Before its `after_sends`-th send the rank sleeps `delay`.
    Stall {
        /// The stalling rank.
        rank: usize,
        /// The send that is delayed (1-based).
        after_sends: usize,
        /// The delay.
        delay: Duration,
    },
}

/// Options of [`solve_static_ranks`].
#[derive(Debug, Clone, PartialEq)]
pub struct RankOptions {
    /// Number of in-process ranks (threads).
    pub ranks: usize,
    /// The preconditioner.
    pub preconditioner: RankPreconditioner,
    /// Relative tolerance on the true residual, `|b - K u| <= tolerance |b|`.
    pub tolerance: f64,
    /// Iteration cap, summed over restarts.
    pub max_iterations: usize,
    /// Receive time-out of the transport.
    pub timeout: Duration,
    /// Worker threads of the (serial) assembly.
    pub threads: Option<usize>,
    /// Test hook: a fault injected into one rank (`None`: none).
    pub fault: Option<RankFault>,
}

impl RankOptions {
    /// `ranks` ranks, block Jacobi, tolerance 1e-10, 20 000 iterations, the transport's default time-out.
    pub fn new(ranks: usize) -> RankOptions {
        RankOptions {
            ranks,
            preconditioner: RankPreconditioner::BlockJacobi,
            tolerance: 1e-10,
            max_iterations: 20_000,
            timeout: DEFAULT_TIMEOUT,
            threads: None,
            fault: None,
        }
    }
}

/// What a rank solve did.
#[derive(Debug, Clone, PartialEq)]
pub struct RankReport {
    /// The solver name, as in the result's [`SolverStats`].
    pub solver: String,
    /// Number of ranks.
    pub ranks: usize,
    /// Iterations summed over restarts (equal on every rank).
    pub iterations: usize,
    /// Restarts from the true residual.
    pub restarts: usize,
    /// `|b_F - K_FF u_F| / |b_F|`.
    pub relative_residual: f64,
    /// The rank of every free equation.
    pub part_of_equation: Vec<usize>,
    /// Per rank: owned equations.
    pub owned_equations: Vec<usize>,
    /// Per rank: owned nodes.
    pub owned_nodes: Vec<usize>,
    /// Per rank: ghost columns.
    pub ghosts: Vec<usize>,
    /// Per rank: the shift of the incomplete Cholesky factor (block Jacobi: `None`).
    pub shifts: Vec<Option<f64>>,
    /// Wall time of the rank solves (maximum over ranks, after the matrices are built) [s].
    pub solve_seconds: f64,
    /// Wall time of the preparation (assembly, elimination) [s].
    pub assembly_seconds: f64,
}

struct FaultComm<'a> {
    inner: &'a dyn Communicator,
    fault: Option<RankFault>,
    sends: AtomicUsize,
}

impl Communicator for FaultComm<'_> {
    fn rank(&self) -> usize {
        self.inner.rank()
    }
    fn size(&self) -> usize {
        self.inner.size()
    }
    fn send(&self, dest: usize, tag: Tag, data: &[f64]) -> std::result::Result<(), CommError> {
        let k = self.sends.fetch_add(1, Ordering::SeqCst) + 1;
        let me = self.inner.rank();
        match self.fault {
            Some(RankFault::Fail { rank, after_sends }) if rank == me && k >= after_sends => {
                return Err(CommError {
                    rank: me,
                    peer: dest,
                    tag,
                    reason: format!("injected failure at send {k} (RankFault::Fail)"),
                });
            }
            Some(RankFault::Stall {
                rank,
                after_sends,
                delay,
            }) if rank == me && k == after_sends => std::thread::sleep(delay),
            _ => {}
        }
        self.inner.send(dest, tag, data)
    }
    fn recv(&self, src: usize, tag: Tag) -> std::result::Result<Vec<f64>, CommError> {
        self.inner.recv(src, tag)
    }
}

struct RankOut {
    x: Vec<f64>,
    iterations: usize,
    restarts: usize,
    shift: Option<f64>,
    seconds: f64,
}

fn rank_preconditioner(
    sys: &DistCsr,
    blocks: &[usize],
    which: &RankPreconditioner,
) -> std::result::Result<(Box<dyn Preconditioner>, Option<f64>), String> {
    let owned = sys.owned_block();
    match which {
        RankPreconditioner::BlockJacobi => BlockJacobi::new(&owned, blocks)
            .map(|m| (Box::new(m) as Box<dyn Preconditioner>, None))
            .map_err(|e| format!("block Jacobi of the owned rows: {e}")),
        RankPreconditioner::IncompleteCholesky(o) => IncompleteCholesky::new(&owned, o)
            .map(|m| {
                let s = m.shift();
                (Box::new(m) as Box<dyn Preconditioner>, Some(s))
            })
            .map_err(|e| format!("incomplete Cholesky of the owned rows: {e}")),
    }
}

/// The outer loop of ADR 0282 D5' on one rank: every decision from globally reduced numbers.
fn rank_loop(
    op: &DistOperator,
    comm: &dyn Communicator,
    b: &[f64],
    m: &dyn Preconditioner,
    tolerance: f64,
    max_iterations: usize,
) -> std::result::Result<(Vec<f64>, usize, usize), String> {
    let cs = |e: CommError| e.to_string();
    let n = b.len();
    let target = tolerance * gnorm2(comm, b).map_err(cs)?;
    let mut x = vec![0.0; n];
    let mut used = 0;
    let mut restarts = 0;
    let mut r = vec![0.0; n];
    loop {
        let control = SolveControl {
            tol_abs: target,
            tol_rel: 0.0,
            max_iter: max_iterations - used,
        };
        let s = if restarts == 0 {
            krylov::cg(op, b, &mut x, m, &control).map_err(cs)?
        } else {
            let mut delta = vec![0.0; n];
            let s = krylov::cg(op, &r, &mut delta, m, &control).map_err(cs)?;
            for (xi, di) in x.iter_mut().zip(&delta) {
                *xi += di;
            }
            s
        };
        used += s.iterations;
        let bad_local = if x.iter().any(|v| !v.is_finite()) { 1.0 } else { 0.0 };
        let bad = comm.allreduce_max(bad_local).map_err(cs)?;
        if !s.final_residual.is_finite() || bad > 0.0 {
            return Err(format!("non-finite iterate after {used} iterations"));
        }
        op.try_apply(&x, &mut r).map_err(cs)?;
        for (ri, bi) in r.iter_mut().zip(b) {
            *ri = bi - *ri;
        }
        let true_res = gnorm2(comm, &r).map_err(cs)?;
        if true_res <= target {
            return Ok((x, used, restarts));
        }
        if !s.converged || restarts == MAX_RESTARTS || used >= max_iterations {
            return Err(format!(
                "not converged in {used} iterations and {restarts} restart(s) (true residual {true_res:e}, target \
                 {target:e} = {tolerance:e} relative)"
            ));
        }
        restarts += 1;
    }
}

/// The static solve of a model on `options.ranks` in-process ranks (ADR 0431 D4). The result carries the name
/// `"<F-381 name> on <p> rank(s)"`; with one rank every number has the bits of `rcfd_fem::solve::solve_static_report` with
/// the same preconditioner, tolerance and cap.
pub fn solve_static_ranks(
    model: &SolidModel,
    prescribed: &[Prescribed],
    forces: &[NodalForceAt],
    options: &RankOptions,
) -> Result<(SolidResult, RankReport)> {
    let prep = prepare(model, prescribed, forces, options.threads, RANK_SOLVER)?;
    let free = prep.sys.free().to_vec();
    let nf = free.len();
    let p = options.ranks;
    let base = match options.preconditioner {
        RankPreconditioner::BlockJacobi => "cg-block-jacobi",
        RankPreconditioner::IncompleteCholesky(_) => "cg-ic0",
    };
    let name = format!("{base} on {p} rank(s)");
    if nf == 0 {
        let (result, rr) = finish(&prep, &[], &name, 0)?;
        return Ok((
            result,
            RankReport {
                solver: name,
                ranks: p,
                iterations: 0,
                restarts: 0,
                relative_residual: rr,
                part_of_equation: Vec::new(),
                owned_equations: Vec::new(),
                owned_nodes: Vec::new(),
                ghosts: Vec::new(),
                shifts: Vec::new(),
                solve_seconds: 0.0,
                assembly_seconds: prep.assembly_seconds,
            },
        ));
    }
    let part = node_partition(&free, &prep.dofs, model.mesh().nodes(), p)?;
    let systems = RankSystems::build(&prep.sys.matrix, &free, &prep.dofs, &part, p)?;
    let rhs = &prep.sys.rhs;
    let run = |c: &dyn Communicator| -> std::result::Result<RankOut, String> {
        let comm = FaultComm {
            inner: c,
            fault: options.fault,
            sends: AtomicUsize::new(0),
        };
        let r = comm.rank();
        let sys = &systems.ranks[r];
        let (m, shift) = rank_preconditioner(sys, &systems.node_blocks[r], &options.preconditioner)?;
        let b: Vec<f64> = sys.owned.iter().map(|&g| rhs[g]).collect();
        let op = DistOperator::new(sys, &comm);
        let t0 = Instant::now();
        let (x, iterations, restarts) =
            rank_loop(&op, &comm, &b, m.as_ref(), options.tolerance, options.max_iterations)?;
        Ok(RankOut {
            x,
            iterations,
            restarts,
            shift,
            seconds: t0.elapsed().as_secs_f64(),
        })
    };
    let per_rank = run_ranks_with_timeout(p, options.timeout, |c| run(c));
    let failures: Vec<String> = per_rank
        .iter()
        .enumerate()
        .filter_map(|(r, o)| o.as_ref().err().map(|e| format!("rank {r}: {e}")))
        .collect();
    if !failures.is_empty() {
        return Err(FemError::Solver {
            solver: RANK_SOLVER,
            reason: format!("{} of {p} rank(s) failed: {}", failures.len(), failures.join("; ")),
        });
    }
    let outs: Vec<RankOut> = per_rank.into_iter().map(|o| o.expect("checked")).collect();
    let (iterations, restarts) = (outs[0].iterations, outs[0].restarts);
    if let Some(r) = outs
        .iter()
        .position(|o| o.iterations != iterations || o.restarts != restarts)
    {
        return Err(FemError::Solver {
            solver: RANK_SOLVER,
            reason: format!(
                "ranks disagree: rank 0 took {iterations} iterations and {restarts} restart(s), rank {r} {} and {}",
                outs[r].iterations, outs[r].restarts
            ),
        });
    }
    let mut u_free = vec![0.0; nf];
    for (sys, o) in systems.ranks.iter().zip(&outs) {
        for (&g, &v) in sys.owned.iter().zip(&o.x) {
            u_free[g] = v;
        }
    }
    let (result, relative_residual) = finish(&prep, &u_free, &name, iterations)?;
    let report = RankReport {
        solver: name,
        ranks: p,
        iterations,
        restarts,
        relative_residual,
        part_of_equation: part,
        owned_equations: systems.ranks.iter().map(|s| s.n_owned).collect(),
        owned_nodes: systems.owned_nodes.clone(),
        ghosts: systems.ranks.iter().map(|s| s.n_local - s.n_owned).collect(),
        shifts: outs.iter().map(|o| o.shift).collect(),
        solve_seconds: outs.iter().fold(0.0, |s, o| s.max(o.seconds)),
        assembly_seconds: prep.assembly_seconds,
    };
    Ok((result, report))
}

// ---------------------------------------------------------------------------------------------------------------------
// The device solve (ADR 0431 D6).

/// The preconditioner of the device solve (both have a CPU twin).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DevicePreconditioner {
    /// The inverse diagonal.
    Jacobi,
    /// Chebyshev polynomial of the Jacobi-scaled matrix (`degree` 0: the default of `rcfd-linalg`).
    Chebyshev {
        /// Number of terms.
        degree: usize,
    },
}

/// Options of [`solve_static_device`].
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceOptions {
    /// `cpu`, `cuda` or `auto` (threshold `rcfd_gpu::AUTO_MIN_UNKNOWNS` on the free unknowns).
    pub device: LinearDevice,
    /// The preconditioner.
    pub preconditioner: DevicePreconditioner,
    /// Relative tolerance on the true residual.
    pub tolerance: f64,
    /// Iteration cap, summed over restarts.
    pub max_iterations: usize,
    /// Worker threads of the assembly.
    pub threads: Option<usize>,
}

impl DeviceOptions {
    /// The given device, Jacobi, tolerance 1e-10, 50 000 iterations.
    pub fn new(device: LinearDevice) -> DeviceOptions {
        DeviceOptions {
            device,
            preconditioner: DevicePreconditioner::Jacobi,
            tolerance: 1e-10,
            max_iterations: 50_000,
            threads: None,
        }
    }
}

/// What a device solve did.
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceReport {
    /// The solver name, as in the result's [`SolverStats`].
    pub solver: String,
    /// The backend that produced the result: `"cuda"` or `"cpu"`.
    pub backend: String,
    /// Why that backend (the reason of `rcfd_gpu::resolve`, and of a fall-back).
    pub reason: String,
    /// Iterations summed over restarts.
    pub iterations: usize,
    /// Restarts from the true residual.
    pub restarts: usize,
    /// `|b_F - K_FF u_F| / |b_F|`.
    pub relative_residual: f64,
    /// Wall time of the solve, uploads and downloads included [s].
    pub solve_seconds: f64,
    /// Wall time of the preparation [s].
    pub assembly_seconds: f64,
}

/// The outer loop of ADR 0282 D5' around a backend's CG.
fn backend_loop(
    backend: &dyn LinearBackend,
    a: &CsrMatrix,
    b: &[f64],
    lo: &LinearOptions,
    tolerance: f64,
    max_iterations: usize,
) -> std::result::Result<(Vec<f64>, usize, usize), String> {
    let n = b.len();
    let target = tolerance * norm2(b);
    let mut x = vec![0.0; n];
    let mut used = 0;
    let mut restarts = 0;
    let mut r = vec![0.0; n];
    let be = |e: rcfd_linalg::BackendError| format!("{} backend: {}", backend.name(), e.message);
    loop {
        let control = SolveControl {
            tol_abs: target,
            tol_rel: 0.0,
            max_iter: max_iterations - used,
        };
        let s = if restarts == 0 {
            backend.cg(a, b, &mut x, &control, lo).map_err(be)?
        } else {
            let mut delta = vec![0.0; n];
            let s = backend.cg(a, &r, &mut delta, &control, lo).map_err(be)?;
            for (xi, di) in x.iter_mut().zip(&delta) {
                *xi += di;
            }
            s
        };
        used += s.iterations;
        if !s.final_residual.is_finite() || x.iter().any(|v| !v.is_finite()) {
            return Err(format!("non-finite iterate after {used} iterations"));
        }
        a.residual(b, &x, &mut r);
        let true_res = norm2(&r);
        if true_res <= target {
            return Ok((x, used, restarts));
        }
        if !s.converged || restarts == MAX_RESTARTS || used >= max_iterations {
            return Err(format!(
                "not converged in {used} iterations and {restarts} restart(s) (true residual {true_res:e}, target \
                 {target:e} = {tolerance:e} relative)"
            ));
        }
        restarts += 1;
    }
}

/// The static solve of a model with the CG of `rcfd-gpu` on the device chosen by `options.device` (ADR 0431 D6). `cuda`
/// without a usable device, or a failing device solve under `cuda`, is a located error; `auto` falls back to the CPU and
/// records why in [`DeviceReport::reason`].
pub fn solve_static_device(
    model: &SolidModel,
    prescribed: &[Prescribed],
    forces: &[NodalForceAt],
    options: &DeviceOptions,
) -> Result<(SolidResult, DeviceReport)> {
    let prep = prepare(model, prescribed, forces, options.threads, DEVICE_SOLVER)?;
    let nf = prep.sys.free().len();
    let (precond, degree, base) = match options.preconditioner {
        DevicePreconditioner::Jacobi => (GpuPrecond::Jacobi, 0, "cg-jacobi"),
        DevicePreconditioner::Chebyshev { degree } => (GpuPrecond::Chebyshev, degree, "cg-chebyshev"),
    };
    let lo = LinearOptions {
        device: options.device,
        precond,
        chebyshev_degree: degree,
    };
    let resolution = rcfd_gpu::resolve(options.device, nf).map_err(|e| FemError::Solver {
        solver: DEVICE_SOLVER,
        reason: e.message,
    })?;
    let mut reason = resolution.reason;
    let (a, b) = (&prep.sys.matrix, &prep.sys.rhs);
    let t0 = Instant::now();
    let cpu = |reason: String| -> Result<(Vec<f64>, usize, usize, &'static str, String)> {
        if nf == 0 {
            return Ok((Vec::new(), 0, 0, "cpu", reason));
        }
        let (x, it, rs) =
            backend_loop(&CpuBackend, a, b, &lo, options.tolerance, options.max_iterations).map_err(|e| {
                FemError::Solver {
                    solver: DEVICE_SOLVER,
                    reason: e,
                }
            })?;
        Ok((x, it, rs, "cpu", reason))
    };
    let (u_free, iterations, restarts, backend, why) = match resolution.backend {
        Some(dev) if nf > 0 => match backend_loop(dev, a, b, &lo, options.tolerance, options.max_iterations) {
            Ok((x, it, rs)) => (x, it, rs, "cuda", reason),
            Err(e) if options.device == LinearDevice::Auto => {
                rcfd_gpu::note_fallback(&e);
                reason = format!("{reason}; the device solve failed ({e}), falling back to cpu");
                cpu(reason)?
            }
            Err(e) => {
                return Err(FemError::Solver {
                    solver: DEVICE_SOLVER,
                    reason: format!("device = \"cuda\": the device solve failed: {e}"),
                })
            }
        },
        _ => cpu(reason)?,
    };
    let solve_seconds = t0.elapsed().as_secs_f64();
    let name = format!("{base} ({backend})");
    let (result, relative_residual) = finish(&prep, &u_free, &name, iterations)?;
    Ok((
        result,
        DeviceReport {
            solver: name,
            backend: backend.to_string(),
            reason: why,
            iterations,
            restarts,
            relative_residual,
            solve_seconds,
            assembly_seconds: prep.assembly_seconds,
        },
    ))
}
