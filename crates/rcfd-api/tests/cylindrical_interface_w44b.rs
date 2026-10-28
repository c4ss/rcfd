//! Wave 44B (ADR 0112): the cylindrical frozen-rotor interface in the case tree (`mesh.generator =
//! "annulus_3d_cylindrical_interface"`), its non-orthogonality pre-flight and the near-isotropic interface layer
//! (`mesh.isotropic_interface`). Checks T1-T5 and their limits were registered in the ADR before this code ran.

use rcfd_api::case::Case;
use rcfd_api::sim::{build_mesh, Simulation};
use rcfd_mesh::gen::{annulus_3d, Grading};
use rcfd_mesh::non_conformal::join_non_conformal_cylindrical;
use rcfd_mesh::Mesh;

const RI: f64 = 0.5;
const RO: f64 = 1.0;
const V_I: f64 = 0.5;

fn fnv(sim: &Simulation) -> u64 {
    let mut h = rcfd_core::hash::Fnv64::new();
    for name in ["ux", "uy", "uz", "p"] {
        for x in sim.field(name).unwrap() {
            h.write_f64(x);
        }
    }
    for x in sim.flux() {
        h.write_f64(*x);
    }
    h.finish()
}

/// T4: a case without the new keys is bit-identical to HEAD 2f1a3ce.
#[test]
fn t4_annulus_interface_case_without_the_keys_is_bit_identical() {
    let text = "[mesh]\ngenerator = \"annulus_3d_interface\"\nnx = 24\nny = 4\nnz = 2\nny_right = 3\nr_in = 0.5\nr_out = 1.0\nlz = 0.4\n[physics]\nnu = 0.1\n[boundaries.inner]\nvelocity = \"fixed\"\nvelocity_expression_x = \"-1.0 [m/s] * y / 1 [m]\"\nvelocity_expression_y = \"1.0 [m/s] * x / 1 [m]\"\n[boundaries.outer]\nvelocity = \"fixed\"\n[boundaries.zmin]\nvelocity = \"symmetry\"\n[boundaries.zmax]\nvelocity = \"symmetry\"\n[solver]\nalgorithm = \"simplec\"\nconvection = \"linear_upwind\"\nmax_iterations = 30\n[sources.rotor]\ntype = \"mrf\"\nrotation_axis = [0.0, 0.0, 1.0]\nangular_velocity = 1.0\nzone = \"all\"\n";
    let mut sim = Simulation::new(&Case::parse_toml(text).unwrap()).unwrap();
    sim.run(30);
    let h = fnv(&sim);
    println!("T4 hash {h:#018x}");
    // recorded at HEAD 2f1a3ce before any code of this wave (two runs, identical), on Windows. The annulus generator calls the
    // platform's libm (sin, cos), so the hash differs per platform: the x86_64 Linux value was produced at 2f1a3ce (fee6ecc in
    // the current history) and at HEAD in one Linux container (ADR 0137); other platforms print it and skip.
    if cfg!(windows) {
        assert_eq!(h, 0x6eca7279950f43c3);
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        assert_eq!(h, 0xde77a7409c95de77);
    } else {
        eprintln!("T4: no recorded hash for this platform (printed above)");
    }
}

/// `[mesh]` body of the generator with extra keys.
fn tree_mesh(extra: &str) -> String {
    format!(
        "generator = \"annulus_3d_cylindrical_interface\"\nr_in = {RI}\nr_interface = 0.75\nr_out = {RO}\nlz = 0.4\n{extra}"
    )
}

/// K2 of ADR 0109 through the tree.
const K2_TREE: &str = "nx = 96\nny = 6\nnz = 4\nnx_right = 70\nny_right = 6\nnz_right = 6\ngrading = 3.0\n";

/// The same mesh through the library (ADR 0109 test code).
fn k2_library() -> Mesh {
    let a = annulus_3d(96, 6, 4, RI, 0.75, 0.4, 0.0, Grading::Expansion(3.0)).unwrap();
    let b = annulus_3d(70, 6, 6, 0.75, RO, 0.4, 0.0, Grading::Expansion(1.0 / 3.0)).unwrap();
    let (mut m, rep) =
        join_non_conformal_cylindrical(&a, "outer", &b, "inner", [0.0; 3], [0.0, 0.0, 1.0], 1e-8).unwrap();
    m.add_cell_zone("rotor", (0..rep.b_cell_offset).collect()).unwrap();
    m
}

