//! Computer-fan parameter schema, pre-meshing constraint checker and manufacturing perturbations (wave 46A, ADR 0118).
//!
//! [`ComputerFan`] is one serialisable description of an axial computer fan (40 to 140 mm): rotor, blade control points,
//! motor hub, frame and material, plus an optional [`Deviation`] that carries the manufacturing errors of one physical
//! specimen. [`PARAMS`] gives every scalar its unit, bounds and default, and says whether the existing sector generators
//! (ADR 0065 `fan_mesh`, ADR 0068 `fan_cgrid`) realise it. [`ComputerFan::axial_fan`] maps blade `k` onto the
//! [`AxialFan`] those generators take. [`ComputerFan::check`] rejects a variant before any meshing, with located,
//! machine-readable [`Violation`]s. [`ComputerFan::ensemble`] draws perturbed specimens from named distributions with the
//! seeded [`Rng`], and [`ComputerFan::static_unbalance`] gives the resulting static unbalance from blade mass and centroid.
//!
//! Conventions: lengths in metres, angles in degrees in this schema (radians in [`AxialFan`]), rotation about `+z`, blade
//! `k` at azimuth `2 pi k / N`, the frame inlet face at `z = 0`. Skew is the circumferential position of the leading edge
//! (`lean = r skew` in [`Station`]; positive toward the direction of rotation); sweep is its axial shift.

use crate::fan::{AxialFan, FanError, SectionFamily, Side, Station};
use rcfd_core::rng::Rng;
use serde::{Deserialize, Serialize};
use std::f64::consts::{PI, TAU};

/// One spanwise control point of the blade (linear variation between neighbouring points).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ControlPoint {
    /// Span fraction `(r - r_hub) / (r_tip - r_hub)`; the first point is 0, the last 1.
    pub span: f64,
    /// Chord [m].
    pub chord: f64,
    /// Stagger from the axis [deg] (positive turns the trailing edge toward `-theta`).
    pub stagger_deg: f64,
    /// Maximum camber (chord fraction).
    pub camber: f64,
    /// Chordwise position of the maximum camber (chord fraction; NACA 4-digit only).
    pub camber_pos: f64,
    /// Maximum thickness (chord fraction).
    pub thickness: f64,
    /// Axial shift of the leading edge [m].
    pub sweep: f64,
    /// Circumferential position of the leading edge [deg], positive toward the direction of rotation.
    pub skew_deg: f64,
}

/// Motor hub (cup) of radius `r_hub`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MotorHub {
    /// Axial position of the hub's upstream face [m].
    pub z_start: f64,
    /// Axial length of the hub [m].
    pub length: f64,
}

/// Square frame with a circular bore of radius `r_tip + tip_gap`, struts at the outlet face.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frame {
    /// Outer side of the square outline [m].
    pub side: f64,
    /// Axial depth [m] (inlet face `z = 0`, outlet face `z = depth`).
    pub depth: f64,
    /// Radius of the rounded inlet bell [m] (0 = sharp).
    pub inlet_bell_radius: f64,
    /// Number of motor struts.
    pub n_struts: usize,
    /// Circumferential width of a strut [m].
    pub strut_width: f64,
    /// Axial depth of a strut [m], ending at the outlet face.
    pub strut_depth: f64,
}

/// Manufacturing error of one blade.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BladeDeviation {
    /// Stagger error of this blade [deg] (added at every control point).
    pub stagger_deg: f64,
    /// Relative chord error.
    pub chord_rel: f64,
    /// Relative error of the absolute thickness.
    pub thickness_rel: f64,
    /// Relative mass error (density or fill), spread uniformly over the blade.
    pub mass_rel: f64,
    /// Equivalent sand-grain roughness of this blade [m].
    pub roughness_ks: f64,
}

/// Manufacturing errors of one specimen.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Deviation {
    /// Stagger error common to all blades [deg].
    pub stagger_common_deg: f64,
    /// Error of the nominal (concentric) tip gap [m].
    pub tip_gap_delta: f64,
    /// Offset of the rotation axis from the bore centre [m].
    pub eccentricity: f64,
    /// Direction of that offset [deg].
    pub eccentricity_angle_deg: f64,
    /// Per-blade errors (empty, or one per blade).
    pub blades: Vec<BladeDeviation>,
}

/// Parametric axial computer fan (ADR 0118).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComputerFan {
    /// Blade count.
    pub n_blades: usize,
    /// Hub (blade root) radius [m].
    pub r_hub: f64,
    /// Blade tip radius [m].
    pub r_tip: f64,
    /// Nominal radial tip gap [m]; the frame bore radius is `r_tip + tip_gap`.
    pub tip_gap: f64,
    /// Axial position of the root leading edge from the frame inlet face [m].
    pub z_le_root: f64,
    /// Section family.
    pub family: SectionFamily,
    /// Spanwise control points.
    pub stations: Vec<ControlPoint>,
    /// Motor hub.
    pub hub: MotorHub,
    /// Frame.
    pub frame: Frame,
    /// Rotor material density [kg/m^3].
    pub density: f64,
    /// Nominal equivalent sand-grain roughness of the blades [m].
    pub roughness_ks: f64,
    /// Manufacturing errors of this specimen (`None` = nominal).
    #[serde(default)]
    pub deviation: Option<Deviation>,
}

/// Unit, bounds and default of one scalar parameter (serialises, so the schema itself can be exported).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct ParamSpec {
    /// Path (`stations[]` and `deviation.blades[]` stand for every entry).
    pub path: &'static str,
    /// SI unit (`-` = dimensionless, `deg` = degrees, `count`).
    pub unit: &'static str,
    /// Inclusive lower bound.
    pub min: f64,
    /// Inclusive upper bound.
    pub max: f64,
    /// Default of [`ComputerFan::default`] (`None` for list entries, which differ per entry).
    pub default: Option<f64>,
    /// Whether the existing sector generators (ADR 0065, 0068) realise the parameter.
    pub meshed: bool,
}

