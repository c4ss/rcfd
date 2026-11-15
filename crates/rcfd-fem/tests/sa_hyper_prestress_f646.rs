//! F-646 (ADR 0586): the modes about a nonlinear static state of a stretched hyperelastic bar. S0, T0: the set-ups; KM: the
//! driver's tangent at the state against the central differences of the nonlinear static residual, entry by entry; F1: the
//! frequencies of `tangent_modes` against the central differences of the residual along each mode; C0: the same measure at
//! rest; R0, R1: at a load of zero, against `modal_analysis`; N1 to N3: negative controls; KE: the eigenvalues of the
//! full-matrix central difference (a record). Rows NH (neo-Hookean) and MR (Mooney-Rivlin). Every measured value is printed;
//! a non-finite value or a refused call is a miss.

use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::{prescribed_equations, reduce};
use rcfd_fem::element::ElementState;
use rcfd_fem::follower::tangent_modes;
use rcfd_fem::geometric::{prestressed_modes, Prestress};
use rcfd_fem::mass::consistent;
use rcfd_fem::material::HyperelasticEnergy;
use rcfd_fem::modal::{modal_analysis, ModalOptions, ModalResult};
use rcfd_fem::nonlinear::{initial_states, internal_force, solve, tangent, NewtonOptions};
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{Constraint, Load, Material, SolidModel};
use rcfd_linalg::eigen::{lowest_eigenpairs, Shift, SubspaceOptions};
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;

/// Measured values against their limits; the test fails at the end if any missed.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value <= limit;
        println!(
            "{name}: {value:.6e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} not <= {limit:e}"));
        }
    }
    fn at_least(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value >= limit;
        println!(
            "{name}: {value:.6e} (limit >= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} not >= {limit:e}"));
        }
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {detail} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name}: {detail}"));
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:?}", self.0);
    }
}

// ----------------------------------------------------------------------------------------------- set-up (ADR 0586 D2)

const RHO: f64 = 1000.0;
/// The axial force on each node of `xmax`, N.
const P: f64 = 4e3;
/// The step of the directional central difference, m (arithmetic 2).
const A_STEP: f64 = 5e-6;
/// The step of the full-matrix central difference, m (arithmetic 3).
const H_STEP: f64 = 1e-6;
const MODES: usize = 6;

fn nhc() -> HyperelasticEnergy {
    HyperelasticEnergy::neo_hookean(1e6, 1e7).unwrap()
}
fn mrc() -> HyperelasticEnergy {
    HyperelasticEnergy::mooney_rivlin(3.5e5, 1.5e5, 1e7).unwrap()
}

/// Bar S: Full Hex8 `[0, 2] x [0, 0.5] x [0, 0.5]`, 8 x 2 x 2, held on `xmin`, `P` in `x` on each node of `xmax`.
fn bar(energy: HyperelasticEnergy) -> SolidModel {
    let mesh = hex_box([0.0; 3], [2.0, 0.5, 0.5], [8, 2, 2]).unwrap();
    let mut model = SolidModel::new(mesh);
    model
        .add_material("all", Material::hyperelastic(energy, RHO).unwrap())
        .unwrap();
    for k in 0..3 {
        model.add_constraint(Constraint::displacement("xmin", k, 0.0)).unwrap();
    }
    model.add_load(Load::nodal_force("xmax", [P, 0.0, 0.0])).unwrap();
    model
}

/// The dead load times the load factor.
fn external(model: &SolidModel, load_factor: f64) -> Vec<f64> {
    let dofs = model.dof_map();
    let mut f = vec![0.0; dofs.n_equations()];
    for &n in model.mesh().node_set("xmax").unwrap() {
        f[dofs.equation(n, 0).unwrap()] += load_factor * P;
    }
    f
}

/// The nonlinear static residual `r(u) = f_int(u) - load_factor f_ext`.
fn residual(model: &SolidModel, u: &[f64], states: &[ElementState], load_factor: f64) -> Vec<f64> {
    let f = internal_force(model, u, states).unwrap();
    let fe = external(model, load_factor);
    f.iter().zip(&fe).map(|(a, b)| a - b).collect()
}

