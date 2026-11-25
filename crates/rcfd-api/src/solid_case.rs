//! A solid case file of its own (F-386, ADR 0438; stub ADR 0287) and the library entry that runs it.
//!
//! A TOML file describes a linear solid model of `rcfd-fem`: the mesh (the box generator of `rcfd_fem::builders` or a Gmsh
//! `.msh` file read by `rcfd_io::gmsh`), node sets selected by a box, materials, displacement constraints and loads on named
//! sets, a static or modal analysis, the linear solver and the outputs. [`run_solid_case_file`] reads, builds and runs it and
//! writes the `.vtu` (`rcfd_post::solid_output::write_solid_vtu`; for a modal analysis [`modal_vtu_string`]) and a JSON
//! report (displacement and stress extremes, reactions per constraint, frequencies, solver statistics).
//!
//! The file has its own schema ([`schema`]), separate from the flow case schema: nothing here changes a flow case, its schema,
//! its hashes, the CLI or the MCP tool list. Every problem is a [`Diagnostic`] with the dotted key and, where the key or its
//! table is in the file, the line, all problems of a file returned together (as the flow reader, `crate::case`).
//!
//! Named tables (`[constraints.<name>]`, ...) are processed in ascending order of their names. The format is documented in
//! `docs/solid-case-file.md`.
//!
//! Two opt-in tables of F-616 (ADR 0785), run by [`run_solid_case_any`]: `[superelements.<name>]` attaches reduced part
//! files of ADR 0469 to the model at node sets (`rcfd_fem::cms::superelement`, ADR 0536) for the static solve and the modal
//! analysis; `[reduction]` reduces the model to a part file instead of solving it, with the interface enrichment of ADR
//! 0532 by its key `interface_enrichment`, the cutoff and the load's residual vector of ADR 0474 by `cutoff` and
//! `residual_vectors`, and the load patterns of ADR 0511 by `load_patterns` (F-889, ADR 0797). A case without them takes the
//! paths it took before, bit for bit.

use crate::case::{collect_errors, locate_line, Diagnostic};
use rcfd_core::settings::{Schema, SettingsTree};
use rcfd_core::units::Dimension;
use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::{hex_box, tet_box, upgrade_to_tet10};
use rcfd_fem::cms::superelement::{SuperModes, SuperStatic, SuperelementModel};
use rcfd_fem::cms::{reduce, CmsOptions, ReducedPart, MATCH_TOLERANCE};
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::modal::{modal_analysis, ModalOptions, ModalResult};
use rcfd_fem::recover::von_mises;
use rcfd_fem::solve::{solve_static_report, CgPreconditioner, IcOrdering, SolveReport, StaticOptions, StaticSolver};
use rcfd_fem::{Constraint, FemError, Load, Material, SolidMesh, SolidModel, SolidResult};
use rcfd_linalg::amg_elastic::ElasticAmgSettings;
use rcfd_linalg::eigen::ResidualTest;
use rcfd_linalg::precond::IcOptions;
use rcfd_post::solid_output::{solid_fields, solid_vtu_string, vtk_from_fem};
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};

/// Format name written into the report.
pub const REPORT_FORMAT: &str = "rcfd-solid-report/1";

/// A list of three lengths.
fn len3() -> Schema {
    Schema::list(Schema::float(0.0).dim(Dimension::LENGTH))
}

/// The schema of a solid case file (ADR 0438 D1). Separate from `crate::schema::case_schema`.
pub fn schema() -> Schema {
    let mesh = Schema::object()
        .doc("The mesh: the box generator or a Gmsh .msh file")
        .field(
            "source",
            Schema::enumeration("generator", &["generator", "file"]).doc("Where the mesh comes from"),
        )
        .field(
            "generator",
            Schema::enumeration("box", &["box"])
                .doc("The generator (source = generator): the box [origin, origin + lengths]"),
        )
        .field(
            "element",
            Schema::enumeration("hex8", &["hex8", "tet4", "tet10"])
                .doc("Element of the box: hex8 (hex_box), tet4 (tet_box, six per cell), tet10 (tet_box upgraded)"),
        )
        .field(
            "origin",
            len3().doc("Lowest corner of the box (3 lengths); empty = the origin"),
        )
        .field(
            "lengths",
            len3().doc("Edge lengths of the box (3 lengths, required for the generator)"),
        )
        .field(
            "divisions",
            Schema::list(Schema::int(1).range(1.0, 1.0e6))
                .doc("Cells along x, y, z (3 integers, required for the generator)"),
        )
        .field(
            "file",
            Schema::string("").doc("Gmsh .msh file (source = file), relative to the case file"),
        )
        .field(
            "quadratic",
            Schema::boolean(false).doc("source = file: upgrade a mesh of Tet4 to Tet10 by mid-edge nodes"),
        );
    let node_set = Schema::object()
        .doc("A node set: the nodes p with box_min <= p <= box_max componentwise (inclusive)")
        .field("box_min", len3().doc("Lower corner of the selection box (3 lengths)"))
        .field("box_max", len3().doc("Upper corner of the selection box (3 lengths)"));
    let material = Schema::object()
        .doc("An isotropic linear elastic material on an element set")
        .field("element_set", Schema::string("all").doc("The element set"))
        .field(
            "youngs_modulus",
            Schema::float(0.0)
                .dim(Dimension::PRESSURE)
                .doc("Young's modulus (required)"),
        )
        .field("poisson_ratio", Schema::float(0.0).doc("Poisson ratio (required)"))
        .field("density", Schema::float(0.0).dim(Dimension::DENSITY).doc("Density"))
        .field(
            "expansion",
            Schema::float(0.0)
                .unit("1/K")
                .doc("Thermal expansion coefficient (a plain number per kelvin)"),
        );
    let constraint = Schema::object()
        .doc("A prescribed displacement of the components of a node set")
        .field("set", Schema::string("").doc("The node set (required)"))
        .field(
            "components",
            Schema::list(Schema::enumeration("x", &["x", "y", "z"])).doc("The components (required)"),
        )
        .field(
            "value",
            Schema::float(0.0)
                .dim(Dimension::LENGTH)
                .doc("The prescribed displacement"),
        );
    let load = Schema::object()
        .doc("A load")
        .field(
            "kind",
            Schema::enumeration("traction", &["traction", "pressure", "nodal_force", "gravity"])
                .doc("The kind of load"),
        )
        .field(
            "set",
            Schema::string("").doc("A surface set (traction, pressure) or a node set (nodal_force); none for gravity"),
        )
        .field(
            "traction",
            Schema::list(Schema::float(0.0).dim(Dimension::PRESSURE))
                .doc("kind = traction: the traction vector (3 pressures)"),
        )
        .field(
            "pressure",
            Schema::float(0.0)
                .dim(Dimension::PRESSURE)
                .doc("kind = pressure: the pressure, positive pressing on the solid"),
        )
        .field(
            "force",
            Schema::list(Schema::float(0.0).dim(Dimension::FORCE))
                .doc("kind = nodal_force: the force on each node (3 forces)"),
        )
        .field(
            "acceleration",
            Schema::list(Schema::float(0.0).dim(Dimension::ACCELERATION))
                .doc("kind = gravity: the acceleration (3 accelerations)"),
        );
    let analysis = Schema::object()
        .doc("The analysis")
        .field(
            "type",
            Schema::enumeration("static", &["static", "modal"]).doc("Static solve or the lowest modes"),
        )
        .field(
            "modes",
            Schema::int(6).range(1.0, 1.0e4).doc("modal: the number of modes"),
        )
        .field(
            "tolerance",
            Schema::float(1e-10)
                .range(0.0, 1.0)
                .doc("modal: the residual tolerance of each pair"),
        )
        .field(
            "residual_test",
            Schema::enumeration("relative", &["relative", "backward"]).doc("modal: the residual measure (ADR 0354)"),
        );
    let solver = Schema::object()
        .doc("The linear solver of the static solve")
        .field(
            "method",
            Schema::enumeration("direct", &["direct", "cg"]).doc("direct (envelope LDL^T) or preconditioned CG"),
        )
        .field(
            "preconditioner",
            Schema::enumeration(
                "ic_fill",
                &[
                    "block_jacobi",
                    "ic0",
                    "ic_fill",
                    "amg",
                    "amg_quadratic",
                    "amg_quadratic_smoothed",
                ],
            )
            .doc("method = cg: the preconditioner (ADRs 0282, 0357, 0305, 0370, 0375)"),
        )
        .field(
            "fill_level",
            Schema::int(1)
                .range(0.0, 16.0)
                .doc("preconditioner = ic_fill: the level of fill"),
        )
        .field(
            "ordering",
            Schema::enumeration("natural", &["natural", "node_rcm"]).doc("preconditioner = ic_fill: the ordering"),
        )
        .field(
            "tolerance",
            Schema::float(1e-10)
                .range(0.0, 1.0)
                .doc("method = cg: the relative residual tolerance"),
        )
        .field(
            "max_iterations",
            Schema::int(20000)
                .range(1.0, 1.0e9)
                .doc("method = cg: the iteration cap"),
        );
    let output = Schema::object()
        .doc("Outputs, relative to the case file's directory (or the output directory of the run)")
        .field("vtu", Schema::string("result.vtu").doc("The .vtu file (empty: none)"))
        .field(
            "report",
            Schema::string("report.json").doc("The JSON report (empty: none)"),
        );
    let superelement = Schema::object()
        .doc("A reduced part of ADR 0469 attached to the model at a node set (ADR 0536)")
        .field(
            "file",
            Schema::string("").doc("The part file (required), relative to the case file"),
        )
        .field(
            "set",
            Schema::string("").doc("The node set of the model the part attaches to (required)"),
        );
    let reduction = Schema::object()
        .doc("Reduce the model to a part file instead of solving it (ADRs 0469 and 0532)")
        .field("interface", Schema::string("").doc("The interface node set (required)"))
        .field(
            "modes",
            Schema::int(0)
                .range(0.0, 1.0e4)
                .doc("Fixed-interface modes kept (0: static condensation)"),
        )
        .field(
            "interface_enrichment",
            Schema::int(0)
                .range(0.0, 1.0e4)
                .doc("Krylov blocks of the interface enrichment of ADR 0532 (0: none)"),
        )
        .field(
            "cutoff",
            Schema::float(0.0).unit("rad/s").doc(
                "Keep every fixed-interface mode below this angular frequency, a plain number in rad/s (ADR 0474; not \
                 given: no cutoff; modes must then be 0)",
            ),
        )
        .field(
            "residual_vectors",
            Schema::boolean(false).doc("Add the static residual vector of the interior load of the case (ADR 0474)"),
        )
        .field(
            "load_patterns",
            Schema::list(Schema::string("")).doc(
                "Names of loads of the case, each asking for its own static residual vector, in this order (ADR 0511)",
            ),
        )
        .field(
            "file",
            Schema::string("part.cms").doc("The part file written, in the output directory"),
        )
        .optional();
    Schema::object()
        .doc("A solid case file (F-386, ADR 0438)")
        .field("title", Schema::string("").doc("Title"))
        .field("mesh", mesh)
        .field("node_sets", Schema::named(node_set).doc("Node sets selected by a box"))
        .field("materials", Schema::named(material).doc("Materials"))
        .field("constraints", Schema::named(constraint).doc("Displacement constraints"))
        .field("loads", Schema::named(load).doc("Loads"))
        .field("analysis", analysis)
        .field("solver", solver)
        .field("output", output)
        .field(
            "superelements",
            Schema::named(superelement).doc("Reduced parts attached to the model (ADR 0785)"),
        )
        .field("reduction", reduction)
}

