// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Deterministic adversarial / pathological SQL generators and the pure scaling
//! decision-rule, shared by the heap bench, the instruction bench, the scaling
//! gate, and the wall-clock testbed.
//!
//! The inputs here are the ones most likely to make a parser scale super-linearly
//! or overflow the stack: deeply nested parentheses / subqueries, M-way joins,
//! long `IN (...)` and `VALUES` lists, chained CTEs, and long left-associative
//! operator chains. Every generator is a pure function of an integer `depth` /
//! `width` parameter — no wall-clock, no RNG, no environment — so the SQL a given
//! parameter produces is byte-identical run-to-run and the derived measurements are
//! reproducible and git-diffable (ADR-0016/0017).
//!
//! Two axes, measured differently:
//! - **Width** families (operator chain, M-way join, `IN`, `VALUES`, CTE chain)
//!   grow a flat list iteratively, so the question is whether parse cost stays
//!   *linear* in the width. [`superlinear_steps`](crate::adversarial::superlinear_steps) is the deterministic verdict the
//!   scaling gate asserts on.
//! - **Depth** families (nested parens, nested subqueries) drive genuine recursive
//!   descent, so the question is the recursion *guard*: a clean rejection at the
//!   configured limit, never a stack overflow. The recursion gate drives these.

use std::fmt::Write as _;

// ---------------------------------------------------------------------------
// Width generators (flat lists — the linear-scaling axis)
// ---------------------------------------------------------------------------

/// `SELECT 1 + 1 + … + 1` with `width` additions (a left-associative chain of
/// `width + 1` operands). The classic Pratt stressor: a parser that recursed per
/// operator instead of looping would build a `width`-deep call stack, so this
/// flushes out accidental recursion on the expression hot path. `width = 0` is the
/// degenerate `SELECT 1`.
pub fn operator_chain(width: usize) -> String {
    let mut sql = String::with_capacity("SELECT 1".len() + width * " + 1".len());
    sql.push_str("SELECT 1");
    for _ in 0..width {
        sql.push_str(" + 1");
    }
    sql
}

/// `SELECT * FROM t0 JOIN t1 ON t0.c = t1.c JOIN t2 …` with `width` explicit joins
/// over `width + 1` tables. Exercises the `FROM`-clause join loop; like the
/// operator chain it is left-associative and must parse iteratively.
pub fn many_way_join(width: usize) -> String {
    let mut sql = String::from("SELECT * FROM t0");
    for i in 1..=width {
        let _ = write!(sql, " JOIN t{i} ON t{}.c = t{i}.c", i - 1);
    }
    sql
}

/// `SELECT * FROM t WHERE x IN (1, 2, …, width)` — a long parenthesized value list.
/// `width` is the element count; callers use `width >= 1` (an empty `IN ()` is not
/// valid SQL, and the [`WIDTH_LADDER`] starts well above 1).
pub fn in_list(width: usize) -> String {
    let mut sql = String::from("SELECT * FROM t WHERE x IN (");
    for i in 0..width {
        if i > 0 {
            sql.push_str(", ");
        }
        let _ = write!(sql, "{}", i + 1);
    }
    sql.push(')');
    sql
}

/// `VALUES (1), (2), …, (width)` — a long row list as a top-level statement.
/// `width` is the row count (`>= 1`).
pub fn values_rows(width: usize) -> String {
    let mut sql = String::from("VALUES ");
    for i in 0..width {
        if i > 0 {
            sql.push_str(", ");
        }
        let _ = write!(sql, "({})", i + 1);
    }
    sql
}

/// A `WITH` chain of `width` CTEs, each selecting from the previous one:
/// `WITH c0 AS (SELECT 1 AS x), c1 AS (SELECT x FROM c0), … SELECT x FROM c{width-1}`.
/// The CTE bodies are *siblings* in the `WITH` list (not nested), so a correct RAII
/// recursion guard restores depth between them and this stays a width — not a
/// depth — stressor. `width >= 1`.
pub fn cte_chain(width: usize) -> String {
    let mut sql = String::from("WITH c0 AS (SELECT 1 AS x)");
    for i in 1..width {
        let _ = write!(sql, ", c{i} AS (SELECT x FROM c{})", i - 1);
    }
    let _ = write!(sql, " SELECT x FROM c{}", width - 1);
    sql
}

// ---------------------------------------------------------------------------
// Depth generators (genuine recursion — the guard axis)
// ---------------------------------------------------------------------------

