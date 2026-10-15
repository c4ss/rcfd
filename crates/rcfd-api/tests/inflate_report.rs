//! Inflation report through the API (ADR 0021, checks N1 to N6, library level).

mod wiring_common;

use rcfd_api::case::Case;
use rcfd_api::inflate::{inflate, InflateError, InflateRequest};
use rcfd_api::service::{Service, ServiceConfig};
use rcfd_api::sim::build_mesh;
use rcfd_mesh::inflate::{inflate_patch, LayerSizing};
use rcfd_mesh::layers::first_cell_height_from_y_plus;
use serde_json::json;

const CASE: &str = "[mesh]\ngenerator = \"box_2d\"\nnx = 8\nny = 4\nlx = 2.0\nly = 1.0\n";

fn mesh() -> rcfd_mesh::Mesh {
    build_mesh(&Case::parse_toml(CASE).unwrap()).unwrap()
}

fn req(layers: usize) -> InflateRequest {
    InflateRequest {
        patch: "ymax".into(),
        layers,
        first: Some(0.01),
        growth: Some(1.2),
        ..Default::default()
    }
}

#[test]
fn n1_report_equals_the_library_report() {
    let m = mesh();
    let (new, doc) = inflate(&m, &req(6)).unwrap();
    let sizing = LayerSizing::FirstGrowth {
        first: 0.01,
        growth: 1.2,
        layers: 6,
    };
    let (lib_mesh, lib) = inflate_patch(&m, "ymax", sizing).unwrap();
    assert_eq!(doc["report"], serde_json::to_value(&lib).unwrap());
    assert_eq!(doc["sizing"], serde_json::to_value(sizing).unwrap());
    assert_eq!(doc["patch"], "ymax");
    assert_eq!(new.n_cells(), lib_mesh.n_cells());
    // N2: eight patch faces, six layers
    assert_eq!(doc["cells_before"], m.n_cells());
    assert_eq!(
        doc["cells_after"].as_u64().unwrap() - doc["cells_before"].as_u64().unwrap(),
        8 * 6
    );
    for k in ["patch", "sizing", "cells_before", "cells_after", "report"] {
        assert!(doc.get(k).is_some(), "{k}");
    }
}

#[test]
fn n3_y_plus_target_gives_the_first_layer_height() {
    let m = mesh();
    let r = InflateRequest {
        patch: "ymax".into(),
        layers: 4,
        y_plus: Some((0.8, 0.05, 1e-5)),
        ..Default::default()
    };
    let (_, doc) = inflate(&m, &r).unwrap();
    let want = first_cell_height_from_y_plus(0.8, 0.05, 1e-5);
    assert!((want - 3.2e-4).abs() / 3.2e-4 <= 1e-12, "{want}");
    for k in ["first_height_min", "first_height_max"] {
        let got = doc["report"][k].as_f64().unwrap();
        assert!((got - want).abs() / want <= 1e-12, "{k} {got} vs {want}");
    }
}

#[test]
fn n5_errors() {
    let m = mesh();
    let mut r = req(6);
    r.patch = "nowhere".into();
    match inflate(&m, &r) {
        Err(InflateError::Library(msg)) => assert!(msg.contains("nowhere"), "{msg}"),
        other => panic!("{other:?}"),
    }
    assert!(matches!(inflate(&m, &req(0)), Err(InflateError::Usage(_))));
    let none = InflateRequest {
        patch: "ymax".into(),
        layers: 3,
        ..Default::default()
    };
    assert!(matches!(inflate(&m, &none), Err(InflateError::Usage(_))));
    let mut zero = req(3);
    zero.growth = Some(0.0);
    assert!(matches!(inflate(&m, &zero), Err(InflateError::Library(_))));
}

#[test]
fn n6_service_method_equals_the_direct_report() {
    let svc = Service::new(ServiceConfig::default());
    let id = svc.call("session.create", &json!({ "toml": CASE })).unwrap()["session"]
        .as_str()
        .unwrap()
        .to_string();
    let r = svc
        .call(
            "mesh.inflate",
            &json!({"session": id, "patch": "ymax", "layers": 6, "first": 0.01, "growth": 1.2}),
        )
        .unwrap();
    let (_, doc) = inflate(&mesh(), &req(6)).unwrap();
    assert_eq!(r, doc);
    assert!(svc
        .call(
            "mesh.inflate",
            &json!({"session": id, "patch": "nowhere", "layers": 6, "first": 0.01})
        )
        .is_err());
}
