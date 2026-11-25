//! F-889 (ADR 0797): the keys `cutoff`, `residual_vectors` and `load_patterns` of the table `[reduction]` of the solid case
//! file. The rows D1 and S1 are records of main 25311db5, made before any library line; the rest are the rows K1 to K5, E1,
//! R, R2, Z and N of the registration. The case files are texts of this file, written to `CARGO_TARGET_TMPDIR`.

use rcfd_api::solid_case::{run_solid_case_any, schema, CaseRun};
use rcfd_fem::builders::hex_box;
use rcfd_fem::cms::{reduce, Assembly, CmsOptions, ReducedPart};
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::{Constraint, Load, Material, SolidModel};
use serde_json::Value;
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
    fn finish(self) {
        assert!(self.failed.is_empty(), "{:#?}", self.failed);
    }
}

// ------------------------------------------------------------------------------------------------- the case texts

/// A fresh directory of this test under the target's temporary directory.
fn dir(tag: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("solid_case_reduction_keys_f889")
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

/// The case text of a half of the bar (`8 x 2 x 2` Hex8 on `0.5 x 0.1 x 0.08` from `x0`), clamped on `xmin` when asked, with
/// the node set `corner` at `(x0 + 0.5, 0.1, 0.08)` when asked, then `tail`.
fn half_text(x0: f64, clamp: bool, corner: bool, tail: &str) -> String {
    let mut t = format!(
        "[mesh]\nelement = \"hex8\"\norigin = [{x0:?}, 0.0, 0.0]\nlengths = [0.5, 0.1, 0.08]\ndivisions = [8, 2, 2]\n\n"
    );
    if corner {
        let x = x0 + 0.5;
        t.push_str(&format!(
            "[node_sets.corner]\nbox_min = [{x:?}, 0.1, 0.08]\nbox_max = [{x:?}, 0.1, 0.08]\n\n"
        ));
    }
    t.push_str("[materials.steel]\nyoungs_modulus = 2.1e11\npoisson_ratio = 0.3\ndensity = 7850.0\n\n");
    if clamp {
        t.push_str("[constraints.clamp]\nset = \"xmin\"\ncomponents = [\"x\", \"y\", \"z\"]\n\n");
    }
    t.push_str(tail);
    t
}

/// Part A of the registration with `tail` (loads, then `[reduction]`): clamped at `xmin`, interface `xmax`.
fn a_text(tail: &str) -> String {
    half_text(0.0, true, false, tail)
}

/// Part B with `tail`: free, interface `xmin`, the node set `corner`.
fn b_text(tail: &str) -> String {
    half_text(0.5, false, true, tail)
}

const TIP: &str = "[loads.tip]\nkind = \"nodal_force\"\nset = \"corner\"\nforce = [1.0, 1.0, 1.0]\n\n";
const M_LOADS: &str = "[loads.a_traction]\nkind = \"traction\"\nset = \"xmax\"\ntraction = [1.0e6, 2.0e6, -3.0e6]\n\n\
                       [loads.b_gravity]\nkind = \"gravity\"\nacceleration = [0.0, -9.81, -9.81]\n\n\
                       [loads.c_pressure]\nkind = \"pressure\"\nset = \"ymax\"\npressure = 2.0e5\n\n";

/// The `[reduction]` table with `keys` (lines) and the part file `file`.
fn reduction(interface: &str, keys: &str, file: &str) -> String {
    format!("[reduction]\ninterface = \"{interface}\"\n{keys}file = \"{file}\"\n")
}

fn any(path: &Path) -> CaseRun {
    run_solid_case_any(path, None).unwrap_or_else(|e| panic!("{}: {e:?}", path.display()))
}

/// Runs the part case `text` written as `name` in `d`; returns the bytes of its part file `file`.
fn part_bytes(d: &Path, name: &str, text: &str, file: &str) -> Vec<u8> {
    match any(&write(d, name, text)) {
        CaseRun::Reduction(_) => std::fs::read(d.join(file)).unwrap(),
        _ => panic!("{name} is not a reduction"),
    }
}

// ------------------------------------------------------------------------------------------------------ D1 and S1

/// The FNV-1a hash of bytes.
fn fnv(bytes: impl IntoIterator<Item = u8>) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn report_text(report: &Value) -> String {
    let mut r = report.clone();
    r.as_object_mut().unwrap().remove("outputs");
    serde_json::to_string_pretty(&r).unwrap()
}

/// Part A, B and Bm with the keys `keys` (lines of `[reduction]`), as the case texts of D1.
fn d1_cases() -> Vec<(String, String)> {
    let sets = [
        ("m0", "modes = 0\n"),
        ("m5", "modes = 5\n"),
        ("m5e1", "modes = 5\ninterface_enrichment = 1\n"),
    ];
    let mut out = Vec::new();
    for (tag, keys) in sets {
        out.push((format!("A-{tag}"), a_text(&reduction("xmax", keys, "part.cms"))));
        out.push((
            format!("B-{tag}"),
            b_text(&format!("{TIP}{}", reduction("xmin", keys, "part.cms"))),
        ));
        out.push((
            format!("Bm-{tag}"),
            b_text(&format!("{M_LOADS}{}", reduction("xmin", keys, "part.cms"))),
        ));
    }
    out
}

/// Two digests of a case of D1: the part file's bytes and the report without `outputs`.
fn d1_digests(text: &str, tag: &str) -> [u64; 2] {
    let d = dir(&format!("d1-{tag}"));
    let path = write(&d, "case.toml", text);
    match any(&path) {
        CaseRun::Reduction(r) => [
            fnv(std::fs::read(d.join("part.cms")).unwrap()),
            fnv(report_text(&r.report).bytes()),
        ],
        _ => panic!("{tag} is not a reduction"),
    }
}

/// The host of arrangement I: the half B with the traction and gravity of F-616, attached to A's part (modes = 5).
fn d1_host(modal: bool) -> u64 {
    let d = dir(if modal { "d1-host-modal" } else { "d1-host-static" });
    let part = a_text(&reduction("xmax", "modes = 5\n", "a.cms"));
    part_bytes(&d, "part.toml", &part, "a.cms");
    let mut host = half_text(
        0.5,
        false,
        false,
        "[loads.a_traction]\nkind = \"traction\"\nset = \"xmax\"\ntraction = [1.0e6, 2.0e6, -3.0e6]\n\n\
         [loads.b_gravity]\nkind = \"gravity\"\nacceleration = [0.0, -9.81, -9.81]\n\n\
         [superelements.half]\nfile = \"a.cms\"\nset = \"xmin\"\n\n",
    );
    if modal {
        host.push_str("[analysis]\ntype = \"modal\"\nmodes = 8\ntolerance = 1e-12\nresidual_test = \"backward\"\n");
    }
    match any(&write(&d, "host.toml", &host)) {
        CaseRun::Superelements(r) => fnv(report_text(&r.report).bytes()),
        _ => panic!("not a run with superelements"),
    }
}

/// The records of main 25311db5 (Windows x86_64 and Linux x86_64: IEEE operations and square roots only).
const D1: [(&str, [u64; 2]); 9] = [
    ("A-m0", [0x94d6a0a929b5dc94, 0xb25445f588201213]),
    ("B-m0", [0xb0b6bf16e6e9ed76, 0x54dbb9667397e230]),
    ("Bm-m0", [0x36775db86f61c38c, 0x54dbb9667397e230]),
    ("A-m5", [0xa70a3d9bd1e944b7, 0x08b193f3e583ac86]),
    ("B-m5", [0xa2f2aefb0b329699, 0xd51b94a6b55d66dd]),
    ("Bm-m5", [0x9dea17632dbdd196, 0xd51b94a6b55d66dd]),
    ("A-m5e1", [0xabb8aa47864d2456, 0xc6ded481d08de187]),
    ("B-m5e1", [0xae6fe5a9d8188478, 0x5d8592fddae66626]),
    ("Bm-m5e1", [0x91215a3e6ffaca0a, 0x5d8592fddae66626]),
];
const D1_HOST: [u64; 2] = [0xde0d5a2a00fe34bc, 0x7e4e75f630b4cff0];
/// The schema of main without the properties `cutoff`, `residual_vectors` and `load_patterns` of `[reduction]`.
const S1: u64 = 0x36594e64f4f9fb2d;

#[test]
fn d1_part_cases_without_the_new_keys() {
    let mut failed = Vec::new();
    for ((tag, text), want) in d1_cases().into_iter().zip(D1) {
        let got = d1_digests(&text, &tag);
        println!("    (\"{tag}\", [{:#018x}, {:#018x}]),", got[0], got[1]);
        if tag != want.0 || got != want.1 {
            failed.push(tag);
        }
    }
    let host = [d1_host(false), d1_host(true)];
    println!("HOST [{:#018x}, {:#018x}]", host[0], host[1]);
    if host != D1_HOST {
        failed.push("host".to_string());
    }
    assert!(failed.is_empty(), "D1 digests differ: {failed:?}");
}

#[test]
fn s1_schema_without_the_new_keys() {
    let mut v = schema().to_json_schema();
    // For the report of the additive diff: the whole schema as text, when a path is given.
    if let Ok(p) = std::env::var("F889_SCHEMA_OUT") {
        std::fs::write(p, serde_json::to_string_pretty(&v).unwrap()).unwrap();
    }
    let table = &mut v["properties"]["reduction"];
    for key in ["cutoff", "residual_vectors", "load_patterns"] {
        table["properties"].as_object_mut().unwrap().remove(key);
        if let Some(required) = table.get_mut("required").and_then(Value::as_array_mut) {
            required.retain(|x| x != key);
        }
    }
    let text = serde_json::to_string(&v).unwrap();
    let got = fnv(text.bytes());
    println!("S1 {got:#018x} ({} bytes)", text.len());
    assert_eq!(got, S1, "S1 schema digest differs");
}

// ------------------------------------------------------------------------------------- the same by library calls

/// Half of the bar from `x0` by library calls, with `loads`.
fn lib_half(x0: f64, clamp: bool, corner: bool, loads: &[Load]) -> SolidModel {
    let mut mesh = hex_box([x0, 0.0, 0.0], [0.5, 0.1, 0.08], [8, 2, 2]).unwrap();
    if corner {
        let at = [x0 + 0.5, 0.1, 0.08];
        let set: Vec<usize> = (0..mesh.nodes().len()).filter(|&i| mesh.nodes()[i] == at).collect();
        assert_eq!(set.len(), 1, "one node at {at:?}");
        mesh.add_node_set("corner", set).unwrap();
    }
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(2.1e11, 0.3, 7850.0, 0.0).unwrap())
        .unwrap();
    if clamp {
        for c in 0..3 {
            m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
        }
    }
    for l in loads {
        m.add_load(l.clone()).unwrap();
    }
    m
}

