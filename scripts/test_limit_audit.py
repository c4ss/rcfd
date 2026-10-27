"""Self-test of scripts/limit_audit.py (ADR 0129 A1): one planted example per class and one clean test.
Run: python -m unittest scripts/test_limit_audit.py -v"""
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from limit_audit import checking_fns, scan  # noqa: E402

PLANTED = r'''
const BUDGET: Duration = Duration::from_millis(500);

#[test]
fn planted_i_sleep_always_exceeds_its_budget() {
    let t = Instant::now();
    for _ in 0..500 {
        std::thread::sleep(Duration::from_millis(2));
    }
    let slow = t.elapsed();
    assert!(slow > BUDGET, "negative control");
}

#[test]
fn planted_i_literals_only() {
    let tol = 1e-3;
    assert!(0.5 * tol < 1e-2);
}

#[test]
fn planted_ii_no_check() {
    let x = compute(3);
    println!("{x}");
}

#[test]
fn planted_iii_negative_control_expects_success() {
    let r = run_without_the_fix();
    assert_eq!(r.status, Status::Converged);
}

#[test]
fn planted_iv_silent_skip() {
    if std::env::var("CI").is_ok() {
        return;
    }
    assert_eq!(compute(2), 4);
}

#[test]
fn planted_v_tautologies() {
    let y = compute(2);
    assert!(true);
    assert_eq!(y, y);
}

#[test]
fn clean_checks_the_computed_value_through_a_helper() {
    let y = compute(2);
    // a comment with assert!(true) and "return;" must not count
    let s = "return; assert!(true)";
    check_four(y, s);
    if y == 4 {
        assert_ne!(compute(3), y);
    }
}

fn check_four(y: i32, _s: &str) {
    assert_eq!(y, 4, "{}", "x");
}
'''


class Planted(unittest.TestCase):
    def test_every_class_found_and_clean_not_flagged(self):
        hits = scan("planted.rs", PLANTED, checking_fns([PLANTED]))
        lines = PLANTED.splitlines()
        found = {}
        for _, ln, cls, text in hits:
            fn = next(lines[k] for k in range(ln - 1, -1, -1) if lines[k].startswith("fn "))
            found.setdefault(cls, set()).add(fn.split("(")[0][3:])
            print(f"{ln}: ({cls}) {text}")
        self.assertEqual(found.get("i"), {"planted_i_sleep_always_exceeds_its_budget", "planted_i_literals_only"})
        self.assertEqual(found.get("ii"), {"planted_ii_no_check"})
        self.assertEqual(found.get("iii"), {"planted_iii_negative_control_expects_success"})
        self.assertEqual(found.get("iv"), {"planted_iv_silent_skip"})
        self.assertEqual(found.get("v"), {"planted_v_tautologies"})
        self.assertEqual(sum(text.startswith("assert") for _, _, cls, text in hits if cls == "v"), 2)
        self.assertFalse(any("clean" in f for fs in found.values() for f in fs))


if __name__ == "__main__":
    unittest.main()
