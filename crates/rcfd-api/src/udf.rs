//! Sandboxed user functions (F-122): WebAssembly modules that supply a momentum source, a velocity
//! profile, a kinematic viscosity or a monitor integrand.
//!
//! ABI version 1 (docs/udf-abi.md). A module must export `rcfd_abi_version() -> i32` returning
//! [`ABI_VERSION`], and may export any of
//!
//! * `momentum_source(x, y, z, t, ux, uy, uz: f64, comp: i32) -> f64`: acceleration component
//!   `comp` (0, 1, 2) in m/s^2 at a cell centre
//! * `velocity_profile(x, y, z, t: f64, comp: i32) -> f64`: velocity component in m/s at a boundary
//!   face centre
//! * `kinematic_viscosity(x, y, z, t, temperature, speed: f64) -> f64`: m^2/s at a face centre
//! * `monitor(x, y, z, volume, ux, uy, uz, p, temperature: f64) -> f64`: integrand per cell; the
//!   monitor is the sum of `value * volume`
//!
//! Hook catalogue v2 (F-239, ADR 0088), also ABI version 1 (optional, additive exports):
//!
//! * `temperature_profile(x, y, z, t) -> K` and `wall_heat_flux(x, y, z, t) -> W/m^2` (into the domain) at a
//!   boundary face centre
//! * `thermal_conductivity(x, y, z, t, temperature) -> W/(m K)` at a face centre; it must be positive
//! * `energy_source(x, y, z, t, temperature) -> W/m^3` and `scalar_source(x, y, z, t, value) -> value/s` at a cell
//!   centre
//! * `initial_value(x, y, z, comp: i32) -> f64` at a cell centre: velocity (`comp` 0, 1, 2), kinematic pressure
//!   (3), temperature (4)
//! * `execute_at_end` with the `monitor` signature, evaluated once when the run ends
//!
//! Isolation: the module may not import anything, so it has no way to reach the file system, the
//! network, the clock or the host memory (ABI 1 offers no host functions and no WASI). Each call
//! is limited by fuel (instructions), linear memory is capped, and results must be finite.

use crate::case::{Case, Diagnostic};
use rcfd_core::vec3::Vec3;
use std::collections::BTreeMap;
use wasmi::core::F64;
use wasmi::{Config, Engine, Linker, Module, Store, StoreLimits, StoreLimitsBuilder, TypedFunc};

/// The ABI version this build speaks.
pub const ABI_VERSION: i32 = 1;
/// Largest accepted module (bytes).
pub const MAX_MODULE_BYTES: usize = 4 << 20;
/// Cap on the linear memory of an instance (bytes).
pub const MAX_MEMORY_BYTES: usize = 16 << 20;
/// Default instruction budget of one call.
pub const DEFAULT_FUEL: u64 = 1_000_000;

type MomentumFn = TypedFunc<(F64, F64, F64, F64, F64, F64, F64, i32), F64>;
type ProfileFn = TypedFunc<(F64, F64, F64, F64, i32), F64>;
type ViscosityFn = TypedFunc<(F64, F64, F64, F64, F64, F64), F64>;
type MonitorFn = TypedFunc<(F64, F64, F64, F64, F64, F64, F64, F64, F64), F64>;
type Fn4 = TypedFunc<(F64, F64, F64, F64), F64>;
type Fn5 = TypedFunc<(F64, F64, F64, F64, F64), F64>;
type InitFn = TypedFunc<(F64, F64, F64, i32), F64>;

/// An instantiated user-function module.
pub struct Udf {
    name: String,
    store: Store<StoreLimits>,
    fuel_cap: u64,
    fuel_added: u64,
    momentum: Option<MomentumFn>,
    profile: Option<ProfileFn>,
    viscosity: Option<ViscosityFn>,
    monitor: Option<MonitorFn>,
    temperature_profile: Option<Fn4>,
    wall_heat_flux: Option<Fn4>,
    conductivity: Option<Fn5>,
    energy_source: Option<Fn5>,
    scalar_source: Option<Fn5>,
    initial_value: Option<InitFn>,
    at_end: Option<MonitorFn>,
    /// Instructions used by the most expensive call so far.
    pub max_fuel_per_call: u64,
}

fn w(x: f64) -> F64 {
    F64::from_float(x)
}

