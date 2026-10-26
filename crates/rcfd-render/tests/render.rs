//! Registered GPU checks of ADR 0125. Every check runs on every DX12 / Vulkan adapter of the machine (WARP
//! included); with no adapter the test prints a located SKIP line and passes.

mod cpu_ref;

use std::sync::OnceLock;
use std::time::Instant;

use rcfd_core::vec3::Vec3;
use rcfd_mesh::gen::{box_mesh, cavity_2d, Grading};
use rcfd_render::{
    adapters, Camera, ColorMap, Field, Image, PartStyle, Projection, Renderer, Shading, SurfaceMesh, View,
};

fn renderers() -> &'static [Renderer] {
    static R: OnceLock<Vec<Renderer>> = OnceLock::new();
    R.get_or_init(|| {
        adapters()
            .iter()
            .filter_map(|a| Renderer::from_adapter(a).ok())
            .collect()
    })
}

/// The adapters, or a located SKIP message (and `None`) when there is none.
macro_rules! gpus {
    () => {{
        let r = renderers();
        if r.is_empty() {
            eprintln!(
                "SKIP {}:{}: no wgpu adapter (DX12 or Vulkan) on this machine",
                file!(),
                line!()
            );
            return;
        }
        r
    }};
}

fn name(r: &Renderer) -> String {
    let i = r.info();
    format!("{} ({:?}, {:?})", i.name, i.backend, i.device_type)
}

fn lcg(s: &mut u64) -> f64 {
    *s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    (*s >> 11) as f64 / (1u64 << 53) as f64
}

fn ortho(target: Vec3, half_height: f64) -> Camera {
    Camera {
        target,
        distance: 2.0,
        yaw: 0.0,
        pitch: 0.0,
        projection: Projection::Orthographic { half_height },
        near: 0.5,
        far: 4.0,
    }
}

fn save(img: &Image, file: &str) {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("rcfd-render");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(file), img.to_png().unwrap()).unwrap();
}

fn count(img: &Image, rgba: [u8; 4]) -> usize {
    img.rgba.chunks_exact(4).filter(|p| *p == rgba).count()
}

/// Unit square [0,1]^2 at z = 0 as `n` vertical strips (one part), points shared between strips.
fn strips(n: usize) -> SurfaceMesh {
    let mut pts = Vec::new();
    for i in 0..=n {
        let x = i as f64 / n as f64;
        pts.push([x, 0.0, 0.0]);
        pts.push([x, 1.0, 0.0]);
    }
    let faces: Vec<Vec<usize>> = (0..n).map(|i| vec![2 * i, 2 * i + 2, 2 * i + 3, 2 * i + 1]).collect();
    SurfaceMesh::new(pts, &faces, vec![0; n], vec!["square".into()]).unwrap()
}

// R0: the step-0 triangle through the library: (-1,-1), (1,-1), (-1,1) in clip space on 64 x 64 pixels.
// Closed form with the top-left rule: (4096 - 64 diagonal centres) / 2 = 2016 covered, 2080 background.
#[test]
fn r0_triangle_pixel_count_closed_form() {
    for r in gpus!() {
        let s = SurfaceMesh::new(
            vec![[-1.0, -1.0, 0.0], [1.0, -1.0, 0.0], [-1.0, 1.0, 0.0]],
            &[vec![0, 1, 2]],
            vec![0],
            vec!["t".into()],
        )
        .unwrap();
        let m = r.upload(&s, Field::None).unwrap();
        let mut v = View::new(ortho([0.0; 3], 1.0), 64, 64);
        v.background = [0, 0, 0, 255];
        v.parts = vec![PartStyle::Solid([255, 0, 0])];
        let img = r.render(&m, &v).unwrap();
        let (red, black) = (count(&img, [255, 0, 0, 255]), count(&img, [0, 0, 0, 255]));
        eprintln!("R0 {}: red {red} black {black}", name(r));
        assert_eq!((red, black), (2016, 2080));
        assert_ne!(
            red,
            4096 / 2 + 64 / 2,
            "negative control: the inclusive-diagonal count 2080 is a different number"
        );
    }
}

