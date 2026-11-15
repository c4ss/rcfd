//! F-639 (ADR 0570): the curvature-free fallback of the total Lagrangian contact body. FD (the driver), FC (the contact
//! set), P-f (ADR 0542 P with the option), NF-HS (the default keeps ADR 0551's refusal); the heavy records HS20f, HS10f (ADR
//! 0551 HS20t and HS10t with the option, with FB) and HS20m-M, HS10m-M (ADR 0551 HS20m and HS10m read by the moments of the
//! nodal forces). Every limit is the one registered in ADR 0570.

mod f601_common;
mod f623_common;
mod f624_common;

use f601_common::*;
use f623_common::*;
use rcfd_fem::builders::{hex_box, upgrade_to_tet10};
use rcfd_fem::contact::{
    solve, ContactOptions, ContactPair, ContactResult, ContactSet, Kinematics, QuadraticContactor, Target,
};
use rcfd_fem::convert::split_hexahedra;
use rcfd_fem::error::Result as FemResult;
use rcfd_fem::hex20::upgrade_to_hex20;
use rcfd_fem::nonlinear::{initial_states, solve_from, Contribution, NewtonOptions};
use rcfd_fem::solve::DirectFactor;
use rcfd_fem::{Constraint, FemError, SolidMesh, SolidModel};
use std::cell::Cell;
use std::collections::BTreeMap;

// ------------------------------------------------------------------------------------------------------------ FD

/// ADR 0570 D4 FD: the force `k u_q` at `q`; the exact tangent reported as `k - kick`; the fallback `k` when `fallback`.
struct Spring {
    q: usize,
    k: f64,
    kick: f64,
    fallback: bool,
    calls: Cell<usize>,
}

impl Contribution for Spring {
    fn name(&self) -> &str {
        "spring"
    }
    fn add(&self, u: &[f64], _lf: f64, force: &mut [f64], tangent: &mut Vec<(usize, usize, f64)>) -> FemResult<()> {
        force[self.q] += self.k * u[self.q];
        tangent.push((self.q, self.q, self.k - self.kick));
        Ok(())
    }
    fn has_fallback_tangent(&self) -> bool {
        self.fallback
    }
    fn add_fallback_tangent(&self, _u: &[f64], _lf: f64, tangent: &mut Vec<(usize, usize, f64)>) -> FemResult<()> {
        self.calls.set(self.calls.get() + 1);
        tangent.push((self.q, self.q, self.k));
        Ok(())
    }
}

