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
        self.assertEqual(pr.sec_e(self.root, pr.load_adrs(self.root))[1], ["e: 1 ADRs cite from memory without a 'Sources read' addendum, above the recorded 0"])
        # the addendum closes it: the ADR still holds the words, the open count is 0
        adr = [f for f in os.listdir(os.path.join(self.root, "docs", "adr")) if f.startswith("0003")][0]
        with open(os.path.join(self.root, "docs", "adr", adr), "a", encoding="utf-8") as h:
            h.write("\n## Sources read (2026-11-18)\n\n| citation | status |\n|---|---|\n| the value | read: Table 2 |\n")
        self.assertEqual(pr.sec_e(self.root, pr.load_adrs(self.root))[1], [])

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

    def test_a_questions_heading_that_says_none_asks_nothing(self):
        head = "# ADR 000{}: x\n\nStatus: REGISTERED 2026-11-17.\n\n"
        self.put("docs/adr/0002-f003-registration-plates.md",
                 head.format(2) + f"### {pr.Q_HEADING}\n\nNone. Taken on the standing instruction: route (b).\n\n### For the lead\n\nx\n")
        self.put("docs/adr/0003-f003-registration-shells.md",
                 head.format(3) + f"## {pr.Q_HEADING}\n\n1. Nonetheless a question.\n\n## For the lead\n\nNone.\n")
        q = pr.open_questions(self.items, pr.load_adrs(self.root))
        self.assertEqual([w for w, _ in q], ["ADR 0003"])

    def test_the_heading_answer_of_the_maintainer_answers(self):
        self.put("docs/adr/0002-f003-registration-plates.md",
                 f"# ADR 0002: x\n\nStatus: REGISTERED 2026-11-17.\n\n## {pr.Q_HEADING}\n\nQ1.\n\n"
                 "## Answer of the maintainer (2026-11-20), written into this ADR by the lead\n\nA1.\n")
        self.assertEqual(pr.open_questions(self.items, pr.load_adrs(self.root)), [])

    def test_a_dated_entry_of_the_maintainer_after_the_marker_answers_it(self):
        self.items[1]["note"] = "NEEDS THE USER: a default change. Maintainer (2026-11-18): approved."
        self.items[3]["note"] = "Maintainer (2026-11-10): go ahead. NEEDS USER: a later question."
        q = pr.open_questions(self.items, pr.load_adrs(self.root))
        self.assertEqual([w for w, _ in q], ["F-004"])

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

    def test_a_brief_whose_branch_is_one_of_several_in_a_merge_line_is_merged(self):
        # ADR 0752 (F-851, point 4): push_main.py writes one line for several merges checked as one joint diff
        head = "Branch: {0}\nItems: F-003\nADR: 0002\nRules version: 1\nModel: m\nIssued: 2026-11-18\n"
        for name in ("one", "two", "three"):
            self.put(f"{pr.BRIEFS}/{name}.md", head.format(name))
        self.put(pr.LEDGER, "date\tkind\tcommit\tbranch\titems\tadrs\tresult\tminutes\tleft_out\tleft_out_minutes\tdetail\n"
                 "2026-11-21\tmerge\tabc\ttwo,three\tF-003\t0002\tpass\t1.0\t0\t0.0\texit=0;mutations=0;joint=def;cmds=x\n")
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

    def test_a_qualified_kind_passes_and_a_listed_row_is_passed_over(self):
        reg = self.REG.replace("| 10 | code verification |", "| 10 | code verification (negative control) |")
        self.put("docs/adr/0005-f004-registration-x.md", reg + "| R9 reading | stated | the brief | stated | - | reading, no run |\n")
        fail = pr.sec_r(self.root, pr.load_adrs(self.root))[1]
        self.assertEqual(len(fail), 1)
        self.assertIn("kind 'reading, no run' is not one of the four", fail[0])
        self.put("docs/process/exceptions/registration-kind.txt", "# reason\n0005-f004-registration-x.md: R9 reading\n")
        self.assertEqual(pr.sec_r(self.root, pr.load_adrs(self.root))[1], [])
        self.put("docs/adr/0005-f004-registration-x.md", reg.replace("(negative control)", "[negative control]"))
        self.assertEqual(pr.sec_r(self.root, pr.load_adrs(self.root))[1], [])

    def test_an_amendment_gives_the_kinds_of_a_table_without_the_column(self):
        reg = (self.REG.replace("| Margin | Kind |", "| Margin |").replace("|---|---|---|---|---|---|", "|---|---|---|---|---|")
               .replace(" | solution verification |", " |").replace(" | code verification |", " |"))
        self.put("docs/adr/0005-f004-registration-x.md", reg)
        fail = pr.sec_r(self.root, pr.load_adrs(self.root))[1]
        self.assertEqual(len(fail), 1)
        self.assertIn('no column "kind"', fail[0])
        kinds = "\n## Amendment 1\n\n| Id | Kind |\n|---|---|\n| C1 order | solution verification |\n"
        self.put("docs/adr/0005-f004-registration-x.md", reg + kinds)
        fail = pr.sec_r(self.root, pr.load_adrs(self.root))[1]
        self.assertEqual(len(fail), 1)
        self.assertIn("kind '' is not one of the four", fail[0])
        self.assertIn("C2 drift", fail[0])
        self.put("docs/adr/0005-f004-registration-x.md", reg + kinds + "| C2 drift | code verification |\n")
        self.assertEqual(pr.sec_r(self.root, pr.load_adrs(self.root))[1], [])
        self.put("docs/adr/0005-f004-registration-x.md", reg + kinds + "| C2 drift | a check |\n")
        self.assertEqual(len(pr.sec_r(self.root, pr.load_adrs(self.root))[1]), 1)

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


