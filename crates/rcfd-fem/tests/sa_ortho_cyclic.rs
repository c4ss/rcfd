//! F-544 (ADR 0462): cyclic symmetry with the orthotropic and the tabulated material of ADR 0452 when every material with its
//! own axes has cylindrical axes about the cyclic axis. Checks F1 to F3t, W1 (sector against full rotor), I1 to I4 (isotropic
//! constants on the stocky ring), T1 (a uniform table), S3, A1, A2, R1 to R3 and the negative controls NC1 to NC3 of the
//! registration.

use rcfd_core::vec3::Vec3;
use rcfd_fem::buckling::{factors, BucklingOptions};
use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::prescribed_equations;
use rcfd_fem::cyclic::{
    full_rotor, modal_analysis_harmonic, nodal_forces_of, solve_static_harmonic, FullRotor, HarmonicLoad, HarmonicModes,
};
use rcfd_fem::geometric::Prestress;
use rcfd_fem::hex20::upgrade_to_hex20_mapped;
use rcfd_fem::loads::{nodal_load_vector, NodalForceAt};
use rcfd_fem::mat_ortho::FOLLOW_UP;
use rcfd_fem::mat_table::interpolate;
use rcfd_fem::material::{Axes, MaterialAxes};
use rcfd_fem::modal::{modal_analysis, modal_analysis_with, ModalOptions};
use rcfd_fem::mpc;
use rcfd_fem::rotating::{
    HarmonicSpeedModes, HarmonicWhirlSpeedModes, RotatingOptions, RotatingPart, RotatingSector, SpeedModes, WhirlModes,
};
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::transient::{Integrator, TransientOptions};
use rcfd_fem::{Constraint, FemError, Load, Material, SolidMesh, SolidModel};
use rcfd_linalg::eigen::ResidualTest;
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;

struct Checks {
    failed: Vec<String>,
}