/// `SELECT ((( … 1 … )))` with `depth` nested parentheses — expression recursion
/// through the Pratt core, one recursion-guard entry per level. Mirrors the
/// engine's own `nested_parens` acceptance fixture so the bench and the parser test
/// drive the identical guard site.
pub fn nested_parens(depth: usize) -> String {
    let mut sql = String::with_capacity("SELECT ".len() + 1 + 2 * depth);
    sql.push_str("SELECT ");
    for _ in 0..depth {
        sql.push('(');
    }
    sql.push('1');
    for _ in 0..depth {
        sql.push(')');
    }
    sql
}

/// `((( … SELECT 1 … )))` with `depth` nested parenthesized queries — query
/// recursion through the set-operation operand path, one guard entry per level.
pub fn nested_subqueries(depth: usize) -> String {
    let mut sql = String::with_capacity("SELECT 1".len() + 2 * depth);
    for _ in 0..depth {
        sql.push('(');
    }
    sql.push_str("SELECT 1");
    for _ in 0..depth {
        sql.push(')');
    }
    sql
}

// ---------------------------------------------------------------------------
// Family registries + the width ladder
// ---------------------------------------------------------------------------

/// A named width generator, so the benches/gates can iterate every flat-list family
/// uniformly instead of repeating the list at each call site.
#[derive(Clone, Copy)]
pub struct WidthFamily {
    pub name: &'static str,
    pub generate: fn(usize) -> String,
}

/// Every width (flat-list) family, in a fixed order so derived numbers are
/// deterministic. The first four are the spike-confirmed both-accept set the
/// scaling gate requires; `cte_chain` is measured opportunistically (gated only
/// when both parsers accept the whole ladder).
pub const WIDTH_FAMILIES: &[WidthFamily] = &[
    WidthFamily {
        name: "operator_chain",
        generate: operator_chain,
    },
    WidthFamily {
        name: "many_way_join",
        generate: many_way_join,
    },
    WidthFamily {
        name: "in_list",
        generate: in_list,
    },
    WidthFamily {
        name: "values_rows",
        generate: values_rows,
    },
    WidthFamily {
        name: "cte_chain",
        generate: cte_chain,
    },
];

/// The four width families the scaling gate *requires* to remain both-accept; a
/// core family silently dropping out of either parser's surface must fail the gate
/// (anti-vanishing), so they are named here rather than inferred.
pub const CORE_WIDTH_FAMILIES: &[&str] =
    &["operator_chain", "many_way_join", "in_list", "values_rows"];

/// A named depth generator (the recursion axis).
#[derive(Clone, Copy)]
pub struct DepthFamily {
    pub name: &'static str,
    pub generate: fn(usize) -> String,
}

/// The recursion families, in fixed order. Two distinct guard sites: parenthesized
/// expressions (Pratt) and parenthesized queries (set-op operand).
pub const DEPTH_FAMILIES: &[DepthFamily] = &[
    DepthFamily {
        name: "nested_parens",
        generate: nested_parens,
    },
    DepthFamily {
        name: "nested_subqueries",
        generate: nested_subqueries,
    },
];

/// A geometric (doubling) width ladder. Doubling is load-bearing: with each step
/// doubling the width, a *linear* metric at most doubles (factor approaches 2 from
/// below as the fixed per-statement overhead amortizes), while a *quadratic* one
/// quadruples (factor approaches 4) — so the growth factor cleanly separates the
/// two without pinning any absolute, layout-dependent byte count. The top width is
/// large enough to expose super-linearity yet small enough that the whole gate
/// parses in well under a second.
pub const WIDTH_LADDER: &[usize] = &[64, 128, 256, 512, 1024, 2048];

// ---------------------------------------------------------------------------
// Pure scaling decision-rule (no dhat, no IO — unit-testable in isolation)
// ---------------------------------------------------------------------------

/// One (width, measured-metric) point on a scaling curve.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScaleSample {
    pub width: usize,
    pub metric: u64,
}

/// One adjacent step of a scaling curve: how the width grew versus how the metric
/// grew across it. The ratio of the two is what the linearity verdict reads.
#[derive(Clone, Copy, Debug)]
pub struct GrowthStep {
    pub from_width: usize,
    pub to_width: usize,
    pub from_metric: u64,
    pub to_metric: u64,
}

impl GrowthStep {
    /// How much the *width* grew across this step (2.0 on a doubling ladder).
    pub fn width_factor(self) -> f64 {
        self.to_width as f64 / self.from_width as f64
    }