fn lib_a(loads: &[Load]) -> SolidModel {
    lib_half(0.0, true, false, loads)
}

fn lib_b(loads: &[Load]) -> SolidModel {
    lib_half(0.5, false, true, loads)
}

fn corner_force(force: [f64; 3]) -> Load {
    Load::nodal_force("corner", force)
}

fn traction() -> Load {
    Load::traction("xmax", [1.0e6, 2.0e6, -3.0e6])
}

fn gravity() -> Load {
    Load::gravity([0.0, -9.81, -9.81])
}

fn pressure() -> Load {
    Load::pressure("ymax", 2.0e5)
}

/// The loads of B3 in the order of their names.
fn f_loads() -> [Load; 3] {
    [
        corner_force([1.0, 0.0, 0.0]),
        corner_force([0.0, 1.0, 0.0]),
        corner_force([0.0, 0.0, 1.0]),
    ]
}

/// The load patterns of B3, of the three forces in the order given.
fn f_patterns(order: &[usize]) -> Vec<Vec<f64>> {
    order.iter().map(|&i| lib_pattern(&f_loads()[i])).collect()
}

/// The pattern of one load of B: the load vector of the model with that load alone.
fn lib_pattern(load: &Load) -> Vec<f64> {
    nodal_load_vector(&lib_b(std::slice::from_ref(load)), &[]).unwrap()
}

