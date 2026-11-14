//! F-542 (ADR 0459): the orthotropic and the tabulated material of ADR 0452 on the rotating modes, the buckling factors and
//! the transient integrator. Checks RB1 to RB4, RC2, RC2t, RQ1, RP1, RP1t, W1, BK1 to BK4, TR1 to TR3, R1 (the copy of ADR
//! 0457's R1), R3 (the copy of ADR 0457's R3 with its rows) and the negative controls NB1 to NB5 and NR1 of the registration.

use rcfd_core::vec3::Vec3;
use rcfd_fem::assemble::stiffness;
use rcfd_fem::buckling::{factors, BucklingOptions};
use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::cyclic::modal_analysis_harmonic;
use rcfd_fem::damped::{damped_modes, DampedOptions};
use rcfd_fem::geometric::{geometric_stiffness, prestressed_modes, Prestress};
use rcfd_fem::harmonic_direct::{DirectHarmonic, DirectHarmonicOptions};
use rcfd_fem::hex20::{upgrade_to_hex20, upgrade_to_hex20_mapped};
use rcfd_fem::loads::NodalForceAt;
use rcfd_fem::mass::consistent;
use rcfd_fem::mat_ortho::FOLLOW_UP;
use rcfd_fem::mat_table::interpolate;
use rcfd_fem::material::MaterialAxes;
use rcfd_fem::modal::{modal_analysis, modal_analysis_with, ModalOptions, ModalResult};
use rcfd_fem::model::{ElasticSupport, LinearTerm};
use rcfd_fem::mpc::{modal_analysis_mpc, solve_static_mpc};
use rcfd_fem::nonlinear::{solve, NewtonOptions};
use rcfd_fem::rotating::{RotatingOptions, RotatingPart, RotatingSector, SpeedModes};
use rcfd_fem::solve::{solve_static, solve_static_with, CgPreconditioner, StaticOptions, StaticSolver};
use rcfd_fem::transient::{Integrator, LoadHistory, LoadTable, Method, Rayleigh, TransientOptions};
use rcfd_fem::{BlockProperty, Constraint, ElementKind, FemError, Load, Material, SolidMesh, SolidModel};
use rcfd_linalg::eigen::{ResidualTest, Shift};
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
        println!("{name}: {} ({detail})", if ok { "met" } else { "MISSED" });
        if !ok {
            self.failed.push(name.to_string());
        }
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{name}: {value:.4e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.failed.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    fn above(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value > limit;
        println!(
            "{name}: {value:.4e} (expected > {limit:e}) {}",
            if ok { "as expected" } else { "NOT AS EXPECTED" }
        );
        if !ok {
            self.failed.push(format!("{name} = {value:e} <= {limit:e}"));
        }
    }
    fn below(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value < limit;
        println!(
            "{name}: {value:.4e} (expected < {limit:e}) {}",
            if ok { "as expected" } else { "NOT AS EXPECTED" }
        );
        if !ok {
            self.failed.push(format!("{name} = {value:e} >= {limit:e}"));
        }
    }
    fn finish(self) {
        assert!(self.failed.is_empty(), "missed: {:?}", self.failed);
    }
}

// ------------------------------------------------------------------------------------------------------- helpers

type Axes = [[f64; 3]; 3];

const I3: Axes = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
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
const AXIS_Z: Vec3 = [0.0, 0.0, 1.0];
/// The rotation axis of the blocks passes through `O_B` (D7).
const O_B: Vec3 = [-0.5, 0.2, 0.0];

/// Rodrigues rotation matrix (rows orthonormal, right-handed): the `rot` of `sa_ortho.rs`.
fn rot(axis: Vec3, angle: f64) -> Axes {
    let n = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
    let k = [axis[0] / n, axis[1] / n, axis[2] / n];
    let (s, co) = angle.sin_cos();
    let mut r = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            let kk = [[0.0, -k[2], k[1]], [k[2], 0.0, -k[0]], [-k[1], k[0], 0.0]][i][j];
            r[i][j] = (if i == j { co } else { 0.0 }) + s * kk + (1.0 - co) * k[i] * k[j];
        }
    }
    r
}

fn fixed(r: Axes) -> MaterialAxes {
    MaterialAxes::fixed(r).unwrap()
}

fn iso_e(e: f64) -> Material {
    Material::isotropic_elastic(e, NU_ISO, RHO_ISO, 0.0).unwrap()
}

/// Isotropic constants in an orthotropic material with the given axes.
fn iso_o_axes(axes: MaterialAxes) -> Material {
    let g = E_ISO / (2.0 * (1.0 + NU_ISO));
    Material::orthotropic([E_ISO; 3], [NU_ISO; 3], [g; 3], RHO_ISO, [0.0; 3], axes).unwrap()
}

/// `ISO_O` of D7.
fn iso_o(r: Axes) -> Material {
    iso_o_axes(fixed(r))
}

/// `ISO_C` of D7: the isotropic constants with cylindrical axes about the rotation axis of the blocks.
fn iso_c() -> Material {
    iso_o_axes(MaterialAxes::cylindrical(O_B, AXIS_Z).unwrap())
}

fn ortho(axes: MaterialAxes) -> Material {
    Material::orthotropic(E_O, NU_O, G_O, RHO_O, [0.0; 3], axes).unwrap()
}

fn cyl() -> Material {
    ortho(MaterialAxes::cylindrical([0.5, -1.0, 0.0], [0.0, 0.0, 1.0]).unwrap())
}

fn tab_of(base: Material) -> Material {
    Material::tabulated(base, TABLE_T.to_vec(), vec![("e".to_string(), TABLE_E.to_vec())]).unwrap()
}

/// `TAB` (with `Field::Linear`) and `TAB_U` (with `Field::Uniform`).
fn tab() -> Material {
    tab_of(iso_e(E_ISO))
}

/// `TAB_A`: the base with the expansion 1.2e-5 (with `Field::Uniform`).
fn tab_a() -> Material {
    tab_of(Material::isotropic_elastic(E_ISO, NU_ISO, RHO_ISO, 1.2e-5).unwrap())
}

#[derive(Clone, Copy)]
enum Field {
    None,
    Linear,
    Uniform,
}

