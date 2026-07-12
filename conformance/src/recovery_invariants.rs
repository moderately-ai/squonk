// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Whole-tree invariant coverage for the recovering parse path
//! ([`parse_recovering`]).
//!
//! The invariant walkers (`spans::assert_parsed_span_invariants`, the `NodeIdWalk`
//! uniqueness check, and symbol resolvability) historically only ever swept trees
//! from the fail-fast `parse_with`, never the recovering path — yet recovery resyncs
//! at each `;` reusing one `Parser`, so its NodeId counter, source spans, and
//! interner all cross the resync boundary. A bug there (a counter reset, a span from
//! statement N drawn against the wrong offset, a symbol interned from a half-parsed
//! broken statement then left dangling) would be invisible: the surviving statements
//! each look individually fine.
//!
//! Recovery is corruption-resistant *by construction* — the NodeId counter is
//! monotonic and the interner is append-only across resyncs (ADR-0002 node identity,
//! ADR-0005 recovery), so these checks are a **regression net**, not a bug hunt: they
//! assert the whole-tree invariants continue to hold over the recovered `Parsed`, so
//! a future change that breaks them across a resync fails loudly here. A finding would
//! be a headline surprise, not the expected outcome.

use std::collections::HashSet;

use squonk::dialect::{Ansi, Lenient, MySql, Postgres};
use squonk::{Dialect, Parsed, parse_recovering};
use squonk_ast::generated::NodeIdWalk;
use squonk_ast::generated::visit::{self, Visit};
use squonk_ast::{Expr, FunctionArg, Ident, NamedOperatorExpr, NoExt, ParameterKind, Resolver};
use squonk_ast::{Statement, Symbol};

use crate::spans;

/// Matches `fuzz::MAX_PARSE_INPUT_BYTES`: a per-iteration budget, not a stress size.
/// Well under `u32::MAX`, so `parse_recovering`'s streaming setup never fails on an
/// in-bounds buffer (its only error path is the `u32`-bytes guard).
const MAX_RECOVER_INPUT_BYTES: usize = 65536;

/// Seed + replay corpus for the recovery-invariants target.
///
/// Broken scripts whose recovered partial trees must hold every whole-tree invariant;
/// they double as libFuzzer seeds so the mutator starts from real resync shapes. The
/// stable `recover_invariants_replays_committed_inputs` test replays them without
/// nightly. Recovery is panic-free and invariant-preserving by construction, so no
/// entry here is expected to fail — a fuzz-found regression would land beside a fix.
pub const RECOVER_INVARIANTS_REPLAYS: &[&[u8]] = &[
    // valid; broken; valid — the canonical resync-across-a-gap shape.
    b"SELECT alpha; FROM x; SELECT beta",
    // A grammar error followed by a fresh lexical fault while resyncing.
    b"SELECT alpha; ) 'unterminated",
    // Only empty statements: no survivors, no ids, still must not panic.
    b";;;",
    // Several broken statements interleaved with survivors across many resyncs.
    b"SELECT a; FROM; SELECT b; ); SELECT c",
    // Empty input: zero statements, zero errors, trivially invariant-holding.
    b"",
];

/// Feed raw bytes to [`parse_recovering`] under every built-in dialect and assert the
/// recovered partial tree holds the whole-tree invariants: no panic, unique nonzero
/// NodeIds across the whole tree, non-synthetic in-bounds spans, and every surviving
/// symbol resolvable in the root's interner.
///
/// Drops invalid UTF-8 and oversized buffers exactly as [`fuzz::parse_no_panic`]
/// does; those are not SQL inputs. All four dialects run because each carries its own
/// lexical surface (backticks/`#` for MySQL, the permissive union for Lenient), so the
/// resync path meets different token streams under each.
///
/// [`fuzz::parse_no_panic`]: crate::fuzz::parse_no_panic
pub fn recover_invariants(input: &[u8]) {
    if input.len() > MAX_RECOVER_INPUT_BYTES {
        return;
    }
    let Ok(src) = std::str::from_utf8(input) else {
        return;
    };

    assert_recovering_invariants(src, Ansi);
    assert_recovering_invariants(src, Postgres);
    assert_recovering_invariants(src, MySql);
    assert_recovering_invariants(src, Lenient);
}

