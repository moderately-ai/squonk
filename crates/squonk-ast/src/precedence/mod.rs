// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! One binding-power table for parsing and rendering SQL expressions and set
//! operations.

use crate::ast::{
    BinaryOperator, EqualsSpelling, IsDistinctFromSpelling, IsNotDistinctFromSpelling, SetOperator,
    UnaryOperator,
};

/// Operator associativity used by parser validation and renderer grouping.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Assoc {
    /// Left-associative (`a - b - c` groups as `(a - b) - c`).
    Left,
    /// Right-associative (`a = b = c` groups as `a = (b = c)`).
    Right,
    /// Non-associative — chaining the operator is a parse error.
    NonAssoc,
}

/// Pratt and render-time binding powers for an infix operator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BindingPower {
    /// Left-hand operand.
    pub left: u8,
    /// Right-hand operand.
    pub right: u8,
    /// The operator's associativity; see [`Assoc`].
    pub assoc: Assoc,
}

/// Dialect-owned Pratt and render-time binding powers.
///
/// The table is field-based rather than array-indexed so adding a
/// [`BinaryOperator`] or [`UnaryOperator`] forces this module to name its binding
/// power. Dialects can replace the whole table on [`FeatureSet`] or build a
/// small delta with [`with_binary`](Self::with_binary).
///
/// [`FeatureSet`]: crate::dialect::FeatureSet
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BindingPowerTable {
    /// Binding power of logical `OR`.
    pub or: BindingPower,
    /// MySQL `XOR` logical exclusive-or: binds tighter than `OR` and looser than
    /// `AND` (MySQL's `OR < XOR < AND` precedence). No standard operator occupies
    /// this rank, so it gets its own field rather than sharing `or`/`and`.
    pub xor: BindingPower,
    /// Binding power of logical `AND`.
    pub and: BindingPower,
    /// Binding power of the comparison operators (`=`/`<`/`>`/…).
    pub comparison: BindingPower,
    /// The range/pattern/membership predicate tier — `[NOT] BETWEEN`, `[NOT] IN`,
    /// `[NOT] LIKE`/`ILIKE`/`SIMILAR TO`. `None` tracks [`comparison`](Self::comparison)
    /// (the historical placement every dialect parsed these at, so a dialect that
    /// re-associates its comparisons carries these with it); `Some` re-ranks them to a
    /// dedicated tier, read through [`range_predicate`](Self::range_predicate).
    ///
    /// PostgreSQL/DuckDB place this family one tier ABOVE the comparison operators
    /// (`<`/`>`/`=`) and below the `%left Op` "any other operator" rank — gram.y's
    /// `%nonassoc BETWEEN IN_P LIKE ILIKE SIMILAR` row — so `a = b BETWEEN c AND d` groups
    /// `a = (b BETWEEN c AND d)` (engine-measured on pg_query 6.1 and DuckDB
    /// `json_serialize_sql`); the PostgreSQL/Lenient presets set
    /// `Some(`[`RANGE_PREDICATE_ABOVE_COMPARISON`]`)`. MySQL/SQLite rank BETWEEN at or
    /// below comparison (their manuals), so they leave this `None` and track `comparison`
    /// automatically, staying byte-identical. The `IS [NOT] NULL`/`IS DISTINCT`/truth-value
    /// predicates are a SEPARATE (in PostgreSQL/DuckDB, looser) tier — see
    /// [`is_predicate_override`](Self::is_predicate_override).
    pub range_predicate_override: Option<BindingPower>,
    /// The `IS`-family predicate tier — the postfix `IS [NOT] NULL` / `ISNULL` / `NOTNULL` /
    /// `NOT NULL` null tests, the `IS [NOT] {TRUE|FALSE|UNKNOWN}` truth-value tests, `IS [NOT]
    /// NORMALIZED`, and the infix `IS [NOT] DISTINCT FROM` (keyword form). `None` tracks
    /// [`comparison`](Self::comparison) (MySQL/SQLite rank `IS` at the comparison/equality tier,
    /// so they stay byte-identical); `Some` re-ranks the whole family to a dedicated tier read
    /// through [`predicate`](Self::predicate).
    ///
    /// PostgreSQL and DuckDB place this family one tier BELOW the comparison operators
    /// (`<`/`>`/`=`) and above `NOT` — PostgreSQL gram.y's `%nonassoc IS ISNULL NOTNULL` row,
    /// which sits under `%nonassoc '<' '>' '='` — so `a <> b IS NULL` groups `(a <> b) IS NULL`
    /// and `a IS DISTINCT FROM b = c` groups `a IS DISTINCT FROM (b = c)` (engine-measured on
    /// PostgreSQL 16 `pg_get_viewdef` and DuckDB 1.5.4 `json_serialize_sql`); the
    /// PostgreSQL/DuckDB/Lenient presets set `Some(`[`IS_PREDICATE_BELOW_COMPARISON`]`)`.
    /// Non-associative, matching both engines (`a IS DISTINCT FROM b IS DISTINCT FROM c` is a
    /// parse error). SQLite's bare general `IS`/`IS NOT` and MySQL's `<=>` are comparison-tier
    /// null-safe (in)equality, distinguished by spelling, and are unaffected by this override.
    pub is_predicate_override: Option<BindingPower>,
    /// The `==` spelling of equality ([`BinaryOperator::Eq`] with
    /// [`EqualsSpelling::Double`]). Carries the
    /// [`comparison`](Self::comparison) value in every dialect but DuckDB, where `==`
    /// is not the `%nonassoc '='` comparison but a generic `%left Op` operator: it
    /// binds *tighter than* the comparisons and looser than additive, left-associative
    /// (`1 == 2 == 3` is `((1 = 2) = 3)`, `1 < 2 == 3` is `(1 < (2 = 3))`, `1 + 1 == 2`
    /// is `((1 + 1) = 2)` — measured on 1.5.4). A distinct field so this DuckDB
    /// re-ranking never disturbs the `=`/`<`/`>` comparisons it shares
    /// [`BinaryOperator::Eq`] with; [`with_binary`](Self::with_binary) keeps the two in
    /// sync for the comparison-family callers (SQLite/MySQL move both together) and lets
    /// DuckDB move `==` alone.
    pub double_equals: BindingPower,
    /// Binding power of the additive operators (`+`/`-`).
    pub additive: BindingPower,
    /// Binding power of the multiplicative operators (`*`/`/`/`%`).
    pub multiplicative: BindingPower,
    /// PostgreSQL exponentiation (`^`, [`BinaryOperator::Exponent`]). Its OWN precedence
    /// row in gram.y (`%left '^'`): tighter than [`multiplicative`](Self::multiplicative)
    /// `* / %` and looser than the unary sign, left-associative (`2 ^ 3 ^ 2` is
    /// `(2 ^ 3) ^ 2`, `2 ^ 3 * 2` is `(2 ^ 3) * 2` — engine-measured on pg_query). Distinct
    /// from [`bitwise_xor`](Self::bitwise_xor): MySQL's `^` (bitwise XOR) binds tighter than
    /// `*`, PostgreSQL's `^` (power) binds tighter than `*` but is a different operator at a
    /// different (higher) rank, so the two never share a field. Only reachable under
    /// [`CaretOperator::Exponent`](crate::dialect::CaretOperator).
    pub exponent: BindingPower,
    /// Binding power of the `||` string-concatenation operator.
    pub string_concat: BindingPower,
    /// PostgreSQL's "any other operator" rank — the precedence gram.y gives every
    /// native and user-defined symbolic operator outside the arithmetic/comparison
    /// core (`%left Op OPERATOR`): looser than additive `+`/`-`, tighter than the
    /// comparison/`BETWEEN` family. The `@>`/`<@` containment and `->>` JSON
    /// operators bind here, left-associative. It carries the same value as
    /// [`string_concat`](Self::string_concat) (`||` is itself an "any other operator"
    /// in PostgreSQL) but is a distinct field so moving one does not move the other.
    pub any_operator: BindingPower,
    /// The `->` token's rank ([`BinaryOperator::JsonGet`], which is also the DuckDB
    /// lambda arrow — one token, one rank). In PostgreSQL/SQLite it carries the
    /// [`any_operator`](Self::any_operator) value (`->` is an ordinary `Op` there),
    /// but DuckDB lexes `->` as its own `LAMBDA_ARROW` grammar token ranked *below
    /// every* expression operator — `x -> x % 2 = 0` and even `x -> x OR y` put the
    /// whole right side in the lambda body, and `NOT x -> y` takes `NOT x` as the
    /// left operand (measured on 1.5.4 via `json_serialize_sql`) — while its `->>`
    /// stays at the `Op` rank. A distinct field so a dialect can move `->` without
    /// moving `->>`/`@>`/`<@`, the same split rationale as
    /// [`string_concat`](Self::string_concat) vs `any_operator`.
    pub json_get: BindingPower,
    /// Bitwise OR (`|`). In PostgreSQL/SQLite/DuckDB the four binary bitwise operators
    /// share one rank between additive and comparison (engine-measured: `1 | 2 & 2` is
    /// `(1 | 2) & 2`), so the fields below carry the same standard value but stay distinct
    /// — MySQL ranks `|` < `&` < `<<`/`>>` at three separate levels (its documented
    /// grammar), so a dialect moves each independently, exactly as
    /// [`string_concat`](Self::string_concat) and [`any_operator`](Self::any_operator)
    /// split.
    pub bitwise_or: BindingPower,
    /// Bitwise AND (`&`). Standard-equal to [`bitwise_or`](Self::bitwise_or); tighter than
    /// it in MySQL.
    pub bitwise_and: BindingPower,
    /// Bitwise shift (`<<` / `>>`, one shared rank — the two shifts never diverge in any
    /// dialect). Looser than additive everywhere; tighter than `&` and looser than
    /// additive in MySQL.
    pub bitwise_shift: BindingPower,
    /// Bitwise exclusive-or (PostgreSQL `#`, MySQL `^`). In the standard table it carries
    /// PostgreSQL's "any other operator" rank (looser than additive, like `#`); MySQL
    /// re-ranks it *tighter than* multiplicative (`^` binds above `*`), so it is its own
    /// field.
    pub bitwise_xor: BindingPower,
    /// Binding power of the prefix `NOT` operator.
    pub prefix_not: u8,
    /// Binding power of the prefix `+`/`-` sign.
    pub prefix_sign: u8,
    /// Prefix bitwise complement (`~`). Binds like the unary sign in SQLite/MySQL, but in
    /// PostgreSQL/DuckDB it sits between the arithmetic operators and the binary bitwise
    /// family: one above [`bitwise_or`](Self::bitwise_or)'s left rank, so the bitwise
    /// binaries do not fold into its operand (`~ 1 & 3` is `(~ 1) & 3`) while the tighter
    /// arithmetic does (`~ 1 + 1` is `~ (1 + 1)`) — both engine-measured — and the render
    /// still parenthesizes `~ (a & b)` (the strict-inequality break the equal-rank case
    /// would lose).
    pub prefix_bitwise_not: u8,
    /// `expr AT TIME ZONE zone` (PostgreSQL): binds tighter than the arithmetic
    /// operators and looser than `COLLATE` and the unary sign.
    pub at_time_zone: BindingPower,
    /// `expr COLLATE collation` (PostgreSQL): just tighter than `AT TIME ZONE`,
    /// just looser than the unary sign.
    pub collate: BindingPower,
    /// `base[index]` / `base[lo:hi]` array subscript (PostgreSQL): binds tighter
    /// than the unary sign and looser than the `::` typecast.
    pub subscript: BindingPower,
    /// `expr::type` typecast (PostgreSQL): one of the tightest-binding operators,
    /// just looser than composite field selection.
    pub typecast: BindingPower,
    /// `(expr).field` composite field selection (PostgreSQL): the tightest of the
    /// postfix operators.
    pub field_selection: BindingPower,
}

