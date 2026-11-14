//! ADR 0431 (F-433): elasticity solves on the ranks of rcfd-dist. Checks R1 to R11, the negative controls N1, N2, N4 and
//! the no-device checks D3, D4 of the registration. Run with `--test-threads=1` so that the recorded times (R11) are not
//! disturbed by the other tests of this file.

use rcfd_api::solid_parallel::{
    node_partition, solve_static_device, solve_static_ranks, DeviceOptions, RankFault, RankOptions, RankPreconditioner,
    RankReport, RankSystems,
};
use rcfd_fem::builders::{hex_box, tet_box, upgrade_to_tet10};
use rcfd_fem::constraint::{prescribed_equations, reduce};
use rcfd_fem::solve::{solve_static_report, CgPreconditioner, SolveReport, StaticOptions, StaticSolver};
use rcfd_fem::{Constraint, Load, Material, SolidModel, SolidResult};
use rcfd_linalg::precond::IcOptions;
use rcfd_linalg::LinearDevice;
use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

const E_MOD: f64 = 2.0e11;
const NU: f64 = 0.3;
const TOL: f64 = 1e-10;
const CAP: usize = 20_000;
const DIFF_LIMIT: f64 = 1e-8;
const RATIO_LIMIT: f64 = 2.0;

fn steel(m: &mut SolidModel) {
    m.add_material("all", Material::isotropic_elastic(E_MOD, NU, 0.0, 0.0).unwrap())
        .unwrap();
}

fn clamp_xmin(m: &mut SolidModel) {
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
}

/// D7: the clamped cube, `n` divisions per side.
fn cube(n: usize) -> SolidModel {
    let mesh = hex_box([0.0; 3], [1.0; 3], [n; 3]).unwrap();
    let mut m = SolidModel::new(mesh);
    steel(&mut m);
    clamp_xmin(&mut m);
    m.add_load(Load::traction("xmax", [2.0e6, 1.0e6, -1.5e6])).unwrap();
    m
}

/// D7: the Tet10 cantilever.
fn tet10_cantilever() -> SolidModel {
    let t4 = tet_box([0.0; 3], [8.0, 1.5, 1.5], [16, 3, 3]).unwrap();
    let (mesh, _) = upgrade_to_tet10(&t4).unwrap();
    let mut m = SolidModel::new(mesh);
    steel(&mut m);
    clamp_xmin(&mut m);
    m.add_load(Load::traction("xmax", [0.0, 0.0, -1.0e6])).unwrap();
    m
}

fn ic() -> IcOptions {
    IcOptions::default()
}

fn serial(model: &SolidModel, p: CgPreconditioner, tol: f64) -> (SolidResult, SolveReport) {
    let o = StaticOptions::with_solver(StaticSolver::PreconditionedCg {
        preconditioner: p,
        tolerance: tol,
        max_iterations: CAP,
    });
    solve_static_report(model, &[], &[], &o).unwrap()
}

fn ranks(model: &SolidModel, p: usize, pre: RankPreconditioner, tol: f64) -> (SolidResult, RankReport) {
    let mut o = RankOptions::new(p);
    o.preconditioner = pre;
    o.tolerance = tol;
    o.max_iterations = CAP;
    solve_static_ranks(model, &[], &[], &o).unwrap()
}

