//! The viewport (ADR 0132, wave 48C): a run's stored fields on its boundary surface, drawn by the render core into a texture
//! of eframe's own wgpu device (W48-7 (a)) and shown by egui; time scrubbing through the scalar-only update (W48-8 (a)).
//!
//! State and decisions live here; [`crate::viewport_ui`] only draws. The store is read on a worker thread per open project
//! ([`StoreView`] is the code it runs, callable from tests); the UI thread only does GPU work: one [`Renderer::upload`] per
//! run, then [`Renderer::update_scalar`] for every change of time or field, and [`Renderer::render_into`] when something
//! shown changed. Mouse mapping is pure functions of the camera ([`orbit`], [`pan`], [`dolly`], [`fit`], [`clip_planes`]).

use crate::watch::Wake;
use rcfd_core::vec3::{add, dot, norm, scale, sub, Vec3};
use rcfd_mesh::{Mesh, PatchKind};
use rcfd_render::{
    wgpu, Camera, ColorMap, Field, GpuMesh, PickImage, Projection, Renderer, Shading, SurfaceMesh, View,
};
use rcfd_tsstore::{IndexSource, Location, Mode, Pick, Reader};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Orbit sensitivity, radians per pixel of drag.
pub const ORBIT_RAD_PER_PX: f64 = 0.01;
/// Dolly factor per wheel notch away from the user (`< 1`: closer).
pub const DOLLY_PER_NOTCH: f64 = 0.9;
/// Vertical field of view of the 3D view (radians).
pub const FOVY: f64 = 0.8;
/// Start yaw and pitch of the 3D view (radians).
pub const START_3D: (f64, f64) = (0.6, 0.4);
/// Snapshots per second while playing (ADR 0132 V7's scrubbing rate).
pub const PLAY_PERIOD: Duration = Duration::from_millis(100);
/// Largest texture side (eframe requests `max_texture_dimension_2d` 8192).
pub const MAX_SIDE: u32 = 8192;
/// Ticks of the legend.
pub const LEGEND_TICKS: usize = 5;

// ---------------------------------------------------------------- camera mapping (V3)

/// Turntable orbit by a drag of `dx`, `dy` pixels: `yaw - k dx`, `pitch + k dy` clamped to +-pi/2.
pub fn orbit(c: &Camera, dx: f64, dy: f64) -> Camera {
    let h = std::f64::consts::FRAC_PI_2;
    Camera {
        yaw: c.yaw - ORBIT_RAD_PER_PX * dx,
        pitch: (c.pitch + ORBIT_RAD_PER_PX * dy).clamp(-h, h),
        ..*c
    }
}

/// World units per pixel at the target, for a texture `height` pixels high.
pub fn world_per_pixel(c: &Camera, height: f64) -> f64 {
    match c.projection {
        Projection::Perspective { fovy } => 2.0 * c.distance * (0.5 * fovy).tan() / height,
        Projection::Orthographic { half_height } => 2.0 * half_height / height,
    }
}

/// Pan by a drag of `dx`, `dy` pixels (y down) in a texture `height` pixels high: the scene follows the pointer.
pub fn pan(c: &Camera, dx: f64, dy: f64, height: f64) -> Camera {
    let s = world_per_pixel(c, height);
    let [r, u, _] = c.basis();
    Camera {
        target: add(sub(c.target, scale(r, s * dx)), scale(u, s * dy)),
        ..*c
    }
}

/// Dolly by `notches` wheel notches (positive: away from the user, closer): perspective scales the distance,
/// orthographic the half height, by `DOLLY_PER_NOTCH^notches`.
pub fn dolly(c: &Camera, notches: f64) -> Camera {
    let f = DOLLY_PER_NOTCH.powf(notches);
    match c.projection {
        Projection::Perspective { .. } => Camera {
            distance: c.distance * f,
            ..*c
        },
        Projection::Orthographic { half_height } => Camera {
            projection: Projection::Orthographic {
                half_height: half_height * f,
            },
            ..*c
        },
    }
}

/// Bounding-sphere centre and radius of a box, as [`Camera::fit`] takes them.
pub fn sphere(lo: Vec3, hi: Vec3) -> (Vec3, f64) {
    (scale(add(lo, hi), 0.5), (0.5 * norm(sub(hi, lo))).max(1e-30))
}

/// Near and far planes around the bounding sphere of `[lo, hi]`: `zc` the view depth of its centre,
/// `near = max(zc - 1.01 r, 1e-3 r)`, `far = max(zc + 1.01 r, 2 near)`.
pub fn clip_planes(c: &Camera, lo: Vec3, hi: Vec3) -> Camera {
    let (centre, r) = sphere(lo, hi);
    let back = c.basis()[2];
    let zc = -dot(sub(centre, c.eye()), back);
    let near = (zc - 1.01 * r).max(1e-3 * r);
    let far = (zc + 1.01 * r).max(2.0 * near);
    Camera { near, far, ..*c }
}

/// Fit the box with the camera's current yaw, pitch and projection kind (`fovy` [`FOVY`]).
pub fn fit(c: &Camera, lo: Vec3, hi: Vec3) -> Camera {
    let p = match c.projection {
        Projection::Perspective { .. } => Projection::Perspective { fovy: FOVY },
        Projection::Orthographic { half_height } => Projection::Orthographic { half_height },
    };
    Camera::fit(lo, hi, c.yaw, c.pitch, p)
}