fn dense(a: &CsrMatrix) -> Vec<Vec<f64>> {
    let mut d = vec![vec![0.0; a.n]; a.n];
    for (i, row) in d.iter_mut().enumerate() {
        for k in a.row_ptr[i]..a.row_ptr[i + 1] {
            row[a.col[k]] += a.val[k];
        }
    }
    d
}

fn quad(a: &[Vec<f64>], x: &[f64]) -> f64 {
    a.iter()
        .zip(x)
        .map(|(row, xi)| xi * row.iter().zip(x).map(|(v, xj)| v * xj).sum::<f64>())
        .sum()
}

fn freq(lambda: f64) -> f64 {
    lambda.signum() * lambda.abs().sqrt() / (2.0 * PI)
}

/// Worst relative difference of two frequency lists, position by position; infinite if the lengths differ.
fn worst(a: &[f64], b: &[f64]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return f64::INFINITY;
    }
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y).abs() / y.abs())
        .fold(0.0f64, |m, v| if v.is_finite() { m.max(v) } else { f64::INFINITY })
}

/// D3 (a): the frequency of the central difference of the residual along each mode (scaled to a unit largest component),
/// with the step `A_STEP`.
fn directional(
    model: &SolidModel,
    u: &[f64],
    states: &[ElementState],
    load_factor: f64,
    modes: &ModalResult,
    m: &[Vec<f64>],
) -> Vec<f64> {
    modes
        .modes
        .iter()
        .map(|phi| {
            let scale = phi.iter().fold(0.0f64, |s, v| s.max(v.abs()));
            let ph: Vec<f64> = phi.iter().map(|v| v / scale).collect();
            let up: Vec<f64> = u.iter().zip(&ph).map(|(x, p)| x + A_STEP * p).collect();
            let um: Vec<f64> = u.iter().zip(&ph).map(|(x, p)| x - A_STEP * p).collect();
            let rp = residual(model, &up, states, load_factor);
            let rm = residual(model, &um, states, load_factor);
            let g: f64 = ph.iter().zip(rp.iter().zip(&rm)).map(|(p, (a, b))| p * (a - b)).sum();
            freq(g / (2.0 * A_STEP) / quad(m, &ph))
        })
        .collect()
}

/// The Rayleigh quotients of a matrix along the (scaled) modes, as frequencies.
fn rayleigh(k: &[Vec<f64>], m: &[Vec<f64>], modes: &ModalResult) -> Vec<f64> {
    modes
        .modes
        .iter()
        .map(|phi| freq(quad(k, phi) / quad(m, phi)))
        .collect()
}

/// The full-matrix central difference of the residual (D3 b), dense, all equations.
fn full_difference(model: &SolidModel, u: &[f64], states: &[ElementState], load_factor: f64) -> Vec<Vec<f64>> {
    let n = u.len();
    let mut k = vec![vec![0.0; n]; n];
    let mut w = u.to_vec();
    for j in 0..n {
        w[j] = u[j] + H_STEP;
        let rp = residual(model, &w, states, load_factor);
        w[j] = u[j] - H_STEP;
        let rm = residual(model, &w, states, load_factor);
        w[j] = u[j];
        for i in 0..n {
            k[i][j] = (rp[i] - rm[i]) / (2.0 * H_STEP);
        }
    }
    k
}

fn max_abs(a: &[Vec<f64>]) -> f64 {
    a.iter().flatten().fold(0.0f64, |s, v| s.max(v.abs()))
}

fn modes_ok(c: &mut Checks, name: &str, r: &Result<ModalResult, String>) {
    match r {
        Ok(m) => {
            let worst_res = m.residuals.iter().cloned().fold(0.0f64, f64::max);
            c.holds(
                name,
                m.frequencies.len() == MODES && worst_res <= 1e-10,
                &format!(
                    "Ok, {} frequencies {:?} Hz, iterations {}, worst residual {worst_res:.3e}",
                    m.frequencies.len(),
                    m.frequencies,
                    m.iterations
                ),
            );
        }
        Err(e) => c.holds(name, false, e),
    }
}

