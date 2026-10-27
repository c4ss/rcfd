"""Self-test of scripts/release_notes.py: python -m unittest scripts/test_release_notes.py"""
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from release_notes import check  # noqa: E402

CARGO = '[workspace]\nmembers = []\n\n[workspace.package]\nversion = "0.2.0"\nedition = "2021"\n'
PY = '"""doc"""\n__version__ = "0.2.0"\n'
PLUGIN = '{"name": "rcfd", "version": "0.2.0"}'
LOG = "# Changelog\n\n## Unreleased\n\n### Added\n\n* later\n\n## 0.2.0 - 2026-11-02\n\n### Fixed\n\n* a fix\n\n## 0.1.0 - 2026-10-30\n\n* first\n"


class Check(unittest.TestCase):
    def test_good_tag_gives_only_its_own_section(self):
        notes, errs = check("v0.2.0", CARGO, PY, PLUGIN, LOG)
        self.assertEqual(errs, [])
        self.assertEqual(notes, "### Fixed\n\n* a fix\n")

    def test_bad_tag_form(self):
        self.assertEqual(check("0.2.0", CARGO, PY, PLUGIN, LOG)[0], None)
        self.assertEqual(check("v0.2", CARGO, PY, PLUGIN, LOG)[0], None)

    def test_each_version_mismatch_is_named(self):
        _, errs = check("v0.2.0", CARGO.replace("0.2.0", "0.1.0"), PY.replace("0.2.0", "0.1.0"),
                        PLUGIN.replace("0.2.0", "0.1.0"), LOG)
        self.assertEqual(len(errs), 3, errs)

    def test_missing_date_or_section_or_empty_body_fails(self):
        self.assertTrue(check("v0.2.0", CARGO, PY, PLUGIN, LOG.replace("0.2.0 - 2026-11-02", "0.2.0 - YYYY-MM-DD"))[1])
        self.assertTrue(check("v0.2.0", CARGO, PY, PLUGIN, LOG.replace("## 0.2.0 - 2026-11-02", "## 0.3.0 - 2026-11-02"))[1])
        self.assertTrue(check("v0.2.0", CARGO, PY, PLUGIN, LOG.replace("### Fixed\n\n* a fix\n", ""))[1])


if __name__ == "__main__":
    unittest.main()