impl Checks {
    fn new() -> Checks {
        Checks { failed: Vec::new() }
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {} ({detail})", if ok { "holds" } else { "FAILS" });
        if !ok {
            self.failed.push(format!("{name}: {detail}"));
        }
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{name}: {value:.3e} (limit {limit:.0e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.failed.push(format!("{name}: {value:e} > {limit:e}"));
        }
    }
    fn above(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value > limit;
        println!(
            "{name}: {value:.3e} (expected > {limit:.0e}) {}",
            if ok { "as expected" } else { "NOT AS EXPECTED" }
        );
        if !ok {
            self.failed.push(format!("{name}: {value:e} <= {limit:e}"));
        }
    }
    fn finish(self) {
        assert!(self.failed.is_empty(), "failed: {:?}", self.failed);
    }
}

const I3: Axes = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
const AXIS: Vec3 = [0.0, 0.0, 1.0];
const ORIGIN: Vec3 = [0.0; 3];
const E_ISO: f64 = 2.0e11;
const NU_ISO: f64 = 0.3;
const RHO_ISO: f64 = 7850.0;
const E_O: [f64; 3] = [1.4e11, 1.0e10, 1.0e10];
const NU_O: [f64; 3] = [0.3, 0.3, 0.45];
const G_O: [f64; 3] = [5.0e9, 5.0e9, 3.5e9];
const RHO_O: f64 = 1600.0;
const TABLE_T: [f64; 2] = [200.0, 600.0];
const TABLE_E: [f64; 2] = [2.0e11, 1.6e11];
const T_REF: f64 = 293.0;
/// The index of the cyclic constraint in every sector model (after the three clamps).
const J: usize = 3;

// ------------------------------------------------------------------------------------------------------------ materials

fn cyl(origin: Vec3, axis: Vec3) -> MaterialAxes {
    MaterialAxes::cylindrical(origin, axis).unwrap()
}

fn iso() -> Material {
    Material::isotropic_elastic(E_ISO, NU_ISO, RHO_ISO, 0.0).unwrap()
}

fn iso_e(e: f64) -> Material {
    Material::isotropic_elastic(e, NU_ISO, RHO_ISO, 0.0).unwrap()
}

/// `ISO_C` of D8.
fn iso_c() -> Material {
    let g = E_ISO / (2.0 * (1.0 + NU_ISO));
    Material::orthotropic([E_ISO; 3], [NU_ISO; 3], [g; 3], RHO_ISO, [0.0; 3], cyl(ORIGIN, AXIS)).unwrap()
}

fn ortho(axes: MaterialAxes) -> Material {
    Material::orthotropic(E_O, NU_O, G_O, RHO_O, [0.0; 3], axes).unwrap()
}

/// `ORTHO_C` of D8.
fn ortho_c() -> Material {
    ortho(cyl(ORIGIN, AXIS))
}

fn o_columns() -> Vec<(String, Vec<f64>)> {
    vec![
        ("e_1".to_string(), vec![1.4e11, 1.0e11]),
        ("g_12".to_string(), vec![5.0e9, 4.0e9]),
    ]
}

/// `TAB_O` of D8.
fn tab_o() -> Material {
    Material::tabulated(ortho_c(), TABLE_T.to_vec(), o_columns()).unwrap()
}

/// `TAB_OA` of D8.
fn tab_oa() -> Material {
    let base = Material::orthotropic(E_O, NU_O, G_O, RHO_O, [2.0e-5, 3.0e-5, 1.0e-5], cyl(ORIGIN, AXIS)).unwrap();
    Material::tabulated(base, TABLE_T.to_vec(), o_columns()).unwrap()
}

/// `TAB_U` of D8 (with `Field::Uniform`).
fn tab_u() -> Material {
    Material::tabulated(iso(), TABLE_T.to_vec(), vec![("e".to_string(), TABLE_E.to_vec())]).unwrap()
}

// ------------------------------------------------------------------------------------------------------------ models

#[derive(Clone, Copy, PartialEq)]
enum Shape {
    D,
    Ds,
    Sr,
    Sr6,
}

fn sectors(shape: Shape) -> usize {
    match shape {
        Shape::D | Shape::Sr => 12,
        Shape::Ds | Shape::Sr6 => 6,
    }
}

fn mesh_of(shape: Shape) -> SolidMesh {
    let a = 2.0 * PI / sectors(shape) as f64;
    let b = match shape {
        Shape::D => hex_box([0.25, 0.0, -0.1], [0.75, a, 0.2], [3, 2, 1]),
        Shape::Ds => hex_box([0.25, 0.0, -0.1], [0.75, a, 0.2], [2, 1, 1]),
        Shape::Sr => hex_box([0.5, 0.0, -0.25], [0.5, a, 0.5], [2, 2, 2]),
        Shape::Sr6 => hex_box([0.5, 0.0, -0.25], [0.5, a, 0.5], [2, 1, 2]),
    }
    .unwrap();
    upgrade_to_hex20_mapped(&b, |p| [p[0] * p[1].cos(), p[0] * p[1].sin(), p[2]])
        .unwrap()
        .0
}

#[derive(Clone, Copy, PartialEq)]
enum Field {
    None,
    /// `T = 293 + 250 (r - 0.25)/0.75 + 200 z` (D8).
    Radial,
    /// `T = 443`.
    Uniform,
}

fn field_value(field: Field, x: Vec3) -> f64 {
    match field {
        Field::None => unreachable!("no field"),
        Field::Radial => T_REF + 250.0 * ((x[0] * x[0] + x[1] * x[1]).sqrt() - 0.25) / 0.75 + 200.0 * x[2],
        Field::Uniform => 443.0,
    }
}

fn clamp(m: &mut SolidModel) {
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
}

/// The sector of `shape` with `mat` on `"all"`, `xmin` clamped, the cyclic constraint (index `J`) and the field.
fn sector(shape: Shape, mat: Material, field: Field) -> SolidModel {
    let mesh = mesh_of(shape);
    let t: Vec<f64> = if field == Field::None {
        Vec::new()
    } else {
        mesh.nodes().iter().map(|&x| field_value(field, x)).collect()
    };
    let mut m = SolidModel::new(mesh);
    m.add_material("all", mat).unwrap();
    clamp(&mut m);
    m.add_constraint(Constraint::cyclic("ymin", "ymax", AXIS, ORIGIN, sectors(shape)))
        .unwrap();
    if field != Field::None {
        m.add_load(Load::temperature(t, T_REF)).unwrap();
    }
    m
}

/// The full rotor of a sector of `shape` with `mat` and the sector field copied node by node (D4).
fn full_of(shape: Shape, sector_model: &SolidModel, mat: Material, field: Field) -> (FullRotor, SolidModel) {
    let rotor = full_rotor(sector_model).unwrap();
    let mut full = SolidModel::new(rotor.mesh().clone());
    full.add_material("all", mat).unwrap();
    clamp(&mut full);
    if field != Field::None {
        let xs = sector_model.mesh().nodes();
        let mut t = vec![f64::NAN; rotor.mesh().nodes().len()];
        for j in 0..sectors(shape) {
            for (p, &x) in xs.iter().enumerate() {
                t[rotor.node(j, p)] = field_value(field, x);
            }
        }
        assert!(t.iter().all(|v| v.is_finite()), "every full node has a temperature");
        full.add_load(Load::temperature(t, T_REF)).unwrap();
    }
    (rotor, full)
}

fn eq_opts(modes: usize) -> ModalOptions {
    let mut o = ModalOptions::new(modes).tolerance(1e-9);
    o.max_iterations = 400;
    o
}

fn opts(n: usize) -> ModalOptions {
    ModalOptions::new(n)
        .residual_test(ResidualTest::Backward)
        .tolerance(1e-12)
}

fn mv(m: &CsrMatrix, x: &[f64]) -> Vec<f64> {
    let mut y = vec![0.0; m.n];
    m.matvec(x, &mut y);
    y
}

fn quad(a: &CsrMatrix, x: &[f64]) -> f64 {
    mv(a, x).iter().zip(x).map(|(p, q)| p * q).sum()
}

fn inf(v: &[f64]) -> f64 {
    v.iter().fold(0.0f64, |m, x| m.max(x.abs()))
}

/// `max |a - b| / max |b|`.
fn dev(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len());
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max) / inf(b)
}

fn max_rel(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len());
    a.iter().zip(b).map(|(x, y)| (x / y - 1.0).abs()).fold(0.0, f64::max)
}

/// `max |sqrt(a/b) - 1|` over the first `min(len)` entries: the relative frequency of two eigenvalue lists.
fn worst_freq(a: &[f64], b: &[f64]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(s, f)| ((s / f).sqrt() - 1.0).abs())
        .fold(0.0f64, f64::max)
}