fn finite(name: &str, what: &str, v: F64) -> Result<f64, String> {
    let v = v.to_float();
    if v.is_finite() {
        Ok(v)
    } else {
        Err(format!("user function '{name}': {what} returned a non-finite value"))
    }
}

impl Udf {
    /// Compile, validate and instantiate a module.
    pub fn load(name: &str, bytes: &[u8], fuel_per_call: u64) -> Result<Udf, String> {
        let err = |m: String| format!("user function '{name}': {m}");
        if bytes.len() > MAX_MODULE_BYTES {
            return Err(err(format!("module is larger than {MAX_MODULE_BYTES} bytes")));
        }
        if fuel_per_call == 0 {
            return Err(err("fuel_per_call must be positive".into()));
        }
        let mut config = Config::default();
        config.consume_fuel(true);
        let engine = Engine::new(&config);
        let module = Module::new(&engine, bytes).map_err(|e| err(format!("not a valid WebAssembly module: {e}")))?;
        if let Some(i) = module.imports().next() {
            return Err(err(format!(
                "imports '{}.{}', but ABI {ABI_VERSION} allows no imports (no file system, network or host access)",
                i.module(),
                i.name()
            )));
        }
        let limits = StoreLimitsBuilder::new()
            .memory_size(MAX_MEMORY_BYTES)
            .instances(1)
            .memories(1)
            .tables(1)
            .table_elements(10_000)
            .trap_on_grow_failure(true)
            .build();
        let mut store = Store::new(&engine, limits);
        store.limiter(|l| l);
        let mut u = Udf {
            name: name.to_string(),
            store,
            fuel_cap: fuel_per_call,
            fuel_added: 0,
            momentum: None,
            profile: None,
            viscosity: None,
            monitor: None,
            temperature_profile: None,
            wall_heat_flux: None,
            conductivity: None,
            energy_source: None,
            scalar_source: None,
            initial_value: None,
            at_end: None,
            max_fuel_per_call: 0,
        };
        u.refuel();
        let linker: Linker<StoreLimits> = Linker::new(&engine);
        let instance = linker
            .instantiate(&mut u.store, &module)
            .and_then(|pre| pre.start(&mut u.store))
            .map_err(|e| err(format!("cannot instantiate: {e}")))?;
        let version = instance
            .get_typed_func::<(), i32>(&u.store, "rcfd_abi_version")
            .map_err(|e| err(format!("must export rcfd_abi_version() -> i32: {e}")))?
            .call(&mut u.store, ())
            .map_err(|e| err(format!("rcfd_abi_version failed: {e}")))?;
        if version != ABI_VERSION {
            return Err(err(format!(
                "ABI version {version} is not supported (this build speaks version {ABI_VERSION})"
            )));
        }
        macro_rules! export {
            ($field:ident, $name:expr) => {
                if instance.get_export(&u.store, $name).is_some() {
                    u.$field = Some(
                        instance
                            .get_typed_func::<_, F64>(&u.store, $name)
                            .map_err(|e| err(format!("export '{}' has the wrong signature: {e}", $name)))?,
                    );
                }
            };
        }
        export!(momentum, "momentum_source");
        export!(profile, "velocity_profile");
        export!(viscosity, "kinematic_viscosity");
        export!(monitor, "monitor");
        export!(temperature_profile, "temperature_profile");
        export!(wall_heat_flux, "wall_heat_flux");
        export!(conductivity, "thermal_conductivity");
        export!(energy_source, "energy_source");
        export!(scalar_source, "scalar_source");
        export!(initial_value, "initial_value");
        export!(at_end, "execute_at_end");
        Ok(u)
    }

