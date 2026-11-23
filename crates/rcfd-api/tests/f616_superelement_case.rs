//! F-616 (ADR 0785): the keys `superelements` and `reduction.interface_enrichment` of the solid case file. The checks K1 to
//! K3, B1 to B6 and R1 to R4 of the registration: the solid bar of ADR 0536 run from case files against the same models by
//! library calls, in bits. The case files are texts of this file, written to `CARGO_TARGET_TMPDIR`.

use rcfd_api::case::Diagnostic;
use rcfd_api::solid_case::{
    run_solid_case_any, run_solid_case_file, CaseRun, ReductionRun, SolidCase, SuperOutcome, SuperRun,
};
use rcfd_fem::builders::hex_box;
use rcfd_fem::cms::superelement::{SuperModes, SuperStatic, SuperelementModel};
use rcfd_fem::cms::{reduce, CmsOptions, ReducedPart};
use rcfd_fem::modal::ModalOptions;
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{Constraint, Load, Material, SolidModel};
use rcfd_linalg::eigen::ResidualTest;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

struct Checks {
    failed: Vec<String>,
}

impl Checks {
    fn new() -> Checks {
        Checks { failed: Vec::new() }
    }
    fn holds(&mut self, id: &str, ok: bool, detail: &str) {
        println!("{id}: {} ({detail})", if ok { "met" } else { "MISSED" });
        if !ok {
            self.failed.push(format!("{id}: {detail}"));
        }
    }
    /// `a` and `b` have the same length and the same bits in every entry.
    fn bits(&mut self, id: &str, a: &[f64], b: &[f64]) {
        let same = a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits());
        let diff = a.iter().zip(b).filter(|(x, y)| x.to_bits() != y.to_bits()).count();
        self.holds(
            id,
            same && !a.is_empty(),
            &format!("{} and {} values, {diff} differ in bits", a.len(), b.len()),
        );
    }
    fn finish(self) {
        assert!(self.failed.is_empty(), "{:#?}", self.failed);
    }
}

// ------------------------------------------------------------------------------------------------- the case texts

/// A fresh directory of this test under the target's temporary directory.
fn dir(tag: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("f616_superelement_case")
        .join(tag);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn write(dir: &Path, name: &str, text: &str) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, text).unwrap();
    p
}

/// The case text of a third of the bar of ADR 0536 from `x0` (`8 x 2 x 2` Hex8 on `0.5 x 0.1 x 0.08`), clamped on `xmin`
/// and with the traction on `xmax` when asked, with gravity; then `tail`.
fn box_text(x0: f64, clamp: bool, tip: bool, tail: &str) -> String {
    let mut t = format!(
        "[mesh]\nelement = \"hex8\"\norigin = [{x0:?}, 0.0, 0.0]\nlengths = [0.5, 0.1, 0.08]\ndivisions = [8, 2, 2]\n\n\
         [materials.steel]\nyoungs_modulus = 2.1e11\npoisson_ratio = 0.3\ndensity = 7850.0\n\n"
    );
    if clamp {
        t.push_str("[constraints.clamp]\nset = \"xmin\"\ncomponents = [\"x\", \"y\", \"z\"]\n\n");
    }
    if tip {
        t.push_str("[loads.a_traction]\nkind = \"traction\"\nset = \"xmax\"\ntraction = [1.0e6, 2.0e6, -3.0e6]\n\n");
    }
    t.push_str("[loads.b_gravity]\nkind = \"gravity\"\nacceleration = [0.0, -9.81, -9.81]\n\n");
    t.push_str(tail);
    t
}

fn reduction_tail(interface: &str, extra: &str, file: &str) -> String {
    format!("[reduction]\ninterface = \"{interface}\"\nmodes = 5\n{extra}file = \"{file}\"\n")
}

fn super_tail(entries: &[(&str, &str, &str)]) -> String {
    entries
        .iter()
        .map(|(name, file, set)| format!("[superelements.{name}]\nfile = \"{file}\"\nset = \"{set}\"\n\n"))
        .collect()
}

