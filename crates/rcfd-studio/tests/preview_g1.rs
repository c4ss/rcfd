//! ADR 0138 (wave 48G1) checks of the mesh preview of a case that need no GPU: G1 (the previewed mesh and surface are
//! the solver's), G4 (failure paths), G5 (cancellation), G7 (every shipped case), G9 (2D boundary hover), and the cell
//! estimate (unregistered).

mod common;
mod pv;

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_render::wgpu;
use rcfd_studio::preview::{self, Built, Estimate, Limits, Refusal, Status};
use rcfd_studio::viewport::{surface_of, CameraState, StoreView, ViewGpu};
use rcfd_studio::StudioApp;
use rcfd_tsstore::{Location, Precision, WriteOptions, Writer};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn build_ok(root: &Path, rel: &str) -> Built {
    match preview::build(root, rel, &Limits::default(), false, &|| false).result {
        Ok(b) => b,
        Err(e) => panic!("{rel}: {e:?}"),
    }
}

/// The solver's mesh of a case file, as `rcfd run <file>` builds it (`Case::load`, `Simulation::new`); falls back to
/// `build_mesh` of the same case (reported) when the simulation cannot be set up.
fn solver_mesh(file: &Path) -> (rcfd_mesh::Mesh, &'static str) {
    let case = Case::load(file).unwrap_or_else(|d| panic!("{}: {d:?}", file.display()));
    match Simulation::new(&case) {
        Ok(sim) => ((*sim.geom.mesh).clone(), "Simulation::new"),
        Err(d) => {
            eprintln!(
                "G1 note: {}: Simulation::new refused ({}); compared with build_mesh",
                file.display(),
                d.first().map(|x| x.to_string()).unwrap_or_default()
            );
            (rcfd_api::sim::build_mesh(&case).unwrap(), "build_mesh")
        }
    }
}

/// G1 limit: mesh hash, counts and patches, and the surface field by field, bit for bit; the start camera too.
fn assert_same(b: &Built, mesh: &rcfd_mesh::Mesh, what: &str) {
    assert_eq!(b.mesh_hash, mesh.content_hash(), "{what}: mesh hash");
    assert_eq!(
        (b.cells, b.faces, b.points),
        (mesh.n_cells(), mesh.n_faces(), mesh.n_points()),
        "{what}: counts"
    );
    let want: Vec<(String, rcfd_mesh::PatchKind, usize)> = mesh
        .patches()
        .iter()
        .map(|p| (p.name.clone(), p.kind.clone(), p.size))
        .collect();
    let got: Vec<(String, rcfd_mesh::PatchKind, usize)> = b
        .patches
        .iter()
        .map(|p| (p.name.clone(), p.kind.clone(), p.faces))
        .collect();
    assert_eq!(got, want, "{what}: patches");
    let (s, two_d) = surface_of(mesh).unwrap();
    pv::same_surface(&b.surface, &s).unwrap_or_else(|e| panic!("{what}: surface: {e}"));
    assert_eq!(b.two_d, two_d, "{what}: 2D");
    let (lo, hi) = s.bounds();
    let (blo, bhi) = b.surface.bounds();
    assert_eq!(
        CameraState::start(blo, bhi, b.two_d),
        CameraState::start(lo, hi, two_d),
        "{what}: start camera"
    );
}

/// The shipped case files: `cases/*/*.toml` (cases) and `cases/*/case.json` (benchmark descriptors), sorted.
fn shipped() -> (Vec<String>, Vec<String>) {
    let root = pv::repo_root();
    let (mut tomls, mut jsons) = (Vec::new(), Vec::new());
    for d in std::fs::read_dir(root.join("cases")).unwrap() {
        let d = d.unwrap().path();
        if !d.is_dir() {
            continue;
        }
        for f in std::fs::read_dir(&d).unwrap() {
            let f = f.unwrap().path();
            let rel = f.strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/");
            match f.extension().and_then(|e| e.to_str()) {
                Some("toml") => tomls.push(rel),
                Some("json") if f.file_name().is_some_and(|n| n == "case.json") => jsons.push(rel),
                _ => {}
            }
        }
    }
    tomls.sort();
    jsons.sort();
    (tomls, jsons)
}

