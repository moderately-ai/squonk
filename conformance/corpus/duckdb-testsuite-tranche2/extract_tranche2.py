# SPDX-License-Identifier: MIT
#!/usr/bin/env python3
"""Extract a DuckDB tranche-2 SQL corpus (accepts + rejects) from the vendored
test suite's sqllogictest-style `.test` files.

The tranche-2 sibling of `conformance/corpus/duckdb-testsuite/extract_core.py`: same
recipe and filters, applied to the DML/DDL/copy/functions/pivot/prepared/optimizer/
pragma/tpc `test/sql` directories the core tranche (select/join/subquery/aggregate/
window/cte/order/limit/types) did not cover — to measure the residual grammar-gap
inventory over the rest of the executable spec. Three artifacts:

  python3 extract_tranche2.py <duckdb_checkout> statements.sql statements_with_schema.sql rejects.sql [--cap N] [--reject-cap M] [--count]

- statements.sql          flat accepts (statement ok + query bodies), one per line.
- rejects.sql             flat rejects (statement error bodies), one per line — the
                          over-acceptance differential's food.
- statements_with_schema.sql  the same queries + rejects regrouped under their source
                          .test file with that file's concrete CREATE setup DDL, so the
                          DuckDB oracle binds names instead of binding-rejecting FROM t
                          over an empty database (# file: / # setup / # query / # reject).

Run once at vendoring time; outputs are committed. Not part of the build.
"""
import os
import re
import sys
from collections import OrderedDict

CORE_DIRS = ["insert", "update", "delete", "merge", "create", "alter", "index", "constraints", "copy", "storage", "attach", "function", "pivot", "prepared", "optimizer", "pragma", "settings", "tpch", "tpcds"]

# Template / non-SQL markers that make a line unusable as a standalone statement.
BAD_MARKERS = ("${", "<FILE>", "__TEST_DIR__", "concurrentloop", "\\x", "{DATA_DIR}",
               "{TEMP_DIR}", "{BIG_DIR}", "{type}", "[INVALID]")
PLACEHOLDER = re.compile(r"\{[A-Za-z_][A-Za-z_0-9]*\}")  # {type}, {DATA_DIR}, …

CREATE_HEAD = re.compile(r"^\s*CREATE\b", re.I)
CREATE_OBJ = re.compile(
    r"^\s*CREATE\s+(?:OR\s+REPLACE\s+)?(?:TEMP(?:ORARY)?\s+)?"
    r"(TABLE|VIEW|TYPE|SCHEMA|SEQUENCE|MACRO|FUNCTION)\s+(?:IF\s+NOT\s+EXISTS\s+)?"
    r"([A-Za-z_0-9\".]+)",
    re.I,
)
CREATE_BUCKET = {"schema": 0, "type": 1, "sequence": 2, "table": 3,
                 "view": 4, "macro": 5, "function": 5}
DDL_FILE_REFS = ("read_csv", "read_parquet", "read_json", "read_ndjson", "'data/",
                 "'test/", ".csv", ".parquet", ".json", ".tbl", ".db")


def iter_test_files(test_root):
    for d in CORE_DIRS:
        base = os.path.join(test_root, d)
        for dirpath, _dirs, files in os.walk(base):
            for f in sorted(files):
                if f.endswith(".test"):
                    yield os.path.join(dirpath, f)


def strip_line_comment(s):
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
    """Yield (kind, sql) for each record in one .test file, in source order.
    kind is 'ok' (statement ok), 'error' (statement error), 'query', or 'other'."""
    with open(path, "r", errors="replace") as fh:
        lines = fh.readlines()
    i, n = 0, len(lines)
    out = []
    while i < n:
        stripped = lines[i].strip()
        head = stripped.split()
        if head and head[0] in ("statement", "query"):
            if head[0] == "query":
                kind = "query"
            elif len(head) >= 2 and head[1] == "ok":
                kind = "ok"
            elif len(head) >= 2 and head[1] == "error":
                kind = "error"
            else:
                kind = "other"  # statement maybe, etc.
            i += 1
            body = []
            while i < n:
                cur = lines[i]
                cs = cur.strip()
                if cs == "----":  # query result separator: SQL ends here
                    while i < n and lines[i].strip() != "":
                        i += 1
                    break
                if cs == "":  # blank line: end of record
                    break
                body.append(cur)
                i += 1
            sql = normalize(body)
            if sql:
                out.append((kind, sql))
        else:
            i += 1
    return out


def is_setup_ddl(sql):
    if not CREATE_HEAD.match(sql):
        return False
    if ";" in sql:
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
    m = CREATE_OBJ.match(sql)
    if not m:
        return (6, sql)
    kind = m.group(1).lower()
    name = m.group(2).replace('"', "").lower()
    return (CREATE_BUCKET[kind], (kind, name))


