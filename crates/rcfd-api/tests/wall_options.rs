//! Near-wall options in the settings tree (F-203, F-043, ADR 0015 check C5): validation with located errors and the
//! apply functions on library models.
//!
//! The build path applies the options through `TurbulenceModel::apply_wall_options` (hook installed in wave 13);
//! `c5_tree_run_equals_library_run` is the registered equality check. The earlier test that recorded the rejection
//! state was replaced, as ADR 0015 prescribed.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_api::wall_opts::{apply_kepsilon, apply_sst, WallOptions, WallOptionsFromCase, BUILD_HOOK_INSTALLED};
use rcfd_turb::kepsilon::WallFunction;
use rcfd_turb::model::{KEpsilonModel, SstModel};
use rcfd_turb::sst::{SstVariant, SstWallMode};
use rcfd_turb::{KEpsilon, KEpsilonVariant, KOmegaSst};
use rcfd_turb::{ModelContext, TurbulenceModel};
use std::sync::Arc;

const SST_RAW: &str = include_str!("../../../cases/channel/sst_channel.toml");
const KE_RAW: &str = include_str!("../../../cases/channel/kepsilon_channel.toml");

fn lf(s: &str) -> String {
    s.replace("\r\n", "\n")
}

fn errors(text: &str) -> Vec<(String, String)> {
    match Case::parse_toml(text) {
        Ok(_) => Vec::new(),
        Err(e) => e.into_iter().map(|d| (d.path, d.message)).collect(),
    }
}

fn with_sst(extra: &str) -> String {
    lf(SST_RAW).replace("omega_initial = 39.5", &format!("omega_initial = 39.5\n{extra}"))
}

fn with_ke(extra: &str) -> String {
    let base = if extra.contains("wall_function") {
        lf(KE_RAW).replace("wall_function = \"standard\"\n", "")
    } else {
        lf(KE_RAW)
    };
    base.replace("epsilon_initial = 0.36", &format!("epsilon_initial = 0.36\n{extra}"))
}

#[test]
fn defaults_are_accepted() {
    assert!(errors(&lf(SST_RAW)).is_empty());
    assert!(errors(&lf(KE_RAW)).is_empty());
    let c = Case::parse_toml(&lf(KE_RAW)).unwrap();
    let o = WallOptions::from_case(&c);
    assert_eq!(o.wall_function, WallFunction::Standard);
    assert_eq!(o.roughness_height, 0.0);
    assert_eq!(o.wall_omega, SstWallMode::Automatic);
    assert_eq!(o.sst_variant, SstVariant::Sst);
}

#[test]
fn located_errors_for_misplaced_and_invalid_options() {
    let has = |t: &str, path: &str| errors(t).iter().any(|e| e.0 == path);
    // wall function on SST, SST keys on k-epsilon
    assert!(has(
        &with_sst("wall_function = \"scalable\""),
        "turbulence.wall_function"
    ));
    assert!(has(&with_ke("wall_omega = \"low_re\""), "turbulence.wall_omega"));
    assert!(has(&with_ke("sst_variant = \"bsl\""), "turbulence.sst_variant"));
    // invalid enumeration values and negative roughness (schema level)
    assert!(has(&with_ke("wall_function = \"blended\""), "turbulence.wall_function"));
    assert!(has(
        &with_ke("roughness_height = -1.0e-3"),
        "turbulence.roughness_height"
    ));
    assert!(has(
        &with_ke("roughness_constant = -0.3"),
        "turbulence.roughness_constant"
    ));
    // roughness with the enhanced treatment, roughness constant without roughness
    assert!(has(
        &with_ke("wall_function = \"enhanced\"\nroughness_height = 1.0e-3"),
        "turbulence.roughness_height"
    ));
    assert!(has(
        &with_ke("roughness_constant = 0.5"),
        "turbulence.roughness_constant"
    ));
    // roughness with the low-Re SST wall condition
    assert!(has(
        &with_sst("wall_omega = \"low_re\"\nroughness_height = 1.0e-3"),
        "turbulence.roughness_height"
    ));
}

/// C5 (registered in ADR 0015): the run through the settings tree equals the direct library run to 1e-12 after 60
/// iterations. The "library run" is the same case with the model supplied as a registered plugin that builds the
/// library model and sets the options on it directly (no `apply_wall_options`, no `wall_opts` code).
fn library_run_key(name: &'static str, build: fn(&ModelContext) -> Box<dyn TurbulenceModel>) -> String {
    rcfd_turb::register_model(
        name,
        "library run (test)",
        Arc::new(move |ctx: &ModelContext| Ok(build(ctx))),
    );
    name.to_string()
}

fn run60(text: &str) -> Simulation {
    let c = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"));
    let mut s = Simulation::new(&c).unwrap_or_else(|e| panic!("{e:?}"));
    for _ in 0..60 {
        s.step();
    }
    s
}

