//! ADR 0049 checks F1-F4 (case-tree wiring of the VOF free-surface solver). The limits are those registered in the ADR before any
//! run; none was changed afterwards.

use rcfd_api::case::{Case, Diagnostic};
use rcfd_api::sim::Simulation;
use rcfd_fv::vof::{indicator_fraction, VofSettings, VofSolver};
use rcfd_fv::{FixedValue, FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::box_2d;
use std::sync::Arc;

const G: f64 = 9.81;
const A: f64 = 0.05715;

fn pool_text(extra_mp: &str, extra: &str) -> String {
    format!(
        r#"
[mesh]
generator = "box_2d"
nx = 10
ny = 20
lx = 0.5
ly = 1.0
[physics]
gravity = [0.0, -9.81, 0.0]
[multiphase]
enabled = true
density_1 = 1000.0
density_2 = 1.0
viscosity_1 = 1.0e-3
viscosity_2 = 1.8e-5
{extra_mp}
[multiphase.initial]
shape = "half_space"
normal = [0.0, 1.0, 0.0]
point = [0.0, 0.5, 0.0]
[solver]
algorithm = "piso"
convection = "upwind"
n_correctors = 3
dt = 1.0e-3
end_time = 0.5
{extra}
[boundaries.xmin]
velocity = "fixed"
[boundaries.xmax]
velocity = "fixed"
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
"#
    )
}

fn dam_text() -> String {
    format!(
        r#"
[mesh]
generator = "box_2d"
nx = 120
ny = 40
lx = {lx:.17e}
ly = {ly:.17e}
[physics]
gravity = [0.0, -9.81, 0.0]
[multiphase]
enabled = true
density_1 = 1000.0
density_2 = 1.0
viscosity_1 = 1.0e-3
viscosity_2 = 1.8e-5
[multiphase.initial]
shape = "box"
box_min = [-1.0, -1.0, -1.0]
box_max = [{a:.17e}, {h:.17e}, 1.0]
[solver]
algorithm = "piso"
convection = "upwind"
n_correctors = 3
dt = 1.5e-3
end_time = {t:.17e}
[boundaries.xmin]
velocity = "fixed"
[boundaries.xmax]
velocity = "fixed"
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
"#,
        lx = 12.0 * A,
        ly = 4.0 * A,
        a = A,
        h = 2.0 * A,
        t = 260.0 * 1.5e-3
    )
}

fn errors(text: &str) -> Vec<Diagnostic> {
    match Case::parse_toml(text) {
        Err(e) => e,
        Ok(c) => {
            let mut e = c.semantic_errors();
            e.extend(Simulation::new(&c).err().unwrap_or_default());
            e
        }
    }
}

fn sim(text: &str) -> Simulation {
    let case = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"));
    Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"))
}

#[test]
fn f1_located_errors() {
    // (description, text, expected path)
    let base = |mp: &str, extra: &str| pool_text(mp, extra);
    let mut rows: Vec<(&str, String, &str)> = vec![
        (
            "steady algorithm",
            base("", "").replace("\"piso\"", "\"simple\""),
            "solver.algorithm",
        ),
        (
            "coupled algorithm",
            base("", "").replace("\"piso\"", "\"coupled\""),
            "solver.algorithm",
        ),
        (
            "compressible algorithm",
            base("", "").replace("\"piso\"", "\"compressible\""),
            "solver.algorithm",
        ),
        (
            "non-constant density",
            base("", "") + "\n[fluid]\ndensity = \"ideal_gas\"\n",
            "fluid.density",
        ),
        (
            "turbulence",
            base("", "") + "\n[turbulence]\nmodel = \"smagorinsky\"\n",
            "turbulence.model",
        ),
        (
            "energy",
            base("", "") + "\n[energy]\nenabled = true\n",
            "energy.enabled",
        ),
        (
            "sources",
            base("", "") + "\n[sources.bed]\ntype = \"porous\"\nmodel = \"ergun\"\nporosity = 0.4\nparticle_diameter = 0.01\n",
            "sources",
        ),
        (
            "dynamic mesh",
            base("", "")
                + "\n[dynamic_mesh]\nenabled = true\nmoving_patches = [\"xmin\"]\ndeformation = \"smoothing\"\ndisplacement_expression_x = \"1 [m/s^2] * t^2\"\nfixed_patches = [\"xmax\"]\n",
            "dynamic_mesh.enabled",
        ),
        (
            "fractional step",
            base("", "time_advance = \"fractional_step\""),
            "solver.time_advance",
        ),
        (
            "statistics",
            base("", "") + "\n[statistics]\nenabled = true\nstart_time = 0.1\nfields = [\"ux\"]\n",
            "statistics.enabled",
        ),
        (
            "symmetry boundary",
            base("", "").replace("[boundaries.xmin]\nvelocity = \"fixed\"", "[boundaries.xmin]\nvelocity = \"symmetry\""),
            "boundaries.xmin.velocity",
        ),
        (
            "partial slip boundary",
            base("", "").replace(
                "[boundaries.xmin]\nvelocity = \"fixed\"",
                "[boundaries.xmin]\nvelocity = \"partial_slip\"\nslip_length = 0.1",
            ),
            "boundaries.xmin.velocity",
        ),
        (
            "marangoni boundary",
            base("", "").replace(
                "[boundaries.xmin]\nvelocity = \"fixed\"",
                "[boundaries.xmin]\nvelocity = \"marangoni\"",
            ),
            "boundaries.xmin.velocity",
        ),
        (
            "periodic mesh",
            base("", "").replace("ly = 1.0", "ly = 1.0\nperiodic_x = true"),
            "mesh.periodic_x",
        ),
        (
            "physics.rho",
            base("", "").replace("[physics]", "[physics]\nrho = 2.0"),
            "physics.rho",
        ),
        (
            "physics.nu",
            base("", "").replace("[physics]", "[physics]\nnu = 0.02"),
            "physics.nu",
        ),
        (
            "body force",
            base("", "").replace("[physics]", "[physics]\nbody_force = [0.0, 1.0, 0.0]"),
            "physics.body_force",
        ),
        (
            "unset density_1",
            base("", "").replace("density_1 = 1000.0\n", ""),
            "multiphase.density_1",
        ),
        (
            "unset viscosity_2",
            base("", "").replace("viscosity_2 = 1.8e-5\n", ""),
            "multiphase.viscosity_2",
        ),
        (
            "unset shape",
            base("", "").replace("shape = \"half_space\"", "shape = \"unset\""),
            "multiphase.initial.shape",
        ),
        (
            "sphere without radius",
            base("", "").replace(
                "shape = \"half_space\"\nnormal = [0.0, 1.0, 0.0]\npoint = [0.0, 0.5, 0.0]",
                "shape = \"sphere\"\ncentre = [0.2, 0.5, 0.0]",
            ),
            "multiphase.initial.radius",
        ),
        (
            "adaptive time step",
            base("", "max_courant = 0.5"),
            "solver.max_courant",
        ),
        (
            "bdf2",
            base("", "time_scheme = \"bdf2\""),
            "solver.time_scheme",
        ),
        (
            "pressure kind",
            base("", "").replace(
                "[boundaries.ymax]\nvelocity = \"fixed\"",
                "[boundaries.ymax]\nvelocity = \"fixed\"\npressure = \"freestream\"",
            ),
            "boundaries.ymax.pressure",
        ),
    ];
    // multiphase keys set while disabled
    let off = |extra_mp: &str| {
        let t = pool_text("", "").replace("enabled = true", "enabled = false");
        let t = t.replace(
            "density_1 = 1000.0\ndensity_2 = 1.0\nviscosity_1 = 1.0e-3\nviscosity_2 = 1.8e-5\n",
            "",
        );
        let t = t.replace(
            "shape = \"half_space\"\nnormal = [0.0, 1.0, 0.0]\npoint = [0.0, 0.5, 0.0]\n",
            "",
        );
        t.replace(
            "[multiphase]\nenabled = false\n",
            &format!("[multiphase]\nenabled = false\n{extra_mp}\n"),
        )
    };
    rows.push((
        "tension while disabled",
        off("surface_tension = 0.07"),
        "multiphase.surface_tension",
    ));
    rows.push((
        "density while disabled",
        off("density_1 = 1000.0"),
        "multiphase.density_1",
    ));
    rows.push((
        "compression while disabled",
        off("compression = 0.5"),
        "multiphase.compression",
    ));
    // ADR 0114: the opt-out keys are inert while disabled
    for (key, path) in [
        ("component_diagonal = \"averaged\"", "multiphase.component_diagonal"),
        ("viscous_stress = \"laplacian\"", "multiphase.viscous_stress"),
        ("curvature = \"csf\"", "multiphase.curvature"),
    ] {
        rows.push(("option key while disabled", off(key), path));
    }
    for (what, text, path) in rows {
        let e = errors(&text);
        let hit = e.iter().any(|d| d.path == path && !d.message.is_empty());
        println!(
            "F1 {what}: expected path {path}: {}",
            if hit { "ok" } else { "MISSING" }
        );
        assert!(hit, "{what}: no located error at {path}; got {e:?}");
    }
    // the valid pool case builds without a diagnostic
    assert!(errors(&pool_text("", "")).is_empty());
    // a disabled section with default values is silent
    let off_ok = pool_text("", "")
        .replace("enabled = true", "enabled = false")
        .replace(
            "density_1 = 1000.0\ndensity_2 = 1.0\nviscosity_1 = 1.0e-3\nviscosity_2 = 1.8e-5\n",
            "",
        )
        .replace(
            "shape = \"half_space\"\nnormal = [0.0, 1.0, 0.0]\npoint = [0.0, 0.5, 0.0]\n",
            "",
        );
    assert!(errors(&off_ok).is_empty(), "{:?}", errors(&off_ok));
}

fn lib_pool() -> VofSolver {
    let mesh = Arc::new(box_2d(10, 20, 0.5, 1.0, false, false).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    for p in ["xmin", "xmax", "ymin", "ymax"] {
        u.set_bc(&geom, p, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    let p_rgh = ScalarField::uniform("p_rgh", &geom, 0.0);
    let mut alpha = ScalarField::uniform("alpha", &geom, 0.0);
    alpha.values = indicator_fraction(&geom, 10, |x| x[1] <= 0.5);
    let mut s = VofSettings::new([1000.0, 1.0], [1e-3, 1.8e-5]);
    s.gravity = [0.0, -G, 0.0];
    s.n_correctors = 3;
    VofSolver::new(geom, u, p_rgh, alpha, s).unwrap()
}

fn max_diff(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max)
}

/// F2a / F3: the hydrostatic pool through the tree equals the library and stays at rest; the fields are available and bounded.
#[test]
fn f2_f3_hydrostatic_pool_through_the_tree() {
    let mut s = sim(&pool_text("", ""));
    let mut lib = lib_pool();
    let mut worst_u: f64 = 0.0;
    for _ in 0..500 {
        s.step();
        lib.step(1e-3).unwrap();
        worst_u = worst_u.max(
            s.velocity()
                .values
                .iter()
                .map(|v| v[0].abs().max(v[1].abs()))
                .fold(0.0, f64::max),
        );
    }
    let alpha = s.field("alpha").unwrap();
    let p_rgh = s.field("p_rgh").unwrap();
    let p = s.field("p").unwrap();
    let dp = max_diff(&p, &lib.p) / (1000.0 * G * 0.5);
    let da = max_diff(&alpha, &lib.transport.alpha.values);
    let (lo, hi) = alpha
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), &x| (l.min(x), h.max(x)));
    println!(
        "F2 pool: max|U| {worst_u:.3e} (limit 1e-10), tree vs library: p {dp:.3e}, alpha {da:.3e}; F3 alpha in [{lo:.3e}, {hi:.12}], {} p_rgh values",
        p_rgh.len()
    );
    assert!(worst_u <= 1e-10, "velocity {worst_u}");
    assert!(dp <= 1e-9 && da <= 1e-9, "tree differs from the library {dp} {da}");
    assert!(lo >= -1e-9 && hi <= 1.0 + 1e-9, "bounds");
    assert_eq!(p_rgh.len(), alpha.len());
    assert!(s
        .field("rho")
        .unwrap()
        .iter()
        .all(|&r| (1.0 - 1e-9..=1000.0 + 1e-9).contains(&r)));
}

/// The dam-break front of the library run (same helper as `rcfd-fv/tests/vof_dambreak.rs`).
fn front(alpha: &[f64]) -> f64 {
    let nx = 120usize;
    let dx = 12.0 * A / nx as f64;
    let mut i = nx - 1;
    while i > 0 && alpha[i] <= 0.5 {
        i -= 1;
    }
    let xc = (i as f64 + 0.5) * dx;
    if i + 1 < nx && alpha[i] > alpha[i + 1] {
        xc + dx * (alpha[i] - 0.5) / (alpha[i] - alpha[i + 1])
    } else {
        xc
    }
}

/// F2b: the dam break through the tree reproduces the library run; D1 is met through the tree as well.
#[test]
fn f2_dam_break_through_the_tree() {
    let mut s = sim(&dam_text());
    let mesh = Arc::new(box_2d(120, 40, 12.0 * A, 4.0 * A, false, false).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    for p in ["xmin", "xmax", "ymin", "ymax"] {
        u.set_bc(&geom, p, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    let p_rgh = ScalarField::uniform("p_rgh", &geom, 0.0);
    let mut alpha = ScalarField::uniform("alpha", &geom, 0.0);
    alpha.values = indicator_fraction(&geom, 10, |x| x[0] <= A && x[1] <= 2.0 * A);
    let mut st = VofSettings::new([1000.0, 1.0], [1e-3, 1.8e-5]);
    st.gravity = [0.0, -G, 0.0];
    st.n_correctors = 3;
    let mut lib = VofSolver::new(geom, u, p_rgh, alpha, st).unwrap();
    let h = 2.0 * A;
    let t_unit = (h / G).sqrt();
    let mut track = vec![(0.0, front(&s.field("alpha").unwrap()))];
    let mut worst_front = 0.0f64;
    let t0 = std::time::Instant::now();
    while !s.is_done() {
        s.step();
        lib.step(1.5e-3).unwrap();
        let ft = front(&s.field("alpha").unwrap());
        let fl = front(&lib.transport.alpha.values);
        worst_front = worst_front.max((ft - fl).abs() / fl);
        track.push((s.time() / t_unit, ft));
    }
    let da = max_diff(&s.field("alpha").unwrap(), &lib.transport.alpha.values);
    let du = s
        .velocity()
        .values
        .iter()
        .zip(&lib.u.values)
        .map(|(a, b)| (a[0] - b[0]).abs().max((a[1] - b[1]).abs()))
        .fold(0.0, f64::max);
    let at = |ts: f64| -> f64 {
        let k = track.iter().position(|p| p.0 >= ts).unwrap_or_else(|| {
            panic!(
                "run ended at t* = {:?} after {} steps: {:?}",
                track.last(),
                track.len() - 1,
                s.stop_reason()
            )
        });
        let (p0, p1) = (track[k - 1], track[k]);
        p0.1 + (p1.1 - p0.1) * (ts - p0.0) / (p1.0 - p0.0)
    };
    let v13 = (at(3.0) - at(1.0)) / h / 2.0;
    println!(
        "F2 dam break tree vs library: front relative difference {worst_front:.3e}, alpha {da:.3e}, U {du:.3e} (limit 1e-9); D1 through the tree v/sqrt(gH) {v13:.3} (limit 1.44..1.94); {} steps {:.1} s",
        track.len() - 1,
        t0.elapsed().as_secs_f64()
    );
    assert!(worst_front <= 1e-9 && da <= 1e-9 && du <= 1e-9 * (G * h).sqrt());
    assert!((1.69 * 0.85..=1.69 * 1.15).contains(&v13) && v13 < 2.0, "D1 {v13}");
}

/// F3 (outputs): the result files carry alpha and p_rgh.
#[test]
fn f3_outputs_carry_alpha_and_p_rgh() {
    use rcfd_api::runctl::{self, RunOptions};
    let dir = std::env::temp_dir().join(format!("rcfd_vof_run_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let t = pool_text("", "").replace("end_time = 0.5", "end_time = 0.01");
    let s = sim(&t);
    let m = std::sync::Mutex::new(s);
    let out = runctl::drive(&m, &RunOptions::default(), &|| false, &mut |_| {});
    assert_eq!(out.exit_code, 0, "{out:?}");
    let s = m.into_inner().unwrap();
    runctl::write_results(&s, &dir).unwrap();
    let vtu = std::fs::read_to_string(dir.join("solution.vtu")).unwrap();
    assert!(
        vtu.contains("Name=\"alpha\"") && vtu.contains("Name=\"p_rgh\""),
        "fields in vtu"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// F4: a case without multiphase runs exactly as before; an explicit `enabled = false` section changes no bit.
#[test]
fn f4_single_phase_unchanged() {
    let text = |mp: &str| {
        format!(
            r#"
[mesh]
generator = "box_2d"
nx = 12
ny = 12
lx = 1.0
ly = 1.0
[physics]
nu = 0.05
[solver]
algorithm = "piso"
dt = 0.01
end_time = 0.2
{mp}
[boundaries.xmin]
velocity = "fixed"
[boundaries.xmax]
velocity = "fixed"
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
"#
        )
    };
    let mut a = sim(&text(""));
    let mut b = sim(&format!("{}\n[multiphase]\nenabled = false\n", text("")));
    a.run(1000);
    b.run(1000);
    for n in ["ux", "uy", "p"] {
        let (x, y) = (a.field(n).unwrap(), b.field(n).unwrap());
        assert!(x.iter().zip(&y).all(|(p, q)| p.to_bits() == q.to_bits()), "{n}");
    }
    assert!(a.field("alpha").is_err(), "no alpha field without multiphase");
}

/// ADR 0114 (5): the dam break with surface tension, 20 steps: the tree without the option keys equals the library defaults, the
/// tree with the three opt-out keys equals the library with the four flags off (<= 1e-9, the F2 limit), and the two differ.
#[test]
fn f5_vof_option_keys_reach_the_library() {
    let lib = |off: bool| {
        let mesh = Arc::new(box_2d(120, 40, 12.0 * A, 4.0 * A, false, false).unwrap());
        let geom = Arc::new(FvGeometry::new(mesh).unwrap());
        let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
        for p in ["xmin", "xmax", "ymin", "ymax"] {
            u.set_bc(&geom, p, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
        }
        let p_rgh = ScalarField::uniform("p_rgh", &geom, 0.0);
        let mut alpha = ScalarField::uniform("alpha", &geom, 0.0);
        alpha.values = indicator_fraction(&geom, 10, |x| x[0] <= A && x[1] <= 2.0 * A);
        let mut st = VofSettings::new([1000.0, 1.0], [1e-3, 1.8e-5]);
        st.gravity = [0.0, -G, 0.0];
        st.n_correctors = 3;
        st.sigma = 0.07;
        if off {
            st.component_diagonal = false;
            st.full_viscous_stress = false;
            st.height_function = false;
            st.height_function_csf_fallback = false;
        }
        let mut s = VofSolver::new(geom, u, p_rgh, alpha, st).unwrap();
        for _ in 0..20 {
            s.step(1.5e-3).unwrap();
        }
        (s.transport.alpha.values.clone(), s.u.values.clone())
    };
    let tree = |keys: &str| {
        let t = dam_text().replace(
            "viscosity_2 = 1.8e-5
",
            &format!(
                "viscosity_2 = 1.8e-5
surface_tension = 0.07
{keys}
"
            ),
        );
        let mut s = sim(&t);
        for _ in 0..20 {
            s.step();
        }
        (s.field("alpha").unwrap(), s.velocity().values.clone())
    };
    let du = |a: &[[f64; 3]], b: &[[f64; 3]]| {
        a.iter()
            .zip(b)
            .map(|(x, y)| (x[0] - y[0]).abs().max((x[1] - y[1]).abs()))
            .fold(0.0, f64::max)
    };
    let (ld, lo) = (lib(false), lib(true));
    let td = tree("");
    let to = tree(
        "component_diagonal = \"averaged\"
viscous_stress = \"laplacian\"
curvature = \"csf\"",
    );
    let (e_d, e_o) = (
        max_diff(&td.0, &ld.0).max(du(&td.1, &ld.1)),
        max_diff(&to.0, &lo.0).max(du(&to.1, &lo.1)),
    );
    let gap = max_diff(&ld.0, &lo.0).max(du(&ld.1, &lo.1));
    println!("F5 tree vs library: defaults {e_d:.3e}, opt-outs {e_o:.3e} (limit 1e-9); defaults vs opt-outs {gap:.3e} (must be > 1e-9)");
    assert!(e_d <= 1e-9 && e_o <= 1e-9, "tree differs from the library: {e_d} {e_o}");
    assert!(gap > 1e-9, "the option keys have no effect: {gap}");
}
