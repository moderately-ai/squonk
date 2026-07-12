# SPDX-License-Identifier: MIT
#!/usr/bin/env python3
"""Extract DuckDB's top-level `stmt` statement productions from the vendored
libpg_query grammar manifest.

DuckDB's parser is a fork of libpg_query. Unlike upstream PostgreSQL — where the
top-level `stmt:` alternatives are listed inline in `gram.y` — DuckDB factors the
alternative list out into `third_party/libpg_query/grammar/statements.list`, one
production per line. `scripts/generate_grammar.py` then materializes the bison rule
verbatim from that manifest:

    stmt_list = "stmt: " + "\n\t| ".join(statements) + "\n\t| /*EMPTY*/\n\t{ $$ = NULL; }\n"

so `statements.list` *is* the direct-alternative set of the top-level `stmt`
production (plus the empty statement, which carries no `*Stmt` node). This script
sorts and de-dups it into the pinned denominator — the DuckDB analogue of the
PostgreSQL `extract_stmt_productions.py` instrument.
"""

import sys


def main():
    manifest, output = sys.argv[1:3]
    with open(manifest, encoding="utf-8") as source:
        productions = sorted({line.strip() for line in source if line.strip()})
    if not productions:
        raise SystemExit("statements.list is empty")
    with open(output, "w", encoding="utf-8") as target:
        target.write("\n".join(productions) + "\n")
    print(f"stmt_productions={len(productions)}", file=sys.stderr)


if __name__ == "__main__":
    main()
