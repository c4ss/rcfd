//! Validation-data pack manifest (F-125): offline checks only. The test never reads the network,
//! never needs the downloaded `validation-data/data/` files and never touches a proprietary file.

use rcfd_io::datapack::{sha256_hex, FileState, Manifest};
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn load() -> Manifest {
    Manifest::load(&root().join("validation-data").join("manifest.json")).unwrap()
}

#[test]
fn manifest_is_valid_and_covers_the_roadmap_datasets() {
    let m = load();
    let problems = m.validate(&root());
    assert!(problems.is_empty(), "{problems:#?}");
    let ids: Vec<&str> = m.datasets.iter().map(|d| d.id.as_str()).collect();
    for want in [
        "cavity-ghia1982",
        "backward-step-tmr",
        "flat-plate-tmr",
        "naca0012-tmr",
        "dambreak-openfoam",
        "gri-mech-30",
        "nasa-rotor37",
        "hannover-compressor",
    ] {
        assert!(ids.contains(&want), "dataset {want} missing");
    }
    // every dataset carries a licence record and a citation; unresolved ones say why
    for d in &m.datasets {
        assert!(!d.licence.spdx.is_empty() && !d.citation.is_empty(), "{}", d.id);
        if d.status == "unresolved" {
            assert!(d.unresolved_reason.contains("EULA") || d.unresolved_reason.len() > 20);
            assert!(d.files.is_empty());
        } else {
            assert!(d.files.iter().all(|f| f.sha256.len() == 64));
        }
    }
    // downloaded files come from public hosts only
    for f in m
        .datasets
        .iter()
        .flat_map(|d| &d.files)
        .filter_map(|f| f.url.as_deref())
    {
        let host = f.split('/').nth(2).unwrap();
        assert!(
            ["raw.githubusercontent.com", "ntrs.nasa.gov", "combustion.berkeley.edu"].contains(&host),
            "unexpected host {host}"
        );
    }
    // GPL set-up files are fetched, never vendored
    let dam = m.datasets.iter().find(|d| d.id == "dambreak-openfoam").unwrap();
    assert_eq!(dam.licence.spdx, "GPL-3.0-only");
    assert!(dam.files.iter().all(|f| f.local.is_none()));
}

#[test]
fn in_repo_tables_match_their_recorded_checksums() {
    let m = load();
    let state = m.verify(&root(), &root().join("validation-data").join("data-does-not-exist"));
    let local: Vec<_> = state
        .iter()
        .filter(|(id, _, _)| id.starts_with("cavity") || id.starts_with("heated"))
        .collect();
    assert_eq!(local.len(), 3);
    for (id, name, s) in local {
        assert_eq!(*s, FileState::Ok, "{id}/{name}");
    }
    // fetched files are simply "not fetched" when the data directory is absent
    assert!(state.iter().any(|(_, _, s)| *s == FileState::Missing));
}

#[test]
fn validation_catches_tampering_and_proprietary_sources() {
    let good = load();
    let mut m = good.clone();
    m.datasets[0].files[0].sha256 = sha256_hex(b"other");
    assert!(m.validate(&root()).iter().any(|p| p.contains("checksum differs")));
    let mut m = good.clone();
    m.datasets[2].files[0].url = Some("https://example.org/ANSYS/doc.dat".into());
    assert!(m.validate(&root()).iter().any(|p| p.contains("only public upstreams")));
    let mut m = good.clone();
    m.datasets[2].files[0].url = Some("file:///C:/Program Files/x".into());
    let p = m.validate(&root());
    assert!(p.iter().any(|p| p.contains("file://")) && p.iter().any(|p| p.contains("url must be http")));
    let mut m = good.clone();
    m.datasets[2].licence.spdx.clear();
    assert!(m.validate(&root()).iter().any(|p| p.contains("licence record")));
    let mut m = good.clone();
    m.datasets[2].case = Some("nonexistent".into());
    assert!(m.validate(&root()).iter().any(|p| p.contains("does not exist")));
    let mut m = good.clone();
    m.datasets[2].files[0].sha256 = "xyz".into();
    assert!(m.validate(&root()).iter().any(|p| p.contains("64 lower-case hex")));
    let mut m = good;
    m.datasets[0].files[0].local = Some("../../etc/passwd".into());
    assert!(m.validate(&root()).iter().any(|p| p.contains("repo-relative")));
}

#[test]
fn provenance_report_lists_every_dataset_with_licence_and_checksums() {
    let m = load();
    let md = m.provenance_markdown();
    for d in &m.datasets {
        assert!(md.contains(&d.id) && md.contains(&d.licence.spdx), "{}", d.id);
        for f in &d.files {
            assert!(md.contains(&f.sha256), "{} checksum missing from the report", f.name());
        }
    }
    assert!(md.contains("unresolved"));
}
