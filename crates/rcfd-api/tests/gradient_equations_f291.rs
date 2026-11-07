//! F-291 (ADR 0204): the gradient options of F-256 on the momentum Laplacian, the pressure equation, the turbulence transport
//! and the electric potential (`[solver.gradient_equations]`). Limits are those registered in
//! docs/adr/0204-f291-registration-gradient-options-on-turbulence-momentum-pressure-and-the-electric-potential.md before this file
//! was written.
//!
//! P: the skewed `wedge_2d` mesh W reaches 20 degrees of non-orthogonality. F: the default runs are finite and run to their end.
//! G1: located errors (R1, R4 to R8) with a control per refusal. G2: the tree run equals the library run (`rcfd_fv`, `rcfd_turb`)
//! with the same options to 1e-12 of each field's scale. G3: every option set moves the tree result by more than 1e-9 of the
//! scale, so G2 cannot pass unwired; G3-K: the K value reaches the limiter. G4 (i): the keys and the table written at their
//! defaults change neither the settings hash nor a bit; G4 (iii): a new case writes no table; G4 (iv): the library runs with
//! `Some(GradientOptions::default())` equal those with `None` bit for bit.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::diffusion::{BoundaryGradient, GradientOptions};
use rcfd_fv::gradient::Limiter;
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::simple::{Coupling, SimpleSettings, SimpleSolver};
use rcfd_fv::{FixedValue, FvGeometry, ScalarField, VectorField, ZeroGradient};
use rcfd_linalg::SolveControl;
use rcfd_turb::kepsilon::WallFunction;
use rcfd_turb::model::WallOptions;
use rcfd_turb::sst::{SstVariant, SstWallMode};
use rcfd_turb::{KEpsilonVariant, MixingProfile, ModelContext, SLimiter, SaWallMode, TurbulenceModel, WallTreatment};
use std::sync::Arc;

/// G2 limit (relative to each compared field's scale).
const TREE_LIBRARY: f64 = 1e-12;
/// G3 limit: the smallest relative change an option set must make against the default run.
const TEETH: f64 = 1e-9;
/// G3-K limit: O5 (K 5) against O6 (K 0.5).
const TEETH_K: f64 = 1e-12;
/// Mesh W cells per direction.
const N: usize = 16;
/// Iterations of the steady cases, steps of the transient ones (dt 1/32 to end time 1/2, exact in binary), prescribed-flow
/// iterations of E1.
const ITERS: usize = 60;
const STEPS: usize = 16;
const DT: f64 = 0.03125;
const E_ITERS: usize = 20;
/// `physics.nu` of the flow cases.
const NU: f64 = 0.1;

/// One option set: name, the `[solver]` lines of the case, the library options.
struct Set {
    name: &'static str,
    keys: &'static str,
    opts: GradientOptions,
}

/// ADR 0190's option sets O1 to O6 and C2.
fn sets() -> Vec<Set> {
    let d = GradientOptions::default();
    let venk = |k| Some(Limiter::Venkatakrishnan(k));
    vec![
        Set {
            name: "O1",
            keys: "gradient_face_correction = true\n",
            opts: GradientOptions {
                face_correction: true,
                ..d
            },
        },
        Set {
            name: "O2",
            keys: "gradient_boundary = \"corrected\"\n",
            opts: GradientOptions {
                boundary: BoundaryGradient::Corrected,
                ..d
            },
        },
        Set {
            name: "O3",
            keys: "gradient_boundary = \"dropped\"\n",
            opts: GradientOptions {
                boundary: BoundaryGradient::Dropped,
                ..d
            },
        },
        Set {
            name: "O4",
            keys: "gradient_secondary_limiter = \"barth_jespersen\"\n",
            opts: GradientOptions {
                secondary_limiter: Some(Limiter::BarthJespersen),
                ..d
            },
        },
        Set {
            name: "O5",
            keys: "gradient_secondary_limiter = \"venkatakrishnan\"\n",
            opts: GradientOptions {
                secondary_limiter: venk(5.0),
                ..d
            },
        },
        Set {
            name: "O6",
            keys: "gradient_secondary_limiter = \"venkatakrishnan\"\ngradient_limiter_k = 0.5\n",
            opts: GradientOptions {
                secondary_limiter: venk(0.5),
                ..d
            },
        },
        c2(),
    ]
}

/// ADR 0016's option set C2: face correction, corrected boundary gradient, Venkatakrishnan with K = 5.
fn c2() -> Set {
    Set {
        name: "C2",
        keys: "gradient_face_correction = true\ngradient_boundary = \"corrected\"\ngradient_secondary_limiter = \"venkatakrishnan\"\n",
        opts: GradientOptions {
            face_correction: true,
            secondary_limiter: Some(Limiter::Venkatakrishnan(5.0)),
            boundary: BoundaryGradient::Corrected,
        },
    }
}