/// Dialect-owned set-operation binding powers.
///
/// Set operations combine [`SetExpr`](crate::ast::SetExpr) query bodies rather
/// than [`Expr`](crate::ast::Expr) nodes, so they use a small parallel table
/// keyed by [`SetOperator`]. The same binding-power discipline still applies:
/// parser precedence climbing and render-time parenthesization both read this
/// table, so `INTERSECT` cannot drift from `UNION`/`EXCEPT` in one direction only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SetOperationBindingPowerTable {
    /// Binding power of `UNION`/`EXCEPT`.
    pub union_except: BindingPower,
    /// Binding power of `INTERSECT` (tighter than `UNION`/`EXCEPT`).
    pub intersect: BindingPower,
}

impl SetOperationBindingPowerTable {
    /// Standard SQL/PostgreSQL set-operation binding powers.
    pub const STANDARD: Self = Self {
        union_except: BindingPower {
            left: 10,
            right: 11,
            assoc: Assoc::Left,
        },
        intersect: BindingPower {
            left: 20,
            right: 21,
            assoc: Assoc::Left,
        },
    };

    /// Return the binding power for `op`.
    pub const fn set_operation(&self, op: &SetOperator) -> BindingPower {
        match op {
            SetOperator::Union | SetOperator::Except => self.union_except,
            SetOperator::Intersect => self.intersect,
        }
    }

    /// Return a copy of this table with one set-operator class replaced.
    pub const fn with_set_operator(mut self, op: &SetOperator, bp: BindingPower) -> Self {
        match op {
            SetOperator::Union | SetOperator::Except => self.union_except = bp,
            SetOperator::Intersect => self.intersect = bp,
        }
        self
    }

    /// Return whether a child set operation needs parentheses under `parent`.
    pub const fn needs_parens(
        &self,
        parent: &SetOperator,
        child: &SetOperator,
        side: Side,
    ) -> bool {
        needs_parens_between(self.set_operation(parent), self.set_operation(child), side)
    }
}

