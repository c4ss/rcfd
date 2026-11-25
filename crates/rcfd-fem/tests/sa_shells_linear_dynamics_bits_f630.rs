//! F-630 (ADR 0798): check B1, the bit record of the linear dynamic paths. Models without shells on every path, the shell
//! paths that are taken without an option, and the text of every refusal of shells without the option.
//!
//! This file uses only the API of main 2d125d55. It was run on the unchanged library before any library code of F-630; the
//! lines it printed are `sa_shells_linear_dynamics_bits_f630.tsv` (Windows x86_64) and `.linux.tsv` (Linux x86_64, the image
//! of ADR 0150). `F630_BITS_OUT=<file>` writes the lines. The record of main is asserted on its platform; on another one the
//! refusals only (they do not call the mathematical library).

use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::damped::{damped_modes, DampedModes, DampedOptions};
use rcfd_fem::geometric::Prestress;
use rcfd_fem::harmonic::{
    participation, HarmonicLoad, HarmonicOptions, HarmonicResponse, HarmonicSolver, ModalBasis, ModalDamping,
};
use rcfd_fem::harmonic_direct::{BaseMotion, Dashpot, DirectHarmonic, DirectHarmonicOptions, LossFactor};
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::modal::{modal_analysis, ModalOptions, ModalResult};
use rcfd_fem::transient::{Integrator, LoadHistory, LoadTable, Method, MotionHistory, Rayleigh, TransientOptions};
use rcfd_fem::{BlockProperty, Constraint, ElasticSupport, ElementKind, Load, Material, SolidMesh, SolidModel};
use rcfd_linalg::complex::C64;
use rcfd_linalg::eigen::ResidualTest;

/// FNV-1a 64 over the count and the `to_bits()` of each value, little-endian bytes (the digest of ADR 0291).
fn digest(values: &[f64]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |x: u64| {
        for b in x.to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    eat(values.len() as u64);
    for v in values {
        eat(v.to_bits());
    }
    h
}

struct Lines(Vec<(String, String)>);

impl Lines {
    fn values(&mut self, name: &str, v: &[f64]) {
        assert!(v.iter().all(|x| x.is_finite()), "{name} is not finite");
        self.0
            .push((name.to_string(), format!("{:016x} {}", digest(v), v.len())));
    }
    fn response(&mut self, name: &str, r: &HarmonicResponse) {
        let v: Vec<f64> = r.re.iter().chain(&r.im).copied().collect();
        self.values(name, &v);
    }
    fn modal(&mut self, name: &str, r: &ModalResult) {
        let v: Vec<f64> = r.eigenvalues.iter().chain(r.modes.iter().flatten()).copied().collect();
        self.values(name, &v);
    }
    fn damped(&mut self, name: &str, r: &DampedModes) {
        let v: Vec<f64> = r
            .modes
            .iter()
            .flat_map(|m| [m.eigenvalue.re, m.eigenvalue.im])
            .chain(r.modes.iter().flat_map(|m| m.shape.iter().flat_map(|z| [z.re, z.im])))
            .collect();
        self.values(name, &v);
    }
    fn refusal<T>(&mut self, name: &str, r: Result<T, rcfd_fem::FemError>) {
        let text = match r {
            Ok(_) => "taken".to_string(),
            Err(e) => format!("{e:?}"),
        };
        self.0
            .push((format!("refusal {name}"), text.replace(['\t', '\n'], " ")));
    }
}

fn steel() -> Material {
    Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 0.0).unwrap()
}

fn aluminium() -> Material {
    Material::isotropic_elastic(7.0e10, 0.33, 2700.0, 0.0).unwrap()
}

fn mopts(n: usize) -> ModalOptions {
    let mut o = ModalOptions::new(n)
        .residual_test(ResidualTest::Backward)
        .tolerance(1e-12);
    o.subspace_size = Some(24);
    o.max_iterations = 1000;
    o
}

/// The Hex8 box `1 x 0.2 x 0.1`, 4 x 2 x 1; element sets `one` (x < 1/2) and `two`.
fn box_mesh() -> SolidMesh {
    let mut mesh = hex_box([0.0; 3], [1.0, 0.2, 0.1], [4, 2, 1]).unwrap();
    let left = |e: usize| {
        let x: f64 = mesh.element_nodes(e).iter().map(|&n| mesh.nodes()[n][0]).sum::<f64>() / 8.0;
        x < 0.5
    };
    let one: Vec<usize> = (0..mesh.n_elements()).filter(|&e| left(e)).collect();
    let two: Vec<usize> = (0..mesh.n_elements()).filter(|&e| !left(e)).collect();
    mesh.add_element_set("one", one).unwrap();
    mesh.add_element_set("two", two).unwrap();
    mesh.add_node_set("corner", vec![0]).unwrap();
    mesh
}

