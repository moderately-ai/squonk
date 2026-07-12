# SPDX-License-Identifier: MIT
#!/usr/bin/env python3
"""Extract a signature-weighted DuckDB SQL corpus from the vendored test suite.

Reads DuckDB's sqllogictest-style `test/sql/**/*.test` files, pulls the SQL out of
the `statement ok|error|maybe` and `query <types>` records, normalizes each to a
single line, dedupes, and weights the result toward DuckDB's signature grammar
surface (EXCLUDE/REPLACE, GROUP BY ALL, FROM-first, PIVOT, list/struct/map
literals, lambdas, ASOF, QUALIFY, UNION BY NAME, positional joins).

Two artifacts are emitted:

  python3 extract.py statements.sql [statements_with_schema.sql]

- `statements.sql` — the flat, schema-independent fast-path corpus (one statement
  per line), unchanged.
- `statements_with_schema.sql` (optional second arg) — the *same* selected queries
  regrouped under their source `.test` file, each group prefixed by that file's own
  concrete `CREATE` setup DDL. This is the setup-driver artifact: provisioning a
  file's DDL before `prepare`ing its queries lets the DuckDB oracle bind names
  instead of binding-rejecting `FROM integers` over an empty database, turning the
  blind binding-reject quadrant into real accept/reject signal
  ([[duckdb-corpus-oracle-at-scale]]; ADR-0015).

Run once at vendoring time; both outputs are committed. Not part of the build.
"""
import os
import re
import sys
from collections import OrderedDict, Counter

DUCKDB = os.path.expanduser("~/workspace/github.com/duckdb/duckdb")
TEST_ROOT = os.path.join(DUCKDB, "test", "sql")

# Signature-surface detectors: (family, compiled regex over the normalized SQL).
# Tight on purpose — a `[` type suffix (`INT[]`) or a `{PLACEHOLDER}` template must
# NOT read as a list/struct literal.
SIG = [
    ("star_exclude_replace", re.compile(r"\*\s*(EXCLUDE|REPLACE|RENAME)\b", re.I)),
    ("star_exclude_replace", re.compile(r"\bCOLUMNS\s*\(", re.I)),
    ("group_order_by_all", re.compile(r"\b(GROUP|ORDER)\s+BY\s+ALL\b", re.I)),
    ("from_first", re.compile(r"^\s*FROM\s+", re.I)),
    ("pivot_unpivot", re.compile(r"\b(UN)?PIVOT\b", re.I)),
    # list literal opens with a quote/digit/negative/NULL/nested — not `[]`/`[colname]`.
    ("collection_literals", re.compile(r"\[\s*(?:'|\"|\d|\[|\{|-|NULL\b)", re.I)),
    # struct literal: a colon-separated key inside braces (excludes `{TEMPLATE}`).
    ("collection_literals", re.compile(r"\{[^{}]*:[^{}]*\}")),
    ("collection_literals", re.compile(r"\bMAP\s*(\{|\[)", re.I)),
    # lambda arrow, not `->>` JSON extract and not `>=`/`<-`.
    ("lambda", re.compile(r"[A-Za-z_)]\s*->\s*[^>]", re.I)),
    ("asof_positional_join", re.compile(r"\b(ASOF|POSITIONAL)\b", re.I)),
    ("qualify", re.compile(r"\bQUALIFY\b", re.I)),
    ("union_by_name", re.compile(r"\bUNION\s+(ALL\s+)?BY\s+NAME\b", re.I)),
]

# Directories to draw the general (non-signature) fill from, capped per file so no
# single family dominates the tail.
GENERAL_CAP_PER_FILE = 5
SIG_CAP_PER_FILE = 25
# Global per-family cap keeps the nine families balanced rather than letting the
# huge collection-literal pool crowd out ASOF/QUALIFY/UNION-BY-NAME.
FAMILY_CAP = 170
TARGET_TOTAL = 1350

# Template / non-SQL markers that make a line unusable as a standalone statement.
BAD_MARKERS = ("${", "<FILE>", "__TEST_DIR__", "concurrentloop", "\\x", "{DATA_DIR}",
               "{TEMP_DIR}", "{BIG_DIR}", "{type}", "[INVALID]")
PLACEHOLDER = re.compile(r"\{[A-Za-z_][A-Za-z_0-9]*\}")  # {type}, {DATA_DIR}, …
CONFIG_HEAD = ("SET ", "PRAGMA", "RESET", "INSTALL", "LOAD ")