/// A parsed solid case file: the checked settings, the text (for the lines of later diagnostics) and the directory relative
/// paths resolve against.
#[derive(Debug, Clone)]
pub struct SolidCase {
    /// The settings (defaults filled in).
    pub tree: SettingsTree,
    /// The file's text.
    pub text: String,
    /// The directory of the case file.
    pub base_dir: PathBuf,
    raw: Value,
}

/// The analysis of a built case.
#[derive(Debug, Clone, PartialEq)]
pub enum SolidAnalysis {
    /// A static solve with these options.
    Static(StaticOptions),
    /// A modal analysis with these options.
    Modal(ModalOptions),
}

/// A built case: the model and what to do with it.
#[derive(Debug, Clone)]
pub struct SolidSetup {
    /// The model.
    pub model: SolidModel,
    /// The analysis.
    pub analysis: SolidAnalysis,
    /// The constraints by name: (name, node set).
    pub constraint_sets: Vec<(String, String)>,
    /// Title of the case.
    pub title: String,
    /// `output.vtu` as given.
    pub vtu: String,
    /// `output.report` as given.
    pub report: String,
    /// The reduced parts to attach, in ascending order of their names (ADR 0785); empty without the key.
    pub superelements: Vec<SuperelementEntry>,
    /// The reduction asked instead of an analysis (ADR 0785); `None` without the table.
    pub reduction: Option<ReductionSetup>,
    text: String,
}

/// One table `[superelements.<name>]`: a reduced part and the node set of the host it attaches to (ADR 0785).
#[derive(Debug, Clone)]
pub struct SuperelementEntry {
    /// The name of the table.
    pub name: String,
    /// `file` as given.
    pub file: String,
    /// The part file read.
    pub path: PathBuf,
    /// The node set of the host.
    pub set: String,
    /// The part.
    pub part: ReducedPart,
}

/// The table `[reduction]` (ADR 0785).
#[derive(Debug, Clone)]
pub struct ReductionSetup {
    /// The interface node set.
    pub interface: String,
    /// The options of `rcfd_fem::cms::reduce`: the mode count, the blocks of interface enrichment, the cutoff, the load's
    /// residual vector and the load patterns (ADR 0797).
    pub options: CmsOptions,
    /// `reduction.file` as given.
    pub file: String,
}

/// The outcome of a run with superelements.
#[derive(Debug, Clone)]
pub enum SuperOutcome {
    /// The static solution of the combined model.
    Static(SuperStatic),
    /// The lowest modes of the combined model.
    Modal(SuperModes),
}

/// A run with superelements: the setup (its model is the host), the combined model, its outcome, the report and the files
/// written.
#[derive(Debug, Clone)]
pub struct SuperRun {
    /// The setup.
    pub setup: SolidSetup,
    /// The host with the parts attached, in the order of `setup.superelements`.
    pub model: SuperelementModel,
    /// The outcome.
    pub outcome: SuperOutcome,
    /// The JSON report.
    pub report: Value,
    /// The files written.
    pub written: Vec<PathBuf>,
}

/// A reduction: the setup, the part, the report and the files written.
#[derive(Debug, Clone)]
pub struct ReductionRun {
    /// The setup.
    pub setup: SolidSetup,
    /// The reduced part.
    pub part: ReducedPart,
    /// The JSON report.
    pub report: Value,
    /// The files written.
    pub written: Vec<PathBuf>,
}