const fn p(
    path: &'static str,
    unit: &'static str,
    min: f64,
    max: f64,
    default: Option<f64>,
    meshed: bool,
) -> ParamSpec {
    ParamSpec {
        path,
        unit,
        min,
        max,
        default,
        meshed,
    }
}

/// The schema: every scalar of [`ComputerFan`] (ADR 0118 table; the bounds are the 40-140 mm computer-fan envelope).
pub const PARAMS: &[ParamSpec] = &[
    p("n_blades", "count", 3.0, 15.0, Some(7.0), true),
    p("r_hub", "m", 0.004, 0.05, Some(0.0215), true),
    p("r_tip", "m", 0.018, 0.07, Some(0.058), true),
    p("tip_gap", "m", 1e-4, 5e-3, Some(0.001), true),
    p("z_le_root", "m", 0.0, 0.05, Some(0.002), true),
    p("density", "kg/m^3", 500.0, 3000.0, Some(1300.0), false),
    p("roughness_ks", "m", 0.0, 5e-4, Some(0.0), false),
    p("hub.z_start", "m", 0.0, 0.05, Some(0.001), false),
    p("hub.length", "m", 0.002, 0.06, Some(0.024), false),
    p("frame.side", "m", 0.04, 0.15, Some(0.12), false),
    p("frame.depth", "m", 0.005, 0.06, Some(0.025), false),
    p("frame.inlet_bell_radius", "m", 0.0, 0.02, Some(0.002), false),
    p("frame.n_struts", "count", 0.0, 12.0, Some(4.0), false),
    p("frame.strut_width", "m", 3e-4, 0.015, Some(0.004), false),
    p("frame.strut_depth", "m", 0.0, 0.015, Some(0.002), false),
    p("stations[].span", "-", 0.0, 1.0, None, true),
    p("stations[].chord", "m", 0.002, 0.1, None, true),
    p("stations[].stagger_deg", "deg", -79.0, 79.0, None, true),
    p("stations[].camber", "-", 0.0, 0.2, None, true),
    p("stations[].camber_pos", "-", 0.06, 0.94, None, true),
    p("stations[].thickness", "-", 0.005, 0.4, None, true),
    p("stations[].sweep", "m", -0.03, 0.03, None, true),
    p("stations[].skew_deg", "deg", -60.0, 60.0, None, true),
    p("deviation.stagger_common_deg", "deg", -10.0, 10.0, Some(0.0), true),
    p("deviation.tip_gap_delta", "m", -0.005, 0.005, Some(0.0), true),
    p("deviation.eccentricity", "m", 0.0, 0.005, Some(0.0), false),
    p(
        "deviation.eccentricity_angle_deg",
        "deg",
        -360.0,
        360.0,
        Some(0.0),
        false,
    ),
    p("deviation.blades[].stagger_deg", "deg", -10.0, 10.0, None, false),
    p("deviation.blades[].chord_rel", "-", -0.3, 0.3, None, false),
    p("deviation.blades[].thickness_rel", "-", -0.6, 0.6, None, false),
    p("deviation.blades[].mass_rel", "-", -0.5, 0.5, None, false),
    p("deviation.blades[].roughness_ks", "m", 0.0, 5e-4, None, false),
];

/// The spec of `key`.
pub fn param_spec(key: &str) -> Option<&'static ParamSpec> {
    PARAMS.iter().find(|s| s.path == key)
}

impl Default for ComputerFan {
    /// A 120 mm, 7-blade fan (an illustration of the schema, not a measured fan).
    fn default() -> Self {
        let cp = |span, chord, stagger_deg, camber, thickness, skew_deg| ControlPoint {
            span,
            chord,
            stagger_deg,
            camber,
            camber_pos: 0.4,
            thickness,
            sweep: 0.0,
            skew_deg,
        };
        Self {
            n_blades: 7,
            r_hub: 0.0215,
            r_tip: 0.058,
            tip_gap: 0.001,
            z_le_root: 0.002,
            family: SectionFamily::Naca4,
            stations: vec![
                cp(0.0, 0.022, 40.0, 0.06, 0.055, 0.0),
                cp(0.5, 0.029, 52.0, 0.055, 0.042, 4.0),
                cp(1.0, 0.036, 62.0, 0.05, 0.035, 8.0),
            ],
            hub: MotorHub {
                z_start: 0.001,
                length: 0.024,
            },
            frame: Frame {
                side: 0.12,
                depth: 0.025,
                inlet_bell_radius: 0.002,
                n_struts: 4,
                strut_width: 0.004,
                strut_depth: 0.002,
            },
            density: 1300.0,
            roughness_ks: 0.0,
            deviation: None,
        }
    }
}

/// Machine-readable reason of a [`Violation`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Code {
    /// A scalar outside its [`PARAMS`] bounds (or not finite).
    OutOfBounds,
    /// Structurally invalid (station order, list lengths, the [`AxialFan::validate`] rules).
    Invalid,
    /// Absolute blade thickness below the minimum wall.
    MinThickness,
    /// Tip gap outside its range (with eccentricity: the smallest gap).
    TipGap,
    /// The smallest tip gap is not positive: the blade touches the bore.
    TipRub,
    /// Hub-to-tip ratio outside its range.
    HubTipRatio,
    /// Solidity `c N / (2 pi r)` outside its range.
    Solidity,
    /// Neighbouring blade sections intersect.
    Collision,
    /// Neighbouring blade sections closer than the minimum passage gap.
    PassageGap,
    /// Neighbouring blades overlap in the axial view (an axial two-plate mould cannot release the steel between them).
    AxialOverlap,
    /// An axial line crosses one blade section more than twice (undercut for an axial pull).
    Undercut,
    /// Surface draft against the axial pull below the minimum.
    Draft,
    /// Frame wall between bore and outline thinner than the minimum wall.
    FrameWall,
    /// Blade or hub outside the frame depth, or closer to the struts than the minimum axial gap.
    AxialFit,
    /// Blade root not on the motor hub.
    RootOffHub,
}