fn solid(two_materials: bool) -> SolidModel {
    let mut m = SolidModel::new(box_mesh());
    if two_materials {
        m.add_material("one", steel()).unwrap();
        m.add_material("two", aluminium()).unwrap();
    } else {
        m.add_material("all", steel()).unwrap();
    }
    m.add_load(Load::nodal_force("xmax", [3.0, 5.0, 7.0])).unwrap();
    m
}

fn fix(m: &mut SolidModel, set: &str, components: usize) {
    for c in 0..components {
        m.add_constraint(Constraint::displacement(set, c, 0.0)).unwrap();
    }
}

fn clamped(two_materials: bool) -> SolidModel {
    let mut m = solid(two_materials);
    fix(&mut m, "xmin", 3);
    m
}

/// The box held by a rigid set on `xmin` whose reference is pinned through three clamped corner nodes, with springs on `xmax`.
fn on_transformation() -> SolidModel {
    let mut m = solid(false);
    m.add_constraint(Constraint::rigid_set("xmin", [0.0, 0.1, 0.05]))
        .unwrap();
    m.add_support(ElasticSupport::nodal_spring("xmax", 2, 1.0e7)).unwrap();
    m.add_support(ElasticSupport::nodal_spring("xmin", 0, 1.0e9)).unwrap();
    m.add_support(ElasticSupport::nodal_spring("xmin", 1, 1.0e9)).unwrap();
    m.add_support(ElasticSupport::nodal_spring("xmin", 2, 1.0e9)).unwrap();
    m
}

fn history(m: &SolidModel) -> Vec<LoadHistory> {
    vec![LoadHistory::model_loads(m, LoadTable::constant(1.0, -1.0, 1.0).unwrap()).unwrap()]
}

fn run(mut it: Integrator) -> Vec<f64> {
    let z = vec![0.0; it.n_unknowns()];
    it.start(0.0, &z, &z).unwrap();
    it.run(30).unwrap();
    let mut v = it.displacement().unwrap();
    v.extend(it.velocity().unwrap());
    v.extend(it.acceleration().unwrap());
    v
}

const DT: f64 = 2.0e-5;
const W: f64 = 900.0;