// G1: the cavity front face against rcfd-post's independent 2-D renderer.
#[test]
fn g1_cavity_matches_rcfd_post_render_scalar() {
    let gpus = gpus!();
    let n = 16;
    let mesh = cavity_2d(n, 1.0).unwrap();
    let mut seed = 3u64;
    let cells: Vec<f64> = (0..mesh.n_cells()).map(|_| lcg(&mut seed)).collect();
    // Mirrored field (cell (i, j) takes the value of (n-1-i, j)) for the negative control; cells are x-fastest.
    let mirrored: Vec<f64> = (0..mesh.n_cells())
        .map(|c| cells[(c / n) * n + (n - 1 - c % n)])
        .collect();
    let opt = rcfd_post::render::RenderOptions {
        width: 272,
        colorbar: false,
        range: Some((0.0, 1.0)),
        ..Default::default()
    };
    let reference = rcfd_post::render::render_scalar(&mesh, &cells, &opt).unwrap().image;
    let wrong = rcfd_post::render::render_scalar(&mesh, &mirrored, &opt).unwrap().image;
    assert_eq!((reference.width, reference.height), (272, 272));
    let s = SurfaceMesh::from_mesh(&mesh, &[]).unwrap();
    let faces = s.face_values_from_cells(&cells).unwrap();
    // rcfd-post maps x to 8 + 256 x and y to 8 + 256 (1 - y): an orthographic view of half height 136/256.
    let mut v = View::new(ortho([0.5, 0.5, 1.0 / 32.0], 136.0 / 256.0), 272, 272);
    v.range = Some((0.0, 1.0));
    for r in gpus {
        let img = r.render(&r.upload(&s, Field::Face(&faces)).unwrap(), &v).unwrap();
        let px = img.rgba.len() / 4;
        let over = |a: &Image, lim: u8| {
            (0..px)
                .filter(|i| (0..3).any(|k| img.rgba[4 * i + k].abs_diff(a.rgba[4 * i + k]) > lim))
                .count()
        };
        let max = (0..img.rgba.len())
            .map(|i| img.rgba[i].abs_diff(reference.rgba[i]))
            .max()
            .unwrap();
        let nc = over(&wrong, 1) as f64 / (256.0 * 256.0);
        eprintln!(
            "G1 {}: max diff {max}, pixels > 0: {}, negative control {nc:.3}",
            name(r),
            over(&reference, 0)
        );
        save(
            &img,
            &format!("g1_cavity_{:?}_{}.png", r.info().backend, r.info().device),
        );
        assert!(max <= 1, "G1 max channel diff {max} > 1");
        assert!(nc >= 0.5, "G1 negative control does not discriminate: {nc}");
    }
}

fn cube() -> SurfaceMesh {
    let m = box_mesh([4, 4, 4], [1.0, 1.0, 1.0], [Grading::Uniform; 3]).unwrap();
    SurfaceMesh::from_mesh(&m, &[]).unwrap()
}

fn cube_view(s: &SurfaceMesh, yaw_deg: f64) -> View {
    let (lo, hi) = s.bounds();
    let cam = Camera::fit(
        lo,
        hi,
        yaw_deg.to_radians(),
        25f64.to_radians(),
        Projection::Perspective { fovy: 0.8 },
    );
    View::new(cam, 320, 240)
}

struct Scene {
    label: &'static str,
    shading: Shading,
    vertex: bool,
    parts: Vec<PartStyle>,
    limit: u8,
}

fn scenes() -> Vec<Scene> {
    vec![
        Scene {
            label: "a face unlit",
            shading: Shading::Unlit,
            vertex: false,
            parts: vec![],
            limit: 1,
        },
        Scene {
            label: "b face flat",
            shading: Shading::Flat,
            vertex: false,
            parts: vec![],
            limit: 2,
        },
        Scene {
            label: "c vertex smooth",
            shading: Shading::Smooth,
            vertex: true,
            parts: vec![],
            limit: 2,
        },
        Scene {
            label: "d vertex unlit, xmax solid, ymax hidden",
            shading: Shading::Unlit,
            vertex: true,
            // patch order of box_mesh: xmin, xmax, ymin, ymax, zmin, zmax
            parts: vec![
                PartStyle::Field,
                PartStyle::Solid([255, 0, 0]),
                PartStyle::Field,
                PartStyle::Hidden,
            ],
            limit: 1,
        },
    ]
}