const MODAL: &str = "[analysis]\ntype = \"modal\"\nmodes = 8\ntolerance = 1e-12\nresidual_test = \"backward\"\n";

// -------------------------------------------------------------------------------------- the same by library calls

fn lib_box(x0: f64, len: f64, nx: usize, clamp: bool, tip: bool) -> SolidModel {
    let mesh = hex_box([x0, 0.0, 0.0], [len, 0.1, 0.08], [nx, 2, 2]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(2.1e11, 0.3, 7850.0, 0.0).unwrap())
        .unwrap();
    if clamp {
        for c in 0..3 {
            m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
        }
    }
    if tip {
        m.add_load(Load::traction("xmax", [1.0e6, 2.0e6, -3.0e6])).unwrap();
    }
    m.add_load(Load::gravity([0.0, -9.81, -9.81])).unwrap();
    m
}

fn lib_third(x0: f64, clamp: bool, tip: bool) -> SolidModel {
    lib_box(x0, 0.5, 8, clamp, tip)
}

fn lib_part(model: &SolidModel, interface: &str, blocks: usize) -> ReducedPart {
    reduce(
        model,
        interface,
        &CmsOptions::new(5).interface_enrichment(blocks).threads(1),
    )
    .unwrap()
}

fn lib_combined(host: SolidModel, parts: &[&ReducedPart]) -> SuperelementModel {
    let mut s = SuperelementModel::new(host).unwrap();
    for p in parts {
        s.add_part((*p).clone()).unwrap();
    }
    s
}

fn lib_modes(s: &SuperelementModel) -> SuperModes {
    s.modal_analysis(
        &ModalOptions::new(8)
            .tolerance(1e-12)
            .residual_test(ResidualTest::Backward)
            .threads(1),
    )
    .unwrap()
}

// ----------------------------------------------------------------------------------------------------- the runs

fn run_any(path: &Path) -> CaseRun {
    run_solid_case_any(path, None).unwrap_or_else(|e| panic!("{}: {e:#?}", path.display()))
}

fn reduction_of(path: &Path) -> ReductionRun {
    match run_any(path) {
        CaseRun::Reduction(r) => *r,
        _ => panic!("{}: a reduction expected", path.display()),
    }
}

fn super_of(path: &Path) -> SuperRun {
    match run_any(path) {
        CaseRun::Superelements(r) => *r,
        _ => panic!("{}: a run with superelements expected", path.display()),
    }
}

fn static_of(r: &SuperRun) -> &SuperStatic {
    match &r.outcome {
        SuperOutcome::Static(s) => s,
        SuperOutcome::Modal(_) => panic!("static expected"),
    }
}

fn modes_of(r: &SuperRun) -> &SuperModes {
    match &r.outcome {
        SuperOutcome::Modal(m) => m,
        SuperOutcome::Static(_) => panic!("modal expected"),
    }
}

fn flat(v: &[Vec<f64>]) -> Vec<f64> {
    v.iter().flatten().copied().collect()
}

/// The static and modal runs of a host case text against the library's combined model.
fn bar_checks(
    ch: &mut Checks,
    ids: (&str, &str),
    d: &Path,
    host_text: &str,
    lib: &SuperelementModel,
) -> (SuperRun, SuperRun) {
    let st = super_of(&write(d, "host_static.toml", host_text));
    let want = lib.solve_static(Some(1)).unwrap();
    ch.bits(
        &format!("{} coordinates", ids.0),
        &static_of(&st).coordinates,
        &want.coordinates,
    );
    ch.bits(
        &format!("{} host displacement", ids.0),
        &static_of(&st).host_displacement,
        &want.host_displacement,
    );
    let mo = super_of(&write(d, "host_modal.toml", &format!("{host_text}{MODAL}")));
    let want = lib_modes(lib);
    ch.bits(
        &format!("{} eigenvalues", ids.1),
        &modes_of(&mo).eigenvalues,
        &want.eigenvalues,
    );
    ch.bits(
        &format!("{} vectors", ids.1),
        &flat(&modes_of(&mo).vectors),
        &flat(&want.vectors),
    );
    ch.holds(
        &format!("{} eight modes", ids.1),
        modes_of(&mo).vectors.len() == 8 && want.vectors.len() == 8,
        &format!("{} and {}", modes_of(&mo).vectors.len(), want.vectors.len()),
    );
    (st, mo)
}

