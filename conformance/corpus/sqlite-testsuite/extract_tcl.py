# SPDX-License-Identifier: CC0-1.0
#!/usr/bin/env python3
"""Extract a core-tranche SQLite SQL corpus (accepts + rejects) from the public-domain
TCL test suite (`test/*.test`) of the pinned upstream source tree.

Unlike the sqllogictest format (one statement per record), the TCL tests embed SQL in
`execsql`/`catchsql`/`do_execsql_test`/`do_catchsql_test` brace blocks amid heavy TCL
noise ($vars, [commands], string maps, loops). This extractor is deliberately
CONSERVATIVE: it takes only blocks whose body is PURE literal SQL — no `$` variable, no
`[...]` command substitution, no `\\` escape, no nested `{...}` TCL braces. A noisy block
is skipped (counted in the skip rate); a mangled statement is never emitted.

  python3 extract_tcl.py <sqlite_src_checkout> statements.sql statements_with_schema.sql rejects.sql [--cap N] [--reject-cap M] [--count]

- statements.sql          flat accepts (execsql + do_execsql_test bodies), one per line.
- rejects.sql             flat rejects (catchsql + do_catchsql_test bodies), one per line —
                          the over-acceptance differential's food. NB many are RUNTIME
                          rejects, not parse rejects; the sweep's reject classifier sorts that.
- statements_with_schema.sql  the same queries + rejects regrouped under their source
                          .test file with that file's pure `CREATE TABLE` setup DDL, so the
                          SQLite oracle binds names instead of binding-rejecting `FROM t`
                          over an empty database (# file: / # setup / # query / # reject).

Run once at vendoring time; outputs are committed. Not part of the build.
"""
import os
import re
import sys
from collections import OrderedDict

# Curated file families (longest-prefix-first so `index`<-`indexexpr`, `with`<-`without`).
# The ticket's core list (select/where/expr/join/orderby/limit/cte(with)/window/insert/
# update/delete/create/index/trigger/view/pragma/altertab) extended day-scale with the
# adjacent statement surface (subquery/distinct/collate/upsert/returning/conflict/cast/
# between/like/func/check/rowid/autoinc/default/attach/vacuum/reindex/analyze/without_rowid).
FAMILY_PREFIXES = [
    "altertab", "without_rowid", "indexexpr", "indexedby", "orderby", "subquery",
    "distinct", "collate", "returning", "conflict", "between", "trigger", "pragma",
    "reindex", "analyze", "autoinc", "default", "vacuum", "attach", "upsert", "window",
    "select", "insert", "update", "delete", "createtab", "check", "index", "where",
    "expr", "join", "limit", "view", "cast", "like", "func", "rowid", "with",
]

KW_RE = re.compile(r"(?<![\w.])(do_execsql_test|do_catchsql_test|execsql|catchsql)(?![\w])")
WORD_RE = re.compile(r"[A-Za-z_][A-Za-z_0-9]*")
IMPURE = ("$", "[", "]", "\\")

CREATE_TABLE = re.compile(r"^\s*CREATE\s+(?:TEMP(?:ORARY)?\s+)?TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?"
                          r'("?[A-Za-z_][A-Za-z_0-9.]*"?)', re.I)


def family_of(fname):
    base = fname[:-5] if fname.endswith(".test") else fname
    for fam in FAMILY_PREFIXES:
        if base.startswith(fam):
            return fam
    return None


def iter_test_files(test_root):
    for f in sorted(os.listdir(test_root)):
        if not f.endswith(".test"):
            continue
        fam = family_of(f)
        if fam is not None:
            yield fam, os.path.join(test_root, f)


def find_brace_block(text, start):
    """From `start`, locate the first `{...}` SQL body. Returns (body, max_depth, end_idx)
    or None if the call is not a simple brace-delimited body (quoted body, comment, or a
    stray construct before the brace — all conservatively skipped)."""
    i, n = start, len(text)
    while i < n:
        c = text[i]
        if c == "{":
            break
        if c == '"':          # quoted (substitution-prone) body — skip
            return None
        if c == "#":          # comment before the brace — skip
            return None
        if c == "\n":
            j = i + 1
            while j < n and text[j] in " \t":
                j += 1
            if j < n and text[j] == "{":
                i = j
                break
            return None
        i += 1
    if i >= n or text[i] != "{":
        return None
    depth, max_depth, j = 0, 0, i
    body_start = i + 1
    while j < n:
        c = text[j]
        if c == "\\":
            j += 2
            continue
        if c == "{":
            depth += 1
            max_depth = max(max_depth, depth)
        elif c == "}":
            depth -= 1
            if depth == 0:
                return text[body_start:j], max_depth, j
        j += 1
    return None