fn cube_fields(s: &SurfaceMesh) -> (Vec<f64>, Vec<f64>) {
    let mut seed = 11u64;
    let faces = (0..s.n_faces()).map(|_| lcg(&mut seed)).collect();
    let verts = s.points.iter().map(|p| p[0] + 2.0 * p[1] * p[1] - p[2]).collect();
    (faces, verts)
}

// G2: the cube boundary in perspective against the CPU reference rasteriser, four shading/part scenes.
// D1: two renders on one adapter are byte-identical.
#[test]
fn g2_cube_matches_cpu_reference_and_d1_is_deterministic() {
    let gpus = gpus!();
    let s = cube();
    let (fv, vv) = cube_fields(&s);
    for sc in scenes() {
        let field = if sc.vertex {
            Field::Vertex(&vv)
        } else {
            Field::Face(&fv)
        };
        let mut v = cube_view(&s, 35.0);
        v.shading = sc.shading;
        v.parts = sc.parts.clone();
        let reference = cpu_ref::render(&s, field, &v);
        let mut nv = cube_view(&s, 37.0);
        nv.shading = sc.shading;
        nv.parts = sc.parts.clone();
        let wrong = cpu_ref::render(&s, field, &nv);
        for r in gpus {
            let m = r.upload(&s, field).unwrap();
            let img = r.render(&m, &v).unwrap();
            let again = r.render(&m, &v).unwrap();
            let d = cpu_ref::diff(&img, &reference, sc.limit);
            let n = cpu_ref::diff(&img, &wrong, sc.limit);
            eprintln!(
                "G2 {} [{}]: {d:?}; negative control interior max {}",
                sc.label,
                name(r),
                n.interior_max
            );
            save(
                &img,
                &format!("g2_{}_{:?}_{}.png", &sc.label[..1], r.info().backend, r.info().device),
            );
            assert!(d.interior_max <= sc.limit, "G2 interior");
            assert!(d.edge_over as f64 <= 0.01 * d.edge_pixels as f64, "G2 edge");
            assert!(d.mean <= 0.25, "G2 mean");
            assert!(n.interior_max > sc.limit, "G2 negative control does not discriminate");
            assert!(img.rgba == again.rgba, "D1: two renders differ");
        }
        save(&reference.image, &format!("g2_{}_cpu_reference.png", &sc.label[..1]));
    }
}

// G3: a known scalar ramp reads back the colour-map values at registered pixels (rows 64, 128, 192, every column);
// a strip of 8 faces with values k/7 reads back colormap(k/7) at the strip centres.
#[test]
fn g3_colour_ramp_reads_back_the_colour_map() {
    let gpus = gpus!();
    let s = strips(1);
    let ramp: Vec<f64> = s.points.iter().map(|p| p[0]).collect();
    let v = View::new(ortho([0.5, 0.5, 0.0], 0.5), 256, 256);
    let gui6 = ColorMap(vec![
        [68, 1, 84],
        [65, 68, 135],
        [42, 120, 142],
        [34, 168, 132],
        [122, 209, 81],
        [253, 231, 37],
    ]);
    let s8 = strips(8);
    let steps: Vec<f64> = (0..8).map(|k| k as f64 / 7.0).collect();
    for r in gpus {
        let img = r.render(&r.upload(&s, Field::Vertex(&ramp)).unwrap(), &v).unwrap();
        let (mut worst, mut nc) = (0u8, 0usize);
        for y in [64usize, 128, 192] {
            for x in 0..256usize {
                let t = (x as f64 + 0.5) / 256.0;
                let want = rcfd_post::render::colormap(t);
                let got = &img.rgba[4 * (y * 256 + x)..][..3];
                worst = worst.max((0..3).map(|k| got[k].abs_diff(want[k])).max().unwrap());
                let other = gui6.eval(t);
                nc += (0..3).any(|k| got[k].abs_diff(other[k]) > 1) as usize;
            }
        }
        let img8 = r.render(&r.upload(&s8, Field::Face(&steps)).unwrap(), &v).unwrap();
        let mut worst8 = 0u8;
        for k in 0..8usize {
            let want = rcfd_post::render::colormap(k as f64 / 7.0);
            let got = &img8.rgba[4 * (128 * 256 + 32 * k + 16)..][..3];
            worst8 = worst8.max((0..3).map(|c| got[c].abs_diff(want[c])).max().unwrap());
        }
        eprintln!(
            "G3 {}: ramp max diff {worst}, strips max diff {worst8}, negative control {nc}/768",
            name(r)
        );
        assert!(worst <= 1 && worst8 <= 1, "G3");
        assert!(nc >= 384, "G3 negative control does not discriminate");
    }
}

