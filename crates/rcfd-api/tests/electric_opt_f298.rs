//! F-298 (ADR 0221) part (a), case tree: with `solver.gradient_equations.electric_potential` the boundary current density and the
//! Joule heat use the currents of the operator the potential was solved with, checked by a current and a power balance. Limits are
//! those registered in
//! docs/adr/0221-f298-registration-electric-current-and-joule-heat-with-the-gradient-options-and-the-fixed-pressure-flux-correction.md
//! before this file was written.
//!
//! E2: ADR 0204's E1 (mesh W, prescribed zero flow, steady energy, the potential fixed on the inlet side and on the ramp) run for
//! 200 iterations. For a finished run the test rebuilds, with `rcfd_fv` calls only, the field (the tree's final potential with the
//! case's conditions), the new currents `F_new` (`face_currents_opt` with the set's options), the unchanged ones `F_old`, and the
//! residual `r` of the solved equation. P: W is skewed. F: the runs end finite. E-tree: per cell the new currents are the
//! operator's. R-eq-J, R-eq-Q: the tree's current density and Joule heat are the library's. B1: the boundary currents add up to
//! zero within `tol_A = sum |r|`; B2: `tol_A <= 1e-8 I_in`; B3: the same per group of cells. J1: the Joule heat equals the
//! boundary power within `R = sum |phi| |r|`. T-B1, T-O1, T-B3, T-J1, T-J1-D: the unchanged quantities miss (the default's own
//! Joule heat included). D1: the keys and the table at their defaults change no hash and no bit.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_fv::cht::group_net_outflow;
use rcfd_fv::diffusion::{laplacian_opt, BoundaryGradient, GradientOptions};
use rcfd_fv::electric::joule_heat_opt;
use rcfd_fv::electric::{
    boundary_power, boundary_power_opt, face_conductivity, face_currents, face_currents_opt, joule_heat,
};
use rcfd_fv::gradient::{least_squares, Limiter};
use rcfd_fv::{FixedValue, FvGeometry, ScalarField, ZeroGradient};

/// E-tree limit, relative to the flux scale.
const IDENTITY: f64 = 1e-12;
/// R-eq-J and R-eq-Q limits, relative to the largest boundary current and the largest cell heat.
const REPORT: f64 = 1e-12;
/// Round-off allowance of B1, B3 (relative to `F`) and of J1 (relative to `W`).
const ROUND_OFF: f64 = 1e-12;
/// B2: the solver tolerance against the current into the domain.
const CONVERGED: f64 = 1e-8;
/// P: the smallest maximum non-orthogonality of W, degrees.
const SKEWED: f64 = 20.0;

/// One option set: name, the `[solver]` lines, the library options.
struct Set {
    name: &'static str,
    keys: &'static str,
    opts: Option<GradientOptions>,
}

/// The default and ADR 0190's sets O1 to O6 and C2.
fn sets() -> Vec<Set> {
    let d = GradientOptions::default();
    let venk = |k| Some(Limiter::Venkatakrishnan(k));
    let set = |name, keys, opts| Set {
        name,
        keys,
        opts: Some(opts),
    };
    vec![
        Set {
            name: "default",
            keys: "",
            opts: None,
        },
        set(
            "O1",
            "gradient_face_correction = true\n",
            GradientOptions {
                face_correction: true,
                ..d
            },
        ),
        set(
            "O2",
            "gradient_boundary = \"corrected\"\n",
            GradientOptions {
                boundary: BoundaryGradient::Corrected,
                ..d
            },
        ),
        set(
            "O3",
            "gradient_boundary = \"dropped\"\n",
            GradientOptions {
                boundary: BoundaryGradient::Dropped,
                ..d
            },
        ),
        set(
            "O4",
            "gradient_secondary_limiter = \"barth_jespersen\"\n",
            GradientOptions {
                secondary_limiter: Some(Limiter::BarthJespersen),
                ..d
            },
        ),
        set(
            "O5",
            "gradient_secondary_limiter = \"venkatakrishnan\"\n",
            GradientOptions {
                secondary_limiter: venk(5.0),
                ..d
            },
        ),
        set(
            "O6",
            "gradient_secondary_limiter = \"venkatakrishnan\"\ngradient_limiter_k = 0.5\n",
            GradientOptions {
                secondary_limiter: venk(0.5),
                ..d
            },
        ),
        set(
            "C2",
            "gradient_face_correction = true\ngradient_boundary = \"corrected\"\ngradient_secondary_limiter = \"venkatakrishnan\"\n",
            GradientOptions {
                face_correction: true,
                secondary_limiter: venk(5.0),
                boundary: BoundaryGradient::Corrected,
            },
        ),
    ]
}

