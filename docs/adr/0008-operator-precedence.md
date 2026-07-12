# ADR-0008: Operator precedence — one binding-power table

- **Status:** Accepted (2026-06-26)
- **Atoms:** A19, A20

## Context

Operator precedence is where the prior art's **highest-severity bug** lived: precedence lived in two parallel matches kept in sync by a *runtime panic*, dialect `parse_infix` hooks *ignored* the precedence handed to them (so `a DIV b + c` mis-bound), and two different numbering schemes coexisted.

## Decision

**One `const` binding-power table** — `(operator) → (left_bp, right_bp, Assoc)` — in `squonk-ast`, feeding **both** directions:
1. the **Pratt parser**, which recurses at the operator's *own* right binding power (there is no hook *parameter* to ignore, so the mis-bind is structurally impossible);
2. **render-time parenthesization** (Calcite `leftPrec`/`rightPrec`), so `parse(render(x))` round-trips by construction and the fully-parenthesized render mode (ADR-0010) is a real precedence oracle.

Parens are **derived at render, not stored** (no `Expr::Nested` paren mechanism; an optional off-by-default fidelity marker exists for byte-faithful round-trip). Explicit source parentheses still act as a parse-time grouping barrier: `a < b < c` is rejected when comparison is `NonAssoc`, but `(a < b) < c` and `a < (b < c)` are valid nested comparison ASTs whose required parentheses are reintroduced by render-time parenthesization. `Assoc { Left, Right, NonAssoc }` is **per-operator, per-dialect** data — associativity genuinely differs (PG/standard comparison is `NonAssoc` → `a < b < c` is a clean `ParseError`; MySQL is `Left` → the same source means `(a < b) < c`; a future SQLite preset would inherit the same delta). One numbering scheme; dialects supply relative deltas as `const` data. Extension operators return their bp through the typed hook, so they honour precedence too.

## Consequences

- Kills all three prior-art failures at once: the mis-bind (structural), the dual-match desync (one source of truth → no runtime sync-panic), the two numbering schemes.
- **Performant and statically safe:** the lookup is a single `.rodata` const consult (jump-table `match` or array index, const-folded under `Parser<D>` monomorphization). **Exhaustiveness is compiler-enforced** via a `match` over the operator enum (a missing operator is a non-exhaustive-match build error). The residual runtime checks (`NonAssoc`, unknown operator) are correct *input validation* → `ParseError`, not panic.
- Spike-validate the bp encoding maps onto both Pratt and the renderer before both depend on it. Benchmark backlog #7 (`match` vs codegen'd array) — **closed 2026-07-08: keep the `match`**; the measured outcome, rationale, and resurrection path live in `docs/performance.md` (the live A/B harness was retired with them).

## Amendment (2026-06-27): set-operation precedence is in scope

Set operations (`UNION` / `INTERSECT` / `EXCEPT`) are a precedence concern, not merely a list-folding one: SQL and PostgreSQL rank `INTERSECT` **above** `UNION`/`EXCEPT`, with left-associativity within a precedence level — so `a UNION b INTERSECT c` means `a UNION (b INTERSECT c)`. The M1 parser originally folded set operators purely left-associatively in a `parse_set_expr` loop that consulted no binding-power table, producing the wrong `(a UNION b) INTERSECT c`; the PostgreSQL differential fuzz loop (ADR-0015) surfaced this as a structural divergence. It has since been fixed — `parse_set_expr_bp` (`crates/squonk/src/parser/query.rs`) is a precedence climb reading `self.features().set_operation_binding_power(&op)` against the `SetOperationBindingPowers` table, so set-op precedence now lives in the one binding-power table like every other operator.

This is precisely the failure this ADR exists to prevent — a precedence decision made in a *second place*, diverging from the single source of truth — so the "one binding-power table" discipline is hereby declared to **cover set operators as well**:

- Set-operator precedence/associativity is the same kind of `const`, per-dialect data as expression operators (a parallel table keyed by `SetOperator`, since set operations combine query bodies (`SetExpr`) rather than `Expr`).
- The set-operation parser becomes a **precedence climb** driven by that data (not a hand-written left-fold), so a mixed chain binds correctly by construction.
- Set-operation **render parenthesization is derived from the same data**, keeping `parse(render(x))` a real oracle for set-op grouping exactly as for expressions.

The discipline is what generalizes (data-driven precedence feeding both parse and render), not necessarily one literal `match`. Landed via `prod-adr-precedence-and-setops` (done); the data-driven climb is documented inline at `parse_set_expr_bp`.

## Interconnects

- code: `crates/squonk-ast/src/precedence/mod.rs` — `BindingPowerTable`, `SetOperationBindingPowerTable`; `crates/squonk/src/parser/query.rs` — `parse_set_expr_bp`
- invariant: precedence lives in one binding-power table feeding both parse and render; there is no `Expr::Nested` paren node, and set-operator precedence is read from `set_operation_binding_power`, never a second hand-rolled fold.
- xtask: `cargo xtask precedence`.

## References

Atoms A19–A20. matklad's Pratt parsing; Calcite render-time parenthesization. Set-op precedence amendment surfaced by the ADR-0015 differential fuzz loop (`prod-fuzz-differential-loop`), tracked by `prod-adr-precedence-and-setops`.