/// The four F-256 keys and the table with its four selectors written at their defaults (G4 (i)).
const DEFAULT_KEYS: &str = "gradient_face_correction = false\ngradient_secondary_limiter = \"none\"\ngradient_limiter_k = 5.0\ngradient_boundary = \"owner\"\n";
const DEFAULT_TABLE: &str = "momentum = false\npressure = false\nturbulence = false\nelectric_potential = false\n";

/// The boundary set-ups of the cases.
#[derive(Clone, Copy, PartialEq)]
enum Flow {
    /// ADR 0190's S1 lid cavity without the scalar; `Some(role)`: the lid's `turbulence` role (T1 to T3: `wall`; T2b, ADR 0204
    /// amendment 1: `zero_gradient`).
    Cavity(Option<&'static str>),
    /// The channel of P1 to P3.
    Channel,
    /// ADR 0190's E1 (prescribed zero flow, steady energy); `true`: with the electric potential of E1.
    Frozen(bool),
}

/// A case text: the flow, the run lines, extra `[solver]` lines, extra `[physics]` lines, extra top-level tables and the lines
/// of `[solver.gradient_equations]` (none: no table).
fn text(flow: Flow, run: &str, solver: &str, physics: &str, tables: &str, selectors: &str) -> String {
    let table = if selectors.is_empty() {
        String::new()
    } else {
        format!("[solver.gradient_equations]\n{selectors}")
    };
    let mesh = format!(
        "[mesh]\ngenerator = \"wedge_2d\"\nnx = {N}\nny = {N}\nlx = 1.0\nly = 1.0\nramp_start = 0.0\nramp_angle = 30.0\n"
    );
    match flow {
        Flow::Cavity(role) => {
            let lid = role.map(|r| format!("turbulence = \"{r}\"\n")).unwrap_or_default();
            format!(
                r#"title = "f291 cavity"
{mesh}[physics]
nu = {NU}
{physics}[boundaries.xmin]
velocity = "fixed"
[boundaries.ymin]
velocity = "fixed"
[boundaries.xmax]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
{lid}{tables}[solver]
convection = "linear_upwind"
{run}{solver}{table}"#
            )
        }
        Flow::Channel => format!(
            r#"title = "f291 channel"
{mesh}[physics]
nu = {NU}
{physics}[boundaries.xmin]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.xmax]
velocity = "zero_gradient"
pressure = "fixed"
pressure_value = 0.0
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
{tables}[solver]
convection = "linear_upwind"
non_orthogonal_correctors = 1
{run}{solver}{table}"#
        ),
        Flow::Frozen(electric) => {
            let (el, xmin, ymin) = if electric {
                (
                    "[electric]\nenabled = true\nconductivity = 1.0\n",
                    "electric = \"potential\"\nelectric_potential = 1.0\n",
                    "electric = \"potential\"\nelectric_potential = 0.0\n",
                )
            } else {
                ("", "", "")
            };
            format!(
                r#"title = "f291 prescribed flow"
{mesh}[physics]
rho = 1.0
cp = 1.0
conductivity = 0.1
{physics}[energy]
enabled = true
relax = 0.9
[initial]
temperature = 0.5
{el}[boundaries.xmin]
velocity = "fixed"
temperature = "fixed"
temperature_value = 1.0
{xmin}[boundaries.ymin]
velocity = "fixed"
temperature = "fixed"
temperature_value = 0.0
{ymin}[boundaries.xmax]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
{tables}[solver]
convection = "linear_upwind"
frozen_flow = true
{run}{solver}{table}"#
            )
        }
    }
}

/// Run lines: steady (`iters` iterations) or the transient pimple run.
fn steady(iters: usize) -> String {
    format!("algorithm = \"simplec\"\nmax_iterations = {iters}\ntolerance_u = 1e-14\ntolerance_p = 1e-14\n")
}
fn transient() -> String {
    format!("algorithm = \"pimple\"\ndt = {DT}\nend_time = 0.5\ntime_scheme = \"euler\"\n")
}

/// The turbulence tables of T1 to T3.
const SST: &str =
    "[turbulence]\nmodel = \"k_omega_sst\"\nconvection = \"linear_upwind\"\nk_initial = 0.01\nomega_initial = 1.0\n";
const KE: &str =
    "[turbulence]\nmodel = \"k_epsilon\"\nconvection = \"linear_upwind\"\nk_initial = 0.01\nepsilon_initial = 9e-4\n";