// P1: picking returns the right face, mesh face, owner cell and value on the 8x8 cavity; the wrong mapping
// (id read as a triangle index) must fail.
#[test]
fn p1_picking_returns_face_cell_and_value() {
    let gpus = gpus!();
    let n = 8;
    let mesh = cavity_2d(n, 1.0).unwrap();
    let s = SurfaceMesh::from_mesh(&mesh, &[]).unwrap();
    let cells: Vec<f64> = (0..mesh.n_cells()).map(|c| 1.0 + c as f64 * 0.125).collect();
    let faces = s.face_values_from_cells(&cells).unwrap();
    let tri_face: Vec<usize> = s.triangles().map(|(f, _)| f).collect();
    let v = View::new(ortho([0.5, 0.5, 1.0 / 16.0], 0.5), 256, 256);
    let zf = 1.0 / n as f64;
    for r in gpus {
        let p = r.pick(&r.upload(&s, Field::Face(&faces)).unwrap(), &v).unwrap();
        let (mut ok, mut wrong_ok) = (0, 0);
        for j in 0..n {
            for i in 0..n {
                let (px, py) = (32 * i as u32 + 16, 256 - 32 * j as u32 - 16);
                let (f, val) = p.at(px, py).expect("face under the probe");
                let c = s
                    .face(f)
                    .iter()
                    .fold([0.0; 3], |a, &q| rcfd_core::vec3::add(a, s.points[q]));
                let cell = j * n + i;
                let good = (c[0] / 4.0 - (i as f64 + 0.5) / n as f64).abs() < 1e-12
                    && (c[1] / 4.0 - (j as f64 + 0.5) / n as f64).abs() < 1e-12
                    && (c[2] / 4.0 - zf).abs() < 1e-12
                    && s.cell[f] == Some(cell)
                    && s.mesh_face[f].map(|mf| mesh.owner()[mf]) == Some(cell)
                    && val == cells[cell] as f32;
                ok += good as usize;
                // negative control: the id read as a triangle index
                let wf = tri_face.get(f).copied().unwrap_or(usize::MAX);
                wrong_ok += (wf < s.n_faces() && s.cell[wf] == Some(cell) && s.face(wf) == s.face(f)) as usize;
            }
        }
        let bg = p.at(0, 0).is_none() && p.at(300, 0).is_none();
        eprintln!(
            "P1 {}: {ok}/64 correct, negative control {wrong_ok}/64 correct, background {bg}",
            name(r)
        );
        assert_eq!(ok, 64, "P1");
        assert!(bg, "P1 background");
        assert!(wrong_ok <= 32, "P1 negative control does not discriminate");
    }
}