// ------------------------------------------------------------------------------------------------------------ the loads

/// The harmonic loads of D8 on a sector mesh: (cosine part, sine part), built on an isotropic twin (the loads take no
/// material); the interior target at radius `target_r` on the top face `z_top`.
fn parts(mesh: &SolidMesh, alpha: f64, target_r: f64, z_top: f64) -> (Vec<f64>, Vec<f64>) {
    let low0 = mesh.surface_set_nodes("ymin").unwrap()[0];
    let high0 = mesh.surface_set_nodes("ymax").unwrap()[0];
    let target = [target_r * (alpha / 2.0).cos(), target_r * (alpha / 2.0).sin(), z_top];
    let interior = (0..mesh.nodes().len())
        .min_by(|&a, &b| {
            let d = |p: usize| {
                let x = mesh.nodes()[p];
                (x[0] - target[0]).powi(2) + (x[1] - target[1]).powi(2) + (x[2] - target[2]).powi(2)
            };
            d(a).total_cmp(&d(b))
        })
        .unwrap();
    let twin = |load: Load| {
        let mut m = SolidModel::new(mesh.clone());
        m.add_material("all", iso()).unwrap();
        m.add_load(load).unwrap();
        m
    };
    let fa = nodal_load_vector(
        &twin(Load::pressure("xmax", 1e6)),
        &[
            NodalForceAt::new(low0, [1e4, -2e4, 3e4]),
            NodalForceAt::new(high0, [-5e3, 1e4, 2e4]),
        ],
    )
    .unwrap();
    let fb = nodal_load_vector(
        &twin(Load::traction("zmax", [2e5, -1e5, 5e5])),
        &[NodalForceAt::new(interior, [3e4, 1e4, -1e4])],
    )
    .unwrap();
    (fa, fb)
}

fn shape_parts(shape: Shape, mesh: &SolidMesh) -> (Vec<f64>, Vec<f64>) {
    let alpha = 2.0 * PI / sectors(shape) as f64;
    match shape {
        Shape::D | Shape::Ds => parts(mesh, alpha, 0.6, 0.1),
        Shape::Sr | Shape::Sr6 => parts(mesh, alpha, 0.75, 0.25),
    }
}

fn harmonic_load(shape: Shape, k: usize, fa: &[f64], fb: &[f64]) -> HarmonicLoad {
    let single = k == 0 || 2 * k == sectors(shape);
    let sin = if single { vec![0.0; fb.len()] } else { fb.to_vec() };
    HarmonicLoad::new(k, fa.to_vec(), sin)
}

// --------------------------------------------------------------------------------------------------- F1, F1t, NC1, NC2

/// The procedure of ADR 0386 E2 for one sector model and its full model (F1 or F1t); returns the sorted sector eigenvalues
/// below the cut-off.
fn modal_equivalence(
    tag: &str,
    sector_model: &SolidModel,
    rotor: &FullRotor,
    full: &SolidModel,
    ch: &mut Checks,
) -> Vec<f64> {
    let nsec = rotor.sectors();
    let harmonics: Vec<HarmonicModes> = (0..=nsec / 2)
        .map(|k| modal_analysis_harmonic(sector_model, &[], k, &eq_opts(4)).unwrap())
        .collect();
    for h in &harmonics {
        println!(
            "{tag} k = {}: multiplicity {}, f = {:?} Hz, pair splits {:?}",
            h.harmonic, h.multiplicity, h.frequencies, h.pair_splits
        );
    }
    let lambda_c = 0.999
        * harmonics
            .iter()
            .map(|h| h.values.iter().fold(0.0f64, |m, &v| m.max(v)))
            .fold(f64::INFINITY, f64::min);
    let mut below: Vec<(f64, usize, usize)> = Vec::new();
    for h in &harmonics {
        for (i, &l) in h.eigenvalues.iter().enumerate() {
            if l < lambda_c {
                below.push((l, h.harmonic, i));
            }
        }
    }
    below.sort_by(|a, b| a.0.total_cmp(&b.0));
    let n_c = below.len();
    let fr = modal_analysis(full, &eq_opts(n_c + 8)).unwrap();
    println!(
        "{tag} full: {} unknowns, {} modes, largest {:e}, cut-off {lambda_c:e}, n_c {n_c}",
        fr.unknowns,
        fr.eigenvalues.len(),
        fr.eigenvalues.last().unwrap()
    );
    assert!(
        *fr.eigenvalues.last().unwrap() > lambda_c,
        "defective set-up: the full model's computed modes do not pass the cut-off"
    );
    let near = below
        .iter()
        .map(|b| b.0)
        .chain(fr.eigenvalues.iter().copied())
        .map(|l| (l / lambda_c - 1.0).abs())
        .fold(f64::INFINITY, f64::min);
    assert!(
        near >= 1e-7,
        "defective set-up: an eigenvalue within 1e-7 of the cut-off"
    );
    let full_below: Vec<f64> = fr.eigenvalues.iter().copied().filter(|&l| l < lambda_c).collect();
    ch.holds(
        &format!("{tag} count"),
        full_below.len() == n_c,
        &format!("full {} = sector {n_c}", full_below.len()),
    );
    let sorted: Vec<f64> = below.iter().map(|b| b.0).collect();
    ch.at_most(&format!("{tag} frequencies"), worst_freq(&sorted, &full_below), 1e-8);
    let split = harmonics
        .iter()
        .filter(|h| h.multiplicity == 2)
        .flat_map(|h| h.pair_splits.iter().copied())
        .fold(0.0f64, f64::max);
    ch.at_most(&format!("{tag} pair splits"), split, 1e-8);
    // The residual of each expanded mode on the full rotor's K (RotatingPart::operator at 0) and M, free rows.
    let part = RotatingPart::new(
        full,
        AXIS,
        ORIGIN,
        &RotatingOptions::new(2).modal(eq_opts(2)).stress_stiffening(false),
    )
    .unwrap();
    let k = part.operator(0.0);
    let m = part.mass();
    let sdofs = sector_model.dof_map();
    let fdofs = full.dof_map();
    let mut free = vec![true; fdofs.n_equations()];
    for (e, _) in prescribed_equations(full, &[]).unwrap() {
        free[e] = false;
    }
    let mut worst: f64 = 0.0;
    for &(l, kh, i) in &below {
        let phi = rotor.expand(&sdofs, &fdofs, kh, &harmonics[kh].modes[i]);
        let (kp, mp) = (mv(&k, &phi), mv(m, &phi));
        let (mut rn, mut kn) = (0.0, 0.0);
        for e in 0..phi.len() {
            if free[e] {
                rn += (kp[e] - l * mp[e]).powi(2);
                kn += kp[e].powi(2);
            }
        }
        worst = worst.max((rn / kn).sqrt());
    }
    ch.at_most(&format!("{tag} expanded residual"), worst, 1e-7);
    sorted
}

