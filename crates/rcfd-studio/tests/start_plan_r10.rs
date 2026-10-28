//! ADR 0133 R10 (W48-5 (a)): the start plan's store proposal, pure logic. Every planned case still parses with its
//! `set` applied.

mod common;

use rcfd_api::case::Case;
use rcfd_api::chain::apply_set;
use rcfd_studio::runs::{PlanRoute, StartPlan};
use std::path::Path;

fn cavity(alg: &str, extra: &str) -> String {
    format!(
        "[mesh]\ngenerator = \"cavity_2d\"\nnx = 12\nny = 12\n[physics]\nnu = 0.01\n[boundaries.lid]\nvelocity = \"fixed\"\nvelocity_value = [1.0, 0.0, 0.0]\n[boundaries.walls]\nvelocity = \"fixed\"\n[solver]\nalgorithm = \"{alg}\"\nmax_iterations = 400\nend_time = 0.3\ndt = 0.01\n{extra}"
    )
}

/// `piston("", "")` of `crates/rcfd-api/tests/dynamic_mesh.rs`.
const PISTON: &str = r#"
[mesh]
generator = "box_2d"
nx = 20
ny = 2
lx = 1.0
ly = 0.2
[physics]
nu = 0.01
rho = 1.0
[boundaries.xmin]
velocity = "fixed"
[boundaries.xmax]
velocity = "zero_gradient"
pressure = "fixed"
pressure_value = 0.0
[boundaries.ymin]
velocity = "zero_gradient"
[boundaries.ymax]
velocity = "zero_gradient"
[solver]
algorithm = "piso"
convection = "central"
dt = 0.005
end_time = 0.5

[dynamic_mesh]
enabled = true
moving_patches = ["xmin"]
deformation = "smoothing"
displacement_expression_x = "1 [m/s^2] * t^2"
fixed_patches = ["xmax"]
"#;

fn plan(text: &str) -> StartPlan {
    StartPlan::new("cases/x.toml", text, Path::new(".")).unwrap()
}

fn parses_with_set(text: &str, p: &StartPlan) {
    let mut c = Case::parse_toml(text).unwrap();
    for s in p.set() {
        apply_set(&mut c, &s).unwrap_or_else(|e| panic!("{s}: {e:?}"));
    }
}

#[test]
fn r10_store_proposal_of_the_start_plan() {
    // (a) steady, no store keys
    let a_text = cavity("simplec", "");
    let a = plan(&a_text);
    eprintln!(
        "R10 (a): set {:?}, snapshots {:?}, estimated {:?} bytes",
        a.set(),
        a.snapshots(),
        a.estimated_bytes()
    );
    assert_eq!(a.set(), ["output.store_interval=4", "output.store_precision=f32"]);
    assert_eq!(a.snapshots(), Some(100));
    assert_eq!(a.estimated_bytes(), Some(144 * 4 * 100 * 4));
    assert_eq!(a.estimated_bytes(), Some(230_400));
    parses_with_set(&a_text, &a);
    // (b) the case sets its own interval: nothing added
    let b_text = cavity("simplec", "[output]\nstore_interval = 7\n");
    let b = plan(&b_text);
    eprintln!("R10 (b): set {:?}, note {:?}", b.set(), b.note);
    assert!(b.set().is_empty());
    assert!(b.store.is_none());
    parses_with_set(&b_text, &b);
    // (b0) ADR 0152 (a registered change): an explicit 0 is the schema default, which a case saved by the Studio spells
    // out, so it is not the case's own: the plan is (a)'s proposal
    let b0_text = cavity("simplec", "[output]\nstore_interval = 0\n");
    let b0 = plan(&b0_text);
    eprintln!("R10 (b0): set {:?}", b0.set());
    assert_eq!(b0.set(), ["output.store_interval=4", "output.store_precision=f32"]);
    parses_with_set(&b0_text, &b0);
    // the registered (b0) as the route before ADR 0152: an explicit 0 in the text was the case's own choice
    let before = PlanRoute {
        default_store_keys_set: true,
        ..PlanRoute::default()
    };
    let b0_before = StartPlan::new_route("cases/x.toml", &b0_text, Path::new("."), before).unwrap();
    assert!(b0_before.set().is_empty(), "{:?}", b0_before.set());
    // (c) switched off
    let mut c = plan(&a_text);
    c.store.as_mut().unwrap().on = false;
    assert!(c.set().is_empty());
    assert_eq!(c.estimated_bytes(), None);
    // (d) f64 picked
    let mut d = plan(&a_text);
    d.store.as_mut().unwrap().f32 = false;
    eprintln!("R10 (d): set {:?}, estimated {:?} bytes", d.set(), d.estimated_bytes());
    assert_eq!(d.set(), ["output.store_interval=4"]);
    assert_eq!(d.estimated_bytes(), Some(460_800));
    parses_with_set(&a_text, &d);
    // (e) transient piso with end_time 0.3
    let e_text = cavity("piso", "");
    let e = plan(&e_text);
    eprintln!("R10 (e): set {:?}", e.set());
    assert_eq!(
        e.set(),
        ["output.store_time_interval=0.003", "output.store_precision=f32"]
    );
    parses_with_set(&e_text, &e);
    // (f) a moving mesh: no store, a note naming the key (amendment 3: the piston case of rcfd-api's dynamic_mesh.rs)
    let f_text = PISTON.to_string();
    let f = plan(&f_text);
    eprintln!("R10 (f): set {:?}, note {:?}", f.set(), f.note);
    assert!(f.set().is_empty());
    assert!(f.note.as_deref().is_some_and(|n| n.contains("dynamic_mesh.enabled")));
    parses_with_set(&f_text, &f);
    // the user's overrides follow the store keys
    let mut g = plan(&a_text);
    g.extra_set = vec!["solver.max_iterations=50".into(), "  ".into()];
    assert_eq!(
        g.set(),
        [
            "output.store_interval=4",
            "output.store_precision=f32",
            "solver.max_iterations=50"
        ]
    );
}

#[test]
fn r10_at_most_100_snapshots_for_any_iteration_count() {
    for m in (1..=3000u64).chain([9_999, 10_000, 10_001, 1_000_000]) {
        let text = cavity("simplec", "").replace("max_iterations = 400", &format!("max_iterations = {m}"));
        let p = plan(&text);
        let s = p.snapshots().unwrap();
        assert!(s <= 100, "M = {m}: {s} snapshots");
    }
}
