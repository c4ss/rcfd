//! GLB structural validator (F-117, ADR 0011 section 4): one negative test per rule G1-G8 and a positive test on a
//! GLB that was not produced by rcfd's writer.

use rcfd_mesh::gen::{box_mesh, Grading};
use rcfd_post::glb_check::validate_glb;
use rcfd_post::gltf::{glb_patches, GltfOptions, GltfScalar};
use serde_json::{json, Value};

fn valid() -> Vec<u8> {
    let m = box_mesh([3, 3, 3], [1.0; 3], [Grading::Uniform; 3]).unwrap();
    let x: Vec<f64> = m.cell_centres().iter().map(|c| c[0]).collect();
    glb_patches(
        &m,
        &["xmin", "xmax", "zmax"],
        Some(GltfScalar::Cell(&x)),
        &GltfOptions {
            range: None,
            scalar_name: "x".into(),
        },
    )
    .unwrap()
    .0
}

/// Split a GLB into (JSON value, BIN bytes).
fn split(b: &[u8]) -> (Value, Vec<u8>) {
    let jl = u32::from_le_bytes(b[12..16].try_into().unwrap()) as usize;
    let doc: Value = serde_json::from_slice(&b[20..20 + jl]).unwrap();
    let bl = u32::from_le_bytes(b[20 + jl..24 + jl].try_into().unwrap()) as usize;
    (doc, b[28 + jl..28 + jl + bl].to_vec())
}

fn join(doc: &Value, bin: &[u8]) -> Vec<u8> {
    let mut js = serde_json::to_vec(doc).unwrap();
    while js.len() % 4 != 0 {
        js.push(b' ');
    }
    let mut bin = bin.to_vec();
    while bin.len() % 4 != 0 {
        bin.push(0);
    }
    let total = 12 + 8 + js.len() + 8 + bin.len();
    let mut o = b"glTF".to_vec();
    o.extend_from_slice(&2u32.to_le_bytes());
    o.extend_from_slice(&(total as u32).to_le_bytes());
    o.extend_from_slice(&(js.len() as u32).to_le_bytes());
    o.extend_from_slice(b"JSON");
    o.extend_from_slice(&js);
    o.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    o.extend_from_slice(b"BIN\0");
    o.extend_from_slice(&bin);
    o
}

/// Apply `f` to the JSON (and the BIN bytes) of the valid file and expect an error at `pointer` (prefix match).
fn expect(pointer: &str, f: impl FnOnce(&mut Value, &mut Vec<u8>)) {
    let (mut d, mut b) = split(&valid());
    f(&mut d, &mut b);
    let e = validate_glb(&join(&d, &b)).expect_err(pointer);
    assert!(e.pointer.starts_with(pointer), "expected pointer {pointer}, got {e}");
}

fn expect_raw(pointer: &str, bytes: Vec<u8>) {
    let e = validate_glb(&bytes).expect_err(pointer);
    assert!(e.pointer.starts_with(pointer), "expected pointer {pointer}, got {e}");
}

#[test]
fn writer_output_passes_every_rule() {
    validate_glb(&valid()).unwrap();
}

#[test]
fn g1_container_rules() {
    let v = valid();
    let mut b = v.clone();
    b[0] = b'x';
    expect_raw("@0", b);
    let mut b = v.clone();
    b[4] = 1;
    expect_raw("@4", b);
    let mut b = v.clone();
    b[8] = b[8].wrapping_add(4);
    expect_raw("@8", b);
    let mut b = v.clone();
    b[12] = b[12].wrapping_add(1); // JSON chunk length not a multiple of 4
    expect_raw("@12", b);
    let mut b = v.clone();
    b[16..20].copy_from_slice(b"BIN\0");
    expect_raw("@16", b);
    // trailing bytes: an extra chunk header that runs past the end
    let mut b = v.clone();
    b.extend_from_slice(&[1, 2, 3, 4]);
    let l = b.len() as u32;
    b[8..12].copy_from_slice(&l.to_le_bytes());
    expect_raw("@", b);
    // BIN chunk longer than the buffer by more than 3 bytes
    expect("/buffers/0/byteLength", |d, _| {
        let l = d["buffers"][0]["byteLength"].as_u64().unwrap();
        d["buffers"][0]["byteLength"] = json!(l - 8);
    });
    expect("/asset/version", |d, _| d["asset"]["version"] = json!("1.0"));
    expect("/extensionsRequired", |d, _| {
        d["extensionsRequired"] = json!(["KHR_unknown"])
    });
}