fn lib_part(model: &SolidModel, interface: &str, options: CmsOptions) -> ReducedPart {
    reduce(model, interface, &options.threads(1)).unwrap()
}

fn lib_text(model: &SolidModel, interface: &str, options: CmsOptions) -> Vec<u8> {
    lib_part(model, interface, options).to_text().into_bytes()
}

const F_LOADS: &str = "[loads.f_x]\nkind = \"nodal_force\"\nset = \"corner\"\nforce = [1.0, 0.0, 0.0]\n\n\
                       [loads.f_y]\nkind = \"nodal_force\"\nset = \"corner\"\nforce = [0.0, 1.0, 0.0]\n\n\
                       [loads.f_z]\nkind = \"nodal_force\"\nset = \"corner\"\nforce = [0.0, 0.0, 1.0]\n\n";
const CUTOFF: f64 = 32964.0;

fn b3_case(keys: &str) -> String {
    b_text(&format!("{F_LOADS}{}", reduction("xmin", keys, "part.cms")))
}

fn b_tip_case(keys: &str) -> String {
    b_text(&format!("{TIP}{}", reduction("xmin", keys, "part.cms")))
}

fn bm_case(keys: &str) -> String {
    b_text(&format!("{M_LOADS}{}", reduction("xmin", keys, "part.cms")))
}