// P2: depth and visibility through picking: the cube seen along -z picks a zmax face at the centre; with zmax
// hidden it picks a zmin face; per-point values from the pick pass match the CPU reference on interior pixels.
#[test]
fn p2_pick_depth_visibility_and_interpolated_value() {
    let gpus = gpus!();
    let s = cube();
    let (_, vv) = cube_fields(&s);
    let (lo, hi) = s.bounds();
    let front = View::new(
        Camera::fit(lo, hi, 0.0, 0.0, Projection::Orthographic { half_height: 1.0 }),
        128,
        128,
    );
    let mut hidden = front.clone();
    hidden.parts = vec![PartStyle::Field; 5];
    hidden.parts.push(PartStyle::Hidden);
    let zpart = |f: usize| s.parts[s.face_part[f]].clone();
    let mut v = cube_view(&s, 35.0);
    v.shading = Shading::Smooth;
    let reference = cpu_ref::render(&s, Field::Vertex(&vv), &v);
    let edge = cpu_ref::edge_mask(&reference);
    let (vlo, vhi) = s.field_range(Field::Vertex(&vv)).unwrap().unwrap();
    for r in gpus {
        let m = r.upload(&s, Field::Vertex(&vv)).unwrap();
        let a = r.pick(&m, &front).unwrap().at(64, 64).map(|x| zpart(x.0));
        let b = r.pick(&m, &hidden).unwrap().at(64, 64).map(|x| zpart(x.0));
        let p = r.pick(&m, &v).unwrap();
        let (mut worst, mut id_bad) = (0f64, 0usize);
        for i in 0..p.ids.len() {
            if edge[i] {
                continue;
            }
            id_bad += (p.ids[i] != reference.ids[i]) as usize;
            if p.ids[i] > 0 {
                worst = worst.max((p.values[i] as f64 - reference.values[i]).abs() / (vhi - vlo));
            }
        }
        eprintln!(
            "P2 {}: centre {a:?}, zmax hidden {b:?}, interior id mismatches {id_bad}, value err {worst:.2e}",
            name(r)
        );
        assert_eq!(a.as_deref(), Some("zmax"));
        assert_eq!(b.as_deref(), Some("zmin"));
        assert_eq!(id_bad, 0, "P2 interior ids");
        assert!(worst <= 1e-5, "P2 value");
    }
}

// V1: part visibility and solid colour on the cavity seen along -z.
#[test]
fn v1_part_visibility_and_solid_colour() {
    let gpus = gpus!();
    let mesh = cavity_2d(8, 1.0).unwrap();
    let s = SurfaceMesh::from_mesh(&mesh, &[]).unwrap();
    let fb = s.parts.iter().position(|p| p == "frontAndBack").unwrap();
    let base = View::new(ortho([0.5, 0.5, 1.0 / 16.0], 0.5), 256, 256);
    for r in gpus {
        let m = r.upload(&s, Field::None).unwrap();
        let mut hidden = base.clone();
        hidden.parts = vec![PartStyle::Field; s.parts.len()];
        hidden.parts[fb] = PartStyle::Hidden;
        let mut solid = hidden.clone();
        solid.parts[fb] = PartStyle::Solid([255, 0, 0]);
        let drawn_hidden = 65536 - count(&r.render(&m, &hidden).unwrap(), [255; 4]);
        let drawn_visible = 65536 - count(&r.render(&m, &base).unwrap(), [255; 4]);
        let red = count(&r.render(&m, &solid).unwrap(), [255, 0, 0, 255]);
        eprintln!(
            "V1 {}: hidden {drawn_hidden}, visible (negative control) {drawn_visible}, solid red {red}",
            name(r)
        );
        assert_eq!(drawn_hidden, 0);
        assert_eq!(drawn_visible, 65536);
        assert_eq!(red, 65536);
    }
}

// K1: a cut plane x <= 100/256 on the unit square at 256 x 256: columns 0..=99 drawn, 100..=255 not, every row,
// in the colour and the pick pass. The negative control expects the boundary one column further.
#[test]
fn k1_cut_plane_position_is_exact() {
    let gpus = gpus!();
    let s = strips(1);
    let mut v = View::new(ortho([0.5, 0.5, 0.0], 0.5), 256, 256);
    v.parts = vec![PartStyle::Solid([0, 0, 255])];
    v.clip = Some(([1.0, 0.0, 0.0], 100.0 / 256.0));
    for r in gpus {
        let m = r.upload(&s, Field::None).unwrap();
        let img = r.render(&m, &v).unwrap();
        let p = r.pick(&m, &v).unwrap();
        let mismatches = |edge_col: u32| {
            let mut bad = 0;
            for y in 0..256u32 {
                for x in 0..256u32 {
                    let drawn = img.pixel(x as usize, y as usize) == [0, 0, 255, 255];
                    let picked = p.at(x, y).is_some();
                    bad += (drawn != (x < edge_col) || picked != (x < edge_col)) as usize;
                }
            }
            bad
        };
        let (good, nc) = (mismatches(100), mismatches(101));
        eprintln!("K1 {}: mismatches {good}, negative control {nc}", name(r));
        assert_eq!(good, 0);
        assert!(nc > 0);
    }
}