#[test]
fn f1_f1t_nc1_nc2_modal_sector_against_full_rotor() {
    let mut ch = Checks::new();
    let s = sector(Shape::D, ortho_c(), Field::None);
    let (rotor, full) = full_of(Shape::D, &s, ortho_c(), Field::None);
    let sorted = modal_equivalence("F1 ORTHO_C", &s, &rotor, &full, &mut ch);
    // NC1, NC2: full rotors of other readings of the material.
    for (name, mat, limit) in [
        ("NC1 Fixed(I)", ortho(MaterialAxes::fixed(I3).unwrap()), 1e-2),
        (
            "NC2 cylindrical through (0.05, 0, 0)",
            ortho(cyl([0.05, 0.0, 0.0], AXIS)),
            1e-4,
        ),
    ] {
        let (_, other) = full_of(Shape::D, &s, mat, Field::None);
        let fr = modal_analysis(&other, &eq_opts(sorted.len() + 8)).unwrap();
        ch.above(name, worst_freq(&sorted, &fr.eigenvalues[..sorted.len()]), limit);
    }
    let st = sector(Shape::D, tab_o(), Field::Radial);
    let (rotor_t, full_t) = full_of(Shape::D, &st, tab_o(), Field::Radial);
    modal_equivalence("F1t TAB_O", &st, &rotor_t, &full_t, &mut ch);
    ch.finish();
}

// --------------------------------------------------------------------------------------------------- F2, F2t, NC3

/// F2 or F2t: every index of the sector against the full rotor under the expanded load; returns `e_u` of `k = 1` against the
/// full rotor `nc` (NC3) when given.
fn static_equivalence(
    tag: &str,
    sector_model: &SolidModel,
    rotor: &FullRotor,
    full: &SolidModel,
    nc: Option<&SolidModel>,
    ch: &mut Checks,
) -> Option<f64> {
    let sdofs = sector_model.dof_map();
    let fdofs = full.dof_map();
    let (fa, fb) = shape_parts(Shape::D, sector_model.mesh());
    let o = StaticOptions::default();
    let mut worst: f64 = 0.0;
    let mut nc_value = None;
    for k in 0..=rotor.sectors() / 2 {
        let load = harmonic_load(Shape::D, k, &fa, &fb);
        let hs = solve_static_harmonic(sector_model, &[], &load, &o).unwrap();
        let f_full = rotor.expand_load(&sdofs, &fdofs, k, &load.cos, &load.sin);
        let forces = nodal_forces_of(&fdofs, &f_full);
        let u_exp = rotor.expand_field(&sdofs, &fdofs, k, &hs.cos, &hs.sin);
        let fs = mpc::solve_static_mpc(full, &[], &forces, &[], &o).unwrap();
        let e_u = dev(&u_exp, fs.result.displacement_values());
        println!("{tag} k = {k}: e_u {e_u:.3e}, residual {:.2e}", hs.relative_residual);
        worst = worst.max(e_u);
        if let (Some(other), 1) = (nc, k) {
            let fo = mpc::solve_static_mpc(other, &[], &forces, &[], &o).unwrap();
            nc_value = Some(dev(&u_exp, fo.result.displacement_values()));
        }
    }
    ch.at_most(&format!("{tag} e_u"), worst, 1e-8);
    nc_value
}