    /// Name given in the case file.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Exported entry points, by ABI name.
    pub fn exports(&self) -> Vec<&'static str> {
        [
            (self.momentum.is_some(), "momentum_source"),
            (self.profile.is_some(), "velocity_profile"),
            (self.viscosity.is_some(), "kinematic_viscosity"),
            (self.monitor.is_some(), "monitor"),
            (self.temperature_profile.is_some(), "temperature_profile"),
            (self.wall_heat_flux.is_some(), "wall_heat_flux"),
            (self.conductivity.is_some(), "thermal_conductivity"),
            (self.energy_source.is_some(), "energy_source"),
            (self.scalar_source.is_some(), "scalar_source"),
            (self.initial_value.is_some(), "initial_value"),
            (self.at_end.is_some(), "execute_at_end"),
        ]
        .into_iter()
        .filter_map(|(on, n)| on.then_some(n))
        .collect()
    }

    /// One call of a single-result export: fuel renewed, cost recorded, result finite.
    fn call1<P: wasmi::WasmParams>(
        &mut self,
        f: Option<TypedFunc<P, F64>>,
        what: &str,
        args: P,
    ) -> Result<f64, String> {
        let f = f.ok_or_else(|| format!("'{}' has no {what}", self.name))?;
        self.refuel();
        let before = self.store.fuel_consumed().unwrap_or(0);
        let v = f
            .call(&mut self.store, args)
            .map_err(|e| format!("user function '{}': {what} failed: {e}", self.name))?;
        self.account(before);
        finite(&self.name, what, v)
    }

    /// Boundary temperature (K) at a face centre.
    pub fn temperature_profile(&mut self, x: Vec3, t: f64) -> Result<f64, String> {
        self.call1(
            self.temperature_profile,
            "temperature_profile",
            (w(x[0]), w(x[1]), w(x[2]), w(t)),
        )
    }

    /// Wall heat flux into the domain (W/m^2) at a face centre.
    pub fn wall_heat_flux(&mut self, x: Vec3, t: f64) -> Result<f64, String> {
        self.call1(self.wall_heat_flux, "wall_heat_flux", (w(x[0]), w(x[1]), w(x[2]), w(t)))
    }

    /// Thermal conductivity (W/(m K)) at a face centre; it must be positive.
    pub fn thermal_conductivity(&mut self, x: Vec3, t: f64, temperature: f64) -> Result<f64, String> {
        let args = (w(x[0]), w(x[1]), w(x[2]), w(t), w(temperature));
        let v = self.call1(self.conductivity, "thermal_conductivity", args)?;
        if v <= 0.0 {
            return Err(format!(
                "user function '{}': thermal_conductivity returned {v}, which is not positive",
                self.name
            ));
        }
        Ok(v)
    }

    /// Volumetric heat source (W/m^3) at a cell centre.
    pub fn energy_source(&mut self, x: Vec3, t: f64, temperature: f64) -> Result<f64, String> {
        let args = (w(x[0]), w(x[1]), w(x[2]), w(t), w(temperature));
        self.call1(self.energy_source, "energy_source", args)
    }

    /// Source of a passive scalar (value per second) at a cell centre.
    pub fn scalar_source(&mut self, x: Vec3, t: f64, value: f64) -> Result<f64, String> {
        self.call1(
            self.scalar_source,
            "scalar_source",
            (w(x[0]), w(x[1]), w(x[2]), w(t), w(value)),
        )
    }

    /// Initial value of field component `comp` (0..2 velocity, 3 kinematic pressure, 4 temperature) at a cell centre.
    pub fn initial_value(&mut self, x: Vec3, comp: i32) -> Result<f64, String> {
        self.call1(self.initial_value, "initial_value", (w(x[0]), w(x[1]), w(x[2]), comp))
    }

    /// End-of-run integrand of one cell (the `monitor` signature).
    pub fn execute_at_end(&mut self, x: Vec3, volume: f64, u: Vec3, p: f64, temperature: f64) -> Result<f64, String> {
        let args = (
            w(x[0]),
            w(x[1]),
            w(x[2]),
            w(volume),
            w(u[0]),
            w(u[1]),
            w(u[2]),
            w(p),
            w(temperature),
        );
        self.call1(self.at_end, "execute_at_end", args)
    }

    fn refuel(&mut self) {
        let used = self.store.fuel_consumed().unwrap_or(0);
        let left = self.fuel_added.saturating_sub(used);
        if left < self.fuel_cap {
            let d = self.fuel_cap - left;
            let _ = self.store.add_fuel(d);
            self.fuel_added += d;
        }
    }

    fn account(&mut self, before: u64) {
        let used = self.store.fuel_consumed().unwrap_or(0).saturating_sub(before);
        self.max_fuel_per_call = self.max_fuel_per_call.max(used);
    }

    /// Acceleration (m/s^2) at a cell centre.
    pub fn momentum_source(&mut self, x: Vec3, t: f64, u: Vec3) -> Result<Vec3, String> {
        let f = self
            .momentum
            .ok_or_else(|| format!("'{}' has no momentum_source", self.name))?;
        self.refuel();
        let before = self.store.fuel_consumed().unwrap_or(0);
        let mut out = [0.0; 3];
        for (k, o) in out.iter_mut().enumerate() {
            let v = f
                .call(
                    &mut self.store,
                    (w(x[0]), w(x[1]), w(x[2]), w(t), w(u[0]), w(u[1]), w(u[2]), k as i32),
                )
                .map_err(|e| format!("user function '{}': momentum_source failed: {e}", self.name))?;
            *o = finite(&self.name, "momentum_source", v)?;
        }
        self.account(before);
        Ok(out)
    }

    /// Velocity (m/s) at a boundary face centre.
    pub fn velocity_profile(&mut self, x: Vec3, t: f64) -> Result<Vec3, String> {
        let f = self
            .profile
            .ok_or_else(|| format!("'{}' has no velocity_profile", self.name))?;
        self.refuel();
        let before = self.store.fuel_consumed().unwrap_or(0);
        let mut out = [0.0; 3];
        for (k, o) in out.iter_mut().enumerate() {
            let v = f
                .call(&mut self.store, (w(x[0]), w(x[1]), w(x[2]), w(t), k as i32))
                .map_err(|e| format!("user function '{}': velocity_profile failed: {e}", self.name))?;
            *o = finite(&self.name, "velocity_profile", v)?;
        }
        self.account(before);
        Ok(out)
    }

    /// Kinematic viscosity (m^2/s) at a face centre; it must be positive.
    pub fn kinematic_viscosity(&mut self, x: Vec3, t: f64, temperature: f64, speed: f64) -> Result<f64, String> {
        let f = self
            .viscosity
            .ok_or_else(|| format!("'{}' has no kinematic_viscosity", self.name))?;
        self.refuel();
        let before = self.store.fuel_consumed().unwrap_or(0);
        let v = f
            .call(
                &mut self.store,
                (w(x[0]), w(x[1]), w(x[2]), w(t), w(temperature), w(speed)),
            )
            .map_err(|e| format!("user function '{}': kinematic_viscosity failed: {e}", self.name))?;
        self.account(before);
        let v = finite(&self.name, "kinematic_viscosity", v)?;
        if v <= 0.0 {
            return Err(format!(
                "user function '{}': kinematic_viscosity returned {v}, which is not positive",
                self.name
            ));
        }
        Ok(v)
    }

    /// Monitor integrand of one cell.
    pub fn monitor(&mut self, x: Vec3, volume: f64, u: Vec3, p: f64, temperature: f64) -> Result<f64, String> {
        let f = self.monitor.ok_or_else(|| format!("'{}' has no monitor", self.name))?;
        self.refuel();
        let before = self.store.fuel_consumed().unwrap_or(0);
        let v = f
            .call(
                &mut self.store,
                (
                    w(x[0]),
                    w(x[1]),
                    w(x[2]),
                    w(volume),
                    w(u[0]),
                    w(u[1]),
                    w(u[2]),
                    w(p),
                    w(temperature),
                ),
            )
            .map_err(|e| format!("user function '{}': monitor failed: {e}", self.name))?;
        self.account(before);
        finite(&self.name, "monitor", v)
    }
}

