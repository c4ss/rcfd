//! F-386 (ADR 0438, the registration of ADR 0287): a solid case file of its own and the library entry that runs it. Checks
//! A1 to A14, K1 to K3 and the negative controls N1 to N4 of the registration. The case files are in `tests/solid_cases/`;
//! outputs go to `CARGO_TARGET_TMPDIR`.

use rcfd_api::case::{Case, Diagnostic};
use rcfd_api::solid_case::{
    modal_vtu_string, run, run_solid_case_file, SolidAnalysis, SolidCase, SolidOutcome, SolidRun, REPORT_FORMAT,
};
use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::{hex_box, tet_box, upgrade_to_tet10};
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::mass::consistent;
use rcfd_fem::modal::{modal_analysis, ModalOptions, ModalResult};
use rcfd_fem::recover::{von_mises, IntegrationPointField};
use rcfd_fem::solve::{solve_static_report, CgPreconditioner, IcOrdering, StaticOptions, StaticSolver};
use rcfd_fem::{Constraint, Load, Material, SolidMesh, SolidModel, SolidResult};
use rcfd_linalg::amg_elastic::ElasticAmgSettings;
use rcfd_linalg::precond::IcOptions;
use rcfd_linalg::CsrMatrix;
use rcfd_post::solid_output::{solid_fields, solid_vtu_string};
use serde_json::Value;
use std::f64::consts::PI;
use std::path::{Path, PathBuf};

const E_MOD: f64 = 2.0e11;
const NU: f64 = 0.3;
const RHO: f64 = 7850.0;
// Cantilever (ADR 0433 D1 to D5).
const C_L: f64 = 2.0;
const C_H: f64 = 0.1;
const C_P: f64 = 1000.0;
// Bar (ADR 0284 D5).
const BAR_L: f64 = 1.0;
// Uniaxial (ADR 0438 arithmetic 1).
const SIGMA: f64 = 1.0e6;

/// Measured values against their limits; every value is printed, the test fails at the end if any missed.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{name}: {value:.4e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    fn at_least(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value >= limit;
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

fn cases() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("solid_cases")
}

fn out_dir(tag: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join("sb_solid_case").join(tag);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn run_file(name: &str) -> SolidRun {
    let path = cases().join(name);
    run_solid_case_file(&path, Some(&out_dir(name))).unwrap_or_else(|e| panic!("{name}: {e:#?}"))
}

fn static_of(r: &SolidRun) -> &SolidResult {
    match &r.outcome {
        SolidOutcome::Static { result, .. } => result,
        SolidOutcome::Modal(_) => panic!("static expected"),
    }
}

fn modal_of(r: &SolidRun) -> &ModalResult {
    match &r.outcome {
        SolidOutcome::Modal(m) => m,
        SolidOutcome::Static { .. } => panic!("modal expected"),
    }
}

/// Largest `|a - b| / max |b|` and whether every value has the same bits.
fn compare(a: &[f64], b: &[f64]) -> (f64, bool) {
    assert_eq!(a.len(), b.len());
    let scale = b.iter().fold(0.0f64, |m, x| m.max(x.abs()));
    let d = a.iter().zip(b).fold(0.0f64, |m, (x, y)| m.max((x - y).abs()));
    let bits = a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits());
    (if scale > 0.0 { d / scale } else { d }, bits)
}

fn a7(ch: &mut Checks, name: &str, a: &[f64], b: &[f64]) {
    let (d, bits) = compare(a, b);
    ch.at_most(&format!("A7 {name} file run against library calls"), d, 1e-12);
    println!("A7 {name}: bit-identical {bits} (expected true)");
}

// ---------------------------------------------------------------------------------------------------------------------
// The same models by library calls.

/// The single node within 1e-12 of `p`.
fn node_at(mesh: &SolidMesh, p: Vec3) -> usize {
    let found: Vec<usize> = (0..mesh.nodes().len())
        .filter(|&n| (0..3).all(|c| (mesh.nodes()[n][c] - p[c]).abs() <= 1e-12))
        .collect();
    assert_eq!(found.len(), 1, "node at {p:?}");
    found[0]
}