#[test]
fn f2_f2t_nc3_static_sector_against_full_rotor() {
    let mut ch = Checks::new();
    let s = sector(Shape::D, ortho_c(), Field::None);
    let (rotor, full) = full_of(Shape::D, &s, ortho_c(), Field::None);
    let (_, fixed) = full_of(Shape::D, &s, ortho(MaterialAxes::fixed(I3).unwrap()), Field::None);
    let nc3 = static_equivalence("F2 ORTHO_C", &s, &rotor, &full, Some(&fixed), &mut ch).unwrap();
    ch.above("NC3 full rotor with Fixed(I) at k = 1", nc3, 1e-2);
    let st = sector(Shape::D, tab_o(), Field::Radial);
    let (rotor_t, full_t) = full_of(Shape::D, &st, tab_o(), Field::Radial);
    static_equivalence("F2t TAB_O", &st, &rotor_t, &full_t, None, &mut ch);
    ch.finish();
}

// --------------------------------------------------------------------------------------------------- F3, F3t

fn rot_eq(modes: usize) -> RotatingOptions {
    RotatingOptions::new(modes).modal(eq_opts(modes))
}

fn harmonic_modes(r: HarmonicSpeedModes, what: &str) -> HarmonicModes {
    match r {
        HarmonicSpeedModes::Modes(h) => *h,
        HarmonicSpeedModes::LossOfStiffness(l) => panic!("defective set-up: {what}: loss of stiffness {l:?}"),
    }
}

fn below_cutoff(hs: &[HarmonicModes]) -> (Vec<f64>, f64) {
    let lambda_c = 0.999
        * hs.iter()
            .map(|h| h.values.iter().fold(0.0f64, |m, &v| m.max(v)))
            .fold(f64::INFINITY, f64::min);
    let mut below: Vec<f64> = hs
        .iter()
        .flat_map(|h| h.eigenvalues.iter().copied())
        .filter(|&l| l < lambda_c)
        .collect();
    below.sort_by(f64::total_cmp);
    (below, lambda_c)
}

fn rotating_equivalence(tag: &str, sector_model: &SolidModel, rotor: &FullRotor, full: &SolidModel, ch: &mut Checks) {
    let nsec = rotor.sectors();
    let rs = RotatingSector::new(sector_model, &rot_eq(4)).unwrap();
    let rest: Vec<HarmonicModes> = (0..=nsec / 2)
        .map(|k| harmonic_modes(rs.modes(k, 0.0).unwrap(), "rest"))
        .collect();
    let w_ref = rest.iter().map(|h| h.values[0]).fold(f64::INFINITY, f64::min).sqrt();
    println!("{tag}: omega_ref = {w_ref:.9e} rad/s");
    let n_full = below_cutoff(&rest).0.len() + 8;
    let fp = RotatingPart::new(full, AXIS, ORIGIN, &rot_eq(n_full)).unwrap();
    let sdofs = sector_model.dof_map();
    let fdofs = full.dof_map();
    for factor in [0.0, 0.5, 0.9] {
        let w = factor * w_ref;
        let hs: Vec<HarmonicModes> = (0..=nsec / 2)
            .map(|k| harmonic_modes(rs.modes(k, w).unwrap(), "comparison speed"))
            .collect();
        let (below, lambda_c) = below_cutoff(&hs);
        let fr = match fp.modes(w).unwrap() {
            SpeedModes::Modes(r) => r,
            SpeedModes::LossOfStiffness(l) => panic!("defective set-up: the full rotor loses stiffness at {w}: {l:?}"),
        };
        assert!(
            *fr.eigenvalues.last().unwrap() > lambda_c,
            "defective set-up: the full model's computed modes do not pass the cut-off"
        );
        let near = below
            .iter()
            .chain(fr.eigenvalues.iter())
            .map(|l| (l / lambda_c - 1.0).abs())
            .fold(f64::INFINITY, f64::min);
        assert!(
            near >= 1e-7,
            "defective set-up: an eigenvalue within 1e-7 of the cut-off"
        );
        let full_below: Vec<f64> = fr.eigenvalues.iter().copied().filter(|&l| l < lambda_c).collect();
        ch.holds(
            &format!("{tag} {factor} omega_ref count"),
            full_below.len() == below.len(),
            &format!("full {} = sector {}", full_below.len(), below.len()),
        );
        ch.at_most(
            &format!("{tag} {factor} omega_ref frequencies"),
            worst_freq(&below, &full_below),
            1e-8,
        );
        let keff = fp.operator(w);
        let mut worst: f64 = 0.0;
        for h in &hs {
            for (i, mode) in h.modes.iter().enumerate() {
                let x = rotor.expand(&sdofs, &fdofs, h.harmonic, mode);
                let rq = quad(&keff, &x) / quad(fp.mass(), &x);
                worst = worst.max((rq / h.eigenvalues[i] - 1.0).abs());
            }
        }
        ch.at_most(&format!("{tag} {factor} omega_ref Rayleigh quotients"), worst, 1e-8);
    }
}

#[test]
fn f3_f3t_rotating_sector_against_full_rotor() {
    let mut ch = Checks::new();
    let s = sector(Shape::D, ortho_c(), Field::None);
    let (rotor, full) = full_of(Shape::D, &s, ortho_c(), Field::None);
    rotating_equivalence("F3 ORTHO_C", &s, &rotor, &full, &mut ch);
    let st = sector(Shape::D, tab_o(), Field::Radial);
    let (rotor_t, full_t) = full_of(Shape::D, &st, tab_o(), Field::Radial);
    rotating_equivalence("F3t TAB_O", &st, &rotor_t, &full_t, &mut ch);
    ch.finish();
}