/// G1 (a) and G7: every shipped TOML case previews, and its mesh and surface are the solver's; every benchmark descriptor
/// is refused with a located message. Negative controls (G1): the profile-inlet case parsed with base folder `.` (the
/// case editor's rule) is refused; one more cell in x gives another mesh hash.
#[test]
fn g1_g7_shipped_cases_preview_the_solvers_mesh() {
    let root = pv::repo_root();
    let (tomls, jsons) = shipped();
    assert_eq!(
        (tomls.len(), jsons.len()),
        (10, 6),
        "set-up: shipped case files {tomls:?} {jsons:?}"
    );
    for rel in &tomls {
        let t = Instant::now();
        let b = build_ok(&root, rel);
        let preview_s = t.elapsed().as_secs_f64();
        let t = Instant::now();
        let (mesh, by) = solver_mesh(&root.join(rel));
        let solver_s = t.elapsed().as_secs_f64();
        assert_same(&b, &mesh, rel);
        eprintln!(
            "G1/G7 {rel}: {} cells, {} patches, surface {} faces {} triangles, 2D {}; preview {preview_s:.3} s, \
             {by} {solver_s:.3} s; identical",
            b.cells,
            b.patches.len(),
            b.surface.n_faces(),
            b.triangles,
            b.two_d
        );
    }
    for rel in &jsons {
        match preview::build(&root, rel, &Limits::default(), false, &|| false).result {
            Err(Refusal::Failed(m)) => {
                let first = m.lines().next().unwrap_or_default();
                assert!(first.starts_with(&format!("{rel}: ")), "{rel}: not located: {first}");
                eprintln!(
                    "G7 {rel}: refused, {} diagnostics; first: {first:.160}",
                    m.lines().count()
                );
            }
            other => panic!("{rel}: a benchmark descriptor previewed: {other:?}"),
        }
    }
    // negative controls
    let text = std::fs::read_to_string(root.join("cases/channel/profile_inlet.toml")).unwrap();
    let dot = Case::parse_toml(&text);
    assert!(dot.is_err(), "control: the profile case parsed with base '.'");
    eprintln!("G1 control: profile_inlet.toml with base '.': {}", dot.unwrap_err()[0]);
    let cav = std::fs::read_to_string(root.join("cases/wedge/wedge.toml")).unwrap();
    let more = cav.replace("nx = 60", "nx = 61");
    assert_ne!(cav, more, "set-up: nx = 60 in wedge.toml");
    let a = rcfd_api::sim::build_mesh(&Case::parse_toml(&cav).unwrap()).unwrap();
    let b = rcfd_api::sim::build_mesh(&Case::parse_toml(&more).unwrap()).unwrap();
    assert_ne!(a.content_hash(), b.content_hash(), "control: nx + 1");
    eprintln!("G1 control: wedge nx 60 -> 61 changes the mesh hash");
}

/// G1 (b): in a project, the cavity (recipe) and the hex box (mesh file `../meshes/hex.msh`) preview the mesh of
/// `Simulation::new` of the same file, and the surface equals the run viewport's surface of a store written from that
/// mesh (the hook's `Writer::create`). Negative control: the hex case with base folder `.` does not build.
#[test]
fn g1b_project_cases_equal_the_run_viewport_surface() {
    let root = pv::project("g1b", &[]);
    let mut st = pv::studio(&root);
    for rel in ["cases/cavity.toml", "cases/hex.toml"] {
        assert_eq!(pv::show_case(&mut st, rel), Status::Ready, "{rel}");
        let b = pv::built(&st);
        let (mesh, by) = solver_mesh(&root.join(rel));
        assert_same(&b, &mesh, rel);
        // the run viewport's surface of a store holding this mesh
        let dir = pv::temp("g1b-store").join("fields");
        std::fs::create_dir_all(&dir).unwrap();
        let mut w = Writer::create(&dir, &mesh, WriteOptions::default()).unwrap();
        let mut s = w.snapshot(1, 0.0).unwrap();
        s.field("p", "Pa", Location::Cell, &vec![0.0; mesh.n_cells()], Precision::F64)
            .unwrap();
        s.commit().unwrap();
        w.finish().unwrap();
        let sv = StoreView::open(&dir).unwrap();
        pv::same_surface(&b.surface, &sv.surface).unwrap_or_else(|e| panic!("{rel}: store surface: {e}"));
        assert_eq!(b.two_d, sv.two_d, "{rel}");
        // the viewport shows it (no run selected), with the run viewport's start camera
        assert!(st.viewport.run.is_none());
        let shown = st.viewport.surface.clone().expect("installed");
        assert!(Arc::ptr_eq(&shown, &b.surface), "{rel}: the viewport shows the preview");
        let (lo, hi) = sv.surface.bounds();
        assert_eq!(st.viewport.camera, Some(CameraState::start(lo, hi, sv.two_d)), "{rel}");
        eprintln!(
            "G1b {rel}: {} cells, 2D {}, equal to {by} and to the store surface ({} faces); viewport camera {:?}",
            b.cells,
            b.two_d,
            sv.surface.n_faces(),
            st.viewport.camera.map(|c| c.cam.projection)
        );
    }
    let dot = rcfd_api::sim::build_mesh(&Case::parse_toml(pv::HEX_CASE).unwrap());
    assert!(dot.is_err(), "control: the hex case built with base '.'");
    eprintln!("G1b control: hex case with base '.': {}", dot.unwrap_err()[0]);
}