/// What [`run_solid_case_any`] ran.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum CaseRun {
    /// A case without the tables of ADR 0785.
    Solid(Box<SolidRun>),
    /// A case with `[superelements.<name>]`.
    Superelements(Box<SuperRun>),
    /// A case with `[reduction]`.
    Reduction(Box<ReductionRun>),
}

/// The outcome of a run.
#[derive(Debug, Clone)]
pub enum SolidOutcome {
    /// A static solve.
    Static {
        /// The result.
        result: SolidResult,
        /// What the solver did.
        report: SolveReport,
    },
    /// A modal analysis.
    Modal(ModalResult),
}

/// A run: the setup, its outcome, the report and the files written.
#[derive(Debug, Clone)]
pub struct SolidRun {
    /// The setup.
    pub setup: SolidSetup,
    /// The outcome.
    pub outcome: SolidOutcome,
    /// The JSON report.
    pub report: Value,
    /// The files written.
    pub written: Vec<PathBuf>,
}

fn diag(text: &str, path: &str, message: impl Into<String>) -> Diagnostic {
    Diagnostic {
        path: path.to_string(),
        line: line_of(text, path),
        message: message.into(),
    }
}

/// The line of a dotted path: the line of the key without trailing list indices, else of its table header.
fn line_of(text: &str, path: &str) -> Option<usize> {
    let mut segs: Vec<&str> = path.split('.').filter(|s| !s.is_empty()).collect();
    while segs.len() > 1 && segs.last().is_some_and(|s| s.chars().all(|c| c.is_ascii_digit())) {
        segs.pop();
    }
    if segs.is_empty() {
        return None;
    }
    locate_line(text, &segs.join("."))
}

fn sorted_names(v: &Value) -> Vec<String> {
    let mut names: Vec<String> = v.as_object().map(|m| m.keys().cloned().collect()).unwrap_or_default();
    names.sort();
    names
}

fn f64_at(v: &Value, key: &str) -> f64 {
    v.get(key).and_then(Value::as_f64).unwrap_or(0.0)
}

fn str_at<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}

fn floats(v: &Value, key: &str) -> Vec<f64> {
    v.get(key)
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_f64).collect())
        .unwrap_or_default()
}

fn strings(v: &Value, key: &str) -> Vec<String> {
    v.get(key)
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect())
        .unwrap_or_default()
}

fn vec3_of(x: &[f64]) -> Vec3 {
    [x[0], x[1], x[2]]
}

impl SolidCase {
    /// Parses TOML text; relative paths resolve against `base_dir`. Every schema problem is returned with its key and line.
    pub fn parse_toml_in(text: &str, base_dir: &Path) -> Result<SolidCase, Vec<Diagnostic>> {
        let raw: Value = toml::from_str(text).map_err(|e| {
            let line = e
                .span()
                .map(|s| text[..s.start.min(text.len())].matches('\n').count() + 1);
            vec![Diagnostic {
                path: String::new(),
                line,
                message: e.message().to_string(),
            }]
        })?;
        let mut errs = Vec::new();
        collect_errors(&schema(), &raw, "", &mut errs);
        if !errs.is_empty() {
            for e in &mut errs {
                e.line = line_of(text, &e.path);
            }
            return Err(errs);
        }
        let tree = SettingsTree::from_json(schema(), &raw).map_err(|e| vec![diag(text, "", e.to_string())])?;
        let case = SolidCase {
            tree,
            text: text.to_string(),
            base_dir: base_dir.to_path_buf(),
            raw,
        };
        let errs = case.semantic_errors();
        if errs.is_empty() {
            Ok(case)
        } else {
            Err(errs)
        }
    }