fn solids(l: &mut Lines) {
    let rayleigh = Rayleigh {
        mass: 12.0,
        stiffness: 3.0e-6,
    };
    let (one, two, tr, free) = (clamped(false), clamped(true), on_transformation(), solid(false));
    // transient
    let t = TransientOptions::new(DT);
    let td = TransientOptions::new(DT)
        .damping(rayleigh)
        .method(Method::GeneralisedAlpha { rho_inf: 0.8 });
    for (name, m) in [("clamped", &one), ("two materials", &two), ("transformation", &tr)] {
        l.values(
            &format!("transient {name}"),
            &run(Integrator::new(m, &history(m), &t).unwrap()),
        );
        l.values(
            &format!("transient damped {name}"),
            &run(Integrator::new(m, &history(m), &td).unwrap()),
        );
    }
    let table = LoadTable::new(vec![(-1.0, -1.0), (0.0, 0.0), (1.0, 1.0)]).unwrap();
    let moved: Vec<Prescribed> = free
        .mesh()
        .node_set("xmin")
        .unwrap()
        .iter()
        .flat_map(|&n| {
            [
                Prescribed::new(n, 0, 0.0),
                Prescribed::new(n, 1, 0.0),
                Prescribed::new(n, 2, 0.5),
            ]
        })
        .collect();
    let mut it = Integrator::with_motion(&free, &history(&free), &[MotionHistory::new(moved, table)], &td).unwrap();
    let nq = it.free().len();
    it.start_reduced(0.0, &vec![0.0; nq], &vec![0.0; nq]).unwrap();
    it.run(30).unwrap();
    l.values("transient motion", &it.displacement().unwrap());
    // damped modes
    let tip = one.mesh().node_set("xmax").unwrap()[0];
    let d = DampedOptions::with_modal(mopts(6))
        .rayleigh(12.0, 3.0e-6)
        .dashpot(Dashpot::ground(tip, 2, 50.0));
    l.damped("damped clamped", &damped_modes(&one, &d).unwrap());
    let d = DampedOptions::with_modal(mopts(6))
        .loss_factor("one", LossFactor::Constant(0.02))
        .loss_factor("two", LossFactor::Constant(0.07));
    l.damped("damped two materials", &damped_modes(&two, &d).unwrap());
    let d = DampedOptions::with_modal(mopts(6))
        .residual_vectors(true)
        .dashpot(Dashpot::ground(tip, 1, 80.0));
    l.damped("damped residual", &damped_modes(&one, &d).unwrap());
    // superposition
    let all = ModalBasis::all_modes(&one, Some(2)).unwrap();
    l.values("all modes eigenvalues", all.eigenvalues());
    let flat: Vec<f64> = all.modes().iter().flatten().copied().collect();
    l.values("all modes", &flat);
    let load = HarmonicLoad::from_model(&one).unwrap();
    let h = HarmonicOptions::new(ModalDamping::Ratio(0.01));
    let s = HarmonicSolver::new(&one, all.clone(), &h).unwrap();
    l.response("superposition all", &s.response(&load, W).unwrap());
    let four = ModalBasis::from_result(&modal_analysis(&one, &mopts(4)).unwrap()).unwrap();
    let s = HarmonicSolver::new(&one, four.clone(), &h).unwrap();
    l.response("superposition four", &s.response(&load, W).unwrap());
    let pre = Prestress::uniform(&one, [2.0e7, 0.0, 0.0, 0.0, 0.0, 0.0]).unwrap();
    let s = HarmonicSolver::new(&one, four, &h.clone().prestress(pre.clone())).unwrap();
    l.response("superposition prestress", &s.response(&load, W).unwrap());
    let free_basis = ModalBasis::all_modes(&free, None).unwrap();
    let s = HarmonicSolver::new(&free, free_basis, &h).unwrap();
    l.response("superposition free", &s.response(&load, W).unwrap());
    let r = modal_analysis(&tr, &mopts(4)).unwrap();
    l.modal("modal transformation", &r);
    let s = HarmonicSolver::new(&tr, ModalBasis::from_result(&r).unwrap(), &h).unwrap();
    l.response("superposition transformation", &s.response(&load, W).unwrap());
    // direct
    let o = DirectHarmonicOptions::new().rayleigh(12.0, 3.0e-6);
    for (name, m) in [("clamped", &one), ("two materials", &two), ("transformation", &tr)] {
        let s = DirectHarmonic::new(m, &o).unwrap();
        l.response(&format!("direct {name}"), &s.response(&load, W).unwrap());
    }
    let o = DirectHarmonicOptions::new()
        .loss_factor("one", LossFactor::Constant(0.02))
        .dashpot(Dashpot::ground(tip, 2, 50.0));
    let s = DirectHarmonic::new(&two, &o).unwrap();
    l.response("direct loss", &s.response(&load, W).unwrap());
    let mut base = BaseMotion::new();
    base.add_set(&two, "xmin", 2, C64::new(1.0e-3, 2.0e-4)).unwrap();
    l.response("direct base", &s.response_with_base(&load, &base, W).unwrap());
    let s = DirectHarmonic::new(&one, &DirectHarmonicOptions::new().prestress(pre)).unwrap();
    l.response("direct prestress", &s.response(&load, W).unwrap());
}