#[test]
fn g2_alignment_stride_and_extent() {
    expect("/accessors/0/byteOffset", |d, _| {
        d["accessors"][0]["byteOffset"] = json!(2)
    });
    expect("/bufferViews/0/byteStride", |d, _| {
        d["bufferViews"][0]["byteStride"] = json!(5)
    });
    expect("/bufferViews/0/byteStride", |d, _| {
        d["bufferViews"][0]["byteStride"] = json!(256)
    });
    // stride smaller than the element size (VEC3 float = 12)
    expect("/bufferViews/0/byteStride", |d, _| {
        d["bufferViews"][0]["byteStride"] = json!(8)
    });
    // extent beyond the view
    expect("/accessors/0", |d, _| {
        let c = d["accessors"][0]["count"].as_u64().unwrap();
        d["accessors"][0]["count"] = json!(c + 1);
    });
}

#[test]
fn g3_views_and_accessor_fields() {
    expect("/bufferViews/0", |d, _| {
        d["bufferViews"][0]["byteOffset"] = json!(1_000_000)
    });
    expect("/bufferViews/1/byteLength", |d, _| {
        d["bufferViews"][1]["byteLength"] = json!(0)
    });
    expect("/accessors/0/count", |d, _| d["accessors"][0]["count"] = json!(0));
    expect("/accessors/0/normalized", |d, _| {
        d["accessors"][0]["normalized"] = json!(true)
    });
    expect("/accessors/0/componentType", |d, _| {
        d["accessors"][0]["componentType"] = json!(5124)
    });
    expect("/accessors/0/type", |d, _| d["accessors"][0]["type"] = json!("MAT4"));
    expect("/bufferViews/0/buffer", |d, _| d["bufferViews"][0]["buffer"] = json!(3));
}

#[test]
fn g4_min_max() {
    expect("/accessors/0/min/0", |d, _| d["accessors"][0]["min"][0] = json!(-5.0));
    expect("/accessors/0/max/1", |d, _| d["accessors"][0]["max"][1] = json!(99.0));
    expect("/accessors/0/min", |d, _| {
        d["accessors"][0].as_object_mut().unwrap().remove("max");
    });
    expect("/accessors/0/min/0", |d, _| {
        d["accessors"][0]["min"][0] = json!(10.0);
        d["accessors"][0]["max"][0] = json!(-10.0);
    });
    expect("/accessors/0/min", |d, _| d["accessors"][0]["min"] = json!([0.0, 0.0]));
    // POSITION without min and max
    let (mut d, b) = split(&valid());
    d["accessors"][0].as_object_mut().unwrap().remove("min");
    d["accessors"][0].as_object_mut().unwrap().remove("max");
    let e = validate_glb(&join(&d, &b)).unwrap_err();
    assert!(e.pointer.starts_with("/accessors/0/min"), "{e}");
}

fn first_prim(d: &Value) -> Value {
    d["meshes"][0]["primitives"][0].clone()
}

#[test]
fn g5_indices_and_modes() {
    let (d0, _) = split(&valid());
    let ia = first_prim(&d0)["indices"].as_u64().unwrap() as usize;
    let ptr = "/meshes/0/primitives/0/indices";
    // an index beyond the vertex count (data edit): first index of that accessor
    expect(ptr, |d, b| {
        let view = d["accessors"][ia]["bufferView"].as_u64().unwrap() as usize;
        let o = d["bufferViews"][view]["byteOffset"].as_u64().unwrap() as usize;
        b[o..o + 4].copy_from_slice(&1_000_000u32.to_le_bytes());
        // min/max are checked first, so drop them to reach the index range rule
        d["accessors"][ia].as_object_mut().unwrap().remove("min");
        d["accessors"][ia].as_object_mut().unwrap().remove("max");
    });
    // float indices
    expect(ptr, |d, _| {
        d["accessors"][ia]["componentType"] = json!(5126);
        d["accessors"][ia].as_object_mut().unwrap().remove("min");
        d["accessors"][ia].as_object_mut().unwrap().remove("max");
    });
    // triangle mode with a count that is not a multiple of 3
    expect(ptr, |d, _| {
        let c = d["accessors"][ia]["count"].as_u64().unwrap();
        d["accessors"][ia]["count"] = json!(c - 1);
        d["accessors"][ia].as_object_mut().unwrap().remove("min");
        d["accessors"][ia].as_object_mut().unwrap().remove("max");
    });
    expect("/meshes/0/primitives/0/mode", |d, _| {
        d["meshes"][0]["primitives"][0]["mode"] = json!(9)
    });
}

