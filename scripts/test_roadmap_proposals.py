#!/usr/bin/env python3
"""Tests of scripts/roadmap_proposals.py (ADR 0755, F-864): the form of a roadmap proposal, its validation and its
application. The ids of the rows are those of the table of checks of ADR 0755.

    python scripts/test_roadmap_proposals.py
"""
import contextlib
import copy
import io
import json
import pathlib
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import roadmap_proposals as rp  # noqa: E402
import roadmap_shards as rs  # noqa: E402

ROOT = pathlib.Path(__file__).resolve().parent.parent
BASE = "a" * 40


def item(n, deps=(), status="todo", note=None, gap=None, phase=1):
    out = {"id": f"F-{n:03d}", "title": f"t{n}", "phase": phase, "deps": [f"F-{d:03d}" for d in deps], "status": status,
           "acceptance": "a"}
    if gap is not None:
        out["gap"] = gap
    if note is not None:
        out["note"] = note
    return out


def roadmap():
    return [item(1, note="Lead (2026-11-01): entered."), item(2, [1], status="partial", note="n2", gap="old gap"),
            item(3, [1], phase=7)]


def prop(item_id="F-001", adr="0901", base=BASE, status="done", **more):
    out = {"form": 1, "adr": adr, "item": item_id, "base": base, "status": status,
           "note": f"ADR {adr}, 2026-11-21, {status}: one sentence."}
    out.update(more)
    return out


def follow(title="a follow-up", deps=("F-001",), **more):
    out = {"title": title, "deps": list(deps), "acceptance": "the registration gives the limits"}
    out.update(more)
    return out


def at_base(items):
    """A base lookup for the pure function: every base gives these items."""
    frozen = copy.deepcopy(items)
    return lambda base: copy.deepcopy(frozen)


def write(folder, proposal, name=None):
    path = pathlib.Path(folder) / (name or f"{proposal['adr']}-{proposal['item']}.json")
    path.write_text(proposal if isinstance(proposal, str) else json.dumps(proposal, ensure_ascii=False, indent=2),
                    encoding="utf-8")
    return path


def git(repo, *args):
    return subprocess.run(["git", "-C", str(repo), *args], check=True, capture_output=True, text=True).stdout.strip()


def commit(repo, items, message):
    path = pathlib.Path(repo) / "docs" / "roadmap.json"
    path.parent.mkdir(exist_ok=True)
    with path.open("w", encoding="utf-8", newline="\n") as file:
        file.write(rs.encode(items))
    git(repo, "add", "docs/roadmap.json")
    git(repo, "-c", "user.name=t", "-c", "user.email=t@example.invalid", "-c", "commit.gpgsign=false", "commit", "-q",
        "-m", message)
    return git(repo, "rev-parse", "HEAD")


def history(repo):
    """main: c0 (roadmap()), then c1 (the note of F-003 changed). side: one commit from c0. HEAD is c1.
    Returns (c0, c1, side)."""
    git(repo, "init", "-q", "-b", "main")
    c0 = commit(repo, roadmap(), "c0")
    git(repo, "checkout", "-q", "-b", "side")
    moved = roadmap()
    moved[0]["title"] = "on the side"
    side = commit(repo, moved, "side")
    git(repo, "checkout", "-q", "main")
    later = roadmap()
    later[2]["note"] = "Lead (2026-11-20): a gate result."
    c1 = commit(repo, later, "c1")
    return c0, c1, side


def run(*argv):
    """(exit status, standard output, standard error) of the command."""
    out, err = io.StringIO(), io.StringIO()
    with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
        status = rp.main([str(a) for a in argv])
    return status, out.getvalue(), err.getvalue()


class Named(unittest.TestCase):
    def refused(self, name, items, proposals, **more):
        before = copy.deepcopy(items)
        with self.assertRaises(rp.ProposalError) as caught:
            rp.apply(items, proposals, base_items=more.pop("base_items", at_base(items)), **more)
        self.assertEqual(caught.exception.name, name, str(caught.exception))
        self.assertTrue(str(caught.exception).startswith(name + ": "), str(caught.exception))
        self.assertEqual(items, before)
        return str(caught.exception)