fn a_case(keys: &str) -> String {
    a_text(&reduction("xmax", keys, "part.cms"))
}

fn run_part(d: &Path, name: &str, text: &str) -> (Vec<u8>, ReducedPart) {
    match any(&write(d, name, text)) {
        CaseRun::Reduction(r) => (std::fs::read(d.join("part.cms")).unwrap(), r.part),
        _ => panic!("{name} is not a reduction"),
    }
}

// --------------------------------------------------------------------------------------------------------- K1 to K5

#[test]
fn k1_cutoff_alone() {
    let mut ch = Checks::new();
    let d = dir("k1");
    for (tag, text, model, interface, modes) in [
        (
            "B",
            b_tip_case("cutoff = 32964.0\n"),
            lib_b(&[corner_force([1.0; 3])]),
            "xmin",
            9,
        ),
        ("A", a_case("cutoff = 32964.0\n"), lib_a(&[]), "xmax", 5),
    ] {
        let (bytes, part) = run_part(&d, &format!("k1-{tag}.toml"), &text);
        let want = lib_text(&model, interface, CmsOptions::new(0).cutoff(CUTOFF));
        ch.holds(
            &format!("K1 {tag}"),
            bytes == want && part.modes() == modes && part.residual_vectors() == 0,
            &format!(
                "{} and {} bytes, {} modes, {} residual vectors",
                bytes.len(),
                want.len(),
                part.modes(),
                part.residual_vectors()
            ),
        );
        let eight = lib_text(&model, interface, CmsOptions::new(0).cutoff(2.0 * CUTOFF));
        ch.holds(&format!("K1 {tag} differs"), bytes != eight, "from the cutoff 65928");
    }
    ch.finish();
}

