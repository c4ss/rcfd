//! wgpu offscreen renderer (ADR 0125).

use std::sync::{mpsc, Arc};
use std::task::{Context, Poll, Wake, Waker};

use rcfd_core::vec3::Vec3;
use rcfd_core::{Error, Result};
use rcfd_post::render::Image;
use wgpu::util::DeviceExt;

use crate::camera::Camera;
use crate::surface::{Field, SurfaceMesh};
use crate::ColorMap;

/// Bytes per vertex. Layout (little endian): position f32x3 @0, flat normal f32x3 @12, smooth normal f32x3 @24,
/// scalar f32 @36, surface face u32 @40, part u32 @44, surface point u32 @48. The point id is not read by the
/// shader yet: it is the hook for deformation, a later per-point displacement storage buffer read as
/// `disp[point]` in the vertex shader without rebuilding this buffer.
pub const VERTEX_BYTES: u64 = 52;

/// Lighting and normals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shading {
    /// No lighting: pixels carry the exact colour-map (or solid) colour.
    Unlit,
    /// Two-sided headlight with face normals: `c (0.25 + 0.75 |n . l|)`, `l` the camera back vector.
    Flat,
    /// The same with point normals interpolated over each triangle.
    Smooth,
}

/// How one part is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartStyle {
    /// Coloured by the field (grey when there is none).
    Field,
    /// One solid colour.
    Solid([u8; 3]),
    /// Not drawn (and not pickable).
    Hidden,
}

/// Everything that is not the mesh: camera, image size, colouring, cut plane.
#[derive(Debug, Clone)]
pub struct View {
    /// Camera.
    pub camera: Camera,
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// Background colour.
    pub background: [u8; 4],
    /// Colour map.
    pub colormap: ColorMap,
    /// Colour range; `None` takes the min and max of the field.
    pub range: Option<(f64, f64)>,
    /// Shading.
    pub shading: Shading,
    /// Style per part index; parts beyond the list are [`PartStyle::Field`].
    pub parts: Vec<PartStyle>,
    /// Cut plane `(n, d)`: only points with `n . p <= d` are drawn (clipped per fragment in the shader).
    pub clip: Option<(Vec3, f64)>,
}

impl View {
    /// Unlit view with a white background, the viridis map, the field range, all parts drawn, no cut.
    pub fn new(camera: Camera, width: u32, height: u32) -> View {
        View {
            camera,
            width,
            height,
            background: [255; 4],
            colormap: ColorMap::viridis(),
            range: None,
            shading: Shading::Unlit,
            parts: Vec::new(),
            clip: None,
        }
    }
}

/// A surface and its field uploaded to one device.
pub struct GpuMesh {
    vertices: wgpu::Buffer,
    n_vertices: u32,
    n_parts: usize,
    range: Option<(f64, f64)>,
}

/// Result of a picking pass: per pixel the surface face (or none) and the interpolated field value.
#[derive(Debug, Clone)]
pub struct PickImage {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Surface face + 1 per pixel (0 = background), row-major from the top.
    pub ids: Vec<u32>,
    /// Field value per pixel (as rendered, f32; 0 where there is no face or no field).
    pub values: Vec<f32>,
}

impl PickImage {
    /// Surface face and field value at pixel `(x, y)`; `None` on background or outside the image.
    pub fn at(&self, x: u32, y: u32) -> Option<(usize, f32)> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let i = (y * self.width + x) as usize;
        self.ids[i].checked_sub(1).map(|f| (f as usize, self.values[i]))
    }
}

/// One adapter, its device and the two pipelines (colour and pick).
pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    info: wgpu::AdapterInfo,
    max_dim: u32,
    layout: wgpu::BindGroupLayout,
    colour: wgpu::RenderPipeline,
    pick: wgpu::RenderPipeline,
}

