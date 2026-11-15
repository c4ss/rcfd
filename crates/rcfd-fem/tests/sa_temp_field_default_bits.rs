//! F-613 (ADR 0534): check N1 (record B1, D8). The paths F-613 touches give the bits they gave before F-613: the quasi-static
//! driver with the temperature history of ADR 0526 (the shifted cube) and of an elastic cube with a non-zero expansion and no
//! temperature, the static Newton driver of an elastic bar with a non-zero expansion, the assembled internal force and tangent
//! of a J2 bar at a temperature, the nonlinear transient integrator of the shifted rod with Rayleigh damping and the history of
//! ADR 0526, of an elastic bar with a non-zero expansion and damping, of a BBar Hex8 bar, and the refusals of a temperature
//! load. Uses only API that existed at 679081a1; the expected digests were printed by this file run on the unchanged base
//! 679081a1 (`f613-scratch/b1-base.log`) and written into `EXPECTED` before any library code of F-613.

use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::loads::NodalForceAt;
use rcfd_fem::material::{PointContext, PronyTerm, TimeTemperatureShift};
use rcfd_fem::mesh::SolidFormulation;
use rcfd_fem::nl_transient::{Integrator as NlIntegrator, NlTransientOptions};
use rcfd_fem::nonlinear::{self, solve_in_time_with, NewtonOptions, TimeHistory, TimeLoading, TimeSteppingOptions};
use rcfd_fem::transient::{LoadHistory, LoadTable, MotionHistory, Rayleigh};
use rcfd_fem::{BlockProperty, Constraint, Load, Material, SolidModel};