#[test]
fn k2_residual_vectors_alone() {
    let mut ch = Checks::new();
    let d = dir("k2");
    let five = || CmsOptions::new(5);
    for (tag, loads, text) in [
        ("B3", f_loads().to_vec(), F_LOADS),
        ("B", vec![corner_force([1.0; 3])], TIP),
    ] {
        let case = |keys: &str| b_text(&format!("{text}{}", reduction("xmin", keys, "part.cms")));
        let model = lib_b(&loads);
        let (on, part) = run_part(
            &d,
            &format!("{tag}-on.toml"),
            &case("modes = 5\nresidual_vectors = true\n"),
        );
        let (off, _) = run_part(
            &d,
            &format!("{tag}-off.toml"),
            &case("modes = 5\nresidual_vectors = false\n"),
        );
        let (none, _) = run_part(&d, &format!("{tag}-none.toml"), &case("modes = 5\n"));
        let want_on = lib_text(&model, "xmin", five().residual_vectors(true));
        let want_off = lib_text(&model, "xmin", five());
        ch.holds(
            &format!("K2 {tag} on"),
            on == want_on && part.residual_vectors() == 1,
            &format!(
                "{} and {} bytes, {} vector",
                on.len(),
                want_on.len(),
                part.residual_vectors()
            ),
        );
        ch.holds(
            &format!("K2 {tag} false"),
            off == want_off && none == want_off,
            &format!("{}, {} and {} bytes", off.len(), none.len(), want_off.len()),
        );
        ch.holds(&format!("K2 {tag} differs"), on != want_off, "with the key and without");
    }
    ch.finish();
}

#[test]
fn k3_load_patterns_alone() {
    let mut ch = Checks::new();
    let d = dir("k3");
    let model = lib_b(&f_loads());
    let five = || CmsOptions::new(5);
    let (bytes, part) = run_part(
        &d,
        "k3.toml",
        &b3_case("modes = 5\nload_patterns = [\"f_z\", \"f_x\", \"f_y\"]\n"),
    );
    let want = lib_text(&model, "xmin", five().load_patterns(f_patterns(&[2, 0, 1])));
    let ascending = lib_text(&model, "xmin", five().load_patterns(f_patterns(&[0, 1, 2])));
    ch.holds(
        "K3 f_z, f_x, f_y",
        bytes == want,
        &format!(
            "{} and {} bytes, {} vectors",
            bytes.len(),
            want.len(),
            part.residual_vectors()
        ),
    );
    ch.holds(
        "K3 the order",
        bytes != ascending,
        "differs from the order f_x, f_y, f_z",
    );
    ch.finish();
}

#[test]
fn k4_combinations() {
    let mut ch = Checks::new();
    let d = dir("k4");
    let model = lib_b(&f_loads());
    for (tag, keys, cutoff, residual, patterns) in [
        ("cutoff+vector", "", true, true, false),
        ("cutoff+patterns", "", true, false, true),
        ("vector+patterns", "modes = 5\n", false, true, true),
        ("all three", "", true, true, true),
    ] {
        for blocks in [0, 1] {
            let mut text = keys.to_string();
            let mut options = CmsOptions::new(if cutoff { 0 } else { 5 }).interface_enrichment(blocks);
            if cutoff {
                text.push_str("cutoff = 32964.0\n");
                options = options.cutoff(CUTOFF);
            }
            if residual {
                text.push_str("residual_vectors = true\n");
                options = options.residual_vectors(true);
            }
            if patterns {
                text.push_str("load_patterns = [\"f_z\", \"f_x\", \"f_y\"]\n");
                options = options.load_patterns(f_patterns(&[2, 0, 1]));
            }
            if blocks > 0 {
                text.push_str(&format!("interface_enrichment = {blocks}\n"));
            }
            let name = format!("{}-{blocks}", tag.replace(['+', ' '], "-"));
            let (bytes, part) = run_part(&d, &format!("{name}.toml"), &b3_case(&text));
            let want = lib_text(&model, "xmin", options);
            ch.holds(
                &format!("K4 {tag}, {blocks} blocks"),
                bytes == want,
                &format!(
                    "{} and {} bytes, {} modes, {} vectors",
                    bytes.len(),
                    want.len(),
                    part.modes(),
                    part.residual_vectors()
                ),
            );
        }
    }
    ch.finish();
}

