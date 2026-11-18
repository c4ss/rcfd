"""Self-test of scripts/render_roadmap.py on a small roadmap in a temporary folder.

    python -m unittest scripts/test_render_roadmap.py
"""
import contextlib
import io
import json
import os
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import render_roadmap as rr  # noqa: E402

HEAD = "# rcfd roadmap\n\nCurrent priority (set by the maintainer, 2026-11-18): the fan.\n\n"
HAND = HEAD + """## Phase 0 - Workspace and architecture

- [x] **F-001** One
  - depends on: none
  - acceptance: error below `1e-3` on the
    refined mesh
  - note: ADR 0002, 2026-11-17: done.
    - a sub-bullet of the note
- [~] **F-002** Two
  - depends on: F-001
  - acceptance: runs

## Additions

A paragraph between items.

- [ ] **F-003** Three
  - depends on: F-001, F-002
  - acceptance: exists
"""


def items():
    return [
        {"id": "F-001", "title": "One", "phase": 0, "deps": [], "status": "done",
         "acceptance": "error below 1e-3 on the refined mesh", "note": "ADR 0002, 2026-11-17: done. a sub-bullet of the note"},
        {"id": "F-002", "title": "Two", "phase": 0, "deps": ["F-001"], "status": "partial", "acceptance": "runs", "note": "",
         "gap": "the second case"},
        {"id": "F-003", "title": "Three", "phase": 9, "deps": ["F-001", "F-002"], "status": "todo", "acceptance": "exists",
         "note": ""},
    ]


class Pure(unittest.TestCase):
    def test_schema_accepts_the_sample_and_names_each_fault(self):
        self.assertEqual(rr.schema_problems(items()), [])
        bad = items()
        bad[0]["gap"] = "stray"
        bad[1]["gap"] = "x" * 501
        bad[2]["deps"] = ["F-009"]
        bad[2]["colour"] = "red"
        bad.append(dict(bad[0], status="finished"))
        got = rr.schema_problems(bad)
        for want in ("F-001: gap on an item that is not partial", "F-002: gap must be one line of 1 to 500 characters",
                     "F-003: dep F-009 is not an item", "F-003: unknown key ['colour']", "F-001: id occurs twice",
                     "F-001: status 'finished'"):
            self.assertIn(want, got)

    def test_rendered_items_by_phase_with_gap_before_note(self):
        text = rr.render_items(items())
        self.assertTrue(text.startswith(rr.MARKER + "\n\n## Phase 0 - Workspace and architecture\n\n2 items: 1 done, 1 partly done, 0 not started.\n"))
        self.assertIn("- [~] **F-002** Two\n  - depends on: F-001\n  - acceptance: runs\n  - gap: the second case\n", text)
        self.assertIn("## Phase 9 - Advanced physics and scale (stretch)", text)
        self.assertLess(text.index("F-001"), text.index("F-002"))

    def test_the_hand_kept_page_agrees_with_the_json_apart_from_layout(self):
        self.assertEqual(rr.diff(HAND, items()), [])

    def test_diff_names_lost_text_status_duplicate_and_missing_ids(self):
        its = items()
        its[0]["note"] = "ADR 0002, 2026-11-17: done."
        its[1]["status"] = "done"
        its.append({"id": "F-004", "title": "Four", "phase": 0, "deps": [], "status": "todo", "acceptance": "x", "note": ""})
        page = HAND + "- [ ] **F-003** Three\n  - depends on: F-001, F-002\n  - acceptance: exists\n"
        got = {(k, i) for k, i, _ in rr.diff(page, its)}
        self.assertEqual(got, {("text", "F-001"), ("status", "F-002"), ("duplicate", "F-003"), ("id", "F-004")})

    def test_generated_page_reads_back_equal(self):
        page = HEAD + rr.render_items(items())
        self.assertEqual(rr.render_page(page, items()), page)
        self.assertEqual({k: len(v) for k, v in rr.page_items(page).items()}, {"F-001": 1, "F-002": 1, "F-003": 1})


class Command(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = self.tmp.name
        os.makedirs(os.path.join(self.root, "docs"))
        self.write(rr.JSON, json.dumps(items()))
        self.write(rr.PAGE, HAND)

    def tearDown(self):
        self.tmp.cleanup()

    def write(self, rel, text):
        with open(os.path.join(self.root, rel), "w", encoding="utf-8", newline="\n") as fh:
            fh.write(text)

    def page(self):
        with open(os.path.join(self.root, rr.PAGE), encoding="utf-8") as fh:
            return fh.read()

    def run_main(self, *args):
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            code = rr.main(["--root", self.root, *args])
        return code, buf.getvalue()

    def test_before_adoption_only_the_schema_is_checked_and_nothing_is_written(self):
        self.assertEqual(self.run_main("--check")[0], 0)
        self.assertEqual(self.run_main()[0], 1)
        self.assertEqual(self.page(), HAND)

    def test_adopt_keeps_the_head_then_check_follows_the_json(self):
        self.assertEqual(self.run_main("--adopt")[0], 0)
        page = self.page()
        self.assertTrue(page.startswith(HEAD + rr.MARKER))
        self.assertNotIn("A paragraph between items.", page)
        self.assertEqual(self.run_main("--check")[0], 0)
        its = items()
        its[2]["status"] = "done"
        self.write(rr.JSON, json.dumps(its))
        self.assertEqual(self.run_main("--check")[0], 1)
        self.assertEqual(self.run_main()[0], 0)
        self.assertIn("- [x] **F-003** Three", self.page())
        self.assertEqual(self.run_main("--check")[0], 0)

    def test_adopt_refuses_to_lose_text_unless_told(self):
        self.write(rr.PAGE, HAND.replace("ADR 0002, 2026-11-17: done.", "ADR 0002, 2026-11-17: done, and more on the page."))
        code, out = self.run_main("--adopt")
        self.assertEqual((code, self.page().count(rr.MARKER)), (1, 0))
        self.assertIn("1 difference(s) would be lost", out)
        self.assertEqual(self.run_main("--adopt", "--json-wins")[0], 0)
        self.assertNotIn("and more on the page", self.page())

    def test_a_schema_fault_fails_the_check(self):
        its = items()
        its[0]["status"] = "finished"
        self.write(rr.JSON, json.dumps(its))
        code, out = self.run_main("--check")
        self.assertEqual(code, 1)
        self.assertIn("schema: F-001: status 'finished'", out)


if __name__ == "__main__":
    unittest.main()