def ordered_setup(records):
    last = {}
    for idx, (bucket, key, sql) in enumerate(records):
        last[key] = (idx, bucket, sql)
    items = sorted(last.values(), key=lambda t: (t[1], t[0]))
    return [sql for (_idx, _bucket, sql) in items]


def usable(sql):
    if not (5 <= len(sql) <= 400):
        return False
    if ";" in sql:
        return False
    if any(m in sql for m in BAD_MARKERS):
        return False
    if PLACEHOLDER.search(sql):
        return False
    if not re.match(r"^[A-Za-z(\[]", sql):
        return False
    if sql.upper().startswith(("REQUIRE", "MODE ", "RESTART", "SLEEP", "STATEMENT")):
        return False
    return True


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    flags = [a for a in sys.argv[1:] if a.startswith("--")]
    checkout = args[0]
    test_root = os.path.join(checkout, "test", "sql")
    count_only = "--count" in flags
    cap = None
    reject_cap = None
    for fl in flags:
        if fl.startswith("--cap="):
            cap = int(fl.split("=", 1)[1])
        if fl.startswith("--reject-cap="):
            reject_cap = int(fl.split("=", 1)[1])

    seen_accept = OrderedDict()   # sql -> source path
    seen_reject = OrderedDict()   # sql -> source path
    file_ddl = OrderedDict()      # path -> [(bucket, key, sql)]
    per_file_accept = {}
    per_file_reject = {}
    per_dir_accept = {}
    per_dir_reject = {}

    for path in iter_test_files(test_root):
        rel = os.path.relpath(path, checkout)
        top = rel.split(os.sep)[2]  # test/sql/<dir>/...
        typed = extract_typed_records(path)
        # Harvest CREATE setup DDL from statement-ok records.
        for kind, sql in typed:
            if kind == "ok" and is_setup_ddl(sql):
                bucket, key = ddl_meta(sql)
                file_ddl.setdefault(path, []).append((bucket, key, sql))
        for kind, sql in typed:
            if not usable(sql):
                continue
            if kind in ("ok", "query"):
                if sql in seen_accept or sql in seen_reject:
                    continue
                if cap is not None and per_file_accept.get(path, 0) >= cap:
                    continue
                per_file_accept[path] = per_file_accept.get(path, 0) + 1
                per_dir_accept[top] = per_dir_accept.get(top, 0) + 1
                seen_accept[sql] = path
            elif kind == "error":
                if sql in seen_accept or sql in seen_reject:
                    continue
                if reject_cap is not None and per_file_reject.get(path, 0) >= reject_cap:
                    continue
                per_file_reject[path] = per_file_reject.get(path, 0) + 1
                per_dir_reject[top] = per_dir_reject.get(top, 0) + 1
                seen_reject[sql] = path

    print(f"accepts={len(seen_accept)} rejects={len(seen_reject)}", file=sys.stderr)
    print("  per-dir accepts:", file=sys.stderr)
    for d in CORE_DIRS:
        print(f"    {d:12} accept={per_dir_accept.get(d,0):5} reject={per_dir_reject.get(d,0):5}", file=sys.stderr)

    if count_only:
        return

    # Flat accepts.
    with open(args[1], "w") as out:
        for sql in seen_accept:
            out.write(sql + "\n")
    # Grouped setup-driver artifact (accepts + rejects per source file).
    groups = OrderedDict()
    for sql, path in seen_accept.items():
        groups.setdefault(path, {"query": [], "reject": []})["query"].append(sql)
    for sql, path in seen_reject.items():
        groups.setdefault(path, {"query": [], "reject": []})["reject"].append(sql)
    n_ddl = 0
    with open(args[2], "w") as out:
        for path, buckets in groups.items():
            out.write(f"# file: {os.path.relpath(path, checkout)}\n")
            ddl = ordered_setup(file_ddl.get(path, []))
            if ddl:
                out.write("# setup\n")
                for stmt in ddl:
                    out.write(stmt + "\n")
                    n_ddl += 1
            if buckets["query"]:
                out.write("# query\n")
                for q in buckets["query"]:
                    out.write(q + "\n")
            if buckets["reject"]:
                out.write("# reject\n")
                for q in buckets["reject"]:
                    out.write(q + "\n")
    # Flat rejects.
    with open(args[3], "w") as out:
        for sql in seen_reject:
            out.write(sql + "\n")
    print(f"grouped files={len(groups)} setup_ddl={n_ddl}", file=sys.stderr)


if __name__ == "__main__":
    main()
