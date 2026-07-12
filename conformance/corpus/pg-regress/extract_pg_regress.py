# SPDX-License-Identifier: PostgreSQL
#!/usr/bin/env python3
"""Extract a statement-level SQL corpus from PostgreSQL's src/test/regress/sql/*.sql
regression suite — the PG executable spec — for the spec-audit accept/reject sweep.

The regress `.sql` files are psql scripts, not plain SQL: they interleave psql
meta-commands (backslash), `COPY ... FROM STDIN` inline data blocks, `\\gset`/`\\gexec`
result constructs, and psql `:var` interpolation with real SQL, and a single statement
spans many lines terminated by `;` — but a `;` inside a dollar-quoted body, a string, or
a comment does NOT terminate. This extractor is a psql-aware statement splitter:

  python3 extract_pg_regress.py <regress_sql_dir> <checkout_root> statements.sql [--max-len N] [--count]

- Splits on top-level `;` respecting dollar-quoting (`$$`/`$tag$`, never `$1` params),
  single/double quotes (with `''` doubling and E-string `\\` escapes), line comments
  (`--`) and NESTED block comments (`/* /* */ */`).
- Strips psql meta-commands: a backslash at statement start skips the line; a backslash
  mid-statement (`\\gset`/`\\gexec`/`\\g`/`\\gx`/`\\;`/…) flushes the accumulated SQL as a
  statement and skips the rest of the line (the `\\g*` acts as a terminator).
- Skips `COPY ... FROM STDIN` inline data: the terminable `COPY` head is kept, the data
  rows up to and including the lone `\\.` terminator are dropped.
- Drops statements carrying psql `:'var'` / `:"var"` interpolation (not standalone SQL).

Output `statements.sql` is grouped under `# file:` markers (one source `.sql` per group,
provenance so gap families trace back), deduped globally (first file wins), one statement
per line, `;`-free. The flat view is every non-`#` non-blank line. Run once at vendoring
time; the output is committed. Not part of the build.
"""
import os
import re
import sys
from collections import OrderedDict

DOLLAR_OPEN = re.compile(r"\$([A-Za-z_][A-Za-z_0-9]*)?\$")
# psql variable interpolation that makes a statement non-standalone: :'x' and :"x".
PSQL_VAR = re.compile(r":['\"]")