impl BindingPowerTable {
    /// Standard M1 binding powers shared by the ANSI and PostgreSQL presets.
    pub const STANDARD: Self = Self {
        or: BindingPower {
            left: 10,
            right: 11,
            assoc: Assoc::Left,
        },
        // Between `OR` (10) and `AND` (20): MySQL ranks `XOR` strictly between them.
        // Left-associative, like the other boolean operators (`a XOR b XOR c` groups
        // left).
        xor: BindingPower {
            left: 15,
            right: 16,
            assoc: Assoc::Left,
        },
        and: BindingPower {
            left: 20,
            right: 21,
            assoc: Assoc::Left,
        },
        comparison: BindingPower {
            left: 40,
            right: 41,
            assoc: Assoc::NonAssoc,
        },
        // Range/pattern/membership predicates track comparison by default; the
        // PostgreSQL/Lenient presets re-rank them one tier above it (see the field docs).
        range_predicate_override: None,
        // The `IS`-family predicates track comparison by default (MySQL/SQLite rank them at
        // the comparison/equality tier); the PostgreSQL/DuckDB/Lenient presets re-rank them
        // one tier below comparison (see the field docs).
        is_predicate_override: None,
        // `==` tracks `=` by default (same rank, same non-associativity); DuckDB's
        // preset re-ranks it alone to the generic `%left Op` level (see the field docs).
        double_equals: BindingPower {
            left: 40,
            right: 41,
            assoc: Assoc::NonAssoc,
        },
        additive: BindingPower {
            left: 50,
            right: 51,
            assoc: Assoc::Left,
        },
        multiplicative: BindingPower {
            left: 60,
            right: 61,
            assoc: Assoc::Left,
        },
        // PostgreSQL `^` exponentiation sits above multiplicative (`60`) and below the
        // unary sign (`80`), left-associative — its own gram.y precedence row. Only the
        // presets whose `caret_operator` is `Exponent` (PostgreSQL/DuckDB) reach it;
        // elsewhere `^` is bitwise XOR (MySQL) or nothing, at a different rank.
        exponent: BindingPower {
            left: 65,
            right: 66,
            assoc: Assoc::Left,
        },
        // `||` binds looser than additive and tighter than comparison, matching
        // PostgreSQL's "any other operator" rank: `a || b + c` is `a || (b + c)`.
        string_concat: BindingPower {
            left: 45,
            right: 46,
            assoc: Assoc::Left,
        },
        // PostgreSQL's "any other operator" rank sits at the same level as `||`
        // (both are `%left Op OPERATOR` in gram.y): looser than additive (50),
        // tighter than comparison (40), left-associative.
        any_operator: BindingPower {
            left: 45,
            right: 46,
            assoc: Assoc::Left,
        },
        // `->` is an ordinary "any other operator" in the standard table; DuckDB's
        // preset moves this field below `or` (its lambda-arrow rank).
        json_get: BindingPower {
            left: 45,
            right: 46,
            assoc: Assoc::Left,
        },
        // The binary bitwise operators sit at PostgreSQL's "any other operator" rank in
        // the standard table (looser than additive `50`, tighter than comparison `40`),
        // engine-measured for PostgreSQL/SQLite/DuckDB. MySQL's preset re-ranks them at
        // three distinct levels. All left-associative.
        bitwise_or: BindingPower {
            left: 45,
            right: 46,
            assoc: Assoc::Left,
        },
        bitwise_and: BindingPower {
            left: 45,
            right: 46,
            assoc: Assoc::Left,
        },
        bitwise_shift: BindingPower {
            left: 45,
            right: 46,
            assoc: Assoc::Left,
        },
        bitwise_xor: BindingPower {
            left: 45,
            right: 46,
            assoc: Assoc::Left,
        },
        prefix_not: 30,
        prefix_sign: 80,
        // PostgreSQL/DuckDB rank prefix `~` one above the bitwise binaries' left rank
        // (`45`): the bitwise binaries do not capture into its operand (`~ 1 & 3` groups
        // `(~ 1) & 3`) but additive `50` does (`~ 1 + 1` groups `~ (1 + 1)`). SQLite/MySQL
        // override this to the tight `prefix_sign` rank. The value differs from `bitwise_*`
        // deliberately so the renderer's strict-inequality parenthesization still wraps
        // `~ (a & b)`.
        prefix_bitwise_not: 46,
        // The PostgreSQL postfix operators all bind tighter than the binary
        // arithmetic operators; their relative order follows gram.y (lowest to
        // highest): `AT TIME ZONE` < `COLLATE` < unary sign < `[]` < `::` < `.`.
        at_time_zone: BindingPower {
            left: 70,
            right: 71,
            assoc: Assoc::Left,
        },
        collate: BindingPower {
            left: 74,
            right: 75,
            assoc: Assoc::Left,
        },
        subscript: BindingPower {
            left: 84,
            right: 85,
            assoc: Assoc::Left,
        },
        typecast: BindingPower {
            left: 88,
            right: 89,
            assoc: Assoc::Left,
        },
        field_selection: BindingPower {
            left: 92,
            right: 93,
            assoc: Assoc::Left,
        },
    };

    /// Return the binary binding power for `op`.
    pub const fn binary(&self, op: &BinaryOperator) -> BindingPower {
        match op {
            BinaryOperator::Or => self.or,
            BinaryOperator::Xor => self.xor,
            BinaryOperator::And => self.and,
            // `RLIKE`/`REGEXP` match at comparison precedence (like `LIKE`); folding
            // onto `self.comparison` keeps them a single source of truth with the
            // comparisons and the predicates, so a dialect that moves comparison
            // precedence moves regex match with it.
            // The keyword `IS [NOT] DISTINCT FROM` is the infix member of the `IS`-family
            // predicate tier (PostgreSQL `%prec IS`): it reads [`predicate`](Self::predicate),
            // so a preset that re-ranks the family below comparison carries the distinct test
            // with the postfix `IS NULL`/truth-value predicates as one source of truth (and
            // shares their non-associative chain rejection and render parenthesization).
            BinaryOperator::IsDistinctFrom(IsDistinctFromSpelling::Keyword)
            | BinaryOperator::IsNotDistinctFrom(IsNotDistinctFromSpelling::Keyword) => {
                self.predicate()
            }
            // SQLite's bare general `IS`/`IS NOT` and MySQL's `<=>` are comparison-tier
            // null-safe (in)equality (SQLite groups `IS` with `=`; MySQL's `<=>` is a
            // comparison operator), so they stay on `comparison` even when a dialect moves the
            // keyword `IS DISTINCT FROM` family below it.
            BinaryOperator::IsDistinctFrom(IsDistinctFromSpelling::Is)
            | BinaryOperator::IsNotDistinctFrom(
                IsNotDistinctFromSpelling::Is | IsNotDistinctFromSpelling::NullSafeEq,
            ) => self.comparison,
            // `==` reads its own field (DuckDB re-ranks it to the generic `%left Op`
            // level); every other equality/comparison spelling stays on `comparison`.
            BinaryOperator::Eq(EqualsSpelling::Double) => self.double_equals,
            BinaryOperator::Eq(_)
            | BinaryOperator::NotEq(_)
            | BinaryOperator::Lt
            | BinaryOperator::LtEq
            | BinaryOperator::Gt
            | BinaryOperator::GtEq
            | BinaryOperator::Regexp(_)
            | BinaryOperator::Glob
            | BinaryOperator::Match => self.comparison,
            BinaryOperator::Plus | BinaryOperator::Minus => self.additive,
            // `DIV`/`//` (integer division) and `MOD` join `*`/`/`/`%` at multiplicative.
            BinaryOperator::Multiply
            | BinaryOperator::Divide
            | BinaryOperator::Modulo(_)
            | BinaryOperator::IntegerDivide(_) => self.multiplicative,
            BinaryOperator::Exponent => self.exponent,
            BinaryOperator::StringConcat => self.string_concat,
            // The PostgreSQL `@>`/`<@` containment and `->>` JSON operators bind at
            // the "any other operator" rank (ADR-0008), folded onto one field — a
            // dialect moving that rank moves them together. `->` reads its own field
            // because DuckDB re-ranks it alone (see `json_get`).
            BinaryOperator::Contains
            | BinaryOperator::ContainedBy
            | BinaryOperator::StartsWith
            | BinaryOperator::JsonGetText
            // The PostgreSQL `jsonb` existence/path/search operators (`?`/`?|`/`?&`/`@?`/
            // `@@`/`#>`/`#>>`/`#-`) are all ordinary `%left Op` operators (engine-measured:
            // tighter than comparison, looser than additive, left-associative), so they ride
            // the same "any other operator" rank as `@>`/`<@`/`->>`.
            | BinaryOperator::JsonExists
            | BinaryOperator::JsonExistsAny
            | BinaryOperator::JsonExistsAll
            | BinaryOperator::JsonPathExists
            | BinaryOperator::JsonPathMatch
            | BinaryOperator::JsonExtractPath
            | BinaryOperator::JsonExtractPathText
            | BinaryOperator::JsonDeletePath
            // The `&&` overlap operator is an ordinary `%left Op` operator in
            // PostgreSQL/DuckDB (engine-measured on DuckDB 1.5.4: tighter than comparison
            // and `AND`, left-associative), so it rides the same "any other operator" rank.
            | BinaryOperator::Overlap => self.any_operator,
            BinaryOperator::JsonGet => self.json_get,
            // The binary bitwise operators each read their own field: coincident in the
            // standard table, but MySQL ranks `|` < `&` < `<<`/`>>` at distinct levels.
            BinaryOperator::BitwiseOr => self.bitwise_or,
            BinaryOperator::BitwiseAnd => self.bitwise_and,
            BinaryOperator::BitwiseShiftLeft | BinaryOperator::BitwiseShiftRight => {
                self.bitwise_shift
            }
            // Both XOR spellings (`#`/`^`) fold onto one rank; the spelling tag never
            // affects precedence (it is which dialect, and each has one XOR rank).
            BinaryOperator::BitwiseXor(_) => self.bitwise_xor,
            // `OVERLAPS` carries a fixed cross-dialect rank (its PostgreSQL `%nonassoc
            // OVERLAPS` row, just above comparison and below `Op`), not a table field —
            // see `OVERLAPS_PREDICATE`.
            BinaryOperator::Overlaps => OVERLAPS_PREDICATE,
        }
    }

