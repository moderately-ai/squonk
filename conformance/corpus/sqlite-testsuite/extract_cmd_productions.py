# SPDX-License-Identifier: CC0-1.0
#!/usr/bin/env python3
"""Extract the canonical top-level command families of SQLite's `cmd` production.

SQLite's grammar (`src/parse.y`) is a lemon grammar: unlike PostgreSQL's bison
`stmt:` alternatives (each a named `*Stmt` sub-production), the top-level statement
alternatives are the `cmd ::= <rhs>` rules, keyed by their leading terminal keywords
rather than by a named sub-production. This script is the SQLite analogue of the
pg-regress `extract_stmt_productions.py`: it resolves each `cmd ::=` alternative to a
canonical command name and emits the sorted, deduplicated inventory.

Deliberately EXCLUDED: `EXPLAIN` / `EXPLAIN QUERY PLAN`. They are not `cmd`
alternatives — the grammar makes them an `ecmd`-level prefix that wraps any `cmd`
(`ecmd ::= explain cmdx SEMI`), so they are statement modifiers over the inventory
below, not members of it. (PostgreSQL, by contrast, makes EXPLAIN a first-class
`ExplainStmt`; the two inventories differ because the two grammars structure EXPLAIN
differently, and this extractor is faithful to SQLite's own structure.)

The resolution table below is derived directly from `parse.y` and is the only
SQLite-specific input; it maps the handful of `cmd` alternatives that begin with a
non-terminal to the keyword sequence that non-terminal expands to. A `cmd` alternative
beginning with a terminal keeps that keyword (plus the object keyword for the two-word
CREATE/DROP/ALTER heads).
"""

import re
import sys

# Leading non-terminals a `cmd ::=` alternative can begin with, mapped to the command
# keywords they expand to (parse.y line refs are for SQLite 3.53.2):
#   create_table ::= createkw temp TABLE ...              (CREATE TABLE)
#   create_vtab  ::= createkw VIRTUAL TABLE ...           (CREATE VIRTUAL TABLE)
#   select       ::= WITH? selectnowith -> oneselect      (SELECT / VALUES)
#   insert_cmd   ::= INSERT orconf | REPLACE              (INSERT — folds the REPLACE spelling)
#   alter_add    ::= ALTER TABLE fullname ADD ...         (ALTER TABLE ... ADD COLUMN)
LEADING_NONTERMINAL = {
    "create_table": ["CREATE", "TABLE"],
    "create_vtab": ["CREATE", "VIRTUAL", "TABLE"],
    "select": ["SELECT"],
    "insert_cmd": ["INSERT"],
    "alter_add": ["ALTER", "TABLE"],
}
# The object keyword scanned for after a leading `createkw` (which is bare `CREATE`):
#   cmd ::= createkw temp VIEW ...          -> CREATE VIEW
#   cmd ::= createkw uniqueflag INDEX ...    -> CREATE INDEX
#   cmd ::= createkw trigger_decl BEGIN ...  -> CREATE TRIGGER (trigger_decl ::= temp TRIGGER ...)
CREATEKW_OBJECT = {"VIEW": "VIEW", "INDEX": "INDEX", "trigger_decl": "TRIGGER"}
# Terminal heads whose command name includes a second (object) keyword.
TWO_WORD_HEADS = {"DROP", "ALTER"}


def command_of(rhs):
    """Resolve one `cmd ::=` right-hand side to its canonical command name."""
    # Tokenize: drop lemon `(label)` bindings, keep the first side of a `A|B` token
    # class (e.g. `COMMIT|END` -> COMMIT), and drop the optional leading `with` CTE
    # prefix (`with ::= . | WITH wqlist | WITH RECURSIVE wqlist`).
    tokens = [t.split("(")[0].split("|")[0] for t in rhs.split()]
    if tokens and tokens[0] == "with":
        tokens = tokens[1:]
    if not tokens:
        raise SystemExit(f"empty cmd alternative: {rhs!r}")
    head = tokens[0]
    if head in LEADING_NONTERMINAL:
        return " ".join(LEADING_NONTERMINAL[head])
    if head == "createkw":
        for tok in tokens[1:]:
            if tok in CREATEKW_OBJECT:
                return f"CREATE {CREATEKW_OBJECT[tok]}"
        raise SystemExit(f"createkw alternative with no object keyword: {rhs!r}")
    if head in TWO_WORD_HEADS:
        return f"{head} {tokens[1]}"
    return head


def main():
    grammar, output = sys.argv[1:3]
    text = open(grammar, encoding="utf-8").read()
    # Each `cmd ::= <rhs>.` alternative; the rhs runs to the rule-terminating `.`
    # (lemon symbols never contain `.`, so `[^.]*` captures the full multi-line rhs
    # before the C action). `^cmd ` excludes `cmdx`/`cmdlist`.
    rules = re.findall(r"^cmd ::=\s*([^.]*)\.", text, re.M)
    if not rules:
        raise SystemExit("no `cmd ::=` alternatives found")
    commands = sorted({command_of(rhs) for rhs in rules})
    with open(output, "w", encoding="utf-8") as target:
        target.write("\n".join(commands) + "\n")
    print(f"cmd_alternatives={len(rules)} commands={len(commands)}", file=sys.stderr)


if __name__ == "__main__":
    main()
