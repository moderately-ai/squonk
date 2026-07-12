# SPDX-License-Identifier: PostgreSQL
#!/usr/bin/env python3
"""Extract the direct alternatives of PostgreSQL's top-level `stmt` production."""

import re
import sys


def main():
    grammar, output = sys.argv[1:3]
    text = open(grammar, encoding="utf-8").read()
    match = re.search(r"(?ms)^stmt:\s*(.*?)^\s*;\s*$", text)
    if match is None:
        raise SystemExit("top-level stmt production not found")
    productions = sorted(set(re.findall(r"\b[A-Za-z][A-Za-z0-9]+Stmt\b", match.group(1))))
    if not productions:
        raise SystemExit("top-level stmt production is empty")
    with open(output, "w", encoding="utf-8") as target:
        target.write("\n".join(productions) + "\n")
    print(f"stmt_productions={len(productions)}", file=sys.stderr)


if __name__ == "__main__":
    main()
