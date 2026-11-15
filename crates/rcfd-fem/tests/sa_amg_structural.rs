//! F-629 (ADR 0552): six-unknown nodes in the smoothed-aggregation CG. The checks of the registration in its order: NN1 to NN3
//! (the six rigid fields with their rotation components), IT1 to IT5 (the plate and cantilever models of ADR 0432 and ADR 0454
//! and the Beam2 cantilever BM, against the direct solve, at the default settings S0 and the forced hierarchy S1), NC1 (the
//! rotation components change the count), B2a, B2b (the solid-only bits), R1 (the transformation path). Every limit is the
//! registered one; every estimate is that of `f629-scratch/estimate2.log`.

use rcfd_core::vec3::Vec3;
use rcfd_fem::assemble::stiffness;
use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::{prescribed_equations, reduce, Prescribed};
use rcfd_fem::loads::{nodal_load_vector, NodalForceAt};
use rcfd_fem::solve::{
    node_blocks, smoothed_aggregation_structural, solve_static_report, solve_static_with, structural_rigid_body_modes,
    CgPreconditioner, StaticOptions, StaticSolver, STRUCTURAL_AMG_NAME,
};
use rcfd_fem::{
    BeamSection, BlockProperty, Constraint, ElasticSupport, ElementKind, FemError, Load, Material, SolidMesh,
    SolidModel,
};
use rcfd_linalg::amg_elastic::{rigid_body_modes, ElasticAmg, ElasticAmgSettings};
use rcfd_linalg::vecops::norm2;
use rcfd_linalg::{cg, SolveControl};

struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value <= limit && value.is_finite();
        println!(
            "{name}: {value:.4e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    fn at_least(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value >= limit && value.is_finite();
        println!(
            "{name}: {value:.4e} (limit >= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} < {limit:e}"));
        }
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {detail} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name}: {detail}"));
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:#?}", self.0);
    }
}

/// NaN-propagating maximum of `|v|`.
fn max_abs(v: &[f64]) -> f64 {
    v.iter().fold(0.0f64, |m, x| {
        if m.is_nan() || x.is_nan() {
            f64::NAN
        } else {
            m.max(x.abs())
        }
    })
}

const E: f64 = 2.0e11;
const RHO: f64 = 7850.0;
/// ADR 0552 arithmetic 2: the true relative residual of the CG and its iteration cap.
const TAU: f64 = 1e-9;
const MAX_IT: usize = 5000;

fn s0() -> ElasticAmgSettings {
    ElasticAmgSettings::default()
}

/// ADR 0552 arithmetic 1: the forced hierarchy.
fn s1() -> ElasticAmgSettings {
    ElasticAmgSettings {
        max_coarse: 60,
        ..ElasticAmgSettings::default()
    }
}

fn structural(settings: ElasticAmgSettings) -> StaticOptions {
    StaticOptions::with_solver(StaticSolver::PreconditionedCg {
        preconditioner: CgPreconditioner::SmoothedAggregationStructural(settings),
        tolerance: TAU,
        max_iterations: MAX_IT,
    })
}

// ------------------------------------------------------------------------------------------------------- the models

/// A grid of `nx x ny` cells on `[0, a] x [0, b]` at z = 0, nodes in rows of constant y; cell nodes `(i, j), (i + 1, j), (i +
/// 1, j + 1), (i, j + 1)` (the grid of `si_shell.rs` and `sa_shell3.rs`).
fn grid(a: f64, b: f64, nx: usize, ny: usize) -> (Vec<Vec3>, Vec<[usize; 4]>) {
    let idx = |i: usize, j: usize| j * (nx + 1) + i;
    let mut nodes = Vec::new();
    for j in 0..=ny {
        for i in 0..=nx {
            nodes.push([a * i as f64 / nx as f64, b * j as f64 / ny as f64, 0.0]);
        }
    }
    let mut el = Vec::new();
    for j in 0..ny {
        for i in 0..nx {
            el.push([idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1)]);
        }
    }
    (nodes, el)
}