fn cube() -> (SolidModel, usize) {
    let mesh = hex_box([0.0; 3], [1.0; 3], [1, 1, 1]).unwrap();
    let node = mesh.nodes().iter().position(|x| *x == [1.0, 1.0, 1.0]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", elastic(1.0, 0.3)).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("zmin", c, 0.0)).unwrap();
    }
    let q = m.dof_map().equation(node, 2).unwrap();
    (m, q)
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

#[test]
fn fd_driver_fallback_against_the_honest_tangent() {
    let (m, q) = cube();
    let n = m.dof_map().n_equations();
    let mut load = vec![0.0; n];
    load[q] = 0.01;
    let start = vec![0.0; n];
    let states = initial_states(&m).unwrap();
    let o = NewtonOptions::new(1);
    let spring = |kick: f64, fallback: bool| Spring {
        q,
        k: 1.0,
        kick,
        fallback,
        calls: Cell::new(0),
    };
    let mut c = Checks::new();
    let honest = spring(0.0, false);
    let h = solve_from(&m, &[], &load, &start, &states, &o, &[&honest]).unwrap();
    let spoiled = spring(100.0, true);
    let s = solve_from(&m, &[], &load, &start, &states, &o, &[&spoiled]);
    match &s {
        Ok(s) => {
            c.holds("FD spoiled run succeeds", true, "");
            c.holds(
                "FD displacement bit for bit",
                bits(&s.u) == bits(&h.u),
                &format!("u_q {:e} against {:e}", s.u[q], h.u[q]),
            );
            c.holds(
                "FD residuals and energies bit for bit",
                bits(&s.residuals) == bits(&h.residuals) && bits(&s.energies) == bits(&h.energies),
                &format!("{:?} against {:?}", s.residuals, h.residuals),
            );
            let calls = spoiled.calls.get();
            c.holds(
                "FD a fallback at every correction",
                calls == s.energies.len() && calls >= 1,
                &format!("{calls} calls, {} corrections", s.energies.len()),
            );
        }
        Err(e) => c.holds("FD spoiled run succeeds", false, &format!("{e}")),
    }
    let none = spring(100.0, false);
    let e = solve_from(&m, &[], &load, &start, &states, &o, &[&none]);
    println!("FD without fallback: {:?}", e.as_ref().err());
    c.holds(
        "FD without fallback refused with the singular tangent",
        matches!(&e, Err(FemError::Solver { solver, reason }) if *solver == "newton"
            && reason.starts_with("singular tangent: pivot")),
        "",
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ FC

const SPHERE: Target = Target::RigidSphere {
    centre: [0.5, 0.45, 0.0],
    radius: 0.97,
};

fn summed(t: &[(usize, usize, f64)]) -> BTreeMap<(usize, usize), f64> {
    let mut m = BTreeMap::new();
    for &(r, s, v) in t {
        *m.entry((r, s)).or_insert(0.0) += v;
    }
    m
}

#[test]
fn fc_contact_set_fallback_tangent() {
    let mut c = Checks::new();
    for (tag, m, target) in [
        ("M20s", hex20_contactor_model(), SPHERE),
        ("M20q", hex20_on_hex20_model(), Target::Surface("target".into())),
    ] {
        let pair = ContactPair::new("contactor", target, 10.0);
        let base = ContactOptions::new(1, 0.0).quadratic_contactor(QuadraticContactor::Mortar);
        let off = ContactSet::new(&m, std::slice::from_ref(&pair), &base).unwrap();
        let set = ContactSet::new(&m, &[pair], &base.clone().curvature_fallback(true)).unwrap();
        let k = set.nodes(0).len();
        let lam: Vec<f64> = (0..k).map(|i| 0.1 + 0.01 * i as f64).collect();
        set.set_multipliers(0, &lam).unwrap();
        let n = m.dof_map().n_equations();
        let u = t1_state(n);
        let mut tf = Vec::new();
        set.add_fallback_tangent(&u, 1.0, &mut tf).unwrap();
        let calls = set.fallbacks();
        let tc = set.convex_tangent(&u).unwrap();
        let mut f = vec![0.0; n];
        let mut te = Vec::new();
        set.add(&u, 1.0, &mut f, &mut te).unwrap();
        let (sf, sc) = (summed(&tf), summed(&tc));
        let keys: std::collections::BTreeSet<_> = sf.keys().chain(sc.keys()).copied().collect();
        let differ = keys
            .iter()
            .filter(|key| {
                sf.get(key).copied().unwrap_or(0.0).to_bits() != sc.get(key).copied().unwrap_or(0.0).to_bits()
            })
            .count();
        c.holds(
            &format!("FC {tag}: fallback against convex_tangent bit for bit"),
            differ == 0 && !tf.is_empty(),
            &format!("{differ} entries differ of {}; {} triplets", keys.len(), tf.len()),
        );
        let pf: Vec<(usize, usize)> = tf.iter().map(|t| (t.0, t.1)).collect();
        let pe: Vec<(usize, usize)> = te.iter().map(|t| (t.0, t.1)).collect();
        c.holds(
            &format!("FC {tag}: the index sequence of add's triplets"),
            pf == pe,
            &format!("{} against {}", pf.len(), pe.len()),
        );
        c.holds(
            &format!("FC {tag}: the counter after one call"),
            calls == 1,
            &format!("{calls}"),
        );
        c.holds(
            &format!("FC {tag}: the flag with and without the option"),
            set.has_fallback_tangent() && !off.has_fallback_tangent(),
            "",
        );
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ P-f

#[test]
fn p_f_adr0542_p_with_the_fallback() {
    use f624_common::{fit_model, options, Route, Setup, BASE};
    let s = Setup { n: 4, ..BASE };
    let fit = fit_model(s);
    let (pair, o) = options(&s, Route::Contact);
    let t0 = std::time::Instant::now();
    let r = solve(&fit.model, &[], &[], &[pair], &o.curvature_fallback(true));
    let mut c = Checks::new();
    match &r {
        Ok(r) => {
            let rec: Vec<(usize, usize)> = r
                .steps
                .iter()
                .flat_map(|s| s.solves.iter().map(|x| (x.residuals.len() - 1, x.convex_iterations)))
                .collect();
            println!(
                "P-f: succeeds in {:?}; (iterations, fallback iterations) per solve {rec:?}",
                t0.elapsed()
            );
            c.holds(
                "P-f: succeeds with a fallback iteration",
                rec.iter().any(|x| x.1 >= 1),
                &format!("{rec:?}"),
            );
        }
        Err(e) => {
            println!("P-f: fails in {:?}: {e}", t0.elapsed());
            c.holds(
                "P-f: fails without the curvature part",
                matches!(e, FemError::Solver { reason, .. } if reason.contains("without the curvature part")),
                &format!("{e}"),
            );
        }
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ Hertz

fn sphere_pair(h: f64) -> ContactPair {
    ContactPair::new(
        "top",
        Target::RigidSphere {
            centre: [0.0, 0.0, R_H],
            radius: R_H,
        },
        100.0 / h,
    )
}

fn hs20_mesh() -> SolidMesh {
    upgrade_to_hex20(&hertz_block(4, 5, 4)).unwrap().0
}

fn hs10_mesh() -> SolidMesh {
    upgrade_to_tet10(&split_hexahedra(&hertz_block(3, 4, 4)).unwrap())
        .unwrap()
        .0
}

/// ADR 0551 HS's model and options (D6), the total Lagrangian body and the fallback as asked.
fn hertz_setup(
    mesh: SolidMesh,
    tl: bool,
    fallback: bool,
) -> (SolidModel, Vec<rcfd_fem::constraint::Prescribed>, ContactOptions) {
    let pres = hertz_prescribed(&mesh);
    let mut model = SolidModel::new(mesh);
    model.add_material("all", elastic(1.0, NU_H)).unwrap();
    let mut o = ContactOptions::new(1, 1e-8)
        .newton(NewtonOptions::new(1).max_iterations(30))
        .quadratic_contactor(QuadraticContactor::Mortar);
    if tl {
        o = o
            .kinematics(Kinematics::TotalLagrangian)
            .factor(DirectFactor::SupernodalNestedDissection);
    }
    if fallback {
        o = o.curvature_fallback(true);
    }
    (model, pres, o)
}

/// ADR 0570 D5: `a_M = sqrt(5/2 M_2/M_0)` over the nodes with `t > 0` at their reference positions.
fn moment_radius(model: &SolidModel, r: &ContactResult) -> f64 {
    let (mut m0, mut m2) = (0.0, 0.0);
    for v in r.nodes.iter().filter(|v| v.force > 0.0) {
        let x = point_reference(model, v);
        m0 += v.force;
        m2 += v.force * (x[0] * x[0] + x[1] * x[1]);
    }
    (2.5 * m2 / m0).sqrt()
}

/// HS20f, HS10f (`tl`, with FB) and HS20m-M, HS10m-M (`!tl`): ADR 0570 D4.
fn hertz_run(c: &mut Checks, tag: &str, mesh: SolidMesh, h: f64, tl: bool) {
    let (model, pres, o) = hertz_setup(mesh, tl, tl);
    let t0 = std::time::Instant::now();
    let r = solve(&model, &pres, &[], &[sphere_pair(h)], &o);
    let elapsed = t0.elapsed();
    let r = match r {
        Ok(r) => r,
        Err(e) => {
            c.holds(&format!("{tag} solve"), false, &format!("{e} ({elapsed:?})"));
            return;
        }
    };
    c.holds(&format!("{tag} solve"), true, "succeeds with the gap tolerance 1e-8");
    let s = r.steps.last().unwrap();
    let fallbacks: usize = s.solves.iter().map(|v| v.convex_iterations).sum();
    println!(
        "{tag}: {} unknowns, {elapsed:?}, {} Newton solves ({} augmentations), iterations {:?}, fallback iterations {:?}, \
         contactor nodes {}, active {}",
        model.dof_map().n_equations(),
        s.solves.len(),
        s.solves.len() - 1,
        s.solves.iter().map(|v| v.residuals.len() - 1).collect::<Vec<_>>(),
        s.solves.iter().map(|v| v.convex_iterations).collect::<Vec<_>>(),
        r.nodes.len(),
        r.nodes.iter().filter(|n| n.active).count()
    );
    let dofs = model.dof_map();
    let re = r.result.reactions().unwrap();
    let mut eqs: Vec<usize> = pres
        .iter()
        .filter(|p| p.component == 2)
        .map(|p| dofs.equation(p.node, 2).unwrap())
        .collect();
    eqs.sort_unstable();
    eqs.dedup();
    let f_fe = 4.0 * eqs.iter().map(|&e| re[e]).sum::<f64>().abs();
    let hz = hertz_read(&model, &r);
    let (a_h, p_h, appr) = hertz_theory(f_fe);
    let a_m = moment_radius(&model, &r);
    println!(
        "{tag}: F_fe {f_fe:.6e} (4 sum t |n_z| {:.6e}), a_M {a_m:.6e}, a_H {a_h:.6e}, nodal-fit a_fe {:.6e} (error {:.3e}, \
         recorded, no limit), p_fit {:.6e}, p_H {p_h:.6e}, penetration {:.3e}",
        hz.force,
        hz.a_fe,
        (hz.a_fe / a_h - 1.0).abs(),
        hz.p_fit,
        hz.penetration
    );
    c.at_most(
        &format!("{tag} contact radius (moments)"),
        (a_m / a_h - 1.0).abs(),
        0.15,
    );
    if tl {
        c.at_most(&format!("{tag} peak pressure"), (hz.p_fit / p_h - 1.0).abs(), 0.20);
        c.at_most(&format!("{tag} approach"), appr, 0.10);
        c.holds(
            &format!("FB {tag}"),
            fallbacks >= 1,
            &format!("{fallbacks} fallback iterations"),
        );
    } else {
        println!(
            "{tag}: peak error {:.3e}, approach error {appr:.3e} (recorded)",
            (hz.p_fit / p_h - 1.0).abs()
        );
    }
}

#[test]
fn nf_hs_default_keeps_the_refusal() {
    let (model, pres, o) = hertz_setup(hs20_mesh(), true, false);
    let t0 = std::time::Instant::now();
    let e = solve(&model, &pres, &[], &[sphere_pair(A_H / 4.0)], &o);
    println!("NF-HS ({:?}): {:?}", t0.elapsed(), e.as_ref().err());
    let mut c = Checks::new();
    c.holds(
        "NF-HS: ADR 0551's refusal",
        matches!(&e, Err(FemError::Solver { solver, reason }) if *solver == "contact"
            && reason.contains("load step 1 of 1, augmentation 0")
            && reason.contains("singular tangent: pivot")
            && reason.contains("at node 1129 component 1")),
        "",
    );
    c.finish();
}

#[test]
#[ignore = "heavy record of ADR 0570 (run once): HS20f"]
fn hs20f_hertz_on_hex20_mortar_total_lagrangian_with_fallback() {
    let mut c = Checks::new();
    hertz_run(&mut c, "HS20f", hs20_mesh(), A_H / 4.0, true);
    c.finish();
}

#[test]
#[ignore = "heavy record of ADR 0570 (run once): HS10f"]
fn hs10f_hertz_on_tet10_mortar_total_lagrangian_with_fallback() {
    let mut c = Checks::new();
    hertz_run(&mut c, "HS10f", hs10_mesh(), A_H / 3.0, true);
    c.finish();
}

#[test]
#[ignore = "heavy record of ADR 0570 (run once): HS20m-M"]
fn hs20m_m_hertz_on_hex20_mortar_moment_radius() {
    let mut c = Checks::new();
    hertz_run(&mut c, "HS20m-M", hs20_mesh(), A_H / 4.0, false);
    c.finish();
}

#[test]
#[ignore = "heavy record of ADR 0570 (run once): HS10m-M"]
fn hs10m_m_hertz_on_tet10_mortar_moment_radius() {
    let mut c = Checks::new();
    hertz_run(&mut c, "HS10m-M", hs10_mesh(), A_H / 3.0, false);
    c.finish();
}