    /// Loads a solid case file.
    pub fn load(path: impl AsRef<Path>) -> Result<SolidCase, Vec<Diagnostic>> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path).map_err(|e| {
            vec![Diagnostic {
                path: String::new(),
                line: None,
                message: format!("cannot read {}: {e}", path.display()),
            }]
        })?;
        let base = path.parent().map(Path::to_path_buf).unwrap_or_default();
        Self::parse_toml_in(&text, &base)
    }

    fn data(&self) -> Value {
        self.tree.to_json()
    }

    fn given(&self, path: &str) -> bool {
        let mut v = &self.raw;
        for s in path.split('.') {
            match v.get(s) {
                Some(x) => v = x,
                None => return false,
            }
        }
        true
    }

    /// Problems the schema cannot see: required keys, keys of another kind, list lengths, the solver of a modal analysis.
    fn semantic_errors(&self) -> Vec<Diagnostic> {
        let t = &self.text;
        let d = self.data();
        let mut out = Vec::new();
        let need = |out: &mut Vec<Diagnostic>, path: &str, what: &str| {
            if !self.given(path) {
                out.push(diag(t, path, format!("required: {what}")));
            }
        };
        let len3 = |out: &mut Vec<Diagnostic>, path: &str, v: &Value, key: &str, empty_ok: bool| {
            let n = v.get(key).and_then(Value::as_array).map_or(0, Vec::len);
            if !(n == 3 || (empty_ok && n == 0)) {
                out.push(diag(t, path, format!("expected 3 values, got {n}")));
            }
        };
        let mesh = &d["mesh"];
        if str_at(mesh, "source") == "generator" {
            need(&mut out, "mesh.lengths", "the edge lengths of the box");
            need(&mut out, "mesh.divisions", "the divisions of the box");
            len3(&mut out, "mesh.origin", mesh, "origin", true);
            len3(&mut out, "mesh.lengths", mesh, "lengths", false);
            len3(&mut out, "mesh.divisions", mesh, "divisions", false);
            for k in ["file", "quadratic"] {
                if self.given(&format!("mesh.{k}")) {
                    out.push(diag(t, &format!("mesh.{k}"), "applies to source = file"));
                }
            }
        } else {
            need(&mut out, "mesh.file", "the Gmsh .msh file");
            for k in ["generator", "element", "origin", "lengths", "divisions"] {
                if self.given(&format!("mesh.{k}")) {
                    out.push(diag(t, &format!("mesh.{k}"), "applies to source = generator"));
                }
            }
        }
        for name in sorted_names(&d["node_sets"]) {
            let v = &d["node_sets"][&name];
            for k in ["box_min", "box_max"] {
                let p = format!("node_sets.{name}.{k}");
                need(&mut out, &p, "a corner of the selection box");
                len3(&mut out, &p, v, k, false);
            }
        }
        if sorted_names(&d["materials"]).is_empty() {
            out.push(diag(t, "materials", "required: at least one [materials.<name>] table"));
        }
        for name in sorted_names(&d["materials"]) {
            need(&mut out, &format!("materials.{name}.youngs_modulus"), "Young's modulus");
            need(
                &mut out,
                &format!("materials.{name}.poisson_ratio"),
                "the Poisson ratio",
            );
        }
        for name in sorted_names(&d["constraints"]) {
            need(&mut out, &format!("constraints.{name}.set"), "the node set");
            need(&mut out, &format!("constraints.{name}.components"), "the components");
            let c = d["constraints"][&name]["components"].as_array().map_or(0, Vec::len);
            if self.given(&format!("constraints.{name}.components")) && c == 0 {
                out.push(diag(
                    t,
                    &format!("constraints.{name}.components"),
                    "at least one component",
                ));
            }
        }
        for name in sorted_names(&d["loads"]) {
            let v = &d["loads"][&name];
            let kind = str_at(v, "kind");
            let p = |k: &str| format!("loads.{name}.{k}");
            need(&mut out, &p("kind"), "the kind of load");
            let own = match kind {
                "traction" => "traction",
                "pressure" => "pressure",
                "nodal_force" => "force",
                _ => "acceleration",
            };
            need(&mut out, &p(own), &format!("the {own} of a {kind} load"));
            if own != "pressure" {
                len3(&mut out, &p(own), v, own, false);
            }
            for k in ["traction", "pressure", "force", "acceleration"] {
                if k != own && self.given(&p(k)) {
                    out.push(diag(t, &p(k), format!("does not apply to kind = {kind}")));
                }
            }
            if kind == "gravity" {
                if self.given(&p("set")) {
                    out.push(diag(t, &p("set"), "does not apply to kind = gravity"));
                }
            } else {
                need(&mut out, &p("set"), "the set of the load");
            }
        }
        let modal = str_at(&d["analysis"], "type") == "modal";
        if !modal {
            for k in ["modes", "tolerance", "residual_test"] {
                if self.given(&format!("analysis.{k}")) {
                    out.push(diag(t, &format!("analysis.{k}"), "applies to type = modal"));
                }
            }
        }
        let solver = &d["solver"];
        let method = str_at(solver, "method");
        if modal && method != "direct" {
            out.push(diag(
                t,
                "solver.method",
                "applies to type = static (the modal analysis has its own eigensolver)",
            ));
        }
        if method == "direct" {
            for k in [
                "preconditioner",
                "fill_level",
                "ordering",
                "tolerance",
                "max_iterations",
            ] {
                if self.given(&format!("solver.{k}")) {
                    out.push(diag(t, &format!("solver.{k}"), "applies to method = cg"));
                }
            }
        } else if str_at(solver, "preconditioner") != "ic_fill" {
            for k in ["fill_level", "ordering"] {
                if self.given(&format!("solver.{k}")) {
                    out.push(diag(t, &format!("solver.{k}"), "applies to preconditioner = ic_fill"));
                }
            }
        }
        // ADR 0785: the tables `superelements` and `reduction`.
        let parts = sorted_names(&d["superelements"]);
        for name in &parts {
            need(&mut out, &format!("superelements.{name}.file"), "the part file");
            need(
                &mut out,
                &format!("superelements.{name}.set"),
                "the node set of the model the part attaches to",
            );
        }
        let reduction = self.given("reduction");
        if !parts.is_empty() && !reduction && !modal && method != "direct" {
            out.push(diag(
                t,
                "solver.method",
                "with superelements the static solve is direct (the envelope factorisation)",
            ));
        }
        if reduction {
            need(&mut out, "reduction.interface", "the interface node set");
            for k in ["analysis", "solver"] {
                if self.given(k) {
                    out.push(diag(t, k, "does not apply to a reduction (nothing is solved)"));
                }
            }
            if let Some(first) = parts.first() {
                out.push(Diagnostic {
                    path: "superelements".to_string(),
                    line: line_of(t, &format!("superelements.{first}")),
                    message: "does not apply to a reduction (nothing is solved)".to_string(),
                });
            }
            if self.given("output.vtu") {
                out.push(diag(
                    t,
                    "output.vtu",
                    "does not apply to a reduction (no .vtu is written)",
                ));
            }
        }
        out
    }

    /// The analysis and its options (ADR 0438 D5).
    pub fn analysis(&self) -> SolidAnalysis {
        let d = self.data();
        let a = &d["analysis"];
        if str_at(a, "type") == "modal" {
            let test = match str_at(a, "residual_test") {
                "backward" => ResidualTest::Backward,
                _ => ResidualTest::Relative,
            };
            let modes = a["modes"].as_i64().unwrap_or(6) as usize;
            return SolidAnalysis::Modal(
                ModalOptions::new(modes)
                    .tolerance(f64_at(a, "tolerance"))
                    .residual_test(test),
            );
        }
        let s = &d["solver"];
        let solver = match str_at(s, "method") {
            "cg" => {
                let preconditioner = match str_at(s, "preconditioner") {
                    "block_jacobi" => CgPreconditioner::BlockJacobi,
                    "ic0" => CgPreconditioner::IncompleteCholesky(IcOptions::default()),
                    "amg" => CgPreconditioner::SmoothedAggregation(ElasticAmgSettings::default()),
                    "amg_quadratic" => CgPreconditioner::SmoothedAggregation(ElasticAmgSettings::quadratic()),
                    "amg_quadratic_smoothed" => {
                        CgPreconditioner::SmoothedAggregation(ElasticAmgSettings::quadratic_smoothed())
                    }
                    _ => CgPreconditioner::IncompleteCholeskyFill {
                        options: IcOptions::default(),
                        level: s["fill_level"].as_i64().unwrap_or(1) as usize,
                        ordering: match str_at(s, "ordering") {
                            "node_rcm" => IcOrdering::NodeReverseCuthillMcKee,
                            _ => IcOrdering::Natural,
                        },
                    },
                };
                StaticSolver::PreconditionedCg {
                    preconditioner,
                    tolerance: f64_at(s, "tolerance"),
                    max_iterations: s["max_iterations"].as_i64().unwrap_or(20000) as usize,
                }
            }
            _ => StaticSolver::Direct,
        };
        SolidAnalysis::Static(StaticOptions::with_solver(solver))
    }

    fn mesh(&self, d: &Value) -> Result<SolidMesh, Diagnostic> {
        let t = &self.text;
        let m = &d["mesh"];
        if str_at(m, "source") == "file" {
            let file = str_at(m, "file");
            let path = self.base_dir.join(file);
            let is_msh = path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("msh"));
            if !is_msh {
                return Err(diag(t, "mesh.file", format!("'{file}': only Gmsh .msh files are read")));
            }
            let fv = rcfd_io::gmsh::read_file(&path)
                .map_err(|e| diag(t, "mesh.file", format!("{}: {e}", path.display())))?;
            let mesh = rcfd_fem::convert::from_mesh(&fv).map_err(|e| diag(t, "mesh.file", e.to_string()))?;
            if m["quadratic"].as_bool().unwrap_or(false) {
                return upgrade_to_tet10(&mesh)
                    .map(|(m, _)| m)
                    .map_err(|e| diag(t, "mesh.quadratic", e.to_string()));
            }
            return Ok(mesh);
        }
        let origin = floats(m, "origin");
        let origin = if origin.is_empty() { [0.0; 3] } else { vec3_of(&origin) };
        let lengths = vec3_of(&floats(m, "lengths"));
        let div: Vec<usize> = m["divisions"]
            .as_array()
            .map(|a| a.iter().filter_map(Value::as_i64).map(|i| i as usize).collect())
            .unwrap_or_default();
        let div = [div[0], div[1], div[2]];
        let built = match str_at(m, "element") {
            "tet4" => tet_box(origin, lengths, div),
            "tet10" => tet_box(origin, lengths, div).and_then(|b| upgrade_to_tet10(&b).map(|(m, _)| m)),
            _ => hex_box(origin, lengths, div),
        };
        built.map_err(|e| diag(t, "mesh", e.to_string()))
    }

    /// Builds the model by the library calls (ADR 0438 D4): mesh, node sets, materials, constraints, loads, each named table
    /// in ascending name order. A missing set, an empty selection, a duplicate name or a refusal of `rcfd-fem` is a
    /// diagnostic with the key and the line.
    pub fn build(&self) -> Result<SolidSetup, Vec<Diagnostic>> {
        let t = &self.text;
        let d = self.data();
        let mut mesh = self.mesh(&d).map_err(|e| vec![e])?;
        let mut errs = Vec::new();
        for name in sorted_names(&d["node_sets"]) {
            let v = &d["node_sets"][&name];
            let (lo, hi) = (floats(v, "box_min"), floats(v, "box_max"));
            let nodes: Vec<usize> = mesh
                .nodes()
                .iter()
                .enumerate()
                .filter(|(_, p)| (0..3).all(|c| p[c] >= lo[c] && p[c] <= hi[c]))
                .map(|(i, _)| i)
                .collect();
            if nodes.is_empty() {
                errs.push(diag(
                    t,
                    &format!("node_sets.{name}.box_min"),
                    format!("the box of node set '{name}' selects no node"),
                ));
            } else if let Err(e) = mesh.add_node_set(&name, nodes) {
                errs.push(diag(t, &format!("node_sets.{name}"), e.to_string()));
            }
        }
        if !errs.is_empty() {
            return Err(errs);
        }
        let list = |names: Vec<&str>| names.join(", ");
        let mut model = SolidModel::new(mesh);
        for name in sorted_names(&d["materials"]) {
            let v = &d["materials"][&name];
            let set = str_at(v, "element_set");
            let path = format!("materials.{name}");
            if model.mesh().element_set(set).is_none() {
                let known = list(model.mesh().element_set_names().collect());
                errs.push(diag(
                    t,
                    &format!("{path}.element_set"),
                    format!("element set '{set}' does not exist (element sets: {known})"),
                ));
                continue;
            }
            let mat = Material::isotropic_elastic(
                f64_at(v, "youngs_modulus"),
                f64_at(v, "poisson_ratio"),
                f64_at(v, "density"),
                f64_at(v, "expansion"),
            );
            match mat.and_then(|m| model.add_material(set, m)) {
                Ok(()) => {}
                Err(e) => errs.push(diag(t, &path, e.to_string())),
            }
        }
        let mut constraint_sets = Vec::new();
        for name in sorted_names(&d["constraints"]) {
            let v = &d["constraints"][&name];
            let set = str_at(v, "set");
            let path = format!("constraints.{name}");
            if model.mesh().node_set(set).is_none() {
                let known = list(model.mesh().node_set_names().collect());
                errs.push(diag(
                    t,
                    &format!("{path}.set"),
                    format!("node set '{set}' does not exist (node sets: {known})"),
                ));
                continue;
            }
            constraint_sets.push((name.clone(), set.to_string()));
            for c in v["components"].as_array().into_iter().flatten() {
                let component = match c.as_str() {
                    Some("y") => 1,
                    Some("z") => 2,
                    _ => 0,
                };
                if let Err(e) = model.add_constraint(Constraint::displacement(set, component, f64_at(v, "value"))) {
                    errs.push(diag(t, &path, e.to_string()));
                }
            }
        }
        // ADR 0797 D2: the model without its loads and the loads by name, for the load patterns of a reduction.
        let patterns = strings(&d["reduction"], "load_patterns");
        let unloaded = (!patterns.is_empty()).then(|| model.clone());
        let mut named_loads: Vec<(String, Load)> = Vec::new();
        let before_loads = errs.len();
        for name in sorted_names(&d["loads"]) {
            let v = &d["loads"][&name];
            let set = str_at(v, "set");
            let path = format!("loads.{name}");
            let kind = str_at(v, "kind");
            let missing = match kind {
                "traction" | "pressure" => model.mesh().surface_set(set).is_none().then(|| {
                    let known = list(model.mesh().surface_set_names().collect());
                    format!("surface set '{set}' does not exist (surface sets: {known})")
                }),
                "nodal_force" => model.mesh().node_set(set).is_none().then(|| {
                    let known = list(model.mesh().node_set_names().collect());
                    format!("node set '{set}' does not exist (node sets: {known})")
                }),
                _ => None,
            };
            if let Some(m) = missing {
                errs.push(diag(t, &format!("{path}.set"), m));
                continue;
            }
            let load = match kind {
                "traction" => Load::traction(set, vec3_of(&floats(v, "traction"))),
                "pressure" => Load::pressure(set, f64_at(v, "pressure")),
                "nodal_force" => Load::nodal_force(set, vec3_of(&floats(v, "force"))),
                _ => Load::gravity(vec3_of(&floats(v, "acceleration"))),
            };
            if unloaded.is_some() {
                named_loads.push((name.clone(), load.clone()));
            }
            if let Err(e) = model.add_load(load) {
                errs.push(diag(t, &path, e.to_string()));
            }
        }
        let loads_built = errs.len() == before_loads;
        // ADR 0785 D1, D3: the part files and the node sets they attach to, in ascending order of the names.
        let mut superelements = Vec::new();
        let names = sorted_names(&d["superelements"]);
        if !names.is_empty() {
            let mesh = model.mesh();
            let nodes = mesh.nodes();
            let tol = match_tolerance(mesh);
            let near = |p: &Vec3, h: usize| (0..3).all(|c| (nodes[h][c] - p[c]).abs() <= tol);
            for name in names {
                let v = &d["superelements"][&name];
                let path = format!("superelements.{name}");
                let (file, set) = (str_at(v, "file"), str_at(v, "set"));
                let file_path = self.base_dir.join(file);
                let part = match ReducedPart::read(&file_path) {
                    Ok(p) => Some(p),
                    Err(e) => {
                        errs.push(diag(
                            t,
                            &format!("{path}.file"),
                            format!("{}: {e}", file_path.display()),
                        ));
                        None
                    }
                };
                let Some(set_nodes) = mesh.node_set(set) else {
                    let known = list(mesh.node_set_names().collect());
                    errs.push(diag(
                        t,
                        &format!("{path}.set"),
                        format!("node set '{set}' does not exist (node sets: {known})"),
                    ));
                    continue;
                };
                let Some(part) = part else { continue };
                let boundary = part.boundary();
                if let Some(b) = boundary
                    .iter()
                    .find(|b| !set_nodes.iter().any(|&h| near(&b.position, h)))
                {
                    errs.push(diag(
                        t,
                        &format!("{path}.set"),
                        format!(
                            "the part's boundary node {} at {:?} has no node of the set '{set}' within {tol:e}",
                            b.node, b.position
                        ),
                    ));
                    continue;
                }
                let loose = set_nodes
                    .iter()
                    .filter(|&&h| !boundary.iter().any(|b| near(&b.position, h)))
                    .count();
                if loose > 0 {
                    errs.push(diag(
                        t,
                        &format!("{path}.set"),
                        format!(
                            "{loose} of the {} nodes of the set '{set}' are at no boundary node of the part: the part \
                             does not hold them",
                            set_nodes.len()
                        ),
                    ));
                    continue;
                }
                superelements.push(SuperelementEntry {
                    name,
                    file: file.to_string(),
                    path: file_path,
                    set: set.to_string(),
                    part,
                });
            }
        }
        // ADR 0785 D2: the reduction asked instead of an analysis.
        let reduction = if self.given("reduction") {
            let r = &d["reduction"];
            let interface = str_at(r, "interface");
            let interface_known = model.mesh().node_set(interface).is_some();
            if !interface_known {
                let known = list(model.mesh().node_set_names().collect());
                errs.push(diag(
                    t,
                    "reduction.interface",
                    format!("node set '{interface}' does not exist (node sets: {known})"),
                ));
            }
            let mut options = CmsOptions::new(r["modes"].as_i64().unwrap_or(0) as usize)
                .interface_enrichment(r["interface_enrichment"].as_i64().unwrap_or(0) as usize);
            // ADR 0797: the cutoff, the load's residual vector and the load patterns, in the order of the list.
            if self.given("reduction.cutoff") {
                options = options.cutoff(f64_at(r, "cutoff"));
            }
            let residual = r["residual_vectors"].as_bool().unwrap_or(false);
            options = options.residual_vectors(residual);
            let no_vector = |what: &str| {
                format!(
                    "{what} no interior part (nothing outside the interface and the constraints): there is no vector to add"
                )
            };
            if residual && interface_known && loads_built {
                // A refusal of the load vector is left to the reduction, which reports it.
                if nodal_load_vector(&model, &[]).is_ok_and(|f| !has_interior_part(&model, interface, &f)) {
                    errs.push(diag(
                        t,
                        "reduction.residual_vectors",
                        no_vector("the loads of the case have"),
                    ));
                }
            }
            let key = "reduction.load_patterns";
            for name in &patterns {
                if d["loads"].get(name).is_none() {
                    let known = sorted_names(&d["loads"]).join(", ");
                    errs.push(diag(t, key, format!("load '{name}' does not exist (loads: {known})")));
                    continue;
                }
                let load = named_loads.iter().find(|(n, _)| n == name).map(|(_, l)| l.clone());
                let (Some(load), Some(mut single), true) = (load, unloaded.clone(), loads_built) else {
                    continue;
                };
                match single.add_load(load).and_then(|()| nodal_load_vector(&single, &[])) {
                    Ok(p) if interface_known && !has_interior_part(&model, interface, &p) => {
                        errs.push(diag(t, key, no_vector(&format!("load '{name}' has"))));
                    }
                    Ok(p) => options = options.load_pattern(p),
                    Err(e) => errs.push(diag(t, key, e.to_string())),
                }
            }
            Some(ReductionSetup {
                interface: interface.to_string(),
                options,
                file: str_at(r, "file").to_string(),
            })
        } else {
            None
        };
        if !errs.is_empty() {
            return Err(errs);
        }
        let o = &d["output"];
        Ok(SolidSetup {
            model,
            analysis: self.analysis(),
            constraint_sets,
            title: str_at(&d, "title").to_string(),
            vtu: str_at(o, "vtu").to_string(),
            report: str_at(o, "report").to_string(),
            superelements,
            reduction,
            text: self.text.clone(),
        })
    }
}

