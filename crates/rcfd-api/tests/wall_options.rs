//! Near-wall options in the settings tree (F-203, F-043, ADR 0015 check C5): validation with located errors and the
//! apply functions on library models.
//!
//! The run-through-the-tree equals library-run check of C5 is NOT part of this file: the simulation build path
//! does not apply the options yet (`wall_opts::BUILD_HOOK_INSTALLED` is false, see ADR 0015 results), so every
//! non-default value is rejected with a located error instead of being ignored. The test
//! `non_default_values_are_rejected_until_the_build_path_applies_them` records that state and must be replaced by the
//! equality check when the hook exists.

use rcfd_api::case::Case;
use rcfd_api::wall_opts::{apply_kepsilon, apply_sst, WallOptions, BUILD_HOOK_INSTALLED};
use rcfd_turb::kepsilon::WallFunction;
use rcfd_turb::sst::{SstVariant, SstWallMode};
use rcfd_turb::{KEpsilon, KEpsilonVariant, KOmegaSst};

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

#[test]
fn non_default_values_are_rejected_until_the_build_path_applies_them() {
    let hook = BUILD_HOOK_INSTALLED;
    assert!(!std::hint::black_box(hook));
    for (t, path) in [
        (with_ke("wall_function = \"scalable\""), "turbulence.wall_function"),
        (with_ke("roughness_height = 1.0e-3"), "turbulence.roughness_height"),
        (with_sst("wall_omega = \"low_re\""), "turbulence.wall_omega"),
        (with_sst("sst_variant = \"bsl\""), "turbulence.sst_variant"),
    ] {
        let e = errors(&t);
        assert!(
            e.iter().any(|d| d.0 == path && d.1.contains("does not apply it yet")),
            "{path}: {e:?}"
        );
    }
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
