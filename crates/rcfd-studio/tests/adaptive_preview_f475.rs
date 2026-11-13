//! ADR 0377 (F-475): the Studio mesh preview of a case with `[mesh.adapt]` shows the mesh the run solves on after its
//! first step. A1 (`preview::build`'s adapted mesh is the run's), A2 (the Studio shows it, with its cells and mesh hash),
//! C1 (the loop's cost reported), D2 (a newer request cancels the loop), E1 (no other shipped case changes), F1 (negative
//! control: a case the run refuses is not adapted).

mod common;
mod pv;

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_studio::preview::{self, adapt_text, Built, Limits, Status};
use rcfd_studio::viewport::surface_of;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

const REL: &str = "cases/cavity/adapt_re100.toml";
/// The run's mesh hash after its first step (ADR 0373 A1).
const RUN_HASH: u64 = 0x05bb_8bea_e5da_57da;

fn build_ok(root: &Path, rel: &str) -> Built {
    match preview::build(root, rel, &Limits::default(), false, &|| false).result {
        Ok(b) => b,
        Err(e) => panic!("{rel}: {e:?}"),
    }
}

/// The mesh of the run of `file` after its first step (`Simulation::new`, one `step`).
fn run_mesh(file: &Path) -> rcfd_mesh::Mesh {
    let case = Case::load(file).unwrap_or_else(|d| panic!("{d:?}"));
    let mut sim = Simulation::new(&case).unwrap_or_else(|d| panic!("{d:?}"));
    sim.step();
    (*sim.geom.mesh).clone()
}

/// G1's comparison (ADR 0138): mesh hash, counts, patches, the surface bit for bit, 2D.
fn assert_same(b: &Built, mesh: &rcfd_mesh::Mesh, what: &str) {
    assert_eq!(b.mesh_hash, mesh.content_hash(), "{what}: mesh hash");
    assert_eq!(
        (b.cells, b.faces, b.points),
        (mesh.n_cells(), mesh.n_faces(), mesh.n_points()),
        "{what}: counts"
    );
    let want: Vec<_> = mesh
        .patches()
        .iter()
        .map(|p| (p.name.clone(), p.kind.clone(), p.size))
        .collect();
    let got: Vec<_> = b
        .patches
        .iter()
        .map(|p| (p.name.clone(), p.kind.clone(), p.faces))
        .collect();
    assert_eq!(got, want, "{what}: patches");
    let (s, two_d) = surface_of(mesh).unwrap();
    pv::same_surface(&b.surface, &s).unwrap_or_else(|e| panic!("{what}: surface: {e}"));
    assert_eq!(b.two_d, two_d, "{what}: 2D");
}

/// A1 and C1: the adapted mesh of `preview::build` is the run's after its first step, and the report says what the loop
/// did.
#[test]
fn a1_c1_the_preview_adapts_like_the_run() {
    let root = pv::repo_root();
    let t = Instant::now();
    let b = build_ok(&root, REL);
    let build_s = t.elapsed().as_secs_f64();
    let a = b.adapted.clone().expect("A1: no adapted mesh");
    let run = run_mesh(&root.join(REL));
    eprintln!(
        "A1: set-up {} cells {:016x}; adapted {} cells {:016x}; run {} cells {:016x}; build {build_s:.3} s",
        b.cells,
        b.mesh_hash,
        a.cells,
        a.mesh_hash,
        run.n_cells(),
        run.content_hash()
    );
    assert_same(&a, &run, "A1");
    assert_eq!(a.cells, 2799, "A1 cells");
    assert_eq!(a.mesh_hash, RUN_HASH, "A1 hash");
    // C1
    let case = Case::load(root.join(REL)).unwrap();
    let base = Simulation::new(&case).unwrap().geom.mesh.content_hash();
    let r = a.adapt.clone().expect("C1: no report");
    eprintln!("C1: {r:?}");
    eprintln!("C1 text: {}", adapt_text(&a).unwrap_or_default());
    assert_eq!(b.adapt.as_ref(), Some(&r), "C1: the same report on both");
    assert_eq!(r.base_cells, 576, "C1 base cells");
    assert_eq!(r.base_hash, base, "C1 base hash");
    assert_eq!(
        r.solves,
        vec![(576, 97, true), (1269, 122, true), (2799, 188, true)],
        "C1 solves"
    );
    assert!(
        r.loop_s > 0.0 && r.loop_s <= a.build_s,
        "C1 loop_s {} build_s {}",
        r.loop_s,
        a.build_s
    );
    assert!(r.refused.is_none(), "C1 refused");
    let text = adapt_text(&a).unwrap();
    for want in ["3 solves", "576, 1,269, 2,799 cells", "97 + 122 + 188 iterations"] {
        assert!(text.contains(want), "C1 text lacks {want:?}: {text}");
    }
}