// --------------------------------------------------------------------------------------------------- W1

fn whirl_modes(r: HarmonicWhirlSpeedModes, what: &str) -> WhirlModes {
    match r {
        HarmonicWhirlSpeedModes::Modes { modes, .. } => modes,
        HarmonicWhirlSpeedModes::LossOfStiffness(l) => panic!("defective set-up: {what}: loss of stiffness {l:?}"),
    }
}

#[test]
fn w1_sector_whirl_against_full_rotor() {
    let mut ch = Checks::new();
    let s = sector(Shape::Ds, ortho_c(), Field::None);
    let (_, full) = full_of(Shape::Ds, &s, ortho_c(), Field::None);
    let o = RotatingOptions::new(4).modal(eq_opts(4)).coriolis(true);
    let rs = RotatingSector::new(&s, &o).unwrap();
    let w_ref = (0..=3)
        .map(|k| whirl_modes(rs.whirl_modes(k, 0.0).unwrap(), "rest").modes[0].omega)
        .fold(f64::INFINITY, f64::min);
    println!("W1: omega_ref = {w_ref:.9e} rad/s");
    let fp = RotatingPart::new(&full, AXIS, ORIGIN, &o).unwrap();
    for factor in [0.4, 0.8] {
        let w = factor * w_ref;
        let mut all: Vec<f64> = (0..=3)
            .flat_map(|k| whirl_modes(rs.whirl_modes(k, w).unwrap(), "W1").frequencies_all)
            .collect();
        all.sort_by(f64::total_cmp);
        let fr = match fp.whirl_modes(w).unwrap().modes() {
            Some(m) => m.clone(),
            None => panic!("defective set-up: the full rotor loses stiffness at {w}"),
        };
        ch.holds(
            &format!("W1 {factor} omega_ref count"),
            all.len() == fr.frequencies_all.len(),
            &format!("{} = {}", all.len(), fr.frequencies_all.len()),
        );
        ch.at_most(
            &format!("W1 {factor} omega_ref frequencies"),
            max_rel(&all, &fr.frequencies_all),
            1e-8,
        );
    }
    ch.finish();
}

// --------------------------------------------------------------------------------------------------- I1 to I4, T1

fn harmonic_frequencies(m: &SolidModel) -> Vec<Vec<f64>> {
    (0..=6)
        .map(|k| modal_analysis_harmonic(m, &[], k, &opts(4)).unwrap().frequencies)
        .collect()
}

fn harmonic_statics(m: &SolidModel) -> Vec<Vec<f64>> {
    let (fa, fb) = shape_parts(Shape::Sr, m.mesh());
    (0..=6)
        .map(|k| {
            let r = solve_static_harmonic(
                m,
                &[],
                &harmonic_load(Shape::Sr, k, &fa, &fb),
                &StaticOptions::default(),
            )
            .unwrap();
            r.cos.iter().chain(&r.sin).copied().collect()
        })
        .collect()
}

fn rest_omega(m: &SolidModel) -> f64 {
    (0..=6)
        .map(|k| modal_analysis_harmonic(m, &[], k, &opts(4)).unwrap().values[0])
        .fold(f64::INFINITY, f64::min)
        .sqrt()
}

/// The rotating sector's frequencies of `k = 0 .. 6` at `w`, and its unit pre-stress.
fn rotating_frequencies(m: &SolidModel, w: f64) -> (Vec<Vec<f64>>, Vec<f64>) {
    let rs = RotatingSector::new(m, &RotatingOptions::new(4).modal(opts(4))).unwrap();
    let f = (0..=6)
        .map(|k| harmonic_modes(rs.modes(k, w).unwrap(), "I3").frequencies)
        .collect();
    let s = rs.part().unit_prestress().stress().iter().flatten().copied().collect();
    (f, s)
}

fn whirl_rest_omega(m: &SolidModel) -> f64 {
    let rs = RotatingSector::new(m, &RotatingOptions::new(4).modal(opts(4)).coriolis(true)).unwrap();
    (0..=3)
        .map(|k| whirl_modes(rs.whirl_modes(k, 0.0).unwrap(), "rest").modes[0].omega)
        .fold(f64::INFINITY, f64::min)
}

fn whirl_omegas(m: &SolidModel, w: f64) -> Vec<Vec<f64>> {
    let rs = RotatingSector::new(m, &RotatingOptions::new(4).modal(opts(4)).coriolis(true)).unwrap();
    (0..=3)
        .map(|k| {
            whirl_modes(rs.whirl_modes(k, w).unwrap(), "I4")
                .modes
                .iter()
                .map(|x| x.omega)
                .collect()
        })
        .collect()
}

fn worst_of(a: &[Vec<f64>], b: &[Vec<f64>], f: fn(&[f64], &[f64]) -> f64) -> f64 {
    a.iter().zip(b).map(|(x, y)| f(x, y)).fold(0.0, f64::max)
}

