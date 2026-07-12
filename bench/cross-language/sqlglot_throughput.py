#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Moderately AI Inc.

"""Warm end-to-end parse throughput for sqlglot over the shared conformance corpus.

WHAT THIS MEASURES (read this before reading any number it prints): the warm,
single-thread, end-to-end throughput — statements parsed per wall-clock second —
of `sqlglot.parse_one(sql)` (tokenize -> build the full sqlglot expression tree, NO
optimization / NO schema binding) over the both-accept subset of the shared corpus.

WHAT IT IS NOT: it is NOT apples-to-apples with the Rust parser's per-parse compute
or memory. CPython is an interpreter with a GIL; the object model and allocator are
nothing like Rust's. The ONLY honest cross-language metric is this end-to-end
throughput ("which tool is faster to reach for"), and even that must be read with
the runtime caption below. Memory is deliberately excluded (see the notes doc);
`--rss` prints a peak-RSS figure with a heavy caveat only if explicitly asked.

This runner cannot be executed in the sandboxed worktree (no network, no sqlglot).
It is written from the sqlglot API and is run later in a real environment; see
`docs/performance.md` for the exact `pip install` + run line.

Phases (mirrors the Rust harness's subset-fairness + warm-up methodology):
  1. ACCEPT-PROBE: parse every candidate once under try/except; record which the
     tool accepts (per-corpus coverage). `--emit-accepts FILE` dumps the accepted
     ids and exits — this is the input to the cross-tool intersection.
  2. SUBSET: with `--subset FILE`, time only the ids in FILE that this tool also
     accepts (the comparable both-accept subset). Without it, time this tool's own
     accept set and print a LOUD caveat that self-coverage is not comparable.
  3. WARM-UP: parse the subset repeatedly for `--warmup-secs` to leave interpreter
     import / first-call caching behind (CPython has no JIT, but pycache, attribute
     caches, and allocator arenas still warm).
  4. TIMED: `--reps` timed passes (each an inner loop calibrated to >= --min-pass-secs),
     reporting best (peak) and median parses/sec.
"""

from __future__ import annotations

import argparse
import statistics
import sys
import time
from typing import List, Optional, Set

from corpus_loader import (
    CORPORA,
    Candidate,
    default_corpus_root,
    load_candidates,
)


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(
        description="sqlglot warm parse-throughput over the shared conformance corpus",
    )
    p.add_argument(
        "--corpus-root",
        default=None,
        help="path to conformance/corpus (default: resolved relative to this script)",
    )
    p.add_argument(
        "--dialect",
        default="",
        help="sqlglot read dialect (e.g. postgres); empty = generic. Names the parse surface.",
    )
    p.add_argument(
        "--subset",
        default=None,
        help="file of `<corpus>:<index>` ids (one per line) to restrict timing to "
        "(the both-accept subset). Omit to time this tool's own accept set.",
    )
    p.add_argument(
        "--emit-accepts",
        default=None,
        help="write the ids this tool accepts to FILE (sorted) and exit; the "
        "intersection input. No timing is done.",
    )
    p.add_argument("--warmup-secs", type=float, default=2.0, help="warm-up duration")
    p.add_argument("--reps", type=int, default=7, help="number of timed passes")
    p.add_argument(
        "--min-pass-secs",
        type=float,
        default=0.20,
        help="each timed pass loops the subset until at least this long (kills clock noise)",
    )
    p.add_argument(
        "--rss",
        action="store_true",
        help="also print peak RSS with caveat (NOT a parser-memory figure; see notes)",
    )
    return p.parse_args()


def load_subset_ids(path: str) -> Set[str]:
    """Read a `<corpus>:<index>` id manifest, tolerating blank/`#` lines."""
    ids: Set[str] = set()
    with open(path, "r", encoding="utf-8") as fh:
        for line in fh:
            line = line.strip()
            if line and not line.startswith("#"):
                ids.add(line)
    return ids


def accept_probe(parse_one, candidates: List[Candidate], dialect: str):
    """Parse each candidate once; return (accepted_ids, per-corpus (accepted,total)).

    Broad `except Exception` is intentional: any failure to build the tree is a
    reject for subset purposes, however sqlglot signals it (ParseError,
    TokenizeError, or anything else). The probe runs OUTSIDE every timed window.
    """
    read = dialect or None
    accepted: Set[str] = set()
    coverage = {key: [0, 0] for key, _rel, _shape in CORPORA}
    for c in candidates:
        coverage[c.corpus][1] += 1
        try:
            parse_one(c.sql, read=read)
        except Exception:  # noqa: BLE001 - reject = "did not build a tree", any cause
            continue
        accepted.add(c.id)
        coverage[c.corpus][0] += 1
    return accepted, coverage


def calibrate_passes(parse_one, sqls: List[str], read, min_pass_secs: float) -> int:
    """Inner-loop count so one timed pass spans >= min_pass_secs (clock-noise floor)."""
    t0 = time.perf_counter()
    sink = 0
    for sql in sqls:
        sink ^= id(parse_one(sql, read=read))
    one_pass = time.perf_counter() - t0
    if one_pass <= 0:
        return 1024
    return max(1, int(min_pass_secs / one_pass) + 1)