fn rel_diff(a: &SolidResult, b: &SolidResult) -> f64 {
    let (x, y) = (a.displacement_values(), b.displacement_values());
    let m = y.iter().fold(0.0f64, |s, v| s.max(v.abs()));
    x.iter().zip(y).fold(0.0f64, |s, (p, q)| s.max((p - q).abs())) / m
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

fn same_bits(a: &SolidResult, b: &SolidResult) -> bool {
    bits(a.displacement_values()) == bits(b.displacement_values())
}

/// R8: every node's free equations on one rank, counts summing, every rank owning a node.
fn split_ok(model: &SolidModel, rep: &RankReport) -> bool {
    let sys = reduce_of(model);
    let dofs = model.dof_map();
    let mut rank_of: HashMap<usize, usize> = HashMap::new();
    for (g, &q) in sys.iter().enumerate() {
        let (n, _) = dofs.node_component(q).unwrap();
        let r = rep.part_of_equation[g];
        if *rank_of.entry(n).or_insert(r) != r {
            return false;
        }
    }
    rep.owned_equations.iter().sum::<usize>() == sys.len()
        && rep.owned_nodes.len() == rep.ranks
        && rep.owned_nodes.iter().all(|&k| k > 0)
}

fn reduce_of(model: &SolidModel) -> Vec<usize> {
    let fixed = prescribed_equations(model, &[]).unwrap();
    let k = rcfd_fem::assemble::stiffness_with(model, None).unwrap();
    let f = vec![0.0; k.n];
    reduce(&k, &f, &fixed).unwrap().free().to_vec()
}

struct Case {
    model: SolidModel,
    serial_bj: (SolidResult, SolveReport),
    serial_ic: (SolidResult, SolveReport),
}

fn cube_case() -> &'static Case {
    static C: OnceLock<Case> = OnceLock::new();
    C.get_or_init(|| {
        let model = cube(23);
        let serial_bj = serial(&model, CgPreconditioner::BlockJacobi, TOL);
        let serial_ic = serial(&model, CgPreconditioner::IncompleteCholesky(ic()), TOL);
        Case {
            model,
            serial_bj,
            serial_ic,
        }
    })
}

fn tet_case() -> &'static Case {
    static C: OnceLock<Case> = OnceLock::new();
    C.get_or_init(|| {
        let model = tet10_cantilever();
        let serial_bj = serial(&model, CgPreconditioner::BlockJacobi, TOL);
        let serial_ic = serial(&model, CgPreconditioner::IncompleteCholesky(ic()), TOL);
        Case {
            model,
            serial_bj,
            serial_ic,
        }
    })
}

fn one_rank_bits(tag: &str, case: &Case, pre: RankPreconditioner) -> bool {
    let (s, srep) = match pre {
        RankPreconditioner::BlockJacobi => &case.serial_bj,
        _ => &case.serial_ic,
    };
    let (r, rep) = ranks(&case.model, 1, pre, TOL);
    let it_ok = rep.iterations == srep.iterations && r.stats().unwrap().iterations() == s.stats().unwrap().iterations();
    let u_ok = same_bits(&r, s);
    let re_ok = bits(r.reactions().unwrap()) == bits(s.reactions().unwrap());
    let res_ok = r.stats().unwrap().relative_residual().to_bits() == s.stats().unwrap().relative_residual().to_bits();
    let rs_ok = rep.restarts == srep.restarts;
    println!(
        "{tag}: {} unknowns, serial '{}' {} iterations {} restarts residual {:.3e} shift {:?}; 1 rank '{}' {} iterations {} \
         restarts residual {:.3e} shifts {:?}; iterations equal {it_ok}, restarts equal {rs_ok}, displacements bit-equal \
         {u_ok}, reactions bit-equal {re_ok}, residual bit-equal {res_ok}",
        r.stats().unwrap().unknowns(),
        srep.solver,
        srep.iterations,
        srep.restarts,
        srep.relative_residual,
        srep.shift,
        rep.solver,
        rep.iterations,
        rep.restarts,
        rep.relative_residual,
        rep.shifts
    );
    let shift_ok = match pre {
        RankPreconditioner::BlockJacobi => true,
        _ => rep.shifts[0] == srep.shift,
    };
    it_ok && u_ok && re_ok && res_ok && rs_ok && shift_ok
}

#[test]
fn r1_cube_block_jacobi_one_rank_has_the_serial_bits() {
    assert!(one_rank_bits(
        "R1 cube block Jacobi",
        cube_case(),
        RankPreconditioner::BlockJacobi
    ));
}

#[test]
fn r2_tet10_block_jacobi_one_rank_has_the_serial_bits() {
    assert!(one_rank_bits(
        "R2 Tet10 block Jacobi",
        tet_case(),
        RankPreconditioner::BlockJacobi
    ));
}

#[test]
fn r3_cube_ic0_one_rank_has_the_serial_bits() {
    assert!(one_rank_bits(
        "R3 cube IC(0)",
        cube_case(),
        RankPreconditioner::IncompleteCholesky(ic())
    ));
}

