//! G1 of docs/adr/0055-wave29a-rotor-interfaces-bcs.md: the F1/F2 case of ADR 0022 with `pressure_update = "guarded_secant"`.
//! Registered limits: outlet mass-flow error <= 1e-4 relative in 6000 iterations (the limit originally registered for the
//! secant), converged, F2 slope error <= 1 %, imbalance <= 1e-9.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;

fn text(update: &str) -> String {
    format!(
        r#"
[mesh]
generator = "channel_2d"
nx = 100
ny = 20
lx = 10.0
ly = 1.0
depth = 0.1
[physics]
nu = 0.1
rho = 1.0
[boundaries.inlet]
velocity = "zero_gradient"
pressure = "fixed"
pressure_value = 12.0
[boundaries.outlet]
bc_kind = "target_mass_flow_outlet"
target_mass_flow = 0.1
bc_pressure = 0.0
pressure_update = "{update}"
[boundaries.walls]
velocity = "fixed"
[solver]
algorithm = "simplec"
convection = "linear_upwind"
max_iterations = 6000
tolerance_u = 1e-12
tolerance_p = 1e-12
"#
    )
}

fn mass(s: &Simulation, patch: &str) -> f64 {
    s.geom
        .mesh
        .patch(patch)
        .unwrap()
        .faces()
        .map(|f| s.flux()[f])
        .sum::<f64>()
}

fn slope(s: &Simulation, x0: f64, x1: f64) -> f64 {
    let p = s.field("p").unwrap();
    let cc = s.geom.mesh.cell_centres();
    let mut cols: std::collections::BTreeMap<i64, (f64, f64, usize)> = Default::default();
    for (c, x) in cc.iter().enumerate() {
        if x[0] >= x0 && x[0] <= x1 {
            let e = cols.entry((x[0] * 1e9).round() as i64).or_insert((x[0], 0.0, 0));
            e.1 += p[c];
            e.2 += 1;
        }
    }
    let pts: Vec<(f64, f64)> = cols.values().map(|(x, sum, n)| (*x, sum / *n as f64)).collect();
    let n = pts.len() as f64;
    let (mx, mp) = (
        pts.iter().map(|q| q.0).sum::<f64>() / n,
        pts.iter().map(|q| q.1).sum::<f64>() / n,
    );
    pts.iter().map(|q| (q.0 - mx) * (q.1 - mp)).sum::<f64>() / pts.iter().map(|q| (q.0 - mx) * (q.0 - mx)).sum::<f64>()
}

fn check(update: &str) {
    let case = Case::parse_toml(&text(update)).unwrap_or_else(|e| panic!("{e:?}"));
    let mut s = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"));
    let n = s.run(6000);
    assert!(s.failure().is_none(), "{:?}", s.failure());
    let converged = s.is_converged();
    let m = mass(&s, "outlet");
    let e = (m - 0.1).abs() / 0.1;
    let sl = slope(&s, 6.0, 9.0);
    let es = (sl + 1.2).abs() / 1.2;
    let flows: Vec<f64> = s
        .geom
        .mesh
        .patches()
        .iter()
        .map(|p| p.faces().map(|f| s.flux()[f]).sum::<f64>())
        .collect();
    let im = flows.iter().sum::<f64>().abs() / flows.iter().fold(0.0_f64, |a, v| a.max(v.abs()));
    println!(
        "G1 {update}: m_out = {m:.9} after {n} iterations (converged {converged}), error {e:.2e} (limit 1e-4); F2 slope {sl:.6}, error {:.3} % (limit 1 %); imbalance {im:.2e} (1e-9)",
        100.0 * es
    );
    assert!(converged, "{update}: not converged");
    assert!(e <= 1e-4, "{update}: m error {e}");
    assert!(es <= 0.01, "{update}: slope {es}");
    assert!(im <= 1e-9, "{update}: imbalance {im}");
}

#[test]
fn g1_guarded_secant_target_mass_flow_outlet() {
    check("guarded_secant");
}