fn max_diff(a: &Simulation, b: &Simulation, names: &[&str]) -> f64 {
    let mut m = 0.0_f64;
    for n in names {
        let (x, y) = (a.field(n).unwrap(), b.field(n).unwrap());
        assert_eq!(x.len(), y.len());
        assert!(x.iter().all(|v| v.is_finite()));
        for (p, q) in x.iter().zip(&y) {
            m = m.max((p - q).abs());
        }
    }
    m
}

fn plugin_text(base: &str, key: &str) -> String {
    // the plugin path rejects the model-specific keys; the factory below sets those values on the context instead
    let mut out = String::new();
    for l in base.lines() {
        let t = l.trim_start();
        if [
            "wall_function",
            "variant",
            "k_initial",
            "epsilon_initial",
            "omega_initial",
        ]
        .iter()
        .any(|k| t.starts_with(&format!("{k} =")))
        {
            continue;
        }
        if t.starts_with("model = \"k_epsilon\"") || t.starts_with("model = \"k_omega_sst\"") {
            out.push_str(&format!(
                "model = \"plugin\"
plugin = \"{key}\"
"
            ));
        } else {
            out.push_str(l);
            out.push_str("\n");
        }
    }
    out
}

#[test]
fn c5_tree_run_equals_library_run() {
    const _: () = assert!(BUILD_HOOK_INSTALLED);
    fn ke_ctx(ctx: &ModelContext) -> ModelContext {
        let mut c = ctx.clone();
        c.k0 = 1.0; // values of the k-epsilon case file
        c.eps0 = 0.36;
        c
    }
    fn ke_scalable(ctx: &ModelContext) -> Box<dyn TurbulenceModel> {
        let mut m = KEpsilonModel::new(&ke_ctx(ctx)).unwrap();
        m.m.wall_function = WallFunction::Scalable;
        Box::new(m)
    }
    fn ke_rough(ctx: &ModelContext) -> Box<dyn TurbulenceModel> {
        let mut m = KEpsilonModel::new(&ke_ctx(ctx)).unwrap();
        m.m.roughness = rcfd_turb::wall::Roughness::new(2.0e-2, m.m.wc.kappa, m.m.wc.e);
        Box::new(m)
    }
    fn sst_with(ctx: &ModelContext, bsl: bool, low_re: bool, ks: f64) -> Box<dyn TurbulenceModel> {
        let mut ctx = ctx.clone();
        ctx.k0 = 1.0; // values of the SST case file (k_initial, omega_initial)
        ctx.omega0 = 39.5;
        ctx.eps0 = 0.0;
        let mut m = SstModel::new(&ctx).unwrap();
        if bsl {
            m.m.variant = SstVariant::Bsl;
        }
        if low_re {
            m.m.set_wall_mode(SstWallMode::LowRe).unwrap();
        }
        if ks > 0.0 {
            m.m.roughness = rcfd_turb::wall::Roughness::new(ks, m.m.law.kappa, m.m.law.e);
        }
        Box::new(m)
    }
    fn ke_plain(ctx: &ModelContext) -> Box<dyn TurbulenceModel> {
        Box::new(KEpsilonModel::new(&ke_ctx(ctx)).unwrap())
    }
    {
        let key = library_run_key("c5_ke_plain", ke_plain);
        let a = run60(&lf(KE_RAW));
        let b = run60(&plugin_text(&lf(KE_RAW), &key));
        for n in ["ux", "p", "nut", "k", "epsilon"] {
            println!("CONTROL plain {n}: {:.3e}", max_diff(&a, &b, &[n]));
        }
    }
    let k1 = library_run_key("c5_ke_scalable", ke_scalable);
    let k2 = library_run_key("c5_ke_rough", ke_rough);
    let k3 = library_run_key("c5_sst_bsl", |c| sst_with(c, true, false, 0.0));
    let k4 = library_run_key("c5_sst_low_re", |c| sst_with(c, false, true, 0.0));
    let k5 = library_run_key("c5_sst_rough", |c| sst_with(c, false, false, 2.0e-2));
    let ke_names = ["ux", "p", "nut", "k", "epsilon"];
    let sst_names = ["ux", "p", "nut", "k", "omega"];
    let mut worst = 0.0_f64;
    let cases: Vec<(&str, String, String, &[&str])> = vec![
        ("ke scalable", with_ke("wall_function = \"scalable\""), k1, &ke_names),
        ("ke roughness", with_ke("roughness_height = 2.0e-2"), k2, &ke_names),
        ("sst bsl", with_sst("sst_variant = \"bsl\""), k3, &sst_names),
        ("sst low_re", with_sst("wall_omega = \"low_re\""), k4, &sst_names),
        ("sst roughness", with_sst("roughness_height = 2.0e-2"), k5, &sst_names),
    ];
    for (label, tree_text, key, names) in cases {
        let base = if names[4] == "omega" { lf(SST_RAW) } else { lf(KE_RAW) };
        let tree = run60(&tree_text);
        let lib = run60(&plugin_text(&base, &key));
        let default = run60(&base);
        for n in names {
            println!("   {label} {n}: {:.3e}", max_diff(&tree, &lib, &[n]));
        }
        let d = max_diff(&tree, &lib, names);
        let eff = max_diff(&tree, &default, names);
        println!("C5 {label}: tree vs library max |diff| = {d:.3e}; option effect (max over fields) vs default run = {eff:.3e}");
        assert!(d <= 1e-12, "{label}: {d:e}");
        assert!(eff > 0.0, "{label}: the option changed nothing");
        worst = worst.max(d);
    }
    println!("C5 worst difference {worst:.3e} (limit 1e-12)");
}

