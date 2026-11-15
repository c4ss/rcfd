//! F-619 (ADR 0565): check B0 (A4). The direct harmonic solve gives the bits it gave before F-619: the responses of the
//! transformation path of ADR 0543 (the sprung bar with a loss factor, the merged cantilever with the options of RS2, the
//! same with a pre-stress, the bar on a foundation), the refusal message of a base motion through `response_with_base` on
//! the transformation path (ADR 0543 RT1), the plain path's base-motion response, and the transformation `T`, `g` of the
//! merged cantilever. Uses only API that existed at cf32422b; the expected digests were printed by this file run on the
//! unchanged base cf32422b (`f619-scratch/b0-base.log`) and written into `EXPECTED` before any library code of F-619.

use rcfd_fem::builders::hex_box;
use rcfd_fem::geometric::Prestress;
use rcfd_fem::harmonic::{HarmonicLoad, HarmonicResponse};
use rcfd_fem::harmonic_direct::{BaseMotion, Dashpot, DirectHarmonic, DirectHarmonicOptions, LossFactor};
use rcfd_fem::mpc::transformation;
use rcfd_fem::{Constraint, ElasticSupport, Material, SolidMesh, SolidModel};
use rcfd_linalg::complex::C64;
use std::f64::consts::PI;

/// Digests of cf32422b in the order of the run, from the base run (`f619-scratch/b0-base.log`).
const EXPECTED: Option<[(&str, u64); 7]> = Some([
    ("sprung bar", 0x7669_91cc_9ba0_bdf3),
    ("merged cantilever", 0xb446_cd79_516f_b7a9),
    ("merged cantilever pre-stress", 0x1de4_284e_5848_5063),
    ("foundation bar", 0xbf2d_c339_4883_cc20),
    ("refusal base motion", 0x8eb4_0fe2_0059_51eb),
    ("plain base motion", 0x2378_99bf_1faf_af24),
    ("transformation", 0x8843_8525_e040_17d7),
]);

/// FNV-1a 64 over the little-endian bytes of the bits of each value.
fn digest(v: &[f64]) -> u64 {
    digest_bytes(v.iter().flat_map(|x| x.to_bits().to_le_bytes()))
}