    /// Return the prefix binding power for `op`.
    pub const fn prefix(&self, op: &UnaryOperator) -> u8 {
        match op {
            UnaryOperator::Not => self.prefix_not,
            // `PRIOR` (Oracle/Snowflake `CONNECT BY`) binds like the unary sign in every
            // dialect that has it, so it shares `prefix_sign` (tighter than comparison):
            // `PRIOR a = b` groups as `(PRIOR a) = b`. No separate table field — the rank
            // never diverges from the sign operators, so widening the per-preset table
            // would add an axis no measured boundary moves.
            UnaryOperator::Minus | UnaryOperator::Plus | UnaryOperator::Prior => self.prefix_sign,
            UnaryOperator::BitwiseNot => self.prefix_bitwise_not,
        }
    }

    /// Return the binding power of the `IS`-family predicates (`IS [NOT] NULL`, `ISNULL`,
    /// `NOTNULL`, `NOT NULL`, `IS [NOT] {TRUE|FALSE|UNKNOWN}`, `IS [NOT] NORMALIZED`, and the
    /// keyword `IS [NOT] DISTINCT FROM`).
    ///
    /// Defaults to [`comparison`](Self::comparison) — returned when
    /// [`is_predicate_override`](Self::is_predicate_override) is `None`, so MySQL/SQLite (which
    /// rank `IS` at the comparison/equality tier) carry these predicates with their comparisons
    /// — while the PostgreSQL/DuckDB/Lenient presets override it to their dedicated tier one
    /// rank below comparison. The parser climbs the family at this rank and forbids chaining
    /// them with comparisons, and render-time parenthesization reads the *same* level, so the
    /// two can never drift (ADR-0008).
    pub const fn predicate(&self) -> BindingPower {
        match self.is_predicate_override {
            Some(bp) => bp,
            None => self.comparison,
        }
    }

    /// Return the binding power of the range/pattern/membership predicates (`[NOT]
    /// BETWEEN`, `[NOT] IN`, `[NOT] LIKE`/`ILIKE`/`SIMILAR TO`).
    ///
    /// Defaults to [`comparison`](Self::comparison) — returned when
    /// [`range_predicate_override`](Self::range_predicate_override) is `None`, so a dialect
    /// that re-associates its comparisons carries these predicates with it — while the
    /// PostgreSQL/Lenient presets override it to their dedicated tighter tier. The parser
    /// climbs these predicates at this rank and the renderer parenthesizes them by it, one
    /// source of truth (ADR-0008).
    pub const fn range_predicate(&self) -> BindingPower {
        match self.range_predicate_override {
            Some(bp) => bp,
            None => self.comparison,
        }
    }

    /// Return a copy of this table with one binary operator class replaced.
    pub const fn with_binary(mut self, op: &BinaryOperator, bp: BindingPower) -> Self {
        match op {
            BinaryOperator::Or => self.or = bp,
            BinaryOperator::Xor => self.xor = bp,
            BinaryOperator::And => self.and = bp,
            // `==` alone moves only its own field (DuckDB re-ranks it apart from the
            // comparisons); the rest of the comparison family moves `comparison` and
            // carries `==` with it, so a dialect that re-associates its comparisons
            // (SQLite/MySQL set them `Left` via `with_binary(&Eq(Single), …)`) keeps `==`
            // tracking `=` without a second call.
            BinaryOperator::Eq(EqualsSpelling::Double) => self.double_equals = bp,
            BinaryOperator::Eq(_)
            | BinaryOperator::NotEq(_)
            | BinaryOperator::Lt
            | BinaryOperator::LtEq
            | BinaryOperator::Gt
            | BinaryOperator::GtEq
            | BinaryOperator::IsDistinctFrom(_)
            | BinaryOperator::IsNotDistinctFrom(_)
            | BinaryOperator::Regexp(_)
            | BinaryOperator::Glob
            | BinaryOperator::Match => {
                self.comparison = bp;
                self.double_equals = bp;
            }
            BinaryOperator::Plus | BinaryOperator::Minus => self.additive = bp,
            BinaryOperator::Multiply
            | BinaryOperator::Divide
            | BinaryOperator::Modulo(_)
            | BinaryOperator::IntegerDivide(_) => {
                self.multiplicative = bp;
            }
            BinaryOperator::Exponent => self.exponent = bp,
            BinaryOperator::StringConcat => self.string_concat = bp,
            BinaryOperator::Contains
            | BinaryOperator::ContainedBy
            | BinaryOperator::StartsWith
            | BinaryOperator::JsonGetText
            | BinaryOperator::JsonExists
            | BinaryOperator::JsonExistsAny
            | BinaryOperator::JsonExistsAll
            | BinaryOperator::JsonPathExists
            | BinaryOperator::JsonPathMatch
            | BinaryOperator::JsonExtractPath
            | BinaryOperator::JsonExtractPathText
            | BinaryOperator::JsonDeletePath
            | BinaryOperator::Overlap => self.any_operator = bp,
            BinaryOperator::JsonGet => self.json_get = bp,
            BinaryOperator::BitwiseOr => self.bitwise_or = bp,
            BinaryOperator::BitwiseAnd => self.bitwise_and = bp,
            BinaryOperator::BitwiseShiftLeft | BinaryOperator::BitwiseShiftRight => {
                self.bitwise_shift = bp;
            }
            BinaryOperator::BitwiseXor(_) => self.bitwise_xor = bp,
            // `OVERLAPS` has a fixed cross-dialect rank (`OVERLAPS_PREDICATE`), so there is
            // no table field to re-rank; a `with_binary` call for it is a no-op.
            BinaryOperator::Overlaps => {}
        }
        self
    }