/// I1 to I4 (or T1) of `mat` (with `field`) against `base` on SR and SR6.
fn equivalence_on_ring(tag: &str, mat: Material, field: Field, base: Material, prestress: bool, ch: &mut Checks) {
    let a = sector(Shape::Sr, mat.clone(), field);
    let b = sector(Shape::Sr, base.clone(), Field::None);
    ch.at_most(
        &format!("{tag} modal harmonic"),
        worst_of(&harmonic_frequencies(&a), &harmonic_frequencies(&b), max_rel),
        1e-12,
    );
    ch.at_most(
        &format!("{tag} static harmonic"),
        worst_of(&harmonic_statics(&a), &harmonic_statics(&b), dev),
        1e-12,
    );
    let w = 0.5 * rest_omega(&b);
    let (fa, sa) = rotating_frequencies(&a, w);
    let (fb, sb) = rotating_frequencies(&b, w);
    ch.at_most(&format!("{tag} rotating sector"), worst_of(&fa, &fb, max_rel), 1e-12);
    if prestress {
        ch.at_most(&format!("{tag} rotating sector unit pre-stress"), dev(&sa, &sb), 1e-12);
    }
    let a6 = sector(Shape::Sr6, mat, field);
    let b6 = sector(Shape::Sr6, base, Field::None);
    let w6 = 0.4 * whirl_rest_omega(&b6);
    ch.at_most(
        &format!("{tag} sector whirl"),
        worst_of(&whirl_omegas(&a6, w6), &whirl_omegas(&b6, w6), max_rel),
        1e-12,
    );
}

#[test]
fn i1_to_i4_t1_isotropic_constants_and_uniform_table_on_the_ring() {
    let mut ch = Checks::new();
    equivalence_on_ring("I ISO_C", iso_c(), Field::None, iso(), false, &mut ch);
    let e443 = interpolate(&TABLE_T, &TABLE_E, 443.0).unwrap();
    equivalence_on_ring("T1 TAB_U", tab_u(), Field::Uniform, iso_e(e443), true, &mut ch);
    ch.finish();
}

// --------------------------------------------------------------------------------------------------- S3, A1, A2

fn same(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x == y)
}

#[test]
fn s3_a1_a2_thermal_identity_and_axes_variants() {
    let mut ch = Checks::new();
    // S3: the thermal load of a table's expansion at k = 0 on the harmonic path against the static solve.
    let m = sector(Shape::D, tab_oa(), Field::Radial);
    let f_th = nodal_load_vector(&m, &[]).unwrap();
    let hs = solve_static_harmonic(
        &m,
        &[],
        &HarmonicLoad::new(0, f_th.clone(), vec![0.0; f_th.len()]),
        &StaticOptions::default(),
    )
    .unwrap();
    let st = solve_static(&m, &StaticOptions::default()).unwrap();
    println!("S3: max |u| {:.4e}", inf(st.displacement_values()));
    ch.at_most(
        "S3 k = 0 with the thermal load against solve_static",
        dev(&hs.cos, st.displacement_values()),
        1e-10,
    );
    // A1: the axis reversed and the origin moved along the axis give the same numbers.
    let base = sector(Shape::D, ortho_c(), Field::None);
    let (fa, fb) = shape_parts(Shape::D, base.mesh());
    let load = harmonic_load(Shape::D, 1, &fa, &fb);
    let hm = modal_analysis_harmonic(&base, &[], 1, &opts(3)).unwrap();
    let hst = solve_static_harmonic(&base, &[], &load, &StaticOptions::default()).unwrap();
    for (name, axes) in [
        ("axis (0, 0, -1)", cyl(ORIGIN, [0.0, 0.0, -1.0])),
        ("origin (0, 0, 0.3)", cyl([0.0, 0.0, 0.3], AXIS)),
    ] {
        let v = sector(Shape::D, ortho(axes), Field::None);
        match (
            modal_analysis_harmonic(&v, &[], 1, &opts(3)),
            solve_static_harmonic(&v, &[], &load, &StaticOptions::default()),
        ) {
            (Ok(h), Ok(s)) => {
                let modes_same = h
                    .modes
                    .iter()
                    .zip(&hm.modes)
                    .all(|(x, y)| same(&x.cos, &y.cos) && same(&x.sin, &y.sin));
                ch.holds(
                    &format!("A1 {name} modal k = 1"),
                    same(&h.eigenvalues, &hm.eigenvalues) && modes_same,
                    "eigenvalues and modes equal as numbers",
                );
                ch.holds(
                    &format!("A1 {name} static k = 1"),
                    same(&s.cos, &hst.cos) && same(&s.sin, &hst.sin),
                    "both parts equal as numbers",
                );
            }
            (a, b) => ch.holds(
                &format!("A1 {name}"),
                false,
                &format!("modal {:?}, static {:?}", a.err(), b.err()),
            ),
        }
    }
    // A2: a tilt inside the tolerance is accepted.
    let t = sector(Shape::D, ortho(cyl(ORIGIN, [0.0, 1e-11, 1.0])), Field::None);
    let r = modal_analysis_harmonic(&t, &[], 1, &opts(3));
    ch.holds("A2 axis (0, 1e-11, 1) solves", r.is_ok(), &format!("{:?}", r.err()));
    ch.finish();
}

// --------------------------------------------------------------------------------------------------- R1 to R3

fn cyclic_refusal(r: &rcfd_fem::Result<()>, variant: &str) -> bool {
    let want = format!("material 0, element set 'all', with the cyclic constraint {J}");
    matches!(r, Err(FemError::NotYetSupported { item: "material", variant: v, location: l, package: p })
        if *v == variant && *l == want && *p == FOLLOW_UP)
}