/// Modules of a case: `udf.<name>` entries loaded and instantiated, plus one diagnostic per entry
/// that cannot be used (missing or unreadable file, outside the sandbox, rejected module, a
/// requested role without the matching export, more than one provider of a role).
pub fn load_case_udfs(case: &Case) -> (BTreeMap<String, Udf>, Vec<Diagnostic>) {
    let mut out = BTreeMap::new();
    let mut diags = Vec::new();
    let get_s = |p: &str| {
        case.tree
            .get(p)
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default()
    };
    let get_b = |p: &str| case.tree.get(p).ok().and_then(|v| v.as_bool()).unwrap_or(false);
    let (mut momentum, mut viscosity) = (Vec::new(), Vec::new());
    let (mut conductivity, mut energy, mut init) = (Vec::new(), Vec::new(), Vec::new());
    for n in case.tree.names("udf").unwrap_or_default() {
        let p = |f: &str| format!("udf.{n}.{f}");
        let bad = |path: String, message: String| Diagnostic {
            path,
            line: None,
            message,
        };
        if n.is_empty() || !n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            diags.push(bad(
                format!("udf.{n}"),
                "a user-function name uses letters, digits and underscores".into(),
            ));
            continue;
        }
        let file = get_s(&p("file"));
        if file.is_empty() {
            diags.push(bad(p("file"), "a user function needs a .wasm file".into()));
            continue;
        }
        let path = case.base_dir.join(&file);
        if let Some(root) = &case.sandbox {
            let inside = !std::path::Path::new(&file)
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
                && path
                    .canonicalize()
                    .ok()
                    .zip(root.canonicalize().ok())
                    .map_or(true, |(p, r)| p.starts_with(r));
            if !inside {
                diags.push(bad(p("file"), format!("module '{file}' is outside the sandbox")));
                continue;
            }
        }
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) => {
                diags.push(bad(p("file"), format!("cannot read {}: {e}", path.display())));
                continue;
            }
        };
        let fuel = case
            .tree
            .get(&p("fuel_per_call"))
            .ok()
            .and_then(|v| v.as_i64())
            .unwrap_or(DEFAULT_FUEL as i64)
            .max(0) as u64;
        let u = match Udf::load(&n, &bytes, fuel) {
            Ok(u) => u,
            Err(e) => {
                diags.push(bad(p("file"), e));
                continue;
            }
        };
        for (flag, export, list) in [
            ("momentum_source", "momentum_source", &mut momentum),
            ("viscosity", "kinematic_viscosity", &mut viscosity),
            ("conductivity", "thermal_conductivity", &mut conductivity),
            ("energy_source", "energy_source", &mut energy),
            ("initialise", "initial_value", &mut init),
        ] {
            if get_b(&p(flag)) {
                if !u.exports().contains(&export) {
                    diags.push(bad(p(flag), format!("the module has no '{export}' export")));
                } else {
                    list.push(n.clone());
                }
            }
        }
        for (flag, export) in [("monitor", "monitor"), ("at_end", "execute_at_end")] {
            if get_b(&p(flag)) && !u.exports().contains(&export) {
                diags.push(bad(p(flag), format!("the module has no '{export}' export")));
            }
        }
        out.insert(n, u);
    }
    for (what, list) in [
        ("momentum_source", &momentum),
        ("viscosity", &viscosity),
        ("conductivity", &conductivity),
        ("energy_source", &energy),
        ("initialise", &init),
    ] {
        if list.len() > 1 {
            diags.push(Diagnostic {
                path: format!("udf.{}.{what}", list[1]),
                line: None,
                message: format!("only one user function can provide {what} (already {})", list[0]),
            });
        }
    }
    // references by name: (setting path, condition path (empty: none), condition value, export)
    let mut refs: Vec<(String, String, &str, &str)> = Vec::new();
    for n in case.tree.names("boundaries").unwrap_or_default() {
        let b = format!("boundaries.{n}");
        refs.push((
            format!("{b}.velocity_udf"),
            format!("{b}.velocity"),
            "fixed",
            "velocity_profile",
        ));
        refs.push((
            format!("{b}.temperature_udf"),
            format!("{b}.temperature"),
            "fixed",
            "temperature_profile",
        ));
        refs.push((
            format!("{b}.heat_flux_udf"),
            format!("{b}.temperature"),
            "heat_flux",
            "wall_heat_flux",
        ));
    }
    for n in case.tree.names("scalars").unwrap_or_default() {
        refs.push((format!("scalars.{n}.source_udf"), String::new(), "", "scalar_source"));
    }
    for (path, np, nv, export) in refs {
        let u = get_s(&path);
        if u.is_empty() {
            continue;
        }
        let message = if !np.is_empty() && get_s(&np) != nv {
            format!("{path} needs {np} = \"{nv}\"")
        } else if let Some(m) = out.get(&u) {
            if m.exports().contains(&export) {
                continue;
            }
            format!("user function '{u}' has no '{export}' export")
        } else if !diags.iter().any(|d| d.path.starts_with(&format!("udf.{u}"))) {
            format!("no user function '{u}' (define udf.{u} with a file)")
        } else {
            continue;
        };
        diags.push(Diagnostic {
            path,
            line: None,
            message,
        });
    }
    (out, diags)
}
