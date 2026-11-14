//! F-545 (ADR 0463): the transient integrator with isotropic constants in an orthotropic material against the isotropic
//! material, on a Hex20 and a Hex8 model on the elimination and on the transformation path, within limits taken from a
//! rounding probe (ADR 0463, section "The probe"). Checks EQ-H20E, EQ-H20T, EQ-H8E, EQ-H8T, NC1, NC2.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::hex_box;
use rcfd_fem::hex20::upgrade_to_hex20;
use rcfd_fem::material::MaterialAxes;
use rcfd_fem::model::{ElasticSupport, LinearTerm};
use rcfd_fem::transient::{Integrator, LoadHistory, LoadTable, Method, Rayleigh, TransientOptions};
use rcfd_fem::{Constraint, Load, Material, SolidModel};

type Axes = [[f64; 3]; 3];

const I3: Axes = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
const E_ISO: f64 = 2.0e11;
const NU_ISO: f64 = 0.3;
const RHO_ISO: f64 = 7850.0;
const AXIS_Z: Vec3 = [0.0, 0.0, 1.0];
const O_B: Vec3 = [-0.5, 0.2, 0.0];

/// Rodrigues rotation matrix, as in `sa_ortho_rotating.rs`.
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

/// `max |a - b| / max |b|`.
fn dev(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len());
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max) / b.iter().map(|y| y.abs()).fold(0.0, f64::max)
}

fn iso(e: f64) -> Material {
    Material::isotropic_elastic(e, NU_ISO, RHO_ISO, 0.0).unwrap()
}

/// Isotropic constants (with `E` as given) in an orthotropic material with the given axes.
fn iso_o_axes(e: f64, axes: MaterialAxes) -> Material {
    let g = e / (2.0 * (1.0 + NU_ISO));
    Material::orthotropic([e; 3], [NU_ISO; 3], [g; 3], RHO_ISO, [0.0; 3], axes).unwrap()
}

/// The set-up of ADR 0463: Hex20 or Hex8, elimination (`linear = false`: 4 x 2 x 1, `xmin` held in all components) or
/// transformation (`linear = true`: 5 x 2 x 1, `u_x = u_y = 0` on `xmin`, one-term linear constraints `u_z = 0` at the root
/// nodes, springs of 5e7 N/m in z on the tip nodes).
fn setup(mat: Material, hex20: bool, linear: bool) -> SolidModel {
    let n = if linear { [5, 2, 1] } else { [4, 2, 1] };
    let base = hex_box([0.0; 3], [1.0, 0.4, 0.25], n).unwrap();
    let mut mesh = if hex20 {
        upgrade_to_hex20(&base).unwrap().0
    } else {
        base
    };
    let root: Vec<usize> = mesh.node_set("xmin").unwrap().to_vec();
    let tip: Vec<usize> = mesh.node_set("xmax").unwrap().to_vec();
    mesh.add_node_set("tip", tip).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", mat).unwrap();
    if linear {
        m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
        m.add_constraint(Constraint::displacement("xmin", 1, 0.0)).unwrap();
        for &k in &root {
            m.add_constraint(Constraint::linear(vec![LinearTerm::new(k, 2, 1.0)], 0.0))
                .unwrap();
        }
        m.add_support(ElasticSupport::nodal_spring("tip", 2, 5.0e7)).unwrap();
    } else {
        for c in 0..3 {
            m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
        }
    }
    m
}

/// The transient set-up of ADR 0459 D7: the end displacement and velocity.
fn run(m: &SolidModel) -> (Vec<f64>, Vec<f64>) {
    let mut loaded = m.clone();
    loaded.add_load(Load::nodal_force("tip", [0.0, 0.0, -1.0e4])).unwrap();
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

/// (tag, Hex20, transformation, registered limit L).
const SETUPS: [(&str, bool, bool, f64); 4] = [
    ("H20E", true, false, 5e-11),
    ("H20T", true, true, 2e-12),
    ("H8E", false, false, 6e-13),
    ("H8T", false, true, 2e-12),
];

#[test]
fn eq_nc1_nc2_transient_isotropic_equivalence() {
    let mut failed: Vec<String> = Vec::new();
    for (tag, hex20, linear, limit) in SETUPS {
        let (u0, v0) = run(&setup(iso(E_ISO), hex20, linear));
        // NC2: the reference run is deterministic.
        let (u1, v1) = run(&setup(iso(E_ISO), hex20, linear));
        let (d2u, d2v) = (dev(&u1, &u0), dev(&v1, &v0));
        println!("NC2 {tag}: u {d2u:e} v {d2v:e} (limit == 0)");
        if d2u != 0.0 || d2v != 0.0 {
            failed.push(format!("NC2 {tag}"));
        }
        // EQ: three orthotropic materials with isotropic constants.
        let materials = [
            ("Fixed(I)", iso_o_axes(E_ISO, MaterialAxes::fixed(I3).unwrap())),
            (
                "Fixed(rot((1, 2, 3), 0.5))",
                iso_o_axes(E_ISO, MaterialAxes::fixed(rot([1.0, 2.0, 3.0], 0.5)).unwrap()),
            ),
            (
                "Cylindrical",
                iso_o_axes(E_ISO, MaterialAxes::cylindrical(O_B, AXIS_Z).unwrap()),
            ),
        ];
        for (name, mat) in materials {
            let (u, v) = run(&setup(mat, hex20, linear));
            let (du, dv) = (dev(&u, &u0), dev(&v, &v0));
            let ok = du <= limit && dv <= limit;
            println!(
                "EQ-{tag} {name}: u {du:.4e} v {dv:.4e} (limit <= {limit:e}) {}",
                if ok { "met" } else { "MISSED" }
            );
            if !ok {
                failed.push(format!("EQ-{tag} {name}: u {du:e} v {dv:e} > {limit:e}"));
            }
        }
        // NC1: a relative change of 1e-6 of E in the orthotropic material is seen at 100 L.
        let (u, _) = run(&setup(
            iso_o_axes(E_ISO * (1.0 + 1e-6), MaterialAxes::fixed(I3).unwrap()),
            hex20,
            linear,
        ));
        let d = dev(&u, &u0);
        let ok = d > 100.0 * limit;
        println!(
            "NC1 {tag}: u {d:.4e} (expected > {:e}) {}",
            100.0 * limit,
            if ok { "as expected" } else { "NOT AS EXPECTED" }
        );
        if !ok {
            failed.push(format!("NC1 {tag}: {d:e} <= {:e}", 100.0 * limit));
        }
    }
    assert!(failed.is_empty(), "missed: {failed:?}");
}