#[test]
fn r1_r2_r3_refusals() {
    let mut ch = Checks::new();
    let fixed_table =
        Material::tabulated(ortho(MaterialAxes::fixed(I3).unwrap()), TABLE_T.to_vec(), o_columns()).unwrap();
    let cases: Vec<(&str, Material, Field, &str)> = vec![
        (
            "Fixed(I)",
            ortho(MaterialAxes::fixed(I3).unwrap()),
            Field::None,
            "Orthotropic",
        ),
        (
            "tilted (0, 0.1, 1)",
            ortho(cyl(ORIGIN, [0.0, 0.1, 1.0])),
            Field::None,
            "Orthotropic",
        ),
        (
            "tilted (0, 1e-9, 1)",
            ortho(cyl(ORIGIN, [0.0, 1e-9, 1.0])),
            Field::None,
            "Orthotropic",
        ),
        (
            "origin (0.05, 0, 0)",
            ortho(cyl([0.05, 0.0, 0.0], AXIS)),
            Field::None,
            "Orthotropic",
        ),
        (
            "PerElement",
            ortho(MaterialAxes::per_element(vec![I3; 6]).unwrap()),
            Field::None,
            "Orthotropic",
        ),
        ("Tabulated on Fixed(I)", fixed_table, Field::Radial, "Tabulated"),
    ];
    for (name, mat, field, variant) in cases {
        let m = sector(Shape::D, mat, field);
        let n = m.dof_map().n_equations();
        let zero = |k: usize| HarmonicLoad::new(k, vec![0.0; n], vec![0.0; n]);
        let mut spun = m.clone();
        spun.add_load(Load::centrifugal(AXIS, ORIGIN, 100.0)).unwrap();
        let rows: Vec<(&str, rcfd_fem::Result<()>)> = vec![
            (
                "modal_analysis_harmonic k = 0",
                modal_analysis_harmonic(&m, &[], 0, &ModalOptions::new(2)).map(|_| ()),
            ),
            (
                "modal_analysis_harmonic k = 1",
                modal_analysis_harmonic(&m, &[], 1, &ModalOptions::new(2)).map(|_| ()),
            ),
            (
                "solve_static_harmonic k = 0",
                solve_static_harmonic(&m, &[], &zero(0), &StaticOptions::default()).map(|_| ()),
            ),
            (
                "solve_static_harmonic k = 1",
                solve_static_harmonic(&m, &[], &zero(1), &StaticOptions::default()).map(|_| ()),
            ),
            (
                "RotatingSector::new",
                RotatingSector::new(&m, &RotatingOptions::new(2)).map(|_| ()),
            ),
            (
                "RotatingSector::with_unit_prestress",
                RotatingSector::with_unit_prestress(
                    &m,
                    Prestress::uniform(&m, [0.0; 6]).unwrap(),
                    &RotatingOptions::new(2),
                )
                .map(|_| ()),
            ),
            (
                "modal_analysis_with",
                modal_analysis_with(&m, &[], &ModalOptions::new(2)).map(|_| ()),
            ),
            (
                "solve_static (centrifugal)",
                solve_static(&spun, &StaticOptions::default()).map(|_| ()),
            ),
            (
                "RotatingPart::new",
                RotatingPart::new(&m, AXIS, ORIGIN, &RotatingOptions::new(2)).map(|_| ()),
            ),
        ];
        for (entry, r) in rows {
            ch.holds(
                &format!("R1 {name}: {entry} refused"),
                cyclic_refusal(&r, variant),
                &format!("{:?}", r.err()),
            );
        }
    }
    // R2: buckling and the transient integrator keep refusing coaxial cylindrical axes.
    let m = sector(Shape::D, ortho_c(), Field::None);
    let rows: Vec<(&str, rcfd_fem::Result<()>)> = vec![
        (
            "buckling::factors",
            factors(
                &m,
                &Prestress::uniform(&m, [-1e6, 0.0, 0.0, 0.0, 0.0, 0.0]).unwrap(),
                &BucklingOptions::new(1),
            )
            .map(|_| ()),
        ),
        (
            "Integrator::new",
            Integrator::new(&m, &[], &TransientOptions::new(1e-4)).map(|_| ()),
        ),
    ];
    for (entry, r) in rows {
        ch.holds(
            &format!("R2 ORTHO_C: {entry} refused"),
            cyclic_refusal(&r, "Orthotropic"),
            &format!("{:?}", r.err()),
        );
    }
    // R3: a temperature field without a table is refused by the static harmonic solve as before.
    for (name, mat) in [("ISO", iso()), ("ORTHO_C", ortho_c())] {
        let m = sector(Shape::D, mat, Field::Radial);
        let n = m.dof_map().n_equations();
        let r = solve_static_harmonic(
            &m,
            &[],
            &HarmonicLoad::new(0, vec![0.0; n], vec![0.0; n]),
            &StaticOptions::default(),
        );
        let ok = matches!(&r, Err(FemError::InvalidValue { key: "loads", reason })
            if reason.starts_with("the model has 1 loads of its own; a harmonic static solve takes its load"));
        ch.holds(
            &format!("R3 {name} with a temperature field: solve_static_harmonic refused (loads)"),
            ok,
            &format!("{:?}", r.as_ref().err()),
        );
    }
    ch.finish();
}