struct Unpark(std::thread::Thread);
impl Wake for Unpark {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

/// Run a future to completion on this thread (wgpu's futures resolve during the call on native backends).
fn block_on<F: std::future::Future>(f: F) -> F::Output {
    let mut f = std::pin::pin!(f);
    let waker = Waker::from(Arc::new(Unpark(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    loop {
        if let Poll::Ready(v) = f.as_mut().poll(&mut cx) {
            return v;
        }
        std::thread::park();
    }
}

const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const COLOUR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

fn gpu_err(e: impl std::fmt::Display) -> Error {
    Error::Other(format!("wgpu: {e}"))
}

/// Every DX12 and Vulkan adapter on this machine (the DX12 list includes the WARP software rasteriser).
pub fn adapters() -> Vec<wgpu::Adapter> {
    adapters_of(wgpu::Backends::DX12 | wgpu::Backends::VULKAN)
}

/// Adapters of the given back ends.
pub fn adapters_of(backends: wgpu::Backends) -> Vec<wgpu::Adapter> {
    let inst = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    block_on(inst.enumerate_adapters(backends))
}

impl Renderer {
    /// Renderer on the first adapter of [`adapters`], preferring a discrete GPU, then integrated, then any.
    pub fn new() -> Result<Renderer> {
        Self::first_of(wgpu::Backends::DX12 | wgpu::Backends::VULKAN)
    }

    /// Renderer on the best adapter of the given back ends; a located error when there is none.
    pub fn first_of(backends: wgpu::Backends) -> Result<Renderer> {
        let rank = |a: &wgpu::Adapter| match a.get_info().device_type {
            wgpu::DeviceType::DiscreteGpu => 0,
            wgpu::DeviceType::IntegratedGpu => 1,
            _ => 2,
        };
        let a = adapters_of(backends)
            .into_iter()
            .min_by_key(rank)
            .ok_or_else(|| Error::Other(format!("wgpu: no adapter for back ends {backends:?}")))?;
        Self::from_adapter(&a)
    }

    /// Renderer on a given adapter.
    pub fn from_adapter(adapter: &wgpu::Adapter) -> Result<Renderer> {
        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).map_err(gpu_err)?;
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("rcfd-render"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        let entry = |binding, visibility, ty| wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty: wgpu::BindingType::Buffer {
                ty,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                entry(0, wgpu::ShaderStages::VERTEX_FRAGMENT, wgpu::BufferBindingType::Uniform),
                entry(
                    1,
                    wgpu::ShaderStages::FRAGMENT,
                    wgpu::BufferBindingType::Storage { read_only: true },
                ),
            ],
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let attrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x3, 3 => Float32, 4 => Uint32, 5 => Uint32];
        let vbl = [Some(wgpu::VertexBufferLayout {
            array_stride: VERTEX_BYTES,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &attrs,
        })];
        let pipeline = |fs: &str, targets: &[Option<wgpu::ColorTargetState>]| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(fs),
                layout: Some(&pl),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &vbl,
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::Less),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets,
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let colour = pipeline("fs_colour", &[Some(COLOUR.into())]);
        let pick = pipeline(
            "fs_pick",
            &[
                Some(wgpu::TextureFormat::R32Uint.into()),
                Some(wgpu::TextureFormat::R32Float.into()),
            ],
        );
        Ok(Renderer {
            max_dim: device.limits().max_texture_dimension_2d,
            device,
            queue,
            info: adapter.get_info(),
            layout,
            colour,
            pick,
        })
    }

    /// Adapter name, back end, type and driver.
    pub fn info(&self) -> &wgpu::AdapterInfo {
        &self.info
    }

    /// Upload a surface with its field (a field change is a new upload).
    pub fn upload(&self, s: &SurfaceMesh, field: Field) -> Result<GpuMesh> {
        let range = s.field_range(field)?;
        let normals = s.point_normals();
        let face_n: Vec<Vec3> = (0..s.n_faces())
            .map(|f| {
                let a = s.face_area_vector(f);
                rcfd_core::vec3::scale(a, 1.0 / rcfd_core::vec3::norm(a).max(1e-300))
            })
            .collect();
        let mut bytes = Vec::new();
        let mut n = 0u32;
        let f32s = |b: &mut Vec<u8>, v: &[f64]| v.iter().for_each(|x| b.extend_from_slice(&(*x as f32).to_le_bytes()));
        for (f, tri) in s.triangles() {
            for p in tri {
                f32s(&mut bytes, &s.points[p]);
                f32s(&mut bytes, &face_n[f]);
                f32s(&mut bytes, &normals[p]);
                let v = match field {
                    Field::None => 0.0,
                    Field::Face(v) => v[f],
                    Field::Vertex(v) => v[p],
                };
                f32s(&mut bytes, &[v]);
                for id in [f, s.face_part[f], p] {
                    bytes.extend_from_slice(&(id as u32).to_le_bytes());
                }
                n += 1;
            }
        }
        if bytes.is_empty() {
            return Err(Error::Mesh("surface has no faces".into()));
        }
        let vertices = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("vertices"),
            contents: &bytes,
            usage: wgpu::BufferUsages::VERTEX,
        });
        Ok(GpuMesh {
            vertices,
            n_vertices: n,
            n_parts: s.parts.len(),
            range,
        })
    }