#[test]
fn k5_kinds_of_load_and_a_repeated_name() {
    let mut ch = Checks::new();
    let d = dir("k5");
    let model = lib_b(&[traction(), gravity(), pressure()]);
    let patterns = |loads: &[Load]| loads.iter().map(lib_pattern).collect::<Vec<_>>();
    let five = || CmsOptions::new(5);
    let (bytes, part) = run_part(
        &d,
        "k5.toml",
        &bm_case("modes = 5\nload_patterns = [\"c_pressure\", \"a_traction\"]\n"),
    );
    let want = lib_text(
        &model,
        "xmin",
        five().load_patterns(patterns(&[pressure(), traction()])),
    );
    ch.holds(
        "K5 pressure, traction",
        bytes == want && part.residual_vectors() == 2,
        &format!(
            "{} and {} bytes, {} vectors",
            bytes.len(),
            want.len(),
            part.residual_vectors()
        ),
    );
    let (twice, part2) = run_part(
        &d,
        "k5-twice.toml",
        &bm_case("modes = 5\nload_patterns = [\"a_traction\", \"c_pressure\", \"a_traction\"]\n"),
    );
    let want2 = lib_text(
        &model,
        "xmin",
        five().load_patterns(patterns(&[traction(), pressure(), traction()])),
    );
    ch.holds(
        "K5 a repeated name",
        twice == want2 && part2.residual_vectors() == 2,
        &format!(
            "{} and {} bytes, {} vectors",
            twice.len(),
            want2.len(),
            part2.residual_vectors()
        ),
    );
    ch.finish();
}

// ---------------------------------------------------------------------------------------------------------------- E1

/// E1 of ADR 0532 on both parts, as case files: A has no load, and its case therefore gives no `residual_vectors` (finding 2).
#[test]
fn e1_configuration_of_adr_0532() {
    let mut ch = Checks::new();
    let d = dir("e1");
    let a_text = a_text(&reduction(
        "xmax",
        "cutoff = 32964.0\ninterface_enrichment = 1\n",
        "a.cms",
    ));
    let b_text = b_text(&format!(
        "{TIP}{}",
        reduction(
            "xmin",
            "cutoff = 32964.0\nresidual_vectors = true\ninterface_enrichment = 1\n",
            "b.cms"
        )
    ));
    let a_bytes = part_bytes(&d, "a.toml", &a_text, "a.cms");
    let b_bytes = part_bytes(&d, "b.toml", &b_text, "b.cms");
    let e1 = || {
        CmsOptions::new(0)
            .cutoff(CUTOFF)
            .residual_vectors(true)
            .interface_enrichment(1)
    };
    let a_lib = lib_part(&lib_a(&[]), "xmax", e1());
    let b_lib = lib_part(&lib_b(&[corner_force([1.0; 3])]), "xmin", e1());
    ch.holds(
        "E1 A file",
        a_bytes == a_lib.to_text().into_bytes(),
        &format!("{} bytes", a_bytes.len()),
    );
    ch.holds(
        "E1 B file",
        b_bytes == b_lib.to_text().into_bytes(),
        &format!("{} bytes", b_bytes.len()),
    );
    ch.holds(
        "E1 vectors",
        a_lib.residual_vectors() == 27 && b_lib.residual_vectors() == 28,
        &format!("{} and {}", a_lib.residual_vectors(), b_lib.residual_vectors()),
    );
    let a_file = ReducedPart::read(&d.join("a.cms")).unwrap();
    let b_file = ReducedPart::read(&d.join("b.cms")).unwrap();
    let of = |a: &ReducedPart, b: &ReducedPart| {
        Assembly::new(vec![a.clone(), b.clone()])
            .unwrap()
            .eigen()
            .unwrap()
            .values
    };
    let got = of(&a_file, &b_file);
    let want = of(&a_lib, &b_lib);
    let same = got.len() == want.len() && got.iter().zip(&want).all(|(x, y)| x.to_bits() == y.to_bits());
    ch.holds("E1 eigenvalues", same, &format!("{} values equal in bits", got.len()));
    let freq = |v: &[f64]| v.iter().map(|l| l.max(0.0).sqrt()).collect::<Vec<f64>>();
    let same_f = freq(&got)
        .iter()
        .zip(&freq(&want))
        .all(|(x, y)| x.to_bits() == y.to_bits());
    ch.holds(
        "E1 frequencies",
        same_f,
        &format!("{:?} rad/s", freq(&got).iter().take(10).collect::<Vec<_>>()),
    );
    ch.finish();
}

