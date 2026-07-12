#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Moderately AI Inc.

"""Intersect per-tool accept manifests into the both-accept (comparable) subset.

Each runner's `--emit-accepts` writes the `<corpus>:<index>` ids that tool parses.
The fair cross-language throughput number is measured over the statements EVERY
compared tool accepts — the intersection — so no tool is timed on an error path it
happens to be fast (or slow) at. This is the cross-language analogue of the Rust
harness's `subset()` fairness gate (`bench/benches/upstream/mod.rs`), which only
ever measures the ours-AND-theirs intersection.

Usage:
    python3 intersect_accepts.py sqlglot.ids calcite.ids jsqlparser.ids -o both_accept.txt
    # fold in the Rust side too, if you produced rust.ids (see the notes doc):
    python3 intersect_accepts.py *.ids -o both_accept.txt

Pure stdlib, so it runs anywhere Python 3 does (the operator already has Python for
sqlglot). It also prints, to stderr, each input's size and the surviving count, so
the shrink from union to intersection is visible — exactly the coverage story the
notes' caveat framework calls for.
"""

from __future__ import annotations

import argparse
import sys
from typing import List, Set


def read_ids(path: str) -> Set[str]:
    ids: Set[str] = set()
    with open(path, "r", encoding="utf-8") as fh:
        for line in fh:
            line = line.strip()
            if line and not line.startswith("#"):
                ids.add(line)
    return ids


def main() -> int:
    ap = argparse.ArgumentParser(description="intersect accept-id manifests")
    ap.add_argument("manifests", nargs="+", help="two or more `--emit-accepts` id files")
    ap.add_argument("-o", "--output", default=None, help="output file (default: stdout)")
    args = ap.parse_args()

    if len(args.manifests) < 2:
        print("error: need at least two manifests to intersect", file=sys.stderr)
        return 2

    sets: List[Set[str]] = []
    for path in args.manifests:
        s = read_ids(path)
        sets.append(s)
        print(f"# {path}: {len(s)} ids", file=sys.stderr)

    common = set.intersection(*sets)
    union = set.union(*sets)
    print(
        f"# intersection: {len(common)} ids "
        f"(union was {len(union)}; {len(union) - len(common)} dropped as not-all-accept)",
        file=sys.stderr,
    )

    lines = "\n".join(sorted(common))
    if args.output:
        with open(args.output, "w", encoding="utf-8") as fh:
            fh.write(lines + ("\n" if lines else ""))
        print(f"# wrote {len(common)} ids to {args.output}", file=sys.stderr)
    else:
        if lines:
            print(lines)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