/// One shell block (Shell4 on the cells, or Shell3 with each cell cut along `q0-q2`, diagonal 0 of `sa_shell3.rs`), element
/// set "all", node set "all".
fn shell_model(nodes: Vec<Vec3>, cells: &[[usize; 4]], tri: bool, t: f64, nu: f64) -> SolidModel {
    let n = nodes.len();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    if tri {
        let conn: Vec<usize> = cells
            .iter()
            .flat_map(|q| [q[0], q[1], q[2], q[0], q[2], q[3]])
            .collect();
        mesh.add_block(ElementKind::Shell3, conn).unwrap();
        mesh.add_element_set("all", (0..2 * cells.len()).collect()).unwrap();
    } else {
        mesh.add_block(ElementKind::Shell4, cells.iter().flatten().copied().collect())
            .unwrap();
        mesh.add_element_set("all", (0..cells.len()).collect()).unwrap();
    }
    mesh.set_block_property(0, BlockProperty::shell(t).unwrap()).unwrap();
    mesh.add_node_set("all", (0..n).collect()).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(E, nu, RHO, 0.0).unwrap())
        .unwrap();
    m
}

struct Case {
    name: &'static str,
    model: SolidModel,
    prescribed: Vec<Prescribed>,
    forces: Vec<NodalForceAt>,
}

/// P4 / P3: the simply supported plate of ADR 0432 arithmetic 11 (Shell4 16 x 16) and ADR 0454 arithmetic 10 (Shell3 32 x 32).
fn ss_plate(name: &'static str, tri: bool, t: f64) -> Case {
    let n = if tri { 32 } else { 16 };
    let (nodes, el) = grid(1.0, 1.0, n, n);
    let mut prescribed = Vec::new();
    for (k, x) in nodes.iter().enumerate() {
        for comp in [0, 1, 5] {
            prescribed.push(Prescribed::new(k, comp, 0.0));
        }
        let onx = x[0] == 0.0 || x[0] == 1.0;
        let ony = x[1] == 0.0 || x[1] == 1.0;
        if onx || ony {
            prescribed.push(Prescribed::new(k, 2, 0.0));
        }
        if onx {
            prescribed.push(Prescribed::new(k, 3, 0.0));
        }
        if ony {
            prescribed.push(Prescribed::new(k, 4, 0.0));
        }
    }
    let mut model = shell_model(nodes, &el, tri, t, 0.3);
    model.add_load(Load::shell_pressure("all", 1000.0)).unwrap();
    Case {
        name,
        model,
        prescribed,
        forces: Vec::new(),
    }
}

/// C4 / C3: the cantilever strip of ADR 0432 arithmetic 12 and ADR 0454 arithmetic 11.
fn strip(name: &'static str, tri: bool) -> Case {
    let n = 15;
    let (nodes, el) = grid(1.0, 0.1, n, 1);
    let model = shell_model(nodes, &el, tri, 0.01, 0.0);
    let mut prescribed = Vec::new();
    for k in [0, n + 1] {
        for comp in 0..6 {
            prescribed.push(Prescribed::new(k, comp, 0.0));
        }
    }
    let forces = [n, 2 * n + 1]
        .iter()
        .map(|&k| NodalForceAt::new(k, [0.0, 0.0, 0.5]))
        .collect();
    Case {
        name,
        model,
        prescribed,
        forces,
    }
}

/// The BM beam of ADR 0552 D3: 40 Beam2 from the origin to (2, 1, 0.6), unconstrained.
fn beam_model() -> SolidModel {
    let ne = 40;
    let end = [2.0, 1.0, 0.6];
    let nodes: Vec<Vec3> = (0..=ne)
        .map(|i| {
            let s = i as f64 / ne as f64;
            [end[0] * s, end[1] * s, end[2] * s]
        })
        .collect();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Beam2, (0..ne).flat_map(|i| [i, i + 1]).collect())
        .unwrap();
    let section = BeamSection::new(0.012, 3.0e-6, 5.0e-6, 4.0e-6, 0.85, 0.75).unwrap();
    mesh.set_block_property(0, BlockProperty::beam(section, [0.0, 0.0, 1.0]).unwrap())
        .unwrap();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(2.1e11, 0.3, RHO, 0.0).unwrap())
        .unwrap();
    m
}

/// BM: node 0 clamped, the tip force (120, -250, 370) N.
fn beam() -> Case {
    let model = beam_model();
    let tip = model.mesh().nodes().len() - 1;
    Case {
        name: "BM",
        model,
        prescribed: (0..6).map(|c| Prescribed::new(0, c, 0.0)).collect(),
        forces: vec![NodalForceAt::new(tip, [120.0, -250.0, 370.0])],
    }
}