/// ADR 0109 C2: Taylor-Couette, inner wall W_i = 1, outer at rest, MRF Omega = W_i on `rotor`, face_flux.
fn c2_text(mesh: &str) -> String {
    format!(
        "[mesh]\n{mesh}\n[physics]\nnu = 0.1\nrho = 1.0\n[boundaries.inner]\nvelocity = \"fixed\"\nvelocity_expression_x = \"-1.0 [m/s] * y / 1 [m]\"\nvelocity_expression_y = \"1.0 [m/s] * x / 1 [m]\"\n[boundaries.outer]\nvelocity = \"fixed\"\nvelocity_expression_x = \"-0.0 [m/s] * y / 1 [m]\"\nvelocity_expression_y = \"0.0 [m/s] * x / 1 [m]\"\n[boundaries.zmin]\nvelocity = \"symmetry\"\n[boundaries.zmax]\nvelocity = \"symmetry\"\n[solver]\nvelocity_correction = \"face_flux\"\nalgorithm = \"simplec\"\nconvection = \"linear_upwind\"\nnon_orthogonal_correctors = 1\nmax_iterations = 4000\ntolerance_u = 1e-10\ntolerance_p = 1e-10\n[sources.rotor]\ntype = \"mrf\"\nrotation_axis = [0.0, 0.0, 1.0]\nangular_velocity = 1.0\nzone = \"cell_zone\"\ncell_zone = \"rotor\"\n"
    )
}

fn case(text: &str) -> Case {
    Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}\n{text}"))
}

fn run(text: &str) -> (Simulation, usize) {
    let mut sim = Simulation::new(&case(text)).unwrap_or_else(|e| panic!("{e:?}"));
    let n = sim.run(4000);
    assert!(sim.failure().is_none(), "{:?}", sim.failure());
    println!(
        "  {n} iterations, converged {}, last {:?}",
        sim.is_converged(),
        sim.last()
    );
    (sim, n)
}

fn tc_error(sim: &Simulation) -> f64 {
    let cc = sim.geom.mesh.cell_centres();
    let (ux, uy) = (sim.field("ux").unwrap(), sim.field("uy").unwrap());
    let d = RO * RO - RI * RI;
    (0..cc.len())
        .map(|c| {
            let (x, y) = (cc[c][0], cc[c][1]);
            let r = x.hypot(y);
            ((-y * ux[c] + x * uy[c]) / r - (-RI * RI / d * r + RI * RI * RO * RO / d / r)).abs()
        })
        .fold(0.0, f64::max)
}

/// T1: the tree mesh equals the library mesh exactly.
#[test]
fn t1_tree_mesh_equals_the_library_join() {
    let tree = build_mesh(&case(&c2_text(&tree_mesh(K2_TREE)))).unwrap();
    let lib = k2_library();
    println!(
        "T1: tree {:#018x} library {:#018x}, {} cells",
        tree.content_hash(),
        lib.content_hash(),
        tree.n_cells()
    );
    assert_eq!(tree.content_hash(), lib.content_hash());
}