// ------------------------------------------------------------------------------------------------------- R, R2, Z, N

/// The 1-based line of the last line of `text` that holds `marker`.
fn line_with(text: &str, marker: &str) -> usize {
    let lines: Vec<&str> = text.lines().collect();
    lines
        .iter()
        .rposition(|l| l.contains(marker))
        .unwrap_or_else(|| panic!("no line with {marker}"))
        + 1
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
    let shown: Option<Vec<String>> = got.as_ref().err().map(|e| {
        e.iter()
            .map(|x| format!("{} @ {:?}: {}", x.path, x.line, x.message))
            .collect()
    });
    ch.holds(
        id,
        ok,
        &format!("expected {key} on line {line} with '{needle}'; got {shown:?}"),
    );
}

#[test]
fn r_refusals() {
    let mut ch = Checks::new();
    let d = dir("r");
    let b3 = |keys: &str| b3_case(keys);
    // The cutoff: the library's refusals at the key, then the settings' own.
    for (id, value, needle) in [
        ("R cutoff zero", "0.0", "must be finite and positive"),
        ("R cutoff negative", "-1.0", "must be finite and positive"),
        ("R cutoff inf", "inf", ""),
        ("R cutoff nan", "nan", ""),
        ("R cutoff in Hz", "\"5246 Hz\"", ""),
        ("R cutoff with a unit", "\"32964 rad/s\"", ""),
    ] {
        let text = b3(&format!("cutoff = {value}\n"));
        refused(&mut ch, id, &d, &text, "reduction.cutoff", "cutoff =", needle);
    }
    let text = b3("modes = 5\ncutoff = 32964.0\n");
    refused(
        &mut ch,
        "R cutoff with a mode count",
        &d,
        &text,
        "reduction.cutoff",
        "cutoff =",
        "give a mode count or a cutoff",
    );
    // The load patterns.
    let text = b3("modes = 5\nload_patterns = [\"nowhere\"]\n");
    refused(
        &mut ch,
        "R unknown load",
        &d,
        &text,
        "reduction.load_patterns",
        "load_patterns",
        "nowhere",
    );
    let text = b3("modes = 5\nload_patterns = \"f_x\"\n");
    refused(
        &mut ch,
        "R patterns as a string",
        &d,
        &text,
        "reduction.load_patterns",
        "load_patterns",
        "",
    );
    let text = b3("modes = 5\nload_patterns = [1]\n");
    refused(
        &mut ch,
        "R patterns of numbers",
        &d,
        &text,
        "reduction.load_patterns.0",
        "load_patterns",
        "",
    );
    // The load's residual vector without an interior load, on A.
    let a_keys = "modes = 5\nresidual_vectors = true\n";
    refused(
        &mut ch,
        "R vector without a load",
        &d,
        &a_case(a_keys),
        "reduction.residual_vectors",
        "residual_vectors",
        "no interior part",
    );
    let text = a_text(&format!(
        "[loads.t]\nkind = \"traction\"\nset = \"xmax\"\ntraction = [1.0e6, 0.0, 0.0]\n\n{}",
        reduction("xmax", a_keys, "part.cms")
    ));
    refused(
        &mut ch,
        "R vector with an interface load only",
        &d,
        &text,
        "reduction.residual_vectors",
        "residual_vectors",
        "no interior part",
    );
    let on_clamp = "[loads.f_x]\nkind = \"nodal_force\"\nset = \"xmin\"\nforce = [1.0, 0.0, 0.0]\n\n";
    let text = a_text(&format!("{on_clamp}{}", reduction("xmax", a_keys, "part.cms")));
    refused(
        &mut ch,
        "R vector with a load on clamped nodes",
        &d,
        &text,
        "reduction.residual_vectors",
        "residual_vectors",
        "no interior part",
    );
    // A pattern of a load without an interior part.
    let pattern = "modes = 5\nload_patterns = [\"f_x\"]\n";
    let text = a_text(&format!("{on_clamp}{}", reduction("xmax", pattern, "part.cms")));
    refused(
        &mut ch,
        "R pattern on clamped nodes",
        &d,
        &text,
        "reduction.load_patterns",
        "load_patterns",
        "no interior part",
    );
    let text = a_text(&format!(
        "[loads.f_x]\nkind = \"traction\"\nset = \"xmax\"\ntraction = [1.0e6, 0.0, 0.0]\n\n{}",
        reduction("xmax", pattern, "part.cms")
    ));
    refused(
        &mut ch,
        "R pattern on the interface",
        &d,
        &text,
        "reduction.load_patterns",
        "load_patterns",
        "no interior part",
    );
    let text = b3("modes = 5\nresidual_vectors = \"yes\"\n");
    refused(
        &mut ch,
        "R vector as a string",
        &d,
        &text,
        "reduction.residual_vectors",
        "residual_vectors",
        "",
    );
    ch.finish();
}

