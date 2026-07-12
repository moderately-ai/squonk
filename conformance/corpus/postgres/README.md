<!-- SPDX-License-Identifier: MIT -->

# PostgreSQL Regression Corpus

This directory contains a small PostgreSQL-regression-derived corpus used by the
local conformance tests. Source material comes from the upstream PostgreSQL repository
at https://github.com/postgres/postgres/tree/master/src/test/regress/sql
at revision `4b0bf0788b066a4ca1d4f959566678e44ec93422` and is covered by the
PostgreSQL license.

`regress-supported.sql` contains statements inside the parser surface currently
mapped by the PostgreSQL structural oracle. These statements run through both
the structural parity test and the regenerable golden harness.

`regress-guide.sql` keeps nearby PostgreSQL regression statements that are not
supported yet. Each guide case names the upstream source line and the production
ticket that owns promotion into the supported corpus. The conformance tests keep
the guide machine-checked so unsupported statements are not dropped silently.

The corpus keeps supported surface forms even when PostgreSQL's raw parse tree
normalizes them. For example, `CROSS JOIN` is retained in the source and in
canonical rendering, while the neutral PG structural shape compares it as an
inner join with no constraint because that is what PostgreSQL exposes.

Unsupported nearby regression areas are represented by concrete cases in
`regress-guide.sql` and tracked by:

- DDL/DML structural mapping: `prod-pg-map-ddl-dml`
- Query `VALUES` rows containing `DEFAULT`: `prod-sql-values-default`
- Advanced expressions, CASE, function calls, row constructors, and casts using
  PostgreSQL shorthand syntax: `prod-sql-expr-case-functions`,
  `prod-sql-expr-postgres`, `prod-pg-map-expressions`
- Advanced joins, table alias column lists, LATERAL, and parenthesized join
  factors: `prod-sql-select-advanced-joins`, `prod-pg-map-joins`