def is_pure(body, max_depth):
    if max_depth > 1:
        return False
    return not any(m in body for m in IMPURE)


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


def split_statements(body):
    """Split a pure SQL body into individual statements at top-level `;`, keeping
    CREATE TRIGGER BEGIN...END bodies (with nested CASE...END) intact."""
    stmts = []
    i, n = 0, len(body)
    cur_start = 0
    in_s = in_d = False
    seen_create = seen_trigger = in_body = False
    block_depth = 0

    def reset():
        nonlocal seen_create, seen_trigger, in_body, block_depth
        seen_create = seen_trigger = in_body = False
        block_depth = 0

    while i < n:
        c = body[i]
        if in_s:
            if c == "'":
                if i + 1 < n and body[i + 1] == "'":
                    i += 2
                    continue
                in_s = False
            i += 1
            continue
        if in_d:
            if c == '"':
                if i + 1 < n and body[i + 1] == '"':
                    i += 2
                    continue
                in_d = False
            i += 1
            continue
        if c == "'":
            in_s = True
            i += 1
            continue
        if c == '"':
            in_d = True
            i += 1
            continue
        if c == "-" and i + 1 < n and body[i + 1] == "-":
            j = body.find("\n", i)
            i = n if j == -1 else j
            continue
        if c.isalpha() or c == "_":
            m = WORD_RE.match(body, i)
            if m is None:  # non-ASCII letter (inside no string) — skip one char
                i += 1
                continue
            w = m.group(0).upper()
            i = m.end()
            if w == "CREATE":
                seen_create = True
            elif w == "TRIGGER" and seen_create:
                seen_trigger = True
            elif w == "BEGIN" and seen_trigger:
                in_body = True
                block_depth += 1
            elif w == "CASE" and in_body:
                block_depth += 1
            elif w == "END" and in_body:
                block_depth -= 1
                if block_depth == 0:
                    in_body = False
            continue
        if c == ";" and not in_body:
            stmts.append(body[cur_start:i])
            i += 1
            cur_start = i
            reset()
            continue
        i += 1
    if cur_start < n:
        stmts.append(body[cur_start:n])
    return stmts


def normalize(s):
    parts = []
    for line in s.split("\n"):
        line = strip_line_comment(line).strip()
        if line:
            parts.append(line)
    sql = " ".join(parts)
    sql = re.sub(r"\s+", " ", sql).strip()
    sql = sql.rstrip(";").strip()
    return sql


def usable(sql):
    if not (5 <= len(sql) <= 400):
        return False
    if not re.match(r"^[A-Za-z(]", sql):
        return False
    # A top-level `;` only survives split inside a trigger body; reject a stray `;` in a
    # non-trigger statement as a defensive guard against mis-splits.
    if ";" in sql and "TRIGGER" not in sql.upper():
        return False
    if sql.upper().startswith(("EXPLAIN QUERY PLAN", "EXPLAIN")):
        return False  # EXPLAIN wraps another statement; measure the inner grammar directly
    return True


def is_setup_ddl(sql):
    """A pure `CREATE TABLE` (non-virtual) the setup driver can provision so queries bind.
    INDEX/VIEW/TRIGGER/VIRTUAL-TABLE DDL stays in the measured query surface (they are gap
    food, and provisioning them risks batch failure on a missing base object/module)."""
    m = CREATE_TABLE.match(sql)
    if not m:
        return False
    if re.match(r"^\s*CREATE\s+(?:TEMP(?:ORARY)?\s+)?VIRTUAL", sql, re.I):
        return False
    if ";" in sql:  # multi-statement safety
        return False
    # Schema-qualified names (`aux.t1`, `main.t1`) cannot provision on a bare in-memory
    # DB (unknown attached db) and collide with the unqualified form — keep them in the
    # measured query surface, out of setup.
    if "." in table_name(sql):
        return False
    return True


