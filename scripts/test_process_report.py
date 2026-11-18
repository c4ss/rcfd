"""Self-test of scripts/process_report.py on a small repository built in a temporary folder.

    python -m unittest scripts/test_process_report.py
"""
import contextlib
import io
import json
import os
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import process_report as pr  # noqa: E402

PROGRESS = '''CATEGORIES = [
    ("Solver", "Numerics and solvers", [1, 2]),
    ("Solver", "Structural solver", [3, 4]),
]
'''
ROADMAP = """# roadmap

Current priority (set by the maintainer, 2026-11-18):
1. Structural solver (budget 1 registrations a week)
2. Numerics and solvers

- [x] **F-001** One
- [~] **F-002** Two
- [x] **F-003** Three
- [ ] **F-004** Four
"""


def item(n, status, **kw):
    d = {"id": f"F-{n:03d}", "title": f"Title {n}", "phase": 1, "deps": [], "status": status,
         "acceptance": "error below 1e-3", "note": ""}
    d.update(kw)
    return d


class Repo(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = self.tmp.name
        self.items = [item(1, "done"), item(2, "partial", gap="the second case"), item(3, "done"), item(4, "todo")]
        self.put("scripts/roadmap_progress.py", PROGRESS)
        self.put("docs/ROADMAP.md", ROADMAP)
        self.put("docs/adr/0001-settings.md", "# ADR 0001: Settings\n\nStatus: accepted.\n")
        self.put("docs/adr/0002-f003-registration-plates.md",
                 "# ADR 0002: F-003 registration: plates\n\nStatus: **REGISTERED 2026-11-17, before any code.**\n\n## Addendum 1: results\n")
        self.put("docs/adr/0003-f003-registration-shells.md",
                 "# ADR 0003: F-003 registration: shells\n\nStatus: **REGISTERED 2026-11-18.** A value cited from memory.\n")
        self.put("crates/a/tests/t.rs", "// ADR 0002: plates.\n#[test]\nfn t() {}\n")
        self.save()

    def tearDown(self):
        self.tmp.cleanup()

    def put(self, rel, text):
        p = os.path.join(self.root, rel)
        os.makedirs(os.path.dirname(p), exist_ok=True)
        with open(p, "w", encoding="utf-8", newline="\n") as fh:
            fh.write(text)

    def save(self):
        self.put("docs/roadmap.json", json.dumps(self.items))

    def git_init(self):
        for args in (["init", "-q"], ["add", "-A"],
                     ["-c", "user.name=t", "-c", "user.email=t@example.invalid", "-c", "commit.gpgsign=false", "commit", "-q", "-m", "first"]):
            subprocess.run(["git", "-C", self.root, *args], check=True, capture_output=True)

    def load(self):
        return (pr.load_items(self.root), pr.load_categories(self.root), pr.load_adrs(self.root), pr.roadmap_md(self.root))

    def run_main(self, *args):
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            code = pr.main(["--root", self.root, *args])
        return code, buf.getvalue()


class Inputs(Repo):
    def test_categories_are_read_without_running_the_script(self):
        self.assertEqual(pr.item_category(pr.load_categories(self.root))["F-003"], "Structural solver")

    def test_adr_fields(self):
        a = {x["n"]: x for x in pr.load_adrs(self.root)}
        self.assertEqual((a["0002"]["status"], a["0002"]["date"], a["0002"]["addenda"], a["0002"]["items"], a["0002"]["kind"]),
                         ("REGISTERED", "2026-11-17", 1, ["F-003"], "registration"))
        self.assertEqual((a["0001"]["status"], a["0001"]["date"], a["0001"]["kind"]), ("ACCEPTED", "", ""))

    def test_priority_statement_with_a_budget(self):
        date, lines, stated = pr.priority(self.root, pr.load_categories(self.root))
        self.assertEqual((date, lines, stated), ("2026-11-18", [("Structural solver", 1), ("Numerics and solvers", None)], True))

    def test_priority_statement_in_the_old_form_falls_back_to_the_script_order(self):
        self.put("docs/ROADMAP.md", "Current priority (set by the maintainer, 2026-11-04): F-001 > F-002.\n\n- [x] **F-001** One\n")
        date, lines, stated = pr.priority(self.root, pr.load_categories(self.root))
        self.assertEqual((date, [n for n, _ in lines], stated), ("2026-11-04", ["Numerics and solvers", "Structural solver"], False))


class RoadmapChecks(Repo):
    def test_clean_roadmap_has_no_failure(self):
        items, _, _, md = self.load()
        self.assertEqual(pr.sec_a(self.root, items, md)[1], [])

    def test_gap_rules(self):
        self.items[1].pop("gap")
        self.items[0]["gap"] = "stray"
        self.items[3]["gap"] = "x" * 501
        self.items[3]["status"] = "partial"
        self.put("docs/ROADMAP.md", ROADMAP.replace("- [ ] **F-004**", "- [~] **F-004**"))
        self.save()
        self.put(f"{pr.EXC_DIR}/partial-without-gap.txt", "# none\n")
        items, _, _, md = self.load()
        fail = pr.sec_a(self.root, items, md)[1]
        self.assertEqual(sorted(fail), ["a: gap on an item that is not partial, F-001", "a: gap over 500 characters, F-004",
                                        "a: partial item without gap, F-002"])

    def test_an_exception_silences_a_finding_and_a_stale_one_is_reported(self):
        self.items[1].pop("gap")
        self.save()
        self.put(f"{pr.EXC_DIR}/partial-without-gap.txt", "F-002\nF-004\n")
        items, _, _, md = self.load()
        rep, fail = pr.sec_a(self.root, items, md)
        self.assertEqual(fail, [])
        self.assertTrue(any("no longer apply in partial-without-gap.txt" in r and "F-004" in r for r in rep))

    def test_duplicate_and_differing_status_fail_once_the_rule_has_started(self):
        self.put("docs/ROADMAP.md", ROADMAP.replace("- [x] **F-003** Three", "- [~] **F-003** Three\n- [x] **F-003** Three"))
        self.put(f"{pr.EXC_DIR}/roadmap-duplicates.txt", "")
        self.put(f"{pr.EXC_DIR}/roadmap-status-differs.txt", "")
        items, _, _, md = self.load()
        self.assertEqual(sorted(pr.sec_a(self.root, items, md)[1]), ["a: duplicate id, md F-003", "a: status differs, F-003"])


class Generated(Repo):
    def test_index_rows_and_groups(self):
        items, cats, adrs, _ = self.load()
        text = pr.index_text(adrs, items, cats)
        self.assertIn("## Structural solver (2)", text)
        self.assertIn("| [0002](0002-f003-registration-plates.md) | F-003 registration: plates | REGISTERED | 2026-11-17 | 1 | F-003 | done |", text)
        self.assertIn("## Records with no item in the file name (1)", text)

    def test_block_is_replaced_and_a_page_without_one_is_left(self):
        page = "a\n" + pr.BEGIN.format("built") + "\nold\n" + pr.END.format("built") + "\nz\n"
        self.assertEqual(pr.replace_block(page, "built", "new"), page.replace("old", "new"))
        self.assertIsNone(pr.replace_block("no block", "built", "new"))

    def test_write_then_check_then_drift(self):
        self.put("docs/roadmap-overview.md", "# Overview\n\n" + pr.BEGIN.format("overview") + "\n" + pr.END.format("overview") + "\n")
        self.put("docs/structural-solver.md", "# S\n\n" + pr.BEGIN.format("built") + "\n" + pr.END.format("built") + "\n")
        self.git_init()
        self.assertEqual(self.run_main("--check")[0], 1)  # nothing generated yet
        self.run_main("--write")
        over = pr.read(self.root, "docs/roadmap-overview.md")
        self.assertIn("Of the 4 roadmap items, 2 are done by the registered rule, 1 are partly done and 1 have not started", over)
        self.assertLess(over.index("| Structural solver | 1 | 0 | 1 |"), over.index("| Numerics and solvers | 1 | 1 | 0 |"))
        self.assertIn("- F-003: Title 3", pr.read(self.root, "docs/structural-solver.md"))
        self.assertEqual(self.run_main("--check"), (0, "process report check: 0 failure(s)\n"))
        self.items[3]["status"] = "done"
        self.save()
        self.put("docs/ROADMAP.md", ROADMAP.replace("- [ ] **F-004**", "- [x] **F-004**"))
        code, out = self.run_main("--check")
        self.assertEqual(code, 1)
        self.assertIn("generated file is not current: docs/roadmap-overview.md", out)
        self.assertEqual(self.run_main("--list-generated")[1].split(),
                         ["docs/adr/index.md", "docs/roadmap-overview.md", "docs/structural-solver.md"])


class Lints(Repo):
    def test_conflict_marker_and_test_header(self):
        self.put("docs/x.md", "a\n<<<<<<< HEAD\nb\n")
        self.put("crates/a/tests/u.rs", "// no record named here\n#[test]\nfn u() {}\n")
        self.put("crates/a/tests/common/mod.rs", "// a helper, not a test root\n")
        self.put(f"{pr.EXC_DIR}/test-root-without-adr.txt", "")
        self.git_init()
        items, _, adrs, _ = self.load()
        self.assertEqual(pr.sec_k(self.root)[1], ["k: conflict marker in docs/x.md"])
        self.assertEqual(pr.sec_l(self.root, items, adrs)[1], ["l: test file names no ADR in its leading comment, crates/a/tests/u.rs"])

    def test_new_adr_name_needs_a_kind_word(self):
        self.put(f"{pr.EXC_DIR}/adr-without-kind-word.txt", "0001-settings.md\n")
        self.assertEqual(pr.sec_b(self.root, pr.load_adrs(self.root))[1], [])
        self.put("docs/adr/0004-something.md", "# ADR 0004: Something\n\nStatus: PROPOSED 2026-11-18.\n")
        fail = pr.sec_b(self.root, pr.load_adrs(self.root))[1]
        self.assertEqual(len(fail), 1)
        self.assertTrue(fail[0].endswith("0004-something.md"))

    def test_from_memory_count_may_not_rise(self):
        self.put(f"{pr.EXC_DIR}/from-memory-count.txt", "0\n")
        self.assertEqual(pr.sec_e(self.root, pr.load_adrs(self.root))[1], ["e: 1 ADRs cite from memory, above the recorded 0"])

    def test_registration_lint_from_history(self):
        self.git_init()
        for name in ("adr-rewritten", "registration-not-alone", "registration-after-code"):
            self.put(f"{pr.EXC_DIR}/{name}.txt", "")
        self.put("docs/adr/0002-f003-registration-plates.md", "# ADR 0002: F-003 registration: plates\n\nStatus: rewritten.\n")
        self.git_init()
        fail = pr.sec_c(self.root, pr.load_adrs(self.root))[1]
        self.assertIn("c: ADR changed other than by appending, 0002-f003-registration-plates.md", fail)
        self.assertIn("c: registration not committed alone, 0003-f003-registration-shells.md", fail)


class State(Repo):
    def test_question_headings(self):
        self.put("docs/adr/0002-f003-registration-plates.md",
                 f"# ADR 0002: x\n\nStatus: REGISTERED 2026-11-17.\n\n## {pr.Q_HEADING}\n\nQ1.\n")
        self.put("docs/adr/0003-f003-registration-shells.md",
                 f"# ADR 0003: x\n\nStatus: REGISTERED 2026-11-18.\n\n## {pr.Q_HEADING}\n\nQ1.\n\n## {pr.A_HEADING} (2026-11-18)\n\nA1.\n")
        self.items[1]["note"] = "Open: which default"
        self.items[0]["note"] = "Open: settled, the item is done"
        q = pr.open_questions(self.items, pr.load_adrs(self.root))
        self.assertEqual([w for w, _ in q], ["F-002", "ADR 0002"])

    def test_dispatch_counts_and_budget(self):
        items, cats, adrs, _ = self.load()
        rep, over = pr.sec_g(self.root, items, cats, adrs)
        self.assertEqual(over, [("Structural solver", 2, 1)])
        self.assertTrue(any("F-003 2" in r for r in rep))

    def test_briefs_issued_and_not_merged(self):
        head = "Branch: {0}\nItems: F-003\nADR: 0002\nRules version: 1\nModel: m\nIssued: 2026-11-18\n"
        self.put(f"{pr.BRIEFS}/one.md", head.format("one"))
        self.put(f"{pr.BRIEFS}/two.md", head.format("two"))
        self.put(f"{pr.BRIEFS}/three.md", head.format("three") + "\nClosed without merge (2026-11-18): superseded.\n")
        self.put(f"{pr.BRIEFS}/README.md", "what a brief is\n")
        self.put(pr.LEDGER, "date\tkind\tcommit\tbranch\titems\tadrs\tresult\tminutes\tleft_out\tleft_out_minutes\tdetail\n"
                 "2026-11-18\tmerge\tabc\ttwo\tF-003\t0002\tpass\t1.0\t0\t0.0\texit=0\n")
        self.assertEqual([b["branch"] for b in pr.sec_h(self.root)[1]], ["one"])

    def test_weekly_list(self):
        self.put("docs/ROADMAP.md", ROADMAP.replace("2. Numerics and solvers", "2. Numerics and solvers, F-002 first"))
        self.items[1]["note"] = "NEEDS THE USER: which default"
        self.save()
        self.git_init()
        code, out = self.run_main("--weekly")
        self.assertEqual(code, 0)
        self.assertIn("1. Priority items not done: 1", out)
        self.assertIn("F-002 (partial), has an open question", out)
        self.assertIn("Structural solver: 2 registrations this week, budget 1", out)


class NewRules(Repo):
    REG = ("# ADR 0005: F-004 registration: x\n\nStatus: **REGISTERED 2026-11-19.**\n\n"
           "Rules: docs/process/agent-rules.md version 1. Brief: docs/adr/briefs/f004-work.md.\n\nSize (agent): partial, 1, 0, 0, 40000\n\n"
           "| Check | Limit | Source | Estimate | Margin | Kind |\n|---|---|---|---|---|---|\n"
           "| C1 order | 1.9 | tolerances.md, observed order | 2.0 | 1.05 | solution verification |\n"
           "| C2 drift | 1e-3 | own estimate | 1e-4 | 10 | code verification |\n")

    def test_a_registration_under_the_new_rules_is_read(self):
        self.put("docs/adr/0005-f004-registration-x.md", self.REG)
        rep, fail = pr.sec_r(self.root, pr.load_adrs(self.root))
        self.assertEqual(fail, [])
        text = "\n".join(rep)
        self.assertIn("registrations that cite docs/process/agent-rules.md: 1", text)
        self.assertIn("margin above 3 (reported): 1", text)
        self.assertIn("only source is the agent's estimate (the lead holds the item's code, rule 15): 1", text)

    def test_missing_brief_size_columns_and_a_wrong_kind_fail(self):
        bad = (self.REG.replace(" Brief: docs/adr/briefs/f004-work.md.", "").replace("Size (agent)", "Size")
               .replace("| Margin ", "| Slack ").replace("code verification", "a check"))
        self.put("docs/adr/0005-f004-registration-x.md", bad)
        fail = pr.sec_r(self.root, pr.load_adrs(self.root))[1]
        self.assertEqual(len(fail), 4)
        for want in ("cites no brief file", "states no size", 'no column "margin"', "kind 'a check' is not one of the four"):
            self.assertTrue(any(want in f for f in fail), want)

    def test_older_registrations_are_not_read(self):
        self.assertEqual(pr.sec_r(self.root, pr.load_adrs(self.root))[1], [])

    def test_changelog_count_is_kept_current(self):
        self.put("CHANGELOG.md", "Of the 3 roadmap items, 1 are done, 1 partly done and 1 not started; more.\n")
        items, cats, adrs, _ = self.load()
        self.assertEqual(pr.generated(self.root, items, cats, adrs)["CHANGELOG.md"],
                         "Of the 4 roadmap items, 2 are done, 1 partly done and 1 not started; more.\n")

    def test_an_adopted_roadmap_page_is_generated(self):
        items, cats, adrs, _ = self.load()
        self.assertNotIn("docs/ROADMAP.md", pr.generated(self.root, items, cats, adrs))
        self.put("docs/ROADMAP.md", ROADMAP.split("- [x]")[0] + pr.render_roadmap.MARKER + "\n")
        page = pr.generated(self.root, items, cats, adrs)["docs/ROADMAP.md"]
        self.assertIn("- [~] **F-002** Title 2\n  - depends on: none\n  - acceptance: error below 1e-3\n  - gap: the second case\n", page)
        self.assertEqual([i for i, _ in pr.roadmap_md(self.root)], [])


if __name__ == "__main__":
    unittest.main()