/// One mouse action on the view.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Input {
    /// Primary drag, pixels.
    Orbit(f64, f64),
    /// Secondary or middle drag, pixels.
    Pan(f64, f64),
    /// Wheel notches (positive away from the user).
    Dolly(f64),
    /// Double click.
    Fit,
}

/// The camera and the box it frames (the surface bounds).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraState {
    /// Camera.
    pub cam: Camera,
    /// Surface bounds, low corner.
    pub lo: Vec3,
    /// Surface bounds, high corner.
    pub hi: Vec3,
}

impl CameraState {
    /// The start camera of a surface: 2D orthographic at yaw 0, pitch 0; 3D perspective at [`START_3D`].
    pub fn start(lo: Vec3, hi: Vec3, two_d: bool) -> CameraState {
        let cam = if two_d {
            Camera::fit(lo, hi, 0.0, 0.0, Projection::Orthographic { half_height: 1.0 })
        } else {
            Camera::fit(lo, hi, START_3D.0, START_3D.1, Projection::Perspective { fovy: FOVY })
        };
        CameraState { cam, lo, hi }
    }

    /// Apply one input for a texture `height` pixels high; near and far follow ([`clip_planes`]).
    pub fn apply(&mut self, input: Input, height: f64) {
        let c = match input {
            Input::Orbit(dx, dy) => orbit(&self.cam, dx, dy),
            Input::Pan(dx, dy) => pan(&self.cam, dx, dy, height),
            Input::Dolly(n) => dolly(&self.cam, n),
            Input::Fit => fit(&self.cam, self.lo, self.hi),
        };
        self.cam = clip_planes(&c, self.lo, self.hi);
    }
}

// ---------------------------------------------------------------- surface and store

/// The drawn surface of a mesh, and whether it is a 2D case: a mesh with an `Empty` patch is drawn as the faces of its
/// `Empty` patches whose area vector points to +z (the front of a one-cell-thick mesh); any other mesh as all its patches.
pub fn surface_of(mesh: &Mesh) -> rcfd_core::Result<(SurfaceMesh, bool)> {
    let empty: Vec<&str> = mesh
        .patches()
        .iter()
        .filter(|p| p.kind == PatchKind::Empty)
        .map(|p| p.name.as_str())
        .collect();
    if empty.is_empty() {
        return Ok((SurfaceMesh::from_mesh(mesh, &[])?, false));
    }
    let all = SurfaceMesh::from_mesh(mesh, &empty)?;
    let keep: Vec<usize> = (0..all.n_faces())
        .filter(|&f| all.face_area_vector(f)[2] > 0.0)
        .collect();
    Ok((retain_faces(&all, &keep), true))
}

/// The faces `keep` of a surface (in that order), with its points renumbered and parts kept.
fn retain_faces(s: &SurfaceMesh, keep: &[usize]) -> SurfaceMesh {
    let mut out = SurfaceMesh {
        face_offsets: vec![0],
        parts: s.parts.clone(),
        ..Default::default()
    };
    let mut map = vec![usize::MAX; s.points.len()];
    for &f in keep {
        for &p in s.face(f) {
            if map[p] == usize::MAX {
                map[p] = out.points.len();
                out.points.push(s.points[p]);
                out.point_id.push(s.point_id[p]);
            }
            out.face_points.push(map[p]);
        }
        out.face_offsets.push(out.face_points.len());
        out.face_part.push(s.face_part[f]);
        out.mesh_face.push(s.mesh_face[f]);
        out.cell.push(s.cell[f]);
    }
    out
}

/// Step and time of one stored snapshot.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Snap {
    /// Solver step.
    pub step: u64,
    /// Simulated time.
    pub time: f64,
}

/// What the slider runs over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    /// Simulated time (the stored times are strictly increasing).
    Time,
    /// Solver step (steady runs store time 0 at every snapshot).
    Step,
}

/// The slider axis of a snapshot list.
pub fn axis_of(snaps: &[Snap]) -> Axis {
    if snaps.len() >= 2 && snaps.windows(2).all(|w| w[1].time > w[0].time) {
        Axis::Time
    } else {
        Axis::Step
    }
}

/// The shown field at one slider position.
#[derive(Debug, Clone)]
pub struct Frame {
    /// Field name.
    pub field: String,
    /// Snapshot shown (the earlier one when interpolated).
    pub index: usize,
    /// Later snapshot (equal to `index` when not interpolated).
    pub upper: usize,
    /// Weight of `upper`.
    pub weight: f64,
    /// True when the values are interpolated in time (linear mode between two snapshots).
    pub interpolated: bool,
    /// Step marked in the plots: the snapshot's step, or `s0 + w (s1 - s0)` when interpolated.
    pub step: f64,
    /// Time shown: the snapshot's time, or the requested time when interpolated.
    pub time: f64,
    /// Values per surface face (each face's owner cell value).
    pub faces: Arc<Vec<f64>>,
    /// Min and max of `faces` (`None` when there are none).
    pub range: Option<(f64, f64)>,
}

/// Min and max of values (`None` for none).
pub fn min_max(v: &[f64]) -> Option<(f64, f64)> {
    (!v.is_empty()).then(|| {
        v.iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |a, &x| (a.0.min(x), a.1.max(x)))
    })
}

/// A run's store, read synchronously: the code the viewport worker runs.
pub struct StoreView {
    reader: Reader,
    /// The drawn surface.
    pub surface: Arc<SurfaceMesh>,
    /// True for a 2D case ([`surface_of`]).
    pub two_d: bool,
}