/// The models of IT1 to IT5 in the registered order, with the S0 and S1 iteration limits of IT2 and IT4.
fn cases() -> Vec<(Case, usize, usize)> {
    vec![
        (ss_plate("P4a Shell4 plate t/a = 0.01", false, 0.01), 2, 128),
        (ss_plate("P4b Shell4 plate t/a = 0.001", false, 0.001), 2, 797),
        (strip("C4 Shell4 strip", false), 2, 75),
        (ss_plate("P3a Shell3 plate t/a = 0.01", true, 0.01), 48, 75),
        (ss_plate("P3b Shell3 plate t/a = 0.001", true, 0.001), 57, 89),
        (strip("C3 Shell3 strip", true), 2, 35),
        (beam(), 2, 32),
    ]
}

/// The rotation of `angle` about `axis` (Rodrigues), as rows.
fn rotation(axis: Vec3, angle: f64) -> [[f64; 3]; 3] {
    let l = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
    let k = [axis[0] / l, axis[1] / l, axis[2] / l];
    let (s, c) = angle.sin_cos();
    let mut q = [[0.0; 3]; 3];
    let kx = [[0.0, -k[2], k[1]], [k[2], 0.0, -k[0]], [-k[1], k[0], 0.0]];
    for i in 0..3 {
        for j in 0..3 {
            let id = if i == j { 1.0 } else { 0.0 };
            q[i][j] = id * c + s * kx[i][j] + (1.0 - c) * k[i] * k[j];
        }
    }
    q
}

/// NN models: the unconstrained plate `[0, 0.6] x [0, 0.4]` of 4 x 4 cells placed by `Q` (ADR 0432 K1) and the BM beam.
fn nn_models() -> Vec<(&'static str, SolidModel)> {
    let q = rotation([1.0, -2.0, 2.0], 0.7);
    let origin = [0.3, -0.2, 0.5];
    let place = |p: Vec3| -> Vec3 {
        let mut r = origin;
        for i in 0..3 {
            for j in 0..3 {
                r[i] += q[i][j] * p[j];
            }
        }
        r
    };
    let mut out = Vec::new();
    for (name, tri) in [("Shell4 plate in space", false), ("Shell3 plate in space", true)] {
        let (nodes, el) = grid(0.6, 0.4, 4, 4);
        let nodes: Vec<Vec3> = nodes.into_iter().map(place).collect();
        out.push((name, shell_model(nodes, &el, tri, 0.01, 0.3)));
    }
    out.push(("Beam2 BM unconstrained", beam_model()));
    out
}

/// The points and components of the equations `eqs` of `m`.
fn rows(m: &SolidModel, eqs: &[usize]) -> (Vec<[f64; 3]>, Vec<usize>) {
    let dofs = m.dof_map();
    eqs.iter()
        .map(|&q| {
            let (n, c) = dofs.node_component(q).unwrap();
            (m.mesh().nodes()[n], c)
        })
        .unzip()
}

/// `max_j |K b_j|_inf / (max |K| max |b_j|)`.
fn kernel_ratio(k: &rcfd_linalg::CsrMatrix, modes: &[Vec<f64>]) -> f64 {
    let kmax = max_abs(&k.val);
    let mut worst = 0.0f64;
    for b in modes {
        let bmax = max_abs(b);
        if bmax == 0.0 {
            continue;
        }
        let mut kb = vec![0.0; b.len()];
        k.matvec(b, &mut kb);
        let r = max_abs(&kb) / (kmax * bmax);
        worst = if r.is_nan() || worst.is_nan() {
            f64::NAN
        } else {
            worst.max(r)
        };
    }
    worst
}

fn zero_rotation_rows(modes: &mut [Vec<f64>], components: &[usize]) {
    for col in modes.iter_mut() {
        for (r, &c) in components.iter().enumerate() {
            if c >= 3 {
                col[r] = 0.0;
            }
        }
    }
}

// --------------------------------------------------------------------------------------------------------- NN1 to NN3