/// A located reason why a variant is rejected.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Violation {
    /// Reason.
    pub code: Code,
    /// Parameter path (for station-wise checks, the nearest control point).
    pub path: String,
    /// Blade index, for per-blade checks of a specimen with per-blade deviations.
    pub blade: Option<usize>,
    /// Radius [m] where the check failed.
    pub radius: Option<f64>,
    /// Measured value (unit of the check).
    pub value: f64,
    /// Limit it violates.
    pub limit: f64,
    /// Human-readable detail.
    pub message: String,
}

/// Thresholds of the checker (process inputs, not physics; ADR 0118 states their status).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Limits {
    /// Minimum moulded wall [m]: blade thickness, frame wall.
    pub min_wall: f64,
    /// Minimum tip gap [m].
    pub min_tip_gap: f64,
    /// Maximum tip gap over tip radius.
    pub max_tip_gap_rel: f64,
    /// Hub-to-tip ratio range.
    pub hub_tip: (f64, f64),
    /// Solidity range.
    pub solidity: (f64, f64),
    /// Minimum distance between neighbouring blade sections [m].
    pub min_passage_gap: f64,
    /// Minimum axial gap between blade and struts [m].
    pub min_axial_gap: f64,
    /// Apply the axial two-plate mould tests (axial overlap, undercut, draft).
    pub axial_mould: bool,
    /// Minimum draft [deg].
    pub min_draft_deg: f64,
    /// Chord fraction at each edge where the parting line runs (excluded from the draft test).
    pub edge_band: f64,
    /// Radii sampled uniformly over the span (control-point radii are added).
    pub n_radii: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            min_wall: 8e-4,
            min_tip_gap: 3e-4,
            max_tip_gap_rel: 0.05,
            hub_tip: (0.2, 0.7),
            solidity: (0.3, 2.0),
            min_passage_gap: 8e-4,
            min_axial_gap: 1e-3,
            axial_mould: true,
            min_draft_deg: 1.0,
            edge_band: 0.05,
            n_radii: 21,
        }
    }
}

/// Points per section side in the geometric tests.
const N_POLY: usize = 64;
/// Blade surface resolution of the mass properties (chord points per side, spanwise strips).
pub const MASS_RESOLUTION: (usize, usize) = (96, 48);

type Pt = (f64, f64);

fn seg_intersect(a: Pt, b: Pt, c: Pt, d: Pt) -> bool {
    let o = |p: Pt, q: Pt, r: Pt| (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0);
    let (d1, d2, d3, d4) = (o(c, d, a), o(c, d, b), o(a, b, c), o(a, b, d));
    d1 * d2 < 0.0 && d3 * d4 < 0.0
}

fn point_seg_dist(p: Pt, a: Pt, b: Pt) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l2 = dx * dx + dy * dy;
    let t = if l2 > 0.0 {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    ((p.0 - a.0 - t * dx).powi(2) + (p.1 - a.1 - t * dy).powi(2)).sqrt()
}

/// Crossings of the closed polygon with the axial line `s = s0` (half-open rule).
fn crossings(poly: &[Pt], s0: f64) -> Vec<f64> {
    let n = poly.len();
    let mut z = Vec::new();
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        if (a.1 > s0) != (b.1 > s0) {
            z.push(a.0 + (s0 - a.1) / (b.1 - a.1) * (b.0 - a.0));
        }
    }
    z
}

fn inside(poly: &[Pt], q: Pt) -> bool {
    crossings(poly, q.1).iter().filter(|&&z| z > q.0).count() % 2 == 1
}

/// Distance between two closed polygons (0 when they intersect or one contains the other).
fn polygon_distance(a: &[Pt], b: &[Pt]) -> f64 {
    let (na, nb) = (a.len(), b.len());
    for i in 0..na {
        for j in 0..nb {
            if seg_intersect(a[i], a[(i + 1) % na], b[j], b[(j + 1) % nb]) {
                return 0.0;
            }
        }
    }
    if inside(b, a[0]) || inside(a, b[0]) {
        return 0.0;
    }
    let mut d = f64::INFINITY;
    for i in 0..na {
        for j in 0..nb {
            d = d
                .min(point_seg_dist(a[i], b[j], b[(j + 1) % nb]))
                .min(point_seg_dist(b[j], a[i], a[(i + 1) % na]));
        }
    }
    d
}

/// Section of `fan` at radius `r` as a closed polygon in the unrolled plane `(z, s)`, shifted by `ds` in `s`.
fn section_polygon(fan: &AxialFan, r: f64, ds: f64) -> Vec<Pt> {
    let st = fan.station_at(r);
    let xs: Vec<f64> = (0..=N_POLY)
        .map(|i| 0.5 * (1.0 - (PI * i as f64 / N_POLY as f64).cos()))
        .collect();
    let mut v: Vec<Pt> = xs.iter().map(|&x| fan.surface_zs(&st, x, Side::Suction)).collect();
    v.extend(
        xs[1..N_POLY]
            .iter()
            .rev()
            .map(|&x| fan.surface_zs(&st, x, Side::Pressure)),
    );
    v.into_iter().map(|(z, s)| (z, s + ds)).collect()
}

fn rename_path(path: &str) -> String {
    path.replace(".radius", ".span")
        .replace(".stagger", ".stagger_deg")
        .replace(".lean", ".skew_deg")
        .replace("tip_clearance", "tip_gap")
}

impl ComputerFan {
    /// Number of blades whose geometry differs (1 when the blades are identical).
    fn distinct_blades(&self) -> usize {
        match &self.deviation {
            Some(d) if !d.blades.is_empty() => self.n_blades,
            _ => 1,
        }
    }