/// G1 (c): the mesh of a real `rcfd run` started as the runner starts it (`case.snapshot`, `--case-dir`, `--store-dir`)
/// equals the preview (needs `rcfd` and `rcfd-runner` of this profile, as the ADR 0133 tests).
#[test]
fn g1c_a_real_run_stores_the_previewed_mesh() {
    let (_, rcfd) = common::binaries();
    let root = pv::project("g1c", &[]);
    let mut st = pv::studio(&root);
    for rel in ["cases/cavity.toml", "cases/hex.toml"] {
        assert_eq!(pv::show_case(&mut st, rel), Status::Ready, "{rel}");
        let b = pv::built(&st);
        let run = pv::temp("g1c-run");
        std::fs::copy(root.join(rel), run.join("case.snapshot")).unwrap();
        let out = std::process::Command::new(&rcfd)
            .arg("run")
            .arg(run.join("case.snapshot"))
            .arg("--case-dir")
            .arg(root.join("cases"))
            .arg("--out")
            .arg(run.join("output"))
            .arg("--store-dir")
            .arg(run.join("fields"))
            .args([
                "--set",
                "output.store_interval=1",
                "--set",
                "solver.max_iterations=2",
                "--quiet",
            ])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{rel}: rcfd run failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let sv = StoreView::open(&run.join("fields")).unwrap();
        pv::same_surface(&b.surface, &sv.surface).unwrap_or_else(|e| panic!("{rel}: run store: {e}"));
        let mut r = rcfd_tsstore::Reader::open(&run.join("fields")).unwrap();
        assert_eq!(r.mesh().unwrap().content_hash(), b.mesh_hash, "{rel}: mesh of the run");
        eprintln!(
            "G1c {rel}: rcfd run stored {} snapshots; mesh hash {:016x} equal, surface equal",
            sv.snaps().len(),
            b.mesh_hash
        );
    }
}

