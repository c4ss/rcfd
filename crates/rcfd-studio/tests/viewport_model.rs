//! ADR 0132 integration checks without a GPU: V3 (mouse mapping against closed forms) and V9 (no adapter), plus the
//! steady-run slider axis (unregistered).

#[macro_use]
mod vp;

use rcfd_render::{wgpu, Camera, Projection};
use rcfd_studio::viewport::{axis_of, Axis, CameraState, Input, Snap, StoreView, ViewGpu};
use rcfd_studio::{Studio, StudioApp};

const W: f64 = 800.0;
const H: f64 = 600.0;
const LO: [f64; 3] = [-1.0, 0.0, 2.0];
const HI: [f64; 3] = [3.0, 1.0, 2.5];

// ---------------------------------------------------------------- closed forms, written out here

fn back(y: f64, p: f64) -> [f64; 3] {
    [p.cos() * y.sin(), p.sin(), p.cos() * y.cos()]
}
fn right(y: f64) -> [f64; 3] {
    [y.cos(), 0.0, -y.sin()]
}
fn up(y: f64, p: f64) -> [f64; 3] {
    [-p.sin() * y.sin(), p.cos(), -p.sin() * y.cos()]
}
fn axpy(a: f64, x: [f64; 3], y: [f64; 3]) -> [f64; 3] {
    [a * x[0] + y[0], a * x[1] + y[1], a * x[2] + y[2]]
}

