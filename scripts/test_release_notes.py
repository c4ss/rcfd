"""Self-test of scripts/release_notes.py: python -m unittest scripts/test_release_notes.py"""
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from release_notes import PLUGIN_RCFD, check  # noqa: E402

CARGO = '[workspace]\nmembers = []\n\n[workspace.package]\nversion = "0.2.0"\nedition = "2021"\n'
PY = '"""doc"""\n__version__ = "0.2.0"\n'
PLUGIN = '{"name": "rcfd-panel", "version": "0.2.0"}'
RCFD = '{"name": "rcfd", "version": "0.2.0"}'
LOG = "# Changelog\n\n## Unreleased\n\n### Added\n\n* later\n\n## 0.2.0 - 2026-11-02\n\n### Fixed\n\n* a fix\n\n## 0.1.0 - 2026-10-30\n\n* first\n"


class Check(unittest.TestCase):
    def test_good_tag_gives_only_its_own_section(self):
        notes, errs = check("v0.2.0", CARGO, PY, PLUGIN, LOG, RCFD)
        self.assertEqual(errs, [])
        self.assertEqual(notes, "### Fixed\n\n* a fix\n")

    def test_bad_tag_form(self):
        self.assertEqual(check("0.2.0", CARGO, PY, PLUGIN, LOG, RCFD)[0], None)
        self.assertEqual(check("v0.2", CARGO, PY, PLUGIN, LOG, RCFD)[0], None)

    def test_each_version_mismatch_is_named(self):
        _, errs = check("v0.2.0", CARGO.replace("0.2.0", "0.1.0"), PY.replace("0.2.0", "0.1.0"),
                        PLUGIN.replace("0.2.0", "0.1.0"), LOG, RCFD.replace("0.2.0", "0.1.0"))
        self.assertEqual(len(errs), 4, errs)

    def test_rcfd_plugin_version_alone_is_checked(self):
        # ADR 0135 F12: the plugin of clients/claude-plugin joins the release check
        _, errs = check("v0.2.0", CARGO, PY, PLUGIN, LOG, RCFD.replace("0.2.0", "0.1.9"))
        self.assertEqual(len(errs), 1, errs)
        self.assertIn(PLUGIN_RCFD, errs[0])

    def test_missing_date_or_section_or_empty_body_fails(self):
        self.assertTrue(check("v0.2.0", CARGO, PY, PLUGIN, LOG.replace("0.2.0 - 2026-11-02", "0.2.0 - YYYY-MM-DD"), RCFD)[1])
        self.assertTrue(check("v0.2.0", CARGO, PY, PLUGIN, LOG.replace("## 0.2.0 - 2026-11-02", "## 0.3.0 - 2026-11-02"), RCFD)[1])
        self.assertTrue(check("v0.2.0", CARGO, PY, PLUGIN, LOG.replace("### Fixed\n\n* a fix\n", ""), RCFD)[1])

    def test_repository_manifests_agree(self):
        # the files in the repository: both plugin manifests carry the workspace version
        root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

        def read(p):
            return open(os.path.join(root, p), encoding="utf-8").read()

        import release_notes
        import re
        v = release_notes.workspace_version(read("Cargo.toml"))
        self.assertTrue(re.fullmatch(r"\d+\.\d+\.\d+", v or ""), v)
        _, errs = check("v" + v, read("Cargo.toml"), read("python/rcfd/__init__.py"), read(release_notes.PLUGIN),
                        "## %s - 2026-01-01\n\n* x\n" % v, read(PLUGIN_RCFD))
        self.assertEqual(errs, [])


if __name__ == "__main__":
    unittest.main()