/// G4 (a)-(d): located failures and the size limits.
#[test]
fn g4_failure_paths_are_located() {
    let bad = "title = \"x\"\n[mesh\nnx = 3\n";
    let nofile = "[mesh]\nsource = \"file\"\nfile = \"../meshes/missing.msh\"\n";
    let sector =
        "[mesh]\ngenerator = \"annulus_3d_sector\"\nnx = 4\nny = 2\nnz = 2\nr_in = 1.0\nr_out = 0.5\nlz = 1.0\n\
                  sector_angle = 30.0\n";
    let root = pv::project(
        "g4",
        &[
            ("cases/bad.toml", bad.as_bytes()),
            ("cases/nofile.toml", nofile.as_bytes()),
            ("cases/sector.toml", sector.as_bytes()),
            ("cases/huge.toml", pv::box_case(30_000, 30_000).as_bytes()),
        ],
    );
    let mut st = pv::studio(&root);
    let failed = |s: Status| match s {
        Status::Failed(m) => m,
        other => panic!("not failed: {other:?}"),
    };
    // (a) a case that does not parse: the file and the line
    let m = failed(pv::show_case(&mut st, "cases/bad.toml"));
    assert!(m.starts_with("cases/bad.toml: line "), "{m}");
    assert!(st.viewport.surface.is_none(), "nothing shown");
    eprintln!("G4a: {m}");
    // (b) a mesh that does not build: the file, the setting and the missing file
    let m = failed(pv::show_case(&mut st, "cases/nofile.toml"));
    assert!(
        m.starts_with("cases/nofile.toml: mesh.file: ") && m.contains("missing.msh"),
        "{m}"
    );
    eprintln!("G4b: {m}");
    let m = failed(pv::show_case(&mut st, "cases/sector.toml"));
    assert!(m.starts_with("cases/sector.toml: mesh.r_out: "), "{m}");
    eprintln!("G4b: {m}");
    // (c) over the cell limit: refused before building, at once; "build anyway" builds
    let n0 = preview::MESHES_BUILT.load(Ordering::SeqCst);
    let t = Instant::now();
    let s = pv::show_case(&mut st, "cases/huge.toml");
    let dt = t.elapsed().as_secs_f64();
    match &s {
        Status::TooLarge { message, rebuild: true } => {
            assert!(
                message.starts_with("cases/huge.toml: about 900,000,000 cells") && message.contains("2,000,000"),
                "{message}"
            );
            eprintln!("G4c: {message} (in {dt:.3} s)");
        }
        other => panic!("{other:?}"),
    }
    assert!(dt < 5.0, "the refusal took {dt} s");
    assert_eq!(preview::MESHES_BUILT.load(Ordering::SeqCst), n0, "a mesh was built");
    st.preview.limits.cells = 100;
    let s = pv::show_case(&mut st, "cases/cavity.toml");
    assert!(matches!(s, Status::TooLarge { rebuild: true, .. }), "{s:?}");
    assert_eq!(preview::MESHES_BUILT.load(Ordering::SeqCst), n0);
    st.preview.anyway();
    assert!(st.drain_until(pv::WAIT, |s| s.preview.status == Status::Ready));
    assert_eq!(pv::built(&st).cells, 256);
    assert_eq!(
        preview::MESHES_BUILT.load(Ordering::SeqCst),
        n0 + 1,
        "built once, anyway"
    );
    eprintln!("G4c: limit 100 cells: cavity (256) refused; build anyway: 256 cells shown");
    // (d) over the triangle limit: built, not shown; "show anyway" shows it without a rebuild
    st.preview.limits = Limits {
        triangles: 100,
        ..Limits::default()
    };
    let s = pv::show_case(&mut st, "cases/hex.toml");
    match &s {
        Status::TooLarge {
            message,
            rebuild: false,
        } => {
            assert!(
                message.starts_with("cases/hex.toml: the surface has 192 triangles"),
                "{message}"
            );
            eprintln!("G4d: {message}");
        }
        other => panic!("{other:?}"),
    }
    assert!(st.viewport.surface.is_none(), "over the limit: not shown");
    let (n1, req) = (preview::MESHES_BUILT.load(Ordering::SeqCst), st.preview.stats.requests);
    st.preview.anyway();
    assert!(st.drain_until(pv::WAIT, |s| s.viewport.surface.is_some()));
    assert!(Arc::ptr_eq(
        st.viewport.surface.as_ref().unwrap(),
        &pv::built(&st).surface
    ));
    assert_eq!(
        (preview::MESHES_BUILT.load(Ordering::SeqCst), st.preview.stats.requests),
        (n1, req),
        "shown without a rebuild"
    );
    eprintln!("G4d: show anyway: shown without a rebuild");
}

/// G4 (e): no adapter. The viewport's GPU is the located "no adapter" error; 20 headless frames draw the error, the
/// counts and every patch name, without a panic.
#[test]
fn g4e_no_adapter_still_lists_the_mesh() {
    let root = pv::project("g4e", &[]);
    let mut st = pv::studio(&root);
    let e = ViewGpu::headless(wgpu::Backends::empty())
        .err()
        .expect("no adapter for no back end");
    assert!(e.starts_with("viewport: ") && e.contains("no adapter"), "{e}");
    st.viewport.set_gpu(Err(e.clone()));
    assert_eq!(pv::show_case(&mut st, "cases/hex.toml"), Status::Ready);
    let counts = rcfd_studio::preview_ui::counts(&pv::built(&st));
    let mut app = StudioApp::new(st, None);
    let ctx = egui::Context::default();
    for i in 0..20 {
        let (drawn, _) = pv::timed_frame(&mut app, &ctx, Vec::new());
        for want in [
            e.as_str(),
            counts.as_str(),
            "inlet",
            "outlet",
            "walls",
            "no GPU attached",
        ] {
            assert!(drawn.iter().any(|t| t == want), "frame {i}: '{want}' not drawn");
        }
    }
    eprintln!("G4e: 20 frames: '{e}', '{counts}' and the 3 patch names drawn");
}