// ----------------------------------------------------------------------------------------------------- the checks

#[test]
fn k1_b1_b3_b5_b6_arrangement_one() {
    let mut ch = Checks::new();
    let d = dir("arrangement_one");
    let red = reduction_of(&write(
        &d,
        "part_a.toml",
        &box_text(0.0, true, false, &reduction_tail("xmax", "", "a.cms")),
    ));
    let a = lib_third(0.0, true, false);
    let part = lib_part(&a, "xmax", 0);
    ch.holds(
        "K1 the part of the part case",
        red.part.to_text() == part.to_text(),
        "the text of the part in the run against the library's",
    );
    let host_text = box_text(0.5, false, true, &super_tail(&[("half", "a.cms", "xmin")]));
    // K1: the setup of the host case.
    let setup = SolidCase::load(write(&d, "host_k1.toml", &host_text))
        .unwrap()
        .build()
        .unwrap();
    let e = setup.superelements.first();
    ch.holds(
        "K1 the setup's entry",
        setup.superelements.len() == 1
            && e.is_some_and(|e| {
                e.name == "half"
                    && e.set == "xmin"
                    && e.file == "a.cms"
                    && e.path == d.join("a.cms")
                    && e.part.to_text() == part.to_text()
            })
            && setup.reduction.is_none(),
        &format!(
            "{} entries: {:?}",
            setup.superelements.len(),
            e.map(|e| (&e.name, &e.set, &e.file))
        ),
    );
    // B1, B3.
    let lib = lib_combined(lib_third(0.5, false, true), &[&part]);
    let (st, mo) = bar_checks(&mut ch, ("B1 I static", "B3 I modal"), &d, &host_text, &lib);
    println!(
        "record: arrangement I, {} combined unknowns ({} of the host)",
        lib.n_unknowns(),
        lib.n_host_unknowns()
    );
    // B5: the host displacements against the full bar.
    let full = lib_box(0.0, 1.0, 16, true, true);
    let uf = solve_static(&full, &StaticOptions::default()).unwrap();
    let scale = uf.displacement_values().iter().fold(0.0f64, |m, x| m.max(x.abs()));
    let host = &st.setup.model;
    let u = &static_of(&st).host_displacement;
    let mut worst = 0.0f64;
    let mut matched = 0;
    for (n, p) in host.mesh().nodes().iter().enumerate() {
        let f: Vec<usize> = (0..full.mesh().nodes().len())
            .filter(|&k| (0..3).all(|c| (full.mesh().nodes()[k][c] - p[c]).abs() <= 1e-12))
            .collect();
        assert_eq!(f.len(), 1, "node of the full bar at {p:?}");
        matched += 1;
        for c in 0..3 {
            let got = u[host.dof_map().equation(n, c).unwrap()];
            let d = (got - uf.displacement(f[0])[c]).abs();
            worst = if d.is_nan() { f64::NAN } else { worst.max(d) };
        }
    }
    let err = worst / scale;
    ch.holds(
        "B5 the host displacements against the full bar",
        err.is_finite() && err <= 1e-10 && matched == 81,
        &format!("{err:.3e} (limit <= 1e-10), {matched} nodes"),
    );
    // B6: the outputs.
    let r = &st.report;
    let s0 = &r["superelements"][0];
    ch.holds(
        "B6 the static report",
        r["format"] == json!("rcfd-solid-report/1")
            && r["analysis"] == json!("static")
            && r["unknowns"] == json!(248)
            && s0["name"] == json!("half")
            && s0["file"] == json!("a.cms")
            && s0["set"] == json!("xmin")
            && s0["boundary_unknowns"] == json!(27)
            && s0["generalised_coordinates"] == json!(5)
            && s0["offset"] == json!(243)
            && r["displacement"]["max_magnitude"].as_f64().is_some_and(|x| x > 0.0)
            && r["solver"]["name"] == json!("skyline-ldlt"),
        &format!("unknowns {}, entry {s0}", r["unknowns"]),
    );
    let vtu = std::fs::read_to_string(d.join("result.vtu")).unwrap_or_default();
    let on_disk: Value = serde_json::from_str(&std::fs::read_to_string(d.join("report.json")).unwrap_or_default())
        .unwrap_or(Value::Null);
    ch.holds(
        "B6 the files of the modal run (the later of the two in this directory)",
        vtu.contains("Name=\"mode_8\"") && on_disk["analysis"] == json!("modal") && mo.written.len() == 2,
        &format!("{} bytes of .vtu, {} files", vtu.len(), mo.written.len()),
    );
    let st_dir = dir("arrangement_one_static_files");
    std::fs::copy(d.join("a.cms"), st_dir.join("a.cms")).unwrap();
    let st2 = super_of(&write(&st_dir, "host_static.toml", &host_text));
    let vtu = std::fs::read_to_string(st_dir.join("result.vtu")).unwrap_or_default();
    ch.holds(
        "B6 the .vtu of the static run",
        vtu.contains("Name=\"displacement\"") && st2.written.len() == 2,
        &format!("{} bytes, {} files", vtu.len(), st2.written.len()),
    );
    let want = lib_modes(&lib);
    let eig: Vec<f64> = mo.report["eigenvalues"]
        .as_array()
        .map(|a| a.iter().filter_map(Value::as_f64).collect())
        .unwrap_or_default();
    ch.bits("B6 the eigenvalues of the modal report", &eig, &want.eigenvalues);
    ch.holds(
        "B6 the modal report",
        mo.report["analysis"] == json!("modal")
            && mo.report["frequencies"].as_array().is_some_and(|f| f.len() == 8)
            && mo.report["solver"]["name"] == json!("subspace"),
        &format!("{}", mo.report["frequencies"]),
    );
    ch.finish();
}