    /// Blade `k` (with this specimen's deviations) as the [`AxialFan`] of the sector generators, in a domain with
    /// `inlet` metres upstream of the frame and `outlet` metres downstream (the frame inlet face is at `z = inlet`).
    /// Not realised there: the frame outline, bell and struts, the hub length, eccentricity and the per-blade
    /// differences (the generators mesh one periodic passage).
    pub fn axial_fan(&self, k: usize, inlet: f64, outlet: f64) -> AxialFan {
        let d = self.deviation.as_ref();
        let b = d.and_then(|d| d.blades.get(k)).copied().unwrap_or_default();
        let dst = d.map_or(0.0, |d| d.stagger_common_deg) + b.stagger_deg;
        let (cf, tf) = (1.0 + b.chord_rel, (1.0 + b.thickness_rel) / (1.0 + b.chord_rel));
        let span = self.r_tip - self.r_hub;
        AxialFan {
            n_blades: self.n_blades,
            r_hub: self.r_hub,
            r_tip: self.r_tip,
            tip_clearance: self.tip_gap + d.map_or(0.0, |d| d.tip_gap_delta),
            axial_extent: inlet + self.frame.depth + outlet,
            z_le_root: inlet + self.z_le_root,
            family: self.family,
            stations: self
                .stations
                .iter()
                .map(|c| {
                    let r = if c.span == 1.0 {
                        self.r_tip
                    } else {
                        self.r_hub + c.span * span
                    };
                    Station {
                        radius: r,
                        chord: c.chord * cf,
                        stagger: (c.stagger_deg + dst).to_radians(),
                        camber: c.camber,
                        camber_pos: c.camber_pos,
                        thickness: c.thickness * tf,
                        sweep: c.sweep,
                        lean: r * c.skew_deg.to_radians(),
                    }
                })
                .collect(),
        }
    }