/// Recover `src` under `dialect` and run the three whole-tree invariant walkers over
/// the surviving statements. Generic over any built-in dialect (all carry
/// `Ext = NoExt`, which the walkers are fixed to).
fn assert_recovering_invariants<D: Dialect<Ext = NoExt>>(src: &str, dialect: D) {
    // The only error path is the `u32`-bytes streaming guard, unreachable for an
    // in-bounds buffer; treat a setup error as a non-input rather than a failure.
    let Ok(recovered) = parse_recovering(src, dialect) else {
        return;
    };
    let parsed = recovered.parsed();
    spans::assert_parsed_span_invariants(parsed);
    assert_unique_node_ids(parsed.statements());
    assert_symbols_resolve(parsed);
}

/// Assert every id-bearing node across the *whole* recovered tree carries a unique,
/// nonzero NodeId. Uniqueness is checked across all surviving statements together —
/// not per statement — because they share one monotonic counter across resyncs, so a
/// counter reset or reuse at a resync boundary would collide two statements' ids here.
fn assert_unique_node_ids(statements: &[Statement<NoExt>]) {
    let mut walk = NodeIdWalk::default();
    for statement in statements {
        walk.visit_statement(statement);
    }
    let mut seen = HashSet::with_capacity(walk.metas.len());
    for meta in &walk.metas {
        assert!(
            meta.node_id.as_u32() != 0,
            "recovered tree carried a zero node id",
        );
        assert!(
            seen.insert(meta.node_id),
            "recovered tree reused node id {} across a resync boundary — the \
             monotonic counter was reset or a placeholder id survived",
            meta.node_id.as_u32(),
        );
    }
}

/// Assert every raw [`Symbol`] the recovered statements carry resolves in the root's
/// interner — no dangling symbol interned from a partially-parsed broken statement.
fn assert_symbols_resolve(parsed: &Parsed) {
    let mut checker = SymbolResolvability {
        resolver: parsed.resolver(),
    };
    for statement in parsed.statements() {
        checker.visit_statement(statement);
    }
}

/// Walks the recovered tree asserting each raw `Symbol` resolves. Overrides the same
/// five raw-`Symbol` leaf sites as `shared_interner::SymbolCollector` (every other
/// symbol rides an [`Ident`], which the generated walk descends into); kept a focused
/// local copy so this regression net stays self-contained in the recovery module.
struct SymbolResolvability<'a> {
    resolver: &'a dyn Resolver,
}

impl SymbolResolvability<'_> {
    fn check(&self, sym: Symbol) {
        assert!(
            self.resolver.try_resolve(sym).is_some(),
            "recovered tree carried symbol {} absent from the append-only interner",
            sym.as_u32(),
        );
    }
}

