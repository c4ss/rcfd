"""Format check of one crate: `python scripts/fmt_check.py <crate> [<crate> ...]`.

Does what `cargo fmt -p <crate> -- --check` does, in batches. `cargo fmt` hands every target file of the crate to one
rustfmt process by its absolute path; for rcfd-fem (more than 370 test files) that command line is longer than Windows
allows ("The filename or extension is too long", os error 206), so the check could not run there (found at the merge of
F-672, 2026-11-19). The files are the roots of the crate's targets, as cargo finds them without a manifest entry:
build.rs, src/lib.rs, src/main.rs, src/bin/*.rs, and the .rs files of tests, benches and examples; rustfmt follows the
modules from each root. The edition is the workspace's.
"""
import pathlib
import re
import subprocess
import sys

BATCH = 40


def roots(crate_dir):
    out = [p for p in (crate_dir / "build.rs", crate_dir / "src" / "lib.rs", crate_dir / "src" / "main.rs") if p.is_file()]
    for sub in ("src/bin", "tests", "benches", "examples"):
        d = crate_dir / sub
        if d.is_dir():
            out += sorted(d.glob("*.rs"))
            out += sorted(p for p in d.glob("*/main.rs"))
    return out


def edition(root):
    m = re.search(r'^edition\s*=\s*"(\d{4})"', (root / "Cargo.toml").read_text(encoding="utf-8"), re.M)
    return m.group(1) if m else "2021"


def main(argv):
    root = pathlib.Path(__file__).resolve().parent.parent
    if not argv:
        print(__doc__)
        return 2
    ed, bad = edition(root), 0
    for crate in argv:
        d = root / "crates" / crate
        files = [str(p.relative_to(root)).replace("\\", "/") for p in roots(d)]
        if not files:
            print(f"fmt_check: no target file found for {crate} under {d}")
            return 2
        for i in range(0, len(files), BATCH):
            r = subprocess.run(["rustfmt", "--check", "--edition", ed] + files[i:i + BATCH], cwd=root)
            bad += r.returncode != 0
        print(f"fmt_check: {crate}: {len(files)} target files in {-(-len(files) // BATCH)} batches")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