/// The tolerance of the attachment of a part: `MATCH_TOLERANCE` of the largest extent of the host mesh (1 for a mesh
/// without extent), as `SuperelementModel::new` takes it (ADR 0536).
fn match_tolerance(mesh: &SolidMesh) -> f64 {
    let nodes = mesh.nodes();
    let mut extent = 0.0f64;
    for axis in 0..3 {
        let lo = nodes.iter().fold(f64::INFINITY, |a, p| a.min(p[axis]));
        let hi = nodes.iter().fold(f64::NEG_INFINITY, |a, p| a.max(p[axis]));
        if hi >= lo {
            extent = extent.max(hi - lo);
        }
    }
    MATCH_TOLERANCE * if extent > 0.0 { extent } else { 1.0 }
}

/// Whether a load vector over the mesh equations has a non-zero entry at an unknown that is neither of the interface nor
/// constrained (ADR 0797 D3: the interior load of a reduction).
fn has_interior_part(model: &SolidModel, interface: &str, load: &[f64]) -> bool {
    let mesh = model.mesh();
    let dofs = model.dof_map();
    let in_set = |set: &str, node: usize| mesh.node_set(set).is_some_and(|s| s.contains(&node));
    let held = |node: usize, c: usize| {
        in_set(interface, node)
            || model.constraints().iter().any(|k| {
                matches!(k, Constraint::Displacement { set, component, .. } if *component == c && in_set(set, node))
            })
    };
    load.iter()
        .enumerate()
        .any(|(e, v)| *v != 0.0 && dofs.node_component(e).is_some_and(|(node, c)| !held(node, c)))
}