def split_statements(text):
    """Yield each top-level SQL statement (without its terminating `;`) from one psql
    script, honouring dollar-quotes / strings / comments and stripping psql constructs."""
    lines = text.split("\n")
    buf = []            # accumulated chars of the current statement
    i = 0
    n = len(lines)
    # Cross-line lexical state.
    dollar_tag = None   # current open dollar-quote tag (str) or None
    in_squote = False   # inside a '...' string
    squote_e = False    # ... opened as an E'...' (backslash escapes)
    in_dquote = False   # inside a "..." quoted identifier
    block_depth = 0     # nested /* */ depth
    in_copy_data = False

    def at_top_level():
        return (
            dollar_tag is None
            and not in_squote
            and not in_dquote
            and block_depth == 0
        )

    def flush():
        stmt = "".join(buf).strip()
        buf.clear()
        return stmt

    while i < n:
        line = lines[i]
        i += 1

        if in_copy_data:
            if line.rstrip() == "\\.":
                in_copy_data = False
            continue

        # A backslash meta-command at a clean statement boundary: skip the whole line.
        if at_top_level() and "".join(buf).strip() == "" and line.lstrip().startswith("\\"):
            buf.clear()
            continue

        j = 0
        m = len(line)
        flushed = None
        while j < m:
            c = line[j]

            # --- inside a dollar-quoted body: only look for the matching close tag ---
            if dollar_tag is not None:
                if c == "$":
                    mo = DOLLAR_OPEN.match(line, j)
                    if mo and (mo.group(1) or "") == dollar_tag:
                        buf.append(mo.group(0))
                        j = mo.end()
                        dollar_tag = None
                        continue
                buf.append(c)
                j += 1
                continue

            # --- inside a single-quoted string ---
            if in_squote:
                if c == "'":
                    if j + 1 < m and line[j + 1] == "'":  # '' doubled quote
                        buf.append("''")
                        j += 2
                        continue
                    in_squote = False
                    squote_e = False
                    buf.append(c)
                    j += 1
                    continue
                if c == "\\" and squote_e and j + 1 < m:  # E-string backslash escape
                    buf.append(line[j:j + 2])
                    j += 2
                    continue
                buf.append(c)
                j += 1
                continue

            # --- inside a double-quoted identifier ---
            if in_dquote:
                if c == '"':
                    if j + 1 < m and line[j + 1] == '"':  # "" doubled
                        buf.append('""')
                        j += 2
                        continue
                    in_dquote = False
                buf.append(c)
                j += 1
                continue

            # --- inside a block comment (nested) ---
            if block_depth > 0:
                if c == "/" and j + 1 < m and line[j + 1] == "*":
                    block_depth += 1
                    buf.append("/*")
                    j += 2
                    continue
                if c == "*" and j + 1 < m and line[j + 1] == "/":
                    block_depth -= 1
                    buf.append("*/")
                    j += 2
                    continue
                buf.append(c)
                j += 1
                continue

            # --- top level ---
            if c == "-" and j + 1 < m and line[j + 1] == "-":  # line comment
                break  # rest of line is a comment
            if c == "/" and j + 1 < m and line[j + 1] == "*":
                block_depth += 1
                buf.append("/*")
                j += 2
                continue
            if c == "'":
                in_squote = True
                # E'' / e'' string? (the char before the quote is a standalone e/E)
                prev = line[j - 1] if j > 0 else (buf[-1][-1] if buf and buf[-1] else "")
                squote_e = prev in ("e", "E")
                buf.append(c)
                j += 1
                continue
            if c == '"':
                in_dquote = True
                buf.append(c)
                j += 1
                continue
            if c == "$":
                mo = DOLLAR_OPEN.match(line, j)
                if mo:
                    dollar_tag = mo.group(1) or ""
                    buf.append(mo.group(0))
                    j = mo.end()
                    continue
                buf.append(c)
                j += 1
                continue
            if c == "\\":  # psql meta-command mid-statement acts as a terminator
                flushed = flush()
                break
            if c == ";":
                flushed = flush()
                j += 1
                # A COPY ... FROM STDIN head opens an inline data block.
                if re.search(r"\bcopy\b.*\bfrom\s+stdin\b", flushed, re.I | re.S):
                    in_copy_data = True
                if flushed:
                    yield flushed
                flushed = None
                # keep scanning the rest of the line for further statements
                # (reset local flag so the trailing yield below does not double-emit)
                remainder = line[j:]
                line = remainder
                m = len(line)
                j = 0
                continue
            buf.append(c)
            j += 1

        if flushed is not None:
            if re.search(r"\bcopy\b.*\bfrom\s+stdin\b", flushed, re.I | re.S):
                in_copy_data = True
            if flushed:
                yield flushed
        else:
            # statement continues onto the next line
            buf.append("\n")


def normalize(stmt):
    """Collapse whitespace to single spaces (statements are stored one per line)."""
    return re.sub(r"\s+", " ", stmt).strip()


def usable(sql):
    if len(sql) < 5:
        return False
    if PSQL_VAR.search(sql):
        return False
    # Must start like a SQL statement (a keyword / paren), not leftover punctuation.
    if not re.match(r"^[A-Za-z(]", sql):
        return False
    return True


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    flags = [a for a in sys.argv[1:] if a.startswith("--")]
    sql_dir, checkout, out_path = args[0], args[1], args[2]
    count_only = "--count" in flags
    max_len = None
    for fl in flags:
        if fl.startswith("--max-len="):
            max_len = int(fl.split("=", 1)[1])

    seen = OrderedDict()   # sql -> source rel path (first file wins)
    per_file = OrderedDict()   # rel path -> [sql, ...]
    dropped_len = 0

    for name in sorted(os.listdir(sql_dir)):
        if not name.endswith(".sql"):
            continue
        path = os.path.join(sql_dir, name)
        rel = os.path.relpath(path, checkout)
        with open(path, "r", errors="replace") as fh:
            text = fh.read()
        for raw in split_statements(text):
            sql = normalize(raw)
            if not usable(sql):
                continue
            if max_len is not None and len(sql) > max_len:
                dropped_len += 1
                continue
            if sql in seen:
                continue
            seen[sql] = rel
            per_file.setdefault(rel, []).append(sql)

    print(f"statements={len(seen)} files={len(per_file)} dropped_over_maxlen={dropped_len}",
          file=sys.stderr)
    if count_only:
        return

    with open(out_path, "w") as out:
        for rel, stmts in per_file.items():
            out.write(f"# file: {rel}\n")
            for s in stmts:
                out.write(s + "\n")


if __name__ == "__main__":
    main()