class Application(Named):
    """Row V: D3 by its steps."""

    def test_the_note_is_appended_after_one_space_and_the_status_set(self):
        items = roadmap()
        out = rp.apply(items, [prop("F-001")], base_items=at_base(items))
        self.assertEqual(out[0]["note"], "Lead (2026-11-01): entered. ADR 0901, 2026-11-21, done: one sentence.")
        self.assertEqual(out[0]["status"], "done")
        self.assertEqual(out[1:], items[1:])
        self.assertEqual(items, roadmap())  # what was given is not changed

    def test_an_item_without_a_note_gets_the_entry_as_its_note(self):
        items = roadmap()
        out = rp.apply(items, [prop("F-003", status="todo")], base_items=at_base(items))
        self.assertEqual(out[2]["note"], "ADR 0901, 2026-11-21, todo: one sentence.")
        self.assertEqual(list(out[2]), ["id", "title", "phase", "deps", "status", "acceptance", "note"])

    def test_a_gap_replaces_the_gap_of_a_partial_item(self):
        items = roadmap()
        out = rp.apply(items, [prop("F-002", status="partial", gap="what is missing now")], base_items=at_base(items))
        self.assertEqual((out[1]["status"], out[1]["gap"]), ("partial", "what is missing now"))

    def test_a_partial_proposal_without_a_gap_leaves_the_gap(self):
        items = roadmap()
        out = rp.apply(items, [prop("F-002", status="partial")], base_items=at_base(items))
        self.assertEqual(out[1]["gap"], "old gap")

    def test_the_gap_is_removed_when_the_status_is_not_partial(self):
        items = roadmap()
        out = rp.apply(items, [prop("F-002", status="done")], base_items=at_base(items))
        self.assertNotIn("gap", out[1])
        self.assertEqual(out[1]["status"], "done")
        self.assertEqual(render_gap_problems(out), [])

    def test_a_first_gap_lies_before_the_note_as_in_the_roadmap(self):
        items = roadmap()
        out = rp.apply(items, [prop("F-001", status="partial", gap="g")], base_items=at_base(items))
        self.assertEqual(list(out[0]), ["id", "title", "phase", "deps", "status", "acceptance", "gap", "note"])

    def test_follow_ups_are_appended_with_ids_from_the_start_number(self):
        items = roadmap()
        ups = [follow("first", key="one"), follow("second", deps=["new:one", "F-002"], phase=4)]
        out = rp.apply(items, [prop("F-003", status="partial", gap="g", follow_ups=ups)], start=870,
                       base_items=at_base(items))
        self.assertEqual([i["id"] for i in out], ["F-001", "F-002", "F-003", "F-870", "F-871"])
        self.assertEqual(out[3], {"id": "F-870", "title": "first", "phase": 7, "deps": ["F-001"], "status": "todo",
                                  "acceptance": "the registration gives the limits"})
        self.assertEqual(out[4], {"id": "F-871", "title": "second", "phase": 4, "deps": ["F-870", "F-002"],
                                  "status": "todo", "acceptance": "the registration gives the limits"})
        rs.validate(out)

    def test_without_a_start_number_the_ids_follow_the_highest(self):
        items = roadmap()
        out = rp.apply(items, [prop("F-001", follow_ups=[follow()])], base_items=at_base(items))
        self.assertEqual(out[-1]["id"], "F-004")


def render_gap_problems(items):
    return [i["id"] for i in items if "gap" in i and i["status"] != "partial"]