    /// Every scalar as `(path, spec key, value)`.
    pub fn scalars(&self) -> Vec<(String, &'static str, f64)> {
        let mut v: Vec<(String, &'static str, f64)> = Vec::new();
        let mut put = |path: String, key: &'static str, x: f64| v.push((path, key, x));
        put("n_blades".into(), "n_blades", self.n_blades as f64);
        put("r_hub".into(), "r_hub", self.r_hub);
        put("r_tip".into(), "r_tip", self.r_tip);
        put("tip_gap".into(), "tip_gap", self.tip_gap);
        put("z_le_root".into(), "z_le_root", self.z_le_root);
        put("density".into(), "density", self.density);
        put("roughness_ks".into(), "roughness_ks", self.roughness_ks);
        put("hub.z_start".into(), "hub.z_start", self.hub.z_start);
        put("hub.length".into(), "hub.length", self.hub.length);
        let f = &self.frame;
        put("frame.side".into(), "frame.side", f.side);
        put("frame.depth".into(), "frame.depth", f.depth);
        put(
            "frame.inlet_bell_radius".into(),
            "frame.inlet_bell_radius",
            f.inlet_bell_radius,
        );
        put("frame.n_struts".into(), "frame.n_struts", f.n_struts as f64);
        put("frame.strut_width".into(), "frame.strut_width", f.strut_width);
        put("frame.strut_depth".into(), "frame.strut_depth", f.strut_depth);
        for (i, c) in self.stations.iter().enumerate() {
            for (key, x) in [
                ("stations[].span", c.span),
                ("stations[].chord", c.chord),
                ("stations[].stagger_deg", c.stagger_deg),
                ("stations[].camber", c.camber),
                ("stations[].camber_pos", c.camber_pos),
                ("stations[].thickness", c.thickness),
                ("stations[].sweep", c.sweep),
                ("stations[].skew_deg", c.skew_deg),
            ] {
                put(key.replace("[]", &format!("[{i}]")), key, x);
            }
        }
        if let Some(d) = &self.deviation {
            put(
                "deviation.stagger_common_deg".into(),
                "deviation.stagger_common_deg",
                d.stagger_common_deg,
            );
            put(
                "deviation.tip_gap_delta".into(),
                "deviation.tip_gap_delta",
                d.tip_gap_delta,
            );
            put(
                "deviation.eccentricity".into(),
                "deviation.eccentricity",
                d.eccentricity,
            );
            put(
                "deviation.eccentricity_angle_deg".into(),
                "deviation.eccentricity_angle_deg",
                d.eccentricity_angle_deg,
            );
            for (i, b) in d.blades.iter().enumerate() {
                for (key, x) in [
                    ("deviation.blades[].stagger_deg", b.stagger_deg),
                    ("deviation.blades[].chord_rel", b.chord_rel),
                    ("deviation.blades[].thickness_rel", b.thickness_rel),
                    ("deviation.blades[].mass_rel", b.mass_rel),
                    ("deviation.blades[].roughness_ks", b.roughness_ks),
                ] {
                    put(key.replace("[]", &format!("[{i}]")), key, x);
                }
            }
        }
        v
    }

    /// Check the variant before any meshing. An empty list accepts it. Bounds and structure are checked first; when they
    /// fail, the geometric checks are not run (they need a valid blade).
    pub fn check(&self, lim: &Limits) -> Vec<Violation> {
        let mut out = Vec::new();
        let mut viol = |code, path: String, blade, radius, value, limit, message: String| {
            out.push(Violation {
                code,
                path,
                blade,
                radius,
                value,
                limit,
                message,
            })
        };
        for (path, key, x) in self.scalars() {
            let s = param_spec(key).expect("every scalar has a spec");
            if !(x >= s.min && x <= s.max) {
                viol(
                    Code::OutOfBounds,
                    path,
                    None,
                    None,
                    x,
                    if x < s.min { s.min } else { s.max },
                    format!("must lie in [{}, {}] {}", s.min, s.max, s.unit),
                );
            }
        }
        if let Some(d) = &self.deviation {
            if !d.blades.is_empty() && d.blades.len() != self.n_blades {
                viol(
                    Code::Invalid,
                    "deviation.blades".into(),
                    None,
                    None,
                    d.blades.len() as f64,
                    self.n_blades as f64,
                    "needs no entry or one entry per blade".into(),
                );
            }
        }
        for (i, c) in self.stations.iter().enumerate() {
            let want_end = (i == 0 && c.span != 0.0) || (i + 1 == self.stations.len() && c.span != 1.0);
            if want_end {
                viol(
                    Code::Invalid,
                    format!("stations[{i}].span"),
                    None,
                    None,
                    c.span,
                    if i == 0 { 0.0 } else { 1.0 },
                    "the first control point is at span 0, the last at span 1".into(),
                );
            }
        }
        for k in 0..self.distinct_blades() {
            for e in self.axial_fan(k, 0.0, 0.0).validate() {
                let blade = (self.distinct_blades() > 1).then_some(k);
                viol(
                    Code::Invalid,
                    rename_path(&e.path),
                    blade,
                    None,
                    f64::NAN,
                    f64::NAN,
                    e.message,
                );
            }
        }
        if !out.is_empty() {
            return out;
        }
        self.check_geometry(lim, &mut out);
        out
    }

    fn nearest_station(&self, r: f64) -> usize {
        let span = (r - self.r_hub) / (self.r_tip - self.r_hub);
        (0..self.stations.len())
            .min_by(|&a, &b| {
                (self.stations[a].span - span)
                    .abs()
                    .total_cmp(&(self.stations[b].span - span).abs())
            })
            .unwrap_or(0)
    }

    /// Radii of the geometric checks: `n_radii` uniform points and the control-point radii, ascending.
    pub fn check_radii(&self, lim: &Limits) -> Vec<f64> {
        let n = lim.n_radii.max(2);
        let mut r: Vec<f64> = (0..n)
            .map(|j| self.r_hub + (self.r_tip - self.r_hub) * j as f64 / (n - 1) as f64)
            .collect();
        r.extend(
            self.stations
                .iter()
                .map(|c| self.r_hub + c.span * (self.r_tip - self.r_hub)),
        );
        r.sort_by(f64::total_cmp);
        r.dedup_by(|a, b| (*a - *b).abs() <= 1e-12 * self.r_tip);
        r
    }

    fn check_geometry(&self, lim: &Limits, out: &mut Vec<Violation>) {
        let n = self.n_blades;
        let d = self.deviation.clone().unwrap_or_default();
        let nb = self.distinct_blades();
        let fans: Vec<AxialFan> = (0..nb).map(|k| self.axial_fan(k, 0.0, 0.0)).collect();
        let tag = |k: usize| (nb > 1).then_some(k);
        let radii = self.check_radii(lim);
        let push = |out: &mut Vec<Violation>, code, path: String, blade, radius, value, limit, message: String| {
            out.push(Violation {
                code,
                path,
                blade,
                radius,
                value,
                limit,
                message,
            })
        };
        // rotor-level checks
        let ratio = self.r_hub / self.r_tip;
        if ratio < lim.hub_tip.0 || ratio > lim.hub_tip.1 {
            let lm = if ratio < lim.hub_tip.0 {
                lim.hub_tip.0
            } else {
                lim.hub_tip.1
            };
            push(
                out,
                Code::HubTipRatio,
                "r_hub".into(),
                None,
                None,
                ratio,
                lm,
                format!(
                    "hub-to-tip ratio {ratio:.4} outside [{}, {}]",
                    lim.hub_tip.0, lim.hub_tip.1
                ),
            );
        }
        let gap = self.tip_gap + d.tip_gap_delta;
        let min_gap = gap - d.eccentricity;
        let gap_path = if d.eccentricity > 0.0 {
            "deviation.eccentricity"
        } else if d.tip_gap_delta != 0.0 {
            "deviation.tip_gap_delta"
        } else {
            "tip_gap"
        };
        if min_gap <= 0.0 {
            push(out, Code::TipRub, gap_path.into(), None, Some(self.r_tip), min_gap, 0.0, format!("smallest tip gap {min_gap:.3e} m (gap {gap:.3e} m less eccentricity {:.3e} m): the blade touches the bore", d.eccentricity));
        } else if min_gap < lim.min_tip_gap {
            push(
                out,
                Code::TipGap,
                gap_path.into(),
                None,
                Some(self.r_tip),
                min_gap,
                lim.min_tip_gap,
                format!("smallest tip gap {min_gap:.3e} m below {:.3e} m", lim.min_tip_gap),
            );
        }
        let max_gap = gap + d.eccentricity;
        if max_gap > lim.max_tip_gap_rel * self.r_tip {
            push(
                out,
                Code::TipGap,
                gap_path.into(),
                None,
                Some(self.r_tip),
                max_gap / self.r_tip,
                lim.max_tip_gap_rel,
                format!(
                    "largest tip gap {:.4} of r_tip above {}",
                    max_gap / self.r_tip,
                    lim.max_tip_gap_rel
                ),
            );
        }
        let bore = self.r_tip + gap;
        let wall = 0.5 * self.frame.side - bore;
        if wall < lim.min_wall {
            push(
                out,
                Code::FrameWall,
                "frame.side".into(),
                None,
                Some(bore),
                wall,
                lim.min_wall,
                format!(
                    "frame wall {wall:.3e} m between bore and outline below {:.3e} m",
                    lim.min_wall
                ),
            );
        }
        if self.hub.z_start + self.hub.length > self.frame.depth {
            push(
                out,
                Code::AxialFit,
                "hub.length".into(),
                None,
                None,
                self.hub.z_start + self.hub.length,
                self.frame.depth,
                "the motor hub ends behind the frame outlet face".into(),
            );
        }
        let z_max_allowed = self.frame.depth - self.frame.strut_depth - lim.min_axial_gap;
        let pitch_ang = TAU / n as f64;
        // per-blade, per-radius checks
        for (k, f) in fans.iter().enumerate() {
            let fnext = &fans[(k + 1) % nb];
            for &r in &radii {
                let st = f.station_at(r);
                let i = self.nearest_station(r);
                let t_abs = st.chord * st.thickness;
                if t_abs < lim.min_wall {
                    push(
                        out,
                        Code::MinThickness,
                        format!("stations[{i}].thickness"),
                        tag(k),
                        Some(r),
                        t_abs,
                        lim.min_wall,
                        format!(
                            "absolute thickness {t_abs:.3e} m below the minimum wall {:.3e} m",
                            lim.min_wall
                        ),
                    );
                }
                let sol = st.chord * n as f64 / (TAU * r);
                if sol < lim.solidity.0 || sol > lim.solidity.1 {
                    let lm = if sol < lim.solidity.0 {
                        lim.solidity.0
                    } else {
                        lim.solidity.1
                    };
                    push(
                        out,
                        Code::Solidity,
                        format!("stations[{i}].chord"),
                        tag(k),
                        Some(r),
                        sol,
                        lm,
                        format!("solidity {sol:.4} outside [{}, {}]", lim.solidity.0, lim.solidity.1),
                    );
                }
                let a = section_polygon(f, r, 0.0);
                let b = section_polygon(fnext, r, pitch_ang * r);
                let (zmin, zmax) = a.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), q| {
                    (lo.min(q.0), hi.max(q.0))
                });
                if zmax > z_max_allowed {
                    push(
                        out,
                        Code::AxialFit,
                        format!("stations[{i}].sweep"),
                        tag(k),
                        Some(r),
                        zmax,
                        z_max_allowed,
                        "blade reaches closer to the struts than the minimum axial gap (or past the frame)".into(),
                    );
                }
                if zmin < 0.0 {
                    push(
                        out,
                        Code::AxialFit,
                        format!("stations[{i}].sweep"),
                        tag(k),
                        Some(r),
                        zmin,
                        0.0,
                        "blade reaches ahead of the frame inlet face".into(),
                    );
                }
                if r == self.r_hub && (zmin < self.hub.z_start || zmax > self.hub.z_start + self.hub.length) {
                    let (v, lm) = if zmin < self.hub.z_start {
                        (zmin, self.hub.z_start)
                    } else {
                        (zmax, self.hub.z_start + self.hub.length)
                    };
                    push(
                        out,
                        Code::RootOffHub,
                        "z_le_root".into(),
                        tag(k),
                        Some(r),
                        v,
                        lm,
                        "the root section leaves the motor hub".into(),
                    );
                }
                let dist = polygon_distance(&a, &b);
                let pair = format!("blades {k} and {}", (k + 1) % n);
                if dist == 0.0 {
                    push(
                        out,
                        Code::Collision,
                        format!("stations[{i}].chord"),
                        tag(k),
                        Some(r),
                        0.0,
                        lim.min_passage_gap,
                        format!("sections of {pair} intersect"),
                    );
                } else if dist < lim.min_passage_gap {
                    push(
                        out,
                        Code::PassageGap,
                        format!("stations[{i}].chord"),
                        tag(k),
                        Some(r),
                        dist,
                        lim.min_passage_gap,
                        format!("{pair} are {dist:.3e} m apart"),
                    );
                }
                if lim.axial_mould {
                    let smax_a = a.iter().fold(f64::NEG_INFINITY, |m, q| m.max(q.1));
                    let smin_a = a.iter().fold(f64::INFINITY, |m, q| m.min(q.1));
                    let smin_b = b.iter().fold(f64::INFINITY, |m, q| m.min(q.1));
                    let smax_b = b.iter().fold(f64::NEG_INFINITY, |m, q| m.max(q.1));
                    let overlap = smax_a.min(smax_b) - smin_a.max(smin_b);
                    if overlap > 0.0 {
                        push(out, Code::AxialOverlap, format!("stations[{i}].chord"), tag(k), Some(r), overlap, 0.0, format!("{pair} overlap by {overlap:.3e} m in the axial view: an axial two-plate mould cannot release the steel between them"));
                    }
                    let lines = 4 * N_POLY;
                    let mut worst = (0usize, 0.0);
                    for q in 1..lines {
                        let s0 = smin_a + (smax_a - smin_a) * q as f64 / lines as f64;
                        let c = crossings(&a, s0).len();
                        if c > worst.0 {
                            worst = (c, s0);
                        }
                    }
                    if worst.0 > 2 {
                        push(
                            out,
                            Code::Undercut,
                            format!("stations[{i}].stagger_deg"),
                            tag(k),
                            Some(r),
                            worst.0 as f64,
                            2.0,
                            format!(
                                "the axial line s = {:.4e} m crosses the section {} times",
                                worst.1, worst.0
                            ),
                        );
                    }
                }
            }
            if lim.axial_mould {
                self.check_draft(f, &radii, lim, tag(k), out);
            }
        }
        // one violation per (code, path, blade): the first, i.e. the lowest radius
        let mut seen = std::collections::HashSet::new();
        out.retain(|v| seen.insert((v.code, v.path.clone(), v.blade)));
    }

    /// Draft `asin |n_z|` of the 3-D surface normal on both sides, inside the chord band `[edge_band, 1 - edge_band]`.
    fn check_draft(&self, f: &AxialFan, radii: &[f64], lim: &Limits, blade: Option<usize>, out: &mut Vec<Violation>) {
        let (h_r, h_x) = (1e-6 * (self.r_tip - self.r_hub), 1e-6);
        let nx = 41;
        for &r in radii {
            let (r0, r1) = ((r - h_r).max(self.r_hub), (r + h_r).min(self.r_tip));
            let mut worst = (f64::INFINITY, 0.0, Side::Suction);
            for side in [Side::Suction, Side::Pressure] {
                for q in 0..nx {
                    let x = lim.edge_band + (1.0 - 2.0 * lim.edge_band) * q as f64 / (nx - 1) as f64;
                    let dr = rcfd_core::vec3::sub(f.surface_point(r1, x, side), f.surface_point(r0, x, side));
                    let dx = rcfd_core::vec3::sub(f.surface_point(r, x + h_x, side), f.surface_point(r, x - h_x, side));
                    let nrm = rcfd_core::vec3::unit(rcfd_core::vec3::cross(dr, dx));
                    let draft = nrm[2].abs().min(1.0).asin().to_degrees();
                    if draft < worst.0 {
                        worst = (draft, x, side);
                    }
                }
            }
            if worst.0 < lim.min_draft_deg {
                let i = self.nearest_station(r);
                out.push(Violation {
                    code: Code::Draft,
                    path: format!("stations[{i}].stagger_deg"),
                    blade,
                    radius: Some(r),
                    value: worst.0,
                    limit: lim.min_draft_deg,
                    message: format!(
                        "draft {:.3} deg on the {:?} side at chord fraction {:.3}",
                        worst.0, worst.2, worst.1
                    ),
                });
            }
        }
    }

    /// Mass [kg] and centroid [m] of blade `k` (rotated to its azimuth `2 pi k / N`), from the triangulated blade at
    /// [`MASS_RESOLUTION`] and the density times `1 + mass_rel`.
    pub fn blade_mass(&self, k: usize) -> Result<(f64, [f64; 3]), Vec<FanError>> {
        let f = self.axial_fan(k, 0.0, 0.0);
        let s = f.blade_surface(MASS_RESOLUTION.0, MASS_RESOLUTION.1)?;
        let mrel = self
            .deviation
            .as_ref()
            .and_then(|d| d.blades.get(k))
            .map_or(0.0, |b| b.mass_rel);
        let c = s.centroid();
        let (sn, cs) = (TAU * k as f64 / self.n_blades as f64).sin_cos();
        Ok((
            self.density * s.volume() * (1.0 + mrel),
            [cs * c[0] - sn * c[1], sn * c[0] + cs * c[1], c[2]],
        ))
    }

    /// Static unbalance `sum_k m_k c_k` in the rotor plane [kg m] (blades only: the hub is taken as balanced and
    /// axisymmetric; the eccentricity of the axis in the bore moves the gap, not the mass about the axis).
    pub fn static_unbalance(&self) -> Result<[f64; 2], Vec<FanError>> {
        let mut u = [0.0; 2];
        for k in 0..self.n_blades {
            let (m, c) = self.blade_mass(k)?;
            u[0] += m * c[0];
            u[1] += m * c[1];
        }
        Ok(u)
    }
}

