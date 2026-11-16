"""Tests of scripts/adr_test_index.py (ADR 0653 T1). Run: python -m unittest scripts/test_adr_test_index.py -v
A small fake workspace (two crates, ADRs, a roadmap) is written to a temp folder; the metadata is given, so cargo is not run."""
import json
import os
import shutil
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import adr_test_index as ati  # noqa: E402

FILES = {
    "crates/rcfd-a/src/lib.rs": """
//! crate a; a stray brace in a comment: }
pub mod solver;
pub fn f() -> &'static str { "}{ not code" }
/* block comment { with a brace /* nested } */ still comment } */
#[cfg(test)]
mod tests {
    #[test]
    fn lib_top() { let c = '}'; let _ = c; }
    mod deeper {
        #[test]
        #[ignore = "heavy { record"]
        fn lib_deep() {}
    }
}
""",
    "crates/rcfd-a/src/solver.rs": """
pub fn solve<'a>(x: &'a str) -> &'a str { x }
#[cfg(test)]
mod tests {
    #[ignore]
    #[test]
    fn solver_case() { let s = r#"raw } string"#; let _ = s; }
}
""",
    "crates/rcfd-a/tests/alpha_f010.rs": """
#[test]
fn alpha_one() {}
mod m {
    #[test]
    fn inner() {}
    fn not_a_test() {}
}
""",
    "crates/rcfd-a/tests/beta.rs": """
#[path = "shared/helper.rs"]
mod helper;
#[test]
fn beta_one() {}
""",
    "crates/rcfd-a/tests/shared/helper.rs": """
#[test]
fn from_helper() {}
""",
    "crates/rcfd-a/tests/gamma.rs": """
#[test]
fn gamma_distinctive_test() {}
#[test]
fn quick() {}
""",
    "crates/rcfd-b/src/lib.rs": """
#[cfg(test)]
mod tests {
    #[test]
    fn b_unit() {}
}
""",
    "crates/rcfd-b/tests/delta.rs": """
#[test]
fn delta_one() {}
""",
    "crates/rcfd-b/tests/eps.rs": """
#[test]
fn quick() {}
""",
    "docs/adr/0100-f010-registration-alpha.md": "# ADR 0100\nThe tests are in `crates/rcfd-a/tests/beta.rs`; see "
                                                "`gamma_distinctive_test` and `quick`.\n",
    "docs/adr/0101-solver-notes.md": "# ADR 0101\nThe function `rcfd_a::solver::solve` is changed.\n",
    "docs/adr/0102-f020-registration-delta.md": "# ADR 0102\nNothing cited here.\n",
    "docs/adr/0103-f030-f031-registration-two-items.md": "# ADR 0103\n",
    "docs/adr/README.md": "not an ADR\n",
    "docs/roadmap.json": json.dumps([
        {"id": "F-010", "note": "DONE. ADR 0100, 2026-11-12."},
        {"id": "F-020", "note": "PARTIAL. Tests in delta.rs (ADR 0102)."},
        {"id": "F-040", "note": "Covers ADRs 0104-0106."},
    ]),
}


def meta(root):
    def tgt(name, kind, path):
        return {"name": name, "kind": [kind], "src_path": os.path.join(root, *path.split("/")), "test": True}

    a = os.path.join(root, "crates", "rcfd-a")
    b = os.path.join(root, "crates", "rcfd-b")
    return {"workspace_root": root, "packages": [
        {"name": "rcfd-a", "manifest_path": os.path.join(a, "Cargo.toml"),
         "dependencies": [{"name": "rcfd-b", "path": b}, {"name": "serde"}],
         "targets": [tgt("rcfd_a", "lib", "crates/rcfd-a/src/lib.rs"), tgt("alpha_f010", "test", "crates/rcfd-a/tests/alpha_f010.rs"),
                     tgt("beta", "test", "crates/rcfd-a/tests/beta.rs"), tgt("gamma", "test", "crates/rcfd-a/tests/gamma.rs")]},
        {"name": "rcfd-b", "manifest_path": os.path.join(b, "Cargo.toml"), "dependencies": [],
         "targets": [tgt("rcfd_b", "lib", "crates/rcfd-b/src/lib.rs"), tgt("delta", "test", "crates/rcfd-b/tests/delta.rs"),
                     tgt("eps", "test", "crates/rcfd-b/tests/eps.rs")]},
    ]}