/// G5: a newer selection discards an older build. Case A (a 1000 x 1000 box, slow) is asked for, then case B (the
/// cavity) while A builds: B is shown, A is never shown and its result is dropped. Negative control: applying every
/// result (generation check off) shows A after B.
#[test]
fn g5_a_newer_selection_discards_the_older_build() {
    if cfg!(debug_assertions) {
        eprintln!("SKIP g5 (crates/rcfd-studio/tests/preview_g1.rs): registered for release builds");
        return;
    }
    for control in [false, true] {
        let root = pv::project("g5", &[("cases/big.toml", pv::box_case(1000, 1000).as_bytes())]);
        let mut st = pv::studio(&root);
        st.preview.switches.ignore_generation = control;
        let t_a = Instant::now();
        st.open_case("cases/big.toml");
        assert!(st.drain_until(pv::WAIT, |s| s.preview.running() == 1));
        std::thread::sleep(Duration::from_millis(50));
        st.drain();
        assert_eq!(st.preview.running(), 1, "set-up: A still building when B is asked");
        st.open_case("cases/cavity.toml");
        let t_b = Instant::now();
        let (mut b_shown, mut a_shown_after_b) = (None, 0usize);
        let deadline = Instant::now() + pv::WAIT;
        let mut a_done = None;
        loop {
            st.drain();
            let shown = st.preview.built.as_ref().map(|b| b.rel.clone());
            if shown.as_deref() == Some("cases/big.toml") {
                a_shown_after_b += 1;
            }
            if b_shown.is_none() && shown.as_deref() == Some("cases/cavity.toml") && st.preview.status == Status::Ready
            {
                b_shown = Some(t_b.elapsed().as_secs_f64());
            }
            if a_done.is_none() && st.preview.running() == 0 {
                a_done = Some(t_a.elapsed().as_secs_f64());
            }
            if b_shown.is_some() && a_done.is_some() {
                // one more drain after A's thread ended: its result is in the channel
                std::thread::sleep(Duration::from_millis(20));
                st.drain();
                if st.preview.built.as_ref().is_some_and(|b| b.rel == "cases/big.toml") {
                    a_shown_after_b += 1;
                }
                break;
            }
            assert!(Instant::now() < deadline, "G5: no outcome");
            std::thread::sleep(Duration::from_millis(5));
        }
        let (b_s, a_s) = (b_shown.unwrap_or(f64::NAN), a_done.unwrap());
        eprintln!(
            "G5{}: B shown {b_s:.3} s after its request; A's thread ended {a_s:.3} s after A's request; A shown after B in \
             {a_shown_after_b} drains; discarded {}, requests {}",
            if control { " control" } else { "" },
            st.preview.stats.discarded,
            st.preview.stats.requests
        );
        assert!(a_s >= 0.5, "set-up: A must take >= 0.5 s ({a_s:.3} s)");
        if control {
            assert!(a_shown_after_b > 0, "control: the stale result of A was not applied");
        } else {
            assert_eq!(a_shown_after_b, 0, "A shown after B was asked");
            assert!(b_s <= 2.0, "B shown after {b_s} s");
            assert!(st.preview.stats.discarded >= 1, "A's result not discarded");
            let shown = st.viewport.surface.clone().unwrap();
            assert!(Arc::ptr_eq(&shown, &pv::built(&st).surface));
            assert_eq!(pv::built(&st).rel, "cases/cavity.toml");
        }
    }
}