/// The shell strip `1 x 0.2`, `t = 0.1`, 6 x 1 cells, Shell4 or cut into Shell3; node sets `root`, `tip`, `corner`.
fn strip(kind: ElementKind) -> SolidModel {
    let mut nodes = Vec::new();
    for j in 0..2 {
        for i in 0..7 {
            nodes.push([i as f64 / 6.0, 0.2 * j as f64, 0.0]);
        }
    }
    let mut conn = Vec::new();
    for i in 0..6 {
        let (a, b, c, d) = (i, i + 1, i + 8, i + 7);
        if kind == ElementKind::Shell3 {
            conn.extend([a, b, c, a, c, d]);
        } else {
            conn.extend([a, b, c, d]);
        }
    }
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(kind, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::shell(0.1).unwrap()).unwrap();
    let ne = mesh.n_elements();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    mesh.add_node_set("root", vec![0, 7]).unwrap();
    mesh.add_node_set("tip", vec![6, 13]).unwrap();
    mesh.add_node_set("corner", vec![0]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    m.add_load(Load::shell_pressure("all", 1000.0)).unwrap();
    m
}

fn shells(l: &mut Lines) {
    for (name, kind) in [("Shell4", ElementKind::Shell4), ("Shell3", ElementKind::Shell3)] {
        let mut m = strip(kind);
        fix(&mut m, "root", 6);
        let mut t = strip(kind);
        t.add_constraint(Constraint::rigid_set("root", [0.0, 0.1, 0.0]))
            .unwrap();
        fix(&mut t, "corner", 6);
        let r = modal_analysis(&m, &mopts(5)).unwrap();
        l.modal(&format!("{name} modal"), &r);
        l.modal(
            &format!("{name} modal transformation"),
            &modal_analysis(&t, &mopts(5)).unwrap(),
        );
        let basis = ModalBasis::from_result(&r).unwrap();
        let p = participation(&m, &basis, [0.0, 0.0, 1.0], Some(2)).unwrap();
        l.values(&format!("{name} participation"), &p.factors);
        l.values(&format!("{name} load"), &nodal_load_vector(&m, &[]).unwrap());
        let load = HarmonicLoad::from_model(&m).unwrap();
        let plain = HarmonicOptions::new(ModalDamping::Ratio(0.01)).static_correction(false);
        let s = HarmonicSolver::new(&m, basis.clone(), &plain).unwrap();
        l.response(
            &format!("{name} superposition without correction"),
            &s.response(&load, W).unwrap(),
        );
        // the refusals without the option
        let o = TransientOptions::new(DT);
        l.refusal(
            &format!("{name} transient plain"),
            Integrator::new(&m, &history(&m), &o),
        );
        l.refusal(
            &format!("{name} transient transformation"),
            Integrator::new(&t, &history(&t), &o),
        );
        let moved = MotionHistory::new(
            vec![Prescribed::new(6, 2, 1.0)],
            LoadTable::new(vec![(-1.0, -1.0), (1.0, 1.0)]).unwrap(),
        );
        l.refusal(
            &format!("{name} transient motion"),
            Integrator::with_motion(&m, &[], &[moved], &o),
        );
        l.refusal(&format!("{name} damped"), damped_modes(&m, &DampedOptions::new(4)));
        l.refusal(&format!("{name} all modes"), ModalBasis::all_modes(&m, None));
        let h = HarmonicOptions::default();
        l.refusal(
            &format!("{name} static correction"),
            HarmonicSolver::new(&m, basis.clone(), &h),
        );
        l.refusal(
            &format!("{name} static correction transformation"),
            HarmonicSolver::new(&t, basis.clone(), &h),
        );
        let free = strip(kind);
        l.refusal(
            &format!("{name} static correction free"),
            HarmonicSolver::new(&free, basis, &h),
        );
        let d = DirectHarmonicOptions::new();
        l.refusal(&format!("{name} direct plain"), DirectHarmonic::new(&m, &d));
        l.refusal(&format!("{name} direct transformation"), DirectHarmonic::new(&t, &d));
    }
}

#[test]
fn b1_the_default_bits_of_main() {
    let mut l = Lines(Vec::new());
    solids(&mut l);
    shells(&mut l);
    let text: String = l.0.iter().map(|(n, v)| format!("{n}\t{v}\n")).collect();
    print!("{text}");
    if let Ok(path) = std::env::var("F630_BITS_OUT") {
        std::fs::write(path, &text).unwrap();
    }
    let refusals: Vec<&(String, String)> = l.0.iter().filter(|(n, _)| n.starts_with("refusal")).collect();
    assert_eq!(refusals.len(), 20);
    assert!(refusals.iter().all(|(_, v)| v.starts_with("NotYetSupported")));
    let own = if cfg!(all(windows, target_arch = "x86_64")) {
        Some("sa_shells_linear_dynamics_bits_f630.tsv")
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Some("sa_shells_linear_dynamics_bits_f630.linux.tsv")
    } else {
        None
    };
    let file = own.unwrap_or("sa_shells_linear_dynamics_bits_f630.tsv");
    let record = std::fs::read_to_string(format!("{}/tests/{file}", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let recorded: Vec<(&str, &str)> = record.lines().map(|x| x.split_once('\t').unwrap()).collect();
    assert_eq!(recorded.len(), l.0.len(), "lines of the record");
    let mut differ = Vec::new();
    for ((n, v), (rn, rv)) in l.0.iter().zip(&recorded) {
        assert_eq!(n, rn, "order of the record");
        if (own.is_some() || n.starts_with("refusal")) && v != rv {
            differ.push(format!("{n}: {v}, main {rv}"));
        }
    }
    assert!(differ.is_empty(), "{} of {}: {differ:#?}", differ.len(), recorded.len());
}