/// The problem of a refusal of `reduce`: a refused cutoff at its key (ADR 0797 D3), any other at the table.
fn reduction_diag(text: &str, e: &FemError) -> Diagnostic {
    let path = match e {
        FemError::InvalidValue { key: "cutoff", .. } => "reduction.cutoff",
        _ => "reduction",
    };
    diag(text, path, e.to_string())
}

/// A problem at the key `superelements`, on the line of its first table.
fn superelements_diag(setup: &SolidSetup, message: String) -> Diagnostic {
    let first = setup.superelements.first().map_or("", |e| e.name.as_str());
    Diagnostic {
        path: "superelements".to_string(),
        line: line_of(&setup.text, &format!("superelements.{first}")),
        message,
    }
}

/// The refusal of a case with a table of ADR 0785 by an entry whose result cannot hold its outcome.
fn other_entry(setup: &SolidSetup) -> Option<Diagnostic> {
    let message =
        "this case is run by run_solid_case_any (the result of this entry holds a static or a modal outcome of \
                   the model alone)";
    if setup.reduction.is_some() {
        Some(diag(&setup.text, "reduction", message))
    } else if !setup.superelements.is_empty() {
        Some(superelements_diag(setup, message.to_string()))
    } else {
        None
    }
}

/// Runs a built case with superelements (no file written): the parts attached in their order, then the static solve or
/// the modal analysis of the combined model (ADR 0785 D4).
pub fn run_superelements(setup: SolidSetup) -> Result<SuperRun, Vec<Diagnostic>> {
    let mut model =
        SuperelementModel::new(setup.model.clone()).map_err(|e| vec![superelements_diag(&setup, e.to_string())])?;
    for entry in &setup.superelements {
        model.add_part(entry.part.clone()).map_err(|e| {
            vec![diag(
                &setup.text,
                &format!("superelements.{}", entry.name),
                e.to_string(),
            )]
        })?;
    }
    let outcome = match &setup.analysis {
        SolidAnalysis::Static(_) => SuperOutcome::Static(
            model
                .solve_static(None)
                .map_err(|e| vec![diag(&setup.text, "solver", e.to_string())])?,
        ),
        SolidAnalysis::Modal(o) => SuperOutcome::Modal(
            model
                .modal_analysis(o)
                .map_err(|e| vec![diag(&setup.text, "analysis", e.to_string())])?,
        ),
    };
    let report = super_report_json(&setup, &model, &outcome);
    Ok(SuperRun {
        setup,
        model,
        outcome,
        report,
        written: Vec::new(),
    })
}

/// The JSON report of a run with superelements (ADR 0785 D6).
fn super_report_json(setup: &SolidSetup, model: &SuperelementModel, outcome: &SuperOutcome) -> Value {
    let mesh = setup.model.mesh();
    let mut r = Map::new();
    r.insert("format".into(), json!(REPORT_FORMAT));
    r.insert("title".into(), json!(setup.title));
    r.insert(
        "mesh".into(),
        json!({"nodes": mesh.nodes().len(), "elements": mesh.n_elements()}),
    );
    let parts: Vec<Value> = setup
        .superelements
        .iter()
        .enumerate()
        .map(|(p, e)| {
            json!({"name": e.name, "file": e.file, "set": e.set,
                   "boundary_unknowns": e.part.n_boundary(),
                   "generalised_coordinates": e.part.n_reduced() - e.part.n_boundary(),
                   "offset": model.part_offset(p)})
        })
        .collect();
    match outcome {
        SuperOutcome::Static(s) => {
            r.insert("analysis".into(), json!("static"));
            let dofs = setup.model.dof_map();
            let n = mesh.nodes().len();
            let at = |node: usize, c: usize| dofs.equation(node, c).map_or(0.0, |e| s.host_displacement[e]);
            let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
            for node in 0..n {
                for c in 0..3 {
                    lo[c] = lo[c].min(at(node, c));
                    hi[c] = hi[c].max(at(node, c));
                }
            }
            let (mag, mag_node) =
                max_by_index((0..n).map(|node| (0..3).map(|c| at(node, c).powi(2)).sum::<f64>().sqrt()));
            r.insert(
                "displacement".into(),
                json!({"max_magnitude": mag, "max_magnitude_node": mag_node, "min": lo, "max": hi}),
            );
            r.insert("solver".into(), json!({"name": "skyline-ldlt"}));
        }
        SuperOutcome::Modal(m) => {
            r.insert("analysis".into(), json!("modal"));
            let frequencies: Vec<f64> = m
                .eigenvalues
                .iter()
                .map(|l| l.max(0.0).sqrt() / (2.0 * std::f64::consts::PI))
                .collect();
            r.insert("frequencies".into(), json!(frequencies));
            r.insert("eigenvalues".into(), json!(m.eigenvalues));
            r.insert("solver".into(), json!({"name": "subspace"}));
        }
    }
    r.insert("superelements".into(), Value::Array(parts));
    r.insert("unknowns".into(), json!(model.n_unknowns()));
    Value::Object(r)
}