class Convention(Repo):
    """ADR 0777 (F-861): four rules that rested on convention, each through `--check`. The registered values are written
    out here: the date 2026-11-24, 1200 characters, 194023 bytes, the last item 887 and the last ADR 0779 before the date,
    and 25 entries as the largest number of the list of Rust test files."""

    BRIEF = ("Branch: b\nItems: F-003\nADR: 0002\nRules version: 5\nModel: m\nIssued: 2026-11-24\n"
             "Size (lead): done; 0; 0; 0; {total}\n\n## Reading list\n\n{entries}\n"
             "Also in the repository, as needed: `docs/x.md` (its first section).\n\n## Task\n\nDo it.\n")
    ADR = "# ADR {n}: F-004 registration: sst\n\nStatus: **REGISTERED {date}, before any code.**\n\n{body}\n"

    def check(self, *more):
        """Commit the tree, regenerate, and return (exit status, the failures `--check` names)."""
        for args in (["init", "-q"], ["add", "-A"], ["-c", "user.name=t", "-c", "user.email=t@example.invalid",
                                                      "-c", "commit.gpgsign=false", "commit", "-q", "--allow-empty", "-m", "next"]):
            subprocess.run(["git", "-C", self.root, *args], check=True, capture_output=True)
        self.run_main("--write")
        code, out = self.run_main("--check", *more)
        return code, [ln[5:] for ln in out.splitlines() if ln.startswith("FAIL ")]

    def one_failure(self, *words, more=()):
        code, fail = self.check(*more)
        self.assertEqual((code, len(fail)), (1, 1), fail)
        for w in words:
            self.assertIn(w, fail[0])

    def note(self, n, text):
        self.items[n]["note"] = text
        self.save()

    def test_a_note_entry_from_the_date_is_one_line_in_the_form(self):
        good = "ADR 0002, 2026-11-24, done: the plates pass."
        lead = "Lead (2026-11-24): "
        self.note(0, "Lead (2026-11-20): entered. " + good)
        self.note(1, lead + "x" * (1200 - len(lead)) + " " + good)
        self.note(2, "ADR 0003, 2026-11-23, merged and " + "x" * 3000)
        self.assertEqual(self.check(), (0, []))
        self.note(3, "ADR 0002, 2026-11-24, merged and fine.")
        self.one_failure("n: ", "F-004", "outcome", "ADR 0002, 2026-11-24")
        head = "ADR 0002, 2026-11-24, partial: "
        self.note(3, good + " " + head + "x" * (1200 - len(head)))
        self.assertEqual(self.check(), (0, []))
        self.note(3, good + " " + head + "x" * (1201 - len(head)))
        self.one_failure("n: ", "F-004", "1201")
        self.note(3, lead + "x" * (1201 - len(lead)))
        self.one_failure("n: ", "F-004", "1201", "Lead (2026-11-24)")
        self.note(3, "ADR 0002, 2026-11-24, done: the plates\npass.")
        self.one_failure("n: ", "F-004", "line break")
        self.note(3, "")
        self.items.append(item(888, "todo", note="entered for the shells, without a head"))
        self.save()
        self.one_failure("n: ", "F-888", "head")
        self.note(4, "Lead (2026-11-25): entered for the shells.")
        self.assertEqual(self.check(), (0, []))
        self.note(4, "")
        self.assertEqual(self.check(), (0, []))

    def test_a_citation_from_the_date_names_a_place(self):
        new = "docs/adr/0006-f004-registration-sst.md"

        def adr(body, date="2026-11-24", rel=new, n="0006"):
            self.put(rel, self.ADR.format(n=n, date=date, body=body))

        adr("The model is that of Menter (1994), Table 2, and of (Wilcox et al., 2008, p. 12).")
        self.assertEqual(self.check(), (0, []))
        adr("Menter (1994) gives the constants.")
        self.one_failure("s: ", "0006-f004-registration-sst.md", "Menter 1994")
        adr("Menter (1994) gives the constants.\n\n## Addendum 2\n\n- Menter (1994): the constants are in equation (A1).")
        self.assertEqual(self.check(), (0, []))
        adr("Menter (1994) was not read; the constants are derived below.")
        self.assertEqual(self.check(), (0, []))
        adr("The limiter of Spalart and Allmaras (1992) is used.\n\nTable 3 of this ADR lists the runs.")
        self.one_failure("s: ", "Spalart 1992")
        adr("The constants are the usual ones (Menter, 1994).")
        self.one_failure("s: ", "Menter 1994")
        adr("The drag is that of arXiv:2303.09262v1 for the cylinder.")
        self.one_failure("s: ", "arXiv:2303.09262")
        adr("| drag 1.359 | arXiv:2303.09262v1, Table 1 |\n| lift | the same |")
        self.assertEqual(self.check(), (0, []))
        adr("Lead (2026) and March (2023) and Version (2024) name no source; nor does Lead (2026-11-24).")
        self.assertEqual(self.check(), (0, []))
        adr("Menter (1994) gives the constants.", date="2026-11-23")
        self.assertEqual(self.check(), (0, []))
        os.remove(os.path.join(self.root, new))
        self.put("docs/adr/0779-f004-registration-sst.md", "# ADR 0779: x\n\nStatus: registered.\n\nMenter (1994) gives it.\n")
        self.assertEqual(self.check(), (0, []))
        self.put("docs/adr/0780-f004-registration-sst.md", "# ADR 0780: x\n\nStatus: registered.\n\nMenter (1994) gives it.\n")
        self.one_failure("s: ", "0780-f004-registration-sst.md", "Menter 1994")

    def test_a_brief_reading_list_has_its_sum_and_its_limit(self):
        three = "- docs/a.md 100000 (whole)\n- scripts/b.py 94000\n- docs/c.md {}\n"

        def brief(total, entries, name="b"):
            self.put(f"{pr.BRIEFS}/{name}.md", self.BRIEF.format(total=total, entries=entries))

        brief(194023, three.format(23))
        self.assertEqual(self.check(), (0, []))
        brief(194022, three.format(23))
        self.one_failure("u: ", "b.md", "194022", "194023")
        brief(194024, three.format(24))
        self.one_failure("u: ", "b.md", "194024", "limit")
        self.put(f"{pr.EXC_DIR}/brief-over-reading-limit.txt", "# above the limit when the check started\nb.md\n")
        self.assertEqual(self.check(), (0, []))
        os.remove(os.path.join(self.root, pr.EXC_DIR, "brief-over-reading-limit.txt"))
        brief(194023, three.format(23).replace("scripts/b.py 94000", "scripts/b.py"))
        self.one_failure("u: ", "b.md", "scripts/b.py", "size")
        brief(0, "")
        self.one_failure("u: ", "b.md", "no reading list entry")
        brief("many", three.format(23))
        self.one_failure("u: ", "b.md", "total")

    def test_a_listed_size_is_the_size_of_the_file_at_issue(self):
        self.put("docs/a.md", "ten bytes\n")
        self.put(f"{pr.BRIEFS}/b.md", self.BRIEF.format(total=10, entries="- docs/a.md 10\n"))
        self.put(f"{pr.BRIEFS}/c.md", self.BRIEF.format(total=12, entries="- docs/a.md 12\n"))
        self.assertEqual(self.check(), (0, []))  # the sum and the total agree; the files are read only with --history
        self.one_failure("u: ", "c.md", "docs/a.md", "12", "10", more=("--history",))
        self.put("docs/a.md", "now it is longer than at issue\n")
        self.put(f"{pr.BRIEFS}/c.md", self.BRIEF.format(total=10, entries="- docs/a.md 10\n- docs/none.md 0\n"))
        self.one_failure("u: ", "c.md", "docs/none.md", more=("--history",))
        self.put(f"{pr.EXC_DIR}/brief-size-differs.txt", "# differed when the check started\nc.md: docs/none.md\n")
        self.assertEqual(self.check("--history"), (0, []))

    def test_a_test_file_without_its_record_fails_without_a_list_and_a_list_cannot_grow(self):
        rust, spec = "crates/a/tests/u.rs", "apps/studio-web/tests/unit/x.spec.ts"
        self.assertEqual(self.check(), (0, []))
        self.put(rust, "// no record named here\n#[test]\nfn u() {}\n")
        self.one_failure("l: ", rust)
        self.put(rust, "// ADR 0002: plates.\n#[test]\nfn u() {}\n")
        self.put(spec, "// Unit tests of nothing named.\nimport { test } from '@playwright/test';\n")
        self.one_failure("l: ", spec)
        self.put(spec, "// F-999: an item that does not exist.\nimport { test } from '@playwright/test';\n")
        self.one_failure("l: ", spec)
        self.put(spec, "// F-003: the item of this spec.\nimport { test } from '@playwright/test';\n")
        self.assertEqual(self.check(), (0, []))
        files = [f"crates/a/tests/v{n:02d}.rs" for n in range(26)]
        for f in files:
            self.put(f, "// no record named here\n")
        self.put(f"{pr.EXC_DIR}/test-root-without-adr.txt", "# without a record when the rule started\n" + "\n".join(files[:25]) + "\n")
        self.one_failure("l: ", files[25])
        self.put(f"{pr.EXC_DIR}/test-root-without-adr.txt", "# without a record when the rule started\n" + "\n".join(files) + "\n")
        self.one_failure("l: ", "test-root-without-adr.txt", "26", "25")
        os.remove(os.path.join(self.root, files[25]))
        self.put(f"{pr.EXC_DIR}/test-root-without-adr.txt", "# without a record when the rule started\n" + "\n".join(files[:25]) + "\n")
        self.assertEqual(self.check(), (0, []))