# Setup-DDL capture (for statements_with_schema.sql). Only `CREATE` records are used
# as provisioning DDL: they establish object existence + columns, which is all
# `prepare` binding needs — INSERT/DROP are not needed (no rows) and only add
# execute_batch failure surface. The target-name group lets a file's drop/recreate
# keep its final definition (last write wins) so provisioning a whole file's DDL in
# one batch does not double-create.
CREATE_HEAD = re.compile(r"^\s*CREATE\b", re.I)
# Object kind + name, for dedup (last write wins, collapsing a file's drop/recreate)
# and dependency-ordered emission: the types/schemas/sequences a table's columns depend
# on must be provisioned before the table, and views/indexes after. Bucketing by kind
# gives that order without a full dependency sort.
CREATE_OBJ = re.compile(
    r"^\s*CREATE\s+(?:OR\s+REPLACE\s+)?(?:TEMP(?:ORARY)?\s+)?"
    r"(TABLE|VIEW|TYPE|SCHEMA|SEQUENCE|MACRO|FUNCTION)\s+(?:IF\s+NOT\s+EXISTS\s+)?"
    r"([A-Za-z_0-9\".]+)",
    re.I,
)
CREATE_BUCKET = {"schema": 0, "type": 1, "sequence": 2, "table": 3,
                 "view": 4, "macro": 5, "function": 5}
# On-disk-file references fail on a fresh in-memory database — skip such a CREATE so
# the rest of the file's DDL still provisions (the query needing it stays a counted
# binding residual, never a false gate signal).
DDL_FILE_REFS = ("read_csv", "read_parquet", "read_json", "read_ndjson", "'data/",
                 "'test/", ".csv", ".parquet", ".json", ".tbl", ".db")


def iter_test_files():
    for dirpath, _dirs, files in os.walk(TEST_ROOT):
        for f in sorted(files):
            if f.endswith(".test"):
                yield os.path.join(dirpath, f)


def strip_line_comment(s):
    """Remove a trailing `-- …` SQL comment from one physical line, respecting
    single-quoted string literals so `'a--b'` is preserved. Multi-line records are
    joined into one line, so a per-line `--` left in place would comment out every
    following joined line — strip it while the comment still spans only its own line.
    """
    in_str = False
    i = 0
    while i < len(s):
        c = s[i]
        if c == "'":
            in_str = not in_str
        elif c == "-" and not in_str and i + 1 < len(s) and s[i + 1] == "-":
            return s[:i]
        i += 1
    return s


def normalize(lines):
    """Join a record's SQL lines into one normalized single-line statement."""
    parts = []
    for ln in lines:
        s = ln.rstrip("\n")
        if s.strip().startswith("#"):
            continue
        parts.append(strip_line_comment(s).strip())
    sql = " ".join(p for p in parts if p)
    sql = re.sub(r"\s+", " ", sql).strip()
    sql = sql.rstrip(";").strip()
    return sql


def extract_typed_records(path):
    """Yield `(is_query, ok, sql)` for each record in one .test file, in source order.

    `is_query` is True for a `query <types>` record, False for a `statement` record;
    `ok` is True only for `statement ok` (the records usable as setup DDL — `statement
    error`/`maybe` and query bodies are not). Preserving the record kind is what lets
    the caller both keep the flat selection and harvest each file's `CREATE` setup.
    """
    with open(path, "r", errors="replace") as fh:
        lines = fh.readlines()
    i, n = 0, len(lines)
    out = []
    while i < n:
        stripped = lines[i].strip()
        # Skip control lines that precede a record (onlyif/skipif) — the record
        # label follows; we let the loop reach it.
        head = stripped.split()
        if head and head[0] in ("statement", "query"):
            is_query = head[0] == "query"
            ok = (not is_query) and len(head) >= 2 and head[1] == "ok"
            i += 1
            body = []
            while i < n:
                cur = lines[i]
                cs = cur.strip()
                if cs == "----":  # query result separator: SQL ends here
                    # skip to blank line (end of result block)
                    while i < n and lines[i].strip() != "":
                        i += 1
                    break
                if cs == "":  # blank line: end of record
                    break
                body.append(cur)
                i += 1
            sql = normalize(body)
            if sql:
                out.append((is_query, ok, sql))
        else:
            i += 1
    return out


def extract_records(path):
    """Normalized SQL statements from one .test file, in source order (kind-agnostic —
    the flat selection weights statement and query bodies alike)."""
    return [sql for (_is_query, _ok, sql) in extract_typed_records(path)]


def is_setup_ddl(sql):
    """Whether `sql` is a concrete `CREATE` statement usable as prepare-time schema
    setup: a CREATE, single-statement, non-templated, and not reaching out to an
    on-disk file (which would fail on a fresh in-memory database)."""
    if not CREATE_HEAD.match(sql):
        return False
    if ";" in sql:  # single-statement only (execute_batch splits on ;)
        return False
    if any(m in sql for m in BAD_MARKERS):
        return False
    if PLACEHOLDER.search(sql):
        return False
    low = sql.lower()
    if any(m in low for m in DDL_FILE_REFS):
        return False
    return True


def ddl_meta(sql):
    """`(emit_bucket, dedup_key)` for a setup CREATE. Named objects dedup by
    `(kind, name)` so a file's drop/recreate keeps its final definition; other CREATEs
    (INDEX, SECRET, …) dedup by full text and sort after the objects they depend on."""
    m = CREATE_OBJ.match(sql)
    if not m:
        return (6, sql)
    kind = m.group(1).lower()
    name = m.group(2).replace('"', "").lower()
    return (CREATE_BUCKET[kind], (kind, name))