const SA: &str = "[turbulence]\nmodel = \"spalart_allmaras\"\nconvection = \"linear_upwind\"\n";

/// The cases of ADR 0204.
#[derive(Clone, Copy, PartialEq, Debug)]
enum K {
    M1,
    M2,
    P1,
    P2,
    P3,
    T1,
    T2,
    /// T2 with the lid's turbulence role `zero_gradient` (ADR 0204 amendment 1).
    T2b,
    T3,
    /// T3 with the lid's turbulence role `zero_gradient` (ADR 0204 amendment 2).
    T3b,
    E1,
}

impl K {
    fn selector(self) -> &'static str {
        match self {
            K::M1 | K::M2 => "momentum",
            K::P1 | K::P2 | K::P3 => "pressure",
            K::T1 | K::T2 | K::T2b | K::T3 | K::T3b => "turbulence",
            K::E1 => "electric_potential",
        }
    }
    fn fields(self) -> &'static [&'static str] {
        match self {
            K::T1 => &["ux", "uy", "p", "k", "omega"],
            K::T2 | K::T2b => &["ux", "uy", "p", "k", "epsilon"],
            K::T3 | K::T3b => &["ux", "uy", "p", "nuTilda"],
            K::E1 => &["electric_potential"],
            _ => &["ux", "uy", "p"],
        }
    }
    fn steps(self) -> usize {
        match self {
            K::M2 | K::P3 => STEPS,
            K::E1 => E_ITERS,
            _ => ITERS,
        }
    }
    /// The case text with extra `[solver]` lines and the table lines (none: no table).
    fn text(self, solver: &str, selectors: &str) -> String {
        let s = steady(ITERS);
        match self {
            K::M1 => text(Flow::Cavity(None), &s, solver, "", "", selectors),
            K::M2 => text(Flow::Cavity(None), &transient(), solver, "", "", selectors),
            K::P1 => text(Flow::Channel, &s, solver, "", "", selectors),
            K::P2 => text(
                Flow::Channel,
                &s,
                &format!("simplec_non_orthogonal_compensation = true\n{solver}"),
                "",
                "",
                selectors,
            ),
            K::P3 => text(Flow::Channel, &transient(), solver, "", "", selectors),
            K::T1 => text(Flow::Cavity(Some("wall")), &s, solver, "", SST, selectors),
            K::T2 => text(Flow::Cavity(Some("wall")), &s, solver, "", KE, selectors),
            K::T2b => text(Flow::Cavity(Some("zero_gradient")), &s, solver, "", KE, selectors),
            K::T3 => text(Flow::Cavity(Some("wall")), &s, solver, "", SA, selectors),
            K::T3b => text(Flow::Cavity(Some("zero_gradient")), &s, solver, "", SA, selectors),
            K::E1 => text(Flow::Frozen(true), &steady(E_ITERS), solver, "", "", selectors),
        }
    }
    /// The case with a set's keys and the case's selector.
    fn with(self, keys: &str) -> String {
        self.text(keys, &format!("{} = true\n", self.selector()))
    }
}

fn case(text: &str) -> Case {
    Case::parse_toml(text).unwrap_or_else(|d| panic!("case does not parse: {d:?}\n{text}"))
}

/// Run a case through the tree to its end; returns the simulation, the compared fields and why the run did not end as
/// registered (a failure, or another count of iterations or steps), if it did not.
fn run(k: K, text: &str) -> (Simulation, Vec<Vec<f64>>, Option<String>) {
    let mut s = Simulation::new(&case(text)).unwrap_or_else(|d| panic!("not built: {d:?}"));
    let n = s.run(10 * ITERS);
    let why = match s.failure() {
        Some(f) => Some(format!("{f:?}")),
        None if n != k.steps() => Some(format!("{n} iterations or steps, not {}", k.steps())),
        None => None,
    };
    let f = k.fields().iter().map(|name| s.field(name).unwrap()).collect();
    (s, f, why)
}

/// [`run`] of a run that must end as registered (F).
fn tree(k: K, text: &str) -> (Simulation, Vec<Vec<f64>>) {
    let (s, f, why) = run(k, text);
    assert!(why.is_none(), "F {k:?} set-up: {why:?}");
    (s, f)
}

fn finite(f: &[Vec<f64>]) -> bool {
    f.iter().all(|v| v.iter().all(|x| x.is_finite()))
}

/// The scale `S_phi` of every compared field (largest absolute value of the default run).
fn scales(f: &[Vec<f64>]) -> Vec<f64> {
    f.iter()
        .map(|v| v.iter().fold(0.0_f64, |m, x| m.max(x.abs())))
        .collect()
}