fn digest_bytes<I: IntoIterator<Item = u8>>(bytes: I) -> u64 {
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    for b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

const E_MOD: f64 = 2e11;
const RHO: f64 = 7850.0;
const B: f64 = 0.04;
const A: f64 = B * B;
const CUT: f64 = 0.3;
/// `w_1` of the merged cantilever of ADR 0543 RS2 (its measured value, addendum 1), as a fixed frequency scale.
const WC: f64 = 1700.8;

fn steel(nu: f64) -> Material {
    Material::isotropic_elastic(E_MOD, nu, RHO, 0.0).unwrap()
}

fn w1() -> f64 {
    PI * (E_MOD / RHO).sqrt() / 2.0
}

fn bar_on(mesh: SolidMesh) -> SolidModel {
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel(0.0)).unwrap();
    m.add_constraint(Constraint::displacement("all", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m
}

fn bar_mesh() -> SolidMesh {
    hex_box([0.0; 3], [1.0, B, B], [80, 1, 1]).unwrap()
}

fn bar_tip_load(model: &SolidModel) -> HarmonicLoad {
    let dofs = model.dof_map();
    let mut f = vec![0.0; dofs.n_equations()];
    for &a in model.mesh().node_set("xmax").unwrap() {
        f[dofs.equation(a, 0).unwrap()] = 0.25;
    }
    HarmonicLoad::real(f).unwrap()
}

fn merged() -> SolidModel {
    let mut mesh = hex_box([0.0; 3], [0.6, 0.1, 0.1], [6, 2, 2]).unwrap();
    let at: Vec<usize> = (0..mesh.nodes().len())
        .filter(|&a| (mesh.nodes()[a][0] - CUT).abs() < 1e-12)
        .collect();
    mesh.add_node_set("junction", at).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel(0.3)).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m.add_constraint(Constraint::rigid_set("junction", [CUT, 0.05, 0.05]))
        .unwrap();
    m.add_support(ElasticSupport::nodal_spring("xmax", 2, 1e6)).unwrap();
    m
}

fn junction_node(model: &SolidModel) -> usize {
    model
        .mesh()
        .nodes()
        .iter()
        .position(|p| (p[0] - CUT).abs() < 1e-12 && p[1].abs() < 1e-12 && p[2].abs() < 1e-12)
        .unwrap()
}

fn c_load(model: &SolidModel) -> HarmonicLoad {
    let dofs = model.dof_map();
    let n = dofs.n_equations();
    let (mut re, mut im) = (vec![0.0; n], vec![0.0; n]);
    let tip = model.mesh().node_set("xmax").unwrap();
    let share = 1.0 / tip.len() as f64;
    for &a in tip {
        for (c, (fr, fi)) in [(0.3, 0.0), (1.0, 0.2), (-0.5, 0.0)].iter().enumerate() {
            let eq = dofs.equation(a, c).unwrap();
            re[eq] += share * fr;
            im[eq] += share * fi;
        }
    }
    let j = junction_node(model);
    re[dofs.equation(j, 2).unwrap()] += 0.4;
    im[dofs.equation(j, 0).unwrap()] += 0.1;
    HarmonicLoad::new(re, im).unwrap()
}

fn flat(rs: &[HarmonicResponse]) -> Vec<f64> {
    let mut v = Vec::new();
    for r in rs {
        v.push(r.omega);
        v.extend_from_slice(&r.re);
        v.extend_from_slice(&r.im);
    }
    v
}

fn message<T>(r: rcfd_fem::Result<T>) -> String {
    match r {
        Ok(_) => "accepted".to_string(),
        Err(e) => format!("{e}"),
    }
}

#[test]
fn b0_default_bits_of_the_base() {
    let mut got: Vec<(&str, u64)> = Vec::new();
    let lossy = DirectHarmonicOptions::new().loss_factor("all", LossFactor::Constant(0.02));

    let mut sprung = bar_on(bar_mesh());
    sprung
        .add_support(ElasticSupport::nodal_spring("xmax", 0, E_MOD * A / 4.0))
        .unwrap();
    let s = DirectHarmonic::new(&sprung, &lossy).unwrap();
    let r = s.response(&bar_tip_load(&sprung), w1()).unwrap();
    got.push(("sprung bar", digest(&flat(&[r]))));

    let mm = merged();
    let o = lossy
        .clone()
        .rayleigh(0.02 * WC, 0.02 / WC)
        .dashpot(Dashpot::ground(junction_node(&mm), 1, 1e3));
    let s = DirectHarmonic::new(&mm, &o).unwrap();
    let r = s.sweep(&c_load(&mm), &[0.5 * WC, 1.7 * WC]).unwrap();
    got.push(("merged cantilever", digest(&flat(&r))));

    let p = Prestress::uniform(&mm, [2e8, 0.0, 0.0, 0.0, 0.0, 0.0]).unwrap();
    let s = DirectHarmonic::new(&mm, &DirectHarmonicOptions::new().prestress(p)).unwrap();
    let r = s.response(&c_load(&mm), 1.7 * WC).unwrap();
    got.push(("merged cantilever pre-stress", digest(&flat(&[r]))));

    let mut founded = bar_on(bar_mesh());
    founded.add_support(ElasticSupport::foundation("xmax", E_MOD)).unwrap();
    let s = DirectHarmonic::new(&founded, &lossy).unwrap();
    let r = s.response(&bar_tip_load(&founded), w1()).unwrap();
    got.push(("foundation bar", digest(&flat(&[r]))));

    let s = DirectHarmonic::new(&mm, &DirectHarmonicOptions::new()).unwrap();
    let mut base = BaseMotion::new();
    base.add(mm.mesh().node_set("xmin").unwrap()[0], 0, C64::ONE);
    let msg = message(s.response_with_base(&c_load(&mm), &base, 1e3));
    println!("B0 refusal base motion: {msg}");
    got.push(("refusal base motion", digest_bytes(msg.into_bytes())));

    let plain = bar_on(bar_mesh());
    let s = DirectHarmonic::new(&plain, &lossy).unwrap();
    let mut base = BaseMotion::new();
    base.add_set(&plain, "xmin", 0, C64::new(1e-3, 2e-4)).unwrap();
    let zero = HarmonicLoad::real(vec![0.0; plain.dof_map().n_equations()]).unwrap();
    let r = s.response_with_base(&zero, &base, 0.7 * w1()).unwrap();
    got.push(("plain base motion", digest(&flat(&[r]))));

    let tr = transformation(&mm, &[]).unwrap();
    let mut v: Vec<f64> = tr.t().val.clone();
    v.extend(tr.t().col.iter().map(|&c| c as f64));
    v.extend(tr.t().row_ptr.iter().map(|&c| c as f64));
    v.extend_from_slice(tr.g());
    v.extend(tr.independent().iter().map(|&c| c as f64));
    got.push(("transformation", digest(&v)));

    for (name, d) in &got {
        println!("B0 {name}: {d:#018x}");
    }
    let expected = EXPECTED.expect("EXPECTED is written from the base run");
    assert_eq!(got.len(), expected.len());
    for ((name, d), (en, e)) in got.iter().zip(expected.iter()) {
        assert_eq!(name, en);
        assert_eq!(d, e, "B0 {name}: {d:#018x} against {e:#018x}");
    }
}