#[test]
fn b2_b3_arrangement_two() {
    let mut ch = Checks::new();
    let d = dir("arrangement_two");
    reduction_of(&write(
        &d,
        "part_b.toml",
        &box_text(0.5, false, true, &reduction_tail("xmin", "", "b.cms")),
    ));
    let part = lib_part(&lib_third(0.5, false, true), "xmin", 0);
    let host_text = box_text(0.0, true, false, &super_tail(&[("half", "b.cms", "xmax")]));
    let lib = lib_combined(lib_third(0.0, true, false), &[&part]);
    bar_checks(&mut ch, ("B2 II static", "B3 II modal"), &d, &host_text, &lib);
    println!(
        "record: arrangement II, {} combined unknowns ({} of the host)",
        lib.n_unknowns(),
        lib.n_host_unknowns()
    );
    ch.finish();
}

#[test]
fn b4_two_parts_in_name_order() {
    let mut ch = Checks::new();
    let d = dir("two_parts");
    reduction_of(&write(
        &d,
        "part_a.toml",
        &box_text(0.0, true, false, &reduction_tail("xmax", "", "a.cms")),
    ));
    reduction_of(&write(
        &d,
        "part_c.toml",
        &box_text(1.0, false, true, &reduction_tail("xmin", "", "c.cms")),
    ));
    let a = lib_part(&lib_third(0.0, true, false), "xmax", 0);
    let c = lib_part(&lib_third(1.0, false, true), "xmin", 0);
    // The tables are written in the other order: the reader sorts the names.
    let host_text = box_text(
        0.5,
        false,
        false,
        &super_tail(&[("b_tip", "c.cms", "xmax"), ("a_root", "a.cms", "xmin")]),
    );
    let st = super_of(&write(&d, "host.toml", &host_text));
    let lib = lib_combined(lib_third(0.5, false, false), &[&a, &c]);
    let want = lib.solve_static(Some(1)).unwrap();
    ch.bits("B4 coordinates", &static_of(&st).coordinates, &want.coordinates);
    ch.bits(
        "B4 host displacement",
        &static_of(&st).host_displacement,
        &want.host_displacement,
    );
    let names: Vec<&str> = st.setup.superelements.iter().map(|e| e.name.as_str()).collect();
    ch.holds(
        "B4 the order of the parts",
        names == ["a_root", "b_tip"] && st.model.n_parts() == 2 && st.model.part_offset(1) == lib.part_offset(1),
        &format!(
            "{names:?}, {} combined unknowns, part 1 from {}",
            lib.n_unknowns(),
            lib.part_offset(1)
        ),
    );
    ch.finish();
}