/// The four F-256 keys and the table with its four selectors written at their defaults (D1).
const DEFAULT_KEYS: &str = "gradient_face_correction = false\ngradient_secondary_limiter = \"none\"\ngradient_limiter_k = 5.0\ngradient_boundary = \"owner\"\n";
const DEFAULT_TABLE: &str = "momentum = false\npressure = false\nturbulence = false\nelectric_potential = false\n";

/// E2 with extra `[solver]` lines and the lines of `[solver.gradient_equations]` (none: no table).
fn text(solver: &str, selectors: &str) -> String {
    let table = if selectors.is_empty() {
        String::new()
    } else {
        format!("[solver.gradient_equations]\n{selectors}")
    };
    format!(
        r#"title = "f298 electric"
[mesh]
generator = "wedge_2d"
nx = 16
ny = 16
lx = 1.0
ly = 1.0
ramp_start = 0.0
ramp_angle = 30.0
[physics]
rho = 1.0
cp = 1.0
conductivity = 0.1
[energy]
enabled = true
relax = 0.9
[initial]
temperature = 0.5
[electric]
enabled = true
conductivity = 1.0
[boundaries.xmin]
velocity = "fixed"
temperature = "fixed"
temperature_value = 1.0
electric = "potential"
electric_potential = 1.0
[boundaries.ymin]
velocity = "fixed"
temperature = "fixed"
temperature_value = 0.0
electric = "potential"
electric_potential = 0.0
[boundaries.xmax]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[solver]
convection = "linear_upwind"
frozen_flow = true
algorithm = "simplec"
max_iterations = 200
tolerance_u = 1e-14
tolerance_p = 1e-14
{solver}{table}"#
    )
}

fn case(text: &str) -> Case {
    Case::parse_toml(text).unwrap_or_else(|d| panic!("case does not parse: {d:?}\n{text}"))
}

/// The tree fields of a finished run: potential, Joule heat, temperature, boundary current density.
struct Tree {
    sim: Simulation,
    phi: Vec<f64>,
    q: Vec<f64>,
    t: Vec<f64>,
    j: Vec<f64>,
    failure: Option<String>,
}

fn run(text: &str) -> Tree {
    let mut sim = Simulation::new(&case(text)).unwrap_or_else(|d| panic!("not built: {d:?}"));
    sim.run(10_000);
    let failure = sim.failure().map(|f| format!("{f:?}"));
    let phi = sim.field("electric_potential").unwrap();
    let q = sim.field("joule_heat").unwrap();
    let t = sim.field("T").unwrap();
    let j = sim.boundary_field("electric_current_density").unwrap();
    Tree {
        sim,
        phi,
        q,
        t,
        j,
        failure,
    }
}

fn finite(v: &[f64]) -> bool {
    v.iter().all(|x| x.is_finite())
}

/// The library field of E2 with the tree's potential.
fn field(geom: &std::sync::Arc<FvGeometry>, values: &[f64]) -> ScalarField {
    let mut phi = ScalarField::uniform("phi", geom, 0.0);
    phi.set_bc(geom, "xmin", Box::new(FixedValue::scalar(1.0))).unwrap();
    phi.set_bc(geom, "ymin", Box::new(FixedValue::scalar(0.0))).unwrap();
    for patch in ["xmax", "ymax"] {
        phi.set_bc(geom, patch, Box::new(ZeroGradient)).unwrap();
    }
    phi.values = values.to_vec();
    phi
}

/// The current leaving every cell through its faces.
fn net(geom: &FvGeometry, flows: &[f64]) -> Vec<f64> {
    let (own, nei) = (geom.mesh.owner(), geom.mesh.neighbour());
    let mut out = vec![0.0; geom.n_cells()];
    for f in 0..geom.mesh.n_faces() {
        if geom.empty[f] {
            continue;
        }
        out[own[f]] += flows[f];
        if f < geom.n_internal {
            out[nei[f]] -= flows[f];
        }
    }
    out
}

/// The sum over the non-empty boundary faces.
fn boundary_sum(geom: &FvGeometry, flows: &[f64]) -> f64 {
    (geom.n_internal..geom.mesh.n_faces())
        .filter(|&f| !geom.empty[f])
        .map(|f| flows[f])
        .sum()
}