/// Standard normal cumulative distribution `(1 + erf(x / sqrt 2)) / 2` (series for `|x| < 3`, continued fraction beyond).
pub fn normal_cdf(x: f64) -> f64 {
    let y = x.abs() / 2f64.sqrt();
    let erfc = if y < 3.0 {
        // erf y = 2/sqrt(pi) e^{-y^2} sum 2^n y^{2n+1} / (1 3 ... (2n+1))
        let (mut term, mut sum, mut n) = (y, y, 0.0);
        while term > 1e-17 * sum {
            n += 1.0;
            term *= 2.0 * y * y / (2.0 * n + 1.0);
            sum += term;
        }
        1.0 - 2.0 / PI.sqrt() * (-y * y).exp() * sum
    } else {
        // erfc y = e^{-y^2}/sqrt(pi) / (y + (1/2)/(y + 1/(y + (3/2)/(y + ...)))), evaluated bottom-up
        let mut f = y;
        for k in (1..200).rev() {
            f = y + 0.5 * k as f64 / f;
        }
        (-y * y).exp() / PI.sqrt() / f
    };
    if x >= 0.0 {
        1.0 - 0.5 * erfc
    } else {
        0.5 * erfc
    }
}

fn normal_pdf(x: f64) -> f64 {
    (-0.5 * x * x).exp() / TAU.sqrt()
}