#[test]
fn r1_r2_r3_interface_enrichment() {
    let mut ch = Checks::new();
    let d = dir("enrichment");
    let case = |extra: &str, file: &str| box_text(0.0, true, false, &reduction_tail("xmax", extra, file));
    let with = reduction_of(&write(&d, "part_1.toml", &case("interface_enrichment = 1\n", "a1.cms")));
    reduction_of(&write(&d, "part_none.toml", &case("", "a.cms")));
    reduction_of(&write(&d, "part_0.toml", &case("interface_enrichment = 0\n", "a0.cms")));
    let a = lib_third(0.0, true, false);
    let (plain, rich) = (lib_part(&a, "xmax", 0), lib_part(&a, "xmax", 1));
    let read = |f: &str| std::fs::read_to_string(d.join(f)).unwrap();
    // R1.
    ch.holds(
        "R1 the part file with interface_enrichment = 1",
        read("a1.cms") == rich.to_text(),
        "the file's bytes against the library part's text",
    );
    ch.holds(
        "R1 the residual vectors",
        with.part.residual_vectors() >= 1 && with.part.residual_vectors() == rich.residual_vectors(),
        &format!(
            "{} in the run, {} in the library ({} without the option)",
            with.part.residual_vectors(),
            rich.residual_vectors(),
            plain.residual_vectors()
        ),
    );
    ch.holds(
        "R1 the report",
        with.report["analysis"] == json!("reduction")
            && with.report["reduction"]["interface_enrichment"] == json!(1)
            && with.report["reduction"]["modes"] == json!(5)
            && with.report["reduction"]["boundary_unknowns"] == json!(27)
            && with.report["reduction"]["interior_unknowns"] == json!(189)
            && with.report["reduction"]["residual_vectors"] == json!(rich.residual_vectors())
            && with.report["reduction"]["reduced_unknowns"] == json!(rich.n_reduced()),
        &format!("{}", with.report["reduction"]),
    );
    // R2.
    ch.holds(
        "R2 without the key and with 0",
        read("a.cms") == plain.to_text() && read("a0.cms") == plain.to_text(),
        "both files against the library part's text without the option",
    );
    ch.holds(
        "R2 the file of R1 differs",
        read("a1.cms") != read("a.cms"),
        &format!("{} and {} bytes", read("a1.cms").len(), read("a.cms").len()),
    );
    // R3.
    let host_text = box_text(0.5, false, true, &super_tail(&[("half", "a1.cms", "xmin")]));
    let lib = lib_combined(lib_third(0.5, false, true), &[&rich]);
    bar_checks(&mut ch, ("R3 static", "R3 modal"), &d, &host_text, &lib);
    println!(
        "record: arrangement I with the enriched part, {} combined unknowns",
        lib.n_unknowns()
    );
    ch.finish();
}