// W1: PNG output decodes to the same pixels; size validation.
#[test]
fn w1_png_round_trip_and_size_validation() {
    let gpus = gpus!();
    let s = strips(4);
    let vals = [0.0, 1.0, 2.0, 3.0];
    for r in gpus {
        let m = r.upload(&s, Field::Face(&vals)).unwrap();
        let img = r.render(&m, &View::new(ortho([0.5, 0.5, 0.0], 0.5), 100, 37)).unwrap();
        let png = img.to_png().unwrap();
        let mut dec = png::Decoder::new(std::io::Cursor::new(png)).read_info().unwrap();
        let mut buf = vec![0; dec.output_buffer_size().unwrap()];
        let info = dec.next_frame(&mut buf).unwrap();
        assert_eq!((info.width, info.height), (100, 37));
        assert_eq!(&buf[..info.buffer_size()], &img.rgba[..]);
        for (w, h) in [(0, 10), (10, 0), (100_000, 10)] {
            assert!(r.render(&m, &View::new(ortho([0.5; 3], 0.5), w, h)).is_err());
        }
        let mut bad = View::new(ortho([0.5; 3], 0.5), 8, 8);
        bad.range = Some((1.0, 1.0));
        assert!(r.render(&m, &bad).is_err());
        assert!(r.upload(&s, Field::Face(&[0.0, f64::NAN, 0.0, 0.0])).is_err());
        assert!(r.upload(&s, Field::Vertex(&vals)).is_err());
    }
}

// S1: no adapter gives a located error, which the test helper turns into a SKIP line.
#[test]
fn s1_no_adapter_is_a_located_error() {
    let e = Renderer::first_of(rcfd_render::wgpu::Backends::empty())
        .err()
        .expect("no back end, no adapter");
    let msg = e.to_string();
    eprintln!("S1: {msg}");
    assert!(msg.contains("no adapter"), "{msg}");
}

// T1: frame time for 1M triangles (500 x 500 x 1 box, zmin + zmax patches) at 1920 x 1080, flat shading, per-face
// field. Median of 5 renders after one warm-up, each render including uniforms, draw, and readback of the image.
// Asserted only on discrete GPUs (limit 250 ms); other adapters are reported.
#[test]
fn t1_frame_time_one_million_triangles() {
    let gpus = gpus!();
    let m = box_mesh([500, 500, 1], [1.0, 1.0, 0.01], [Grading::Uniform; 3]).unwrap();
    let s = SurfaceMesh::from_mesh(&m, &["zmin", "zmax"]).unwrap();
    let tris = s.triangles().count();
    assert_eq!(tris, 1_000_000);
    let vals: Vec<f64> = (0..s.n_faces()).map(|f| (f % 977) as f64).collect();
    let (lo, hi) = s.bounds();
    let mut v = View::new(
        Camera::fit(lo, hi, 0.4, 0.6, Projection::Perspective { fovy: 0.8 }),
        1920,
        1080,
    );
    v.shading = Shading::Flat;
    for r in gpus {
        let t0 = Instant::now();
        let gm = r.upload(&s, Field::Face(&vals)).unwrap();
        let upload = t0.elapsed().as_secs_f64();
        r.render(&gm, &v).unwrap();
        let mut times: Vec<f64> = (0..5)
            .map(|_| {
                let t = Instant::now();
                r.render(&gm, &v).unwrap();
                t.elapsed().as_secs_f64()
            })
            .collect();
        times.sort_by(f64::total_cmp);
        let med = times[2];
        eprintln!(
            "T1 {}: {tris} triangles, upload {:.1} ms, median frame {:.1} ms",
            name(r),
            upload * 1e3,
            med * 1e3
        );
        if r.info().device_type == rcfd_render::wgpu::DeviceType::DiscreteGpu {
            assert!(med <= 0.25, "T1 frame time {med} s > 0.25 s");
        }
    }
}
