//! ADR 0737 P3 through the `rcfd` binary, with its change of 2026-11-17: `rcfd check <case> --refine 2` of a mesh-file
//! case exits 3 (as before) and prints the reason and, under it, the error catalogue's line of the service's 1009
//! `MESH_FILE` (exit 3's own line is about invalid keys); a generator case prints the edits and exits 0.

use std::process::Command;

const PISO: &str = r#"title = "piso cavity"
[mesh]
generator = "cavity_2d"
nx = 8
ny = 8
[physics]
nu = 0.01
[boundaries.lid]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.walls]
velocity = "fixed"
[solver]
algorithm = "piso"
dt = 0.01
end_time = 1.0
"#;

#[test]
fn check_refine_of_a_mesh_file_case_names_the_catalogue_line() {
    let d = std::env::temp_dir().join(format!("rcfd_check_refine_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let file = d.join("file.toml");
    std::fs::write(
        &file,
        PISO.replace(
            "generator = \"cavity_2d\"\nnx = 8\nny = 8\n",
            "source = \"file\"\nfile = \"m.msh\"\n",
        ),
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_rcfd"))
        .args(["check", file.to_str().unwrap(), "--refine", "2"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(3));
    assert!(out.stdout.is_empty());
    let err = String::from_utf8(out.stderr).unwrap();
    let line = rcfd_api::errors::lookup_rpc(1009).unwrap();
    assert_eq!(line.name, "MESH_FILE");
    assert!(
        err.contains("Refine x2: the resolution is set by the mesh file m.msh"),
        "{err}"
    );
    assert!(err.contains(line.recovery), "{err}");

    let gen = d.join("gen.toml");
    std::fs::write(&gen, PISO).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_rcfd"))
        .args(["check", gen.to_str().unwrap(), "--refine", "2"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        (v["cells"].clone(), v["cells_after"].clone()),
        (serde_json::json!(64), serde_json::json!(256))
    );
    std::fs::remove_dir_all(&d).ok();
}