    fn check_size(&self, view: &View) -> Result<()> {
        let ok = |d: u32| (1..=self.max_dim).contains(&d);
        if !ok(view.width) || !ok(view.height) {
            return Err(Error::InvalidValue {
                path: "view.size".into(),
                reason: format!(
                    "{}x{} outside 1..={} on {}",
                    view.width, view.height, self.max_dim, self.info.name
                ),
            });
        }
        Ok(())
    }

    fn bind_group(&self, mesh: &GpuMesh, view: &View) -> Result<wgpu::BindGroup> {
        let (lo, hi) = match (view.range, mesh.range) {
            (Some((a, b)), _) if !(a.is_finite() && b.is_finite() && b > a) => {
                return Err(Error::InvalidValue {
                    path: "view.range".into(),
                    reason: "colour range needs finite max > min".into(),
                })
            }
            (Some(r), _) | (None, Some(r)) => r,
            (None, None) => (0.0, 1.0),
        };
        let span = if hi > lo { hi - lo } else { 1.0 };
        let stops = &view.colormap.0;
        if !(2..=16).contains(&stops.len()) {
            return Err(Error::InvalidValue {
                path: "view.colormap".into(),
                reason: format!("{} stops, need 2..=16", stops.len()),
            });
        }
        let aspect = view.width as f64 / view.height as f64;
        let vp = view.camera.view_proj(aspect);
        let light = view.camera.basis()[2];
        let mode = match view.shading {
            Shading::Unlit => 0.0,
            Shading::Flat => 1.0,
            Shading::Smooth => 2.0,
        };
        let (cn, cd, on) = match view.clip {
            Some((n, d)) => (n, d, 1.0),
            None => ([0.0; 3], 0.0, 0.0),
        };
        let mut u: Vec<f32> = Vec::with_capacity(16 + 16 + 64);
        for c in 0..4 {
            u.extend((0..4).map(|r| vp[r][c] as f32)); // column-major for WGSL
        }
        u.extend([light[0], light[1], light[2], mode].map(|x| x as f32));
        u.extend([cn[0], cn[1], cn[2], cd].map(|x| x as f32));
        u.extend([lo, 1.0 / span, on, stops.len() as f64].map(|x| x as f32));
        u.extend([if mesh.range.is_some() { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0]);
        for k in 0..16 {
            let s = stops.get(k).copied().unwrap_or([0; 3]);
            u.extend([s[0] as f32, s[1] as f32, s[2] as f32, 0.0]);
        }
        let parts: Vec<f32> = (0..mesh.n_parts.max(1))
            .flat_map(|p| match view.parts.get(p).copied().unwrap_or(PartStyle::Field) {
                PartStyle::Hidden => [0.0; 4],
                PartStyle::Field => [0.0, 0.0, 0.0, 1.0],
                PartStyle::Solid(c) => [c[0] as f32, c[1] as f32, c[2] as f32, 2.0],
            })
            .collect();
        let buf = |data: &[f32], usage| {
            let b: Vec<u8> = data.iter().flat_map(|x| x.to_le_bytes()).collect();
            self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: &b,
                usage,
            })
        };
        let ub = buf(&u, wgpu::BufferUsages::UNIFORM);
        let sb = buf(&parts, wgpu::BufferUsages::STORAGE);
        Ok(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: ub.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: sb.as_entire_binding(),
                },
            ],
        }))
    }

    fn target(&self, view: &View, format: wgpu::TextureFormat) -> wgpu::Texture {
        self.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: view.width,
                height: view.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        })
    }

    /// Draw `mesh` into the given 4-byte-per-pixel targets and read them back (rows unpadded).
    fn draw(&self, mesh: &GpuMesh, view: &View, pick: bool, clear: &[wgpu::Color]) -> Result<Vec<Vec<u8>>> {
        self.check_size(view)?;
        let bg = self.bind_group(mesh, view)?;
        let formats: &[wgpu::TextureFormat] = if pick {
            &[wgpu::TextureFormat::R32Uint, wgpu::TextureFormat::R32Float]
        } else {
            &[COLOUR]
        };
        let targets: Vec<wgpu::Texture> = formats.iter().map(|f| self.target(view, *f)).collect();
        let views: Vec<wgpu::TextureView> = targets.iter().map(|t| t.create_view(&Default::default())).collect();
        let depth = self.target(view, DEPTH);
        let depth_view = depth.create_view(&Default::default());
        let mut enc = self.device.create_command_encoder(&Default::default());
        {
            let attachments: Vec<Option<wgpu::RenderPassColorAttachment>> = views
                .iter()
                .zip(clear)
                .map(|(v, c)| {
                    Some(wgpu::RenderPassColorAttachment {
                        view: v,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(*c),
                            store: wgpu::StoreOp::Store,
                        },
                    })
                })
                .collect();
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &attachments,
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            rp.set_pipeline(if pick { &self.pick } else { &self.colour });
            rp.set_bind_group(0, &bg, &[]);
            rp.set_vertex_buffer(0, mesh.vertices.slice(..));
            rp.draw(0..mesh.n_vertices, 0..1);
        }
        // Copies need rows padded to 256 bytes; the padding is stripped after mapping.
        let row = view.width as u64 * 4;
        let padded = row.div_ceil(256) * 256;
        let size = wgpu::Extent3d {
            width: view.width,
            height: view.height,
            depth_or_array_layers: 1,
        };
        let bufs: Vec<wgpu::Buffer> = targets
            .iter()
            .map(|t| {
                let b = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: None,
                    size: padded * view.height as u64,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                });
                enc.copy_texture_to_buffer(
                    t.as_image_copy(),
                    wgpu::TexelCopyBufferInfo {
                        buffer: &b,
                        layout: wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(padded as u32),
                            rows_per_image: Some(view.height),
                        },
                    },
                    size,
                );
                b
            })
            .collect();
        self.queue.submit([enc.finish()]);
        let (tx, rx) = mpsc::channel();
        for b in &bufs {
            let tx = tx.clone();
            b.slice(..).map_async(wgpu::MapMode::Read, move |r| {
                let _ = tx.send(r);
            });
        }
        self.device.poll(wgpu::PollType::wait_indefinitely()).map_err(gpu_err)?;
        for _ in &bufs {
            rx.recv().map_err(gpu_err)?.map_err(gpu_err)?;
        }
        let mut out = Vec::new();
        for b in &bufs {
            let data = b.slice(..).get_mapped_range().map_err(|e| gpu_err(format!("{e:?}")))?;
            let mut v = Vec::with_capacity((row * view.height as u64) as usize);
            for r in data.chunks(padded as usize) {
                v.extend_from_slice(&r[..row as usize]);
            }
            out.push(v);
        }
        Ok(out)
    }

    /// Render to an RGBA image.
    pub fn render(&self, mesh: &GpuMesh, view: &View) -> Result<Image> {
        let c = view.background.map(|x| x as f64 / 255.0);
        let clear = wgpu::Color {
            r: c[0],
            g: c[1],
            b: c[2],
            a: c[3],
        };
        let rgba = self.draw(mesh, view, false, &[clear])?.remove(0);
        Ok(Image {
            width: view.width as usize,
            height: view.height as usize,
            rgba,
        })
    }

    /// Picking pass: per pixel the visible surface face and its field value.
    pub fn pick(&self, mesh: &GpuMesh, view: &View) -> Result<PickImage> {
        let mut out = self.draw(mesh, view, true, &[wgpu::Color::TRANSPARENT; 2])?;
        let values = out.pop().unwrap_or_default();
        let ids = out.pop().unwrap_or_default();
        let words = |b: &[u8]| -> Vec<[u8; 4]> { b.chunks_exact(4).map(|c| [c[0], c[1], c[2], c[3]]).collect() };
        Ok(PickImage {
            width: view.width,
            height: view.height,
            ids: words(&ids).into_iter().map(u32::from_le_bytes).collect(),
            values: words(&values).into_iter().map(f32::from_le_bytes).collect(),
        })
    }
}