impl StoreView {
    /// Open `dir/store.rts`, read the mesh and build its surface.
    pub fn open(dir: &Path) -> Result<StoreView, String> {
        let at = |e: rcfd_core::Error| format!("{}: {e}", dir.display());
        let mut reader = Reader::open(dir).map_err(at)?;
        let mesh = reader.mesh().map_err(at)?;
        let (surface, two_d) = surface_of(&mesh).map_err(at)?;
        if surface.n_faces() == 0 {
            return Err(format!("{}: the mesh has no surface to draw", dir.display()));
        }
        Ok(StoreView {
            reader,
            surface: Arc::new(surface),
            two_d,
        })
    }

    /// Stored snapshots.
    pub fn snaps(&self) -> Vec<Snap> {
        self.reader
            .snapshots()
            .iter()
            .map(|s| Snap {
                step: s.step,
                time: s.time,
            })
            .collect()
    }

    /// True while the index was scanned (a live or interrupted writer): [`StoreView::refresh`] may add snapshots.
    pub fn live(&self) -> bool {
        !matches!(self.reader.index_source(), IndexSource::Footer)
    }

    /// Pick up committed snapshots of a live writer; how many were added.
    pub fn refresh(&mut self) -> Result<usize, String> {
        self.reader.refresh().map_err(|e| e.to_string())
    }

    /// Cell fields of the last snapshot.
    pub fn fields(&mut self) -> Result<Vec<String>, String> {
        let n = self.reader.snapshots().len();
        if n == 0 {
            return Ok(Vec::new());
        }
        Ok(self
            .reader
            .fields(n - 1)
            .map_err(|e| e.to_string())?
            .into_iter()
            .filter(|f| f.location == Location::Cell)
            .map(|f| f.name)
            .collect())
    }

    fn faces(&self, cells: &[f64]) -> Result<Vec<f64>, String> {
        self.surface.face_values_from_cells(cells).map_err(|e| e.to_string())
    }

    /// The snapshot at slider position `pos` (time or step, by [`axis_of`]), `Pick::Nearest`.
    pub fn index_at(&self, pos: f64) -> Option<usize> {
        match axis_of(&self.snaps()) {
            Axis::Time => self.reader.by_time(pos, Pick::Nearest),
            Axis::Step => self.reader.by_step(pos.max(0.0).round() as u64, Pick::Nearest),
        }
    }

    /// The field `name` at slider position `pos`: the nearest snapshot, or with `linear` (time axis only) the linear
    /// interpolation of `Reader::field_at_time`.
    pub fn sample(&mut self, name: &str, pos: f64, linear: bool) -> Result<Frame, String> {
        let snaps = self.snaps();
        let err = |e: rcfd_core::Error| format!("field '{name}': {e}");
        let (cells, index, upper, weight, interpolated) = if linear && axis_of(&snaps) == Axis::Time {
            let s = self.reader.field_at_time(name, pos, Mode::Linear).map_err(err)?;
            (s.values, s.lower, s.upper, s.weight, s.interpolated)
        } else {
            let i = self.index_at(pos).ok_or_else(|| "the store has no snapshots".to_string())?;
            (self.reader.read_field(i, name).map_err(err)?, i, i, 0.0, false)
        };
        let faces = self.faces(&cells)?;
        let (s0, s1) = (snaps[index].step as f64, snaps[upper].step as f64);
        Ok(Frame {
            field: name.to_string(),
            index,
            upper,
            weight,
            interpolated,
            step: if interpolated { s0 + weight * (s1 - s0) } else { s0 },
            time: if interpolated { pos } else { snaps[index].time },
            range: min_max(&faces),
            faces: Arc::new(faces),
        })
    }

    /// Min and max of the face values of field `name` over every stored snapshot.
    pub fn run_range(&mut self, name: &str) -> Result<Option<(f64, f64)>, String> {
        let mut r: Option<(f64, f64)> = None;
        for i in 0..self.reader.snapshots().len() {
            let cells = self.reader.read_field(i, name).map_err(|e| format!("field '{name}': {e}"))?;
            if let Some((a, b)) = min_max(&self.faces(&cells)?) {
                r = Some(r.map_or((a, b), |(x, y)| (x.min(a), y.max(b))));
            }
        }
        Ok(r)
    }
}

// ---------------------------------------------------------------- worker

/// Requests to the viewport worker.
#[derive(Debug, Clone)]
enum Command {
    Open { run: String, dir: PathBuf },
    Sample { gen: u64, field: String, pos: f64, linear: bool },
    RunRange { field: String },
}

/// Results of the viewport worker.
#[derive(Debug, Clone)]
enum Event {
    Opened {
        run: String,
        surface: Arc<SurfaceMesh>,
        two_d: bool,
        snaps: Vec<Snap>,
        fields: Vec<String>,
    },
    NoStore {
        run: String,
        message: String,
    },
    Snaps {
        run: String,
        snaps: Vec<Snap>,
        fields: Vec<String>,
    },
    Frame {
        run: String,
        gen: u64,
        frame: Frame,
    },
    RunRange {
        run: String,
        field: String,
        range: Option<(f64, f64)>,
    },
    Failed {
        run: String,
        message: String,
    },
}

struct ViewWorker {
    tx: Option<Sender<Command>>,
    rx: Receiver<Event>,
    handle: Option<JoinHandle<()>>,
}

impl ViewWorker {
    fn start(period: Duration, wake: Wake) -> ViewWorker {
        let (ctx, crx) = mpsc::channel();
        let (etx, erx) = mpsc::channel();
        let handle = std::thread::Builder::new()
            .name("rcfd-studio-viewport".into())
            .spawn(move || worker(crx, etx, period, wake))
            .expect("spawn the viewport worker");
        ViewWorker {
            tx: Some(ctx),
            rx: erx,
            handle: Some(handle),
        }
    }

