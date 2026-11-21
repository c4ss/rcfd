#!/usr/bin/env python3
"""Tests of scripts/roadmap_shards.py, and its validation run on the live roadmap (docs/process/framework-reconciliation.md).

    python scripts/test_roadmap_shards.py
"""
import json
import pathlib
import sys
import tempfile
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import roadmap_shards as rs  # noqa: E402

ROOT = pathlib.Path(__file__).resolve().parent.parent


def item(n, deps=(), status="todo"):
    return {"id": f"F-{n:03d}", "title": f"t{n}", "phase": 1, "deps": [f"F-{d:03d}" for d in deps], "status": status,
            "acceptance": "a"}


class Validate(unittest.TestCase):
    def test_a_valid_roadmap_passes(self):
        rs.validate([item(1), item(2, [1]), item(3, [1, 2])])

    def test_a_duplicate_id_is_refused(self):
        with self.assertRaisesRegex(ValueError, "duplicate item F-001"):
            rs.validate([item(1), item(1)])

    def test_a_missing_dependency_is_refused(self):
        with self.assertRaisesRegex(ValueError, "F-002 depends on missing item F-009"):
            rs.validate([item(1), item(2, [9])])

    def test_a_cycle_is_refused_and_names_its_items(self):
        with self.assertRaisesRegex(ValueError, "dependency cycle blocks F-002, F-003"):
            rs.validate([item(1), item(2, [3]), item(3, [2])])

    def test_an_unknown_status_is_refused(self):
        with self.assertRaisesRegex(ValueError, "invalid status"):
            rs.validate([item(1, status="started")])


class RoundTrip(unittest.TestCase):
    def test_split_and_join_give_the_same_items_in_the_same_order(self):
        items = [item(3), item(1), item(2, [1, 3])]
        with tempfile.TemporaryDirectory() as tmp:
            src = pathlib.Path(tmp) / "roadmap.json"
            src.write_text(rs.encode(items), encoding="utf-8")
            rs.split(src, pathlib.Path(tmp) / "items")
            self.assertEqual(rs.join(pathlib.Path(tmp) / "items"), items)

    def test_split_does_not_write_into_an_existing_folder(self):
        with tempfile.TemporaryDirectory() as tmp:
            src = pathlib.Path(tmp) / "roadmap.json"
            src.write_text(rs.encode([item(1)]), encoding="utf-8")
            (pathlib.Path(tmp) / "items").mkdir()
            with self.assertRaisesRegex(ValueError, "already exists"):
                rs.split(src, pathlib.Path(tmp) / "items")

    def test_a_record_whose_id_is_not_its_file_name_is_refused(self):
        with tempfile.TemporaryDirectory() as tmp:
            d = pathlib.Path(tmp)
            (d / "F-001.json").write_text(json.dumps({"legacy_position": 0, "item": item(2)}), encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "must match filename"):
                rs.join(d)


class LiveRoadmap(unittest.TestCase):
    """The checks CI gets from this file: the roadmap on main has valid ids and statuses, no duplicate, no missing
    dependency and no cycle, and splits into one file per item and joins again without a change."""

    def test_the_live_roadmap_is_valid_and_round_trips(self):
        src = ROOT / "docs" / "roadmap.json"
        items = json.loads(src.read_text(encoding="utf-8"))
        rs.validate(items)
        with tempfile.TemporaryDirectory() as tmp:
            rs.split(src, pathlib.Path(tmp) / "items")
            self.assertEqual(rs.join(pathlib.Path(tmp) / "items"), items)


if __name__ == "__main__":
    unittest.main()
