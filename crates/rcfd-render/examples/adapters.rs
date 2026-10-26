//! List the wgpu adapters of this machine with their limits, and render the 16 x 16 cavity boundary on each to
//! `<out-dir>/cavity_<backend>_<device>.png` (ADR 0125 step 0). Usage: `cargo run -p rcfd-render --example adapters [out-dir]`.
use rcfd_mesh::gen::cavity_2d;
use rcfd_render::{adapters, Camera, Field, Projection, Renderer, SurfaceMesh, View};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = std::path::PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| ".".into()));
    let mesh = cavity_2d(16, 1.0)?;
    let s = SurfaceMesh::from_mesh(&mesh, &[])?;
    let cells: Vec<f64> = mesh.cell_centres().iter().map(|c| c[0] * c[1]).collect();
    let faces = s.face_values_from_cells(&cells)?;
    let (lo, hi) = s.bounds();
    let view = View::new(
        Camera::fit(lo, hi, 0.5, 0.4, Projection::Perspective { fovy: 0.8 }),
        640,
        480,
    );
    let list = adapters();
    if list.is_empty() {
        println!("no DX12 or Vulkan adapter");
    }
    for a in list {
        let (i, l) = (a.get_info(), a.limits());
        println!(
            "{} | {:?} | {:?} | driver {} {} | max_texture_2d {} | max_buffer {} | max_storage_binding {}",
            i.name,
            i.backend,
            i.device_type,
            i.driver,
            i.driver_info,
            l.max_texture_dimension_2d,
            l.max_buffer_size,
            l.max_storage_buffer_binding_size
        );
        let r = Renderer::from_adapter(&a)?;
        let img = r.render(&r.upload(&s, Field::Face(&faces))?, &view)?;
        let path = out.join(format!("cavity_{:?}_{}.png", i.backend, i.device));
        std::fs::write(&path, img.to_png()?)?;
        println!("  wrote {}", path.display());
    }
    Ok(())
}