    /// Return whether a child binary expression needs parentheses under `parent`.
    pub const fn needs_parens(
        &self,
        parent: &BinaryOperator,
        child: &BinaryOperator,
        side: Side,
    ) -> bool {
        needs_parens_between(self.binary(parent), self.binary(child), side)
    }
}

/// Standard binding-power table used by the builtin dialect presets.
pub const STANDARD_BINDING_POWERS: BindingPowerTable = BindingPowerTable::STANDARD;

/// The binding power of DuckDB's unparenthesized `<expr> IN <c_expr>` list-membership
/// operator ([`Expr::InExpr`](crate::ast::Expr::InExpr)).
///
/// A fixed rank — not a [`BindingPowerTable`] field — because the operator exists only
/// under DuckDB (and the permissive Lenient union), where the comparison / string-concat
/// ranks it sits between are the standard `40` / `45`. DuckDB's own `IN_P` precedence is
/// just above the comparison operators and just below `Op`/`||`: `z = w IN y` groups
/// `z = (w IN y)` (tighter than `=`), while `a * b IN y` groups `(a * b) IN y` and
/// `a || b IN y` groups `(a || b) IN y` (looser than `*` and `||` on the left operand;
/// all measured on 1.5.4 via `json_serialize_sql`). Left-associative — `z IN y IN w` is
/// `(z IN y) IN w`. The `c_expr` right operand (subscript in, `::`/`COLLATE` out) is a
/// grammatical restriction enforced by the parser, independent of this rank.
pub const UNPARENTHESIZED_IN_LIST: BindingPower = BindingPower {
    left: 42,
    right: 43,
    assoc: Assoc::Left,
};

/// The binding power of the SQL-standard `OVERLAPS` period predicate
/// ([`BinaryOperator::Overlaps`]).
///
/// A fixed rank — not a [`BindingPowerTable`] field — because `OVERLAPS` exists only under
/// the PostgreSQL/Lenient presets and its precedence never varies by dialect: PostgreSQL's
/// `%nonassoc OVERLAPS` gram.y row sits just above the comparison/`BETWEEN` family (`40`)
/// and just below the `%left Op` "any other operator" rank (`45`), so `x OVERLAPS y = TRUE`
/// groups `(x OVERLAPS y) = TRUE`. Non-associative: the boolean result is not a `row`, so a
/// second `OVERLAPS` has no valid row operand and the parser rejects the chain by operand
/// shape rather than this flag — the [`Assoc::NonAssoc`] tag drives the renderer's
/// parenthesization, matching the grammar's non-chaining.
pub const OVERLAPS_PREDICATE: BindingPower = BindingPower {
    left: 42,
    right: 43,
    assoc: Assoc::NonAssoc,
};

/// The binding power of the range/pattern/membership predicate tier under the
/// PostgreSQL/Lenient presets (`[NOT] BETWEEN`, `[NOT] IN`, `[NOT] LIKE`/`ILIKE`/`SIMILAR
/// TO`; see [`BindingPowerTable::range_predicate_override`]).
///
/// PostgreSQL's `%nonassoc BETWEEN IN_P LIKE ILIKE SIMILAR` tier: one rank above the
/// comparison operators (`40`) and below the `%left Op` "any other operator" rank (`45`),
/// so `a = b BETWEEN c AND d` groups `a = (b BETWEEN c AND d)`. Shares
/// [`OVERLAPS_PREDICATE`]'s numeric rank (gram.y places `OVERLAPS` one line tighter, but
/// the two never meet as adjacent operands — `OVERLAPS` requires two-element rows — so the
/// distinction is unobservable). Non-associative: `a BETWEEN b AND c BETWEEN d AND e` is a
/// parse error, matching PostgreSQL and DuckDB.
pub const RANGE_PREDICATE_ABOVE_COMPARISON: BindingPower = BindingPower {
    left: 42,
    right: 43,
    assoc: Assoc::NonAssoc,
};

/// The binding power of the `IS`-family predicate tier under the PostgreSQL/DuckDB/Lenient
/// presets (the postfix `IS [NOT] NULL`/`ISNULL`/`NOTNULL`/`NOT NULL` null tests, the `IS [NOT]
/// {TRUE|FALSE|UNKNOWN}` truth-value tests, `IS [NOT] NORMALIZED`, and the keyword `IS [NOT]
/// DISTINCT FROM`; see [`BindingPowerTable::is_predicate_override`]).
///
/// PostgreSQL's `%nonassoc IS ISNULL NOTNULL` tier: one rank BELOW the comparison operators
/// (`40`) and above `%right NOT` (`30`), so `a <> b IS NULL` groups `(a <> b) IS NULL` and
/// `a IS DISTINCT FROM b = c` groups `a IS DISTINCT FROM (b = c)` (engine-measured on PostgreSQL
/// 16 and DuckDB 1.5.4). Non-associative: `a IS DISTINCT FROM b IS DISTINCT FROM c` is a parse
/// error, matching both engines; the postfix `a <> b IS NULL` still parses because its left
/// operand is a comparison (rank `40`), not another `IS`-family predicate.
pub const IS_PREDICATE_BELOW_COMPARISON: BindingPower = BindingPower {
    left: 35,
    right: 36,
    assoc: Assoc::NonAssoc,
};

/// Standard set-operation binding-power table used by the builtin dialect presets.
pub const STANDARD_SET_OPERATION_BINDING_POWERS: SetOperationBindingPowerTable =
    SetOperationBindingPowerTable::STANDARD;

/// Child side within a binary parent expression.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// The left child of a binary parent expression.
    Left,
    /// The right child of a binary parent expression.
    Right,
}