/// The `.vtu` text of a run with superelements: the host mesh with the point array `displacement` (static) or one array
/// `mode_<k>` per mode, the host's part of it (ADR 0785 D6).
pub fn super_vtu_string(run: &SuperRun) -> rcfd_core::Result<String> {
    let model = &run.setup.model;
    let dofs = model.dof_map();
    let n = model.mesh().nodes().len();
    let per_node = |x: &[f64]| -> Vec<f64> {
        (0..n)
            .flat_map(|node| (0..3).map(move |c| (node, c)))
            .map(|(node, c)| dofs.equation(node, c).map_or(0.0, |e| x[e]))
            .collect()
    };
    let (names, values): (Vec<String>, Vec<Vec<f64>>) = match &run.outcome {
        SuperOutcome::Static(s) => (vec!["displacement".to_string()], vec![per_node(&s.host_displacement)]),
        SuperOutcome::Modal(m) => (
            (1..=m.vectors.len()).map(|k| format!("mode_{k}")).collect(),
            m.vectors
                .iter()
                .map(|z| per_node(&run.model.host_displacement(z)))
                .collect(),
        ),
    };
    point_arrays_vtu(model, &names, &values)
}

/// Runs a built case (no file written). A case with a table of ADR 0785 is refused: [`run_solid_case_any`] runs it.
pub fn run(setup: SolidSetup) -> Result<SolidRun, Vec<Diagnostic>> {
    if let Some(d) = other_entry(&setup) {
        return Err(vec![d]);
    }
    let outcome = match &setup.analysis {
        SolidAnalysis::Static(o) => {
            let (result, report) = solve_static_report(&setup.model, &[], &[], o)
                .map_err(|e| vec![diag(&setup.text, "solver", e.to_string())])?;
            SolidOutcome::Static { result, report }
        }
        SolidAnalysis::Modal(o) => SolidOutcome::Modal(
            modal_analysis(&setup.model, o).map_err(|e| vec![diag(&setup.text, "analysis", e.to_string())])?,
        ),
    };
    let report = report_json(&setup, &outcome).map_err(|e| vec![diag(&setup.text, "output.report", e)])?;
    Ok(SolidRun {
        setup,
        outcome,
        report,
        written: Vec::new(),
    })
}

fn max_by_index(values: impl Iterator<Item = f64>) -> (f64, Option<usize>) {
    let mut best = (f64::NEG_INFINITY, None);
    for (i, v) in values.enumerate() {
        if v.is_finite() && (best.1.is_none() || v > best.0) {
            best = (v, Some(i));
        }
    }
    best
}

/// The JSON report of a run (ADR 0438 D6).
fn report_json(setup: &SolidSetup, outcome: &SolidOutcome) -> Result<Value, String> {
    let model = &setup.model;
    let mesh = model.mesh();
    let mut r = Map::new();
    r.insert("format".into(), json!(REPORT_FORMAT));
    r.insert("title".into(), json!(setup.title));
    r.insert(
        "mesh".into(),
        json!({"nodes": mesh.nodes().len(), "elements": mesh.n_elements()}),
    );
    match outcome {
        SolidOutcome::Static { result, report } => {
            r.insert("analysis".into(), json!("static"));
            let n = mesh.nodes().len();
            let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
            for node in 0..n {
                let u = result.displacement(node);
                for c in 0..3.min(u.len()) {
                    lo[c] = lo[c].min(u[c]);
                    hi[c] = hi[c].max(u[c]);
                }
            }
            let (mag, mag_node) = max_by_index((0..n).map(|node| {
                let u = result.displacement(node);
                u.iter().map(|x| x * x).sum::<f64>().sqrt()
            }));
            r.insert(
                "displacement".into(),
                json!({"max_magnitude": mag, "max_magnitude_node": mag_node, "min": lo, "max": hi}),
            );
            let f = solid_fields(model, result).map_err(|e| e.to_string())?;
            let (vm, vm_node) = max_by_index(f.von_mises.iter().copied());
            let (evm, evm_el) = max_by_index(f.element_stress.chunks(6).map(|s| {
                let s: [f64; 6] = [s[0], s[1], s[2], s[3], s[4], s[5]];
                von_mises(&s)
            }));
            r.insert(
                "stress".into(),
                json!({"von_mises_max": vm, "von_mises_max_node": vm_node,
                       "element_von_mises_max": evm, "element_von_mises_max_element": evm_el}),
            );
            let mut reactions = Map::new();
            if let Some(re) = result.reactions() {
                let dofs = result.dofs();
                let sum = |nodes: &mut dyn Iterator<Item = usize>| {
                    let mut s = [0.0; 3];
                    for node in nodes {
                        for (c, sc) in s.iter_mut().enumerate() {
                            if let Some(eq) = dofs.equation(node, c) {
                                *sc += re[eq];
                            }
                        }
                    }
                    s
                };
                for (name, set) in &setup.constraint_sets {
                    let nodes = mesh.node_set(set).unwrap_or(&[]);
                    reactions.insert(name.clone(), json!(sum(&mut nodes.iter().copied())));
                }
                reactions.insert("total".into(), json!(sum(&mut (0..n))));
            }
            r.insert("reactions".into(), Value::Object(reactions));
            r.insert("unknowns".into(), json!(result.stats().map_or(0, |s| s.unknowns())));
            r.insert(
                "solver".into(),
                json!({"name": report.solver, "iterations": report.iterations, "restarts": report.restarts,
                       "relative_residual": report.relative_residual, "shift": report.shift,
                       "breakdowns": report.breakdowns.len()}),
            );
        }
        SolidOutcome::Modal(m) => {
            r.insert("analysis".into(), json!("modal"));
            r.insert("unknowns".into(), json!(m.unknowns));
            r.insert("frequencies".into(), json!(m.frequencies));
            r.insert("eigenvalues".into(), json!(m.eigenvalues));
            r.insert(
                "solver".into(),
                json!({"name": "subspace", "iterations": m.iterations, "shift": m.shift,
                       "residuals": m.residuals, "rigid_free": m.rigid_free}),
            );
        }
    }
    Ok(Value::Object(r))
}

/// The `.vtu` text of a modal result: the mesh with one point array `mode_<k>` (3 per node) per mode (ADR 0438 D6).
pub fn modal_vtu_string(model: &SolidModel, modal: &ModalResult) -> rcfd_core::Result<String> {
    let dofs = modal.dofs();
    let n = model.mesh().nodes().len();
    let names: Vec<String> = (1..=modal.modes.len()).map(|k| format!("mode_{k}")).collect();
    let values: Vec<Vec<f64>> = modal
        .modes
        .iter()
        .map(|phi| {
            (0..n)
                .flat_map(|node| (0..3).map(move |c| (node, c)))
                .map(|(node, c)| dofs.equation(node, c).map_or(0.0, |e| phi[e]))
                .collect()
        })
        .collect();
    point_arrays_vtu(model, &names, &values)
}