#[test]
fn r2_two_problems_together() {
    let mut ch = Checks::new();
    let d = dir("r2");
    let text = a_case("modes = 5\nresidual_vectors = true\nload_patterns = [\"nowhere\"]\n");
    let e = run_solid_case_any(write(&d, "two.toml", &text), None).expect_err("two problems");
    for (key, marker) in [
        ("reduction.residual_vectors", "residual_vectors"),
        ("reduction.load_patterns", "load_patterns"),
    ] {
        let line = line_with(&text, marker);
        ch.holds(
            &format!("R2 {key}"),
            e.iter().any(|x| x.path == key && x.line == Some(line)),
            &format!("{:?}", e.iter().map(|x| (&x.path, x.line)).collect::<Vec<_>>()),
        );
    }
    ch.finish();
}

#[test]
fn z_interior_part_against_the_library() {
    let mut ch = Checks::new();
    let zero = |m: &SolidModel, interface: &str| {
        lib_part(m, interface, CmsOptions::new(5))
            .static_interior()
            .iter()
            .all(|x| *x == 0.0)
    };
    let refused = [
        ("no load", lib_a(&[])),
        (
            "a traction on xmax",
            lib_a(&[Load::traction("xmax", [1.0e6, 0.0, 0.0])]),
        ),
        ("a force on xmin", lib_a(&[Load::nodal_force("xmin", [1.0, 0.0, 0.0])])),
    ];
    for (tag, m) in &refused {
        ch.holds(&format!("Z {tag}"), zero(m, "xmax"), "static_interior is zero");
    }
    for (tag, m) in [
        ("B3", lib_b(&f_loads())),
        ("Bm", lib_b(&[traction(), gravity(), pressure()])),
    ] {
        ch.holds(&format!("Z {tag}"), !zero(&m, "xmin"), "static_interior is not zero");
    }
    ch.finish();
}

#[test]
fn n_accepted_loads() {
    let mut ch = Checks::new();
    let d = dir("n");
    let keys = "modes = 5\nresidual_vectors = true\n";
    let gravity = "[loads.b_gravity]\nkind = \"gravity\"\nacceleration = [0.0, -9.81, -9.81]\n\n";
    let on_interface = "[loads.a_traction]\nkind = \"traction\"\nset = \"xmax\"\ntraction = [1.0e6, 0.0, 0.0]\n\n";
    for (tag, loads) in [
        ("an interface and an interior load", format!("{on_interface}{gravity}")),
        ("an interior load", gravity.to_string()),
    ] {
        let text = a_text(&format!("{loads}{}", reduction("xmax", keys, "part.cms")));
        let got = run_solid_case_any(write(&d, "n.toml", &text), None);
        ch.holds(&format!("N {tag}"), got.is_ok(), &format!("{:?}", got.as_ref().err()));
    }
    let text = bm_case("modes = 5\nload_patterns = [\"b_gravity\"]\n");
    let got = run_solid_case_any(write(&d, "n-pattern.toml", &text), None);
    ch.holds(
        "N the pattern of gravity",
        got.is_ok(),
        &format!("{:?}", got.as_ref().err()),
    );
    ch.finish();
}