/// A2: the Studio shows the adapted mesh, with its cells and mesh hash.
#[test]
fn a2_the_studio_shows_the_adapted_mesh() {
    if cfg!(debug_assertions) {
        eprintln!("SKIP a2 (adaptive_preview_f475.rs): registered for release builds");
        return;
    }
    let text = std::fs::read(pv::repo_root().join(REL)).unwrap();
    let root = pv::project("f475a2", &[("cases/adapt.toml", &text)]);
    let mut st = pv::studio(&root);
    let status = pv::show_case(&mut st, "cases/adapt.toml");
    assert_eq!(status, Status::Ready, "A2 status");
    let shown = st.preview.displayed().cloned().expect("A2 nothing shown");
    let built = pv::built(&st);
    let t = adapt_text(&shown).unwrap_or_default();
    eprintln!(
        "A2: shown {} cells {:016x}, built {} cells; {}",
        shown.cells, shown.mesh_hash, built.cells, t
    );
    assert_eq!((shown.cells, shown.mesh_hash), (2799, RUN_HASH), "A2 shown");
    assert_eq!((built.cells, built.mesh_hash), (2799, RUN_HASH), "A2 built");
    let surf = st.viewport.surface.clone().expect("A2 no surface on the viewport");
    assert!(
        Arc::ptr_eq(&surf, &shown.surface),
        "A2 the viewport's surface is not the shown mesh's"
    );
    assert!(
        t.contains("2,799 cells") && t.contains("05bb8beae5da57da"),
        "A2 text: {t}"
    );
}

/// D2: a newer request (another case) during the adaptive loop discards it and its thread stops at the next solve.
#[test]
fn d2_a_newer_request_cancels_the_adaptive_loop() {
    if cfg!(debug_assertions) {
        eprintln!("SKIP d2 (adaptive_preview_f475.rs): registered for release builds");
        return;
    }
    let text = std::fs::read(pv::repo_root().join(REL)).unwrap();
    let root = pv::project("f475d2", &[("cases/adapt.toml", &text)]);
    let t = Instant::now();
    let _ = build_ok(&root, "cases/adapt.toml");
    let t_full = t.elapsed().as_secs_f64();
    let mut st = pv::studio(&root);
    let t_a = Instant::now();
    st.open_case("cases/adapt.toml");
    assert!(
        st.drain_until(pv::WAIT, |s| s.preview.running() == 1),
        "set-up: no build"
    );
    let wait = Duration::from_secs_f64(0.15 * t_full).saturating_sub(t_a.elapsed());
    std::thread::sleep(wait);
    st.drain();
    let switch_s = t_a.elapsed().as_secs_f64();
    assert_eq!(
        st.preview.running(),
        1,
        "set-up: the adaptive build is not running at the switch"
    );
    assert!(
        st.preview.built.is_none(),
        "set-up: the adaptive build ended before the switch"
    );
    st.open_case("cases/cavity.toml");
    let (mut a_done, mut adapt_shown) = (None, 0usize);
    let deadline = Instant::now() + pv::WAIT;
    loop {
        st.drain();
        if st.preview.built.as_ref().is_some_and(|b| b.rel == "cases/adapt.toml") {
            adapt_shown += 1;
        }
        if a_done.is_none() && st.preview.running() == 0 {
            a_done = Some(t_a.elapsed().as_secs_f64());
        }
        let cavity = st.preview.built.as_ref().is_some_and(|b| b.rel == "cases/cavity.toml");
        if a_done.is_some() && cavity && st.preview.status == Status::Ready {
            std::thread::sleep(Duration::from_millis(20));
            st.drain();
            if st.preview.built.as_ref().is_some_and(|b| b.rel == "cases/adapt.toml") {
                adapt_shown += 1;
            }
            break;
        }
        assert!(Instant::now() < deadline, "D2: no outcome");
        std::thread::sleep(Duration::from_millis(5));
    }
    let a_s = a_done.unwrap();
    eprintln!(
        "D2: T_full {t_full:.3} s; switch at {switch_s:.3} s ({:.3} T_full); the adaptive thread ended {a_s:.3} s after its \
         request ({:.3} T_full); adaptive shown after the switch in {adapt_shown} drains; discarded {}",
        switch_s / t_full,
        a_s / t_full,
        st.preview.stats.discarded
    );
    assert_eq!(adapt_shown, 0, "D2 the adaptive result shown after the switch");
    assert!(st.preview.stats.discarded >= 1, "D2 not discarded");
    assert!(
        a_s <= 0.6 * t_full,
        "D2 the adaptive thread ended at {:.3} T_full",
        a_s / t_full
    );
}