class Index(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.root = tempfile.mkdtemp(prefix="adr_index_")
        for p, text in FILES.items():
            f = os.path.join(cls.root, *p.split("/"))
            os.makedirs(os.path.dirname(f), exist_ok=True)
            with open(f, "w", encoding="utf-8") as h:
                h.write(text)
        cls.idx = ati.build(cls.root, meta(cls.root))
        cls.t = {(t["crate"], t["binary"], t["name"]): t for t in cls.idx["tests"]}

    @classmethod
    def tearDownClass(cls):
        shutil.rmtree(cls.root, ignore_errors=True)

    def test_enumeration_names_and_ignored(self):
        want = {
            ("rcfd-a", "rcfd_a", "tests::lib_top"): False,
            ("rcfd-a", "rcfd_a", "tests::deeper::lib_deep"): True,
            ("rcfd-a", "rcfd_a", "solver::tests::solver_case"): True,
            ("rcfd-a", "alpha_f010", "alpha_one"): False,
            ("rcfd-a", "alpha_f010", "m::inner"): False,
            ("rcfd-a", "beta", "beta_one"): False,
            ("rcfd-a", "beta", "helper::from_helper"): False,
            ("rcfd-a", "gamma", "gamma_distinctive_test"): False,
            ("rcfd-a", "gamma", "quick"): False,
            ("rcfd-b", "rcfd_b", "tests::b_unit"): False,
            ("rcfd-b", "delta", "delta_one"): False,
            ("rcfd-b", "eps", "quick"): False,
        }
        self.assertEqual({k: v["ignored"] for k, v in self.t.items()}, want)
        self.assertEqual(self.t[("rcfd-a", "beta", "helper::from_helper")]["file"], "crates/rcfd-a/tests/shared/helper.rs")
        self.assertEqual(self.t[("rcfd-a", "rcfd_a", "solver::tests::solver_case")]["file"], "crates/rcfd-a/src/solver.rs")

    def adrs(self, *k):
        return self.t[k]["adrs"]

    def kinds(self, *k):
        return sorted({e["kind"] for e in self.t[k]["evidence"]})

    def test_e1_item_in_file_name(self):
        self.assertEqual(self.adrs("rcfd-a", "alpha_f010", "m::inner"), ["0100"])
        self.assertEqual(self.kinds("rcfd-a", "alpha_f010", "m::inner"), ["E1"])
        self.assertEqual(self.t[("rcfd-a", "alpha_f010", "alpha_one")]["items"], ["F-010"])

    def test_item_regex(self):
        self.assertEqual(ati.items_in_stem("alpha_f010"), ["F-010"])
        self.assertEqual(ati.items_in_stem("f249_moo"), ["F-249"])
        self.assertEqual(ati.items_in_stem("x_f12_y_f700_tests"), ["F-012", "F-700"])
        self.assertEqual(ati.items_in_stem("fan01_hub"), [])
        self.assertEqual(ati.items_in_stem("buf123"), [])
        self.assertEqual(ati.items_in_stem("aoa_sweep_w48y"), [])

    def test_e2_adr_cites_file_including_its_modules(self):
        self.assertEqual(self.adrs("rcfd-a", "beta", "beta_one"), ["0100"])
        self.assertEqual(self.kinds("rcfd-a", "beta", "beta_one"), ["E2"])
        # a module file of the target is a different file: not cited
        self.assertEqual(self.adrs("rcfd-a", "beta", "helper::from_helper"), [])

    def test_e3_distinctive_function_only(self):
        self.assertEqual(self.adrs("rcfd-a", "gamma", "gamma_distinctive_test"), ["0100"])
        self.assertEqual(self.kinds("rcfd-a", "gamma", "gamma_distinctive_test"), ["E3"])
        self.assertEqual(self.adrs("rcfd-a", "gamma", "quick"), [])  # short and in two binaries

    def test_e4_roadmap_note(self):
        self.assertEqual(self.adrs("rcfd-b", "delta", "delta_one"), ["0102"])
        self.assertEqual(self.kinds("rcfd-b", "delta", "delta_one"), ["E4"])
        self.assertEqual(self.t[("rcfd-b", "delta", "delta_one")]["items"], ["F-020"])

    def test_e5_module_path_deepest(self):
        self.assertEqual(self.adrs("rcfd-a", "rcfd_a", "solver::tests::solver_case"), ["0101"])
        self.assertEqual(self.kinds("rcfd-a", "rcfd_a", "solver::tests::solver_case"), ["E5"])
        self.assertEqual(self.adrs("rcfd-a", "rcfd_a", "tests::lib_top"), [])

    def test_unmapped(self):
        self.assertEqual(self.adrs("rcfd-b", "eps", "quick"), [])
        self.assertEqual(self.adrs("rcfd-b", "rcfd_b", "tests::b_unit"), [])

    def test_adr_tables(self):
        self.assertEqual(self.idx["items"]["F-010"], ["0100"])
        self.assertEqual(self.idx["items"]["F-030"], ["0103"])
        self.assertEqual(self.idx["items"]["F-031"], ["0103"])
        self.assertNotIn("F-040", self.idx["items"])  # ADRs 0104-0106 do not exist
        self.assertEqual(ati.adr_refs("ADRs 0104-0106 and ADR 0200"), {"0104", "0105", "0106", "0200"})
        self.assertEqual(self.idx["adrs"]["0100"]["items"], ["F-010"])
        self.assertIn("rcfd-a/gamma::gamma_distinctive_test", self.idx["adrs"]["0100"]["tests"])
        self.assertNotIn("README", json.dumps(self.idx["adrs"]))
        self.assertEqual(self.idx["crates"]["rcfd-a"], {"dir": "crates/rcfd-a", "deps": ["rcfd-b"]})

    def test_coverage(self):
        c = self.idx["coverage"]
        self.assertEqual(c["integration_binaries"], {"binaries": 5, "mapped": 4, "share": 0.8})
        self.assertEqual(c["all_tests"]["tests"], 12)
        self.assertEqual(c["all_tests"]["mapped"], 6)  # alpha x2, beta_one, gamma_distinctive, delta, solver_case
        self.assertEqual(c["non_ignored_tests"]["tests"], 10)
        self.assertIn("rcfd-b/eps (test, 1 tests)", c["unmapped_binaries"])
        self.assertIn("rcfd-b/rcfd_b (lib, 1 tests)", c["unmapped_binaries"])

    def test_adr_commands(self):
        self.assertEqual(ati.adr_commands(self.idx, "100"), [
            "cargo test -p rcfd-a --release --test alpha_f010",
            "cargo test -p rcfd-a --release --test beta -- --exact beta_one",
            "cargo test -p rcfd-a --release --test gamma -- --exact gamma_distinctive_test",
        ])
        self.assertEqual(ati.adr_commands(self.idx, "0101"),
                         ["cargo test -p rcfd-a --release --lib -- --exact solver::tests::solver_case --include-ignored"])

    def test_blank_keeps_offsets(self):
        src = 'a "x{" b /* { */ c \'}\' d // }\ne r#"}"# f'
        b = ati.blank(src)
        self.assertEqual(len(b), len(src))
        self.assertNotIn("{", b)
        self.assertNotIn("}", b)
        self.assertEqual(b.split(), ["a", "b", "c", "d", "e", "f"])


if __name__ == "__main__":
    unittest.main()
