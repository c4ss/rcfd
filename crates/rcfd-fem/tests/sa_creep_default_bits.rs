//! F-411 (ADR 0489): check N1 (record B0, D9). The paths of ADR 0485 D8 (the nonlinear static path, the one-attempt path of
//! F-423, the nonlinear transient integrator, the J2 history path of F-422, the assembled force and tangent, the isotropic and
//! J2 constitutive calls, the linear static and modal solves and the refusal of J2) and three paths of F-410 and F-422
//! (`solve_from_at` of a three-branch viscoelastic Hex8 at a strain step and five increments, `nl_transient` of a
//! viscoelastic Hex8 bar started by a velocity, `element_force_and_tangent` of a distorted Hex20 with J2 from a non-zero
//! state) give the bits they gave before F-411 for models without the creep material. Uses only API that existed at
//! 81960c8d; the expected digests were printed by this file run on the unchanged base 81960c8d
//! (`f411-scratch/b0-base.log`) and written into `EXPECTED` before any library code of F-411.

use rcfd_fem::builders::{hex_box, tet_box, upgrade_to_tet10};
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::element::{element_for, ElementState};
use rcfd_fem::hex20::upgrade_to_hex20;
use rcfd_fem::material::{PointContext, PronyTerm};
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::nl_transient::{Integrator as NlIntegrator, NlTransientOptions};
use rcfd_fem::nonlinear::{
    element_force_and_tangent, initial_states, internal_force, solve, solve_from, solve_from_at, tangent, Contribution,
    NewtonOptions, NonlinearResult,
};
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::transient::{LoadHistory, LoadTable};
use rcfd_fem::{Constraint, ElementKind, Load, Material, SolidModel};

/// Digests of 81960c8d in the order of the run, from the base run (`f411-scratch/b0-base.log`).
const EXPECTED: Option<[(&str, u64); 15]> = None;

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

fn steel() -> Material {
    Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 0.0).unwrap()
}

fn j2() -> Material {
    Material::j2(steel(), 2.0e8, 2.0e9, 1.0e9).unwrap()
}

/// Every value of a nonlinear result: displacement, reactions, per increment the load factor, step, halvings, residuals,
/// energies, then the state values.
fn flat(r: &NonlinearResult) -> Vec<f64> {
    let mut v = r.result.displacement_values().to_vec();
    v.extend_from_slice(r.result.reactions().unwrap());
    for s in &r.steps {
        v.push(s.load_factor);
        v.push(s.step as f64);
        v.push(s.halvings as f64);
        v.extend_from_slice(&s.residuals);
        v.extend_from_slice(&s.energies);
    }
    for s in &r.states {
        v.push(s.values().len() as f64);
        v.extend_from_slice(s.values());
    }
    v
}