def timed_rate(parse_one, sqls: List[str], read, passes: int) -> float:
    """One measurement: `passes` full sweeps of the subset, parses/sec."""
    sink = 0
    t0 = time.perf_counter()
    for _ in range(passes):
        for sql in sqls:
            # `id(...)` is the blackhole: cheap, but forces the tree to exist so a
            # future CPython optimizer could not elide the parse. (CPython does not
            # today; kept for parity with the JVM runners, where it matters.)
            sink ^= id(parse_one(sql, read=read))
    dt = time.perf_counter() - t0
    if sink == 0x1234567:  # unreachable; keeps `sink` observably live
        print("", file=sys.stderr)
    return (passes * len(sqls)) / dt if dt > 0 else float("inf")


def peak_rss_caption() -> str:
    """Best-effort peak RSS with the unit caveat baked in. Never a per-parse figure."""
    try:
        import resource

        maxrss = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
    except Exception:  # noqa: BLE001
        return "peak RSS    : unavailable"
    # ru_maxrss is BYTES on macOS, KILOBYTES on Linux — the classic footgun.
    if sys.platform == "darwin":
        mib = maxrss / (1024 * 1024)
    else:
        mib = maxrss / 1024
    return (
        f"peak RSS    : ~{mib:.0f} MiB  (WHOLE process: interpreter + sqlglot + corpus, "
        "NOT per-parse; not comparable to Rust dhat figures — see notes)"
    )


def main() -> int:
    args = parse_args()

    try:
        import sqlglot
    except ImportError:
        print(
            "error: sqlglot is not installed. See docs/performance.md\n"
            "       (e.g. `pip install \"sqlglot==25.34.0\"`).",
            file=sys.stderr,
        )
        return 2

    parse_one = sqlglot.parse_one
    version = getattr(sqlglot, "__version__", "unknown")
    corpus_root = args.corpus_root or default_corpus_root()
    candidates = load_candidates(corpus_root)

    accepted, coverage = accept_probe(parse_one, candidates, args.dialect)

    if args.emit_accepts:
        with open(args.emit_accepts, "w", encoding="utf-8") as fh:
            for cid in sorted(accepted):
                fh.write(cid + "\n")
        print(
            f"wrote {len(accepted)} accepted ids to {args.emit_accepts} "
            f"(sqlglot {version}, dialect={args.dialect or 'generic'})"
        )
        return 0

    # Subset selection: requested ids (the both-accept manifest) intersected with
    # what THIS tool accepts, so the timed loop never touches an error path.
    by_id = {c.id: c for c in candidates}
    requested: Optional[Set[str]] = None
    if args.subset:
        requested = load_subset_ids(args.subset)
        measured_ids = sorted(requested & accepted)
        missing = sorted(requested - accepted)
    else:
        measured_ids = sorted(accepted)
        missing = []

    measured_sqls = [by_id[i].sql for i in measured_ids if i in by_id]
    if not measured_sqls:
        print("error: measured subset is empty (no accepted ids to time)", file=sys.stderr)
        return 1

    read = args.dialect or None

    # Warm-up: loop the subset until warmup-secs elapses (>= one full pass).
    warm_end = time.perf_counter() + args.warmup_secs
    sink = 0
    passes_done = 0
    while time.perf_counter() < warm_end:
        for sql in measured_sqls:
            sink ^= id(parse_one(sql, read=read))
        passes_done += 1
    if passes_done == 0:  # warmup-secs == 0
        for sql in measured_sqls:
            sink ^= id(parse_one(sql, read=read))

    passes = calibrate_passes(parse_one, measured_sqls, read, args.min_pass_secs)
    rates = [timed_rate(parse_one, measured_sqls, read, passes) for _ in range(args.reps)]
    best = max(rates)
    median = statistics.median(rates)

    total_candidates = sum(v[1] for v in coverage.values())
    total_accepted = sum(v[0] for v in coverage.values())

    print("# cross-language throughput: sqlglot")
    print(f"#   runtime         : CPython {sys.version.split()[0]}  (interpreter; GIL; single-thread)")
    print(f"#   tool version    : sqlglot {version}")
    print("#   parse unit      : sqlglot.parse_one(sql) -> full AST (NO optimize, NO bind)")
    print(f"#   dialect         : {args.dialect or 'generic'}  (read=)")
    print(f"#   corpus root     : {corpus_root}")
    print("#   metric          : parses/sec = statements / wall_seconds (warm, 1 thread, END-TO-END)")
    print(
        f"#   method          : warm-up >= {args.warmup_secs:g}s (excl. import/startup), "
        f"{args.reps} timed passes x {passes} inner loops (>= {args.min_pass_secs:g}s each)"
    )
    if args.subset:
        print(f"#   subset          : {args.subset}  ({len(requested)} requested ids)")
        if missing:
            print(
                f"#   WARNING         : {len(missing)} requested id(s) NOT accepted by sqlglot "
                "-> excluded from timing (subset/version drift; regenerate the intersection)"
            )
    else:
        print("#   subset          : SELF-COVERAGE (this tool's own accept set)")
        print("#   WARNING         : self-coverage is NOT the comparable both-accept subset;")
        print("#                     pass --subset both_accept.txt for a fair cross-tool number.")
    print("#")
    print(f"# coverage (sqlglot accepts / candidates), per corpus:")
    for key, _rel, _shape in CORPORA:
        acc, tot = coverage[key]
        print(f"#   {key:<28} {acc:>5}/{tot}")
    print(f"#   {'TOTAL':<28} {total_accepted:>5}/{total_candidates}")
    print("#")
    print(f"# throughput over the measured subset ({len(measured_sqls)} statements):")
    print(f"#   best   : {best:>12,.0f} parses/sec")
    print(f"#   median : {median:>12,.0f} parses/sec")
    if args.rss:
        print(f"#   {peak_rss_caption()}")
    print(f"#   (blackhole {sink:#x} — ignore)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
