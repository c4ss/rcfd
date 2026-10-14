//! Wave-11 bug: mesh generators silently ignored documented parameters (`cavity_2d` ignored `mesh.ny`
//! and `mesh.ly`). Documented meaning (docs/case-schema.md, schema docs): `nx`/`ny` are the cells in
//! x/y and `lx`/`ly` the lengths, so a cavity is an `nx` x `ny` grid on `lx` x `ly`; parameters a
//! generator cannot honour are rejected with a located diagnostic instead of being ignored.

use rcfd_api::case::Case;
use rcfd_api::sim::build_mesh;

fn mesh_of(text: &str) -> Result<rcfd_mesh::Mesh, Vec<rcfd_api::case::Diagnostic>> {
    let case = Case::parse_toml(text).expect("case parses");
    build_mesh(&case)
}

#[test]
fn cavity_2d_honours_ny() {
    let m = mesh_of("[mesh]\ngenerator = \"cavity_2d\"\nnx = 20\nny = 2\n").unwrap();
    assert_eq!(m.n_cells(), 40, "nx = 20, ny = 2 is 40 cells");
}

#[test]
fn cavity_2d_honours_lx_ly() {
    let m = mesh_of("[mesh]\ngenerator = \"cavity_2d\"\nnx = 8\nny = 4\nlx = 2.0\nly = 0.5\n").unwrap();
    let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
    for p in m.points() {
        for d in 0..3 {
            lo[d] = lo[d].min(p[d]);
            hi[d] = hi[d].max(p[d]);
        }
    }
    assert!((hi[0] - lo[0] - 2.0).abs() < 1e-12 && (hi[1] - lo[1] - 0.5).abs() < 1e-12);
    assert_eq!(m.n_cells(), 32);
}

#[test]
fn square_cavity_unchanged() {
    let m = mesh_of("[mesh]\ngenerator = \"cavity_2d\"\nnx = 12\nny = 12\n").unwrap();
    assert_eq!(m.n_cells(), 144);
}

fn rejects(text: &str, path: &str) {
    let err = match mesh_of(text) {
        Ok(_) => panic!("expected a diagnostic at {path}"),
        Err(e) => e,
    };
    assert!(err.iter().any(|d| d.path == path), "{err:?}");
}

#[test]
fn unsupported_mesh_options_are_rejected_not_ignored() {
    // wall layers exist for box_2d and channel_2d only
    rejects(
        "[mesh]\ngenerator = \"wedge_2d\"\nwall_layers = 4\nfirst_layer_height = 0.01\n",
        "mesh.wall_layers",
    );
    rejects(
        "[mesh]\ngenerator = \"nozzle_2d\"\nwall_layers = 4\nfirst_layer_height = 0.01\n",
        "mesh.wall_layers",
    );
    // periodic flags exist for box_2d only
    rejects(
        "[mesh]\ngenerator = \"channel_2d\"\nperiodic_x = true\n",
        "mesh.periodic_x",
    );
    rejects(
        "[mesh]\ngenerator = \"cavity_2d\"\nperiodic_y = true\n",
        "mesh.periodic_y",
    );
    // grading is not applied by these generators
    rejects("[mesh]\ngenerator = \"cavity_2d\"\ngrading = 3.0\n", "mesh.grading");
    rejects("[mesh]\ngenerator = \"wedge_2d\"\ngrading = 3.0\n", "mesh.grading");
    rejects("[mesh]\ngenerator = \"nozzle_2d\"\ngrading = 3.0\n", "mesh.grading");
}