class Composition(Named):
    def test_c1_two_proposals_for_different_items_give_the_same_result_in_either_order(self):
        items = roadmap()
        a = prop("F-001", adr="0902", follow_ups=[follow("from a")])
        b = prop("F-002", adr="0901", status="partial", gap="g", follow_ups=[follow("from b", deps=["F-002"])])
        ab = rp.apply(items, [a, b], start=10, base_items=at_base(items))
        ba = rp.apply(items, [b, a], start=10, base_items=at_base(items))
        self.assertEqual(rs.encode(ab), rs.encode(ba))
        # the order is that of (adr, item): ADR 0901 gets the first id
        self.assertEqual([(i["id"], i["title"]) for i in ab[3:]], [("F-010", "from b"), ("F-011", "from a")])
        self.assertEqual(ab[0]["status"], "done")
        self.assertEqual(ab[1]["gap"], "g")

    def test_c1_one_after_the_other_in_two_calls_gives_the_same_bytes_in_either_order(self):
        items = roadmap()
        base = at_base(items)
        a, b = prop("F-001", adr="0902"), prop("F-002", adr="0901", status="partial", gap="g")
        a_then_b = rp.apply(rp.apply(items, [a], base_items=base), [b], base_items=base)
        b_then_a = rp.apply(rp.apply(items, [b], base_items=base), [a], base_items=base)
        together = rp.apply(items, [a, b], base_items=base)
        self.assertEqual(rs.encode(a_then_b), rs.encode(b_then_a))
        self.assertEqual(rs.encode(a_then_b), rs.encode(together))

    def test_c2_two_proposals_for_the_same_item_are_refused(self):
        message = self.refused("same-item", roadmap(), [prop("F-001", adr="0901"), prop("F-001", adr="0902")])
        self.assertIn("F-001", message)

    def test_c2_the_second_proposal_for_an_item_is_stale_after_the_first(self):
        items = roadmap()
        after_first = rp.apply(items, [prop("F-001", adr="0901")], base_items=at_base(items))
        message = self.refused("stale-item", after_first, [prop("F-001", adr="0902", status="partial", gap="g")],
                               base_items=at_base(items))
        self.assertIn("F-001", message)
        self.assertIn("note", message)
        self.assertIn("status", message)

    def test_c3_an_id_that_exists_is_refused(self):
        message = self.refused("duplicate-id", roadmap(), [prop("F-001", follow_ups=[follow()])], start=2)
        self.assertIn("F-002", message)

    def test_c3_two_follow_ups_with_one_key_are_refused(self):
        ups = [follow("x", key="k"), follow("y", key="k")]
        self.refused("duplicate-id", roadmap(), [prop("F-001", follow_ups=ups)], start=10)

    def test_c4_a_dependency_that_is_no_item_is_refused(self):
        message = self.refused("missing-dependency", roadmap(), [prop("F-001", follow_ups=[follow(deps=["F-999"])])],
                               start=10)
        self.assertIn("F-999", message)

    def test_c4_a_new_dependency_without_its_key_is_refused(self):
        self.refused("missing-dependency", roadmap(), [prop("F-001", follow_ups=[follow(deps=["new:nobody"])])],
                     start=10)

    def test_c5_follow_ups_that_depend_on_each_other_are_refused(self):
        ups = [follow("x", deps=["new:b"], key="a"), follow("y", deps=["new:a"], key="b")]
        message = self.refused("cycle", roadmap(), [prop("F-001", follow_ups=ups)], start=10)
        self.assertIn("F-010", message)
        self.assertIn("F-011", message)

    def test_an_item_that_is_not_in_the_roadmap_is_refused(self):
        self.refused("unknown-item", roadmap(), [prop("F-777")])

    def test_a_roadmap_that_is_not_valid_is_refused(self):
        items = roadmap()
        items[1]["deps"] = ["F-555"]
        self.refused("invalid-roadmap", items, [prop("F-001")])