#[test]
fn nn_six_rigid_fields_with_rotation_components() {
    let mut c = Checks::new();
    for (name, m) in nn_models() {
        let k = stiffness(&m).unwrap();
        let all: Vec<usize> = (0..m.dof_map().n_equations()).collect();
        let (points, components) = rows(&m, &all);
        let modes = structural_rigid_body_modes(&points, &components);
        c.at_most(&format!("NN1 {name}: |K b|/(|K| |b|)"), kernel_ratio(&k, &modes), 1e-12);
        let mut exact = true;
        for (r, &cr) in components.iter().enumerate() {
            if cr >= 3 {
                for (j, col) in modes.iter().enumerate() {
                    let want: f64 = if j == cr { 1.0 } else { 0.0 };
                    exact &= col[r].to_bits() == want.to_bits();
                }
            }
        }
        let n_rot = components.iter().filter(|&&cr| cr >= 3).count();
        c.holds(
            &format!("NN3 {name}: rotation rows exactly delta(c, j)"),
            exact && n_rot > 0,
            &format!("{n_rot} rotation rows"),
        );
        let mut norot = modes.clone();
        zero_rotation_rows(&mut norot, &components);
        c.at_least(
            &format!("NN2 {name}: without rotation components"),
            kernel_ratio(&k, &norot),
            1e-6,
        );
    }
    c.finish();
}

// --------------------------------------------------------------------------------------------------------- IT1 to IT5

fn relative_errors(u: &[f64], ud: &[f64]) -> (f64, f64) {
    let d: Vec<f64> = u.iter().zip(ud).map(|(a, b)| a - b).collect();
    (norm2(&d) / norm2(ud), max_abs(&d) / max_abs(ud))
}

#[test]
fn it_plates_strips_and_beam_against_the_direct_solve() {
    let mut c = Checks::new();
    for (case, cap0, cap1) in cases() {
        let Case {
            name,
            model,
            prescribed,
            forces,
        } = case;
        let direct = solve_static_with(&model, &prescribed, &forces, &StaticOptions::default()).unwrap();
        let ud = direct.displacement_values();
        for (sid, settings, cap, it_err, it_count) in
            [("S0", s0(), cap0, "IT1", "IT2"), ("S1", s1(), cap1, "IT3", "IT4")]
        {
            let r = solve_static_report(&model, &prescribed, &forces, &structural(settings));
            match &r {
                Ok((res, rep)) => {
                    let (e2, einf) = relative_errors(res.displacement_values(), ud);
                    println!(
                        "{name} {sid}: {} iterations, {} restarts, residual {:.3e}, solver {}",
                        rep.iterations, rep.restarts, rep.relative_residual, rep.solver
                    );
                    c.holds(
                        &format!("{it_err} {name} {sid}: solver name"),
                        rep.solver == STRUCTURAL_AMG_NAME,
                        &rep.solver,
                    );
                    c.at_most(&format!("{it_err} {name} {sid}: |u - u_d|_2 / |u_d|_2"), e2, 1e-8);
                    c.at_most(&format!("{it_err} {name} {sid}: |u - u_d|_inf / |u_d|_inf"), einf, 1e-8);
                    c.at_most(
                        &format!("{it_count} {name} {sid}: iterations"),
                        rep.iterations as f64,
                        cap as f64,
                    );
                }
                Err(e) => c.holds(&format!("{it_err} {name} {sid}: solve Ok"), false, &format!("{e:?}")),
            }
        }
        // IT5: the hierarchy of S1.
        let k = stiffness(&model).unwrap();
        let f = nodal_load_vector(&model, &forces).unwrap();
        let fixed = prescribed_equations(&model, &prescribed).unwrap();
        let sys = reduce(&k, &f, &fixed).unwrap();
        let amg =
            smoothed_aggregation_structural(&sys.matrix, sys.free(), &model.dof_map(), model.mesh().nodes(), &s1())
                .unwrap();
        c.at_least(&format!("IT5 {name} S1: levels"), amg.n_levels() as f64, 2.0);
        c.at_most(
            &format!("IT5 {name} S1: operator complexity"),
            amg.operator_complexity(),
            1.5,
        );
    }
    c.finish();
}

// ----------------------------------------------------------------------------------------------------------------- NC1