import datetime  # noqa: E402

import push_main as pm  # noqa: E402

LEDGER_HEAD = "date\tkind\tcommit\tbranch\titems\tadrs\tresult\tminutes\tleft_out\tleft_out_minutes\tdetail\n"


def line(date, kind, detail, result="pass"):
    return {"date": date, "kind": kind, "commit": "abc", "result": result, "detail": detail}


class Measures(Repo):
    """ADR 0754 (F-863): the four measures of the ledger over the last four weeks."""

    TODAY = datetime.date(2026, 12, 19)
    ROWS = [
        line("2026-12-01", "merge", "exit=0;mutations=0;reported=2026-12-01T10:00+00:00;queue_minutes=30;review_minutes=10;"
             "proposed=done;written=done;cmds=x | y;queue_minutes=9999;review_minutes=9999"),
        line("2026-12-02", "merge", "exit=0;mutations=1;flaky=a::b;queue_minutes=90;review_minutes=20;proposed=done;written=partial;cmds=x"),
        line("2026-12-03", "merge", "exit=0;mutations=0;queue_minutes=600;proposed=done;cmds=x"),
        line("2026-12-04", "merge", "exit=0;mutations=0;cmds=x"),
        line("2026-12-05", "merge", "exit=0;mutations=0;queue_minutes=soon;review_minutes=45;written=todo;cmds=x"),
        line("2026-11-21", "merge", "exit=0;mutations=0;queue_minutes=5000;review_minutes=5000;proposed=done;written=todo;cmds=x"),
        line("2026-12-02", "gate", "passed=10;failed=2;ignored=0;fails=slow::one=0503/F-529;escaped=aaaaaaaaaa=slow::one,bbbbbbbbbb=bt::x;"
             "GitHub run 1", "red"),
        line("2026-12-03", "gate", "passed=10;failed=2;ignored=0;escaped=aaaaaaaaaa=slow::one,aaaaaaaaaa=slow::two", "red"),
        line("2026-12-04", "gate", "passed=10;failed=1;ignored=0;escaped=none", "red"),
        line("2026-12-05", "gate", "passed=10;failed=1;ignored=0;GitHub run 4", "red"),
        line("2026-12-06", "gate", "passed=11;failed=0;ignored=0", "green"),
        line("2026-11-20", "gate", "passed=10;failed=1;ignored=0;escaped=dddddddddd=q::q", "red"),
        line("2026-12-06", "leftout", "ran=3;failed=0;queue_minutes=1", "green"),
    ]

    def measures(self, rows=None, today=None):
        return pr.ledger_measures(self.ROWS if rows is None else rows, today or self.TODAY)

    def test_the_heading_names_the_window_and_its_lines(self):
        self.assertEqual(self.measures()[0], "F-863, the 28 days to 2026-12-19: 5 merge lines, 4 red gate lines")
        self.assertEqual(len(self.measures()), 5)

    def test_queue_age(self):
        self.assertEqual(self.measures()[1], "queue age, minutes from the agent's report to the merge: median 90, largest 600; "
                         "stated by 3 of 5 merge lines")
        self.assertEqual(self.measures(self.ROWS[:2] + self.ROWS[3:4])[1], "queue age, minutes from the agent's report to the "
                         "merge: median 60, largest 90; stated by 2 of 3 merge lines")

    def test_review_minutes(self):
        self.assertEqual(self.measures()[2], "review minutes of the lead: median 20, largest 45; stated by 3 of 5 merge lines")
        rows = [line("2026-12-01", "merge", "exit=0;mutations=0;review_minutes=12.5;cmds=x")]
        self.assertEqual(self.measures(rows)[2], "review minutes of the lead: median 12.5, largest 12.5; stated by 1 of 1 merge lines")

    def test_status_written_against_status_proposed(self):
        self.assertEqual(self.measures()[3], "status written differed from the status proposed: 1 of 2 merges that state both; "
                         "stated by 2 of 5 merge lines")
        rows = [line("2026-12-01", "merge", "exit=0;mutations=0;proposed=F-001=done,F-002=done;written=F-002=done,F-001=done;cmds=x")]
        self.assertEqual(self.measures(rows)[3], "status written differed from the status proposed: 0 of 1 merges that state both; "
                         "stated by 1 of 1 merge lines")

    def test_escaped_defects(self):
        self.assertEqual(self.measures()[4], "escaped defects: 3 (a commit and a test), brought in by 2 commits; merges in the "
                         "window: 5; stated by 3 of 4 red gate lines")
        self.assertEqual(self.measures(self.ROWS[:5] + self.ROWS[8:10])[4], "escaped defects: 0 (a commit and a test), brought in "
                         "by 0 commits; merges in the window: 5; stated by 1 of 2 red gate lines")

    def test_a_measure_no_line_states_is_not_stated_and_never_zero(self):
        rows = [line("2026-12-04", "merge", "exit=0;mutations=0;cmds=x"), line("2026-12-05", "merge", "exit=0;mutations=0;cmds=y"),
                line("2026-12-05", "gate", "passed=10;failed=1;ignored=0", "red")]
        self.assertEqual(self.measures(rows), [
            "F-863, the 28 days to 2026-12-19: 2 merge lines, 1 red gate lines",
            "queue age, minutes from the agent's report to the merge: not stated by any of 2 merge lines",
            "review minutes of the lead: not stated by any of 2 merge lines",
            "status written differed from the status proposed: not stated by any of 2 merge lines",
            "escaped defects: not stated by any of 1 red gate lines"])
        self.assertEqual(self.measures([])[1:], [
            "queue age, minutes from the agent's report to the merge: not stated by any of 0 merge lines",
            "review minutes of the lead: not stated by any of 0 merge lines",
            "status written differed from the status proposed: not stated by any of 0 merge lines",
            "escaped defects: not stated by any of 0 red gate lines"])

    def test_the_window_is_the_28_days_that_end_today(self):
        rows = [line("2026-11-21", "merge", "exit=0;mutations=0;queue_minutes=5;cmds=x"),
                line("2026-11-22", "merge", "exit=0;mutations=0;queue_minutes=7;cmds=x"),
                line("2026-12-19", "merge", "exit=0;mutations=0;queue_minutes=9;cmds=x"),
                line("2026-12-20", "merge", "exit=0;mutations=0;queue_minutes=11;cmds=x"),
                line("not a date", "merge", "exit=0;mutations=0;queue_minutes=13;cmds=x")]
        self.assertEqual(self.measures(rows)[:2], ["F-863, the 28 days to 2026-12-19: 2 merge lines, 0 red gate lines",
                         "queue age, minutes from the agent's report to the merge: median 8, largest 9; stated by 2 of 2 merge lines"])
        self.assertIn("6 merge lines", self.measures(self.ROWS, datetime.date(2026, 12, 18))[0])

    def test_lines_written_by_push_main_are_read_back(self):
        merge = datetime.datetime(2026, 12, 1, 11, 30, tzinfo=datetime.timezone.utc).timestamp()
        entries = pm.measure_entries(reported="2026-12-01T10:00+00:00", review_minutes="25", proposed="done", written="partial",
                                     merge_epoch=merge)
        path = os.path.join(self.root, pr.LEDGER)
        pm.append_ledger(path, pm.ledger_line(date="2026-12-01", kind="merge", commit="abc", result="pass",
                                              detail="exit=0;mutations=0;" + ";".join(entries) + ";cmds=x | y"))
        pm.append_ledger(path, pm.ledger_line(date="2026-12-01", kind="merge", commit="abd", result="pass", detail="exit=0;mutations=0;cmds=x"))
        escaped = pm.escaped_entry(["abc=slow::one"], lambda rev: "a" * 40)
        pm.append_ledger(path, pm.ledger_line(date="2026-12-02", kind="gate", commit="abe", result="red",
                                              detail=pm.gate_detail(10, 1, 0, note="GitHub run 1", escaped=escaped)))
        self.assertEqual(pr.ledger_measures(pr.load_ledger(self.root), self.TODAY), [
            "F-863, the 28 days to 2026-12-19: 2 merge lines, 1 red gate lines",
            "queue age, minutes from the agent's report to the merge: median 90, largest 90; stated by 1 of 2 merge lines",
            "review minutes of the lead: median 25, largest 25; stated by 1 of 2 merge lines",
            "status written differed from the status proposed: 1 of 1 merges that state both; stated by 1 of 2 merge lines",
            "escaped defects: 1 (a commit and a test), brought in by 1 commits; merges in the window: 2; stated by 1 of 1 red gate lines"])

    def test_the_weekly_list_prints_them_under_process_measures(self):
        today = datetime.date.today()
        day = (today - datetime.timedelta(days=1)).isoformat()
        old = (today - datetime.timedelta(days=40)).isoformat()
        self.put(pr.LEDGER, LEDGER_HEAD
                 + f"{old}\tmerge\taaa\told\tF-003\t0002\tpass\t1.0\t0\t0.0\texit=0;mutations=0;queue_minutes=4000;cmds=x\n"
                 + f"{day}\tmerge\tabc\tone\tF-003\t0002\tpass\t1.0\t0\t0.0\texit=0;mutations=0;queue_minutes=45;review_minutes=15;"
                   "proposed=done;written=done;cmds=x\n"
                 + f"{day}\tgate\tabd\t\t\t\tred\t\t\t\tpassed=1;failed=1;ignored=0;escaped=abc=t::one\n")
        self.git_init()
        code, out = self.run_main("--weekly")
        self.assertEqual(code, 0)
        tail = [ln.strip() for ln in out.split("\nProcess measures\n", 1)[1].splitlines()]
        want = [f"F-863, the 28 days to {today.isoformat()}: 1 merge lines, 1 red gate lines",
                "queue age, minutes from the agent's report to the merge: median 45, largest 45; stated by 1 of 1 merge lines",
                "review minutes of the lead: median 15, largest 15; stated by 1 of 1 merge lines",
                "status written differed from the status proposed: 0 of 1 merges that state both; stated by 1 of 1 merge lines",
                "escaped defects: 1 (a commit and a test), brought in by 1 commits; merges in the window: 1; stated by 1 of 1 red gate lines"]
        self.assertEqual([ln for ln in tail if ln in want], want)
        code, out = self.run_main("--sections", "m")
        self.assertIn(want[1], out)


if __name__ == "__main__":
    unittest.main()