/// A bar `[1, 0.2, 0.2]` of `[3, 1, 1]` Hex8 with three symmetry planes, stretched by `stretch` at `xmax` if non-zero.
fn bar(material: Material, stretch: f64) -> SolidModel {
    let mesh = hex_box([0.0; 3], [1.0, 0.2, 0.2], [3, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
    if stretch != 0.0 {
        m.add_constraint(Constraint::displacement("xmax", 0, stretch)).unwrap();
    }
    m
}

/// A linear spring `k u` on the `x` equations of the `xmax` nodes.
struct Spring {
    equations: Vec<usize>,
    k: f64,
}

impl Contribution for Spring {
    fn name(&self) -> &str {
        "spring"
    }
    fn add(
        &self,
        u: &[f64],
        _l: f64,
        force: &mut [f64],
        tangent: &mut Vec<(usize, usize, f64)>,
    ) -> rcfd_fem::Result<()> {
        for &q in &self.equations {
            force[q] += self.k * u[q];
            tangent.push((q, q, self.k));
        }
        Ok(())
    }
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

/// The values of an nl_transient run: `u`, `v`, `a`, the internal force after every step, then the bytes of the state.
fn nl_run(model: &SolidModel, loads: &[LoadHistory], v0: &[f64], dt: f64, steps: u64) -> (Vec<f64>, Vec<u8>) {
    let mut it = NlIntegrator::new(model, loads, &NlTransientOptions::new(dt)).unwrap();
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

fn cantilever() -> SolidModel {
    let mesh = upgrade_to_hex20(&hex_box([0.0; 3], [1.0, 0.1, 0.1], [4, 1, 1]).unwrap())
        .unwrap()
        .0;
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m.add_load(Load::traction("xmax", [0.0, -1.0e6, 0.0])).unwrap();
    m
}

fn visco() -> Material {
    let base = Material::isotropic_elastic(2.0e9, 0.35, 1200.0, 0.0).unwrap();
    let terms = [(0.3, 0.01), (0.2, 0.1), (0.1, 1.0)]
        .iter()
        .map(|&(w, t)| PronyTerm::new(w, w, t).unwrap())
        .collect();
    Material::viscoelastic(base, terms, None).unwrap()
}

/// The unit cube of one Hex8 with three symmetry planes.
fn cube(material: Material) -> SolidModel {
    let mesh = hex_box([0.0; 3], [1.0; 3], [1, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
    m
}

#[test]
fn n1_default_bits_of_the_base() {
    let mut got: Vec<(&str, u64)> = Vec::new();

    let r = solve(&bar(steel(), 2e-3), &[], &[], &NewtonOptions::new(2)).unwrap();
    got.push(("bar Hex8 solve", digest(&flat(&r))));
    let r = solve(&bar(j2(), 5e-3), &[], &[], &NewtonOptions::new(4)).unwrap();
    got.push(("J2 bar solve", digest(&flat(&r))));

    // solve_from with a spring contribution, from a non-zero start.
    let model = bar(steel(), 0.0);
    let n = model.dof_map().n_equations();
    let eqs = xmax_x_equations(&model);
    let mut load = vec![0.0; n];
    for &q in &eqs {
        load[q] = 1.0e6;
    }
    let spring = Spring {
        equations: eqs.clone(),
        k: 1.0e9,
    };
    let mut start = vec![0.0; n];
    start[eqs[0]] = 1e-5;
    let states = initial_states(&model).unwrap();
    let eq = solve_from(&model, &[], &load, &start, &states, &NewtonOptions::new(1), &[&spring]).unwrap();
    let mut v = eq.u.clone();
    v.extend_from_slice(&eq.internal_force);
    v.extend_from_slice(&eq.residuals);
    v.extend_from_slice(&eq.energies);
    got.push(("solve_from spring", digest(&v)));

    // nl_transient: an elastic bar under a step load, a J2 bar started by a velocity.
    let mut pattern = vec![0.0; n];
    for &q in &eqs {
        pattern[q] = 2.0e6;
    }
    let table = LoadTable::new(vec![(0.0, 1.0), (1.0, 1.0)]).unwrap();
    let (v, bytes) = nl_run(&model, &[LoadHistory::new(pattern, table)], &vec![0.0; n], 2e-5, 30);
    got.push(("nl_transient elastic bar", digest(&v)));
    got.push(("nl_transient elastic bar bytes", digest_bytes(bytes)));
    let jm = bar(j2(), 0.0);
    let mut v0 = vec![0.0; n];
    for &q in &eqs {
        v0[q] = 40.0;
    }
    let (v, bytes) = nl_run(&jm, &[], &v0, 2e-6, 30);
    got.push(("nl_transient J2 bar", digest(&v)));
    got.push(("nl_transient J2 bar bytes", digest_bytes(bytes)));

    // Assembled force and tangent of a distorted Tet10 model.
    let mesh = upgrade_to_tet10(&tet_box([0.1, -0.2, 0.3], [1.2, 1.0, 0.8], [2, 2, 2]).unwrap())
        .unwrap()
        .0;
    let mut pm = SolidModel::new(mesh);
    pm.add_material("all", steel()).unwrap();
    let np = pm.dof_map().n_equations();
    let u: Vec<f64> = (0..np).map(|i| 1e-4 * ((i as f64) * 0.37).sin()).collect();
    let st = initial_states(&pm).unwrap();
    let mut v = internal_force(&pm, &u, &st).unwrap();
    v.extend_from_slice(&tangent(&pm, &u, &st, Some(2)).unwrap().val);
    got.push(("Tet10 internal_force and tangent", digest(&v)));

    // The constitutive calls.
    let eps = [1e-3, -2e-4, 3e-4, 5e-4, -1e-4, 2e-4];
    let (s, d) = steel().stress(&eps, &PointContext::default(), &[]).unwrap();
    let mut v = s.to_vec();
    v.extend(d.iter().flatten());
    let mut state = [0.0; 13];
    for (k, x) in state.iter_mut().enumerate() {
        *x = 1e-4 * (k as f64 + 1.0).cos();
    }
    let (s, d) = j2().stress(&eps, &PointContext::default(), &state).unwrap();
    v.extend_from_slice(&s);
    v.extend(d.iter().flatten());
    got.push(("Material::stress isotropic and J2", digest(&v)));

    // The linear static and modal solves of a Hex20 cantilever.
    let c = cantilever();
    let r = solve_static(&c, &StaticOptions::default()).unwrap();
    got.push(("solve_static cantilever", digest(r.displacement_values())));
    let modal = modal_analysis(&c, &ModalOptions::new(3)).unwrap();
    got.push(("modal cantilever", digest(&modal.eigenvalues)));

    // The refusal of J2 by the linear static solve.
    let msg = format!("{:?}", solve_static(&bar(j2(), 2e-3), &StaticOptions::default()).err());
    println!("B0 refusal: {msg}");
    got.push(("J2 refusal solve_static", digest_bytes(msg.bytes())));

    // F-410: solve_from_at of the viscoelastic cube at a strain step and five increments of 0.02 s.
    let vm = cube(visco());
    let nv = vm.dof_map().n_equations();
    let dofs = vm.dof_map();
    let xmax: Vec<usize> = vm.mesh().node_set("xmax").unwrap().to_vec();
    let mut u = vec![0.0; nv];
    let pres: Vec<Prescribed> = xmax
        .iter()
        .map(|&nd| {
            u[dofs.equation(nd, 0).unwrap()] = 1e-3;
            Prescribed::new(nd, 0, 1e-3)
        })
        .collect();
    let mut states = initial_states(&vm).unwrap();
    // ADR 0485 arithmetic 6: at constant strain the residual criterion alone.
    let mut tight = NewtonOptions::new(1);
    tight.residual_tolerance = 1e-13;
    tight.energy_tolerance = 1.0;
    let mut v = Vec::new();
    for k in 0..=5 {
        let (t, h) = if k == 0 { (0.0, 0.0) } else { (0.02 * k as f64, 0.02) };
        let ctx = PointContext::new().with_time(t, h).unwrap();
        let eq = solve_from_at(&vm, &pres, &vec![0.0; nv], &u, &states, &tight, &[], &ctx).unwrap();
        u = eq.u.clone();
        states = eq.states.clone();
        v.extend_from_slice(&eq.u);
        v.extend_from_slice(&eq.internal_force);
        v.extend_from_slice(&eq.residuals);
        v.extend_from_slice(&eq.energies);
        for s in &states {
            v.extend_from_slice(s.values());
        }
    }
    got.push(("solve_from_at viscoelastic cube", digest(&v)));

    // F-410: nl_transient of a viscoelastic bar started by a velocity.
    let vb = bar(visco(), 0.0);
    let mut v0 = vec![0.0; n];
    for &q in &eqs {
        v0[q] = 1.0;
    }
    let (v, bytes) = nl_run(&vb, &[], &v0, 2e-5, 30);
    got.push(("nl_transient viscoelastic bar", digest(&v)));
    got.push(("nl_transient viscoelastic bar bytes", digest_bytes(bytes)));

    // F-422: the element force and tangent of a distorted Hex20 with J2 from a non-zero state.
    let el = element_for(ElementKind::Hex20);
    let reference = rcfd_fem::hex20::reference_nodes();
    let coords: Vec<[f64; 3]> = reference
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let f = i as f64;
            [
                0.5 * p[0] + 0.02 * (1.3 * f).sin(),
                0.4 * p[1] + 0.02 * (0.7 * f).cos(),
                0.3 * p[2] + 0.01 * (2.1 * f).sin(),
            ]
        })
        .collect();
    let ue: Vec<f64> = (0..60).map(|i| 2e-4 * ((i as f64) * 0.53).sin()).collect();
    let npts = el.stiffness_rule().len();
    let st: Vec<f64> = (0..npts * 13).map(|k| 1e-4 * ((k % 13) as f64 + 1.0).cos()).collect();
    let (f, k) = element_force_and_tangent(el, &coords, &j2(), &ue, &ElementState::new(st)).unwrap();
    let mut v = f;
    v.extend_from_slice(k.values());
    got.push(("Hex20 J2 element force and tangent", digest(&v)));

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
