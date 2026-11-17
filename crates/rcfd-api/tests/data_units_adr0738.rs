//! ADR 0738 (F-809) P5 and E-809d, the data channel's part.
//!
//! - U1: `data.open` names the units of the store, `time_unit: "s"` (ADR 0734 P2) and `length_unit: "m"`; the store's
//!   bytes are unchanged by it (the reply is built from the reader alone). D1 to D3 of `data_service_f447.rs` pass
//!   unedited, and `DATA_METHODS` is unchanged (D2).
//! - G2 (the surface): `rcfd_post::picture::store_surface`, the surface the engine's cut and arrows are made on, is the
//!   surface `data.surface` hands the Studio, byte for byte, on a box and on a 2D cavity.

use rcfd_api::data_service::{redeem, DATA_METHODS};
use rcfd_api::service::{Service, ServiceConfig};
use rcfd_mesh::gen::{box_mesh, cavity_2d, Grading};
use rcfd_mesh::Mesh;
use rcfd_tsstore::{Codec, Location, Precision, WriteOptions, Writer};
use serde_json::json;
use std::net::TcpStream;
use std::path::{Path, PathBuf};

fn temp(name: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir()
        .join("rcfd-api-tests")
        .join(format!("adr0738-{name}-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    root.canonicalize().unwrap()
}

fn write_store(dir: &Path, mesh: &Mesh) {
    let mut w = Writer::create(
        dir,
        mesh,
        WriteOptions {
            block_values: 64,
            codec: Codec::Raw,
        },
    )
    .unwrap();
    let mut s = w.snapshot(1, 0.5).unwrap();
    let v: Vec<f64> = (0..mesh.n_cells()).map(|c| c as f64).collect();
    s.field("p", "Pa", Location::Cell, &v, Precision::F32).unwrap();
    s.commit().unwrap();
    w.finish().unwrap();
}

#[test]
fn u1_data_open_names_the_units_and_g2_the_surface_is_the_engines() {
    assert_eq!(DATA_METHODS, ["data.info", "data.open", "data.field", "data.surface"]);
    let root = temp("u1");
    let svc = Service::new(ServiceConfig {
        root: Some(root.clone()),
        ..Default::default()
    });
    let addr = svc.enable_data("127.0.0.1:0").unwrap();
    let mut stream = TcpStream::connect(addr).unwrap();
    let meshes = [
        (
            "box",
            box_mesh(
                [6, 5, 4],
                [1.0, 2.0, 3.0],
                [Grading::Expansion(2.0), Grading::Uniform, Grading::Uniform],
            )
            .unwrap(),
        ),
        ("cavity", cavity_2d(9, 0.1).unwrap()),
    ];
    for (name, mesh) in &meshes {
        let dir = root.join(name);
        write_store(&dir, mesh);
        let before = std::fs::read(dir.join("store.rts")).unwrap();
        let open = svc.call("data.open", &json!({"path": name})).unwrap();
        assert_eq!(open["length_unit"], "m");
        assert_eq!(open["time_unit"], "s");
        assert_eq!(open["length_unit"], rcfd_tsstore::LENGTH_UNIT);
        assert_eq!(
            std::fs::read(dir.join("store.rts")).unwrap(),
            before,
            "the store's bytes"
        );
        let r = svc.call("data.surface", &json!({"path": name})).unwrap();
        let got = redeem(&mut stream, r["ticket"].as_u64().unwrap(), 3).unwrap();
        let s = rcfd_post::picture::store_surface(mesh).unwrap();
        let f32b: Vec<u8> = s.points.iter().flat_map(|x| x.to_le_bytes()).collect();
        let u32b = |v: &[u32]| -> Vec<u8> { v.iter().flat_map(|x| x.to_le_bytes()).collect() };
        assert_eq!(got[0], f32b, "{name}: points");
        assert_eq!(got[1], u32b(&s.triangles), "{name}: triangles");
        assert_eq!(got[2], u32b(&s.cells), "{name}: cells");
        eprintln!(
            "U1/G2 {name}: data.open length_unit m, time_unit s; data.surface = store_surface ({} triangles)",
            s.n_triangles()
        );
    }
}