/// E1: no shipped case without `[mesh.adapt]` gets an adapted mesh or a report.
#[test]
fn e1_other_cases_are_not_adapted() {
    let root = pv::repo_root();
    let mut n = 0;
    for d in std::fs::read_dir(root.join("cases")).unwrap() {
        let d = d.unwrap().path();
        if !d.is_dir() {
            continue;
        }
        for f in std::fs::read_dir(&d).unwrap() {
            let f = f.unwrap().path();
            if f.extension().and_then(|e| e.to_str()) != Some("toml") {
                continue;
            }
            let rel = f.strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/");
            if rel == REL {
                continue;
            }
            let b = build_ok(&root, &rel);
            assert!(b.adapted.is_none() && b.adapt.is_none(), "E1 {rel}");
            assert!(adapt_text(&b).is_none(), "E1 {rel} text");
            n += 1;
        }
    }
    eprintln!("E1: {n} cases without [mesh.adapt], none adapted");
    assert_eq!(n, 10, "E1 count");
}

/// F1 (negative control): a `[mesh.adapt]` case the run refuses shows the generator's mesh, not adapted, with the reason.
#[test]
#[ignore = "record (ADR 0377 amendment 1): defective set-up, the edit is refused at the parse by [mesh.adapt]'s check; corrected by f1b"]
fn f1_a_refused_adaptive_case_is_not_adapted() {
    let text = std::fs::read_to_string(pv::repo_root().join(REL)).unwrap();
    let edited = text.replace(
        "[boundaries.lid]\nvelocity = \"fixed\"\n",
        "[boundaries.lid]\nvelocity = \"fixed\"\nvelocity_expression_x = \"1.0\"\n",
    );
    assert_ne!(edited, text, "set-up: the [boundaries.lid] lines");
    let root = pv::project("f475f1", &[("cases/refused.toml", edited.as_bytes())]);
    let b = build_ok(&root, "cases/refused.toml");
    let case = Case::load(root.join("cases/refused.toml")).unwrap();
    let gen = rcfd_api::sim::build_mesh(&case).unwrap();
    let t = adapt_text(&b).unwrap_or_default();
    eprintln!("F1: {} cells {:016x}; {t}", b.cells, b.mesh_hash);
    assert!(b.adapted.is_none(), "F1 adapted");
    let r = b.adapt.as_ref().expect("F1 no report");
    assert!(
        r.refused
            .as_deref()
            .is_some_and(|m| m.contains("velocity_expression_x")),
        "F1 refused: {:?}",
        r.refused
    );
    assert_eq!(b.mesh_hash, gen.content_hash(), "F1 build_mesh");
    assert!(t.starts_with("[mesh.adapt]: not adapted"), "F1 text: {t}");
}

/// F1b (ADR 0377 amendment 1): F1 with a refusal that needs the mesh (a boundary that is not a patch), so the text parses and
/// `Simulation::new` refuses it.
#[test]
fn f1b_a_refused_adaptive_case_is_not_adapted() {
    let text = std::fs::read_to_string(pv::repo_root().join(REL)).unwrap();
    let edited = text.replace(
        "[boundaries.walls]\n",
        "[boundaries.nosuch]\nvelocity = \"fixed\"\n\n[boundaries.walls]\n",
    );
    assert_ne!(edited, text, "set-up: the [boundaries.walls] line");
    let root = pv::project("f475f1b", &[("cases/refused.toml", edited.as_bytes())]);
    let b = build_ok(&root, "cases/refused.toml");
    let case = Case::load(root.join("cases/refused.toml")).unwrap();
    let gen = rcfd_api::sim::build_mesh(&case).unwrap();
    let t = adapt_text(&b).unwrap_or_default();
    eprintln!("F1b: {} cells {:016x}; {t}", b.cells, b.mesh_hash);
    assert!(b.adapted.is_none(), "F1b adapted");
    let r = b.adapt.as_ref().expect("F1b no report");
    assert!(
        r.refused.as_deref().is_some_and(|m| m.contains("nosuch")),
        "F1b refused: {:?}",
        r.refused
    );
    assert_eq!(b.mesh_hash, gen.content_hash(), "F1b build_mesh");
    assert!(t.starts_with("[mesh.adapt]: not adapted"), "F1b text: {t}");
}