def table_name(sql):
    m = CREATE_TABLE.match(sql)
    return m.group(1).replace('"', "").lower() if m else ""


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    flags = [a for a in sys.argv[1:] if a.startswith("--")]
    checkout = args[0]
    test_root = os.path.join(checkout, "test")
    count_only = "--count" in flags
    cap = reject_cap = None
    for fl in flags:
        if fl.startswith("--cap="):
            cap = int(fl.split("=", 1)[1])
        if fl.startswith("--reject-cap="):
            reject_cap = int(fl.split("=", 1)[1])

    seen_accept = OrderedDict()   # sql -> path
    seen_reject = OrderedDict()   # sql -> path
    file_ddl = OrderedDict()      # path -> OrderedDict(name -> create-table sql)
    per_file_accept, per_file_reject = {}, {}
    per_fam_accept, per_fam_reject = {}, {}
    kw_total = kw_nobrace = blocks_pure = blocks_impure = 0

    for fam, path in iter_test_files(test_root):
        with open(path, "r", errors="replace") as fh:
            text = fh.read()
        rel = "test/" + os.path.basename(path)
        for m in KW_RE.finditer(text):
            kw = m.group(1)
            kind = "accept" if kw in ("execsql", "do_execsql_test") else "reject"
            kw_total += 1
            found = find_brace_block(text, m.end())
            if found is None:  # quoted/substitution body or stray construct — skipped
                kw_nobrace += 1
                continue
            body, max_depth, _end = found
            if not is_pure(body, max_depth):
                blocks_impure += 1
                continue
            blocks_pure += 1
            for raw in split_statements(body):
                sql = normalize(raw)
                if not usable(sql):
                    continue
                # Harvest pure CREATE TABLE setup from accept-side blocks.
                if kind == "accept" and is_setup_ddl(sql):
                    name = table_name(sql)
                    if name:
                        file_ddl.setdefault(path, OrderedDict())[name] = sql
                if sql in seen_accept or sql in seen_reject:
                    continue
                if kind == "accept":
                    if cap is not None and per_file_accept.get(path, 0) >= cap:
                        continue
                    per_file_accept[path] = per_file_accept.get(path, 0) + 1
                    per_fam_accept[fam] = per_fam_accept.get(fam, 0) + 1
                    seen_accept[sql] = path
                else:
                    if reject_cap is not None and per_file_reject.get(path, 0) >= reject_cap:
                        continue
                    per_file_reject[path] = per_file_reject.get(path, 0) + 1
                    per_fam_reject[fam] = per_fam_reject.get(fam, 0) + 1
                    seen_reject[sql] = path

    skipped = kw_nobrace + blocks_impure
    skip_rate = (skipped / kw_total * 100) if kw_total else 0.0
    print(f"blocks: keyword_calls={kw_total} pure={blocks_pure} "
          f"impure(subst/nested)={blocks_impure} nobrace(quoted/other)={kw_nobrace} "
          f"skipped={skipped} ({skip_rate:.1f}% of calls skipped)", file=sys.stderr)
    print(f"accepts={len(seen_accept)} rejects={len(seen_reject)}", file=sys.stderr)
    print("  per-family accept/reject:", file=sys.stderr)
    for fam in sorted(set(per_fam_accept) | set(per_fam_reject)):
        print(f"    {fam:14} accept={per_fam_accept.get(fam,0):5} reject={per_fam_reject.get(fam,0):5}",
              file=sys.stderr)

    if count_only:
        return

    with open(args[1], "w") as out:
        for sql in seen_accept:
            out.write(sql + "\n")

    groups = OrderedDict()
    for sql, path in seen_accept.items():
        groups.setdefault(path, {"query": [], "reject": []})["query"].append(sql)
    for sql, path in seen_reject.items():
        groups.setdefault(path, {"query": [], "reject": []})["reject"].append(sql)
    n_ddl = 0
    with open(args[2], "w") as out:
        for path, buckets in groups.items():
            out.write(f"# file: test/{os.path.basename(path)}\n")
            ddl = list(file_ddl.get(path, {}).values())
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

    with open(args[3], "w") as out:
        for sql in seen_reject:
            out.write(sql + "\n")
    print(f"grouped files={len(groups)} setup_ddl={n_ddl}", file=sys.stderr)


if __name__ == "__main__":
    main()