/// Hand-built GLB (not made by rcfd's writer): `n` vertices, indices of the requested type.
fn hand_built(n: usize, ind_type: u64, indices: &[u32]) -> Vec<u8> {
    let mut bin = Vec::new();
    for i in 0..n {
        for c in [i as f32, (i * i % 7) as f32, 0.0] {
            bin.extend_from_slice(&c.to_le_bytes());
        }
    }
    let pos_len = bin.len();
    let ioff = bin.len();
    for &i in indices {
        match ind_type {
            5121 => bin.push(i as u8),
            5123 => bin.extend_from_slice(&(i as u16).to_le_bytes()),
            _ => bin.extend_from_slice(&i.to_le_bytes()),
        }
    }
    let ilen = bin.len() - ioff;
    while bin.len() % 4 != 0 {
        bin.push(0);
    }
    let (mut mn, mut mx) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
    for i in 0..n {
        let p = [i as f64, (i * i % 7) as f64, 0.0];
        for k in 0..3 {
            mn[k] = mn[k].min(p[k]);
            mx[k] = mx[k].max(p[k]);
        }
    }
    let doc = json!({
        "asset": {"version": "2.0"},
        "scene": 0, "scenes": [{"nodes": [0]}], "nodes": [{"mesh": 0}],
        "meshes": [{"primitives": [{"attributes": {"POSITION": 0}, "indices": 1, "mode": 4}]}],
        "buffers": [{"byteLength": bin.len()}],
        "bufferViews": [
            {"buffer": 0, "byteOffset": 0, "byteLength": pos_len, "target": 34962},
            {"buffer": 0, "byteOffset": ioff, "byteLength": ilen, "target": 34963}],
        "accessors": [
            {"bufferView": 0, "componentType": 5126, "count": n, "type": "VEC3", "min": mn, "max": mx},
            {"bufferView": 1, "componentType": ind_type, "count": indices.len(), "type": "SCALAR"}],
    });
    join(&doc, &bin)
}

#[test]
fn hand_built_glb_passes_and_restart_value_is_rejected() {
    validate_glb(&hand_built(300, 5123, &[0, 1, 2, 3, 4, 299])).unwrap();
    validate_glb(&hand_built(300, 5121, &[0, 1, 2])).unwrap();
    let e = validate_glb(&hand_built(300, 5121, &[0, 1, 255])).unwrap_err();
    assert!(e.to_string().contains("restart"), "{e}");
    let e = validate_glb(&hand_built(300, 5123, &[0, 1, 300])).unwrap_err();
    assert!(e.pointer.ends_with("/indices"), "{e}");
    // count not matching the mode
    assert!(validate_glb(&hand_built(300, 5125, &[0, 1])).is_err());
}

#[test]
fn g6_attributes() {
    let (d0, _) = split(&valid());
    let p = first_prim(&d0);
    let nrm = p["attributes"]["NORMAL"].as_u64().unwrap() as usize;
    let pos = p["attributes"]["POSITION"].as_u64().unwrap() as usize;
    let col = p["attributes"]["COLOR_0"].as_u64().unwrap() as usize;
    // non-unit normal
    expect("/meshes/0/primitives/0/attributes/NORMAL", |d, b| {
        let view = d["accessors"][nrm]["bufferView"].as_u64().unwrap() as usize;
        let o = d["bufferViews"][view]["byteOffset"].as_u64().unwrap() as usize;
        b[o..o + 4].copy_from_slice(&3.0f32.to_le_bytes());
    });
    // POSITION as VEC2
    expect("/meshes/0/primitives/0/attributes/POSITION", |d, _| {
        d["accessors"][pos]["type"] = json!("VEC2");
        d["accessors"][pos]["min"] = json!([0.0, 0.0]);
        d["accessors"][pos]["max"] = json!([1.0, 1.0]);
    });
    // COLOR_0 as plain unsigned byte without normalisation
    expect("/meshes/0/primitives/0/attributes/COLOR_0", |d, _| {
        d["accessors"][col].as_object_mut().unwrap().remove("normalized");
    });
    // attribute count differs
    expect("/meshes/0/primitives/0/attributes/NORMAL", |d, _| {
        d["accessors"][nrm]["count"] = json!(3)
    });
    // NaN in the position data
    expect(&format!("/accessors/{pos}"), |d, b| {
        let view = d["accessors"][pos]["bufferView"].as_u64().unwrap() as usize;
        let o = d["bufferViews"][view]["byteOffset"].as_u64().unwrap() as usize;
        b[o..o + 4].copy_from_slice(&f32::NAN.to_le_bytes());
    });
}