/// G9: 2D boundary hover. For every outline segment of the cavity, the pointer at its projected midpoint names the
/// segment's patch, which is `lid` on y = 1 and `walls` elsewhere (from the coordinates, not from the outline list);
/// through the drawn app the readout text is drawn. Negative control: at the image centre no outline is found.
#[test]
fn g9_2d_boundary_hover_names_the_patch() {
    let root = pv::project("g9", &[]);
    let mut st = pv::studio(&root);
    assert_eq!(pv::show_case(&mut st, "cases/cavity.toml"), Status::Ready);
    let b = pv::built(&st);
    assert!(b.two_d);
    st.viewport.set_size(800, 600, false);
    let cam = st.viewport.camera.unwrap().cam;
    let size = st.viewport.size;
    let expect = |m: [f64; 3]| if (m[1] - 1.0).abs() < 1e-12 { "lid" } else { "walls" };
    let mut n = 0;
    for (i, (_, s)) in b.outlines.iter().enumerate() {
        let mid = [0, 1, 2].map(|d| 0.5 * (s[0][d] + s[1][d]));
        let px = preview::project_all(&cam, size, std::iter::once(mid))[0].unwrap();
        let j = preview::outline_at(&b, &cam, size, px, 3.0).unwrap_or_else(|| panic!("segment {i}: no outline"));
        let got = &b.patches[b.outlines[j].0].name;
        assert_eq!(got, expect(mid), "segment {i} at {mid:?}");
        n += 1;
    }
    assert_eq!(n, 64, "set-up: 4 sides x 16 faces");
    let centre = [0.5 * size[0] as f64, 0.5 * size[1] as f64];
    assert_eq!(
        preview::outline_at(&b, &cam, size, centre, 3.0),
        None,
        "control: the centre"
    );
    // through the drawn app
    let mut app = StudioApp::new(st, None);
    let ctx = egui::Context::default();
    let _ = pv::timed_frame(&mut app, &ctx, Vec::new());
    let vp = &app.st.viewport;
    let (rect, cam, size) = (vp.image_rect.unwrap(), vp.camera.unwrap().cam, vp.size);
    let to_pt = |p: [f64; 2]| {
        egui::pos2(
            rect.min.x + (p[0] / size[0] as f64) as f32 * rect.width(),
            rect.min.y + (p[1] / size[1] as f64) as f32 * rect.height(),
        )
    };
    let lid = b
        .outlines
        .iter()
        .find(|(k, _)| b.patches[*k].name == "lid")
        .map(|(_, s)| [0, 1, 2].map(|d| 0.5 * (s[0][d] + s[1][d])))
        .unwrap();
    let lid_pt = to_pt(preview::project_all(&cam, size, std::iter::once(lid))[0].unwrap());
    let _ = pv::timed_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(lid_pt)]);
    let (drawn, _) = pv::timed_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(lid_pt)]);
    assert!(
        drawn.iter().any(|t| t == "patch lid (wall), boundary"),
        "lid readout not drawn"
    );
    let mid_pt = rect.center();
    let _ = pv::timed_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(mid_pt)]);
    let (drawn, _) = pv::timed_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(mid_pt)]);
    assert!(
        !drawn.iter().any(|t| t.ends_with(", boundary")),
        "control: a boundary at the centre"
    );
    eprintln!("G9: 64 of 64 outline midpoints name their patch (lid on y = 1, walls elsewhere); 'patch lid (wall), boundary' drawn; none at the centre");
}

/// Unregistered: the cell estimate made before building equals the built mesh's cells for the generators it lists.
#[test]
fn estimate_equals_the_built_cells() {
    let cases = [
        ("cavity_2d", "nx = 5\nny = 3\n"),
        ("box_2d", "nx = 5\nny = 3\n"),
        ("channel_2d", "nx = 5\nny = 3\n"),
        ("wedge_2d", "nx = 5\nny = 3\n"),
        ("nozzle_2d", "nx = 5\nny = 3\nthroat_height = 0.5\n"),
        ("channel_2d_interface", "nx = 4\nny = 3\nnx_right = 5\nny_right = 2\n"),
        (
            "annulus_3d",
            "nx = 12\nny = 2\nnz = 3\nr_in = 0.5\nr_out = 1.0\nlz = 1.0\n",
        ),
        (
            "annulus_3d_sector",
            "nx = 4\nny = 2\nnz = 3\nr_in = 0.5\nr_out = 1.0\nlz = 1.0\nsector_angle = 60.0\n",
        ),
        (
            "annulus_3d_interface",
            "nx = 12\nny = 2\nnz = 2\nny_right = 3\nnz_right = 4\nr_in = 0.5\nr_out = 1.0\nlz = 1.0\n",
        ),
        (
            "annulus_3d_stage",
            "nx = 12\nny = 2\nnz = 2\nnx_right = 10\nny_right = 3\nnz_right = 4\nr_in = 0.5\nr_out = 1.0\nlz = 1.0\n",
        ),
    ];
    for (g, keys) in cases {
        let text = format!("[mesh]\ngenerator = \"{g}\"\n{keys}");
        let case = Case::parse_toml(&text).unwrap_or_else(|d| panic!("{g}: {d:?}"));
        let n = rcfd_api::sim::build_mesh(&case)
            .unwrap_or_else(|d| panic!("{g}: {d:?}"))
            .n_cells() as u64;
        assert_eq!(preview::estimate(&case), Estimate::Cells(n), "{g}");
    }
    let file = Case::parse_toml_in(pv::HEX_CASE, &PathBuf::from("does-not-exist")).unwrap();
    assert_eq!(preview::estimate(&file), Estimate::Unknown, "a missing mesh file");
}