/// The `.vtu` text of the mesh of `model` with the named point arrays, three values per node.
fn point_arrays_vtu(model: &SolidModel, names: &[String], values: &[Vec<f64>]) -> rcfd_core::Result<String> {
    use rcfd_io::vtk::{fe_vtu_string, FeArray, FeCellBlock};
    let mesh = model.mesh();
    let mut conns: Vec<(u8, Vec<usize>)> = Vec::with_capacity(mesh.blocks().len());
    for block in mesh.blocks() {
        let (ty, perm) = vtk_from_fem(block.kind())?;
        let npe = block.kind().nodes_per_element();
        let mut c = Vec::with_capacity(block.connectivity().len());
        for element in block.connectivity().chunks(npe) {
            c.extend(perm.iter().map(|&a| element[a]));
        }
        conns.push((ty, c));
    }
    let blocks: Vec<FeCellBlock> = conns
        .iter()
        .map(|(ty, c)| FeCellBlock {
            cell_type: *ty,
            connectivity: c,
        })
        .collect();
    let point: Vec<FeArray> = names
        .iter()
        .zip(values)
        .map(|(name, v)| FeArray {
            name,
            components: 3,
            values: v,
        })
        .collect();
    fe_vtu_string(mesh.nodes(), &blocks, &point, &[])
}

fn out_path(dir: &Path, name: &str) -> PathBuf {
    dir.join(name)
}

/// Reads, builds and runs a solid case file and writes its `.vtu` and JSON report (ADR 0438 D7), into `out_dir` when given,
/// else into the case file's directory. All problems are diagnostics with the key and the line.
pub fn run_solid_case_file(path: impl AsRef<Path>, out_dir: Option<&Path>) -> Result<SolidRun, Vec<Diagnostic>> {
    let case = SolidCase::load(path.as_ref())?;
    let setup = case.build()?;
    let mut run = run(setup)?;
    let dir = out_dir.map(Path::to_path_buf).unwrap_or_else(|| case.base_dir.clone());
    let t = case.text.as_str();
    let mut outputs = Map::new();
    if !run.setup.vtu.is_empty() {
        let p = out_path(&dir, &run.setup.vtu);
        let text = match &run.outcome {
            SolidOutcome::Static { result, .. } => solid_vtu_string(&run.setup.model, result),
            SolidOutcome::Modal(m) => modal_vtu_string(&run.setup.model, m),
        }
        .map_err(|e| vec![diag(t, "output.vtu", e.to_string())])?;
        std::fs::write(&p, text).map_err(|e| vec![diag(t, "output.vtu", format!("{}: {e}", p.display()))])?;
        outputs.insert("vtu".into(), json!(p.to_string_lossy()));
        run.written.push(p);
    }
    if !run.setup.report.is_empty() {
        let p = out_path(&dir, &run.setup.report);
        outputs.insert("report".into(), json!(p.to_string_lossy()));
        if let Value::Object(m) = &mut run.report {
            m.insert("outputs".into(), Value::Object(outputs));
        }
        let text = serde_json::to_string_pretty(&run.report).expect("a JSON value serialises");
        std::fs::write(&p, text).map_err(|e| vec![diag(t, "output.report", format!("{}: {e}", p.display()))])?;
        run.written.push(p);
    } else if let Value::Object(m) = &mut run.report {
        m.insert("outputs".into(), Value::Object(outputs));
    }
    Ok(run)
}

/// The report of a reduction (ADR 0785 D6).
fn reduction_report_json(setup: &SolidSetup, reduction: &ReductionSetup, part: &ReducedPart) -> Value {
    let mesh = setup.model.mesh();
    json!({
        "format": REPORT_FORMAT,
        "title": setup.title,
        "mesh": {"nodes": mesh.nodes().len(), "elements": mesh.n_elements()},
        "analysis": "reduction",
        "reduction": {
            "interface": reduction.interface,
            "modes": part.modes(),
            "interface_enrichment": reduction.options.interface_blocks,
            "boundary_unknowns": part.n_boundary(),
            "interior_unknowns": part.n_interior(),
            "residual_vectors": part.residual_vectors(),
            "reduced_unknowns": part.n_reduced(),
        },
    })
}

/// Writes the report of a run of ADR 0785 with its `outputs`, when the case asks for a report.
fn write_report_any(
    report: &mut Value,
    mut outputs: Map<String, Value>,
    name: &str,
    dir: &Path,
    text: &str,
    written: &mut Vec<PathBuf>,
) -> Result<(), Vec<Diagnostic>> {
    let path = (!name.is_empty()).then(|| out_path(dir, name));
    if let Some(p) = &path {
        outputs.insert("report".into(), json!(p.to_string_lossy()));
    }
    if let Value::Object(m) = report {
        m.insert("outputs".into(), Value::Object(outputs));
    }
    if let Some(p) = path {
        let json = serde_json::to_string_pretty(report).expect("a JSON value serialises");
        std::fs::write(&p, json).map_err(|e| vec![diag(text, "output.report", format!("{}: {e}", p.display()))])?;
        written.push(p);
    }
    Ok(())
}

/// Reads, builds and runs any solid case file (ADR 0785 D5) and writes its files into `out_dir` when given, else into the
/// case file's directory: a case with `[reduction]` is reduced to its part file (and the report); a case with
/// `[superelements.<name>]` is solved with the parts attached (the `.vtu` of the host and the report); any other case is
/// run as [`run_solid_case_file`] runs it.
pub fn run_solid_case_any(path: impl AsRef<Path>, out_dir: Option<&Path>) -> Result<CaseRun, Vec<Diagnostic>> {
    let case = SolidCase::load(path.as_ref())?;
    let setup = case.build()?;
    let dir = out_dir.map(Path::to_path_buf).unwrap_or_else(|| case.base_dir.clone());
    let t = case.text.as_str();
    if let Some(reduction) = setup.reduction.clone() {
        let part =
            reduce(&setup.model, &reduction.interface, &reduction.options).map_err(|e| vec![reduction_diag(t, &e)])?;
        let mut report = reduction_report_json(&setup, &reduction, &part);
        let mut written = Vec::new();
        let mut outputs = Map::new();
        let p = out_path(&dir, &reduction.file);
        part.write(&p)
            .map_err(|e| vec![diag(t, "reduction.file", e.to_string())])?;
        outputs.insert("part".into(), json!(p.to_string_lossy()));
        written.push(p);
        write_report_any(&mut report, outputs, &setup.report, &dir, t, &mut written)?;
        return Ok(CaseRun::Reduction(Box::new(ReductionRun {
            setup,
            part,
            report,
            written,
        })));
    }
    if !setup.superelements.is_empty() {
        let mut run = run_superelements(setup)?;
        let mut outputs = Map::new();
        if !run.setup.vtu.is_empty() {
            let p = out_path(&dir, &run.setup.vtu);
            let text = super_vtu_string(&run).map_err(|e| vec![diag(t, "output.vtu", e.to_string())])?;
            std::fs::write(&p, text).map_err(|e| vec![diag(t, "output.vtu", format!("{}: {e}", p.display()))])?;
            outputs.insert("vtu".into(), json!(p.to_string_lossy()));
            run.written.push(p);
        }
        let name = run.setup.report.clone();
        write_report_any(&mut run.report, outputs, &name, &dir, t, &mut run.written)?;
        return Ok(CaseRun::Superelements(Box::new(run)));
    }
    run_solid_case_file(path, out_dir).map(|r| CaseRun::Solid(Box::new(r)))
}