#[test]
fn g7_references() {
    expect("/meshes/0/primitives/0/material", |d, _| {
        d["meshes"][0]["primitives"][0]["material"] = json!(7)
    });
    expect("/nodes/0/mesh", |d, _| d["nodes"][0]["mesh"] = json!(4));
    expect("/scene", |d, _| d["scene"] = json!(3));
    expect("/scenes/0/nodes/0", |d, _| d["scenes"][0]["nodes"] = json!([9]));
    // node 0 and 1 are each other's child: each has a parent, no root: cycle
    expect("/nodes", |d, _| {
        d["nodes"] = json!([{"children": [1]}, {"children": [0]}]);
        d["scenes"] = json!([{"nodes": []}]);
    });
    expect("/nodes/0/children/0", |d, _| d["nodes"][0]["children"] = json!([0]));
    expect("/nodes/1/children/0", |d, _| {
        d["nodes"] = json!([{"children": [2]}, {"children": [2]}, {}]);
        d["scenes"] = json!([{"nodes": [0, 1]}]);
    });
    expect("/accessors/0/bufferView", |d, _| {
        d["accessors"][0]["bufferView"] = json!(99)
    });
    expect("/accessors/0/sparse", |d, _| d["accessors"][0]["sparse"] = json!({}));
}

#[test]
fn g8_hostile_input_never_panics() {
    use rcfd_io::guard::fuzz_no_panic;
    let m = box_mesh([2, 2, 2], [1.0; 3], [Grading::Uniform; 3]).unwrap();
    let x: Vec<f64> = m.cell_centres().iter().map(|c| c[0]).collect();
    let small = glb_patches(&m, &["xmin"], Some(GltfScalar::Cell(&x)), &GltfOptions::default())
        .unwrap()
        .0;
    fuzz_no_panic(&small, 3000, |b| {
        let _ = validate_glb(b);
    });
    // semantic fuzz: replace one numeric leaf of the JSON by hostile values and re-wrap
    let (doc, bin) = split(&small);
    let mut leaves: Vec<Vec<String>> = Vec::new();
    fn walk(v: &Value, path: &mut Vec<String>, out: &mut Vec<Vec<String>>) {
        match v {
            Value::Number(_) => out.push(path.clone()),
            Value::Array(a) => {
                for (i, x) in a.iter().enumerate() {
                    path.push(i.to_string());
                    walk(x, path, out);
                    path.pop();
                }
            }
            Value::Object(o) => {
                for (k, x) in o {
                    path.push(k.clone());
                    walk(x, path, out);
                    path.pop();
                }
            }
            _ => {}
        }
    }
    walk(&doc, &mut vec![], &mut leaves);
    let hostile = [
        json!(0),
        json!(1),
        json!(-1),
        json!(u64::MAX),
        json!(4294967296u64),
        json!(9007199254740993u64),
        json!(1e300),
        json!("x"),
        Value::Null,
    ];
    let mut n_err = 0;
    for path in &leaves {
        for h in &hostile {
            let mut d = doc.clone();
            let mut cur = &mut d;
            for k in path {
                cur = match cur {
                    Value::Array(a) => &mut a[k.parse::<usize>().unwrap()],
                    Value::Object(o) => o.get_mut(k).unwrap(),
                    _ => unreachable!(),
                };
            }
            *cur = h.clone();
            if validate_glb(&join(&d, &bin)).is_err() {
                n_err += 1;
            }
        }
    }
    assert!(
        n_err > leaves.len(),
        "most hostile edits must be rejected ({n_err} of {})",
        leaves.len() * 9
    );
    // a count of u64::MAX in an accessor fails without allocating
    let (mut d, b) = split(&small);
    d["accessors"][0]["count"] = json!(u64::MAX);
    let t = std::time::Instant::now();
    assert!(validate_glb(&join(&d, &b)).is_err());
    assert!(t.elapsed().as_secs_f64() < 1.0);
}

/// Writes GLB files for the manual run of the Khronos validator (scripts/validate_glb.js) when RCFD_EXTERNAL_DIR is
/// set: a contour on one boundary patch, one on all patches of a 20^3 box, and an uncoloured one.
#[test]
fn write_glb_for_external_validator() {
    let Ok(dir) = std::env::var("RCFD_EXTERNAL_DIR") else {
        return;
    };
    std::fs::create_dir_all(&dir).unwrap();
    let m = box_mesh([20, 20, 20], [1.0; 3], [Grading::Uniform; 3]).unwrap();
    let x: Vec<f64> = m.cell_centres().iter().map(|c| c[0] + 0.3 * c[1]).collect();
    let opt = GltfOptions {
        range: None,
        scalar_name: "x".into(),
    };
    let all = ["xmin", "xmax", "ymin", "ymax", "zmin", "zmax"];
    let d = std::path::Path::new(&dir);
    std::fs::write(
        d.join("contour_zmax.glb"),
        glb_patches(&m, &["zmax"], Some(GltfScalar::Cell(&x)), &opt).unwrap().0,
    )
    .unwrap();
    std::fs::write(
        d.join("contour_all.glb"),
        glb_patches(&m, &all, Some(GltfScalar::Cell(&x)), &opt).unwrap().0,
    )
    .unwrap();
    std::fs::write(d.join("plain.glb"), glb_patches(&m, &all, None, &opt).unwrap().0).unwrap();
}
