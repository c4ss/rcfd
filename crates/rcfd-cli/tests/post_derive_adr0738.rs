//! ADR 0738 (F-809) P4: `rcfd post derive <store dir> ... --out <file.glb>` writes the file `rcfd_workspace::derive`
//! makes (the library of the project tool `derive_picture`, G16), prints its reply as one JSON line, exits 2 on a usage
//! error and 1 with the error as JSON when the picture cannot be made.

use rcfd_api::service::{Service, ServiceConfig};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::Command;

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd-cli-adr0738-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d.canonicalize().unwrap()
}

/// A field store of the service's run path: a 2D cavity of 10 x 10 cells, 20 steps.
fn store(root: &std::path::Path) -> PathBuf {
    let svc = Service::new(ServiceConfig {
        root: Some(root.to_path_buf()),
        ..Default::default()
    });
    let toml = "[mesh]\ngenerator = \"cavity_2d\"\nnx = 10\nny = 10\n[physics]\nnu = 0.01\n[boundaries.lid]\nvelocity = \"fixed\"\nvelocity_value = [1.0, 0.0, 0.0]\n[boundaries.walls]\nvelocity = \"fixed\"\n[solver]\nmax_iterations = 20\n[output]\nstore_interval = 10\n";
    let s = svc.call("session.create", &json!({"toml": toml})).unwrap()["session"]
        .as_str()
        .unwrap()
        .to_string();
    svc.call(
        "run.headless",
        &json!({"session": s, "max_steps": 20, "out_dir": "out"}),
    )
    .unwrap();
    let st = root.join("out/fields");
    assert!(st.join("store.rts").is_file());
    st
}

fn rcfd(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_rcfd")).args(args).output().unwrap()
}

#[test]
fn p4_post_derive_writes_the_libraries_file() {
    let root = scratch("p4");
    let st = store(&root);
    let s = st.to_string_lossy().to_string();
    let cases: [(&[&str], Value); 3] = [
        (
            &[
                "--kind",
                "cut_surface",
                "--field",
                "p",
                "--axis",
                "x",
                "--at",
                "0.05",
                "--keep",
                "above",
                "--vectors",
            ],
            json!({"kind": "cut_surface", "field": "p", "axis": "x", "at": 0.05, "keep": "above", "vectors": true}),
        ),
        (
            &[
                "--kind",
                "streamlines",
                "--rake",
                "0.02,0.05,0.0005,0.08,0.05,0.0005,5",
                "--max-steps",
                "300",
                "--snapshot",
                "0",
            ],
            json!({"kind": "streamlines", "seeds": {"rake": {"p0": [0.02, 0.05, 0.0005], "p1": [0.08, 0.05, 0.0005], "n": 5}}, "max_steps": 300, "snapshot": 0}),
        ),
        (
            &["--kind", "slice", "--field", "speed", "--axis", "y", "--at", "0.05"],
            json!({"kind": "slice", "field": "speed", "axis": "y", "at": 0.05}),
        ),
    ];
    for (i, (flags, args)) in cases.iter().enumerate() {
        let out = root.join(format!("p{i}.glb"));
        let o = out.to_string_lossy().to_string();
        let mut a = vec!["post", "derive", &s];
        a.extend_from_slice(flags);
        a.extend_from_slice(&["--out", &o]);
        let r = rcfd(&a);
        assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
        let reply: Value = serde_json::from_slice(&r.stdout).unwrap();
        let want = rcfd_workspace::derive::derive_dir(&st, args).unwrap();
        let got = std::fs::read(&out).unwrap();
        assert!(got == want.glb, "case {i}: the file is the library's");
        assert_eq!(reply["length_unit"], "m");
        assert_eq!(reply["bytes"], got.len());
        assert_eq!(reply["kind"], args["kind"]);
        rcfd_post::glb_check::validate_glb(&got).unwrap();
    }
    // a usage error, and a picture that cannot be made
    assert_eq!(rcfd(&["post", "derive", &s, "--kind", "iso"]).status.code(), Some(2));
    let o = root.join("x.glb").to_string_lossy().to_string();
    let r = rcfd(&[
        "post", "derive", &s, "--kind", "iso", "--field", "p", "--level", "1e12", "--out", &o,
    ]);
    assert_eq!(r.status.code(), Some(1));
    let e: Value = serde_json::from_slice(&r.stderr).unwrap();
    assert_eq!(e["error"]["code"], 1003);
    // `rcfd post <results dir>` is unchanged: a folder without monitors.json fails as before
    assert_eq!(rcfd(&["post", &root.to_string_lossy()]).status.code(), Some(1));
    eprintln!("P4: rcfd post derive wrote the library's file for 3 kinds; exit 2 on usage, 1 with the error as JSON");
}