/// R4 to R8 for one case and preconditioner over P = 2, 4, 8; `repeat` lists the P whose run is repeated (R7).
fn multi_rank(tag: &str, case: &Case, pre: RankPreconditioner, repeat: &[usize], times: bool) -> bool {
    let s = match pre {
        RankPreconditioner::BlockJacobi => &case.serial_bj.0,
        _ => &case.serial_ic.0,
    };
    let (_, rep1) = ranks(&case.model, 1, pre, TOL);
    if times {
        println!(
            "R11 {tag} P = 1: {} iterations, solve {:.3} s (assembly {:.3} s)",
            rep1.iterations, rep1.solve_seconds, rep1.assembly_seconds
        );
    }
    let mut ok = true;
    for p in [2usize, 4, 8] {
        let (r, rep) = ranks(&case.model, p, pre, TOL);
        let d = rel_diff(&r, s);
        let res = rep.relative_residual;
        let ratio = rep.iterations as f64 / rep1.iterations as f64;
        let split = split_ok(&case.model, &rep);
        println!(
            "{tag} P = {p}: difference to serial {d:.3e} (limit {DIFF_LIMIT:e}), residual {res:.3e} (limit {TOL:e}), \
             iterations {} / one rank {} = {ratio:.4} (limit {RATIO_LIMIT}), restarts {}, owned equations {:?}, ghosts {:?}, \
             shifts {:?}; R8 split check {split}",
            rep.iterations, rep1.iterations, rep.restarts, rep.owned_equations, rep.ghosts, rep.shifts
        );
        if times {
            println!("R11 {tag} P = {p}: solve {:.3} s", rep.solve_seconds);
        }
        ok &= d <= DIFF_LIMIT && res <= TOL && ratio <= RATIO_LIMIT && split;
        if repeat.contains(&p) {
            let (again, rep2) = ranks(&case.model, p, pre, TOL);
            let same = same_bits(&again, &r) && rep2.iterations == rep.iterations;
            println!("R7 {tag} P = {p}: repeated run bit-identical {same}");
            ok &= same;
        }
    }
    ok
}

#[test]
fn r4_r7_r8_r11_cube_block_jacobi_on_2_4_8_ranks() {
    assert!(multi_rank(
        "R4 cube block Jacobi",
        cube_case(),
        RankPreconditioner::BlockJacobi,
        &[2, 4, 8],
        true
    ));
}

#[test]
fn r5_r7_r8_tet10_block_jacobi_on_2_4_8_ranks() {
    assert!(multi_rank(
        "R5 Tet10 block Jacobi",
        tet_case(),
        RankPreconditioner::BlockJacobi,
        &[2, 4, 8],
        false
    ));
}

#[test]
fn r6_r7_r8_cube_rank_local_ic0_on_2_4_8_ranks() {
    let pre = RankPreconditioner::IncompleteCholesky(ic());
    assert!(multi_rank("R6 cube IC(0)", cube_case(), pre, &[8], false));
}

#[test]
fn r6_r7_r8_tet10_rank_local_ic0_on_2_4_8_ranks() {
    let pre = RankPreconditioner::IncompleteCholesky(ic());
    assert!(multi_rank("R6 Tet10 IC(0)", tet_case(), pre, &[8], false));
}

fn faulty(fault: RankFault, timeout: Duration) -> (String, f64) {
    let model = &tet_case().model;
    let mut o = RankOptions::new(4);
    o.fault = Some(fault);
    o.timeout = timeout;
    let t = Instant::now();
    let r = solve_static_ranks(model, &[], &[], &o);
    let secs = t.elapsed().as_secs_f64();
    let text = match r {
        Ok(_) => "Ok (no error)".to_string(),
        Err(e @ rcfd_fem::FemError::Solver { .. }) => format!("Solver error: {e}"),
        Err(e) => format!("other error: {e}"),
    };
    (text, secs)
}

#[test]
fn r9_a_failing_rank_ends_the_solve_with_a_located_error() {
    let (text, secs) = faulty(
        RankFault::Fail {
            rank: 1,
            after_sends: 200,
        },
        Duration::from_secs(60),
    );
    println!("R9: {secs:.3} s (limit 10 s): {text}");
    assert!(text.starts_with("Solver error") && text.contains("rank 1") && text.contains("injected failure"));
    assert!(secs <= 10.0);
}

#[test]
fn r10_a_stalling_rank_ends_the_solve_with_a_located_time_out() {
    let (text, secs) = faulty(
        RankFault::Stall {
            rank: 2,
            after_sends: 200,
            delay: Duration::from_secs(3),
        },
        Duration::from_millis(500),
    );
    println!("R10: {secs:.3} s (limit 10 s): {text}");
    assert!(text.starts_with("Solver error") && text.contains("no message within") && text.contains("rank "));
    assert!(secs <= 10.0);
}