fn cantilever_lib(tet10: bool, nu: f64) -> SolidModel {
    let dims = [C_L, C_H, C_H];
    let mut mesh = if tet10 {
        upgrade_to_tet10(&tet_box([0.0; 3], dims, [40, 2, 2]).unwrap())
            .unwrap()
            .0
    } else {
        hex_box([0.0; 3], dims, [80, 4, 4]).unwrap()
    };
    let n0 = node_at(&mesh, [0.0, 0.0, 0.5 * C_H]);
    let n1 = node_at(&mesh, [0.0, C_H, 0.5 * C_H]);
    mesh.add_node_set("root_n0", vec![n0]).unwrap();
    mesh.add_node_set("root_n1", vec![n1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(E_MOD, nu, RHO, 0.0).unwrap())
        .unwrap();
    m.add_constraint(Constraint::displacement("root_n0", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("root_n0", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("root_n1", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m.add_load(Load::traction("xmax", [0.0, 0.0, -C_P / (C_H * C_H)]))
        .unwrap();
    m
}

fn bar_lib() -> SolidModel {
    let mesh = hex_box([0.0; 3], [BAR_L, 0.1, 0.1], [32, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(E_MOD, 0.0, RHO, 0.0).unwrap())
        .unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
    m
}

fn gmsh_lib(file: &str) -> SolidModel {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../rcfd-io/tests/data")
        .join(file);
    let fv = rcfd_io::gmsh::read_file(path).unwrap();
    let mut mesh = rcfd_fem::convert::from_mesh(&fv).unwrap();
    let a = node_at(&mesh, [0.0, 0.0, 0.0]);
    let b = node_at(&mesh, [0.0, 1.0, 0.0]);
    mesh.add_node_set("corner_a", vec![a]).unwrap();
    mesh.add_node_set("corner_b", vec![b]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(E_MOD, NU, RHO, 0.0).unwrap())
        .unwrap();
    m.add_constraint(Constraint::displacement("corner_a", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("corner_a", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("corner_b", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("inlet", 0, 0.0)).unwrap();
    m.add_load(Load::traction("outlet", [SIGMA, 0.0, 0.0])).unwrap();
    m
}

fn solve_lib(m: &SolidModel, solver: StaticSolver) -> SolidResult {
    solve_static_report(m, &[], &[], &StaticOptions::with_solver(solver))
        .unwrap()
        .0
}

fn delta_t() -> f64 {
    let i = C_H.powi(4) / 12.0;
    let g = E_MOD / (2.0 * (1.0 + NU));
    let kappa = 5.0 / 6.0;
    C_P * C_L.powi(3) / (3.0 * E_MOD * i) * (1.0 + 3.0 * E_MOD * i / (kappa * g * C_H * C_H * C_L * C_L))
}

/// `delta / delta_T` with `delta = sum_i f_i u_i / P` (ADR 0433 D3).
fn ratio(model: &SolidModel, res: &SolidResult) -> f64 {
    let f = nodal_load_vector(model, &[]).unwrap();
    let delta: f64 = f.iter().zip(res.displacement_values()).map(|(f, u)| f * u).sum::<f64>() / C_P;
    delta / delta_t()
}

fn set_nodes(model: &SolidModel, name: &str) -> Vec<Vec3> {
    let m = model.mesh();
    m.node_set(name).unwrap().iter().map(|&n| m.nodes()[n]).collect()
}

// ---------------------------------------------------------------------------------------------------------------------

#[test]
fn a1_a2_a7_a8_k1_n1_cantilever_tet10() {
    let mut ch = Checks::new();
    let r = run_file("cantilever_tet10.toml");
    let model = &r.setup.model;
    let res = static_of(&r);

    // A1: the .vtu and the report.
    let vtu = std::fs::read_to_string(&r.written[0]).unwrap();
    ch.holds(
        "A1 .vtu equals solid_vtu_string of the run",
        vtu == solid_vtu_string(model, res).unwrap(),
        &format!("{} bytes at {}", vtu.len(), r.written[0].display()),
    );
    let rep: Value = serde_json::from_str(&std::fs::read_to_string(&r.written[1]).unwrap()).unwrap();
    let fields = [
        "format",
        "title",
        "analysis",
        "mesh",
        "unknowns",
        "displacement",
        "stress",
        "reactions",
        "solver",
        "outputs",
    ];
    let missing: Vec<&str> = fields.iter().copied().filter(|k| rep.get(k).is_none()).collect();
    ch.holds(
        "A1 report has the fields of D6",
        missing.is_empty() && rep["format"] == REPORT_FORMAT,
        &format!("missing {missing:?}"),
    );
    let n = model.mesh().nodes().len();
    let mag = (0..n)
        .map(|i| res.displacement(i).iter().map(|x| x * x).sum::<f64>().sqrt())
        .fold(f64::NEG_INFINITY, f64::max);
    let f = solid_fields(model, res).unwrap();
    let vm = f.von_mises.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let stats = res.stats().unwrap();
    let bits_ok = rep["displacement"]["max_magnitude"].as_f64().unwrap().to_bits() == mag.to_bits()
        && rep["stress"]["von_mises_max"].as_f64().unwrap().to_bits() == vm.to_bits()
        && rep["solver"]["name"] == stats.solver()
        && rep["solver"]["iterations"].as_u64().unwrap() as usize == stats.iterations()
        && rep["unknowns"].as_u64().unwrap() as usize == stats.unknowns();
    ch.holds(
        "A1 report values equal the result's (bits)",
        bits_ok,
        &format!(
            "max |u| {mag:e}, max von Mises {vm:e}, solver {}, {} unknowns",
            stats.solver(),
            stats.unknowns()
        ),
    );

    // A2: reactions in the report.
    let rz = rep["reactions"]["root_n0"][2].as_f64().unwrap() + rep["reactions"]["root_n1"][2].as_f64().unwrap();
    let tot: Vec<f64> = (0..3).map(|c| rep["reactions"]["total"][c].as_f64().unwrap()).collect();
    println!("A2 reactions: {}", rep["reactions"]);
    ch.at_most("A2 |(R_z(n0) + R_z(n1)) / P - 1|", (rz / C_P - 1.0).abs(), 1e-8);
    ch.at_most("A2 |R_z total / P - 1|", (tot[2] / C_P - 1.0).abs(), 1e-8);
    ch.at_most("A2 |R_x total| / P", tot[0].abs() / C_P, 1e-8);
    ch.at_most("A2 |R_y total| / P", tot[1].abs() / C_P, 1e-8);

    // K1: the box node sets.
    for (name, p) in [("root_n0", [0.0, 0.0, 0.5 * C_H]), ("root_n1", [0.0, C_H, 0.5 * C_H])] {
        let s = set_nodes(model, name);
        let ok = s.len() == 1 && (0..3).all(|c| (s[0][c] - p[c]).abs() <= 1e-15);
        ch.holds(&format!("K1 {name} is the node {p:?}"), ok, &format!("{s:?}"));
    }

    // A7: against the library model.
    let lib = cantilever_lib(true, NU);
    let lr = solve_lib(&lib, StaticSolver::Direct);
    a7(
        &mut ch,
        "cantilever Tet10 displacement",
        res.displacement_values(),
        lr.displacement_values(),
    );
    a7(
        &mut ch,
        "cantilever Tet10 reactions",
        res.reactions().unwrap(),
        lr.reactions().unwrap(),
    );

    // A8: ADR 0433 C1.
    let q = ratio(model, res);
    println!("A8 ratio {q:.6} (ADR 0433: 1.000529)");
    ch.at_most("A8 Tet10 |delta / delta_T - 1|", (q - 1.0).abs(), 0.01);

    // N1: a changed Poisson ratio is seen by the comparison.
    let text = std::fs::read_to_string(cases().join("cantilever_tet10.toml")).unwrap();
    let t2 = text.replace("poisson_ratio = 0.3\n", "poisson_ratio = 0.3000000001\n");
    assert_ne!(text, t2);
    let rn = run(SolidCase::parse_toml_in(&t2, &cases()).unwrap().build().unwrap()).unwrap();
    let (d, _) = compare(static_of(&rn).displacement_values(), lr.displacement_values());
    ch.holds(
        "N1 the comparison sees nu + 1e-10",
        d > 0.0 && d < 1e-6,
        &format!("{d:.3e}"),
    );
    ch.finish();
}

#[test]
fn a9_cantilever_hex8() {
    let mut ch = Checks::new();
    let r = run_file("cantilever_hex8.toml");
    let q = ratio(&r.setup.model, static_of(&r));
    println!("A9 ratio {q:.6} (ADR 0433: 0.967873)");
    ch.at_most("A9 Hex8 |delta / delta_T - 1|", (q - 1.0).abs(), 0.05);
    ch.finish();
}

fn mv(m: &CsrMatrix, x: &[f64]) -> Vec<f64> {
    let mut y = vec![0.0; m.n];
    m.matvec(x, &mut y);
    y
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn bar_exact(n: usize) -> f64 {
    (2 * n - 1) as f64 * (E_MOD / RHO).sqrt() / (4.0 * BAR_L)
}

fn bar_discrete(n: usize, elements: usize) -> f64 {
    let h = BAR_L / elements as f64;
    let k = (2 * n - 1) as f64 * PI / (2.0 * BAR_L);
    let lam = 6.0 * E_MOD / RHO / (h * h) * (1.0 - (k * h).cos()) / (2.0 + (k * h).cos());
    lam.sqrt() / (2.0 * PI)
}

#[test]
fn a10_a7_k3_bar_hex8() {
    let mut ch = Checks::new();
    let r = run_file("bar_hex8.toml");
    let model = &r.setup.model;
    let m = modal_of(&r);
    println!("bar frequencies {:?}, iterations {}", m.frequencies, m.iterations);
    let mass = consistent(model).unwrap();
    let dofs = model.dof_map();
    let ax: Vec<f64> = m
        .modes
        .iter()
        .map(|phi| {
            let mut px = phi.clone();
            for (e, v) in px.iter_mut().enumerate() {
                if dofs.node_component(e).unwrap().1 != 0 {
                    *v = 0.0;
                }
            }
            dot(&px, &mv(&mass, &px))
        })
        .collect();
    let axial: Vec<usize> = (0..ax.len()).filter(|&i| ax[i] >= 0.99).collect();
    ch.holds(
        "A10 the first three modes are axial",
        axial.len() >= 3 && axial[..3] == [0, 1, 2],
        &format!("fractions {ax:?}"),
    );
    for n in 1..=3 {
        let f = m.frequencies[n - 1];
        ch.at_most(
            &format!("A10 B1 |f_{n} / f_exact - 1|"),
            (f / bar_exact(n) - 1.0).abs(),
            5e-3,
        );
        ch.at_most(
            &format!("A10 B3 |f_{n} / f_discrete - 1|"),
            (f / bar_discrete(n, 32) - 1.0).abs(),
            1e-9,
        );
    }
    let mphi: Vec<Vec<f64>> = m.modes.iter().map(|p| mv(&mass, p)).collect();
    let mut orth = 0.0f64;
    for (i, p) in m.modes.iter().enumerate() {
        for (j, mp) in mphi.iter().enumerate() {
            orth = orth.max((dot(p, mp) - if i == j { 1.0 } else { 0.0 }).abs());
        }
    }
    ch.at_most("A10 M-orthonormality", orth, 1e-9);

    // A7.
    let lib = modal_analysis(&bar_lib(), &ModalOptions::new(4)).unwrap();
    a7(&mut ch, "bar eigenvalues", &m.eigenvalues, &lib.eigenvalues);
    let flat = |r: &ModalResult| -> Vec<f64> { r.modes.iter().flatten().copied().collect() };
    a7(&mut ch, "bar modes", &flat(m), &flat(&lib));

    // K3: the modal .vtu and the report's frequencies.
    let vtu = std::fs::read_to_string(&r.written[0]).unwrap();
    let arrays = (1..=4).all(|k| vtu.contains(&format!("Name=\"mode_{k}\""))) && !vtu.contains("Name=\"mode_5\"");
    ch.holds(
        "K3 modal .vtu equals modal_vtu_string, arrays mode_1..mode_4",
        vtu == modal_vtu_string(model, m).unwrap() && arrays,
        &format!("{} bytes", vtu.len()),
    );
    let rep: Value = serde_json::from_str(&std::fs::read_to_string(&r.written[1]).unwrap()).unwrap();
    let fr: Vec<f64> = rep["frequencies"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect();
    let same =
        fr.len() == m.frequencies.len() && fr.iter().zip(&m.frequencies).all(|(a, b)| a.to_bits() == b.to_bits());
    ch.holds(
        "K3 report frequencies equal the result's (bits)",
        same,
        &format!("{fr:?}"),
    );
    ch.finish();
}

fn uniaxial(ch: &mut Checks, file: &str, msh: &str) {
    let r = run_file(file);
    let model = &r.setup.model;
    let res = static_of(&r);
    let mesh = model.mesh();
    // K2.
    let area = |s: &str| {
        mesh.surface_set(s)
            .unwrap()
            .iter()
            .map(|&f| mesh.face_area(f))
            .sum::<f64>()
    };
    let ok = mesh.nodes().len() == 351
        && mesh.n_elements() == 1139
        && (area("inlet") - 1.0).abs() <= 1e-12
        && (area("outlet") - 1.0).abs() <= 1e-12
        && (area("walls") - 8.0).abs() <= 1e-12;
    ch.holds(
        &format!("K2 {msh}: 351 nodes, 1139 Tet4, areas 1, 1, 8"),
        ok,
        &format!(
            "{} nodes, {} elements, areas {:e} {:e} {:e}",
            mesh.nodes().len(),
            mesh.n_elements(),
            area("inlet"),
            area("outlet"),
            area("walls")
        ),
    );
    // K1: the corner sets.
    for (name, p) in [("corner_a", [0.0, 0.0, 0.0]), ("corner_b", [0.0, 1.0, 0.0])] {
        let s = set_nodes(model, name);
        let ok = s.len() == 1 && (0..3).all(|c| (s[0][c] - p[c]).abs() <= 1e-12);
        ch.holds(&format!("K1 {msh} {name} is the node {p:?}"), ok, &format!("{s:?}"));
    }
    // A11 / A12.
    let mut err = 0.0f64;
    let scale = SIGMA * 2.0 / E_MOD;
    for (n, p) in mesh.nodes().iter().enumerate() {
        let u = res.displacement(n);
        let ex = [
            SIGMA * p[0] / E_MOD,
            -NU * SIGMA * p[1] / E_MOD,
            -NU * SIGMA * p[2] / E_MOD,
        ];
        for c in 0..3 {
            err = err.max((u[c] - ex[c]).abs());
        }
    }
    let field = IntegrationPointField::from_result(model, res).unwrap();
    let means = field.element_means(field.stress()).unwrap();
    let mut serr = 0.0f64;
    for s in &means {
        for (c, v) in s.iter().enumerate() {
            let ex = if c == 0 { SIGMA } else { 0.0 };
            serr = serr.max((v - ex).abs() / SIGMA);
        }
    }
    let id = if msh.contains("2.2") { "A12" } else { "A11" };
    ch.at_most(&format!("{id} {msh} max |u_h - u| / max |u|"), err / scale, 1e-10);
    ch.at_most(&format!("{id} {msh} max |sigma_e - s e_x e_x| / s"), serr, 1e-10);
    println!(
        "{id} {msh}: solver {:?}, max von Mises {:e}",
        res.stats().map(|s| s.solver().to_string()),
        means.iter().map(von_mises).fold(0.0, f64::max)
    );
    // A7.
    let lr = solve_lib(&gmsh_lib(msh), StaticSolver::Direct);
    a7(
        ch,
        &format!("Gmsh {msh} displacement"),
        res.displacement_values(),
        lr.displacement_values(),
    );
}

#[test]
fn a11_a12_a7_k1_k2_gmsh_uniaxial() {
    let mut ch = Checks::new();
    uniaxial(&mut ch, "gmsh_uniaxial.toml", "tet_v4.1_ascii.msh");
    uniaxial(&mut ch, "gmsh_uniaxial_v22.toml", "tet_v2.2_bin.msh");
    ch.finish();
}

// ---------------------------------------------------------------------------------------------------------------------
// Solver keys.

const SMALL: &str = r#"
[mesh]
element = "hex8"
lengths = [1.0, 0.2, 0.2]
divisions = [4, 1, 1]

[materials.steel]
youngs_modulus = 2.0e11
poisson_ratio = 0.3

[constraints.root]
set = "xmin"
components = ["x", "y", "z"]
"#;

fn parse(text: &str) -> Result<SolidCase, Vec<Diagnostic>> {
    SolidCase::parse_toml_in(text, &cases())
}

fn pcg(p: CgPreconditioner) -> SolidAnalysis {
    SolidAnalysis::Static(StaticOptions::with_solver(StaticSolver::PreconditionedCg {
        preconditioner: p,
        tolerance: 1e-10,
        max_iterations: 20000,
    }))
}

fn fill(level: usize, ordering: IcOrdering) -> CgPreconditioner {
    CgPreconditioner::IncompleteCholeskyFill {
        options: IcOptions::default(),
        level,
        ordering,
    }
}

#[test]
fn a13_solver_keys() {
    let mut ch = Checks::new();
    let rows: Vec<(&str, SolidAnalysis)> = vec![
        (
            "",
            SolidAnalysis::Static(StaticOptions::with_solver(StaticSolver::Direct)),
        ),
        (
            "[solver]\nmethod = \"direct\"\n",
            SolidAnalysis::Static(StaticOptions::with_solver(StaticSolver::Direct)),
        ),
        (
            "[solver]\nmethod = \"cg\"\npreconditioner = \"block_jacobi\"\n",
            pcg(CgPreconditioner::BlockJacobi),
        ),
        (
            "[solver]\nmethod = \"cg\"\npreconditioner = \"ic0\"\n",
            pcg(CgPreconditioner::IncompleteCholesky(IcOptions::default())),
        ),
        ("[solver]\nmethod = \"cg\"\n", pcg(fill(1, IcOrdering::Natural))),
        (
            "[solver]\nmethod = \"cg\"\npreconditioner = \"ic_fill\"\n",
            pcg(fill(1, IcOrdering::Natural)),
        ),
        (
            "[solver]\nmethod = \"cg\"\npreconditioner = \"ic_fill\"\nfill_level = 2\nordering = \"node_rcm\"\n",
            pcg(fill(2, IcOrdering::NodeReverseCuthillMcKee)),
        ),
        (
            "[solver]\nmethod = \"cg\"\npreconditioner = \"amg\"\n",
            pcg(CgPreconditioner::SmoothedAggregation(ElasticAmgSettings::default())),
        ),
        (
            "[solver]\nmethod = \"cg\"\npreconditioner = \"amg_quadratic\"\n",
            pcg(CgPreconditioner::SmoothedAggregation(ElasticAmgSettings::quadratic())),
        ),
        (
            "[solver]\nmethod = \"cg\"\npreconditioner = \"amg_quadratic_smoothed\"\n",
            pcg(CgPreconditioner::SmoothedAggregation(
                ElasticAmgSettings::quadratic_smoothed(),
            )),
        ),
        (
            "[solver]\nmethod = \"cg\"\npreconditioner = \"block_jacobi\"\ntolerance = 1e-8\nmax_iterations = 77\n",
            SolidAnalysis::Static(StaticOptions::with_solver(StaticSolver::PreconditionedCg {
                preconditioner: CgPreconditioner::BlockJacobi,
                tolerance: 1e-8,
                max_iterations: 77,
            })),
        ),
    ];
    for (extra, want) in rows {
        let text = format!("{SMALL}\n{extra}");
        let got = parse(&text).map(|c| c.analysis());
        ch.holds(
            &format!("A13 [solver] {:?}", extra.replace('\n', "; ")),
            got.as_ref().is_ok_and(|g| *g == want),
            &format!("{got:?}"),
        );
    }
    // Refusals naming the key.
    let bad = [
        (
            "[solver]\nmethod = \"cg\"\npreconditioner = \"amg\"\nfill_level = 2\n",
            "solver.fill_level",
            "fill_level = 2",
        ),
        (
            "[analysis]\ntype = \"modal\"\n[solver]\nmethod = \"cg\"\n",
            "solver.method",
            "method = \"cg\"",
        ),
    ];
    for (extra, path, marker) in bad {
        let text = format!("{SMALL}\n{extra}");
        expect_diag(&mut ch, "A13", &text, path, marker, "");
    }
    ch.finish();
}

// ---------------------------------------------------------------------------------------------------------------------
// Errors with the key and the line.

fn line_of(text: &str, marker: &str) -> usize {
    let hits: Vec<usize> = text
        .lines()
        .enumerate()
        .filter(|(_, l)| l.contains(marker))
        .map(|(i, _)| i + 1)
        .collect();
    assert_eq!(hits.len(), 1, "marker {marker:?} once in the text");
    hits[0]
}

/// Parses and builds `text`; the diagnostics must hold one with `path`, the line of `marker` and `needle` in its message.
fn expect_diag(ch: &mut Checks, id: &str, text: &str, path: &str, marker: &str, needle: &str) {
    let line = line_of(text, marker);
    let errs = match parse(text) {
        Err(e) => e,
        Ok(c) => match c.build() {
            Err(e) => e,
            Ok(_) => Vec::new(),
        },
    };
    let ok = errs
        .iter()
        .any(|d| d.path == path && d.line == Some(line) && d.message.contains(needle));
    let shown: Vec<String> = errs.iter().map(ToString::to_string).collect();
    ch.holds(&format!("{id} {path} at line {line}"), ok, &format!("{shown:?}"));
}

fn tet10_text() -> String {
    std::fs::read_to_string(cases().join("cantilever_tet10.toml")).unwrap()
}

fn swap(text: &str, from: &str, to: &str) -> String {
    assert_eq!(text.matches(from).count(), 1, "{from:?} once");
    text.replace(from, to)
}

#[test]
fn a3_to_a6_errors_and_n2_to_n4() {
    let mut ch = Checks::new();
    let base = tet10_text();
    // A3: unknown keys.
    let t = swap(&base, "[materials.steel]\n", "[materials.steel]\nyoungs = 1.0\n");
    expect_diag(
        &mut ch,
        "A3",
        &t,
        "materials.steel.youngs",
        "youngs = 1.0",
        "unknown setting",
    );
    let t = format!("{base}\n[flow]\nx = 1\n");
    expect_diag(&mut ch, "A3", &t, "flow", "[flow]", "unknown setting");
    // A4: missing sets (one text, three diagnostics).
    let t = swap(&base, "set = \"root_n1\"", "set = \"nowhere\" # A4c");
    let t = swap(&t, "set = \"xmax\"", "set = \"nowhere\" # A4l");
    let t = swap(&t, "element_set = \"all\"", "element_set = \"nowhere\" # A4m");
    expect_diag(&mut ch, "A4", &t, "constraints.root_n1.set", "# A4c", "nowhere");
    expect_diag(&mut ch, "A4", &t, "loads.tip.set", "# A4l", "nowhere");
    expect_diag(&mut ch, "A4", &t, "materials.steel.element_set", "# A4m", "nowhere");
    // A5: wrong units.
    let t = swap(&base, "youngs_modulus = 2.0e11", "youngs_modulus = \"200 m\"");
    expect_diag(
        &mut ch,
        "A5",
        &t,
        "materials.steel.youngs_modulus",
        "youngs_modulus",
        "",
    );
    let t = swap(&base, "traction = [0.0, 0.0, -1.0e5]", "traction = [0.0, 0.0, \"5 N\"]");
    expect_diag(&mut ch, "A5", &t, "loads.tip.traction.2", "traction = [", "");
    // A6: wrong types.
    let t = swap(&base, "divisions = [40, 2, 2]", "divisions = [40, 2.5, 2]");
    expect_diag(&mut ch, "A6", &t, "mesh.divisions.1", "divisions", "integer");
    let t = swap(&base, "poisson_ratio = 0.3", "poisson_ratio = \"abc\"");
    expect_diag(
        &mut ch,
        "A6",
        &t,
        "materials.steel.poisson_ratio",
        "poisson_ratio",
        "number",
    );
    let t = swap(&base, "components = [\"z\"]", "components = \"x\"");
    expect_diag(
        &mut ch,
        "A6",
        &t,
        "constraints.root_n1.components",
        "components = \"x\"",
        "list",
    );

    // N2: the schemas are separate.
    let flow = Case::parse_toml(&base);
    ch.holds(
        "N2 the solid file through the flow reader",
        flow.as_ref()
            .is_err_and(|e| e.iter().any(|d| d.message.contains("unknown setting"))),
        &format!("{:?}", flow.as_ref().err().map(|e| e.len())),
    );
    let cavity = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../cases/cavity/adapt_re100.toml");
    let s = SolidCase::load(&cavity);
    ch.holds(
        "N2 a flow case through the solid reader",
        s.as_ref()
            .is_err_and(|e| e.iter().any(|d| d.message.contains("unknown setting"))),
        &format!(
            "{:?}",
            s.as_ref()
                .err()
                .map(|e| e.iter().map(ToString::to_string).collect::<Vec<_>>())
        ),
    );
    // N3: a syntax error.
    let t = "title = \"x\"\n[mesh]\nfile = \"unclosed\n";
    let e = parse(t);
    ch.holds(
        "N3 a syntax error at line 3",
        e.as_ref().is_err_and(|e| e.len() == 1 && e[0].line == Some(3)),
        &format!("{:?}", e.err()),
    );
    // N4: an empty selection.
    let t = swap(
        &base,
        "box_min = [-1e-9, -1e-9, 0.049999999]",
        "box_min = [-1e-9, -1e-9, 0.0499]  # n4",
    );
    let t = swap(
        &t,
        "box_max = [1e-9, 1e-9, 0.050000001]",
        "box_max = [1e-9, 1e-9, 0.0499]",
    );
    expect_diag(
        &mut ch,
        "N4",
        &t,
        "node_sets.root_n0.box_min",
        "# n4",
        "selects no node",
    );
    ch.finish();
}

// ---------------------------------------------------------------------------------------------------------------------
// A14 and the A7 iterative run.

#[test]
fn a14_a7_cg_preconditioners_from_files() {
    let mut ch = Checks::new();
    let direct = solve_lib(&cantilever_lib(true, NU), StaticSolver::Direct);
    let base = tet10_text();
    for p in [
        "block_jacobi",
        "ic0",
        "ic_fill",
        "amg",
        "amg_quadratic",
        "amg_quadratic_smoothed",
    ] {
        let t = swap(
            &base,
            "method = \"direct\"",
            &format!("method = \"cg\"\npreconditioner = \"{p}\"\ntolerance = 1e-10"),
        );
        let t0 = std::time::Instant::now();
        let r = run(parse(&t).unwrap().build().unwrap()).unwrap_or_else(|e| panic!("{p}: {e:#?}"));
        let rr = r.report["solver"]["relative_residual"].as_f64().unwrap();
        println!(
            "A14 {p}: solver {}, iterations {}, restarts {}, shift {}, {:.2} s",
            r.report["solver"]["name"],
            r.report["solver"]["iterations"],
            r.report["solver"]["restarts"],
            r.report["solver"]["shift"],
            t0.elapsed().as_secs_f64()
        );
        ch.at_most(&format!("A14 {p} relative residual"), rr, 1e-10);
        let (d, _) = compare(static_of(&r).displacement_values(), direct.displacement_values());
        ch.at_most(&format!("A14 {p} against the direct solve"), d, 1e-7);
    }
    // A7: the iterative file run against the same library calls.
    let r = run_file("cantilever_tet10_amg.toml");
    let lib = solve_lib(
        &cantilever_lib(true, NU),
        StaticSolver::PreconditionedCg {
            preconditioner: CgPreconditioner::SmoothedAggregation(ElasticAmgSettings::quadratic()),
            tolerance: 1e-10,
            max_iterations: 20000,
        },
    );
    a7(
        &mut ch,
        "cantilever Tet10 CG amg_quadratic displacement",
        static_of(&r).displacement_values(),
        lib.displacement_values(),
    );
    ch.at_least("A7 amg run wrote two files", r.written.len() as f64, 2.0);
    ch.finish();
}