    fn send(&self, c: Command) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(c);
        }
    }
}

impl Drop for ViewWorker {
    fn drop(&mut self) {
        self.tx = None;
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

fn worker(rx: Receiver<Command>, tx: Sender<Event>, period: Duration, wake: Wake) {
    let mut run: Option<(String, PathBuf)> = None;
    let mut view: Option<StoreView> = None;
    let mut missing: Option<String> = None;
    let send = |e: Event| {
        let _ = tx.send(e);
        wake();
    };
    loop {
        let first = match rx.recv_timeout(period) {
            Ok(c) => Some(c),
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => return,
        };
        // latest wins: of the pending sample requests only the newest is served
        let mut cmds: Vec<Command> = first.into_iter().chain(rx.try_iter()).collect();
        if let Some(last) = cmds.iter().rposition(|c| matches!(c, Command::Sample { .. })) {
            let mut k = 0;
            cmds.retain(|c| {
                let keep = !matches!(c, Command::Sample { .. }) || k == last;
                k += 1;
                keep
            });
        }
        for c in cmds {
            match c {
                Command::Open { run: id, dir } => {
                    view = None;
                    missing = None;
                    run = Some((id.clone(), dir.clone()));
                    try_open(&id, &dir, &mut view, &mut missing, &send);
                }
                Command::Sample {
                    gen,
                    field,
                    pos,
                    linear,
                } => {
                    let (Some((id, _)), Some(v)) = (&run, view.as_mut()) else { continue };
                    match v.sample(&field, pos, linear) {
                        Ok(frame) => send(Event::Frame {
                            run: id.clone(),
                            gen,
                            frame,
                        }),
                        Err(message) => send(Event::Failed {
                            run: id.clone(),
                            message,
                        }),
                    }
                }
                Command::RunRange { field } => {
                    let (Some((id, _)), Some(v)) = (&run, view.as_mut()) else { continue };
                    match v.run_range(&field) {
                        Ok(range) => send(Event::RunRange {
                            run: id.clone(),
                            field,
                            range,
                        }),
                        Err(message) => send(Event::Failed {
                            run: id.clone(),
                            message,
                        }),
                    }
                }
            }
        }
        // live runs: a store that appears, or new snapshots of a scanned index
        let Some((id, dir)) = &run else { continue };
        match view.as_mut() {
            None => try_open(id, dir, &mut view, &mut missing, &send),
            Some(v) if v.live() => match v.refresh() {
                Ok(0) => {}
                Ok(_) => {
                    let fields = v.fields().unwrap_or_default();
                    send(Event::Snaps {
                        run: id.clone(),
                        snaps: v.snaps(),
                        fields,
                    });
                }
                Err(message) => send(Event::Failed {
                    run: id.clone(),
                    message,
                }),
            },
            Some(_) => {}
        }
    }
}

fn try_open(
    id: &str,
    dir: &Path,
    view: &mut Option<StoreView>,
    missing: &mut Option<String>,
    send: &impl Fn(Event),
) {
    if !dir.join(rcfd_tsstore::STORE_FILE).is_file() {
        let message = format!(
            "no stored fields: {} not found (start the run with \"Store fields\" on)",
            dir.join(rcfd_tsstore::STORE_FILE).display()
        );
        if missing.as_deref() != Some(message.as_str()) {
            *missing = Some(message.clone());
            send(Event::NoStore {
                run: id.to_string(),
                message,
            });
        }
        return;
    }
    match StoreView::open(dir) {
        Ok(mut v) => {
            let fields = v.fields().unwrap_or_default();
            send(Event::Opened {
                run: id.to_string(),
                surface: v.surface.clone(),
                two_d: v.two_d,
                snaps: v.snaps(),
                fields,
            });
            *view = Some(v);
        }
        Err(message) => {
            if missing.as_deref() != Some(message.as_str()) {
                *missing = Some(message.clone());
                send(Event::Failed {
                    run: id.to_string(),
                    message,
                });
            }
        }
    }
}

// ---------------------------------------------------------------- GPU

/// The viewport texture and its egui id.
struct Target {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
}

/// The egui renderer of eframe's window (the texture is registered there).
pub type EguiRenderer = Arc<egui::mutex::RwLock<eframe::egui_wgpu::Renderer>>;

/// The render core on a device shared with the window, the viewport texture and the uploaded mesh.
pub struct ViewGpu {
    renderer: Renderer,
    egui: Option<EguiRenderer>,
    target: Option<Target>,
    /// The texture's egui id (`None` without an egui renderer, as in headless tests).
    pub tex_id: Option<egui::TextureId>,
    mesh: Option<(Arc<SurfaceMesh>, GpuMesh)>,
    pick: Option<PickImage>,
    /// The window's swap-chain format (`None` headless).
    pub window_format: Option<wgpu::TextureFormat>,
    /// Surfaces uploaded ([`Renderer::upload`]).
    pub uploads: u64,
    /// Fields written with [`Renderer::update_scalar`].
    pub scalar_updates: u64,
    /// Images drawn with [`Renderer::render_into`].
    pub renders: u64,
}

impl ViewGpu {
    fn with(renderer: Renderer, egui: Option<EguiRenderer>, window_format: Option<wgpu::TextureFormat>) -> ViewGpu {
        ViewGpu {
            renderer,
            egui,
            target: None,
            tex_id: None,
            mesh: None,
            pick: None,
            window_format,
            uploads: 0,
            scalar_updates: 0,
            renders: 0,
        }
    }

    /// The app's path: eframe's device and queue (W48-7 (a)); the texture is shown through eframe's egui renderer.
    pub fn from_render_state(rs: &eframe::egui_wgpu::RenderState) -> Result<ViewGpu, String> {
        let r = Renderer::from_device(rs.device.clone(), rs.queue.clone(), rs.adapter.get_info())
            .map_err(|e| format!("viewport: {e}"))?;
        Ok(Self::with(r, Some(rs.renderer.clone()), Some(rs.target_format)))
    }

    /// A device the caller made (tests: the app's code path on a test device); no egui renderer.
    pub fn from_device(device: wgpu::Device, queue: wgpu::Queue, info: wgpu::AdapterInfo) -> Result<ViewGpu, String> {
        let r = Renderer::from_device(device, queue, info).map_err(|e| format!("viewport: {e}"))?;
        Ok(Self::with(r, None, None))
    }

    /// Headless: the best adapter of `backends` (tests; `Backends::empty()` gives the "no adapter" error).
    pub fn headless(backends: wgpu::Backends) -> Result<ViewGpu, String> {
        let r = Renderer::first_of(backends).map_err(|e| format!("viewport: no adapter ({e})"))?;
        Ok(Self::with(r, None, None))
    }

    /// Headless on one adapter.
    pub fn from_adapter(a: &wgpu::Adapter) -> Result<ViewGpu, String> {
        let r = Renderer::from_adapter(a).map_err(|e| format!("viewport: {e}"))?;
        Ok(Self::with(r, None, None))
    }

    /// The render core.
    pub fn renderer(&self) -> &Renderer {
        &self.renderer
    }

    /// The viewport texture (`Rgba8Unorm`, the last drawn size), once made.
    pub fn texture(&self) -> Option<&wgpu::Texture> {
        self.target.as_ref().map(|t| &t.texture)
    }

    /// The uploaded mesh, once made.
    pub fn mesh(&self) -> Option<&GpuMesh> {
        self.mesh.as_ref().map(|m| &m.1)
    }

    fn ensure_target(&mut self, w: u32, h: u32) {
        if self.target.as_ref().is_some_and(|t| t.texture.width() == w && t.texture.height() == h) {
            return;
        }
        let texture = self.renderer.device().create_texture(&wgpu::TextureDescriptor {
            label: Some("rcfd-studio viewport"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        if let Some(e) = &self.egui {
            let mut r = e.write();
            let device = self.renderer.device();
            match self.tex_id {
                None => self.tex_id = Some(r.register_native_texture(device, &view, wgpu::FilterMode::Nearest)),
                Some(id) => r.update_egui_texture_from_wgpu_texture(device, &view, wgpu::FilterMode::Nearest, id),
            }
        }
        self.target = Some(Target { texture, view });
    }

    /// Bring the mesh to `surface` with `faces` (one upload per surface, then scalar-only updates).
    fn set_field(&mut self, surface: &Arc<SurfaceMesh>, faces: &[f64]) -> Result<(), String> {
        match &mut self.mesh {
            Some((s, m)) if Arc::ptr_eq(s, surface) => {
                self.renderer
                    .update_scalar(m, Field::Face(faces))
                    .map_err(|e| format!("viewport: {e}"))?;
                self.scalar_updates += 1;
            }
            _ => {
                self.mesh = None;
                let m = self
                    .renderer
                    .upload(surface, Field::Face(faces))
                    .map_err(|e| format!("viewport: {e}"))?;
                self.mesh = Some((surface.clone(), m));
                self.uploads += 1;
            }
        }
        self.pick = None;
        Ok(())
    }

    fn render(&mut self, view: &View) -> Result<(), String> {
        let side = self.renderer.device().limits().max_texture_dimension_2d.min(MAX_SIDE);
        let (w, h) = (view.width.clamp(1, side), view.height.clamp(1, side));
        self.ensure_target(w, h);
        let (Some(t), Some((_, m))) = (&self.target, &self.mesh) else {
            return Ok(());
        };
        let mut v = view.clone();
        (v.width, v.height) = (w, h);
        self.renderer
            .render_into(m, &v, &t.view)
            .map_err(|e| format!("viewport: {e}"))?;
        self.renders += 1;
        self.pick = None;
        Ok(())
    }

    /// Surface face and rendered value at pixel `(x, y)` of the last image (one pick pass per image, cached).
    pub fn pick_at(&mut self, view: &View, x: u32, y: u32) -> Result<Option<(usize, f32)>, String> {
        if self.pick.is_none() {
            let Some((_, m)) = &self.mesh else { return Ok(None) };
            self.pick = Some(self.renderer.pick(m, view).map_err(|e| format!("viewport: {e}"))?);
        }
        Ok(self.pick.as_ref().and_then(|p| p.at(x, y)))
    }
}

// ---------------------------------------------------------------- model

/// How the colour range is chosen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RangeMode {
    /// Min and max of the shown values.
    Snapshot,
    /// Min and max over every stored snapshot of the field.
    Run,
    /// Typed by the user.
    Fixed(f64, f64),
}

/// What the viewport shows about its run.
#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    /// No run selected.
    Idle,
    /// The store is being opened.
    Loading,
    /// The run has no store (yet); the message says where it was looked for.
    NoStore(String),
    /// The store is open.
    Ready,
    /// The store could not be read (located message).
    Failed(String),
}

/// The hover readout.
#[derive(Debug, Clone, PartialEq)]
pub struct Hover {
    /// Surface face.
    pub face: usize,
    /// Owner cell of the face.
    pub cell: Option<usize>,
    /// The shown value of the face (the store's cell value; interpolated in linear mode).
    pub value: f64,
    /// The rendered (f32) value of the pick pass.
    pub picked: f32,
    /// True in linear mode between two snapshots.
    pub interpolated: bool,
}

impl Hover {
    /// The readout text.
    pub fn text(&self) -> String {
        format!(
            "value {}{}  (face {}, cell {})",
            self.value,
            if self.interpolated { ", interpolated" } else { "" },
            self.face,
            self.cell.map_or("-".to_string(), |c| c.to_string())
        )
    }
}

/// A legend: the range the image is drawn with and its tick labels.
#[derive(Debug, Clone, PartialEq)]
pub struct Legend {
    /// Low end.
    pub lo: f64,
    /// High end.
    pub hi: f64,
    /// Tick values, `lo` to `hi`.
    pub ticks: Vec<f64>,
    /// Tick labels (`{:.4e}`).
    pub labels: Vec<String>,
}

/// A tick label.
pub fn tick_label(v: f64) -> String {
    format!("{v:.4e}")
}

/// The viewport model.
pub struct Viewport {
    worker: Option<ViewWorker>,
    root: Option<PathBuf>,
    /// The run shown (follows the selected run).
    pub run: Option<String>,
    /// Store state.
    pub status: Status,
    /// The drawn surface.
    pub surface: Option<Arc<SurfaceMesh>>,
    /// True for a 2D case.
    pub two_d: bool,
    /// Stored snapshots.
    pub snaps: Vec<Snap>,
    /// Stored cell fields.
    pub fields: Vec<String>,
    /// The field shown.
    pub field: Option<String>,
    /// Slider position (time or step, see [`Viewport::axis`]).
    pub pos: f64,
    /// Linear interpolation in time.
    pub linear: bool,
    /// Colour range mode.
    pub range_mode: RangeMode,
    /// Over-run range of a field (by name), once read.
    pub run_range: Option<(String, Option<(f64, f64)>)>,
    /// Camera.
    pub camera: Option<CameraState>,
    /// The shown frame.
    pub frame: Option<Frame>,
    /// Playing.
    pub playing: bool,
    last_play: Option<Instant>,
    /// GPU (`None`: none attached, as in model tests; `Err`: located message, e.g. no adapter).
    pub gpu: Option<Result<ViewGpu, String>>,
    /// Last located error of a sample or of the GPU (the last image is kept).
    pub error: Option<String>,
    /// Hover readout.
    pub hover: Option<Hover>,
    /// Texture size in pixels.
    pub size: [u32; 2],
    /// Dark background.
    pub dark: bool,
    /// Where the image was last drawn (points; set by the UI).
    pub image_rect: Option<egui::Rect>,
    gen: u64,
    shown_gen: u64,
    pending: bool,
    field_dirty: bool,
    dirty: bool,
}

impl Default for Viewport {
    fn default() -> Self {
        Viewport {
            worker: None,
            root: None,
            run: None,
            status: Status::Idle,
            surface: None,
            two_d: false,
            snaps: Vec::new(),
            fields: Vec::new(),
            field: None,
            pos: 0.0,
            linear: false,
            range_mode: RangeMode::Snapshot,
            run_range: None,
            camera: None,
            frame: None,
            playing: false,
            last_play: None,
            gpu: None,
            error: None,
            hover: None,
            size: [640, 480],
            dark: true,
            image_rect: None,
            gen: 0,
            shown_gen: 0,
            pending: false,
            field_dirty: false,
            dirty: false,
        }
    }
}

impl Viewport {
    /// Attach the GPU (the app: from eframe's render state; tests: a headless one or its error).
    pub fn set_gpu(&mut self, gpu: Result<ViewGpu, String>) {
        self.gpu = Some(gpu);
        self.field_dirty = true;
        self.dirty = true;
    }

    /// The attached GPU, when there is one.
    pub fn gpu(&self) -> Option<&ViewGpu> {
        self.gpu.as_ref().and_then(|g| g.as_ref().ok())
    }

    /// The located GPU error, when the GPU could not be made.
    pub fn gpu_error(&self) -> Option<&str> {
        match &self.gpu {
            Some(Err(e)) => Some(e),
            _ => None,
        }
    }

    fn reset_run(&mut self) {
        self.run = None;
        self.status = Status::Idle;
        self.surface = None;
        self.snaps.clear();
        self.fields.clear();
        self.field = None;
        self.run_range = None;
        self.camera = None;
        self.frame = None;
        self.playing = false;
        self.error = None;
        self.hover = None;
        self.pending = false;
        self.dirty = true;
    }

    /// Close: stop the worker and forget the run (the GPU stays).
    pub fn close(&mut self) {
        self.worker = None;
        self.root = None;
        self.reset_run();
    }

    /// Follow the selected run of the open project at `root` (`runs/<id>/fields/`).
    pub fn sync(&mut self, root: Option<&Path>, selected: Option<&str>, period: Duration, wake: &Wake) {
        if self.root.as_deref() != root {
            self.close();
            self.root = root.map(Path::to_path_buf);
        }
        if self.run.as_deref() == selected {
            return;
        }
        self.reset_run();
        let (Some(root), Some(id)) = (root, selected) else { return };
        self.run = Some(id.to_string());
        self.status = Status::Loading;
        let w = self.worker.get_or_insert_with(|| ViewWorker::start(period, wake.clone()));
        w.send(Command::Open {
            run: id.to_string(),
            dir: root.join("runs").join(id).join("fields"),
        });
    }

    /// Apply what the worker sent; the number of events.
    pub fn drain(&mut self) -> usize {
        let Some(w) = &self.worker else { return 0 };
        let events: Vec<Event> = w.rx.try_iter().collect();
        let n = events.len();
        for e in events {
            self.apply(e);
        }
        n
    }

    fn mine(&self, run: &str) -> bool {
        self.run.as_deref() == Some(run)
    }

    fn apply(&mut self, e: Event) {
        match e {
            Event::Opened {
                run,
                surface,
                two_d,
                snaps,
                fields,
            } if self.mine(&run) => {
                let (lo, hi) = surface.bounds();
                self.camera = Some(CameraState::start(lo, hi, two_d));
                self.surface = Some(surface);
                self.two_d = two_d;
                self.snaps = snaps;
                self.field = fields
                    .iter()
                    .find(|f| f.as_str() == "p")
                    .or(fields.first())
                    .cloned();
                self.fields = fields;
                self.status = Status::Ready;
                self.pos = self.position_of(self.snaps.len().saturating_sub(1));
                self.request();
            }
            Event::NoStore { run, message } if self.mine(&run) => {
                self.status = Status::NoStore(message);
            }
            Event::Snaps { run, snaps, fields } if self.mine(&run) => {
                // a live run grew; follow the newest snapshot when the last one was shown
                let at_end = self.frame.as_ref().is_some_and(|f| f.index + 1 == self.snaps.len());
                self.snaps = snaps;
                if self.field.is_none() {
                    self.field = fields.iter().find(|f| f.as_str() == "p").or(fields.first()).cloned();
                }
                self.fields = fields;
                if self.run_range.is_some() {
                    self.run_range = None;
                    self.ask_run_range();
                }
                if at_end && !self.linear {
                    self.pos = self.position_of(self.snaps.len() - 1);
                    self.request();
                }
            }
            Event::Frame { run, gen, frame } if self.mine(&run) => {
                if gen >= self.shown_gen {
                    self.shown_gen = gen;
                    if gen == self.gen {
                        self.pending = false;
                    }
                    self.frame = Some(frame);
                    self.error = None;
                    self.field_dirty = true;
                    self.dirty = true;
                }
            }
            Event::RunRange { run, field, range } if self.mine(&run) => {
                self.run_range = Some((field, range));
                self.dirty = true;
            }
            Event::Failed { run, message } if self.mine(&run) => {
                self.pending = false;
                if self.surface.is_none() {
                    self.status = Status::Failed(message.clone());
                }
                self.error = Some(message);
            }
            _ => {}
        }
    }

    /// The slider axis.
    pub fn axis(&self) -> Axis {
        axis_of(&self.snaps)
    }

    /// The slider position of snapshot `i`.
    pub fn position_of(&self, i: usize) -> f64 {
        match (self.snaps.get(i), self.axis()) {
            (Some(s), Axis::Time) => s.time,
            (Some(s), Axis::Step) => s.step as f64,
            (None, _) => 0.0,
        }
    }

    /// Slider bounds.
    pub fn bounds(&self) -> (f64, f64) {
        (self.position_of(0), self.position_of(self.snaps.len().saturating_sub(1)))
    }

    /// Ask the worker for the field at the slider position (latest wins).
    pub fn request(&mut self) {
        let (Some(w), Some(field)) = (&self.worker, &self.field) else { return };
        self.gen += 1;
        self.pending = true;
        w.send(Command::Sample {
            gen: self.gen,
            field: field.clone(),
            pos: self.pos,
            linear: self.linear && self.axis() == Axis::Time,
        });
        if self.range_mode == RangeMode::Run
            && self.run_range.as_ref().is_none_or(|(f, _)| f != field)
        {
            self.ask_run_range();
        }
    }

    fn ask_run_range(&mut self) {
        if let (Some(w), Some(field)) = (&self.worker, &self.field) {
            w.send(Command::RunRange { field: field.clone() });
        }
    }

    /// True while a requested frame has not arrived.
    pub fn pending(&self) -> bool {
        self.pending
    }

    /// Move the slider.
    pub fn set_pos(&mut self, pos: f64) {
        let (a, b) = self.bounds();
        self.pos = pos.clamp(a.min(b), b.max(a));
        self.request();
    }

    /// Show snapshot `i` (clamped).
    pub fn show_snapshot(&mut self, i: usize) {
        if self.snaps.is_empty() {
            return;
        }
        self.pos = self.position_of(i.min(self.snaps.len() - 1));
        self.request();
    }

    /// Step one stored snapshot back (`-1`) or forward (`1`) from the shown one.
    pub fn step(&mut self, by: i64) {
        let cur = self.frame.as_ref().map_or(0, |f| f.index) as i64;
        let i = (cur + by).clamp(0, self.snaps.len() as i64 - 1).max(0);
        self.show_snapshot(i as usize);
    }

    /// Switch linear interpolation (time axis only).
    pub fn set_linear(&mut self, on: bool) {
        self.linear = on;
        self.request();
    }

    /// Show another field.
    pub fn set_field(&mut self, name: &str) {
        self.field = Some(name.to_string());
        self.request();
    }

    /// Change the range mode (a fixed range needs finite max > min).
    pub fn set_range_mode(&mut self, m: RangeMode) -> Result<(), String> {
        if let RangeMode::Fixed(a, b) = m {
            if !(a.is_finite() && b.is_finite() && b > a) {
                return Err(format!("colour range: need finite max > min, got {a} .. {b}"));
            }
        }
        self.range_mode = m;
        self.dirty = true;
        if m == RangeMode::Run {
            self.ask_run_range();
        }
        Ok(())
    }

    /// Advance while playing (one snapshot per [`PLAY_PERIOD`], stopping at the last); true when it moved.
    pub fn tick_play(&mut self, now: Instant) -> bool {
        if !self.playing || self.pending {
            return false;
        }
        if self.last_play.is_some_and(|t| now.duration_since(t) < PLAY_PERIOD) {
            return false;
        }
        let i = self.frame.as_ref().map_or(0, |f| f.index);
        if i + 1 >= self.snaps.len() {
            self.playing = false;
            return false;
        }
        self.last_play = Some(now);
        self.show_snapshot(i + 1);
        true
    }

    /// Start or stop playing (from the first snapshot when the last is shown).
    pub fn toggle_play(&mut self) {
        self.playing = !self.playing;
        self.last_play = None;
        if self.playing && self.frame.as_ref().is_some_and(|f| f.index + 1 >= self.snaps.len()) {
            self.show_snapshot(0);
        }
    }

    /// The step the plots mark.
    pub fn marker(&self) -> Option<f64> {
        self.frame.as_ref().map(|f| f.step)
    }

    /// The range the image is drawn with.
    pub fn drawn_range(&self) -> Option<(f64, f64)> {
        let f = self.frame.as_ref()?;
        match self.range_mode {
            RangeMode::Snapshot => f.range,
            RangeMode::Run => match &self.run_range {
                Some((name, r)) if *name == f.field => *r,
                _ => f.range,
            },
            RangeMode::Fixed(a, b) => Some((a, b)),
        }
    }

    /// The legend of the drawn range.
    pub fn legend(&self) -> Option<Legend> {
        let (lo, hi) = self.drawn_range()?;
        let ticks: Vec<f64> = (0..LEGEND_TICKS)
            .map(|k| {
                if k + 1 == LEGEND_TICKS {
                    hi
                } else {
                    lo + k as f64 * (hi - lo) / (LEGEND_TICKS - 1) as f64
                }
            })
            .collect();
        Some(Legend {
            lo,
            hi,
            labels: ticks.iter().map(|&v| tick_label(v)).collect(),
            ticks,
        })
    }

    /// The render core's view of the current state (`None` before a camera exists).
    pub fn view(&self) -> Option<View> {
        let c = self.camera?;
        let mut v = View::new(c.cam, self.size[0].max(1), self.size[1].max(1));
        v.background = if self.dark { [38, 38, 38, 255] } else { [255; 4] };
        v.colormap = ColorMap::viridis();
        v.shading = if self.two_d { Shading::Unlit } else { Shading::Flat };
        // a degenerate range is left to the render core (span 1), which `Some((c, c))` would refuse
        v.range = self.drawn_range().filter(|(a, b)| b > a);
        Some(v)
    }

    /// Set the texture size (pixels) and the theme; marks a redraw when they changed.
    pub fn set_size(&mut self, w: u32, h: u32, dark: bool) {
        let s = [w.clamp(1, MAX_SIDE), h.clamp(1, MAX_SIDE)];
        if s != self.size || dark != self.dark {
            self.size = s;
            self.dark = dark;
            self.dirty = true;
        }
    }

    /// Apply one mouse action.
    pub fn input(&mut self, i: Input) {
        let h = self.size[1] as f64;
        if let Some(c) = &mut self.camera {
            c.apply(i, h);
            self.dirty = true;
        }
    }

    /// GPU work of one frame: the field (upload once, then scalar-only) and the image when something changed.
    pub fn prepare(&mut self) {
        let (Some(Ok(gpu)), Some(surface), Some(frame)) = (&mut self.gpu, &self.surface, &self.frame) else {
            return;
        };
        if self.field_dirty {
            self.field_dirty = false;
            self.dirty = true;
            if let Err(e) = gpu.set_field(surface, &frame.faces) {
                self.error = Some(e);
            }
        }
        if self.dirty {
            self.dirty = false;
            self.hover = None;
            let Some(v) = self.view() else { return };
            if let Some(Ok(gpu)) = &mut self.gpu {
                if let Err(e) = gpu.render(&v) {
                    self.error = Some(e);
                }
            }
        }
    }

    /// The hover readout at texture pixel `(x, y)` (one pick pass per image, cached).
    pub fn hover_at(&mut self, x: u32, y: u32) -> Option<Hover> {
        let v = self.view()?;
        let frame = self.frame.as_ref()?;
        let surface = self.surface.as_ref()?;
        let Some(Ok(gpu)) = &mut self.gpu else { return None };
        match gpu.pick_at(&v, x, y) {
            Ok(Some((face, picked))) => Some(Hover {
                face,
                cell: surface.cell.get(face).copied().flatten(),
                value: frame.faces.get(face).copied()?,
                picked,
                interpolated: frame.interpolated,
            }),
            Ok(None) => None,
            Err(e) => {
                self.error = Some(e);
                None
            }
        }
    }

    /// Texture pixel of a world point under the current view (`None` behind the eye).
    pub fn pixel_of(&self, p: Vec3) -> Option<(f64, f64)> {
        let c = self.camera?;
        let q = c.cam.project(p, self.size[0] as f64, self.size[1] as f64)?;
        Some((q[0], q[1]))
    }
}