#[test]
fn n1_a_split_node_is_refused_naming_the_node() {
    let model = cube(4);
    let fixed = prescribed_equations(&model, &[]).unwrap();
    let k = rcfd_fem::assemble::stiffness_with(&model, None).unwrap();
    let f = vec![0.0; k.n];
    let sys = reduce(&k, &f, &fixed).unwrap();
    let dofs = model.dof_map();
    let free = sys.free();
    let mut part = node_partition(free, &dofs, model.mesh().nodes(), 2).unwrap();
    // component 2 of the node of free equation 2 (first node: components 0, 1, 2) moved to the other rank
    let (node, comp) = dofs.node_component(free[2]).unwrap();
    assert_eq!(comp, 2);
    part[2] = 1 - part[2];
    let e = RankSystems::build(&sys.matrix, free, &dofs, &part, 2)
        .unwrap_err()
        .to_string();
    println!("N1: {e}");
    assert!(e.contains(&format!("node {node} is split")), "{e}");
    // the unmodified assignment is accepted
    let ok = node_partition(free, &dofs, model.mesh().nodes(), 2).unwrap();
    assert!(RankSystems::build(&sys.matrix, free, &dofs, &ok, 2).is_ok());
}

#[test]
fn n2_a_loose_tolerance_is_detected_by_the_comparison() {
    let case = cube_case();
    let (r, _) = ranks(&case.model, 2, RankPreconditioner::BlockJacobi, 1e-4);
    let d = rel_diff(&r, &case.serial_bj.0);
    println!("N2: P = 2 at tolerance 1e-4 differs from serial by {d:.3e} (expected > 1e-8)");
    assert!(d > DIFF_LIMIT);
}

#[test]
fn n4_a_fault_aimed_at_no_rank_is_inert() {
    let case = tet_case();
    let mut o = RankOptions::new(4);
    let (clean, _) = solve_static_ranks(&case.model, &[], &[], &o).unwrap();
    o.fault = Some(RankFault::Fail {
        rank: 9,
        after_sends: 1,
    });
    let (aimed, _) = solve_static_ranks(&case.model, &[], &[], &o).unwrap();
    let same = same_bits(&clean, &aimed);
    println!("N4: fault on rank 9 of 4, result bit-equal to the fault-free run: {same}");
    assert!(same);
}

fn device_run(
    model: &SolidModel,
    d: LinearDevice,
) -> rcfd_fem::Result<(SolidResult, rcfd_api::solid_parallel::DeviceReport)> {
    solve_static_device(model, &[], &[], &DeviceOptions::new(d))
}

#[test]
fn d3_d4_without_a_device_cuda_is_refused_and_auto_falls_back() {
    let status = rcfd_gpu::probe();
    let reason = match status {
        rcfd_gpu::DeviceStatus::Unavailable { reason } => reason.clone(),
        rcfd_gpu::DeviceStatus::Available(_) => {
            println!(
                "D3/D4 NOT RUN in this build: {} (run the default build or RCFD_GPU_DISABLE=1)",
                status.describe()
            );
            return;
        }
    };
    let model = cube(6);
    let e = device_run(&model, LinearDevice::Cuda).map(|_| ()).unwrap_err();
    let text = e.to_string();
    let d3 = matches!(e, rcfd_fem::FemError::Solver { .. }) && text.contains("cuda") && text.contains(&reason);
    println!("D3: device = cuda without a device: {text}; met {d3}");
    let (cpu, crep) = device_run(&model, LinearDevice::Cpu).unwrap();
    let (auto, arep) = device_run(&model, LinearDevice::Auto).unwrap();
    let d4 = arep.backend == "cpu"
        && arep.reason.contains("falling back")
        && arep.reason.contains(&reason)
        && same_bits(&auto, &cpu);
    println!(
        "D4: auto without a device: backend {}, reason '{}', bit-equal to cpu {} ({} iterations, cpu reason '{}'); met {d4}",
        arep.backend,
        arep.reason,
        same_bits(&auto, &cpu),
        arep.iterations,
        crep.reason
    );
    assert!(d3 && d4);
}