// ----------------------------------------------------------------------------------------------------- refusals

/// The 1-based line of the last line of `text` that holds `marker`.
fn line_with(text: &str, marker: &str) -> usize {
    let lines: Vec<&str> = text.lines().collect();
    lines
        .iter()
        .rposition(|l| l.contains(marker))
        .unwrap_or_else(|| panic!("no line with {marker}"))
        + 1
}

fn show(e: &[Diagnostic]) -> Vec<String> {
    e.iter().map(ToString::to_string).collect()
}

/// The case `text` is refused with a problem at `key`, on the line of `marker`, whose message holds `needle`.
fn refused(ch: &mut Checks, id: &str, d: &Path, text: &str, key: &str, marker: &str, needle: &str) {
    let name: String = id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let line = line_with(text, marker);
    let got = run_solid_case_any(write(d, &format!("{name}.toml"), text), None);
    let ok = got.as_ref().is_err_and(|e| {
        e.iter()
            .any(|x| x.path == key && x.line == Some(line) && x.message.contains(needle))
    });
    ch.holds(
        id,
        ok,
        &format!(
            "expected {key} on line {line} with '{needle}'; got {:?}",
            got.as_ref().err().map(|e| show(e))
        ),
    );
}

#[test]
fn k2_k3_refusals_of_superelements() {
    let mut ch = Checks::new();
    let d = dir("refusals_superelements");
    lib_part(&lib_third(0.0, true, false), "xmax", 0)
        .write(&d.join("a.cms"))
        .unwrap();
    write(&d, "bad.cms", "not a part file\n");
    let host = |clamp: bool, tail: &str| box_text(0.5, clamp, true, tail);
    let entry = |body: &str| format!("[superelements.half]\n{body}");
    let good = entry("file = \"a.cms\"\nset = \"xmin\"\n");
    // The control: the case every text below is a change of runs.
    ch.holds(
        "K2 control",
        run_solid_case_any(write(&d, "control.toml", &host(false, &good)), None).is_ok(),
        "the unchanged case runs",
    );
    let h = "[superelements.half]";
    refused(
        &mut ch,
        "K2 file missing",
        &d,
        &host(false, &entry("set = \"xmin\"\n")),
        "superelements.half.file",
        h,
        "required",
    );
    refused(
        &mut ch,
        "K2 set missing",
        &d,
        &host(false, &entry("file = \"a.cms\"\n")),
        "superelements.half.set",
        h,
        "required",
    );
    refused(
        &mut ch,
        "K2 file absent",
        &d,
        &host(false, &entry("file = \"none.cms\"\nset = \"xmin\"\n")),
        "superelements.half.file",
        "none.cms",
        "none.cms",
    );
    refused(
        &mut ch,
        "K2 not a part file",
        &d,
        &host(false, &entry("file = \"bad.cms\"\nset = \"xmin\"\n")),
        "superelements.half.file",
        "bad.cms",
        "bad.cms",
    );
    refused(
        &mut ch,
        "K2 set unknown",
        &d,
        &host(false, &entry("file = \"a.cms\"\nset = \"nowhere\"\n")),
        "superelements.half.set",
        "nowhere",
        "does not exist",
    );
    refused(
        &mut ch,
        "K2 the part is not on the set",
        &d,
        &host(false, &entry("file = \"a.cms\"\nset = \"xmax\"\n")),
        "superelements.half.set",
        "set = \"xmax\"",
        "no node of the set",
    );
    refused(
        &mut ch,
        "K2 the set has nodes the part does not hold",
        &d,
        &host(false, &entry("file = \"a.cms\"\nset = \"all\"\n")),
        "superelements.half.set",
        "set = \"all\"",
        "does not hold",
    );
    refused(
        &mut ch,
        "K2 the host clamped on the interface",
        &d,
        &host(true, &good),
        "superelements.half",
        h,
        "fixed",
    );
    refused(
        &mut ch,
        "K2 a host constraint value",
        &d,
        &host(
            false,
            &format!("[constraints.push]\nset = \"ymax\"\ncomponents = [\"y\"]\nvalue = 1e-3\n\n{good}"),
        ),
        "superelements",
        h,
        "",
    );
    refused(
        &mut ch,
        "K2 solver.method = cg",
        &d,
        &host(false, &format!("{good}\n[solver]\nmethod = \"cg\"\n")),
        "solver.method",
        "method = \"cg\"",
        "direct",
    );
    // K3: the entries of a case without the keys refuse a case with them.
    let part_case = box_text(0.0, true, false, &reduction_tail("xmax", "", "k3.cms"));
    for (id, text, key) in [
        (
            "K3 run_solid_case_file on the host case",
            host(false, &good),
            "superelements",
        ),
        ("K3 run_solid_case_file on the part case", part_case, "reduction"),
    ] {
        let p = write(&d, &format!("{key}_k3.toml"), &text);
        let got = run_solid_case_file(&p, None);
        ch.holds(
            id,
            got.as_ref().is_err_and(|e| {
                e.iter()
                    .any(|x| x.path == key && x.line.is_some() && x.message.contains("run_solid_case_any"))
            }),
            &format!("{:?}", got.as_ref().err().map(|e| show(e))),
        );
    }
    ch.holds(
        "K3 nothing written by the refused part case",
        !d.join("k3.cms").exists(),
        "no part file",
    );
    ch.finish();
}