class StaleInGit(unittest.TestCase):
    """Row C6: the base against the history of a temporary repository."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.repo = pathlib.Path(self.tmp.name) / "repo"
        self.repo.mkdir()
        self.c0, self.c1, self.side = history(self.repo)
        self.items = rs.read_json(self.repo / "docs" / "roadmap.json")
        self.lookup = rp.git_base_items(self.repo, "HEAD")

    def refused(self, name, proposal):
        with self.assertRaises(rp.ProposalError) as caught:
            rp.apply(self.items, [proposal], base_items=self.lookup)
        self.assertEqual(caught.exception.name, name, str(caught.exception))
        return str(caught.exception)

    def test_c6_a_base_that_is_an_ancestor_with_the_item_unchanged_applies(self):
        out = rp.apply(self.items, [prop("F-001", base=self.c0)], base_items=self.lookup)
        self.assertEqual(out[0]["status"], "done")
        out = rp.apply(self.items, [prop("F-003", base=self.c1)], base_items=self.lookup)
        self.assertTrue(out[2]["note"].startswith("Lead (2026-11-20): a gate result. ADR 0901"))

    def test_c6_a_base_that_is_not_an_ancestor_is_refused(self):
        message = self.refused("stale-base", prop("F-002", base=self.side, status="partial"))
        self.assertIn("not an ancestor", message)

    def test_c6_an_item_whose_note_changed_since_the_base_is_refused(self):
        message = self.refused("stale-item", prop("F-003", base=self.c0))
        self.assertIn("F-003", message)
        self.assertIn("note", message)

    def test_c6_a_base_that_is_no_commit_is_refused(self):
        self.refused("stale-base", prop("F-001", base="0" * 40))

    def test_c6_the_roadmap_of_an_earlier_commit_is_checked_against_that_commit(self):
        # the roadmap of c0 with --at c0: a base of c1 is not an ancestor of it
        lookup = rp.git_base_items(self.repo, self.c0)
        with self.assertRaises(rp.ProposalError) as caught:
            rp.apply(roadmap(), [prop("F-001", base=self.c1)], base_items=lookup)
        self.assertEqual(caught.exception.name, "stale-base")


MALFORMED = {
    "not json": "{ this is not json",
    "not an object": "[1, 2]",
    "a missing key": {k: v for k, v in prop().items() if k != "status"},
    "an unknown key": prop(priority="high"),
    "form 2": prop(form=2),
    "a short base": prop(base="abc1234"),
    "an adr of three digits": prop(adr="901"),
    "an item that is no id": prop("F-1"),
    "a status of another word": prop(status="started"),
    "a note of two lines": prop(note="ADR 0901, 2026-11-21, done: one.\nTwo."),
    "a note without its head": prop(note="Done on 2026-11-21: one sentence."),
    "a note of another adr": prop(note="ADR 0900, 2026-11-21, done: one sentence."),
    "an empty note": prop(note=""),
    "a gap on a done item": prop(status="done", gap="g"),
    "a gap of 501 characters": prop(status="partial", gap="g" * 501),
    "a gap of two lines": prop(status="partial", gap="g\nh"),
    "follow-ups that are no list": prop(follow_ups={"title": "x"}),
    "a follow-up without acceptance": prop(follow_ups=[{"title": "x", "deps": []}]),
    "a follow-up with an id": prop(follow_ups=[follow(id="F-900")]),
    "a follow-up with a title of two lines": prop(follow_ups=[follow("x\ny")]),
    "a follow-up whose deps are no list": prop(follow_ups=[follow(deps="F-001")]),
    "a follow-up with a dep that is no id": prop(follow_ups=[follow(deps=["item one"])]),
    "a phase of 12": prop(follow_ups=[follow(phase=12)]),
    "a key with a space": prop(follow_ups=[follow(key="a b")]),
}


class Form(unittest.TestCase):
    """Row C7, and the reading of a well-formed file."""

    def test_a_well_formed_file_is_read(self):
        with tempfile.TemporaryDirectory() as tmp:
            full = prop("F-002", status="partial", gap="g" * 500, follow_ups=[follow(key="k", phase=0), follow(deps=[])])
            self.assertEqual(rp.read_proposal(write(tmp, full)), full)
            self.assertEqual(rp.read_proposal(write(tmp, prop())), prop())

    def test_c7_each_malformed_file_is_refused(self):
        for what, content in MALFORMED.items():
            with self.subTest(what), tempfile.TemporaryDirectory() as tmp:
                path = write(tmp, content, name="0901-F-001.json")
                with self.assertRaises(rp.ProposalError) as caught:
                    rp.read_proposal(path)
                self.assertEqual(caught.exception.name, "malformed", str(caught.exception))
                self.assertIn("0901-F-001.json", str(caught.exception))

    def test_c7_a_file_name_that_is_not_its_adr_and_item_is_refused(self):
        with tempfile.TemporaryDirectory() as tmp:
            for name in ("0902-F-001.json", "0901-F-002.json", "proposal.json"):
                with self.subTest(name), self.assertRaises(rp.ProposalError) as caught:
                    rp.read_proposal(write(tmp, prop(), name=name))
                self.assertEqual(caught.exception.name, "malformed")

    def test_c7_a_malformed_object_is_refused_by_apply_as_well(self):
        items = roadmap()
        with self.assertRaises(rp.ProposalError) as caught:
            rp.apply(items, [prop(status="started")], base_items=at_base(items))
        self.assertEqual(caught.exception.name, "malformed")


class Command(unittest.TestCase):
    """Row W: nothing is written on a failure, and the roadmap is written only when it is named with --in-place."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.repo = pathlib.Path(self.tmp.name) / "repo"
        self.repo.mkdir()
        self.c0, self.c1, self.side = history(self.repo)
        self.roadmap = self.repo / "docs" / "roadmap.json"
        self.before = self.roadmap.read_bytes()
        self.files = pathlib.Path(self.tmp.name) / "proposals"
        self.files.mkdir()
        self.out = pathlib.Path(self.tmp.name) / "out.json"

    def apply(self, *files, extra=()):
        return run("apply", *files, "--repo", self.repo, "--output", self.out, *extra)

    def test_w_every_named_failure_writes_nothing(self):
        c1 = self.c1
        cases = {
            "same-item": ([prop("F-001", adr="0901", base=c1), prop("F-001", adr="0902", base=c1)], ()),
            "duplicate-id": ([prop("F-001", base=c1, follow_ups=[follow()])], ("--start", "2")),
            "missing-dependency": ([prop("F-001", base=c1, follow_ups=[follow(deps=["F-999"])])], ("--start", "10")),
            "cycle": ([prop("F-001", base=c1, follow_ups=[follow("x", deps=["new:b"], key="a"),
                                                           follow("y", deps=["new:a"], key="b")])], ("--start", "10")),
            "stale-base": ([prop("F-001", base=self.side)], ()),
            "stale-item": ([prop("F-003", base=self.c0)], ()),
            "unknown-item": ([prop("F-777", base=c1)], ()),
            "malformed": ([prop("F-001", base=c1), "{ not json"], ()),
        }
        for name, (proposals, extra) in cases.items():
            with self.subTest(name):
                files = [write(self.files, p, name=None if isinstance(p, dict) else "0903-F-002.json") for p in proposals]
                status, out, err = self.apply(*files, extra=extra)
                self.assertEqual(status, 1)
                self.assertTrue(err.startswith(name + ": "), err)
                self.assertEqual(out, "")
                self.assertFalse(self.out.exists())
                self.assertEqual(self.roadmap.read_bytes(), self.before)
                self.assertEqual(sorted(p.name for p in self.out.parent.iterdir()), ["proposals", "repo"])
                for path in files:
                    path.unlink()

    def test_w_a_valid_set_is_written_to_the_output_and_the_roadmap_is_left(self):
        a = write(self.files, prop("F-001", base=self.c1, follow_ups=[follow()]))
        b = write(self.files, prop("F-003", adr="0902", base=self.c1, status="todo"))
        status, out, err = self.apply(a, b, extra=("--start", "870"))
        self.assertEqual((status, out, err), (0, "", ""))
        self.assertEqual(self.roadmap.read_bytes(), self.before)
        written = self.out.read_bytes()
        self.assertNotIn(b"\r", written)
        items = json.loads(written.decode("utf-8"))
        self.assertEqual(written.decode("utf-8"), rs.encode(items))
        self.assertEqual([i["id"] for i in items], ["F-001", "F-002", "F-003", "F-870"])
        self.assertEqual(items[0]["status"], "done")

    def test_w_without_an_output_the_result_goes_to_standard_output(self):
        a = write(self.files, prop("F-001", base=self.c1))
        status, out, err = run("apply", a, "--repo", self.repo)
        self.assertEqual((status, err), (0, ""))
        self.assertEqual(json.loads(out)[0]["status"], "done")
        self.assertEqual(self.roadmap.read_bytes(), self.before)
        self.assertFalse(self.out.exists())

    def test_w_the_roadmap_as_output_is_refused_without_in_place(self):
        a = write(self.files, prop("F-001", base=self.c1))
        for target in (self.roadmap, self.repo / "docs" / ".." / "docs" / "roadmap.json"):
            status, out, err = run("apply", a, "--repo", self.repo, "--output", target)
            self.assertEqual(status, 1)
            self.assertTrue(err.startswith("refused-output: "), err)
            self.assertEqual(self.roadmap.read_bytes(), self.before)

    def test_w_the_roadmap_is_written_with_in_place(self):
        a = write(self.files, prop("F-001", base=self.c1))
        status, out, err = run("apply", a, "--repo", self.repo, "--output", self.roadmap, "--in-place")
        self.assertEqual((status, out, err), (0, "", ""))
        self.assertEqual(json.loads(self.roadmap.read_text(encoding="utf-8"))[0]["status"], "done")

    def test_w_apply_with_a_follow_up_needs_a_start_number(self):
        a = write(self.files, prop("F-001", base=self.c1, follow_ups=[follow()]))
        with self.assertRaises(SystemExit) as caught, contextlib.redirect_stderr(io.StringIO()):
            rp.main(["apply", str(a), "--repo", str(self.repo), "--output", str(self.out)])
        self.assertEqual(caught.exception.code, 2)
        self.assertFalse(self.out.exists())

    def test_check_writes_nothing_and_names_what_it_would_do(self):
        a = write(self.files, prop("F-001", base=self.c1, follow_ups=[follow()]))
        status, out, err = run("check", a, "--repo", self.repo)
        self.assertEqual((status, err), (0, ""))
        self.assertIn("F-001", out)
        self.assertIn("F-004", out)
        self.assertEqual(self.roadmap.read_bytes(), self.before)
        status, out, err = run("check", write(self.files, prop("F-003", adr="0902", base=self.c0)), "--repo", self.repo)
        self.assertEqual(status, 1)
        self.assertTrue(err.startswith("stale-item: "), err)

    def test_a_roadmap_of_an_earlier_commit_is_read_from_a_copy(self):
        copy_of_c0 = pathlib.Path(self.tmp.name) / "c0.json"
        copy_of_c0.write_bytes(rs.encode(roadmap()).encode("utf-8"))
        a = write(self.files, prop("F-003", base=self.c0, status="todo"))
        status, out, err = run("apply", a, "--repo", self.repo, "--roadmap", copy_of_c0, "--at", self.c0)
        self.assertEqual((status, err), (0, ""))
        self.assertEqual(json.loads(out)[2]["note"], "ADR 0901, 2026-11-21, todo: one sentence.")


class LiveRoadmap(unittest.TestCase):
    """Row U: on the roadmap of this repository nothing moves but the item of the proposal."""

    def test_u_a_proposal_changes_its_item_alone(self):
        src = ROOT / "docs" / "roadmap.json"
        raw = src.read_bytes()
        items = json.loads(raw.decode("utf-8"))
        self.assertEqual(rs.encode(items).encode("utf-8"), raw)
        first = items[0]
        out = rp.apply(items, [prop(first["id"], status=first["status"])], base_items=at_base(items))
        self.assertEqual(out[1:], items[1:])
        self.assertEqual({k: v for k, v in out[0].items() if k != "note"}, {k: v for k, v in first.items() if k != "note"})
        self.assertEqual(out[0]["note"], (first.get("note", "") + " ADR 0901, 2026-11-21, %s: one sentence." % first["status"]).strip())
        self.assertEqual(json.loads(src.read_bytes().decode("utf-8")), items)


if __name__ == "__main__":
    unittest.main()