/// `d(a, b)`: the largest over the compared fields of `max_c abs(a - b) / S`.
fn d(a: &[Vec<f64>], b: &[Vec<f64>], s: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len());
    a.iter()
        .zip(b)
        .zip(s)
        .map(|((x, y), s)| {
            assert_eq!(x.len(), y.len());
            x.iter().zip(y).map(|(p, q)| (p - q).abs()).fold(0.0, f64::max) / s
        })
        .fold(0.0, f64::max)
}

/// Bits that differ over the compared fields.
fn bits(a: &[Vec<f64>], b: &[Vec<f64>]) -> usize {
    a.iter()
        .zip(b)
        .map(|(x, y)| x.iter().zip(y).filter(|(p, q)| p.to_bits() != q.to_bits()).count())
        .sum()
}

// ------------------------------------------------------------------------------------------------ library runs

/// The velocity and pressure of the cavity (`channel` false) or the channel, with the tree's boundary conditions.
fn flow_fields(geom: &Arc<FvGeometry>, channel: bool) -> (VectorField, ScalarField) {
    let mut u = VectorField::uniform("U", geom, [0.0; 3]);
    let mut p = ScalarField::uniform("p", geom, 0.0);
    if channel {
        u.set_bc(geom, "xmin", Box::new(FixedValue::vector([1.0, 0.0, 0.0])))
            .unwrap();
        u.set_bc(geom, "xmax", Box::new(ZeroGradient)).unwrap();
        for patch in ["ymin", "ymax"] {
            u.set_bc(geom, patch, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
        }
        p.set_bc(geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    } else {
        for patch in ["xmin", "ymin", "xmax"] {
            u.set_bc(geom, patch, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
        }
        u.set_bc(geom, "ymax", Box::new(FixedValue::vector([1.0, 0.0, 0.0])))
            .unwrap();
    }
    (u, p)
}

fn velocity_pressure(u: &VectorField, p: &ScalarField) -> Vec<Vec<f64>> {
    vec![
        u.values.iter().map(|x| x[0]).collect(),
        u.values.iter().map(|x| x[1]).collect(),
        p.values.clone(),
    ]
}

/// A `SimpleSolver` set up as the tree sets up M1 (`channel` false) or P1 and P2 (`correctors` 1, `compensation` for P2).
fn simple_solver(geom: &Arc<FvGeometry>, channel: bool, compensation: bool) -> SimpleSolver {
    let (u, p) = flow_fields(geom, channel);
    let mut s = SimpleSettings::new(NU, Coupling::Simplec);
    s.convection = ConvScheme::LinearUpwind;
    s.alpha_u = 0.9;
    s.alpha_p = 1.0;
    s.non_orth_correctors = usize::from(channel);
    s.simplec_non_orth_compensation = compensation;
    SimpleSolver::new(geom.clone(), u, p, s)
}

/// A `PimpleSolver` set up as the tree sets up M2 (`channel` false) or P3.
fn pimple_solver(geom: &Arc<FvGeometry>, channel: bool) -> PimpleSolver {
    let (u, p) = flow_fields(geom, channel);
    let mut s = PimpleSettings::pimple(NU, 1);
    s.convection = ConvScheme::LinearUpwind;
    s.n_correctors = 2;
    s.non_orth_correctors = usize::from(channel);
    s.alpha_u = 0.9;
    s.alpha_p = 1.0;
    s.ddt_correction = true;
    s.component_diagonal = false;
    s.time_scheme = TimeScheme::ImplicitEuler;
    PimpleSolver::new(geom.clone(), u, p, s)
}

/// The turbulence model of T1 to T3 built as `TurbState::new` builds it from the case, with the options.
fn turbulence_model(geom: &Arc<FvGeometry>, k: K, opts: Option<GradientOptions>) -> Box<dyn TurbulenceModel> {
    let (key, k0, eps0, omega0) = match k {
        K::T1 => ("k_omega_sst", 0.01, 0.0, 1.0),
        K::T2 | K::T2b => ("k_epsilon", 0.01, 9e-4, 0.0),
        _ => ("spalart_allmaras", 0.0, 0.0, 0.0),
    };
    // the wall patches as `TurbState::new` lists them (the sorted boundary names with the wall role)
    let walls: &[&str] = if matches!(k, K::T2b | K::T3b) {
        &["xmax", "xmin", "ymin"]
    } else {
        &["xmax", "xmin", "ymax", "ymin"]
    };
    let ctx = ModelContext {
        geom: geom.clone(),
        nu: NU,
        walls: walls.iter().map(|w| w.to_string()).collect(),
        k0,
        eps0,
        omega0,
        nutilde0: 3.0 * NU,
        variant: KEpsilonVariant::Standard,
        wall_treatment: WallTreatment::Smooth,
        sa_wall_mode: SaWallMode::Auto,
        sa_limiter: SLimiter::Allmaras,
        sa_negative: true,
        kappa: 0.41,
        e_wall: 9.8,
        convection: ConvScheme::LinearUpwind,
        relax_turb: 0.7,
        relax_nut: 0.5,
        wall_damping: false,
        length_scale: 0.0,
        mixing_profile: MixingProfile::Linear,
        time_accurate: false,
    };
    let mut m = rcfd_turb::create_model(key, &ctx).unwrap();
    m.apply_wall_options(&WallOptions {
        wall_function: WallFunction::Standard,
        roughness_height: 0.0,
        roughness_constant: 0.0,
        wall_omega: SstWallMode::Automatic,
        sst_variant: SstVariant::Sst,
    })
    .unwrap();
    if matches!(k, K::T3 | K::T3b) {
        for c in m.coefficients() {
            m.set_coefficient(c.name, c.default).unwrap();
        }
        for (name, value) in [("des", "off"), ("des_delta", "max_extent"), ("c_des", "0.65")] {
            m.set_option(name, value).unwrap();
        }
    }
    m.set_gradient_options(opts).unwrap();
    m
}

/// The library run of a case with the options on its equation.
fn library(geom: &Arc<FvGeometry>, k: K, opts: Option<GradientOptions>) -> Vec<Vec<f64>> {
    match k {
        K::M1 | K::P1 | K::P2 => {
            let mut sol = simple_solver(geom, k != K::M1, k == K::P2);
            if k == K::M1 {
                sol.gradient_momentum = opts;
            } else {
                sol.gradient_pressure = opts;
            }
            for _ in 0..ITERS {
                sol.iterate();
            }
            velocity_pressure(&sol.u, &sol.p)
        }
        K::M2 | K::P3 => {
            let mut sol = pimple_solver(geom, k == K::P3);
            if k == K::M2 {
                sol.settings.gradient_momentum = opts;
            } else {
                sol.settings.gradient_pressure = opts;
            }
            for _ in 0..STEPS {
                sol.step(DT);
            }
            velocity_pressure(&sol.u, &sol.p)
        }
        K::T1 | K::T2 | K::T2b | K::T3 | K::T3b => {
            let mut model = turbulence_model(geom, k, opts);
            let mut sol = simple_solver(geom, false, false);
            sol.settings.h_from_solution = true;
            for _ in 0..ITERS {
                model.update(&sol.u, &sol.phi);
                sol.nu_face = Some(model.nu_face());
                sol.iterate();
            }
            let mut out = velocity_pressure(&sol.u, &sol.p);
            for name in &k.fields()[3..] {
                let f = model.scalars().into_iter().find(|(n, _)| n == name).unwrap().1;
                out.push(f.values.clone());
            }
            out
        }
        K::E1 => {
            let mut phi = ScalarField::uniform("phi", geom, 0.0);
            phi.set_bc(geom, "xmin", Box::new(FixedValue::scalar(1.0))).unwrap();
            phi.set_bc(geom, "ymin", Box::new(FixedValue::scalar(0.0))).unwrap();
            let sigma = vec![1.0; geom.n_cells()];
            let ctl = SolveControl {
                tol_abs: 1e-300,
                tol_rel: 1e-13,
                max_iter: 5000,
            };
            for _ in 0..E_ITERS {
                rcfd_fv::electric::solve_opt(geom, &mut phi, &sigma, &ctl, 20, opts);
            }
            vec![phi.values]
        }
    }
}

// ------------------------------------------------------------------------------------------------ P, F, G2, G3

/// P: the precondition on mesh W.
fn precondition(s: &Simulation) {
    let q = rcfd_mesh::quality::check_mesh(&s.geom.mesh);
    println!(
        "P: mesh W max non-orthogonality {:.3} degrees (at least 20)",
        q.max_non_orthogonality
    );
    assert!(q.max_non_orthogonality >= 20.0, "P set-up: {}", q.max_non_orthogonality);
}

/// F, G2 and G3 for one case and its sets; returns the tree fields of each set.
fn g2_g3(k: K, sets: &[Set]) -> Vec<(&'static str, Vec<Vec<f64>>)> {
    let (s0, default) = tree(k, &k.text("", ""));
    precondition(&s0);
    let sc = scales(&default);
    println!("F {k:?}: fields {:?}, scales {sc:?}", k.fields());
    assert!(
        finite(&default) && sc.iter().all(|&x| x > 0.0),
        "F {k:?} set-up: the default run"
    );
    let lib0 = library(&s0.geom, k, None);
    let d0 = d(&default, &lib0, &sc);
    println!("G2 {k:?} control: default tree vs library without options {d0:.3e} (limit {TREE_LIBRARY:e})");
    let mut missed = Vec::new();
    if d0 > TREE_LIBRARY {
        missed.push(format!("G2 {k:?} control: {d0:e}"));
    }
    let mut out = Vec::new();
    for set in sets {
        let (s, f, why) = run(k, &k.with(set.keys));
        if why.is_some() || !finite(&f) {
            println!(
                "F {k:?} {}: the tree run did not end finite ({why:?}; G2, G3 NOT MET)",
                set.name
            );
            missed.push(format!("F {k:?} {}: {why:?}, finite {}", set.name, finite(&f)));
            continue;
        }
        let lib = library(&s.geom, k, Some(set.opts));
        let (dl, teeth) = (d(&f, &lib, &sc), d(&f, &default, &sc));
        println!(
            "G2 {k:?} {}: tree vs library {dl:.3e} (limit {TREE_LIBRARY:e}); G3 change against the default {teeth:.3e} (limit > {TEETH:e})",
            set.name
        );
        if dl > TREE_LIBRARY {
            missed.push(format!("G2 {k:?} {}: {dl:e}", set.name));
        }
        if teeth <= TEETH {
            missed.push(format!("G3 {k:?} {}: {teeth:e}", set.name));
        }
        out.push((set.name, f));
    }
    if let (Some(a), Some(b)) = (
        out.iter().find(|(n, _)| *n == "O5"),
        out.iter().find(|(n, _)| *n == "O6"),
    ) {
        let dk = d(&a.1, &b.1, &sc);
        println!("G3-K {k:?}: O5 (K 5) vs O6 (K 0.5) {dk:.3e} (limit > {TEETH_K:e})");
        if dk <= TEETH_K {
            missed.push(format!("G3-K {k:?}: {dk:e}"));
        }
    }
    assert!(missed.is_empty(), "{missed:?}");
    out
}

#[test]
fn g2_g3_m1_steady_momentum_all_sets() {
    g2_g3(K::M1, &sets());
}

#[test]
fn g2_g3_m2_transient_momentum_c2() {
    g2_g3(K::M2, &[c2()]);
}

#[test]
fn g2_g3_p1_steady_pressure_all_sets() {
    g2_g3(K::P1, &sets());
}

#[test]
fn g2_g3_p2_p3_compensation_and_transient_pressure_c2() {
    g2_g3(K::P2, &[c2()]);
    g2_g3(K::P3, &[c2()]);
}

#[test]
fn g2_g3_t1_sst_all_sets() {
    g2_g3(K::T1, &sets());
}

/// T2 and T3 as registered: kept as the record of T2's set-up defect (ADR 0204 amendment 1, finding 1).
#[test]
#[ignore = "ADR 0204 amendment 1 finding 1: T2 does not parse (the k-epsilon wall functions refuse the moving lid as a wall), so T3 never ran here; g2_g3_t2b_k_epsilon_c2 and g2_g3_t3_spalart_allmaras_c2 are the corrected tests"]
fn g2_g3_t2_t3_k_epsilon_and_spalart_allmaras_c2_setup_defect() {
    g2_g3(K::T2, &[c2()]);
    g2_g3(K::T3, &[c2()]);
}

/// T2b (ADR 0204 amendment 1): T2 with the lid's turbulence role `zero_gradient`.
#[test]
fn g2_g3_t2b_k_epsilon_c2() {
    g2_g3(K::T2b, &[c2()]);
}

/// T3 as registered, in a test of its own (ADR 0204 amendment 1): kept as the record of T3's set-up defect (amendment 2).
#[test]
#[ignore = "ADR 0204 amendment 2 finding 3: T3 does not parse (the Spalart-Allmaras wall treatment refuses the moving lid as a wall); g2_g3_t3b_spalart_allmaras_c2 is the corrected test"]
fn g2_g3_t3_spalart_allmaras_c2_setup_defect() {
    g2_g3(K::T3, &[c2()]);
}

/// T3b (ADR 0204 amendment 2): T3 with the lid's turbulence role `zero_gradient`.
#[test]
fn g2_g3_t3b_spalart_allmaras_c2() {
    g2_g3(K::T3b, &[c2()]);
}

/// P2-all (ADR 0204 amendment 1, supplementary, not binding): the channel with the SIMPLEC compensation and every set.
#[test]
fn g2_g3_p2_all_sets_supplementary() {
    g2_g3(K::P2, &sets());
}

#[test]
fn g2_g3_e1_electric_potential_all_sets() {
    g2_g3(K::E1, &sets());
}

// ------------------------------------------------------------------------------------------------ G4

/// The cases of G4 (i), with T2b and T3b in place of T2 and T3 (ADR 0204 amendments 1 and 2).
const ALL: [K; 9] = [K::M1, K::M2, K::P1, K::P2, K::P3, K::T1, K::T2b, K::T3b, K::E1];

/// G4 (i): the four F-256 keys and the table with its selectors written at their defaults leave the settings hash and every
/// bit of the result unchanged.
#[test]
fn g4_i_keys_and_table_at_their_defaults_change_nothing() {
    for k in ALL {
        let (a, b) = (k.text("", ""), k.text(DEFAULT_KEYS, DEFAULT_TABLE));
        let (ha, hb) = (case(&a).tree.content_hash(), case(&b).tree.content_hash());
        assert_eq!(ha, hb, "G4 (i) {k:?}: settings hash");
        let (_, fa) = tree(k, &a);
        let (_, fb) = tree(k, &b);
        let n = bits(&fa, &fb);
        println!(
            "G4 (i) {k:?}: settings hash {ha:016x} both; {n} of {} values differ in a bit (limit 0)",
            fa.iter().map(Vec::len).sum::<usize>()
        );
        assert_eq!(n, 0, "G4 (i) {k:?}");
    }
}

/// G4 (iii), the part of this file: a new case writes no `gradient_equations` table (the H1 record is checked by
/// `case_hashes_w48u.rs`, unedited).
#[test]
fn g4_iii_a_new_case_writes_no_table() {
    let c = Case::new();
    let (t, j) = (c.to_toml(), c.to_json_string());
    println!(
        "G4 (iii): to_toml {} bytes, to_json_string {} bytes, gradient_equations in either: {}",
        t.len(),
        j.len(),
        t.contains("gradient_equations") || j.contains("gradient_equations")
    );
    assert!(!t.contains("gradient_equations") && !j.contains("gradient_equations"));
}

/// G4 (iv): every library run with `Some(GradientOptions::default())` equals the run with `None` bit for bit.
#[test]
fn g4_iv_library_default_options_equal_no_options() {
    // T2b and T3b in place of T2 and T3 (ADR 0204 amendments 1 and 2)
    for k in [K::M1, K::M2, K::P2, K::P3, K::T1, K::T2b, K::T3b, K::E1] {
        let (s, _) = tree(k, &k.text("", ""));
        let a = library(&s.geom, k, None);
        let b = library(&s.geom, k, Some(GradientOptions::default()));
        let n = bits(&a, &b);
        println!(
            "G4 (iv) {k:?}: {n} of {} values differ in a bit (limit 0)",
            a.iter().map(Vec::len).sum::<usize>()
        );
        assert_eq!(n, 0, "G4 (iv) {k:?}");
    }
}

// ------------------------------------------------------------------------------------------------ G1

/// The path of the first diagnostic of a refused case (parse or build), or "accepted".
fn refused_at(text: &str) -> String {
    match Case::parse_toml(text) {
        Err(d) => {
            println!("  {}: {}", d[0].path, d[0].message);
            d.first().map(|x| x.path.clone()).unwrap_or_default()
        }
        Ok(c) => match Simulation::new(&c) {
            Err(d) => {
                println!("  {}: {}", d[0].path, d[0].message);
                d[0].path.clone()
            }
            Ok(_) => "accepted".into(),
        },
    }
}

#[test]
fn g1_located_errors() {
    const O1: &str = "gradient_face_correction = true\n";
    const O5: &str = "gradient_secondary_limiter = \"venkatakrishnan\"\n";
    let sel = |p: &str| format!("solver.gradient_equations.{p}");
    let m1 = |run: &str, solver: &str, tables: &str, selectors: &str| {
        text(Flow::Cavity(None), run, solver, "", tables, selectors)
    };
    let s = steady(ITERS);
    let piso_fs = format!("algorithm = \"piso\"\ntime_advance = \"fractional_step\"\ndt = {DT}\nend_time = 0.5\n");
    let coupled = format!("algorithm = \"coupled\"\nmax_iterations = {ITERS}\n");
    let ml = "[turbulence]\nmodel = \"mixing_length\"\nlength_scale = 0.5\n";
    let bfw = "pressure_interpolation = \"body_force_weighted\"\n";
    let body = "body_force = [0.0, -10.0, 0.0]\n";
    let e1_plain = |solver: &str, selectors: &str| text(Flow::Frozen(false), &s, solver, "", "", selectors);
    let mom = "momentum = true\n";
    let checks: Vec<(&str, String, String)> = vec![
        // R1: values the schema refuses
        (
            "R1 not a boolean",
            m1(&s, "", "", "momentum = \"yes\"\n"),
            sel("momentum"),
        ),
        ("R1 unknown selector", m1(&s, "", "", "energy = true\n"), sel("energy")),
        // R5: a selector without options
        ("R5 no option", m1(&s, "", "", mom), sel("momentum")),
        (
            "R5 K alone",
            m1(&s, "gradient_limiter_k = 1.0\n", "", mom),
            sel("momentum"),
        ),
        // R6: the equation is not solved by a kernel that takes the options
        ("R6 prescribed flow", e1_plain(O1, mom), sel("momentum")),
        ("R6 coupled", m1(&coupled, O1, "", mom), sel("momentum")),
        ("R6 fractional step", m1(&piso_fs, O1, "", mom), sel("momentum")),
        (
            "R6 laminar",
            m1(&s, O1, "", "momentum = true\nturbulence = true\n"),
            sel("turbulence"),
        ),
        (
            "R6-T mixing length",
            m1(&s, O1, ml, "momentum = true\nturbulence = true\n"),
            sel("turbulence"),
        ),
        (
            "R6 electric off",
            m1(&s, O1, "", "momentum = true\nelectric_potential = true\n"),
            sel("electric_potential"),
        ),
        // R7: no secondary flux
        (
            "R7 momentum upwind",
            m1(&s, &format!("convection = \"upwind\"\n{O1}"), "", mom)
                .replace("convection = \"linear_upwind\"\nalgorithm", "algorithm"),
            sel("momentum"),
        ),
        (
            "R7 pressure without correctors",
            text(Flow::Channel, &s, O1, "", "", "pressure = true\n")
                .replace("non_orthogonal_correctors = 1\n", "non_orthogonal_correctors = 0\n"),
            sel("pressure"),
        ),
        (
            "R7 turbulence upwind",
            K::T1.with(O1).replace(
                "convection = \"linear_upwind\"\nk_initial",
                "convection = \"upwind\"\nk_initial",
            ),
            sel("turbulence"),
        ),
        // R8: the pressure options with body_force_weighted
        (
            "R8 body_force_weighted",
            text(Flow::Channel, &s, &format!("{bfw}{O1}"), body, "", "pressure = true\n"),
            sel("pressure"),
        ),
        // R4: K without its limiter, with a selector
        (
            "R4 K without the limiter",
            m1(&s, &format!("{O1}gradient_limiter_k = 1.0\n"), "", mom),
            "solver.gradient_limiter_k".into(),
        ),
        // controls: accepted
        ("control momentum O1", m1(&s, O1, "", mom), "accepted".into()),
        (
            "control prescribed flow O1 (no table)",
            e1_plain(O1, ""),
            "accepted".into(),
        ),
        ("control coupled", m1(&coupled, "", "", ""), "accepted".into()),
        ("control fractional step", m1(&piso_fs, "", "", ""), "accepted".into()),
        (
            "control mixing length, momentum O1",
            m1(&s, O1, ml, mom),
            "accepted".into(),
        ),
        (
            "control upwind with a corrector",
            m1(
                &s,
                &format!("convection = \"upwind\"\nnon_orthogonal_correctors = 1\n{O1}"),
                "",
                mom,
            )
            .replace("convection = \"linear_upwind\"\nalgorithm", "algorithm"),
            "accepted".into(),
        ),
        (
            "control pressure O1",
            text(Flow::Channel, &s, O1, "", "", "pressure = true\n"),
            "accepted".into(),
        ),
        ("control turbulence O1", K::T1.with(O1), "accepted".into()),
        (
            "control body_force_weighted, momentum O1",
            text(Flow::Channel, &s, &format!("{bfw}{O1}"), body, "", mom),
            "accepted".into(),
        ),
        (
            "control K with its limiter",
            m1(&s, &format!("{O5}gradient_limiter_k = 1.0\n"), "", mom),
            "accepted".into(),
        ),
    ];
    let mut missed = Vec::new();
    for (what, t, want) in checks {
        let got = refused_at(&t);
        println!("G1 {what}: {got} (want {want})");
        if got != want {
            missed.push(format!("{what}: {got} (want {want})"));
        }
    }
    assert!(missed.is_empty(), "G1: {missed:?}");
}