#[test]
fn r4_refusals_of_the_reduction() {
    let mut ch = Checks::new();
    let d = dir("refusals_reduction");
    let case = |tail: &str| box_text(0.0, true, false, tail);
    let good = "[reduction]\ninterface = \"xmax\"\nmodes = 5\n";
    ch.holds(
        "R4 control",
        run_solid_case_any(write(&d, "control.toml", &case(good)), None).is_ok() && d.join("part.cms").exists(),
        "the unchanged case runs and writes part.cms (the default name)",
    );
    refused(
        &mut ch,
        "R4 interface missing",
        &d,
        &case("[reduction]\nmodes = 5\n"),
        "reduction.interface",
        "[reduction]",
        "required",
    );
    refused(
        &mut ch,
        "R4 interface unknown",
        &d,
        &case("[reduction]\ninterface = \"nowhere\"\nmodes = 5\n"),
        "reduction.interface",
        "nowhere",
        "does not exist",
    );
    refused(
        &mut ch,
        "R4 analysis given",
        &d,
        &case(&format!("{good}\n[analysis]\ntype = \"static\"\n")),
        "analysis",
        "[analysis]",
        "does not apply to a reduction",
    );
    refused(
        &mut ch,
        "R4 solver given",
        &d,
        &case(&format!("{good}\n[solver]\nmethod = \"direct\"\n")),
        "solver",
        "[solver]",
        "does not apply to a reduction",
    );
    refused(
        &mut ch,
        "R4 superelements given",
        &d,
        &case(&format!(
            "{good}\n[superelements.x]\nfile = \"part.cms\"\nset = \"xmax\"\n"
        )),
        "superelements",
        "[superelements.x]",
        "does not apply to a reduction",
    );
    refused(
        &mut ch,
        "R4 output.vtu given",
        &d,
        &case(&format!("{good}\n[output]\nvtu = \"x.vtu\"\n")),
        "output.vtu",
        "x.vtu",
        "does not apply to a reduction",
    );
    refused(
        &mut ch,
        "R4 modes above the range",
        &d,
        &case("[reduction]\ninterface = \"xmax\"\nmodes = 100000\n"),
        "reduction.modes",
        "modes = 100000",
        "",
    );
    refused(
        &mut ch,
        "R4 modes above the interior size",
        &d,
        &case("[reduction]\ninterface = \"xmax\"\nmodes = 500\n"),
        "reduction",
        "[reduction]",
        "500",
    );
    ch.finish();
}