fn add_field(m: &mut SolidModel, field: Field) {
    let t: Vec<f64> = match field {
        Field::None => return,
        Field::Linear => m.mesh().nodes().iter().map(|x| T_REF + 250.0 * x[0]).collect(),
        Field::Uniform => vec![443.0; m.mesh().nodes().len()],
    };
    m.add_load(Load::temperature(t, T_REF)).unwrap();
}

fn opts(n: usize) -> ModalOptions {
    ModalOptions::new(n)
        .residual_test(ResidualTest::Backward)
        .tolerance(1e-12)
}

fn rot_opts(n: usize) -> RotatingOptions {
    RotatingOptions::new(n).modal(opts(n))
}

fn max_rel(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len());
    a.iter().zip(b).map(|(x, y)| (x / y - 1.0).abs()).fold(0.0, f64::max)
}

/// `max |a - b| / max |b|`.
fn dev(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len());
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max) / b.iter().map(|y| y.abs()).fold(0.0, f64::max)
}

fn matvec(a: &CsrMatrix, x: &[f64]) -> Vec<f64> {
    let mut y = vec![0.0; a.n];
    a.matvec(x, &mut y);
    y
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn modes_of(s: SpeedModes) -> ModalResult {
    match s {
        SpeedModes::Modes(r) => r,
        SpeedModes::LossOfStiffness(l) => panic!("unexpected loss of stiffness {l:?}"),
    }
}

/// The block `BLK` of D7: Hex20 4 x 2 x 1 on `1 x 0.4 x 0.25`, every component of `xmin` held.
fn block(mat: Material, field: Field) -> SolidModel {
    let base = hex_box([0.0; 3], [1.0, 0.4, 0.25], [4, 2, 1]).unwrap();
    let mesh = upgrade_to_hex20(&base).unwrap().0;
    let mut m = SolidModel::new(mesh);
    m.add_material("all", mat).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    add_field(&mut m, field);
    m
}

/// The transformation block `TRB` of D7 (ADR 0457 D7): Hex8 5 x 2 x 1 on `1 x 0.4 x 0.25`; `u_x = u_y = 0` on `xmin`; `u_z
/// = 0` at the root by one-term linear constraints (`linear`) or as a displacement constraint; springs of 5e7 N/m in z at
/// the `xmax` nodes (`springs`).
fn tr_block(mat: Material, field: Field, linear: bool, springs: bool) -> SolidModel {
    let mut mesh = hex_box([0.0; 3], [1.0, 0.4, 0.25], [5, 2, 1]).unwrap();
    let root: Vec<usize> = mesh.node_set("xmin").unwrap().to_vec();
    let tip: Vec<usize> = mesh.node_set("xmax").unwrap().to_vec();
    mesh.add_node_set("tip", tip).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", mat).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 1, 0.0)).unwrap();
    if linear {
        for &n in &root {
            m.add_constraint(Constraint::linear(vec![LinearTerm::new(n, 2, 1.0)], 0.0))
                .unwrap();
        }
    } else {
        m.add_constraint(Constraint::displacement("xmin", 2, 0.0)).unwrap();
    }
    if springs {
        m.add_support(ElasticSupport::nodal_spring("tip", 2, 5.0e7)).unwrap();
    }
    add_field(&mut m, field);
    m
}

fn uniform(m: &SolidModel, s: f64) -> Prestress {
    Prestress::uniform(m, [s, 0.0, 0.0, 0.0, 0.0, 0.0]).unwrap()
}

// ------------------------------------------------------------------------------------------- RB1 to RB4, NB1 to NB4

const E_C: [f64; 3] = [1.4e11, 1.4e10, 1.4e10];
const NU_C: [f64; 3] = [0.3, 0.3, 0.4];
const G_C: [f64; 3] = [7.0e9, 7.0e9, 5.0e9];
const RHO_C: f64 = 1600.0;
/// The speeds of D7, rad/s.
const SPEEDS: [f64; 3] = [0.0, 94.76, 379.0];
/// The reference frequencies of arithmetic 1 (lag, flap), Hz, at the speeds.
const F_REF: [[f64; 2]; 3] = [
    [30.1630170598, 74.6547066940],
    [36.1613542828, 78.7162776575],
    [83.3775278011, 124.1804945103],
];
/// The reference stiffening coefficients (lag, flap) at `Omega_a` and `Omega_b`.
const S_REF: [[f64; 2]; 2] = [[1.74909602, 2.73871527], [1.66058979, 2.70648723]];
/// The Euler-Bernoulli frequencies of arithmetic 1 (recorded, no limit).
const F_EB: [[f64; 2]; 3] = [
    [30.2213794060, 75.5534485386],
    [36.2238878986, 79.6030122256],
    [83.6275559939, 125.1984475444],
];
/// The first-order Southwell coefficients of D9 (lag `Q - 1`, flap `Q`).
const Q_CLOSED: [f64; 2] = [1.764215, 2.764215];

fn blade_material(axes: MaterialAxes) -> Material {
    Material::orthotropic(E_C, NU_C, G_C, RHO_C, [0.0; 3], axes).unwrap()
}

fn cyl_b() -> Material {
    blade_material(MaterialAxes::cylindrical([0.0; 3], AXIS_Z).unwrap())
}

/// The blade `BL` of D7, or `BL_T` turned by `phi` about z (the support as linear constraints in the turned directions).
fn blade(mat: Material, turned: Option<f64>) -> SolidModel {
    let base = hex_box([1.0, -0.01, -0.025], [1.0, 0.02, 0.05], [20, 2, 2]).unwrap();
    let mesh = upgrade_to_hex20(&base).unwrap().0;
    let root: Vec<usize> = mesh.node_set("xmin").unwrap().to_vec();
    let on = |k: usize, c: usize| mesh.nodes()[k][c].abs() <= 1e-12;
    let ys: Vec<usize> = root.iter().copied().filter(|&k| on(k, 1)).collect();
    let zs: Vec<usize> = root.iter().copied().filter(|&k| on(k, 2)).collect();
    assert_eq!((ys.len(), zs.len()), (5, 5));
    match turned {
        None => {
            let mut mesh = mesh;
            mesh.add_node_set("root_y", ys).unwrap();
            mesh.add_node_set("root_z", zs).unwrap();
            let mut m = SolidModel::new(mesh);
            m.add_material("all", mat).unwrap();
            m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
            m.add_constraint(Constraint::displacement("root_y", 1, 0.0)).unwrap();
            m.add_constraint(Constraint::displacement("root_z", 2, 0.0)).unwrap();
            m
        }
        Some(phi) => {
            let (s, c) = phi.sin_cos();
            let nodes: Vec<Vec3> = mesh
                .nodes()
                .iter()
                .map(|x| [c * x[0] - s * x[1], s * x[0] + c * x[1], x[2]])
                .collect();
            let mut tm = SolidMesh::new(nodes).unwrap();
            for b in mesh.blocks() {
                tm.add_block(b.kind(), b.connectivity().to_vec()).unwrap();
            }
            tm.add_element_set("all", (0..mesh.n_elements()).collect()).unwrap();
            tm.add_node_set("root_z", zs).unwrap();
            let mut m = SolidModel::new(tm);
            m.add_material("all", mat).unwrap();
            for &n in &root {
                m.add_constraint(Constraint::linear(
                    vec![LinearTerm::new(n, 0, c), LinearTerm::new(n, 1, s)],
                    0.0,
                ))
                .unwrap();
            }
            for &n in &ys {
                m.add_constraint(Constraint::linear(
                    vec![LinearTerm::new(n, 0, -s), LinearTerm::new(n, 1, c)],
                    0.0,
                ))
                .unwrap();
            }
            m.add_constraint(Constraint::displacement("root_z", 2, 0.0)).unwrap();
            m
        }
    }
}

/// `phi^T M_c phi / phi^T M phi`: the share of the kinetic energy in component `c` (the `share` of `si_rotating.rs`).
fn share(model: &SolidModel, m: &CsrMatrix, phi: &[f64], c: usize) -> f64 {
    let dofs = model.dof_map();
    let (mut part, mut all) = (0.0, 0.0);
    for r in 0..m.n {
        let rc = dofs.node_component(r).unwrap().1;
        for p in m.row_ptr[r]..m.row_ptr[r + 1] {
            let v = m.val[p] * phi[r] * phi[m.col[p]];
            all += v;
            if rc == c {
                part += v;
            }
        }
    }
    part / all
}

/// `(f_lag, f_flap)`, Hz: the lowest mode with a y share above 0.5 and the lowest with a z share above 0.5.
fn lag_flap(model: &SolidModel, m: &CsrMatrix, r: &ModalResult) -> [f64; 2] {
    let pick = |c: usize| {
        (0..r.modes.len())
            .find(|&i| share(model, m, &r.modes[i], c) > 0.5)
            .unwrap_or_else(|| panic!("no mode with a share above 0.5 in component {c}"))
    };
    [r.frequencies[pick(1)], r.frequencies[pick(2)]]
}

/// The lag and flap frequencies at the three speeds, and the four frequencies at each.
fn blade_run(model: &SolidModel, options: &RotatingOptions) -> (Vec<[f64; 2]>, Vec<Vec<f64>>) {
    let part = RotatingPart::new(model, AXIS_Z, [0.0; 3], options).unwrap();
    let mut lf = Vec::new();
    let mut all = Vec::new();
    for &w in &SPEEDS {
        let r = modes_of(part.modes(w).unwrap());
        lf.push(lag_flap(model, part.mass(), &r));
        all.push(r.frequencies.clone());
    }
    (lf, all)
}

fn s_of(f: f64, f0: f64, w: f64) -> f64 {
    let l = (2.0 * PI * f).powi(2);
    let l0 = (2.0 * PI * f0).powi(2);
    (l - l0) / (w * w)
}

#[test]
fn rb1_to_rb4_nb1_to_nb4_rotating_blade() {
    let mut c = Checks::new();
    let bl = blade(cyl_b(), None);
    let (lf, all) = blade_run(&bl, &rot_opts(4));
    let names = ["lag", "flap"];
    for (i, &w) in SPEEDS.iter().enumerate() {
        println!("Omega = {w}: frequencies {:?}", all[i]);
        for k in 0..2 {
            c.at_most(
                &format!("RB1 {} at {w} rad/s ({:.10} Hz)", names[k], lf[i][k]),
                (lf[i][k] / F_REF[i][k] - 1.0).abs(),
                5e-3,
            );
            println!(
                "  recorded: {} against Euler-Bernoulli {:+.4e}",
                names[k],
                lf[i][k] / F_EB[i][k] - 1.0
            );
        }
    }
    for i in 1..3 {
        let w = SPEEDS[i];
        for k in 0..2 {
            let s = s_of(lf[i][k], lf[0][k], w);
            c.at_most(
                &format!("RB2 {} S at {w} rad/s ({s:.8})", names[k]),
                (s / S_REF[i - 1][k] - 1.0).abs(),
                1e-2,
            );
            if i == 1 {
                c.at_most(
                    &format!("RB3 {} S at {w} rad/s against the closed form {}", names[k], Q_CLOSED[k]),
                    (s / Q_CLOSED[k] - 1.0).abs(),
                    2e-2,
                );
            }
        }
    }
    // RB4: the blade turned by 0.7 rad, on the transformation path.
    let bt = blade(cyl_b(), Some(0.7));
    let (_, all_t) = blade_run(&bt, &rot_opts(4));
    let worst = (0..3).map(|i| max_rel(&all_t[i], &all[i])).fold(0.0, f64::max);
    c.at_most("RB4 turned blade against the blade", worst, 1e-9);
    // NB1: the axes turned by 90 degrees about z.
    let n1 = blade(blade_material(fixed(rot(AXIS_Z, PI / 2.0))), None);
    let p1 = RotatingPart::new(&n1, AXIS_Z, [0.0; 3], &rot_opts(4)).unwrap();
    let r1 = modes_of(p1.modes(0.0).unwrap());
    c.below(
        "NB1 lag f/f_ref with E_2 along the span",
        lag_flap(&n1, p1.mass(), &r1)[0] / F_REF[0][0],
        0.5,
    );
    // NB2, NB3: spin softening off, stress stiffening off.
    let (l2, _) = blade_run(&bl, &rot_opts(4).spin_softening(false));
    let s2 = s_of(l2[2][0], l2[0][0], SPEEDS[2]);
    c.above(
        "NB2 lag |S/S_ref - 1| at Omega_b without spin softening",
        (s2 / S_REF[1][0] - 1.0).abs(),
        0.3,
    );
    let (l3, _) = blade_run(&bl, &rot_opts(4).stress_stiffening(false));
    let s3 = s_of(l3[2][1], l3[0][1], SPEEDS[2]);
    c.above(
        "NB3 flap |S/S_ref - 1| at Omega_b without stress stiffening",
        (s3 / S_REF[1][1] - 1.0).abs(),
        0.5,
    );
    // NB4: the turned blade with Fixed(I) axes.
    let n4 = blade(blade_material(fixed(I3)), Some(0.7));
    let p4 = RotatingPart::new(&n4, AXIS_Z, [0.0; 3], &rot_opts(4)).unwrap();
    let r4 = modes_of(p4.modes(0.0).unwrap());
    c.above(
        "NB4 turned blade with Fixed(I) axes against the blade, at rest",
        max_rel(&r4.frequencies, &all[0]),
        1e-2,
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------- RC2, RC2t, W1

/// The frequencies at `Omega_c = 0.5 sqrt(lambda_1)` of `reference` at rest (D7), of `model`, rotating about z through `O_B`.
fn speed_of(reference: &SolidModel) -> f64 {
    let p = RotatingPart::new(reference, AXIS_Z, O_B, &rot_opts(6)).unwrap();
    0.5 * modes_of(p.modes(0.0).unwrap()).eigenvalues[0].sqrt()
}

fn freqs_at(model: &SolidModel, w: f64) -> Vec<f64> {
    let p = RotatingPart::new(model, AXIS_Z, O_B, &rot_opts(6)).unwrap();
    modes_of(p.modes(w).unwrap()).frequencies
}

#[test]
fn rc2_rc2t_w1_isotropic_constants_rotating() {
    let mut c = Checks::new();
    let b_iso = block(iso_e(E_ISO), Field::None);
    let w = speed_of(&b_iso);
    println!("Omega_c (BLK) = {w:.10} rad/s");
    let f_iso = freqs_at(&b_iso, w);
    let t_iso = tr_block(iso_e(E_ISO), Field::None, true, true);
    let wt = speed_of(&t_iso);
    println!("Omega_c (TRB) = {wt:.10} rad/s");
    let ft_iso = freqs_at(&t_iso, wt);
    for (mat, tag) in [
        (iso_o(I3), "Fixed(I)"),
        (iso_o(rot([1.0, 2.0, 3.0], 0.5)), "Fixed(rot((1, 2, 3), 0.5))"),
        (iso_c(), "Cylindrical about the rotation axis"),
    ] {
        let b = block(mat.clone(), Field::None);
        c.at_most(&format!("RC2 {tag}"), max_rel(&freqs_at(&b, w), &f_iso), 1e-12);
        let t = tr_block(mat, Field::None, true, true);
        c.at_most(&format!("RC2t {tag}"), max_rel(&freqs_at(&t, wt), &ft_iso), 1e-12);
    }
    // W1: the dense whirl modes.
    let whirl = |m: &SolidModel| -> Vec<f64> {
        let p = RotatingPart::new(m, AXIS_Z, O_B, &rot_opts(3).coriolis(true)).unwrap();
        let r = p.whirl_modes(w).unwrap();
        r.modes().expect("whirl modes").modes[..3].iter().map(|x| x.omega).collect()
    };
    let wi = whirl(&b_iso);
    let wo = whirl(&block(iso_o(rot([1.0, 2.0, 3.0], 0.5)), Field::None));
    println!("W1: whirl omegas {wi:?}");
    c.at_most("W1 whirl, Fixed(rot) against ISO", max_rel(&wo, &wi), 1e-9);
    c.finish();
}

// ------------------------------------------------------------------------------------------- RQ1, RP1, RP1t, NR1

#[test]
fn rq1_rp1_nr1_rotating_per_point() {
    let mut c = Checks::new();
    // RQ1: the rotating stiffness against the pre-stressed modes (D8).
    let o = opts(6).shift(Shift::Value(0.0));
    for (mat, field, tag) in [(tab(), Field::Linear, "TAB"), (cyl(), Field::None, "CYL")] {
        for (m, path) in [
            (block(mat.clone(), field), "BLK"),
            (tr_block(mat.clone(), field, true, true), "TRB"),
        ] {
            let part = RotatingPart::new(
                &m,
                AXIS_Z,
                O_B,
                &RotatingOptions::new(6).modal(o.clone()).spin_softening(false),
            )
            .unwrap();
            let w = 0.5 * modes_of(part.modes(0.0).unwrap()).eigenvalues[0].sqrt();
            let r = modes_of(part.modes(w).unwrap());
            let p = prestressed_modes(&m, &part.unit_prestress().scaled(w * w), &o).unwrap();
            println!("RQ1 {tag} {path}: Omega = {w:.8} rad/s, frequencies {:?}", r.frequencies);
            c.at_most(
                &format!("RQ1 {tag} {path}"),
                max_rel(&r.eigenvalues, &p.eigenvalues),
                1e-10,
            );
        }
    }
    // RP1, RP1t: the tabulated material at a uniform 443 K against ISO at E(443).
    let e443 = interpolate(&TABLE_T, &TABLE_E, 443.0).unwrap();
    let bi = block(iso_e(e443), Field::None);
    let w = speed_of(&bi);
    let pi = RotatingPart::new(&bi, AXIS_Z, O_B, &rot_opts(6)).unwrap();
    let fi = modes_of(pi.modes(w).unwrap()).frequencies;
    let si: Vec<f64> = pi.unit_prestress().stress().iter().flatten().copied().collect();
    let mut unit_a = Vec::new();
    for (mat, tag) in [(tab(), "TAB_U"), (tab_a(), "TAB_A")] {
        let b = block(mat, Field::Uniform);
        let p = RotatingPart::new(&b, AXIS_Z, O_B, &rot_opts(6)).unwrap();
        let f = modes_of(p.modes(w).unwrap()).frequencies;
        c.at_most(&format!("RP1 {tag} frequencies against ISO at E(443)"), max_rel(&f, &fi), 1e-12);
        let s: Vec<f64> = p.unit_prestress().stress().iter().flatten().copied().collect();
        c.at_most(&format!("RP1 {tag} unit pre-stress against ISO at E(443)"), dev(&s, &si), 1e-12);
        if tag == "TAB_A" {
            unit_a = s;
        }
    }
    let ti = tr_block(iso_e(e443), Field::None, true, true);
    let wt = speed_of(&ti);
    let t = tr_block(tab(), Field::Uniform, true, true);
    c.at_most(
        "RP1t TAB_U transformation against ISO at E(443)",
        max_rel(&freqs_at(&t, wt), &freqs_at(&ti, wt)),
        1e-12,
    );
    // NR1: the stress with the thermal strain included.
    let mut nr = block(tab_a(), Field::Uniform);
    nr.add_load(Load::centrifugal(AXIS_Z, O_B, 1.0)).unwrap();
    let res = solve_static(&nr, &StaticOptions::default()).unwrap();
    let st: Vec<f64> = Prestress::from_static(&nr, &res)
        .unwrap()
        .stress()
        .iter()
        .flatten()
        .copied()
        .collect();
    c.above("NR1 thermal stress included against the unit pre-stress", dev(&st, &unit_a), 1.0);
    c.finish();
}

// ------------------------------------------------------------------------------------------- BK1, BK1b, NB5

const C_H: f64 = 0.1;
const LAMBDA_E: f64 = 17.99146636;
const LAMBDA_EN: f64 = 17.93614677;

/// The column `COL` of D7: the cantilever of ADR 0457 D7, `L = 4`, with the blade's constants and `Fixed(r)` axes.
fn column(r: Axes) -> SolidModel {
    let base = hex_box([0.0; 3], [4.0, C_H, C_H], [20, 1, 1]).unwrap();
    let mut mesh = upgrade_to_hex20(&base).unwrap().0;
    let root: Vec<usize> = mesh.node_set("xmin").unwrap().to_vec();
    let on = |mesh: &SolidMesh, k: usize, c: usize| (mesh.nodes()[k][c] - 0.5 * C_H).abs() <= 1e-12;
    let zs: Vec<usize> = root.iter().copied().filter(|&k| on(&mesh, k, 2)).collect();
    let ys: Vec<usize> = root.iter().copied().filter(|&k| on(&mesh, k, 1)).collect();
    assert_eq!((zs.len(), ys.len()), (2, 2));
    mesh.add_node_set("root_z", zs).unwrap();
    mesh.add_node_set("root_y", ys).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", blade_material(fixed(r))).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("root_z", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("root_y", 1, 0.0)).unwrap();
    m
}

#[test]
fn bk1_nb5_orthotropic_column() {
    let mut c = Checks::new();
    let col = column(I3);
    let r = factors(&col, &uniform(&col, -1.0e6), &BucklingOptions::new(2)).unwrap();
    println!("BK1: factors {:?}", r.factors);
    for k in 0..2 {
        c.at_most(
            &format!("BK1 factor {} against Euler with E_1", k + 1),
            (r.factors[k] / LAMBDA_E - 1.0).abs(),
            1e-2,
        );
        c.at_most(
            &format!("BK1b factor {} against Engesser", k + 1),
            (r.factors[k] / LAMBDA_EN - 1.0).abs(),
            3e-3,
        );
    }
    let n5 = column(rot(AXIS_Z, PI / 2.0));
    let r5 = factors(&n5, &uniform(&n5, -1.0e6), &BucklingOptions::new(1)).unwrap();
    c.below("NB5 axes turned by 90 degrees: factor 1 over lambda_E", r5.factors[0] / LAMBDA_E, 0.5);
    c.finish();
}

// ------------------------------------------------------------------------------------------- BK2, BK2t, BK3, BK4

const S_B: f64 = -2.0e8;

fn buckling_factors(m: &SolidModel) -> Vec<f64> {
    factors(m, &uniform(m, S_B), &BucklingOptions::new(3)).unwrap().factors
}

/// D8: `max_k |lambda'_k/lambda_k - 1|` for factors 1 and 2, `lambda' = -x^T K_sigma x / x^T K_sigma u`, `u` the static solve
/// of the same model under `K_sigma x`, `K_sigma` of an isotropic twin on the same mesh.
fn buckling_identity(m: &SolidModel) -> f64 {
    let p = uniform(m, S_B);
    let r = factors(m, &p, &BucklingOptions::new(3)).unwrap();
    let mut twin = SolidModel::new(m.mesh().clone());
    twin.add_material("all", iso_e(E_ISO)).unwrap();
    let kg = geometric_stiffness(&twin, &p, None).unwrap();
    let dofs = m.dof_map();
    let mut worst: f64 = 0.0;
    for k in 0..2 {
        let x = &r.modes[k];
        let kgx = matvec(&kg, x);
        let mut forces = Vec::new();
        for node in 0..m.mesh().nodes().len() {
            let mut f = [0.0; 3];
            for (comp, fc) in f.iter_mut().enumerate() {
                *fc = kgx[dofs.equation(node, comp).unwrap()];
            }
            forces.push(NodalForceAt::new(node, f));
        }
        let u = solve_static_with(m, &[], &forces, &StaticOptions::default()).unwrap();
        let lambda = -dot(x, &kgx) / dot(u.displacement_values(), &kgx);
        worst = worst.max((lambda / r.factors[k] - 1.0).abs());
    }
    worst
}

#[test]
fn bk2_bk3_bk4_buckling() {
    let mut c = Checks::new();
    let f_iso = buckling_factors(&block(iso_e(E_ISO), Field::None));
    let ft_iso = buckling_factors(&tr_block(iso_e(E_ISO), Field::None, true, true));
    println!("BK2: factors {f_iso:?}; transformation {ft_iso:?}");
    for (r, tag) in [(I3, "Fixed(I)"), (rot([1.0, 2.0, 3.0], 0.5), "Fixed(rot((1, 2, 3), 0.5))")] {
        c.at_most(
            &format!("BK2 {tag}"),
            max_rel(&buckling_factors(&block(iso_o(r), Field::None)), &f_iso),
            1e-12,
        );
        c.at_most(
            &format!("BK2t {tag}"),
            max_rel(&buckling_factors(&tr_block(iso_o(r), Field::None, true, true)), &ft_iso),
            1e-12,
        );
    }
    let e443 = interpolate(&TABLE_T, &TABLE_E, 443.0).unwrap();
    c.at_most(
        "BK3 TAB_U against ISO at E(443)",
        max_rel(
            &buckling_factors(&block(tab(), Field::Uniform)),
            &buckling_factors(&block(iso_e(e443), Field::None)),
        ),
        1e-12,
    );
    c.at_most(
        "BK3 TAB_U transformation against ISO at E(443)",
        max_rel(
            &buckling_factors(&tr_block(tab(), Field::Uniform, true, true)),
            &buckling_factors(&tr_block(iso_e(e443), Field::None, true, true)),
        ),
        1e-12,
    );
    for (mat, field, tag) in [(tab(), Field::Linear, "TAB"), (cyl(), Field::None, "CYL")] {
        c.at_most(
            &format!("BK4 {tag} BLK"),
            buckling_identity(&block(mat.clone(), field)),
            1e-9,
        );
        c.at_most(
            &format!("BK4 {tag} TRB"),
            buckling_identity(&tr_block(mat, field, true, true)),
            1e-9,
        );
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------- TR1, TR2, TR3

/// D8: `max_n |q_n - cos(n theta)|` over 40 steps of average acceleration from the first mode at rest.
fn transient_identity(m: &SolidModel) -> f64 {
    let r = modal_analysis(m, &opts(6)).unwrap();
    let phi = &r.modes[0];
    let w1 = r.eigenvalues[0].sqrt();
    let dt = 2.0 * PI / (20.0 * w1);
    let theta = 2.0 * (0.5 * w1 * dt).atan();
    let mphi = matvec(&consistent(m).unwrap(), phi);
    let den = dot(phi, &mphi);
    let options = TransientOptions::new(dt).method(Method::average_acceleration());
    let mut it = Integrator::new(m, &[], &options).unwrap();
    it.start(0.0, phi, &vec![0.0; phi.len()]).unwrap();
    let mut worst: f64 = 0.0;
    for n in 1..=40 {
        it.step().unwrap();
        let q = dot(&it.displacement().unwrap(), &mphi) / den;
        worst = worst.max((q - (n as f64 * theta).cos()).abs());
    }
    worst
}

/// The transient set-up of D7: the end displacement and velocity.
fn transient_run(m: &SolidModel, set: &str) -> (Vec<f64>, Vec<f64>) {
    let mut loaded = m.clone();
    loaded.add_load(Load::nodal_force(set, [0.0, 0.0, -1.0e4])).unwrap();
    let history = LoadHistory::model_loads(&loaded, LoadTable::constant(1.0, 0.0, 1.0).unwrap()).unwrap();
    let options = TransientOptions::new(2.0e-4)
        .method(Method::average_acceleration())
        .damping(Rayleigh::from_ratio(0.02, 500.0, 5000.0).unwrap());
    let mut it = Integrator::new(m, &[history], &options).unwrap();
    let n = m.dof_map().n_equations();
    it.start(0.0, &vec![0.0; n], &vec![0.0; n]).unwrap();
    it.run(25).unwrap();
    (it.displacement().unwrap(), it.velocity().unwrap())
}

#[test]
fn tr1_tr2_tr3_transient() {
    let mut c = Checks::new();
    for (mat, field, tag) in [(tab(), Field::Linear, "TAB"), (cyl(), Field::None, "CYL")] {
        c.at_most(
            &format!("TR1 {tag} BLK"),
            transient_identity(&block(mat.clone(), field)),
            1e-8,
        );
        c.at_most(
            &format!("TR1 {tag} TRB"),
            transient_identity(&tr_block(mat, field, true, true)),
            1e-8,
        );
    }
    let (u_iso, v_iso) = transient_run(&block(iso_e(E_ISO), Field::None), "xmax");
    let (ut_iso, vt_iso) = transient_run(&tr_block(iso_e(E_ISO), Field::None, true, true), "tip");
    for (r, tag) in [(I3, "Fixed(I)"), (rot([1.0, 2.0, 3.0], 0.5), "Fixed(rot((1, 2, 3), 0.5))")] {
        let (u, v) = transient_run(&block(iso_o(r), Field::None), "xmax");
        c.at_most(&format!("TR2 {tag} BLK displacement"), dev(&u, &u_iso), 1e-12);
        c.at_most(&format!("TR2 {tag} BLK velocity"), dev(&v, &v_iso), 1e-12);
        let (u, v) = transient_run(&tr_block(iso_o(r), Field::None, true, true), "tip");
        c.at_most(&format!("TR2 {tag} TRB displacement"), dev(&u, &ut_iso), 1e-12);
        c.at_most(&format!("TR2 {tag} TRB velocity"), dev(&v, &vt_iso), 1e-12);
    }
    let e443 = interpolate(&TABLE_T, &TABLE_E, 443.0).unwrap();
    let (u, v) = transient_run(&block(tab(), Field::Uniform), "xmax");
    let (ui, vi) = transient_run(&block(iso_e(e443), Field::None), "xmax");
    c.at_most("TR3 TAB_U BLK displacement", dev(&u, &ui), 1e-12);
    c.at_most("TR3 TAB_U BLK velocity", dev(&v, &vi), 1e-12);
    let (u, v) = transient_run(&tr_block(tab(), Field::Uniform, true, true), "tip");
    let (ui, vi) = transient_run(&tr_block(iso_e(e443), Field::None, true, true), "tip");
    c.at_most("TR3 TAB_U TRB displacement", dev(&u, &ui), 1e-12);
    c.at_most("TR3 TAB_U TRB velocity", dev(&v, &vi), 1e-12);
    c.finish();
}

// ------------------------------------------------------------------------------------------- R1 (copy), R3 (copy)

/// The cantilever of `sa_ortho.rs` (ADR 0452 R1), copied.
fn cantilever_with(mat: Material, temperature: bool) -> SolidModel {
    let mesh = hex_box([0.0; 3], [1.0, 0.1, 0.1], [20, 2, 2]).unwrap();
    let t: Vec<f64> = mesh.nodes().iter().map(|x| 293.0 + 100.0 * x[0]).collect();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", mat).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m.add_load(Load::nodal_force("xmax", [0.0, 0.0, -100.0])).unwrap();
    if temperature {
        m.add_load(Load::temperature(t, 293.0)).unwrap();
    }
    m
}

fn refusal(r: &rcfd_fem::Result<()>, variant: &str) -> bool {
    matches!(r, Err(FemError::NotYetSupported { item: "material", variant: v, location: l, package: p })
        if *v == variant && l == "material 0, element set 'all'" && *p == FOLLOW_UP)
}

fn cyclic_refusal(r: &rcfd_fem::Result<()>, variant: &str, j: usize) -> bool {
    let want = format!("material 0, element set 'all', with the cyclic constraint {j}");
    matches!(r, Err(FemError::NotYetSupported { item: "material", variant: v, location: l, package: p })
        if *v == variant && *l == want && *p == FOLLOW_UP)
}

fn shell_plate(mat: Material) -> (SolidModel, Vec<Prescribed>) {
    let n = 4;
    let idx = |i: usize, j: usize| j * (n + 1) + i;
    let mut nodes = Vec::new();
    for j in 0..=n {
        for i in 0..=n {
            nodes.push([i as f64 / n as f64, j as f64 / n as f64, 0.0]);
        }
    }
    let mut conn = Vec::new();
    for j in 0..n {
        for i in 0..n {
            conn.extend([idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1)]);
        }
    }
    let mut pres = Vec::new();
    for (k, x) in nodes.iter().enumerate() {
        if x[0] == 0.0 {
            for comp in 0..6 {
                pres.push(Prescribed::new(k, comp, 0.0));
            }
        }
    }
    let nn = nodes.len();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Shell4, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::shell(0.01).unwrap()).unwrap();
    mesh.add_element_set("all", (0..n * n).collect()).unwrap();
    mesh.add_node_set("all", (0..nn).collect()).unwrap();
    let mut s = SolidModel::new(mesh);
    s.add_material("all", mat).unwrap();
    (s, pres)
}

/// The copy of `sa_ortho_modal::r1_r2_paths_after_f537` after ADR 0459 (D6): E6 and E7 solve as well.
#[test]
fn r1_r2_paths_after_f542() {
    let mut c = Checks::new();
    let steel = Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 0.0).unwrap();
    let twin = cantilever_with(steel.clone(), false);
    let prestress = Prestress::uniform(&twin, [-1e6, 0.0, 0.0, 0.0, 0.0, 0.0]).unwrap();
    let table = Material::tabulated(steel, vec![200.0, 600.0], vec![("e".to_string(), vec![2.0e11, 1.8e11])]).unwrap();
    for (mat, name) in [
        (ortho(fixed(rot([0.0, 0.0, 1.0], 0.5))), "Orthotropic"),
        (table, "Tabulated"),
    ] {
        let m = cantilever_with(mat, true);
        let cg = StaticOptions::with_solver(StaticSolver::PreconditionedCg {
            preconditioner: CgPreconditioner::BlockJacobi,
            tolerance: 1e-10,
            max_iterations: 20000,
        });
        let solved: Vec<(&str, rcfd_fem::Result<()>)> = vec![
            (
                "E1 solve_static",
                solve_static(&m, &StaticOptions::default()).map(|_| ()),
            ),
            (
                "E2 solve_static_with CG",
                solve_static_with(&m, &[], &[], &cg).map(|_| ()),
            ),
            (
                "E3 modal_analysis_with",
                modal_analysis_with(&m, &[], &ModalOptions::new(2)).map(|_| ()),
            ),
            (
                "E6 Integrator::new",
                Integrator::new(&m, &[], &TransientOptions::new(1e-4)).map(|_| ()),
            ),
            (
                "E7 buckling::factors",
                factors(&m, &prestress, &BucklingOptions::new(1)).map(|_| ()),
            ),
            (
                "E8 prestressed_modes",
                prestressed_modes(&m, &prestress, &ModalOptions::new(2)).map(|_| ()),
            ),
            (
                "E9 solve_static_mpc",
                solve_static_mpc(&m, &[], &[], &[], &StaticOptions::default()).map(|_| ()),
            ),
            (
                "E10 modal_analysis_mpc",
                modal_analysis_mpc(&m, &[], &ModalOptions::new(2)).map(|_| ()),
            ),
        ];
        for (entry, r) in solved {
            c.holds(
                &format!("R1 {name} {entry} solves"),
                r.is_ok(),
                &format!("{:?}", r.err()),
            );
        }
        let refused: Vec<(&str, rcfd_fem::Result<()>)> = vec![
            ("E4 stiffness", stiffness(&m).map(|_| ())),
            (
                "E5 nonlinear::solve",
                solve(&m, &[], &[], &NewtonOptions::new(1)).map(|_| ()),
            ),
        ];
        for (entry, r) in refused {
            c.holds(
                &format!("R1 {name} {entry} refused"),
                refusal(&r, name),
                &format!("{:?}", r.err()),
            );
        }
    }
    // R2: a Shell4 plate with the orthotropic material.
    let (mut s, pres) = shell_plate(ortho(fixed(I3)));
    s.add_load(Load::shell_pressure("all", 1000.0)).unwrap();
    let r2 = solve_static_with(&s, &pres, &[], &StaticOptions::default());
    let ok = matches!(&r2, Err(FemError::NotYetSupported { item: "material", variant: "Orthotropic", location, package })
        if location.starts_with("element 0") && location.contains("Shell4") && *package == FOLLOW_UP);
    c.holds(
        "R2 Shell4 with Orthotropic refused",
        ok,
        &format!("{:?}", r2.as_ref().err()),
    );
    c.finish();
}

/// The disc sector D of ADR 0395 (`si_rotating_cyclic.rs`) with `ORTHO` in cylindrical axes about the cyclic axis (D7).
fn sector_cyl() -> SolidModel {
    let n = 12;
    let alpha = 2.0 * PI / n as f64;
    let b = hex_box([0.25, 0.0, -0.1], [0.75, alpha, 0.2], [3, 2, 1]).unwrap();
    let mesh = upgrade_to_hex20_mapped(&b, |p| [p[0] * p[1].cos(), p[0] * p[1].sin(), p[2]])
        .unwrap()
        .0;
    let mut m = SolidModel::new(mesh);
    m.add_material("all", ortho(MaterialAxes::cylindrical([0.0; 3], AXIS_Z).unwrap()))
        .unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m.add_constraint(Constraint::cyclic("ymin", "ymax", AXIS_Z, [0.0; 3], n))
        .unwrap();
    m
}

/// The copy of `sa_ortho_modal::r3_paths_not_opened` after ADR 0459 (D6), with the rows of its R3.
#[test]
fn r3_paths_not_opened_after_f542() {
    let mut c = Checks::new();
    let steel = Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 0.0).unwrap();
    let twin = cantilever_with(steel.clone(), false);
    let prestress = Prestress::uniform(&twin, [-1e6, 0.0, 0.0, 0.0, 0.0, 0.0]).unwrap();
    let table = Material::tabulated(steel, vec![200.0, 600.0], vec![("e".to_string(), vec![2.0e11, 1.8e11])]).unwrap();
    for (mat, name) in [(ortho(fixed(I3)), "Orthotropic"), (table, "Tabulated")] {
        let m = cantilever_with(mat, true);
        let refused: Vec<(&str, rcfd_fem::Result<()>)> = vec![
            ("stiffness", stiffness(&m).map(|_| ())),
            (
                "geometric_stiffness",
                geometric_stiffness(&m, &prestress, None).map(|_| ()),
            ),
            (
                "nonlinear::solve",
                solve(&m, &[], &[], &NewtonOptions::new(1)).map(|_| ()),
            ),
            ("damped_modes", damped_modes(&m, &DampedOptions::new(2)).map(|_| ())),
            (
                "DirectHarmonic::new",
                DirectHarmonic::new(&m, &DirectHarmonicOptions::new()).map(|_| ()),
            ),
            (
                "cyclic::modal_analysis_harmonic",
                modal_analysis_harmonic(&m, &[], 0, &ModalOptions::new(2)).map(|_| ()),
            ),
        ];
        for (entry, r) in refused {
            c.holds(
                &format!("R3 {name} {entry} refused"),
                refusal(&r, name),
                &format!("{:?}", r.err()),
            );
        }
        let solved: Vec<(&str, rcfd_fem::Result<()>)> = vec![
            (
                "buckling::factors",
                factors(&m, &prestress, &BucklingOptions::new(1)).map(|_| ()),
            ),
            (
                "Integrator::new",
                Integrator::new(&m, &[], &TransientOptions::new(1e-4)).map(|_| ()),
            ),
            (
                "RotatingPart::new",
                RotatingPart::new(&m, [1.0, 0.0, 0.0], [0.0; 3], &RotatingOptions::new(2)).map(|_| ()),
            ),
        ];
        for (entry, r) in solved {
            c.holds(
                &format!("R3 {name} {entry} solves"),
                r.is_ok(),
                &format!("{:?}", r.err()),
            );
        }
    }
    let (s, pres) = shell_plate(ortho(fixed(I3)));
    let rs = modal_analysis_with(&s, &pres, &ModalOptions::new(2));
    let ok = matches!(&rs, Err(FemError::NotYetSupported { item: "material", variant: "Orthotropic", location, package })
        if location.starts_with("element 0") && location.contains("Shell4") && *package == FOLLOW_UP);
    c.holds(
        "R3 Shell4 modal with Orthotropic refused",
        ok,
        &format!("{:?}", rs.as_ref().err()),
    );
    // A cyclic constraint with the orthotropic material.
    let mut cy = tr_block(ortho(fixed(I3)), Field::None, false, false);
    cy.add_constraint(Constraint::cyclic("ymin", "ymax", [0.0, 0.0, 1.0], [0.0; 3], 4))
        .unwrap();
    let j = cy.constraints().len() - 1;
    let zero = uniform(&cy, 0.0);
    let rows: Vec<(&str, rcfd_fem::Result<()>)> = vec![
        (
            "modal_analysis_with",
            modal_analysis_with(&cy, &[], &ModalOptions::new(2)).map(|_| ()),
        ),
        (
            "buckling::factors",
            factors(&cy, &uniform(&cy, -1e6), &BucklingOptions::new(1)).map(|_| ()),
        ),
        (
            "Integrator::new",
            Integrator::new(&cy, &[], &TransientOptions::new(1e-4)).map(|_| ()),
        ),
        (
            "RotatingPart::new",
            RotatingPart::new(&cy, AXIS_Z, [0.0; 3], &RotatingOptions::new(2)).map(|_| ()),
        ),
        (
            "RotatingPart::with_unit_prestress",
            RotatingPart::with_unit_prestress(&cy, AXIS_Z, [0.0; 3], zero, &RotatingOptions::new(2)).map(|_| ()),
        ),
    ];
    for (entry, r) in rows {
        c.holds(
            &format!("R3 cyclic constraint with Orthotropic: {entry} refused"),
            cyclic_refusal(&r, "Orthotropic", j),
            &format!("{:?}", r.err()),
        );
    }
    // The sector with cylindrical axes about the cyclic axis (F-537's question 3, refused by D3).
    let sec = sector_cyl();
    let js = sec.constraints().len() - 1;
    let rows: Vec<(&str, rcfd_fem::Result<()>)> = vec![
        (
            "RotatingSector::new",
            RotatingSector::new(&sec, &RotatingOptions::new(2)).map(|_| ()),
        ),
        (
            "RotatingSector::with_unit_prestress",
            RotatingSector::with_unit_prestress(&sec, uniform(&sec, 0.0), &RotatingOptions::new(2)).map(|_| ()),
        ),
    ];
    for (entry, r) in rows {
        c.holds(
            &format!("R3 sector with cylindrical axes about its axis: {entry} refused"),
            cyclic_refusal(&r, "Orthotropic", js),
            &format!("{:?}", r.err()),
        );
    }
    let rh = modal_analysis_harmonic(&sec, &[], 1, &ModalOptions::new(2)).map(|_| ());
    c.holds(
        "R3 sector: cyclic::modal_analysis_harmonic refused",
        refusal(&rh, "Orthotropic"),
        &format!("{:?}", rh.err()),
    );
    // A tabulated material without a temperature field.
    let nt = block(tab(), Field::None);
    let iso_twin = block(iso_e(E_ISO), Field::None);
    let rows: Vec<(&str, rcfd_fem::Result<()>)> = vec![
        ("modal_analysis", modal_analysis(&nt, &ModalOptions::new(2)).map(|_| ())),
        (
            "RotatingPart::new",
            RotatingPart::new(&nt, AXIS_Z, O_B, &RotatingOptions::new(2)).map(|_| ()),
        ),
        (
            "buckling::factors",
            factors(&nt, &uniform(&iso_twin, S_B), &BucklingOptions::new(1)).map(|_| ()),
        ),
        (
            "Integrator::new",
            Integrator::new(&nt, &[], &TransientOptions::new(1e-4)).map(|_| ()),
        ),
    ];
    for (entry, r) in rows {
        c.holds(
            &format!("R3 Tabulated without a temperature field: {entry} refused (temperature)"),
            matches!(&r, Err(FemError::InvalidValue { key: "temperature", .. })),
            &format!("{:?}", r.as_ref().err()),
        );
    }
    c.finish();
}