/// S0 to N3 for one energy (N2 for NH only).
fn run_row(c: &mut Checks, row: &str, energy: HyperelasticEnergy, n2: bool) {
    let model = bar(energy);
    let n = model.dof_map().n_equations();
    let options = ModalOptions::new(MODES);
    let mass = dense(&consistent(&model).unwrap());

    // S0.
    let solved = match solve(&model, &[], &[], &NewtonOptions::new(10)) {
        Ok(r) => r,
        Err(e) => {
            c.holds(&format!("S0 {row}: the nonlinear solve"), false, &e.to_string());
            return;
        }
    };
    let u = solved.result.displacement_values().to_vec();
    let states = solved.states.clone();
    let dofs = model.dof_map();
    let tip = model.mesh().node_set("xmax").unwrap();
    let mean_ux = tip.iter().map(|&k| u[dofs.equation(k, 0).unwrap()]).sum::<f64>() / tip.len() as f64;
    c.holds(
        &format!("S0 {row}: the nonlinear solve"),
        true,
        &format!(
            "converged, {} steps, stretch 1 + mean u_x(xmax)/2 = {:.8}",
            solved.steps.len(),
            1.0 + mean_ux / 2.0
        ),
    );

    // T0.
    let rest_states = initial_states(&model).unwrap();
    let zero = vec![0.0; n];
    let ts = tangent_modes(&model, &[], &u, &states, 1.0, &[], &options).map_err(|e| e.to_string());
    let tr = tangent_modes(&model, &[], &zero, &rest_states, 0.0, &[], &options).map_err(|e| e.to_string());
    let ma = modal_analysis(&model, &options).map_err(|e| e.to_string());
    modes_ok(c, &format!("T0 {row}: tangent_modes at the stretched state"), &ts);
    modes_ok(c, &format!("T0 {row}: tangent_modes at the zero-load state"), &tr);
    modes_ok(c, &format!("T0 {row}: modal_analysis"), &ma);

    // KM.
    let kt = dense(&tangent(&model, &u, &states, None).unwrap());
    let kfd = full_difference(&model, &u, &states, 1.0);
    let mut diff = 0.0f64;
    for i in 0..n {
        for j in 0..n {
            diff = diff.max((kfd[i][j] - kt[i][j]).abs());
        }
    }
    c.at_most(
        &format!("KM {row}: max |K_FD - K_T| / max |K_T| at the stretched state"),
        diff / max_abs(&kt),
        1e-8,
    );

    // R0.
    let r0 = residual(&model, &zero, &rest_states, 0.0);
    c.at_most(
        &format!("R0 {row}: max |r(0)| at the zero-load state (N)"),
        r0.iter().fold(0.0f64, |s, v| s.max(v.abs())),
        1e-9,
    );

    let (Ok(ts), Ok(tr), Ok(ma)) = (ts, tr, ma) else {
        c.holds(&format!("F1, C0, R1, N1 to N3 {row}"), false, "not measured (T0)");
        return;
    };

    // F1.
    let f_fd = directional(&model, &u, &states, 1.0, &ts, &mass);
    println!("  {row} f_FD at the stretched state {f_fd:?}");
    c.at_most(
        &format!("F1 {row}: max |f_FD - f| / f at the stretched state"),
        worst(&f_fd, &ts.frequencies),
        1e-6,
    );
    // C0.
    let f_fd0 = directional(&model, &zero, &rest_states, 0.0, &tr, &mass);
    c.at_most(
        &format!("C0 {row}: max |f_FD - f| / f at rest"),
        worst(&f_fd0, &tr.frequencies),
        1e-6,
    );
    // R1.
    c.at_most(
        &format!("R1 {row}: tangent_modes at zero load against modal_analysis"),
        worst(&tr.frequencies, &ma.frequencies),
        1e-10,
    );
    // N1.
    c.at_least(
        &format!("N1 {row}: stretched against rest"),
        worst(&ts.frequencies, &ma.frequencies),
        1e-2,
    );
    // N2.
    if n2 {
        let pre = solve_static(&model, &StaticOptions::default())
            .and_then(|s| Prestress::from_static(&model, &s))
            .and_then(|p| prestressed_modes(&model, &p, &options));
        match pre {
            Ok(l) => {
                println!("  {row} linear pre-stressed frequencies {:?}", l.frequencies);
                c.at_least(
                    &format!("N2 {row}: tangent_modes at the state against the linear pre-stress path"),
                    worst(&ts.frequencies, &l.frequencies),
                    1e-4,
                );
            }
            Err(e) => c.holds(&format!("N2 {row}: the linear pre-stress path"), false, &e.to_string()),
        }
    }
    // N3.
    let k0 = dense(&tangent(&model, &zero, &rest_states, None).unwrap());
    let f0 = rayleigh(&k0, &mass, &ts);
    c.at_least(
        &format!("N3 {row}: f_FD against the Rayleigh quotients of the tangent at rest"),
        f0.iter()
            .zip(&f_fd)
            .map(|(a, b)| (a - b).abs() / b.abs())
            .fold(0.0f64, |m, v| if v.is_finite() { m.max(v) } else { f64::INFINITY }),
        1e-2,
    );
}

