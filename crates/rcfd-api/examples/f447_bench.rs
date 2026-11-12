//! Data sets and server of the Electron performance spike (F-447, ADR 0350 amendment 1). A measurement aid, not a product.
//!
//! ```text
//! cargo run -p rcfd-api --release --example f447_bench -- gen <folder> [1e5|1e6|4e6 ...]
//! cargo run -p rcfd-api --release --example f447_bench -- serve <folder>
//! ```
//!
//! `gen` runs the lid-driven box of each size for 20 steps through the run path (`runctl::drive`) with the field store on
//! (`p, ux, uy, uz` in `f32`, raw blocks) into `<folder>/project/runs/box-<size>-raw/fields`, writes the deflated copy
//! `box-<size>-deflate` with `rcfd_tsstore::Writer`, and the reference bytes `<folder>/ref/<size>/<snapshot>-<field>.f32`
//! (`Reader::read_field` cast to `f32`, little-endian).
//!
//! `serve` starts a solver service confined to `<folder>` with the `data.*` methods on, prints one line
//! `{"rpc": port, "data": port, "events": file}` and writes `seq unix_ns` of every event to that file (an in-process
//! long-poll: the event log carries no time stamp). It ends when its standard input closes.

use rcfd_api::case::Case;
use rcfd_api::runctl::{self, RunOptions};
use rcfd_api::service::{Service, ServiceConfig};
use rcfd_api::sim::Simulation;
use rcfd_tsstore::{Codec, Location, Precision, Reader, WriteOptions, Writer};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Instant;

const FIELDS: [&str; 4] = ["p", "ux", "uy", "uz"];
const STEPS: usize = 20;

fn size_of(name: &str) -> [usize; 3] {
    match name {
        "1e5" => [50, 50, 40],
        "1e6" => [100, 100, 100],
        "4e6" => [200, 200, 100],
        "tiny" => [6, 5, 4],
        other => panic!("unknown size '{other}' (1e5, 1e6, 4e6, tiny)"),
    }
}

fn box_case(n: [usize; 3]) -> Case {
    let mut case = Case::new();
    let mut set = |path: &str, v: Value| case.set(path, v).unwrap_or_else(|e| panic!("{path}: {e:?}"));
    set("title", json!("F-447 lid-driven box"));
    set("mesh.generator", json!("box_3d"));
    set("mesh.nx", json!(n[0]));
    set("mesh.ny", json!(n[1]));
    set("mesh.nz", json!(n[2]));
    set("mesh.lx", json!(1.0));
    set("mesh.ly", json!(1.0));
    set("mesh.lz", json!(1.0));
    set("physics.nu", json!(0.01));
    for patch in ["xmin", "xmax", "ymin", "ymax", "zmin", "zmax"] {
        set(&format!("boundaries.{patch}.velocity"), json!("fixed"));
    }
    set("boundaries.ymax.velocity_value", json!([1.0, 0.0, 0.0]));
    set("solver.algorithm", json!("simplec"));
    set("solver.convection", json!("upwind"));
    set("output.store_interval", json!(1));
    set("output.store_fields", json!(FIELDS));
    set("output.store_precision", json!("f32"));
    case
}

fn f32_bytes(v: &[f64]) -> Vec<u8> {
    v.iter().flat_map(|x| (*x as f32).to_le_bytes()).collect()
}

