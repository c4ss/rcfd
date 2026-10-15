//! Helpers shared by the wave 16W wiring tests (ADR 0021): a minimal reader of the ASCII VTK files the repository writes.
#![allow(dead_code)]

/// Values of the `DataArray` called `name` (all components, in file order).
pub fn vtk_array(text: &str, name: &str) -> Vec<f64> {
    let key = format!("Name=\"{name}\"");
    let i = text.find(&key).unwrap_or_else(|| panic!("array {name} not in file"));
    let rest = &text[i..];
    let start = rest.find('>').unwrap() + 1;
    let end = rest.find("</DataArray>").unwrap();
    rest[start..end]
        .split_whitespace()
        .map(|t| t.parse::<f64>().unwrap())
        .collect()
}

pub fn has_array(text: &str, name: &str) -> bool {
    text.contains(&format!("Name=\"{name}\""))
}

/// `NumberOfCells` of the first piece.
pub fn vtu_cells(text: &str) -> usize {
    let i = text.find("NumberOfCells=\"").unwrap() + "NumberOfCells=\"".len();
    let j = text[i..].find('"').unwrap();
    text[i..i + j].parse().unwrap()
}

pub fn tmpdir(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_w16w_{}_{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}