/// T2: the C2 run through the tree equals the run on the library mesh read as a file to 1e-12.
#[test]
fn t2_tree_run_equals_the_library_run() {
    let dir = std::env::temp_dir().join(format!("rcfd_w44b_t2_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    rcfd_io::openfoam::write_dir(&k2_library(), &dir).unwrap();
    let file = format!("source = \"file\"\nfile = {:?}", dir.to_string_lossy());
    println!("T2 library:");
    let (lib, n_lib) = run(&c2_text(&file));
    println!("T2 tree:");
    let (tree, n_tree) = run(&c2_text(&tree_mesh(K2_TREE)));
    let _ = std::fs::remove_dir_all(&dir);
    let mut worst: f64 = 0.0;
    for name in ["ux", "uy", "uz", "p"] {
        let (a, b) = (lib.field(name).unwrap(), tree.field(name).unwrap());
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(&b) {
            worst = worst.max((x - y).abs());
        }
    }
    println!(
        "T2: iterations {n_lib} / {n_tree}, max |difference| {worst:.3e} (1e-12), error {:.3e} V_i",
        tc_error(&tree) / V_I
    );
    assert_eq!(n_lib, n_tree);
    assert!(worst <= 1e-12, "{worst}");
}

/// Located error of a case: (path, message) of the first diagnostic.
fn located(text: &str) -> (String, String) {
    let e = match Simulation::new(&case(text)) {
        Ok(_) => panic!("accepted:\n{text}"),
        Err(e) => e,
    };
    (e[0].path.clone(), e[0].message.clone())
}

/// T3: located errors.
#[test]
fn t3_located_errors() {
    let base = "nx = 24\nny = 4\nnz = 2\n";
    let cases: [(String, &str, &[&str]); 7] = [
        (
            format!("generator = \"annulus_3d_cylindrical_interface\"\nr_in = {RI}\nr_out = {RO}\nlz = 0.4\n{base}"),
            "mesh.r_interface",
            &["between"],
        ),
        (
            tree_mesh(base).replace("r_interface = 0.75", "r_interface = 1.0"),
            "mesh.r_interface",
            &["between"],
        ),
        (
            tree_mesh(&format!("{base}isotropic_interface = true\ngrading = 3.0\n")),
            "mesh.grading",
            &["isotropic_interface"],
        ),
        (
            format!("generator = \"annulus_3d\"\nr_in = {RI}\nr_out = {RO}\nlz = 0.4\n{base}r_interface = 0.75\n"),
            "mesh.r_interface",
            &["does not use"],
        ),
        (
            format!(
                "generator = \"annulus_3d\"\nr_in = {RI}\nr_out = {RO}\nlz = 0.4\n{base}isotropic_interface = true\n"
            ),
            "mesh.isotropic_interface",
            &["does not use"],
        ),
        (
            tree_mesh("nx = 48\nny = 8\nnz = 2\nnx_right = 35\nny_right = 8\nnz_right = 3\n"),
            "mesh.r_interface",
            &["non-orthogonal", "factor"],
        ),
        (
            tree_mesh("nx = 48\nny = 1\nnz = 2\nnx_right = 35\nnz_right = 3\nisotropic_interface = true\n")
                .replace("r_in = 0.5", "r_in = 0.7"),
            "mesh.isotropic_interface",
            &["near-isotropic"],
        ),
    ];
    for (mesh, path, words) in cases {
        let (p, m) = located(&c2_text(&mesh));
        println!("T3 {path}: {p}: {m}");
        assert_eq!(p, path, "{m}");
        for w in words {
            assert!(m.contains(w), "{w}: {m}");
        }
    }
}

/// K2 counts with the near-isotropic interface layer instead of grading 3.
const T5_TREE: &str =
    "nx = 96\nny = 6\nnz = 4\nnx_right = 70\nny_right = 6\nnz_right = 6\nisotropic_interface = true\n";

/// T5: the option in the full-annulus solver.
#[test]
fn t5_isotropic_interface_option_converges_in_the_full_annulus() {
    let (sim, _) = run(&c2_text(&tree_mesh(T5_TREE)));
    let e = tc_error(&sim);
    println!("T5: converged {}, error {:.3e} V_i (2e-2)", sim.is_converged(), e / V_I);
    assert!(sim.is_converged());
    assert!(e <= 2e-2 * V_I, "{}", e / V_I);
}

/// Construction and 300-iteration smoke of T5 (rule d), not a criterion.
#[test]
#[ignore = "smoke diagnostic of ADR 0112; run on demand"]
fn smoke_w44b_t5() {
    let mut sim = Simulation::new(&case(&c2_text(&tree_mesh(T5_TREE)))).unwrap();
    println!("smoke T5: {} cells", sim.geom.mesh.n_cells());
    for _ in 0..3 {
        let n = sim.run(100);
        println!("  +{n}: {:?}", sim.last());
    }
}