/// S0, T0, KM, F1, C0, R0, R1, N1 to N3 (ADR 0586 checks), rows NH and MR.
#[test]
fn s0_to_n3_stretched_bar() {
    let mut c = Checks::new();
    run_row(&mut c, "NH", nhc(), true);
    run_row(&mut c, "MR", mrc(), false);
    c.finish();
}

/// KE (a record, no limit): the six lowest frequencies of the symmetrised reduced full-matrix central difference against
/// `tangent_modes` at the stretched state, NH.
#[test]
fn ke_full_difference_record() {
    let model = bar(nhc());
    let solved = solve(&model, &[], &[], &NewtonOptions::new(10)).unwrap();
    let u = solved.result.displacement_values().to_vec();
    let states = solved.states.clone();
    let n = u.len();
    let ts = tangent_modes(&model, &[], &u, &states, 1.0, &[], &ModalOptions::new(MODES)).unwrap();
    let kt = dense(&tangent(&model, &u, &states, None).unwrap());
    let kfd = full_difference(&model, &u, &states, 1.0);
    let mut asym = 0.0f64;
    let mut triplets = Vec::new();
    for i in 0..n {
        for j in 0..n {
            asym = asym.max((kfd[i][j] - kfd[j][i]).abs());
            let v = 0.5 * (kfd[i][j] + kfd[j][i]);
            if v != 0.0 {
                triplets.push((i, j, v));
            }
        }
    }
    println!(
        "KE asymmetry max |K_FD - K_FD^T| / max |K_T|: {:.6e}",
        asym / max_abs(&kt)
    );
    let k = CsrMatrix::from_triplets(n, triplets);
    let fixed: Vec<(usize, f64)> = prescribed_equations(&model, &[])
        .unwrap()
        .into_iter()
        .map(|(eq, _)| (eq, 0.0))
        .collect();
    let zero = vec![0.0; n];
    let kr = reduce(&k, &zero, &fixed).unwrap();
    let mr = reduce(&consistent(&model).unwrap(), &zero, &fixed).unwrap();
    let opts = SubspaceOptions {
        shift: Shift::Value(0.0),
        ..SubspaceOptions::default()
    };
    match lowest_eigenpairs(&kr.matrix, &mr.matrix, MODES, &opts) {
        Ok(eig) => {
            let f: Vec<f64> = eig.values.iter().map(|&l| freq(l)).collect();
            println!("KE frequencies of K_FD {f:?} (iterations {})", eig.iterations);
            println!("KE tangent_modes {:?}", ts.frequencies);
            println!(
                "KE worst relative difference (record, no limit): {:.6e}",
                worst(&f, &ts.frequencies)
            );
        }
        Err(e) => println!("KE lowest_eigenpairs failed (record): {e}"),
    }
}
