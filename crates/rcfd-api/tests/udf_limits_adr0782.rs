//! ADR 0782, F-856: the limits of a user-function module after the change of the interpreter from wasmi 0.31 to
//! wasmi 1.1.0. The fuel of a call, the cap on the linear memory, the refusal of a module that imports anything, and
//! the set of accepted modules, each through the public interface of `rcfd_api::udf` alone. The checks are L1 to L3,
//! M1, M2, I1, A1 and A2 of the registration; the same file was run on the tree before the port as the control.

use rcfd_api::udf::{Udf, ABI_VERSION, DEFAULT_FUEL};

fn wasm(wat_text: &str) -> Vec<u8> {
    wat::parse_str(wat_text).unwrap()
}

/// Module with the given extra items and a correct ABI export.
fn module(body: &str) -> String {
    format!("(module (func (export \"rcfd_abi_version\") (result i32) i32.const {ABI_VERSION}) {body})")
}

const PARAMS: &str = "(param f64 f64 f64 f64 f64 f64) (result f64)";

/// `kinematic_viscosity` that turns a loop as often as its `temperature` argument says and returns 1.5.
fn counted_loop() -> Vec<u8> {
    wasm(&module(&format!(
        "(func (export \"kinematic_viscosity\") {PARAMS} (local $i i32) \
         local.get 4 i32.trunc_f64_s local.set $i \
         (block $done (loop $l \
           local.get $i i32.eqz br_if $done \
           local.get $i i32.const 1 i32.sub local.set $i \
           br $l)) \
         f64.const 1.5)"
    )))
}

fn turns(u: &mut Udf, n: f64) -> Result<f64, String> {
    u.kinematic_viscosity([0.0; 3], 0.0, n, 1.0)
}

/// L1: the fuel stops a long loop, the next call has its whole budget again, and a small budget stops a short loop.
#[test]
fn l1_fuel_stops_a_long_loop_and_is_renewed() {
    let mut u = Udf::load("loop", &counted_loop(), DEFAULT_FUEL).unwrap();
    assert_eq!(turns(&mut u, 100.0).unwrap(), 1.5);
    let e = turns(&mut u, 1.0e7).unwrap_err();
    assert!(e.contains("kinematic_viscosity failed"), "{e}");
    assert_eq!(turns(&mut u, 10.0).unwrap(), 1.5);
    let mut small = Udf::load("small", &counted_loop(), 20).unwrap();
    let e = turns(&mut small, 1000.0).unwrap_err();
    assert!(e.contains("kinematic_viscosity failed"), "{e}");
}

/// L2: the recorded cost of a call is positive, grows with the work and stays within the budget. F1: the numbers.
#[test]
fn l2_the_recorded_cost_grows_with_the_work() {
    let mut u = Udf::load("loop", &counted_loop(), DEFAULT_FUEL).unwrap();
    turns(&mut u, 10.0).unwrap();
    let c10 = u.max_fuel_per_call;
    turns(&mut u, 100.0).unwrap();
    let c100 = u.max_fuel_per_call;
    eprintln!("F1 max_fuel_per_call: 10 turns {c10}, 100 turns {c100}");
    assert!(c10 > 0, "{c10}");
    assert!(c100 > c10, "{c100} against {c10}");
    assert!(c100 <= DEFAULT_FUEL, "{c100}");
}

/// L3: the start function runs under the fuel of one call.
#[test]
fn l3_a_start_function_that_never_ends_is_refused() {
    let e = Udf::load("start", &wasm(&module("(func $s (loop $l br $l)) (start $s)")), 10_000)
        .err()
        .unwrap();
    assert!(e.contains("cannot instantiate"), "{e}");
}

/// `kinematic_viscosity` that grows its one-page memory by `temperature` pages and returns the size in pages.
fn growing() -> Vec<u8> {
    wasm(&module(&format!(
        "(memory 1) (func (export \"kinematic_viscosity\") {PARAMS} \
         local.get 4 i32.trunc_f64_s memory.grow drop \
         memory.size f64.convert_i32_u)"
    )))
}

/// M1: growth to exactly 16 MiB (256 pages) is allowed and one page more fails the call; the fuel is far above the
/// cost of the growth, so the failure is the cap's.
#[test]
fn m1_memory_grows_to_the_cap_and_not_beyond() {
    let fuel = 100_000_000;
    let mut at = Udf::load("at", &growing(), fuel).unwrap();
    assert_eq!(turns(&mut at, 255.0).unwrap(), 256.0);
    let mut over = Udf::load("over", &growing(), fuel).unwrap();
    let e = turns(&mut over, 256.0).unwrap_err();
    assert!(e.contains("kinematic_viscosity failed"), "{e}");
}

/// M2: the cap also holds for the memory a module declares.
#[test]
fn m2_a_declared_memory_above_the_cap_is_refused() {
    assert!(Udf::load("fits", &wasm(&module("(memory 256)")), 10_000).is_ok());
    let e = Udf::load("big", &wasm(&module("(memory 257)")), 10_000).err().unwrap();
    assert!(e.contains("cannot instantiate"), "{e}");
}

/// I1: an import of any kind is refused with the import's name.
#[test]
fn i1_an_import_of_any_kind_is_refused() {
    for (name, import) in [
        ("f", "(func)"),
        ("m", "(memory 1)"),
        ("g", "(global i32)"),
        ("t", "(table 1 funcref)"),
    ] {
        let text = format!(
            "(module (import \"env\" \"{name}\" {import}) \
             (func (export \"rcfd_abi_version\") (result i32) i32.const {ABI_VERSION}))"
        );
        let e = Udf::load("w", &wasm(&text), 1000).err().unwrap();
        assert!(e.contains("imports") && e.contains(&format!("env.{name}")), "{e}");
    }
}

/// A1: the set of accepted modules is the one of the interpreter before: no second memory, no 64-bit memory, no tail
/// call and no extended constant expression.
#[test]
fn a1_proposals_outside_the_earlier_set_are_refused() {
    for (what, body) in [
        ("two memories", "(memory 1) (memory 1)"),
        ("64-bit memory", "(memory i64 1)"),
        (
            "tail call",
            "(func $f (result i32) i32.const 1) (func (export \"x\") (result i32) return_call $f)",
        ),
        (
            "extended constant",
            "(global i32 (i32.add (i32.const 1) (i32.const 2)))",
        ),
    ] {
        let e = Udf::load("w", &wasm(&module(body)), 1000)
            .err()
            .unwrap_or_else(|| panic!("{what}: loaded"));
        assert!(e.contains("not a valid WebAssembly module"), "{what}: {e}");
    }
}

/// A2: the text format is not a module.
#[test]
fn a2_the_text_format_is_refused() {
    let text = module("");
    let e = Udf::load("w", text.as_bytes(), 1000).err().unwrap();
    assert!(e.contains("not a valid WebAssembly module"), "{e}");
}