#[test]
fn nc1_rotation_components_change_the_count() {
    let mut c = Checks::new();
    for case in [beam(), strip("C4 Shell4 strip", false)] {
        let m = &case.model;
        let k = stiffness(m).unwrap();
        let f = nodal_load_vector(m, &case.forces).unwrap();
        let fixed = prescribed_equations(m, &case.prescribed).unwrap();
        let sys = reduce(&k, &f, &fixed).unwrap();
        let dofs = m.dof_map();
        let (points, components) = rows(m, sys.free());
        let bp = node_blocks(sys.free(), &dofs);
        let six = structural_rigid_body_modes(&points, &components);
        let mut norot = six.clone();
        zero_rotation_rows(&mut norot, &components);
        let control = SolveControl {
            tol_abs: TAU * norm2(&sys.rhs),
            tol_rel: 0.0,
            max_iter: MAX_IT,
        };
        let mut its = [0usize; 2];
        for (i, modes) in [six, norot].iter().enumerate() {
            let amg = ElasticAmg::new(&sys.matrix, &bp, modes, &s1()).unwrap();
            let mut x = vec![0.0; sys.rhs.len()];
            its[i] = cg(&sys.matrix, &sys.rhs, &mut x, &amg, &control).iterations;
        }
        c.holds(
            &format!("NC1 {}: it_six < it_norot", case.name),
            its[0] < its[1],
            &format!("{} < {}", its[0], its[1]),
        );
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ B2a, B2b

/// The Hex8 cantilever of ADR 0552 D3 (the `si_amg.rs` pattern): `x = 0` clamped, a traction on `x = 4`.
fn solid_cantilever() -> SolidModel {
    let mut m = SolidModel::new(hex_box([0.0; 3], [4.0, 1.0, 1.0], [16, 4, 4]).unwrap());
    m.add_material("all", Material::isotropic_elastic(E, 0.3, 0.0, 0.0).unwrap())
        .unwrap();
    for comp in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", comp, 0.0)).unwrap();
    }
    m.add_load(Load::traction("xmax", [0.0, 0.0, -1.0e6])).unwrap();
    m
}

#[test]
fn b2_solid_only_bits_unchanged() {
    let mut c = Checks::new();
    let m = solid_cantilever();
    let k = stiffness(&m).unwrap();
    let f = nodal_load_vector(&m, &[]).unwrap();
    let fixed = prescribed_equations(&m, &[]).unwrap();
    let sys = reduce(&k, &f, &fixed).unwrap();
    let (points, components) = rows(&m, sys.free());
    let new = structural_rigid_body_modes(&points, &components);
    let old = rigid_body_modes(&points, &components);
    let same = new.len() == old.len()
        && new
            .iter()
            .zip(&old)
            .all(|(a, b)| a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits()));
    c.holds(
        "B2a structural_rigid_body_modes == rigid_body_modes bit for bit (solid rows)",
        same,
        &format!("{} rows", points.len()),
    );
    for (sid, settings) in [("S0", s0()), ("quadratic", ElasticAmgSettings::quadratic())] {
        let opts = |p: CgPreconditioner| {
            StaticOptions::with_solver(StaticSolver::PreconditionedCg {
                preconditioner: p,
                tolerance: 1e-10,
                max_iterations: 1000,
            })
        };
        let (ra, pa) =
            solve_static_report(&m, &[], &[], &opts(CgPreconditioner::SmoothedAggregation(settings))).unwrap();
        let (rb, pb) = solve_static_report(
            &m,
            &[],
            &[],
            &opts(CgPreconditioner::SmoothedAggregationStructural(settings)),
        )
        .unwrap();
        let bits = ra
            .displacement_values()
            .iter()
            .zip(rb.displacement_values())
            .all(|(x, y)| x.to_bits() == y.to_bits());
        c.holds(
            &format!("B2b {sid}: displacement bits, iterations and restarts equal"),
            bits && pa.iterations == pb.iterations && pa.restarts == pb.restarts,
            &format!(
                "{} / {} iterations, {} / {} restarts",
                pa.iterations, pb.iterations, pa.restarts, pb.restarts
            ),
        );
    }
    c.finish();
}

// ----------------------------------------------------------------------------------------------------------------- R1

#[test]
fn r1_transformation_path_refuses_the_preconditioned_cg() {
    let mut c = Checks::new();
    let Case {
        model: mut m,
        prescribed,
        forces,
        ..
    } = strip("C4 Shell4 strip", false);
    let mut mesh = m.mesh().clone();
    mesh.add_node_set("tip", vec![15, 31]).unwrap();
    let mut with_spring = SolidModel::new(mesh);
    with_spring
        .add_material("all", Material::isotropic_elastic(E, 0.0, RHO, 0.0).unwrap())
        .unwrap();
    with_spring
        .add_support(ElasticSupport::nodal_spring("tip", 2, 1e6))
        .unwrap();
    m = with_spring;
    let r = solve_static_report(&m, &prescribed, &forces, &structural(s0()));
    c.holds(
        "R1 transformation path: InvalidValue { key: \"solver\" }",
        matches!(&r, Err(FemError::InvalidValue { key: "solver", .. })),
        &format!("{:?}", r.as_ref().err()),
    );
    c.finish();
}