impl<'ast> Visit<'ast, NoExt> for SymbolResolvability<'_> {
    fn visit_ident(&mut self, node: &'ast Ident) {
        self.check(node.sym);
        visit::walk_ident(self, node);
    }

    fn visit_function_arg(&mut self, node: &'ast FunctionArg<NoExt>) {
        if let Some(name) = node.name {
            self.check(name);
        }
        visit::walk_function_arg(self, node);
    }

    fn visit_named_operator_expr(&mut self, node: &'ast NamedOperatorExpr<NoExt>) {
        self.check(node.op);
        visit::walk_named_operator_expr(self, node);
    }

    fn visit_parameter_kind(&mut self, node: &'ast ParameterKind) {
        if let ParameterKind::Named { name, .. } = node {
            self.check(*name);
        }
        visit::walk_parameter_kind(self, node);
    }

    fn visit_expr(&mut self, node: &'ast Expr<NoExt>) {
        if let Expr::SessionVariable { name, .. } = node {
            self.check(*name);
        }
        visit::walk_expr(self, node);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deterministic broken scripts: valid;broken;valid patterns, broken-first,
    /// interleaved multi-break, and an unterminated tail. Each must recover to a
    /// partial tree that holds every whole-tree invariant under every dialect.
    const BROKEN_SCRIPTS: &[&str] = &[
        "SELECT alpha; FROM x; SELECT beta",
        "SELECT a FROM t; ); SELECT b FROM u",
        "INSERT INTO t VALUES (1); GARBAGE HERE; UPDATE t SET a = 1",
        "SELECT 1; SELECT FROM; SELECT 3; SELECT 4 WHERE",
        ") ; SELECT ok",
        "SELECT a; FROM; SELECT b; ); SELECT c; SELECT d WHERE; SELECT e",
        "SELECT good; SELECT 'unterminated",
        "SELECT good; ) 'unterminated",
        "CREATE TABLE t (id INT; SELECT recovered",
    ];

    #[test]
    fn recovery_holds_invariants_over_broken_scripts() {
        for &src in BROKEN_SCRIPTS {
            // Reuses the shared body, which runs all four dialects through the three
            // walkers; a broken invariant panics naming the offending node.
            recover_invariants(src.as_bytes());
        }
    }

    #[test]
    fn recovery_holds_invariants_breaking_a_statement_at_each_position() {
        // Truncate a well-formed statement at every byte offset, wedge the prefix
        // between two good statements, and assert the recovered tree stays invariant.
        // This walks the broken statement through every partial-parse depth, so a
        // resync bug that only triggers when a specific clause half-parses surfaces.
        const BASES: &[&str] = &[
            "SELECT a, b FROM t WHERE a = 1 ORDER BY b",
            "INSERT INTO t (a, b) VALUES (1, 2)",
            "CREATE TABLE t (id INT PRIMARY KEY, name TEXT)",
        ];
        for &base in BASES {
            for cut in 0..=base.len() {
                if !base.is_char_boundary(cut) {
                    continue;
                }
                let script = format!("SELECT ok1; {}; SELECT ok2", &base[..cut]);
                recover_invariants(script.as_bytes());
            }
        }
    }

    #[test]
    fn recovery_corpus_is_non_vacuous_and_holds_invariants() {
        // Guard against a trivially-passing suite: recovery must actually produce a
        // partial tree (survivors on both sides of a broken middle statement, plus a
        // recorded error) AND that real tree must pass all three walkers.
        let recovered =
            parse_recovering("SELECT alpha; FROM x; SELECT beta", Ansi).expect("streaming setup");
        assert_eq!(
            recovered.statements().len(),
            2,
            "the two well-formed statements survive the broken middle one",
        );
        assert!(
            recovered.has_errors(),
            "the broken middle statement is recorded, not silently dropped",
        );

        let parsed = recovered.parsed();
        spans::assert_parsed_span_invariants(parsed);
        assert_unique_node_ids(parsed.statements());
        assert_symbols_resolve(parsed);
    }

    #[test]
    fn node_ids_stay_unique_across_many_resync_boundaries() {
        // Survivors on both sides of several broken statements share one monotonic
        // counter; a resync that reset or reused ids would collide across the gaps.
        let src = "SELECT a; )); SELECT b; FROM; SELECT c; SELECT d WHERE; SELECT e";
        let recovered = parse_recovering(src, Ansi).expect("streaming setup");
        assert!(
            recovered.statements().len() >= 4,
            "several survivors across resync boundaries: got {}",
            recovered.statements().len(),
        );
        assert_unique_node_ids(recovered.statements());
    }

    #[test]
    fn recover_invariants_replays_committed_inputs() {
        for input in RECOVER_INVARIANTS_REPLAYS {
            recover_invariants(input);
        }
    }

    #[test]
    fn bolero_recover_invariants_runs_under_cargo_test() {
        // Arbitrary bytes into the recovering path: panic-freedom plus the three
        // invariants. No oracle, so this is cheap; the nightly libFuzzer soak deepens
        // it. Mirrors `fuzz::parse_no_panic`'s raw-byte budget.
        bolero::check!()
            .with_iterations(64)
            .with_max_len(MAX_RECOVER_INPUT_BYTES)
            .for_each(|input: &[u8]| recover_invariants(input));
    }
}