/// Near and far of the registered rule around the bounding sphere of [LO, HI].
fn cf_planes(c: Camera) -> Camera {
    let centre = [0.5 * (LO[0] + HI[0]), 0.5 * (LO[1] + HI[1]), 0.5 * (LO[2] + HI[2])];
    let d = [HI[0] - LO[0], HI[1] - LO[1], HI[2] - LO[2]];
    let r = (0.5 * (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()).max(1e-30);
    let b = back(c.yaw, c.pitch);
    let eye = axpy(c.distance, b, c.target);
    let zc = (eye[0] - centre[0]) * b[0] + (eye[1] - centre[1]) * b[1] + (eye[2] - centre[2]) * b[2];
    let near = (zc - 1.01 * r).max(1e-3 * r);
    let far = (zc + 1.01 * r).max(2.0 * near);
    Camera { near, far, ..c }
}

fn cf_orbit(c: Camera, dx: f64, dy: f64) -> Camera {
    let h = std::f64::consts::FRAC_PI_2;
    cf_planes(Camera {
        yaw: c.yaw - 0.01 * dx,
        pitch: (c.pitch + 0.01 * dy).clamp(-h, h),
        ..c
    })
}

fn scale_px(c: &Camera) -> f64 {
    match c.projection {
        Projection::Perspective { fovy } => 2.0 * c.distance * (fovy / 2.0).tan() / H,
        Projection::Orthographic { half_height } => 2.0 * half_height / H,
    }
}

fn cf_pan(c: Camera, dx: f64, dy: f64, flip_y: bool) -> Camera {
    let s = scale_px(&c);
    let sy = if flip_y { -dy } else { dy };
    let t = axpy(s * sy, up(c.yaw, c.pitch), axpy(-s * dx, right(c.yaw), c.target));
    cf_planes(Camera { target: t, ..c })
}

fn cf_dolly(c: Camera, n: f64) -> Camera {
    let f = 0.9f64.powf(n);
    cf_planes(match c.projection {
        Projection::Perspective { .. } => Camera {
            distance: c.distance * f,
            ..c
        },
        Projection::Orthographic { half_height } => Camera {
            projection: Projection::Orthographic {
                half_height: half_height * f,
            },
            ..c
        },
    })
}

fn cf_fit(c: Camera) -> Camera {
    let p = match c.projection {
        Projection::Perspective { .. } => Projection::Perspective { fovy: 0.8 },
        o => o,
    };
    cf_planes(Camera::fit(LO, HI, c.yaw, c.pitch, p))
}

fn half(c: &Camera) -> f64 {
    match c.projection {
        Projection::Orthographic { half_height } => half_height,
        Projection::Perspective { fovy } => fovy,
    }
}

/// Largest relative difference `|a - b| / max(1, |b|)` over every camera component.
fn diff(a: &Camera, b: &Camera) -> f64 {
    let pairs = [
        (a.target[0], b.target[0]),
        (a.target[1], b.target[1]),
        (a.target[2], b.target[2]),
        (a.distance, b.distance),
        (a.yaw, b.yaw),
        (a.pitch, b.pitch),
        (half(a), half(b)),
        (a.near, b.near),
        (a.far, b.far),
    ];
    pairs
        .iter()
        .map(|(x, y)| (x - y).abs() / y.abs().max(1.0))
        .fold(0.0, f64::max)
}

/// V3: orbit, pan, dolly and fit of the model equal the closed forms within rel. 1e-12; after a pan the old target
/// projects to the image centre plus the drag within 1e-9 px. Negative control: the pan with the y sign flipped.
#[test]
fn v3_mouse_mapping_equals_closed_forms() {
    let script = [
        Input::Orbit(37.0, -12.0),
        Input::Pan(37.0, -12.0),
        Input::Dolly(1.0),
        Input::Pan(-5.0, 80.0),
        Input::Orbit(-5.0, 300.0),
        Input::Dolly(-2.5),
        Input::Dolly(0.25),
        Input::Orbit(10.0, 4.0),
        Input::Pan(3.0, 3.0),
        Input::Fit,
    ];
    let mut worst: f64 = 0.0;
    let mut worst_px: f64 = 0.0;
    let mut control_min: f64 = f64::INFINITY;
    let mut control_px_min: f64 = f64::INFINITY;
    let mut n = 0;
    for proj in [
        Projection::Perspective { fovy: 0.8 },
        Projection::Orthographic { half_height: 1.0 },
    ] {
        for (yaw, pitch) in [(0.6, 0.3), (-2.0, 1.2)] {
            let start = Camera::fit(LO, HI, yaw, pitch, proj);
            // the registered rule reproduces fit's own planes for a fitted camera
            let fp = cf_planes(start);
            assert!(diff(&fp, &start) <= 1e-12, "planes of a fitted camera: {}", diff(&fp, &start));
            let mut st = CameraState {
                cam: start,
                lo: LO,
                hi: HI,
            };
            for i in script {
                let before = st.cam;
                st.apply(i, H);
                let want = match i {
                    Input::Orbit(dx, dy) => cf_orbit(before, dx, dy),
                    Input::Pan(dx, dy) => cf_pan(before, dx, dy, false),
                    Input::Dolly(k) => cf_dolly(before, k),
                    Input::Fit => cf_fit(before),
                };
                let d = diff(&st.cam, &want);
                assert!(d <= 1e-12, "{proj:?} ({yaw}, {pitch}) {i:?}: rel. difference {d:e}");
                worst = worst.max(d);
                n += 1;
                if let Input::Pan(dx, dy) = i {
                    let q = st.cam.project(before.target, W, H).unwrap();
                    let e = (q[0] - (W / 2.0 + dx)).abs().max((q[1] - (H / 2.0 + dy)).abs());
                    assert!(e <= 1e-9, "{proj:?} {i:?}: the old target is {e} px off");
                    worst_px = worst_px.max(e);
                    // negative control: the y sign flipped
                    let flipped = cf_pan(before, dx, dy, true);
                    let dc = diff(&st.cam, &flipped);
                    assert!(dc > 1e-12, "control: the flipped pan matches ({dc:e})");
                    control_min = control_min.min(dc);
                    let qf = flipped.project(before.target, W, H).unwrap();
                    let ef = (qf[0] - (W / 2.0 + dx)).abs().max((qf[1] - (H / 2.0 + dy)).abs());
                    assert!(ef > 1.0, "control: the flipped pan's target is only {ef} px off");
                    control_px_min = control_px_min.min(ef);
                }
                if matches!(i, Input::Orbit(_, dy) if dy == 300.0) {
                    assert_eq!(st.cam.pitch, std::f64::consts::FRAC_PI_2, "the pitch clamps at +pi/2");
                }
            }
        }
    }
    eprintln!(
        "V3: {n} operations, worst rel. difference {worst:e}, worst pan pixel error {worst_px:e}; control: smallest \
         rel. difference of the flipped pan {control_min:e}, smallest pixel miss {control_px_min:.3}"
    );
}

/// V9: with no adapter the viewport reports a located error and headless frames still draw it.
#[test]
fn v9_no_adapter_is_a_located_error_and_frames_draw() {
    let e = ViewGpu::headless(wgpu::Backends::empty()).err().expect("no adapter for no back ends");
    assert!(e.starts_with("viewport: ") && e.contains("no adapter"), "{e}");
    let mut st = Studio::in_memory();
    st.poll = None;
    st.run_poll = None;
    st.viewport.set_gpu(Err(e.clone()));
    let mut app = StudioApp::new(st, None);
    let ctx = egui::Context::default();
    for i in 0..20 {
        let out = vp::frame(&mut app, &ctx);
        let t = vp::texts(&out.shapes);
        assert!(t.iter().any(|s| s == &e), "frame {i}: the error is not drawn");
    }
    eprintln!("V9: \"{e}\" drawn in 20 of 20 frames");
}

/// Unregistered: a steady run stores time 0 at every snapshot; the slider runs over steps and linear is off.
#[test]
fn steady_runs_slide_over_steps() {
    let same = [
        Snap { step: 5, time: 0.0 },
        Snap { step: 10, time: 0.0 },
    ];
    assert_eq!(axis_of(&same), Axis::Step);
    let inc = [
        Snap { step: 5, time: 0.1 },
        Snap { step: 10, time: 0.2 },
    ];
    assert_eq!(axis_of(&inc), Axis::Time);
    let dir = vp::temp("steady").join("fields");
    vp::write_store(&dir, &vp::cavity8(), &[(5, 0.0), (10, 0.0), (15, 0.0)], vp::p_v4);
    let mut sv = StoreView::open(&dir).unwrap();
    let r = rcfd_tsstore::Reader::open(&dir).unwrap();
    for s in [0.0, 5.0, 7.0, 7.5, 8.0, 12.5, 15.0, 99.0] {
        let want = r.by_step(f64::round(s) as u64, rcfd_tsstore::Pick::Nearest);
        assert_eq!(sv.index_at(s), want, "step {s}");
        let f = sv.sample("p", s, true).unwrap();
        assert!(!f.interpolated, "linear is off on the step axis");
    }
}