fn generate(root: &Path, name: &str) {
    let n = size_of(name);
    let runs = root.join("project").join("runs");
    let raw_run = runs.join(format!("box-{name}-raw"));
    let raw = raw_run.join("fields");
    let deflate = runs.join(format!("box-{name}-deflate")).join("fields");
    let refs = root.join("ref").join(name);
    for d in [&raw, &deflate, &refs] {
        let _ = std::fs::remove_dir_all(d);
    }
    std::fs::create_dir_all(&refs).expect("reference folder");
    let t = Instant::now();
    let sim = Simulation::new(&box_case(n)).expect("the box case builds");
    eprintln!(
        "{name}: {} cells built in {:.1} s",
        n[0] * n[1] * n[2],
        t.elapsed().as_secs_f64()
    );
    let sim = Mutex::new(sim);
    let opts = RunOptions {
        max_steps: Some(STEPS),
        out_dir: Some(raw_run.clone()),
        ..Default::default()
    };
    let out = runctl::drive(&sim, &opts, &|| false, &mut |_| {});
    eprintln!(
        "{name}: run path {} after {} steps in {:.1} s ({})",
        out.state, out.iterations, out.elapsed_s, out.reason
    );
    drop(sim);
    let mut r = Reader::open(&raw).expect("the raw store opens");
    let snaps = r.snapshots().to_vec();
    assert_eq!(
        snaps.len(),
        STEPS,
        "{name}: the run path wrote {} snapshots",
        snaps.len()
    );
    let mesh = r.mesh().expect("the stored mesh");
    let opts = WriteOptions {
        codec: Codec::ShuffleDeflate,
        ..Default::default()
    };
    let mut w = Writer::create(&deflate, &mesh, opts).expect("the deflated store");
    let mut non_finite = 0usize;
    for (i, s) in snaps.iter().enumerate() {
        let mut snap = w.snapshot(s.step, s.time).expect("snapshot");
        for f in FIELDS {
            let info = r
                .fields(i)
                .expect("fields")
                .into_iter()
                .find(|x| x.name == f)
                .expect("field");
            assert_eq!((info.precision, info.codec), (Precision::F32, Codec::Raw));
            let v = r.read_field(i, f).expect("field values");
            non_finite += v.iter().filter(|x| !x.is_finite()).count();
            snap.field(f, &info.unit, Location::Cell, &v, Precision::F32)
                .expect("deflated field");
            std::fs::write(refs.join(format!("{i}-{f}.f32")), f32_bytes(&v)).expect("reference file");
        }
        snap.commit().expect("commit");
    }
    w.finish().expect("footer");
    let mut d = Reader::open(&deflate).expect("the deflated store opens");
    for i in 0..snaps.len() {
        for f in FIELDS {
            let (a, b) = (r.read_field(i, f).expect("raw"), d.read_field(i, f).expect("deflated"));
            assert!(
                a.len() == b.len() && a.iter().zip(&b).all(|(x, y)| x.to_bits() == y.to_bits()),
                "{name}: snapshot {i} field {f} differs between the raw and the deflated store"
            );
        }
    }
    let len = |p: &Path| std::fs::metadata(p.join("store.rts")).map_or(0, |m| m.len());
    eprintln!(
        "{name}: {} snapshots, non-finite values {non_finite}, raw {} bytes, deflated {} bytes, index {:?}",
        snaps.len(),
        len(&raw),
        len(&deflate),
        r.index_source()
    );
}

fn unix_ns() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos())
}

fn serve(root: &Path) {
    let root = std::fs::canonicalize(root).expect("the folder exists");
    let svc = Service::new(ServiceConfig {
        root: Some(root.clone()),
        max_cells: 5_000_000,
        event_capacity: 200_000,
        ..Default::default()
    });
    let data = svc.enable_data("127.0.0.1:0").expect("data listener");
    let tcp = svc.serve_tcp("127.0.0.1:0").expect("service listener");
    let events: PathBuf = root.join("events.log");
    let mut log = std::io::BufWriter::new(std::fs::File::create(&events).expect("event stamp file"));
    let observer = svc.clone();
    std::thread::spawn(move || {
        let mut after = 0u64;
        loop {
            let r = observer
                .call(
                    "events.poll",
                    &json!({"after": after, "timeout_ms": 1000, "limit": 1000}),
                )
                .expect("events.poll");
            let now = unix_ns();
            for e in r["events"].as_array().into_iter().flatten() {
                let _ = writeln!(log, "{} {} {now}", e["seq"], e["kind"].as_str().unwrap_or(""));
            }
            let _ = log.flush();
            after = r["next"].as_u64().unwrap_or(after);
        }
    });
    println!(
        "{}",
        json!({"rpc": tcp.addr.port(), "data": data.port(), "events": events.display().to_string(),
               "root": root.display().to_string()})
    );
    let _ = std::io::stdout().flush();
    let mut sink = Vec::new();
    let _ = std::io::stdin().read_to_end(&mut sink);
    svc.shutdown();
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("gen") if args.len() >= 2 => {
            let root = PathBuf::from(&args[1]);
            let sizes: Vec<&str> = if args.len() > 2 {
                args[2..].iter().map(String::as_str).collect()
            } else {
                vec!["1e5", "1e6", "4e6"]
            };
            for s in sizes {
                generate(&root, s);
            }
        }
        Some("serve") if args.len() == 2 => serve(Path::new(&args[1])),
        _ => {
            eprintln!("usage: f447_bench gen <folder> [sizes] | serve <folder>");
            std::process::exit(2);
        }
    }
}