/// Whether a `child` node needs parentheses under a `parent` node on the given
/// side, given their binding powers.
///
/// This is the render-time left/right binding-power comparison (Calcite
/// `leftPrec`/`rightPrec`), factored out so binary-operator,
/// comparison-predicate, and set-operation parenthesization all derive parens from
/// one rule and cannot diverge. A child binding looser than the side the parent
/// reaches across needs parens; an equal-precedence child needs them on the side
/// the parent's associativity does not already imply (always, when non-associative).
pub const fn needs_parens_between(parent: BindingPower, child: BindingPower, side: Side) -> bool {
    let same_precedence = parent.left == child.left && parent.right == child.right;

    match side {
        Side::Left => {
            child.right < parent.left
                || (same_precedence
                    && match parent.assoc {
                        Assoc::Left => false,
                        Assoc::Right | Assoc::NonAssoc => true,
                    })
        }
        Side::Right => {
            child.left < parent.right
                || (same_precedence
                    && match parent.assoc {
                        Assoc::Right => false,
                        Assoc::Left | Assoc::NonAssoc => true,
                    })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{EqualsSpelling, NotEqSpelling};

    #[test]
    fn pratt_binding_powers_group_standard_sql_operators() {
        let plus = STANDARD_BINDING_POWERS.binary(&BinaryOperator::Plus);
        let multiply = STANDARD_BINDING_POWERS.binary(&BinaryOperator::Multiply);
        let equals = STANDARD_BINDING_POWERS.binary(&BinaryOperator::Eq(EqualsSpelling::Single));
        let and = STANDARD_BINDING_POWERS.binary(&BinaryOperator::And);
        let concat = STANDARD_BINDING_POWERS.binary(&BinaryOperator::StringConcat);

        assert!(plus.right < multiply.left);
        assert!(plus.left < multiply.right);
        assert!(and.right < equals.left);
        // PostgreSQL: `||` is looser than additive (`a || b + c` == `a || (b + c)`)
        // and tighter than comparison (`a = b || c` == `a = (b || c)`).
        assert!(concat.right < plus.left);
        assert!(equals.right < concat.left);
    }

    #[test]
    fn render_parentheses_are_derived_from_binding_powers() {
        assert!(STANDARD_BINDING_POWERS.needs_parens(
            &BinaryOperator::Multiply,
            &BinaryOperator::Plus,
            Side::Left
        ));
        assert!(!STANDARD_BINDING_POWERS.needs_parens(
            &BinaryOperator::Plus,
            &BinaryOperator::Multiply,
            Side::Right
        ));
    }

    #[test]
    fn predicate_level_tracks_comparison_precedence() {
        // Predicates (`IS NULL` / `BETWEEN` / `IN`) parse at comparison precedence
        // (`binding_power(Eq)`), so the render-time predicate level is a derived
        // accessor onto it, not a second field — they can never drift (ADR-0008).
        assert_eq!(
            STANDARD_BINDING_POWERS.predicate(),
            STANDARD_BINDING_POWERS.binary(&BinaryOperator::Eq(EqualsSpelling::Single)),
        );

        // Moving comparison precedence as dialect data moves the predicate level
        // with it, keeping render parens aligned with how the parser climbed them.
        const TIGHT_COMPARISON: BindingPowerTable = STANDARD_BINDING_POWERS.with_binary(
            &BinaryOperator::Eq(EqualsSpelling::Single),
            BindingPower {
                left: 70,
                right: 71,
                assoc: Assoc::NonAssoc,
            },
        );
        assert_eq!(
            TIGHT_COMPARISON.predicate(),
            TIGHT_COMPARISON.binary(&BinaryOperator::Eq(EqualsSpelling::Single)),
        );
        assert_eq!(TIGHT_COMPARISON.predicate().left, 70);
    }

    #[test]
    fn range_predicate_defaults_to_comparison_and_tracks_its_associativity() {
        // With no override, the range/pattern/membership tier IS the comparison level, so a
        // dialect that re-associates its comparisons (MySQL's `Left` chain) carries the range
        // predicates with it — no independent drift.
        assert_eq!(STANDARD_BINDING_POWERS.range_predicate_override, None);
        assert_eq!(
            STANDARD_BINDING_POWERS.range_predicate(),
            STANDARD_BINDING_POWERS.comparison,
        );
        const LEFT_COMPARISON: BindingPowerTable = STANDARD_BINDING_POWERS.with_binary(
            &BinaryOperator::Eq(EqualsSpelling::Single),
            BindingPower {
                left: 40,
                right: 41,
                assoc: Assoc::Left,
            },
        );
        assert_eq!(LEFT_COMPARISON.range_predicate().assoc, Assoc::Left);
    }

    #[test]
    fn range_predicate_override_ranks_above_comparison_below_any_operator() {
        // The PostgreSQL/Lenient override: range predicates bind tighter than the comparison
        // operators (so `a = b BETWEEN c AND d` groups `a = (b BETWEEN c AND d)`) and looser
        // than the "any other operator" rank, non-associative.
        const PG_RANGE: BindingPowerTable = BindingPowerTable {
            range_predicate_override: Some(RANGE_PREDICATE_ABOVE_COMPARISON),
            ..STANDARD_BINDING_POWERS
        };
        let range = PG_RANGE.range_predicate();
        let comparison = PG_RANGE.binary(&BinaryOperator::Eq(EqualsSpelling::Single));
        assert!(comparison.left < range.left, "tighter than comparison");
        assert!(
            range.left < PG_RANGE.any_operator.left,
            "looser than any-operator"
        );
        assert_eq!(range.assoc, Assoc::NonAssoc);
        // The override leaves the IS-family predicate level (`predicate()`) on comparison.
        assert_eq!(PG_RANGE.predicate(), comparison);
    }

    #[test]
    fn predicate_operands_parenthesize_like_comparisons() {
        // A compound operand of a predicate is parenthesized exactly when it would
        // be under a comparison: looser-binding operands and equal-precedence
        // (non-associative) comparisons need parens; tighter ones do not (ADR-0008).
        let predicate = STANDARD_BINDING_POWERS.predicate();
        let additive = STANDARD_BINDING_POWERS.binary(&BinaryOperator::Plus);
        let or = STANDARD_BINDING_POWERS.binary(&BinaryOperator::Or);
        let comparison =
            STANDARD_BINDING_POWERS.binary(&BinaryOperator::Eq(EqualsSpelling::Single));

        // `a + b IS NULL`: `+` binds tighter, so the operand stays bare.
        assert!(!needs_parens_between(predicate, additive, Side::Left));
        // `(a OR b) IS NULL`: `OR` binds looser, so it must be parenthesized.
        assert!(needs_parens_between(predicate, or, Side::Left));
        // `(a = b) IS NULL`: same precedence, non-associative -> parenthesized.
        assert!(needs_parens_between(predicate, comparison, Side::Left));
        // A `BETWEEN` bound parses on the right at comparison precedence, so the
        // same operands group the same way there.
        assert!(!needs_parens_between(predicate, additive, Side::Right));
        assert!(needs_parens_between(predicate, comparison, Side::Right));
    }

    #[test]
    fn associativity_controls_equal_precedence_parentheses() {
        assert!(!STANDARD_BINDING_POWERS.needs_parens(
            &BinaryOperator::Minus,
            &BinaryOperator::Plus,
            Side::Left
        ));
        assert!(STANDARD_BINDING_POWERS.needs_parens(
            &BinaryOperator::Minus,
            &BinaryOperator::Plus,
            Side::Right
        ));
        assert!(STANDARD_BINDING_POWERS.needs_parens(
            &BinaryOperator::Eq(EqualsSpelling::Single),
            &BinaryOperator::Lt,
            Side::Left
        ));
        assert!(STANDARD_BINDING_POWERS.needs_parens(
            &BinaryOperator::Eq(EqualsSpelling::Single),
            &BinaryOperator::Lt,
            Side::Right
        ));
    }

    #[test]
    fn postgres_postfix_operators_rank_above_arithmetic_in_gram_y_order() {
        // gram.y precedence, lowest to highest: multiplicative < AT TIME ZONE <
        // COLLATE < unary sign < subscript < typecast < field selection (ADR-0008).
        let table = STANDARD_BINDING_POWERS;
        let multiplicative = table.multiplicative.left;
        assert!(multiplicative < table.at_time_zone.left);
        assert!(table.at_time_zone.left < table.collate.left);
        assert!(table.collate.left < table.prefix_sign);
        assert!(table.prefix_sign < table.subscript.left);
        assert!(table.subscript.left < table.typecast.left);
        assert!(table.typecast.left < table.field_selection.left);

        // Every postfix operator is left-associative (`a::int::text` groups left).
        for bp in [
            table.at_time_zone,
            table.collate,
            table.subscript,
            table.typecast,
            table.field_selection,
        ] {
            assert_eq!(bp.assoc, Assoc::Left);
        }
    }

    #[test]
    fn precedence_values_match_m1_ordering() {
        assert_eq!(STANDARD_BINDING_POWERS.binary(&BinaryOperator::Or).left, 10);
        assert_eq!(
            STANDARD_BINDING_POWERS.binary(&BinaryOperator::And).left,
            20
        );
        assert_eq!(STANDARD_BINDING_POWERS.prefix(&UnaryOperator::Not), 30);
        assert_eq!(
            STANDARD_BINDING_POWERS
                .binary(&BinaryOperator::Eq(EqualsSpelling::Single))
                .left,
            40
        );
        assert_eq!(
            STANDARD_BINDING_POWERS.binary(&BinaryOperator::Plus).left,
            50
        );
        assert_eq!(
            STANDARD_BINDING_POWERS
                .binary(&BinaryOperator::Multiply)
                .left,
            60
        );
        assert_eq!(
            STANDARD_BINDING_POWERS
                .binary(&BinaryOperator::StringConcat)
                .left,
            45
        );
        assert_eq!(STANDARD_BINDING_POWERS.prefix(&UnaryOperator::Plus), 80);
        assert_eq!(STANDARD_BINDING_POWERS.prefix(&UnaryOperator::Minus), 80);
    }

    #[test]
    fn mysql_keyword_operators_rank_at_their_documented_levels() {
        use crate::ast::{IntegerDivideSpelling, ModuloSpelling, RegexpSpelling};

        // `XOR` sits strictly between `OR` and `AND` (MySQL `OR < XOR < AND`) and is
        // left-associative like the other boolean operators.
        let or = STANDARD_BINDING_POWERS.binary(&BinaryOperator::Or);
        let xor = STANDARD_BINDING_POWERS.binary(&BinaryOperator::Xor);
        let and = STANDARD_BINDING_POWERS.binary(&BinaryOperator::And);
        assert!(or.left < xor.left);
        assert!(xor.left < and.left);
        assert_eq!(xor.assoc, Assoc::Left);

        // `DIV`/`//` and the `MOD` keyword share the multiplicative level with `*`/`/`/`%`.
        let multiplicative = STANDARD_BINDING_POWERS.binary(&BinaryOperator::Multiply);
        assert_eq!(
            STANDARD_BINDING_POWERS
                .binary(&BinaryOperator::IntegerDivide(IntegerDivideSpelling::Div)),
            multiplicative
        );
        assert_eq!(
            STANDARD_BINDING_POWERS.binary(&BinaryOperator::IntegerDivide(
                IntegerDivideSpelling::SlashSlash
            )),
            multiplicative
        );
        assert_eq!(
            STANDARD_BINDING_POWERS.binary(&BinaryOperator::Modulo(ModuloSpelling::Mod)),
            multiplicative,
        );
        assert_eq!(
            STANDARD_BINDING_POWERS.binary(&BinaryOperator::Modulo(ModuloSpelling::Percent)),
            multiplicative,
        );

        // `RLIKE`/`REGEXP` match at comparison precedence regardless of spelling.
        let comparison =
            STANDARD_BINDING_POWERS.binary(&BinaryOperator::Eq(EqualsSpelling::Single));
        assert_eq!(
            STANDARD_BINDING_POWERS.binary(&BinaryOperator::Regexp(RegexpSpelling::Rlike)),
            comparison,
        );
        assert_eq!(
            STANDARD_BINDING_POWERS.binary(&BinaryOperator::Regexp(RegexpSpelling::Regexp)),
            comparison,
        );
    }

    #[test]
    fn postgres_at_family_operators_bind_at_the_any_operator_rank() {
        let table = STANDARD_BINDING_POWERS;
        let additive = STANDARD_BINDING_POWERS.binary(&BinaryOperator::Plus);
        let comparison =
            STANDARD_BINDING_POWERS.binary(&BinaryOperator::Eq(EqualsSpelling::Single));

        // `@>`/`<@`/`->`/`->>` all fold onto the "any other operator" rank: the same
        // value as `||`, left-associative, tighter than comparison and looser than
        // additive (PostgreSQL `%left Op OPERATOR`, between the two).
        for op in [
            BinaryOperator::Contains,
            BinaryOperator::ContainedBy,
            BinaryOperator::JsonGet,
            BinaryOperator::JsonGetText,
        ] {
            let bp = STANDARD_BINDING_POWERS.binary(&op);
            assert_eq!(bp, table.any_operator);
            assert_eq!(
                bp,
                STANDARD_BINDING_POWERS.binary(&BinaryOperator::StringConcat)
            );
            assert_eq!(bp.assoc, Assoc::Left);
            assert!(comparison.left < bp.left, "tighter than comparison");
            assert!(bp.left < additive.left, "looser than additive");
        }
    }

    #[test]
    fn any_operator_field_is_independent_of_string_concat() {
        // They carry the same default value, but moving `||` must not move the `@`-family
        // (they are distinct fields in the table).
        const HIGH_CONCAT: BindingPowerTable = STANDARD_BINDING_POWERS.with_binary(
            &BinaryOperator::StringConcat,
            BindingPower {
                left: 70,
                right: 71,
                assoc: Assoc::Left,
            },
        );
        assert_eq!(HIGH_CONCAT.binary(&BinaryOperator::StringConcat).left, 70);
        assert_eq!(
            HIGH_CONCAT.binary(&BinaryOperator::Contains),
            STANDARD_BINDING_POWERS.any_operator,
        );
    }

    #[test]
    fn json_get_field_is_independent_of_the_any_operator_rank() {
        // `->` carries the `any_operator` value in the standard table, but moving it
        // (the DuckDB lambda-arrow re-rank) must not move `->>`/`@>`/`<@` — the same
        // distinct-field contract `string_concat` vs `any_operator` upholds.
        const LOOSE_ARROW: BindingPowerTable = STANDARD_BINDING_POWERS.with_binary(
            &BinaryOperator::JsonGet,
            BindingPower {
                left: 4,
                right: 5,
                assoc: Assoc::Left,
            },
        );
        assert_eq!(
            STANDARD_BINDING_POWERS.binary(&BinaryOperator::JsonGet),
            STANDARD_BINDING_POWERS.any_operator,
        );
        assert_eq!(LOOSE_ARROW.binary(&BinaryOperator::JsonGet).left, 4);
        for untouched in [
            BinaryOperator::JsonGetText,
            BinaryOperator::Contains,
            BinaryOperator::ContainedBy,
        ] {
            assert_eq!(
                LOOSE_ARROW.binary(&untouched),
                STANDARD_BINDING_POWERS.any_operator,
            );
        }
    }

    #[test]
    fn standard_bitwise_operators_rank_between_additive_and_comparison() {
        use crate::ast::BitwiseXorSpelling;

        let table = STANDARD_BINDING_POWERS;
        let additive = table.binary(&BinaryOperator::Plus);
        let comparison = table.binary(&BinaryOperator::Eq(EqualsSpelling::Single));

        // PostgreSQL/SQLite/DuckDB place the four binary bitwise operators at one rank
        // (engine-measured), looser than additive and tighter than comparison, all
        // left-associative — the "any other operator" rank `#` also occupies.
        for op in [
            BinaryOperator::BitwiseOr,
            BinaryOperator::BitwiseAnd,
            BinaryOperator::BitwiseShiftLeft,
            BinaryOperator::BitwiseShiftRight,
            BinaryOperator::BitwiseXor(BitwiseXorSpelling::Hash),
        ] {
            let bp = table.binary(&op);
            assert_eq!(bp.assoc, Assoc::Left, "{op:?} is left-associative");
            assert!(comparison.left < bp.left, "{op:?} tighter than comparison");
            assert!(bp.left < additive.left, "{op:?} looser than additive");
        }
        // The two shift spellings share one rank; the two XOR spellings share one rank.
        assert_eq!(
            table.binary(&BinaryOperator::BitwiseShiftLeft),
            table.binary(&BinaryOperator::BitwiseShiftRight),
        );
        assert_eq!(
            table.binary(&BinaryOperator::BitwiseXor(BitwiseXorSpelling::Hash)),
            table.binary(&BinaryOperator::BitwiseXor(BitwiseXorSpelling::Caret)),
        );
    }

    #[test]
    fn standard_prefix_bitwise_not_sits_between_arithmetic_and_the_bitwise_binaries() {
        // PostgreSQL/DuckDB (engine-measured): `~` binds looser than additive so `~ 1 + 1`
        // groups `~ (1 + 1)`, yet tighter-or-equal to the bitwise binaries so `~ 1 & 3`
        // groups `(~ 1) & 3`. The Pratt rule is `child.left > prefix_rbp` captures, so the
        // prefix rank must be `>= bitwise_or.left` and `< additive.left`.
        let table = STANDARD_BINDING_POWERS;
        let rbp = table.prefix(&UnaryOperator::BitwiseNot);
        assert!(
            table.binary(&BinaryOperator::BitwiseOr).left <= rbp,
            "`&`/`|` do not fold into `~`'s operand"
        );
        assert!(
            rbp < table.binary(&BinaryOperator::Plus).left,
            "additive folds into `~`'s operand"
        );
        // Strictly above the bitwise-binary left rank so the renderer parenthesizes
        // `~ (a & b)` — an equal value would drop the required parentheses.
        assert!(rbp > table.binary(&BinaryOperator::BitwiseAnd).left);
    }

    #[test]
    fn bitwise_fields_move_independently() {
        use crate::ast::BitwiseXorSpelling;

        // MySQL's divergence needs `|` < `&` < `<<`/`>>` at distinct ranks, so moving one
        // must not move the others — the ADR-0008 distinct-field contract.
        const TIGHT_AND: BindingPowerTable = STANDARD_BINDING_POWERS.with_binary(
            &BinaryOperator::BitwiseAnd,
            BindingPower {
                left: 48,
                right: 49,
                assoc: Assoc::Left,
            },
        );
        assert_eq!(TIGHT_AND.binary(&BinaryOperator::BitwiseAnd).left, 48);
        assert_eq!(
            TIGHT_AND.binary(&BinaryOperator::BitwiseOr),
            STANDARD_BINDING_POWERS.binary(&BinaryOperator::BitwiseOr),
        );
        assert_eq!(
            TIGHT_AND.binary(&BinaryOperator::BitwiseShiftLeft),
            STANDARD_BINDING_POWERS.binary(&BinaryOperator::BitwiseShiftLeft),
        );
        assert_eq!(
            TIGHT_AND.binary(&BinaryOperator::BitwiseXor(BitwiseXorSpelling::Caret)),
            STANDARD_BINDING_POWERS.binary(&BinaryOperator::BitwiseXor(BitwiseXorSpelling::Caret)),
        );
    }

    #[test]
    fn comparisons_are_non_associative_for_m1() {
        assert_eq!(
            STANDARD_BINDING_POWERS
                .binary(&BinaryOperator::Eq(EqualsSpelling::Single))
                .assoc,
            Assoc::NonAssoc
        );
        assert_eq!(
            STANDARD_BINDING_POWERS
                .binary(&BinaryOperator::NotEq(NotEqSpelling::AngleBracket))
                .assoc,
            Assoc::NonAssoc
        );
        assert_eq!(
            STANDARD_BINDING_POWERS.binary(&BinaryOperator::Lt).assoc,
            Assoc::NonAssoc
        );
        assert_eq!(
            STANDARD_BINDING_POWERS.binary(&BinaryOperator::LtEq).assoc,
            Assoc::NonAssoc
        );
        assert_eq!(
            STANDARD_BINDING_POWERS.binary(&BinaryOperator::Gt).assoc,
            Assoc::NonAssoc
        );
        assert_eq!(
            STANDARD_BINDING_POWERS.binary(&BinaryOperator::GtEq).assoc,
            Assoc::NonAssoc
        );
    }

    #[test]
    fn binding_power_table_supports_const_deltas() {
        const SQLITE_LIKE: BindingPowerTable = STANDARD_BINDING_POWERS.with_binary(
            &BinaryOperator::StringConcat,
            BindingPower {
                left: 70,
                right: 71,
                assoc: Assoc::Left,
            },
        );

        assert_eq!(SQLITE_LIKE.binary(&BinaryOperator::StringConcat).left, 70);
        assert_eq!(
            SQLITE_LIKE.binary(&BinaryOperator::Plus),
            STANDARD_BINDING_POWERS.binary(&BinaryOperator::Plus),
        );

        const LEFT_ASSOC_COMPARISON: BindingPowerTable = STANDARD_BINDING_POWERS.with_binary(
            &BinaryOperator::Lt,
            BindingPower {
                left: 40,
                right: 41,
                assoc: Assoc::Left,
            },
        );

        assert_eq!(
            LEFT_ASSOC_COMPARISON
                .binary(&BinaryOperator::Eq(EqualsSpelling::Single))
                .assoc,
            Assoc::Left,
        );
        assert!(!LEFT_ASSOC_COMPARISON.needs_parens(
            &BinaryOperator::Lt,
            &BinaryOperator::Eq(EqualsSpelling::Single),
            Side::Left,
        ));
        assert!(LEFT_ASSOC_COMPARISON.needs_parens(
            &BinaryOperator::Lt,
            &BinaryOperator::Eq(EqualsSpelling::Single),
            Side::Right,
        ));
    }

    #[test]
    fn set_operation_binding_powers_rank_intersect_above_union_except() {
        let union = STANDARD_SET_OPERATION_BINDING_POWERS.set_operation(&SetOperator::Union);
        let except = STANDARD_SET_OPERATION_BINDING_POWERS.set_operation(&SetOperator::Except);
        let intersect =
            STANDARD_SET_OPERATION_BINDING_POWERS.set_operation(&SetOperator::Intersect);

        assert_eq!(union, except);
        assert!(union.right < intersect.left);
        assert!(intersect.left > except.right);
    }

    #[test]
    fn set_operation_parentheses_are_derived_from_binding_powers() {
        assert!(!STANDARD_SET_OPERATION_BINDING_POWERS.needs_parens(
            &SetOperator::Union,
            &SetOperator::Intersect,
            Side::Right,
        ));
        assert!(STANDARD_SET_OPERATION_BINDING_POWERS.needs_parens(
            &SetOperator::Intersect,
            &SetOperator::Union,
            Side::Left,
        ));
        assert!(STANDARD_SET_OPERATION_BINDING_POWERS.needs_parens(
            &SetOperator::Union,
            &SetOperator::Except,
            Side::Right,
        ));
        assert!(!STANDARD_SET_OPERATION_BINDING_POWERS.needs_parens(
            &SetOperator::Except,
            &SetOperator::Union,
            Side::Left,
        ));
    }

    #[test]
    fn set_operation_binding_power_table_supports_const_deltas() {
        const UNION_IS_HIGHER: SetOperationBindingPowerTable =
            STANDARD_SET_OPERATION_BINDING_POWERS.with_set_operator(
                &SetOperator::Union,
                BindingPower {
                    left: 30,
                    right: 31,
                    assoc: Assoc::Left,
                },
            );

        assert_eq!(UNION_IS_HIGHER.set_operation(&SetOperator::Except).left, 30,);
        assert_eq!(
            UNION_IS_HIGHER.set_operation(&SetOperator::Intersect).left,
            STANDARD_SET_OPERATION_BINDING_POWERS
                .set_operation(&SetOperator::Intersect)
                .left,
        );
    }
}