def ordered_setup(records):
    """Dedup a file's captured CREATEs (last definition of each key wins) and order them
    so dependencies (schema/type/sequence) precede tables and views/indexes follow, so
    a whole-file `execute_batch` provisions in one shot."""
    last = {}  # dedup_key -> (source_idx, bucket, sql); last occurrence wins
    for idx, (bucket, key, sql) in enumerate(records):
        last[key] = (idx, bucket, sql)
    items = sorted(last.values(), key=lambda t: (t[1], t[0]))  # (bucket, source order)
    return [sql for (_idx, _bucket, sql) in items]


def usable(sql):
    if not (5 <= len(sql) <= 400):
        return False
    if ";" in sql:  # single-statement only (DuckDB prepare executes all-but-last)
        return False
    if any(m in sql for m in BAD_MARKERS):
        return False
    if PLACEHOLDER.search(sql):  # {type}, {DATA_DIR}, … — sqllogictest templating
        return False
    # Must look like a statement (starts with a keyword-ish token).
    if not re.match(r"^[A-Za-z(\[]", sql):
        return False
    if sql.upper().startswith(("REQUIRE", "MODE ", "RESTART", "SLEEP", "STATEMENT")):
        return False
    return True


def families_of(sql):
    # Config statements (SET/PRAGMA/…) carry `=[...]` value lists that are not the
    # list-literal signature surface; never classify them as signature.
    if sql.upper().startswith(CONFIG_HEAD):
        return set()
    fams = set()
    for fam, rx in SIG:
        if rx.search(sql):
            fams.add(fam)
    return fams


def main():
    seen = OrderedDict()  # sql -> set(families)  (first occurrence wins)
    per_file_sig = Counter()
    per_file_gen = Counter()
    fam_used = Counter()

    sig_bucket = OrderedDict()
    gen_bucket = OrderedDict()

    # Per-file setup DDL (captured in source order; deduped + dependency-ordered at emit
    # time) and the source file each selected statement was first drawn from.
    file_ddl = OrderedDict()  # path -> [(bucket, dedup_key, sql)]
    sql_source_file = {}

    for path in iter_test_files():
        typed = extract_typed_records(path)
        for is_query, ok, sql in typed:
            if (not is_query) and ok and is_setup_ddl(sql):
                bucket, key = ddl_meta(sql)
                file_ddl.setdefault(path, []).append((bucket, key, sql))
        for _is_query, _ok, sql in typed:
            if not usable(sql) or sql in seen:
                continue
            fams = families_of(sql)
            if fams:
                # Admit only if at least one of its families is still under cap; then
                # charge all its families (balanced coverage across the nine).
                if all(fam_used[f] >= FAMILY_CAP for f in fams):
                    continue
                if per_file_sig[path] >= SIG_CAP_PER_FILE:
                    continue
                per_file_sig[path] += 1
                for f in fams:
                    fam_used[f] += 1
                seen[sql] = fams
                sig_bucket[sql] = fams
                sql_source_file[sql] = path
            else:
                if per_file_gen[path] >= GENERAL_CAP_PER_FILE:
                    continue
                per_file_gen[path] += 1
                seen[sql] = fams
                gen_bucket[sql] = fams
                sql_source_file[sql] = path

    # Compose: ALL signature statements first (the weighting), then general fill.
    ordered = list(sig_bucket.keys())
    for sql in gen_bucket.keys():
        if len(ordered) >= TARGET_TOTAL:
            break
        ordered.append(sql)

    fam_counts = Counter()
    for sql in ordered:
        for fam in seen[sql]:
            fam_counts[fam] += 1
    sig_total = len(sig_bucket)

    with open(sys.argv[1], "w") as out:
        for sql in ordered:
            out.write(sql + "\n")

    print(f"total={len(ordered)} signature={sig_total} general={len(ordered)-sig_total}", file=sys.stderr)
    for fam, c in sorted(fam_counts.items(), key=lambda kv: -kv[1]):
        print(f"  {fam:24} {c}", file=sys.stderr)

    # Optional grouped setup-driver artifact: the same selected queries, regrouped
    # under their source file with that file's concrete CREATE setup DDL. Markers
    # (`# file:`/`# setup`/`# query`) are unambiguous — no extracted statement starts
    # with `#`. The query lines are the exact same set as statements.sql (a coherence
    # test in the harness asserts it), only reordered by source file.
    if len(sys.argv) > 2:
        groups = OrderedDict()
        for sql in ordered:
            groups.setdefault(sql_source_file[sql], []).append(sql)
        n_ddl = 0
        with open(sys.argv[2], "w") as out:
            for path, queries in groups.items():
                out.write(f"# file: {os.path.relpath(path, DUCKDB)}\n")
                ddl = ordered_setup(file_ddl.get(path, []))
                if ddl:
                    out.write("# setup\n")
                    for stmt in ddl:
                        out.write(stmt + "\n")
                        n_ddl += 1
                out.write("# query\n")
                for q in queries:
                    out.write(q + "\n")
        print(
            f"grouped -> {sys.argv[2]}: files={len(groups)} setup_ddl={n_ddl} "
            f"queries={sum(len(v) for v in groups.values())}",
            file=sys.stderr,
        )


if __name__ == "__main__":
    main()