/// Digests of 679081a1 in the order of the run, from the base run (`f613-scratch/b1-base.log`).
const EXPECTED: Option<[(&str, u64); 12]> = Some([
    ("solve_in_time_with shifted cube with a history", 0x528a_ae24_b5ac_2070),
    ("solve_in_time_with expanding elastic cube", 0xd2dd_5e91_416b_a63e),
    ("nonlinear::solve expanding elastic bar", 0xbabe_9248_97e1_4518),
    ("internal force and tangent of a J2 bar at 383 K", 0x1eb9_b7b2_5e0c_f228),
    ("nl_transient damped shifted rod with a history", 0xc1d6_f486_5b48_adc9),
    (
        "nl_transient damped shifted rod with a history bytes",
        0xae8d_2f86_c098_f91a,
    ),
    ("nl_transient damped expanding elastic bar", 0x1c23_19e9_756c_89cb),
    ("nl_transient damped expanding elastic bar bytes", 0xedee_c051_0046_bebe),
    ("nl_transient BBar bar", 0xbba9_84f2_336a_45fd),
    ("nl_transient BBar bar bytes", 0x2e15_d8a1_63e5_4018),
    ("solve_in_time_with temperature load refusal", 0xb4dd_a70f_5c5f_3ac3),
    ("nl_transient temperature load refusal", 0x8165_3ac5_07c5_978a),
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

fn base(nu: f64) -> Material {
    Material::isotropic_elastic(2.0e9, nu, 1200.0, 0.0).unwrap()
}

/// Steel with a non-zero expansion (it must not act without a temperature).
fn hot_steel() -> Material {
    Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 1.2e-5).unwrap()
}

fn wlf() -> TimeTemperatureShift {
    TimeTemperatureShift::wlf(373.0, 17.44, 51.6).unwrap()
}

fn visco(nu: f64, shift: Option<TimeTemperatureShift>) -> Material {
    let terms = [(0.3, 0.01), (0.2, 0.1), (0.1, 1.0)]
        .iter()
        .map(|&(w, t)| PronyTerm::new(w, w, t).unwrap())
        .collect();
    Material::viscoelastic(base(nu), terms, shift).unwrap()
}

fn symmetric(mesh: rcfd_fem::SolidMesh, material: Material) -> SolidModel {
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
    m
}

fn cube(material: Material) -> SolidModel {
    symmetric(hex_box([0.0; 3], [1.0; 3], [1, 1, 1]).unwrap(), material)
}

fn bar(material: Material) -> SolidModel {
    symmetric(hex_box([0.0; 3], [1.0, 0.2, 0.2], [3, 1, 1]).unwrap(), material)
}

/// The rod of ADR 0485 V1: 20 Hex8 along x, the lateral motion held.
fn rod(material: Material) -> SolidModel {
    let mesh = hex_box([0.0; 3], [1.0, 0.05, 0.05], [20, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material).unwrap();
    m.add_constraint(Constraint::displacement("all", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m
}

fn xmax_x_equations(model: &SolidModel) -> Vec<usize> {
    let dofs = model.dof_map();
    model
        .mesh()
        .node_set("xmax")
        .unwrap()
        .iter()
        .map(|&n| dofs.equation(n, 0).unwrap())
        .collect()
}

fn flat_history(h: &TimeHistory) -> Vec<f64> {
    let mut v = Vec::new();
    for s in &h.steps {
        v.push(s.time);
        v.extend_from_slice(&s.u);
        v.extend_from_slice(&s.internal_force);
        for e in &s.states {
            v.push(e.values().len() as f64);
            v.extend_from_slice(e.values());
        }
        v.push(s.halvings as f64);
        v.push(s.substeps as f64);
        v.push(s.max_creep_increment);
        v.push(s.iterations as f64);
    }
    v
}

fn nl_run(model: &SolidModel, options: &NlTransientOptions, v0: &[f64], steps: u64) -> (Vec<f64>, Vec<u8>) {
    let mut it = NlIntegrator::new(model, &[], options).unwrap();
    let n = v0.len();
    it.start(0.0, &vec![0.0; n], v0).unwrap();
    let mut v = Vec::new();
    for _ in 0..steps {
        it.step().unwrap();
        let s = it.state().unwrap();
        v.extend_from_slice(&s.u);
        v.extend_from_slice(&s.v);
        v.extend_from_slice(&s.a);
        v.extend_from_slice(&s.internal_force);
    }
    (v, it.state().unwrap().to_bytes())
}

fn rod_velocity(model: &SolidModel) -> Vec<f64> {
    let dofs = model.dof_map();
    let n = dofs.n_equations();
    let mut v0: Vec<f64> = (0..n).map(|i| 1e-3 * ((i as f64) * 0.11).sin()).collect();
    for (q, x) in v0.iter_mut().enumerate() {
        let (_, c) = dofs.node_component(q).unwrap();
        if c != 0 {
            *x = 0.0;
        }
    }
    for &nd in model.mesh().node_set("xmin").unwrap() {
        v0[dofs.equation(nd, 0).unwrap()] = 0.0;
    }
    v0
}

fn tight() -> NewtonOptions {
    let mut o = NewtonOptions::new(1);
    o.residual_tolerance = 1e-13;
    o.energy_tolerance = 1.0;
    o
}

#[test]
fn n1_default_bits_of_the_base() {
    let mut got: Vec<(&str, u64)> = Vec::new();

    // The driver with the temperature history of ADR 0526: the shifted cube under a strain step, a table step inside.
    let model = cube(visco(0.35, Some(wlf())));
    let list = model
        .mesh()
        .node_set("xmax")
        .unwrap()
        .iter()
        .map(|&n| Prescribed::new(n, 0, 1e-3))
        .collect();
    let loading = TimeLoading::new().motion(MotionHistory::new(list, LoadTable::constant(1.0, 0.0, 1.0).unwrap()));
    let times: Vec<f64> = (1..=20).map(|k| k as f64 * 3e-5).collect();
    let table = LoadTable::new(vec![(0.0, 383.0), (times[9], 383.0), (times[10], 393.0), (1.0, 393.0)]).unwrap();
    let h = solve_in_time_with(
        &model,
        &[],
        &loading,
        &times,
        &TimeSteppingOptions::new(f64::INFINITY)
            .newton(tight())
            .temperature_history(table.clone()),
    )
    .unwrap();
    got.push((
        "solve_in_time_with shifted cube with a history",
        digest(&flat_history(&h)),
    ));

    // An elastic cube with a non-zero expansion and no temperature under a load history.
    let ec = cube(hot_steel());
    let n = ec.dof_map().n_equations();
    let mut pattern = vec![0.0; n];
    for q in xmax_x_equations(&ec) {
        pattern[q] = 2.0e8;
    }
    let lh = TimeLoading::new().load(LoadHistory::new(
        pattern,
        LoadTable::new(vec![(0.0, 0.0), (3.0, 1.0)]).unwrap(),
    ));
    let h = solve_in_time_with(
        &ec,
        &[],
        &lh,
        &[1.0, 2.0, 3.0],
        &TimeSteppingOptions::new(f64::INFINITY).newton(tight()),
    )
    .unwrap();
    got.push(("solve_in_time_with expanding elastic cube", digest(&flat_history(&h))));

    // The static Newton driver of an elastic bar with a non-zero expansion.
    let eb = bar(hot_steel());
    let forces: Vec<NodalForceAt> = eb
        .mesh()
        .node_set("xmax")
        .unwrap()
        .iter()
        .map(|&nd| NodalForceAt::new(nd, [1.0e7, 2.0e6, 0.0]))
        .collect();
    let r = nonlinear::solve(&eb, &[], &forces, &NewtonOptions::new(3)).unwrap();
    got.push((
        "nonlinear::solve expanding elastic bar",
        digest(r.result.displacement_values()),
    ));

    // The assembled internal force and tangent of a J2 bar at a displacement and a temperature.
    let jb = bar(Material::j2(hot_steel(), 2.0e8, 2.0e9, 1.0e9).unwrap());
    let n = jb.dof_map().n_equations();
    let mut u = vec![0.0; n];
    for (q, x) in u.iter_mut().enumerate() {
        *x = 1e-4 * ((q as f64) * 0.37).sin();
    }
    let fixed: Vec<usize> = {
        let dofs = jb.dof_map();
        let mut f = Vec::new();
        for (set, comp) in [("xmin", 0), ("ymin", 1), ("zmin", 2)] {
            for &nd in jb.mesh().node_set(set).unwrap() {
                f.push(dofs.equation(nd, comp).unwrap());
            }
        }
        f
    };
    for q in fixed {
        u[q] = 0.0;
    }
    let states = nonlinear::initial_states(&jb).unwrap();
    let at = PointContext::new().with_temperature(383.0).unwrap();
    let f = nonlinear::internal_force_at(&jb, &u, &states, &at).unwrap();
    let k = nonlinear::tangent_at(&jb, &u, &states, Some(2), &at).unwrap();
    let mut v = f.clone();
    v.extend_from_slice(&k.val);
    got.push(("internal force and tangent of a J2 bar at 383 K", digest(&v)));

    // nl_transient: the shifted rod with Rayleigh mass and stiffness damping and a temperature history (re-formed).
    let damping = Rayleigh {
        mass: 20.0,
        stiffness: 2.0e-6,
    };
    let sr = rod(visco(0.0, Some(wlf())));
    let v0 = rod_velocity(&sr);
    let step = LoadTable::new(vec![
        (0.0, 383.0),
        (30.0 * 1e-5, 383.0),
        (31.0 * 1e-5, 393.0),
        (1.0, 393.0),
    ])
    .unwrap();
    let (v, bytes) = nl_run(
        &sr,
        &NlTransientOptions::new(1e-5).damping(damping).temperature_history(step),
        &v0,
        60,
    );
    got.push(("nl_transient damped shifted rod with a history", digest(&v)));
    got.push((
        "nl_transient damped shifted rod with a history bytes",
        digest_bytes(bytes),
    ));

    // An elastic bar with a non-zero expansion and damping, no temperature.
    let n = eb.dof_map().n_equations();
    let mut vb = vec![0.0; n];
    for q in xmax_x_equations(&eb) {
        vb[q] = 40.0;
    }
    let jd = Rayleigh {
        mass: 20.0,
        stiffness: 2.0e-8,
    };
    let (v, bytes) = nl_run(&eb, &NlTransientOptions::new(2e-6).damping(jd), &vb, 30);
    got.push(("nl_transient damped expanding elastic bar", digest(&v)));
    got.push(("nl_transient damped expanding elastic bar bytes", digest_bytes(bytes)));

    // A BBar Hex8 bar.
    let mut mesh = hex_box([0.0; 3], [1.0, 0.2, 0.2], [3, 1, 1]).unwrap();
    mesh.set_block_property(0, BlockProperty::solid(SolidFormulation::BBar))
        .unwrap();
    let bb = symmetric(mesh, hot_steel());
    let (v, bytes) = nl_run(&bb, &NlTransientOptions::new(2e-6), &vb, 20);
    got.push(("nl_transient BBar bar", digest(&v)));
    got.push(("nl_transient BBar bar bytes", digest_bytes(bytes)));

    // The refusals of a temperature load.
    let mut tl = cube(hot_steel());
    let nn = tl.mesh().nodes().len();
    tl.add_load(Load::temperature(vec![400.0; nn], 300.0)).unwrap();
    let msg = format!(
        "{:?}",
        solve_in_time_with(
            &tl,
            &[],
            &TimeLoading::new(),
            &[1.0],
            &TimeSteppingOptions::new(f64::INFINITY)
        )
        .err()
    );
    println!("B1 refusal driver: {msg}");
    got.push(("solve_in_time_with temperature load refusal", digest_bytes(msg.bytes())));
    let msg = format!(
        "{:?}",
        NlIntegrator::new(&tl, &[], &NlTransientOptions::new(1e-5)).err()
    );
    println!("B1 refusal integrator: {msg}");
    got.push(("nl_transient temperature load refusal", digest_bytes(msg.bytes())));

    for (name, d) in &got {
        println!("B1 {name}: {d:#018x}");
    }
    let expected = EXPECTED.expect("EXPECTED is written from the base run");
    assert_eq!(got.len(), expected.len());
    for ((name, d), (en, e)) in got.iter().zip(expected.iter()) {
        assert_eq!(name, en);
        assert_eq!(d, e, "B1 {name}: {d:#018x} against {e:#018x}");
    }
}
