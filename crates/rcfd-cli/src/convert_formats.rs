//! `rcfd convert` for the interchange formats of F-119 (ADR 0011): STL, Tecplot ASCII, XDMF and Plot3D input.
//! Parser errors are printed with the file name and a located message.

use rcfd_io::{plot3d, stl, tecplot, xdmf};
use rcfd_mesh::Mesh;
use std::path::Path;

fn ext(p: &str) -> String {
    Path::new(p)
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
}

fn opt<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

/// Extensions read as Plot3D grids.
pub fn is_plot3d(path: &str) -> bool {
    matches!(ext(path).as_str(), "x" | "xyz" | "p3d" | "grd")
}

/// Read a Plot3D grid file as a mesh (`--block n`, 1-based, default 1).
pub fn read_plot3d_mesh(path: &str, args: &[String]) -> Result<Mesh, String> {
    let block: usize = match opt(args, "--block") {
        Some(b) => b
            .parse()
            .ok()
            .filter(|&v| v >= 1)
            .ok_or("--block needs a positive integer")?,
        None => 1,
    };
    let g = plot3d::read_file(path).map_err(|e| e.to_string())?;
    plot3d::block_to_mesh(&g, block - 1).map_err(|e| format!("{path}: {e}"))
}

/// Handle the conversions that this module owns. `None` means "not mine": the caller continues with the mesh
/// formats it knew before. `read_mesh` reads the input as a mesh.
pub fn try_convert(
    input: &str,
    output: &str,
    args: &[String],
    read_mesh: &dyn Fn(&str) -> Result<Mesh, String>,
) -> Option<Result<String, String>> {
    let (ei, eo) = (ext(input), ext(output));
    if ei == "stl" {
        if eo != "stl" {
            return Some(Err(format!(
                "'{input}' is a surface (STL): it can only be re-encoded to .stl (use --ascii for ASCII), not converted to .{eo}"
            )));
        }
        return Some(restl(input, output, args));
    }
    if !matches!(eo.as_str(), "stl" | "dat" | "xmf") {
        return None;
    }
    Some(read_mesh(input).and_then(|mesh| write_mesh(&mesh, input, output, &eo, args)))
}

fn restl(input: &str, output: &str, args: &[String]) -> Result<String, String> {
    let solids = stl::read_file(input).map_err(|e| e.to_string())?;
    let n: usize = solids.iter().map(|s| s.triangles.len()).sum();
    let ascii = args.iter().any(|a| a == "--ascii");
    let mut out = Vec::new();
    if ascii {
        for s in &solids {
            out.extend(stl::write_ascii(s).map_err(|e| e.to_string())?.into_bytes());
        }
    } else {
        if solids.len() != 1 {
            return Err(format!(
                "{input}: {} solids cannot be merged into one binary STL; use --ascii",
                solids.len()
            ));
        }
        out = stl::write_binary(&solids[0]).map_err(|e| e.to_string())?;
    }
    std::fs::write(output, out).map_err(|e| format!("{output}: {e}"))?;
    Ok(format!(
        "converted {n} triangles: {input} -> {output} ({})",
        if ascii { "ASCII" } else { "binary" }
    ))
}

fn write_mesh(mesh: &Mesh, input: &str, output: &str, eo: &str, args: &[String]) -> Result<String, String> {
    match eo {
        "stl" => {
            let names: Vec<String> = match opt(args, "--patch") {
                Some(p) => p
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect(),
                None => stl::surface_patch_names(mesh),
            };
            let refs: Vec<&str> = names.iter().map(String::as_str).collect();
            let solid = stl::solid_from_patches(mesh, "rcfd", &refs).map_err(|e| e.to_string())?;
            let bytes = if args.iter().any(|a| a == "--ascii") {
                stl::write_ascii(&solid).map_err(|e| e.to_string())?.into_bytes()
            } else {
                stl::write_binary(&solid).map_err(|e| e.to_string())?
            };
            std::fs::write(output, bytes).map_err(|e| format!("{output}: {e}"))?;
            Ok(format!(
                "converted {} triangles of patches {}: {input} -> {output}",
                solid.triangles.len(),
                names.join(",")
            ))
        }
        "dat" => {
            let ds = tecplot::dataset_from_mesh(mesh, "rcfd", &[]).map_err(|e| e.to_string())?;
            let text = tecplot::write(&ds, tecplot::Packing::Block).map_err(|e| e.to_string())?;
            std::fs::write(output, text).map_err(|e| format!("{output}: {e}"))?;
            Ok(format!(
                "converted {} cells: {input} -> {output} (Tecplot ASCII)",
                mesh.n_cells()
            ))
        }
        _ => {
            let out = Path::new(output);
            let stem = out
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            let dir = out
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new("."));
            let files = xdmf::write(dir, &stem, mesh, &[]).map_err(|e| e.to_string())?;
            Ok(format!(
                "converted {} cells: {input} -> {output} (+ {} heavy-data files, binary)",
                mesh.n_cells(),
                files.len() - 1
            ))
        }
    }
}