/// A named distribution of one perturbation.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Dist {
    /// Always `value`.
    Fixed {
        /// The value.
        value: f64,
    },
    /// Uniform on `[lo, hi]`.
    Uniform {
        /// Lower end.
        lo: f64,
        /// Upper end.
        hi: f64,
    },
    /// Normal `(mean, sd)` truncated (renormalised, not clamped) to `[lo, hi]`.
    TruncatedNormal {
        /// Mean of the parent normal.
        mean: f64,
        /// Standard deviation of the parent normal.
        sd: f64,
        /// Lower truncation.
        lo: f64,
        /// Upper truncation.
        hi: f64,
    },
}

impl Default for Dist {
    fn default() -> Self {
        Dist::Fixed { value: 0.0 }
    }
}

impl Dist {
    /// Located problem, if any (`lo < hi`, `sd > 0`, truncation mass at least 1e-3).
    pub fn validate(&self, path: &str) -> Option<FanError> {
        let bad = |m: &str| Some(FanError::new(path, m));
        match *self {
            Dist::Fixed { value } if !value.is_finite() => bad("value must be finite"),
            Dist::Uniform { lo, hi } if !(lo.is_finite() && hi.is_finite() && lo < hi) => bad("needs finite lo < hi"),
            Dist::TruncatedNormal { mean, sd, lo, hi } => {
                if !(mean.is_finite() && sd.is_finite() && sd > 0.0 && lo.is_finite() && hi.is_finite() && lo < hi) {
                    bad("needs finite mean, sd > 0 and finite lo < hi")
                } else if normal_cdf((hi - mean) / sd) - normal_cdf((lo - mean) / sd) < 1e-3 {
                    bad("the truncation keeps less than 1e-3 of the parent normal")
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Cumulative distribution.
    pub fn cdf(&self, x: f64) -> f64 {
        match *self {
            Dist::Fixed { value } => {
                if x >= value {
                    1.0
                } else {
                    0.0
                }
            }
            Dist::Uniform { lo, hi } => ((x - lo) / (hi - lo)).clamp(0.0, 1.0),
            Dist::TruncatedNormal { mean, sd, lo, hi } => {
                let (fa, fb) = (normal_cdf((lo - mean) / sd), normal_cdf((hi - mean) / sd));
                ((normal_cdf((x.clamp(lo, hi) - mean) / sd) - fa) / (fb - fa)).clamp(0.0, 1.0)
            }
        }
    }

    /// Mean (analytic).
    pub fn mean(&self) -> f64 {
        match *self {
            Dist::Fixed { value } => value,
            Dist::Uniform { lo, hi } => 0.5 * (lo + hi),
            Dist::TruncatedNormal { mean, sd, lo, hi } => {
                let (a, b) = ((lo - mean) / sd, (hi - mean) / sd);
                mean + sd * (normal_pdf(a) - normal_pdf(b)) / (normal_cdf(b) - normal_cdf(a))
            }
        }
    }

    /// Variance (analytic).
    pub fn variance(&self) -> f64 {
        match *self {
            Dist::Fixed { .. } => 0.0,
            Dist::Uniform { lo, hi } => (hi - lo).powi(2) / 12.0,
            Dist::TruncatedNormal { mean, sd, lo, hi } => {
                let (a, b) = ((lo - mean) / sd, (hi - mean) / sd);
                let z = normal_cdf(b) - normal_cdf(a);
                let m = (normal_pdf(a) - normal_pdf(b)) / z;
                sd * sd * (1.0 + (a * normal_pdf(a) - b * normal_pdf(b)) / z - m * m)
            }
        }
    }

    /// One draw (one uniform per draw; the truncated normal by inverting its CDF with bisection).
    pub fn sample(&self, rng: &mut Rng) -> f64 {
        match *self {
            Dist::Fixed { value } => value,
            Dist::Uniform { lo, hi } => rng.range(lo, hi),
            Dist::TruncatedNormal { mean, sd, lo, hi } => {
                let (a, b) = ((lo - mean) / sd, (hi - mean) / sd);
                let (fa, fb) = (normal_cdf(a), normal_cdf(b));
                let u = fa + (fb - fa) * rng.uniform();
                let (mut x0, mut x1) = (a, b);
                for _ in 0..100 {
                    let xm = 0.5 * (x0 + x1);
                    if xm <= x0 || xm >= x1 {
                        break;
                    }
                    if normal_cdf(xm) < u {
                        x0 = xm;
                    } else {
                        x1 = xm;
                    }
                }
                (mean + sd * 0.5 * (x0 + x1)).clamp(lo, hi)
            }
        }
    }
}

/// Distributions of the manufacturing errors (all `Fixed 0` by default: the nominal fan).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Tolerances {
    /// Stagger error common to all blades [deg], one draw per specimen.
    pub stagger_common_deg: Dist,
    /// Stagger error of each blade [deg].
    pub stagger_blade_deg: Dist,
    /// Relative chord error of each blade.
    pub chord_rel: Dist,
    /// Relative absolute-thickness error of each blade.
    pub thickness_rel: Dist,
    /// Error of the concentric tip gap [m], one draw per specimen.
    pub tip_gap_delta: Dist,
    /// Eccentricity of the axis in the bore [m], one draw per specimen (direction uniform in [0, 360) deg).
    pub eccentricity: Dist,
    /// Relative mass error of each blade.
    pub mass_rel: Dist,
    /// Roughness `ks` of each blade [m].
    pub roughness_ks: Dist,
}

impl Tolerances {
    fn fields(&self) -> [(&'static str, &Dist); 8] {
        [
            ("stagger_common_deg", &self.stagger_common_deg),
            ("stagger_blade_deg", &self.stagger_blade_deg),
            ("chord_rel", &self.chord_rel),
            ("thickness_rel", &self.thickness_rel),
            ("tip_gap_delta", &self.tip_gap_delta),
            ("eccentricity", &self.eccentricity),
            ("mass_rel", &self.mass_rel),
            ("roughness_ks", &self.roughness_ks),
        ]
    }

    /// Located problems of the distributions.
    pub fn validate(&self) -> Vec<FanError> {
        self.fields().iter().filter_map(|(k, d)| d.validate(k)).collect()
    }
}

impl ComputerFan {
    /// `n` perturbed specimens of this (nominal) fan, reproducible from `seed`. Draw order per specimen: common stagger,
    /// tip-gap error, eccentricity, its direction, then per blade stagger, chord, thickness, mass, roughness. Specimens are
    /// not checked: run [`ComputerFan::check`] on each.
    pub fn ensemble(&self, tol: &Tolerances, n: usize, seed: u64) -> Result<Vec<ComputerFan>, Vec<FanError>> {
        let e = tol.validate();
        if !e.is_empty() {
            return Err(e);
        }
        let mut rng = Rng::new(seed);
        Ok((0..n)
            .map(|_| {
                let mut d = Deviation {
                    stagger_common_deg: tol.stagger_common_deg.sample(&mut rng),
                    tip_gap_delta: tol.tip_gap_delta.sample(&mut rng),
                    eccentricity: tol.eccentricity.sample(&mut rng),
                    eccentricity_angle_deg: rng.range(0.0, 360.0),
                    blades: Vec::with_capacity(self.n_blades),
                };
                for _ in 0..self.n_blades {
                    d.blades.push(BladeDeviation {
                        stagger_deg: tol.stagger_blade_deg.sample(&mut rng),
                        chord_rel: tol.chord_rel.sample(&mut rng),
                        thickness_rel: tol.thickness_rel.sample(&mut rng),
                        mass_rel: tol.mass_rel.sample(&mut rng),
                        roughness_ks: tol.roughness_ks.sample(&mut rng),
                    });
                }
                ComputerFan {
                    deviation: Some(d),
                    ..self.clone()
                }
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_cdf_matches_known_values() {
        // Phi(0) = 1/2; Phi(1) and Phi(-3) from the series identity Phi(x) + Phi(-x) = 1 and tabulated values
        assert!((normal_cdf(0.0) - 0.5).abs() < 1e-15);
        assert!((normal_cdf(1.0) + normal_cdf(-1.0) - 1.0).abs() < 1e-15);
        assert!((normal_cdf(1.0) - 0.841_344_746_068_543).abs() < 1e-13);
        assert!((normal_cdf(-3.0) - 0.001_349_898_031_630_1).abs() < 1e-15);
        assert!((normal_cdf(-5.0) - 2.866_515_718_791_9e-7).abs() < 1e-18);
        // continuity across the series / continued-fraction switch at |x| = 3 sqrt 2: the jump over 2h is 2 h phi(x)
        for x in [3.0 * 2f64.sqrt(), -3.0 * 2f64.sqrt()] {
            let h = 1e-9;
            let jump = normal_cdf(x + h) - normal_cdf(x - h) - 2.0 * h * normal_pdf(x);
            assert!(jump.abs() < 5e-16, "{x} {jump:e}");
        }
    }
}