/// `W = sum_int |F_f| (|phi_o| + |phi_n|) + sum_b |F_b| (|phi_P| + |phi_b|)`.
fn power_scale(geom: &FvGeometry, phi: &ScalarField, flows: &[f64]) -> f64 {
    let (own, nei) = (geom.mesh.owner(), geom.mesh.neighbour());
    let bv = phi.boundary_values(geom);
    let mut w = 0.0;
    for f in 0..geom.mesh.n_faces() {
        if geom.empty[f] {
            continue;
        }
        let other = if f < geom.n_internal {
            phi.values[nei[f]].abs()
        } else {
            bv[f - geom.n_internal].abs()
        };
        w += flows[f].abs() * (phi.values[own[f]].abs() + other);
    }
    w
}

fn max_abs(v: &[f64]) -> f64 {
    v.iter().fold(0.0_f64, |m, x| m.max(x.abs()))
}

/// P, F, E-tree, R-eq-J, R-eq-Q, B1 to B3, J1 and the teeth for the default and every set.
#[test]
fn e2_current_and_power_balances_with_every_option_set() {
    let mut missed = Vec::new();
    for set in sets() {
        let n = set.name;
        let selectors = if set.opts.is_some() {
            "electric_potential = true\n"
        } else {
            ""
        };
        let tr = run(&text(set.keys, selectors));
        let geom = tr.sim.geom.clone();
        if set.opts.is_none() {
            let q = rcfd_mesh::quality::check_mesh(&geom.mesh);
            println!(
                "P: mesh W max non-orthogonality {:.3} degrees (at least {SKEWED})",
                q.max_non_orthogonality
            );
            assert!(q.max_non_orthogonality >= SKEWED, "P set-up");
        }
        let ok = tr.failure.is_none() && finite(&tr.phi) && finite(&tr.q) && finite(&tr.t);
        println!(
            "F {n}: failure {:?}, finite {}, {} iterations",
            tr.failure,
            ok,
            tr.sim.iteration()
        );
        if !ok {
            assert!(set.opts.is_some(), "F set-up: the default run");
            missed.push(format!("F {n}: every check of the set NOT MET"));
            continue;
        }
        let phi = field(&geom, &tr.phi);
        let sigma_f = face_conductivity(&geom, &vec![1.0; geom.n_cells()]);
        let f_new = face_currents_opt(&geom, &phi, &sigma_f, true, set.opts);
        let f_old = face_currents(&geom, &phi, &sigma_f, true);
        let bv = phi.boundary_values(&geom);
        let grad = least_squares(&geom, &phi.values, &bv);
        let r = laplacian_opt(
            &geom,
            &sigma_f,
            &phi.boundary_coeffs(&geom),
            Some(&grad),
            &phi.values,
            &bv,
            set.opts,
        )
        .residual(&phi.values);
        let tol: f64 = r.iter().map(|x| x.abs()).sum();
        let cc = geom.mesh.cell_centres();
        let group: Vec<usize> = cc.iter().map(|x| usize::from(x[0] >= 0.5)).collect();
        let tol_g: Vec<f64> = (0..2)
            .map(|g| (0..geom.n_cells()).filter(|&c| group[c] == g).map(|c| r[c].abs()).sum())
            .collect();
        let fsum: f64 = (0..geom.mesh.n_faces())
            .filter(|&f| !geom.empty[f])
            .map(|f| f_new[f].abs())
            .sum();
        let i_in: f64 = (geom.n_internal..geom.mesh.n_faces())
            .filter(|&f| !geom.empty[f])
            .map(|f| (-f_new[f]).max(0.0))
            .sum();
        let rr: f64 = phi.values.iter().zip(&r).map(|(p, x)| p.abs() * x.abs()).sum();
        let w = power_scale(&geom, &phi, &f_new);
        let vol = geom.mesh.cell_volumes();
        let sf = geom.mesh.face_area_vectors();
        // the tree's boundary currents from its current density
        let mut f_tree = vec![0.0; geom.mesh.n_faces()];
        for f in geom.n_internal..geom.mesh.n_faces() {
            if !geom.empty[f] {
                f_tree[f] = -tr.j[f - geom.n_internal] * rcfd_core::vec3::norm(sf[f]);
            }
        }
        // E-tree
        let nn = net(&geom, &f_new);
        let s = (0..geom.n_cells())
            .map(|c| {
                geom.mesh
                    .cell_faces(c)
                    .iter()
                    .filter(|&&f| !geom.empty[f])
                    .map(|&f| f_new[f].abs())
                    .sum::<f64>()
            })
            .fold(0.0, f64::max);
        let et = nn.iter().zip(&r).map(|(a, b)| (a + b).abs()).fold(0.0, f64::max);
        println!("E-tree {n}: {:.3e} of S (limit {IDENTITY:e}; S = {s:.4e})", et / s);
        if et > IDENTITY * s {
            missed.push(format!("E-tree {n}: {:e}", et / s));
        }
        // R-eq-J
        let bmax = max_abs(&f_new[geom.n_internal..]);
        let dj = (geom.n_internal..geom.mesh.n_faces())
            .filter(|&f| !geom.empty[f])
            .map(|f| (f_tree[f] - f_new[f]).abs())
            .fold(0.0, f64::max);
        match set.opts {
            None => {
                let nb = (geom.n_internal..geom.mesh.n_faces())
                    .filter(|&f| {
                        let a = rcfd_core::vec3::norm(sf[f]);
                        let jo = if geom.empty[f] || a == 0.0 { 0.0 } else { -f_old[f] / a };
                        jo.to_bits() != tr.j[f - geom.n_internal].to_bits()
                    })
                    .count();
                println!(
                    "R-eq-J {n}: {nb} boundary faces differ in a bit from the unchanged current density (limit 0)"
                );
                if nb != 0 {
                    missed.push(format!("R-eq-J {n}: {nb} faces"));
                }
            }
            Some(_) => {
                println!(
                    "R-eq-J {n}: {:.3e} of the largest boundary current (limit {REPORT:e})",
                    dj / bmax
                );
                if dj > REPORT * bmax {
                    missed.push(format!("R-eq-J {n}: {:e}", dj / bmax));
                }
            }
        }
        // R-eq-Q
        let q_lib = joule_heat_opt(&geom, &phi, &sigma_f, true, set.opts);
        let qmax = max_abs(&q_lib);
        match set.opts {
            None => {
                let q_old = joule_heat(&geom, &phi, &sigma_f, true);
                let nb = q_old
                    .iter()
                    .zip(&tr.q)
                    .filter(|(a, b)| a.to_bits() != b.to_bits())
                    .count();
                println!("R-eq-Q {n}: {nb} cells differ in a bit from the unchanged Joule heat (limit 0)");
                if nb != 0 {
                    missed.push(format!("R-eq-Q {n}: {nb} cells"));
                }
            }
            Some(_) => {
                let dq = q_lib.iter().zip(&tr.q).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max);
                println!(
                    "R-eq-Q {n}: {:.3e} of the largest cell heat (limit {REPORT:e})",
                    dq / qmax
                );
                if dq > REPORT * qmax {
                    missed.push(format!("R-eq-Q {n}: {:e}", dq / qmax));
                }
            }
        }
        // B1, B2
        let b1 = boundary_sum(&geom, &f_tree).abs();
        let bound1 = tol + ROUND_OFF * fsum;
        println!(
            "B1 {n}: |sum F_b| = {b1:.3e} A against {bound1:.3e} (tol_A {tol:.3e}, 1e-12 F {:.3e}); B2: tol_A / I_in = {:.3e} (limit {CONVERGED:e}; I_in {i_in:.6e})",
            ROUND_OFF * fsum,
            tol / i_in
        );
        if b1 > bound1 {
            missed.push(format!("B1 {n}: {b1:e} > {bound1:e}"));
        }
        if tol > CONVERGED * i_in {
            missed.push(format!("B2 {n}: {:e}", tol / i_in));
        }
        // B3
        let gn = group_net_outflow(&geom, &f_new, &group, 2);
        for g in 0..2 {
            let bound = tol_g[g] + ROUND_OFF * fsum;
            println!("B3 {n} group {g}: |G| = {:.3e} A against {bound:.3e}", gn[g].abs());
            if gn[g].abs() > bound {
                missed.push(format!("B3 {n} group {g}: {:e} > {bound:e}", gn[g].abs()));
            }
        }
        // J1 (sets) and T-J1-D (the default)
        let total: f64 = tr.q.iter().zip(vol).map(|(q, v)| q * v).sum();
        let qmin = tr.q.iter().copied().fold(f64::INFINITY, f64::min);
        let bound_j = rr + ROUND_OFF * w;
        match set.opts {
            Some(_) => {
                let p = boundary_power_opt(&geom, &phi, &sigma_f, true, set.opts);
                let j1 = (total - p).abs();
                println!(
                    "J1 {n}: |sum qV - P| = {j1:.3e} W against {bound_j:.3e} (R {rr:.3e}, 1e-12 W {:.3e}); P = {p:.9e}; smallest cell heat {qmin:.3e}",
                    ROUND_OFF * w
                );
                if j1 > bound_j {
                    missed.push(format!("J1 {n}: {j1:e} > {bound_j:e}"));
                }
            }
            None => {
                let p = boundary_power(&geom, &phi, &sigma_f, true);
                let j1 = (total - p).abs();
                println!(
                    "T-J1-D {n}: the default's own |sum qV - P| = {j1:.3e} W against {bound_j:.3e} (must exceed); P = {p:.9e}; smallest cell heat {qmin:.3e}"
                );
                if j1 <= bound_j {
                    missed.push(format!("T-J1-D: {j1:e} <= {bound_j:e}"));
                }
            }
        }
        if set.opts.is_none() {
            continue;
        }
        // T-B1 (O2 to C2), T-O1 (O1)
        let t_b1 = boundary_sum(&geom, &f_old).abs();
        if n == "O1" {
            let nb = (geom.n_internal..geom.mesh.n_faces())
                .filter(|&f| f_new[f].to_bits() != f_old[f].to_bits())
                .count();
            println!("T-O1: {nb} boundary currents differ in a bit between the new and the unchanged currents (limit 0); unchanged B1 {t_b1:.3e}");
            if nb != 0 {
                missed.push(format!("T-O1: {nb} faces"));
            }
        } else {
            println!("T-B1 {n}: the unchanged currents' |sum F_b| = {t_b1:.3e} A against {bound1:.3e} (must exceed)");
            if t_b1 <= bound1 {
                missed.push(format!("T-B1 {n}: {t_b1:e} <= {bound1:e}"));
            }
        }
        // T-B3
        let go = group_net_outflow(&geom, &f_old, &group, 2);
        let t_b3: Vec<bool> = (0..2).map(|g| go[g].abs() > tol_g[g] + ROUND_OFF * fsum).collect();
        println!(
            "T-B3 {n}: the unchanged currents' |G| = ({:.3e}, {:.3e}) A, miss ({}, {}) (a miss in one group needed)",
            go[0].abs(),
            go[1].abs(),
            t_b3[0],
            t_b3[1]
        );
        if !t_b3.iter().any(|&m| m) {
            missed.push(format!("T-B3 {n}: no group misses"));
        }
        // T-J1
        let q_old = joule_heat(&geom, &phi, &sigma_f, true);
        let p_old = boundary_power(&geom, &phi, &sigma_f, true);
        let t_old: f64 = q_old.iter().zip(vol).map(|(q, v)| q * v).sum();
        let tj = (t_old - p_old).abs();
        println!("T-J1 {n}: the unchanged |sum qV - P| = {tj:.3e} W against {bound_j:.3e} (must exceed)");
        if tj <= bound_j {
            missed.push(format!("T-J1 {n}: {tj:e} <= {bound_j:e}"));
        }
    }
    assert!(missed.is_empty(), "{missed:?}");
}

/// D1: the four F-256 keys and the table at their defaults change neither the settings hash nor a bit of the electric fields.
#[test]
fn d1_defaults_change_no_hash_and_no_bit() {
    let a = text("", "");
    let b = text(DEFAULT_KEYS, DEFAULT_TABLE);
    let (ha, hb) = (case(&a).tree.content_hash(), case(&b).tree.content_hash());
    let (ra, rb) = (run(&a), run(&b));
    let bits = |x: &[f64], y: &[f64]| x.iter().zip(y).filter(|(p, q)| p.to_bits() != q.to_bits()).count();
    let n = bits(&ra.phi, &rb.phi) + bits(&ra.q, &rb.q) + bits(&ra.t, &rb.t) + bits(&ra.j, &rb.j);
    println!(
        "D1: settings hash {ha:016x} / {hb:016x}; {n} values differ in a bit (limit 0); {} and {} iterations",
        ra.sim.iteration(),
        rb.sim.iteration()
    );
    assert_eq!(ha, hb, "D1 settings hash");
    assert_eq!(n, 0, "D1 bits");
}