    /// How much the *metric* grew across this step. A zero baseline is treated as
    /// infinite growth so a degenerate point can never silently pass.
    pub fn metric_factor(self) -> f64 {
        if self.from_metric == 0 {
            f64::INFINITY
        } else {
            self.to_metric as f64 / self.from_metric as f64
        }
    }

    /// The largest metric growth this step may show before it counts as
    /// super-linear: proportional to the width growth, plus `slack` head-room.
    /// Linear work grows at most ~proportionally to the width; the slack absorbs
    /// the modest curvature a fixed per-statement overhead adds at small widths and
    /// any std allocation-pattern drift across toolchains.
    pub fn allowed_factor(self, slack: f64) -> f64 {
        self.width_factor() * (1.0 + slack)
    }

    /// True when the metric outran the width by more than the allowed head-room —
    /// the signature of super-linear (e.g. quadratic) scaling.
    pub fn is_superlinear(self, slack: f64) -> bool {
        self.metric_factor() > self.allowed_factor(slack)
    }
}

/// Default scaling head-room: on a doubling ladder this caps the per-step metric
/// growth at `2 * (1 + 0.25) = 2.5`, comfortably above linear (~2) and well below
/// quadratic (~4).
pub const DEFAULT_SCALING_SLACK: f64 = 0.25;

/// The adjacent growth steps over an ascending-width sample series (`samples.len()
/// - 1` of them; empty for fewer than two points).
fn growth_steps(samples: &[ScaleSample]) -> Vec<GrowthStep> {
    samples
        .windows(2)
        .map(|w| GrowthStep {
            from_width: w[0].width,
            to_width: w[1].width,
            from_metric: w[0].metric,
            to_metric: w[1].metric,
        })
        .collect()
}

/// The steps that scaled super-linearly at `slack` (empty ⇒ the curve is linear).
/// This is the deterministic verdict the scaling gate asserts is empty.
pub fn superlinear_steps(samples: &[ScaleSample], slack: f64) -> Vec<GrowthStep> {
    growth_steps(samples)
        .into_iter()
        .filter(|step| step.is_superlinear(slack))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generators_are_deterministic_and_parameter_shaped() {
        // Byte-identical for the same parameter, and the shape scales with it.
        assert_eq!(operator_chain(3), "SELECT 1 + 1 + 1 + 1");
        assert_eq!(operator_chain(3), operator_chain(3));
        assert_eq!(
            many_way_join(2),
            "SELECT * FROM t0 JOIN t1 ON t0.c = t1.c JOIN t2 ON t1.c = t2.c"
        );
        assert_eq!(in_list(3), "SELECT * FROM t WHERE x IN (1, 2, 3)");
        assert_eq!(values_rows(3), "VALUES (1), (2), (3)");
        assert_eq!(
            cte_chain(2),
            "WITH c0 AS (SELECT 1 AS x), c1 AS (SELECT x FROM c0) SELECT x FROM c1"
        );
        assert_eq!(nested_parens(2), "SELECT ((1))");
        assert_eq!(nested_subqueries(2), "((SELECT 1))");
    }

    #[test]
    fn linear_curve_has_no_superlinear_steps() {
        // A linear metric with fixed overhead: metric = 10 * width + 100. On a
        // doubling ladder its per-step factor is strictly below 2, so nothing trips.
        let samples: Vec<ScaleSample> = WIDTH_LADDER
            .iter()
            .map(|&w| ScaleSample {
                width: w,
                metric: 10 * w as u64 + 100,
            })
            .collect();
        assert!(superlinear_steps(&samples, DEFAULT_SCALING_SLACK).is_empty());
    }

    #[test]
    fn quadratic_curve_is_flagged() {
        // metric = width^2 grows by ~4x per doubling — clearly super-linear.
        let samples: Vec<ScaleSample> = WIDTH_LADDER
            .iter()
            .map(|&w| ScaleSample {
                width: w,
                metric: (w as u64) * (w as u64),
            })
            .collect();
        let flagged = superlinear_steps(&samples, DEFAULT_SCALING_SLACK);
        assert_eq!(
            flagged.len(),
            WIDTH_LADDER.len() - 1,
            "every doubling step of a quadratic curve is super-linear"
        );
    }

    #[test]
    fn a_zero_baseline_never_silently_passes() {
        let samples = [
            ScaleSample {
                width: 64,
                metric: 0,
            },
            ScaleSample {
                width: 128,
                metric: 1,
            },
        ];
        assert_eq!(superlinear_steps(&samples, DEFAULT_SCALING_SLACK).len(), 1);
    }
}