#[test]
fn apply_functions_set_the_library_model() {
    use rcfd_fv::FvGeometry;
    use rcfd_mesh::gen::{box_2d_graded, Grading};
    use std::sync::Arc;
    let mesh = box_2d_graded(4, 20, 1.0, 1.0, [Grading::Uniform, Grading::Uniform], true, false).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let o = WallOptions {
        wall_function: WallFunction::Scalable,
        roughness_height: 1.0e-3,
        roughness_constant: 0.0,
        wall_omega: SstWallMode::Automatic,
        sst_variant: SstVariant::Bsl,
    };
    let mut ke = KEpsilon::new(KEpsilonVariant::Standard, geom.clone(), 1e-5, &["ymin"], 1.0, 1.0).unwrap();
    apply_kepsilon(&o, &mut ke);
    assert_eq!(ke.wall_function, WallFunction::Scalable);
    assert_eq!(ke.roughness.ks, 1.0e-3);
    assert!((ke.roughness.cs - 0.3).abs() < 0.05, "derived C_s {}", ke.roughness.cs);
    let mut sst = KOmegaSst::new(geom, 1e-5, &["ymin"], 1.0, 10.0).unwrap();
    apply_sst(&o, &mut sst).unwrap();
    assert_eq!(sst.variant, SstVariant::Bsl);
    assert_eq!(sst.roughness.ks, 1.0e-3);
    // explicit constant wins; low_re switches the wall condition
    let o2 = WallOptions {
        roughness_constant: 0.5,
        wall_omega: SstWallMode::LowRe,
        roughness_height: 0.0,
        ..o
    };
    apply_sst(&o2, &mut sst).unwrap();
    assert!(!sst.roughness.is_rough());
    assert_eq!(sst.wall_mode, SstWallMode::LowRe);
    let o3 = WallOptions {
        roughness_height: 2.0e-3,
        ..o2
    };
    apply_kepsilon(&o3, &mut ke);
    assert_eq!((ke.roughness.ks, ke.roughness.cs), (2.0e-3, 0.5));
}

/// C5 completion (wave 13): the two settings the first equality test did not cover, `wall_function = "enhanced"` and an
/// explicit `roughness_constant`. Same method and the same 1e-12 limit as `c5_tree_run_equals_library_run`.
#[test]
fn c5b_enhanced_and_explicit_roughness_constant() {
    fn ke_ctx(ctx: &ModelContext) -> ModelContext {
        let mut c = ctx.clone();
        c.k0 = 1.0;
        c.eps0 = 0.36;
        c
    }
    fn ke_enhanced(ctx: &ModelContext) -> Box<dyn TurbulenceModel> {
        let mut m = KEpsilonModel::new(&ke_ctx(ctx)).unwrap();
        m.m.wall_function = WallFunction::Enhanced;
        Box::new(m)
    }
    fn ke_rough_cs(ctx: &ModelContext) -> Box<dyn TurbulenceModel> {
        let mut m = KEpsilonModel::new(&ke_ctx(ctx)).unwrap();
        m.m.roughness = rcfd_turb::wall::Roughness { ks: 2.0e-2, cs: 0.5 };
        Box::new(m)
    }
    let k1 = library_run_key("c5b_ke_enhanced", ke_enhanced);
    let k2 = library_run_key("c5b_ke_rough_cs", ke_rough_cs);
    let names = ["ux", "p", "nut", "k", "epsilon"];
    for (label, tree_text, key) in [
        ("ke enhanced", with_ke("wall_function = \"enhanced\""), k1),
        (
            "ke roughness + explicit constant",
            with_ke("roughness_height = 2.0e-2\nroughness_constant = 0.5"),
            k2,
        ),
    ] {
        let tree = run60(&tree_text);
        let lib = run60(&plugin_text(&lf(KE_RAW), &key));
        let default = run60(&lf(KE_RAW));
        let d = max_diff(&tree, &lib, &names);
        let eff = max_diff(&tree, &default, &names);
        println!("C5b {label}: tree vs library max |diff| = {d:.3e}; option effect vs default run = {eff:.3e}");
        assert!(d <= 1e-12, "{label}: {d:e}");
        assert!(eff > 0.0, "{label}: the option changed nothing");
    }
}
