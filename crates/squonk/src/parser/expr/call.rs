// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

use crate::ast::{
    ArgSyntax, Expr, FilterWhereSpelling, FromFirstLast, FunctionArg, FunctionCall, Keyword,
    Literal, NullTreatment, ObjectName, OrderByExpr, QuoteStyle, SetQuantifier, Span, Spanned,
    WindowFunctionTail,
};
use crate::error::ParseResult;
use crate::parser::Dialect;
use crate::parser::engine::Parser;
use crate::tokenizer::{Operator, Punctuation, TokenKind};
use thin_vec::ThinVec;

/// The parsed aggregate `FILTER (…)` tail: the optional predicate plus the
/// [`FilterWhereSpelling`] recording whether the source wrote the standard `WHERE`.
type ParsedAggregateFilter<X> = (Option<Box<Expr<X>>>, FilterWhereSpelling);

/// MySQL's built-in *aggregate* function names (MySQL 8.4 Reference Manual §14.19.1),
/// engine-verified on mysql:8: their dedicated aggregate grammar requires at least one
/// argument (or the `COUNT(*)` wildcard), so an empty `COUNT()`/`SUM()`/… is
/// `ER_PARSE_ERROR` (1064); each also grammatically admits an `OVER` window clause. Read
/// case-insensitively against a single unquoted function name under
/// [`AggregateCallSyntax::aggregate_calls_reject_empty_arguments`](crate::ast::dialect::CallSyntax)
/// (empty-arg reject) and the windowable set below (`OVER` restriction).
///
/// `ANY_VALUE` is deliberately absent: on mysql:8 it takes the server's general-function
/// path — empty `ANY_VALUE()` is a wrong-argument-count *semantic* reject (not a 1064
/// syntax error) and `ANY_VALUE(x) OVER ()` is a syntax error — so it is neither
/// empty-rejecting nor windowable. `GROUPING` likewise stays out (windowable side): it
/// rejects `OVER` with 1064.
pub(crate) const MYSQL_AGGREGATE_FUNCTIONS: &[&str] = &[
    "COUNT",
    "SUM",
    "AVG",
    "MIN",
    "MAX",
    "BIT_AND",
    "BIT_OR",
    "BIT_XOR",
    "STD",
    "STDDEV",
    "STDDEV_POP",
    "STDDEV_SAMP",
    "VAR_POP",
    "VAR_SAMP",
    "VARIANCE",
    "GROUP_CONCAT",
    "JSON_ARRAYAGG",
    "JSON_OBJECTAGG",
];

/// MySQL's dedicated *window* function names (MySQL 8.4 Reference Manual §14.20.1) — the
/// non-aggregate functions that only make sense with an `OVER` clause. Together with
/// [`MYSQL_AGGREGATE_FUNCTIONS`] these are the *complete* set on which mysql:8
/// grammatically admits `OVER`; the vocabulary must stay complete because an omission
/// would over-*reject* a valid windowed call (verified on mysql:8: every function outside
/// the union — `ABS`, `PERCENTILE_CONT`, `ANY_VALUE`, `GROUPING`, any user function —
/// rejects a trailing `OVER` with `ER_PARSE_ERROR`). Gated by
/// [`AggregateCallSyntax::over_requires_windowable_function`](crate::ast::dialect::CallSyntax).
pub(crate) const MYSQL_WINDOW_FUNCTIONS: &[&str] = &[
    "ROW_NUMBER",
    "RANK",
    "DENSE_RANK",
    "PERCENT_RANK",
    "CUME_DIST",
    "NTILE",
    "LEAD",
    "LAG",
    "FIRST_VALUE",
    "LAST_VALUE",
    "NTH_VALUE",
];

/// DuckDB's `COALESCE` special form — the one ordinary-looking function whose argument
/// list tolerates a single trailing comma (`coalesce(1, 2,)`), because DuckDB parses it
/// as a grammar special form rather than an ordinary `func_application`. Engine-probed on
/// 1.5.4: every sibling built-in (`greatest`/`least`/`nullif`/`ifnull`/`concat`) and any
/// user function reject the trailing comma, so the tolerance is keyed on this bare name
/// alone. Matched case-insensitively against a single *unquoted* name (via
/// [`function_name_in_set`](Parser::function_name_in_set)), so a quoted `"coalesce"(1, 2,)`
/// or a qualified `main.coalesce(1, 2,)` is an ordinary call that rejects the comma —
/// exactly as the engine does, since the quote/qualification defeats the special form.
const DUCKDB_COALESCE_SPECIAL_FORM: &[&str] = &["coalesce"];
/// The `GROUPING` super-aggregate special form. On the dialects that model the SQL:1999
/// grouping-set constructs ([`GroupingSyntax::grouping_sets`](crate::ast::dialect::SelectSyntax)) —
/// PostgreSQL, ANSI, DuckDB — `GROUPING` is a reserved grammar head with a mandatory
/// non-empty argument list (`GROUPING '(' expr_list ')'`), so a bare `GROUPING()` is a
/// `syntax error at or near ")"` there (probed on DuckDB 1.5.4 and libpg_query). Matched
/// case-insensitively against a single *unquoted* name (via
/// [`function_name_in_set`](Parser::function_name_in_set)), so a quoted `"grouping"()` or a
/// qualified `s.grouping()` stays an ordinary call — the engines accept those, since the
/// quote/qualification defeats the reserved special form. Off the flag (SQLite/MySQL),
/// `grouping` is an ordinary function name whose empty call parses cleanly (its error, if
/// any, is name resolution).
const GROUPING_SPECIAL_FORM: &[&str] = &["grouping"];
/// The fixed positional-argument arity `(name, min, max)` MySQL's dedicated window-function
/// grammar (MySQL 8.4 Reference Manual §14.20.1) baked into each production, so a call
/// outside these bounds is a grammar-level `ER_PARSE_ERROR` (1064) on mysql:8 — *not* the
/// generic call's argument-count *binding* error — engine-verified: `ROW_NUMBER(1)`,
/// `NTILE()`, `NTILE(4, 5)`, `FIRST_VALUE(a, b)`, `NTH_VALUE(a)`, `NTH_VALUE(a, 2, 3)`, and
/// `LEAD(a, 2, 3, 4)` are all 1064. `LEAD`/`LAG` span 1–3 (the optional `, N [, default]`
/// offset/default tail); the others are exact. The `stable_integer`/default *argument-kind*
/// restrictions on `LEAD`/`LAG`'s 2nd/3rd args are a name-*binding* reject (`LEAD(a, b)` is
/// `ER_SP_UNDECLARED_VAR`, 1327, not 1064), so they are out of scope for the non-binding
/// parser — only the count is a syntax rule.
///
/// A test asserts this table names exactly the same 11 functions as
/// [`MYSQL_WINDOW_FUNCTIONS`], so the two stay in lockstep.
const MYSQL_WINDOW_FUNCTION_ARITY: &[(&str, usize, usize)] = &[
    ("ROW_NUMBER", 0, 0),
    ("RANK", 0, 0),
    ("DENSE_RANK", 0, 0),
    ("PERCENT_RANK", 0, 0),
    ("CUME_DIST", 0, 0),
    ("NTILE", 1, 1),
    ("FIRST_VALUE", 1, 1),
    ("LAST_VALUE", 1, 1),
    ("LEAD", 1, 3),
    ("LAG", 1, 3),
    ("NTH_VALUE", 2, 2),
];

/// The MySQL window functions whose dedicated grammar admits the post-`)` null-treatment
/// tail (`{RESPECT | IGNORE} NULLS`) — the *value* subset of `MYSQL_WINDOW_FUNCTIONS`, as
/// opposed to the rank/ntile functions that reject it (`ROW_NUMBER() RESPECT NULLS` →
/// `ER_PARSE_ERROR` 1064 on mysql:8). Engine-verified: `FIRST_VALUE`/`LAST_VALUE`/`LEAD`/
/// `LAG`/`NTH_VALUE` grammatically admit it (`RESPECT NULLS` accepts, `IGNORE NULLS`
/// feature-rejects 1235). `NTH_VALUE` additionally admits the `FROM {FIRST | LAST}` frame
/// clause, keyed separately in [`parse_window_function_tail`](Parser::parse_window_function_tail).
const MYSQL_NULL_TREATMENT_WINDOW_FUNCTIONS: &[&str] =
    &["FIRST_VALUE", "LAST_VALUE", "LEAD", "LAG", "NTH_VALUE"];

/// PostgreSQL's SQL/JSON constructor keywords that reject an empty argument list — their
/// dedicated `gram.y` productions (`json_aggregate_func` siblings / `func_expr_common_subexpr`)
/// require the context-item / value argument, so `JSON()`, `JSON_SCALAR()`, and
/// `JSON_SERIALIZE()` are `syntax error at or near ")"` on `pg_query`, whereas the generic
/// call path would otherwise admit them as ordinary niladic calls. Matched case-insensitively
/// against a single *unquoted* name under
/// [`CallSyntax::sqljson_constructors_require_argument`](crate::ast::dialect::CallSyntax), so a
/// quoted `"json"()` stays a general call PostgreSQL accepts (rejected only at name resolution).
///
/// The set is deliberately narrow and extension-shaped: the future SQL/JSON expression-function
/// grammar (`pg-sqljson-expression-functions`: `JSON_VALUE`/`JSON_QUERY`/… with `RETURNING`/
/// `FORMAT`/`ON ERROR` tails) replaces these three names with full productions that carry the
/// same "argument is mandatory" floor, so this gate retires into that grammar as it lands.
const PG_SQLJSON_EMPTY_REJECTING_CONSTRUCTORS: &[&str] = &["JSON", "JSON_SCALAR", "JSON_SERIALIZE"];

impl<'a, D: Dialect> Parser<'a, D> {
    /// Parse a function or aggregate call after its name: `name([ALL | DISTINCT] args | *)`.
    ///
    /// `start` is the span of the name's first token, so the call's span covers the
    /// whole `name(...)` extent. A leading `*` is the wildcard argument (`count(*)`)
    /// and is mutually exclusive with a positional list; an empty `()` is a no-arg
    /// call. The trailing aggregate modifiers are parsed in PostgreSQL's grammar order:
    /// the in-parenthesis `ORDER BY`, then `WITHIN GROUP (ORDER BY …)`, `FILTER
    /// (WHERE …)`, and `OVER …` after the closing `)`.
    pub(in crate::parser) fn parse_function_call(
        &mut self,
        name: ObjectName,
        start: Span,
    ) -> ParseResult<FunctionCall<D::Ext>> {
        // MySQL's default `IGNORE_SPACE`-off tokenizer reads a built-in aggregate whose `(`
        // is not adjacent to its name as a general/stored-function reference, where the
        // aggregate-only argument forms are illegal. Adjacency is a source-offset test: any
        // trivia (space, comment, newline) between the name's last token and `(` leaves a
        // gap, so `count /*c*/(1)` and the newline forms read non-adjacent, matching MySQL.
        let name_adjacent_paren = name.span().end() == self.current_span()?.start();
        self.advance()?; // consume `(`
        let quantifier = self.parse_aggregate_quantifier()?;
        let (wildcard, args) = if quantifier.is_none()
            && self.peek_is_op(Operator::Star)?
            && !self.peek_is_columns_unpack_prefix()?
        {
            // A leading `*` is the `count(*)` wildcard argument — unless it opens
            // DuckDB's `*COLUMNS(...)` unpack prefix (`struct_pack(*COLUMNS(*))`), which
            // flows into the positional-argument grammar as an ordinary expression.
            self.advance()?; // consume `*`
            (true, ThinVec::new())
        } else if self.peek_is_punct(Punctuation::RParen)? {
            (false, ThinVec::new())
        } else if self
            .features()
            .aggregate_call_syntax
            .standalone_argument_order_by
            && self.peek_is_keyword(Keyword::Order)?
            && self.peek_nth_is_keyword(1, Keyword::By)?
        {
            // DuckDB lets a window/rank function carry its ordering as a bare
            // in-parenthesis `ORDER BY` with no positional argument
            // (`rank(ORDER BY x) OVER w`); the `ORDER BY` itself is consumed by the shared
            // `parse_aggregate_order_by` below (the same field the argument-then-`ORDER BY`
            // form fills), so here only the empty positional list is recorded. A trailing
            // comma inside that list still rejects — the sort clause routes through the
            // non-trailing `parse_comma_separated`, matching DuckDB (probed on 1.5.4).
            (false, ThinVec::new())
        } else {
            // DuckDB's `COALESCE` special form tolerates a single trailing comma in its
            // argument list; an ordinary `func_application` does not. The flag gate is
            // read first so non-tolerant dialects skip the name test on the hot path.
            let trailing_comma = self.features().select_syntax.trailing_comma
                && self.function_name_in_set(&name, DUCKDB_COALESCE_SPECIAL_FORM);
            (false, self.parse_function_args(trailing_comma)?)
        };
        // The `VARIADIC` array-spread marker is admissible only on the *last* argument and
        // never alongside an `ALL`/`DISTINCT` quantifier — both PostgreSQL and DuckDB
        // parse-reject the other positions (engine-probed), because their `gram.y`
        // `func_application` productions place `VARIADIC func_arg_expr` last and carry no
        // quantifier. The per-argument flag is only ever set under the gating dialects, so
        // this check is inert off-flag. The first offending argument (a non-final `VARIADIC`,
        // which also catches a second `VARIADIC`) is reported at its own span.
        if let Some(arg) = args
            .iter()
            .enumerate()
            .find_map(|(idx, a)| (a.variadic && idx + 1 != args.len()).then_some(a))
        {
            let bad = arg.span();
            return Err(self.error_at(
                bad,
                "VARIADIC only on the final argument: the array-spread marker must prefix the \
                 last argument of the call",
                self.span_text(bad).to_owned(),
            ));
        }
        if quantifier.is_some() {
            if let Some(arg) = args.iter().find(|a| a.variadic) {
                let bad = arg.span();
                return Err(self.error_at(
                    bad,
                    "no ALL/DISTINCT quantifier: a VARIADIC argument cannot combine with an \
                     aggregate ALL/DISTINCT quantifier",
                    self.span_text(bad).to_owned(),
                ));
            }
        }
        let order_by = self.parse_aggregate_order_by()?;
        let separator = self.parse_group_concat_separator()?;
        let null_treatment = self.parse_null_treatment()?;
        self.expect_punct(Punctuation::RParen, "`)` to close the function call")?;
        let within_group = self.parse_within_group()?;
        let (filter, filter_where) = self.parse_aggregate_filter()?;
        // MySQL's window-function post-`)` tail sits between the argument `)` and the
        // `OVER` clause, so it is parsed here — after the other post-`)` aggregate
        // clauses (which MySQL leaves off) and immediately before `OVER`.
        let window_tail = self.parse_window_function_tail(&name)?;
        let over = self.parse_over_clause()?;

        let span = start.union(self.preceding_span());
        // MySQL's pure window functions (`MYSQL_WINDOW_FUNCTIONS`) are reserved-word call
        // heads recognized by a dedicated grammar; `Some((min, max))` is their fixed
        // positional arity when `name` is a single unquoted one of them. Threaded into the
        // adjacency exemption and the window-grammar gate below.
        let window_arity = self.mysql_window_function_arity(&name);
        // WITHIN GROUP occupies PostgreSQL's aggregate ORDER BY slot and marks an
        // ordered-set aggregate, so PostgreSQL rejects it at parse time alongside an
        // in-parenthesis ORDER BY or a DISTINCT quantifier (gram.y `func_expr`); reject
        // the same combinations so the two parsers agree on them.
        if within_group.is_some() {
            if !order_by.is_empty() {
                return Err(self.error_at(
                    span,
                    "a single ORDER BY: an in-parenthesis ORDER BY cannot combine with WITHIN GROUP",
                    self.span_text(span).to_owned(),
                ));
            }
            if matches!(quantifier, Some(SetQuantifier::Distinct)) {
                return Err(self.error_at(
                    span,
                    "no DISTINCT: a WITHIN GROUP ordered-set aggregate cannot be DISTINCT",
                    self.span_text(span).to_owned(),
                ));
            }
        }
        // MySQL demotes a spaced built-in *aggregate* to a general call, where its
        // built-in-only argument and tail grammar — a `*` wildcard, a leading
        // `DISTINCT`/`ALL` quantifier, an in-parenthesis `ORDER BY`, a `SEPARATOR` tail, or a
        // window `OVER` clause — is a syntax error (`COUNT ( * )`, `MAX ( ALL 1 )`,
        // `SUM ( 1 ) OVER ()` → engine-measured 1064). A normal-argument spaced call
        // (`count (1)`) uses none of these forms, so it parses unchanged (it fails only at
        // name resolution, a binding not a syntax error). The `EXTRACT (…)` special form is
        // rejected on the same adjacency rule in `parse_extract`. The pure *window*
        // functions are exempt (`window_arity.is_none()`): their names are reserved tokens
        // the tokenizer recognizes regardless of spacing, so `row_number () OVER ()` parses
        // on mysql:8 — their own grammar is enforced by the window-grammar gate below.
        if self
            .features()
            .aggregate_call_syntax
            .aggregate_args_require_adjacent_paren
            && !name_adjacent_paren
            && window_arity.is_none()
            && (wildcard
                || quantifier.is_some()
                || !order_by.is_empty()
                || separator.is_some()
                || over.is_some())
        {
            return Err(self.error_at(
                span,
                "the function name adjacent to `(`: a built-in aggregate/window argument or \
                 tail form (`*`, `DISTINCT`/`ALL`, `ORDER BY`, `SEPARATOR`, or `OVER`) \
                 requires no space before the parentheses",
                self.span_text(span).to_owned(),
            ));
        }
        // MySQL's dedicated aggregate grammar requires an argument (or the `COUNT(*)`
        // wildcard), so an empty `COUNT()`/`SUM()`/… is `ER_PARSE_ERROR` (1064) on mysql:8,
        // while a niladic non-aggregate built-in (`NOW()`) or an empty user-function call is
        // accepted (the latter fails only at name resolution). Keyed on the single unquoted
        // aggregate name, so a qualified `db.count()` or a quoted `` `count`() `` stays a
        // general call.
        if self
            .features()
            .aggregate_call_syntax
            .aggregate_calls_reject_empty_arguments
            && !wildcard
            && args.is_empty()
            && self.function_name_in_set(&name, MYSQL_AGGREGATE_FUNCTIONS)
        {
            return Err(self.error_at(
                span,
                "at least one argument: a MySQL built-in aggregate rejects an empty \
                 argument list (use `COUNT(*)` for the row count)",
                self.span_text(span).to_owned(),
            ));
        }
        // `GROUPING()` with no arguments is a parse error on the dialects that model the
        // SQL:1999 grouping-set constructs: their `GROUPING '(' expr_list ')'` grammar
        // production requires a non-empty argument list, so a bare `GROUPING()` is `syntax
        // error at or near ")"` (probed on DuckDB 1.5.4 and libpg_query — both parse-reject
        // it, unlike the arity/placement checks a pure parser leaves to the binder). Gated
        // by the same `grouping_sets` flag that turns the grouping constructs on, since a
        // dialect without them treats `grouping` as an ordinary function name.
        if self.features().grouping_syntax.grouping_sets
            && !wildcard
            && args.is_empty()
            && self.function_name_in_set(&name, GROUPING_SPECIAL_FORM)
        {
            return Err(self.error_at(
                span,
                "at least one argument: GROUPING requires a non-empty argument list",
                self.span_text(span).to_owned(),
            ));
        }
        // PostgreSQL's SQL/JSON constructors require their context-item / value argument, so
        // an empty `JSON()`/`JSON_SCALAR()`/`JSON_SERIALIZE()` is a syntax error (the generic
        // call path would otherwise admit the niladic form). Keyed on a single unquoted name
        // so a quoted `"json"()` stays a general call; the arity floor is shaped to extend
        // into the future JSON_VALUE/JSON_QUERY grammar.
        if self
            .features()
            .call_syntax
            .sqljson_constructors_require_argument
            && !wildcard
            && args.is_empty()
            && self.function_name_in_set(&name, PG_SQLJSON_EMPTY_REJECTING_CONSTRUCTORS)
        {
            return Err(self.error_at(
                span,
                "the SQL/JSON context-item argument: `JSON`/`JSON_SCALAR`/`JSON_SERIALIZE` \
                 reject an empty argument list",
                self.span_text(span).to_owned(),
            ));
        }
        // MySQL admits an `OVER` window clause only on a windowable function — the built-in
        // aggregates ∪ the dedicated window functions — so `OVER` on a scalar built-in or a
        // user function (`PERCENTILE_CONT(x, 0.5) OVER ()`, `ABS(x) OVER ()`) is
        // `ER_PARSE_ERROR` (1064) on mysql:8. The windowable vocabulary is complete
        // (an omission would over-reject a valid windowed call); a qualified name is not a
        // single-part member and is rejected here too, matching the engine.
        if over.is_some()
            && self
                .features()
                .aggregate_call_syntax
                .over_requires_windowable_function
            && !self.function_name_in_set(&name, MYSQL_AGGREGATE_FUNCTIONS)
            && !self.function_name_in_set(&name, MYSQL_WINDOW_FUNCTIONS)
        {
            return Err(self.error_at(
                span,
                "a windowable function: MySQL admits `OVER` only on an aggregate or window \
                 function, not on an ordinary scalar or user function",
                self.span_text(span).to_owned(),
            ));
        }
        // The converse half of MySQL's dedicated window-function grammar: once one of the 11
        // pure window functions is admitted as a call head (they are reserved words carved
        // out of `MYSQL_RESERVED_FUNCTION_NAME`), it carries requirements the generic call it
        // shares a shape with does not. Unlike the aggregates — whose `OVER` is optional and
        // whose arity is a binding concern — each window function *requires* an `OVER` clause,
        // takes a *fixed* positional argument count, and admits none of the aggregate-only
        // argument forms; every violation is `ER_PARSE_ERROR` (1064) on mysql:8. Gated by the
        // same windowable-function flag as the `OVER` restriction above (one indivisible
        // grammar) and keyed on a single unquoted name — a qualified/quoted spelling takes the
        // general-call path and is rejected there.
        if let Some((min_args, max_args)) = window_arity.filter(|_| {
            self.features()
                .aggregate_call_syntax
                .over_requires_windowable_function
        }) {
            if over.is_none() {
                return Err(self.error_at(
                    span,
                    "an OVER clause: a MySQL window function (ROW_NUMBER, RANK, LEAD, …) \
                     requires a windowing OVER clause",
                    self.span_text(span).to_owned(),
                ));
            }
            if wildcard || quantifier.is_some() || !order_by.is_empty() || separator.is_some() {
                return Err(self.error_at(
                    span,
                    "a plain argument list: a MySQL window function rejects the aggregate-only \
                     forms (`*`, a `DISTINCT`/`ALL` quantifier, an in-parenthesis `ORDER BY`, \
                     and `SEPARATOR`)",
                    self.span_text(span).to_owned(),
                ));
            }
            if args.len() < min_args || args.len() > max_args {
                let expected = if min_args == max_args {
                    format!(
                        "exactly {min_args} argument(s): this MySQL window function takes a fixed argument count"
                    )
                } else {
                    format!(
                        "{min_args} to {max_args} arguments: this MySQL window function takes a fixed argument count"
                    )
                };
                return Err(self.error_at(span, expected, self.span_text(span).to_owned()));
            }
        }
        let call_meta = self.make_meta(span);
        Ok(FunctionCall {
            name,
            quantifier,
            args,
            wildcard,
            order_by,
            separator,
            within_group,
            filter,
            filter_where,
            over,
            null_treatment,
            window_tail,
            meta: call_meta,
        })
    }
    /// Whether `name` is a single *unquoted* part whose spelling (ASCII case-insensitive)
    /// is in `set` — the shape a MySQL built-in function name takes: a bare word, never a
    /// qualified or quoted name. A qualified `db.count` reaches the server's general
    /// stored-function path, and a quoted `` `count` `` is likewise a general reference, so
    /// neither is the built-in whose arity/window rules `set` encodes. Mirrors
    /// [`is_mysql_cast_target`](Self::is_mysql_cast_target)'s name allowlist.
    fn function_name_in_set(&self, name: &ObjectName, set: &[&str]) -> bool {
        let [part] = name.0.as_slice() else {
            return false;
        };
        if part.quote != QuoteStyle::None {
            return false;
        }
        let text = self.span_text(part.meta.span);
        set.iter()
            .any(|candidate| text.eq_ignore_ascii_case(candidate))
    }
    /// The fixed positional-argument arity `(min, max)` MySQL's dedicated window-function
    /// grammar assigns to `name`, or `None` when `name` is not a single *unquoted* pure
    /// window function — the same single-part/unquoted shape
    /// [`function_name_in_set`](Self::function_name_in_set) keys on, since a qualified
    /// `db.row_number` or a quoted `` `row_number` `` is a general stored-function
    /// reference (rejected by the `OVER`-restriction gate), not the reserved-word window
    /// head. `Some` doubles as the "is a pure window function" predicate the adjacency
    /// exemption and the window-grammar gate share. Backed by
    /// [`MYSQL_WINDOW_FUNCTION_ARITY`].
    fn mysql_window_function_arity(&self, name: &ObjectName) -> Option<(usize, usize)> {
        let [part] = name.0.as_slice() else {
            return None;
        };
        if part.quote != QuoteStyle::None {
            return None;
        }
        let text = self.span_text(part.meta.span);
        MYSQL_WINDOW_FUNCTION_ARITY
            .iter()
            .find(|(candidate, _, _)| text.eq_ignore_ascii_case(candidate))
            .map(|&(_, min_args, max_args)| (min_args, max_args))
    }
    /// Parse the optional `IGNORE NULLS` / `RESPECT NULLS` null-treatment written
    /// *inside* the call parentheses, after any in-parenthesis `ORDER BY` and
    /// `SEPARATOR`, gated by [`AggregateCallSyntax::null_treatment`]. DuckDB spells the SQL:2016
    /// null-treatment here rather than after the `)` (the standard's post-`)` position
    /// engine-rejects on 1.5.4); only the two-token `IGNORE NULLS`/`RESPECT NULLS` pair
    /// opens it, so a bare `ignore`/`respect` stays unconsumed. When the dialect leaves
    /// the flag off, the keyword is left for the unmatched `)` to reject cleanly.
    ///
    /// [`AggregateCallSyntax::null_treatment`]: crate::ast::dialect::AggregateCallSyntax::null_treatment
    fn parse_null_treatment(&mut self) -> ParseResult<Option<NullTreatment>> {
        if !self.features().aggregate_call_syntax.null_treatment {
            return Ok(None);
        }
        let treatment = if self.peek_is_keyword(Keyword::Ignore)?
            && self.peek_nth_is_keyword(1, Keyword::Nulls)?
        {
            NullTreatment::IgnoreNulls
        } else if self.peek_is_keyword(Keyword::Respect)?
            && self.peek_nth_is_keyword(1, Keyword::Nulls)?
        {
            NullTreatment::RespectNulls
        } else {
            return Ok(None);
        };
        self.advance()?; // IGNORE / RESPECT
        self.advance()?; // NULLS
        Ok(Some(treatment))
    }
    /// Parse MySQL's optional window-function post-`)` tail —
    /// `[FROM {FIRST | LAST}] [{RESPECT | IGNORE} NULLS]` written between a null-treatment
    /// window function's argument `)` and its `OVER` clause — gated by
    /// [`AggregateCallSyntax::window_function_tail`]. Only the mysql:8-*accepted* surface is
    /// consumed, so the parser's accept/reject tracks the engine's:
    ///
    /// - `FROM FIRST` is taken only on a single unquoted `NTH_VALUE`; `FROM LAST` is left
    ///   unconsumed (mysql feature-rejects it, `ER_NOT_SUPPORTED_YET` 1235), as is `FROM`
    ///   on any other window function (`ER_PARSE_ERROR` 1064).
    /// - `RESPECT NULLS` is taken only on the null-treatment window functions
    ///   ([`MYSQL_NULL_TREATMENT_WINDOW_FUNCTIONS`]); `IGNORE NULLS` is left unconsumed
    ///   (mysql feature-rejects it, 1235), as is either on a rank/ntile function (1064).
    ///
    /// The two clauses are consumed in this fixed order (`FROM` before the null
    /// treatment); a reversed or otherwise unadmitted spelling is left for the downstream
    /// `OVER` expectation to reject — the window-grammar gate then requires the `OVER`
    /// these functions cannot omit, so the leftover keyword surfaces as a clean reject,
    /// exactly as mysql:8 rejects it. Returns `Some` only when a clause was consumed.
    /// `name` is threaded to key the per-function admission on a single unquoted
    /// spelling, mirroring the arity and `OVER` gates (a quoted/qualified name takes the
    /// general-call path and consumes no tail).
    ///
    /// [`AggregateCallSyntax::window_function_tail`]: crate::ast::dialect::AggregateCallSyntax::window_function_tail
    fn parse_window_function_tail(
        &mut self,
        name: &ObjectName,
    ) -> ParseResult<Option<WindowFunctionTail>> {
        if !self.features().aggregate_call_syntax.window_function_tail {
            return Ok(None);
        }
        let from_first_last = if self.function_name_in_set(name, &["NTH_VALUE"])
            && self.peek_is_keyword(Keyword::From)?
            && self.peek_nth_is_keyword(1, Keyword::First)?
        {
            self.advance()?; // FROM
            self.advance()?; // FIRST
            Some(FromFirstLast::First)
        } else {
            None
        };
        let null_treatment = if self
            .function_name_in_set(name, MYSQL_NULL_TREATMENT_WINDOW_FUNCTIONS)
            && self.peek_is_keyword(Keyword::Respect)?
            && self.peek_nth_is_keyword(1, Keyword::Nulls)?
        {
            self.advance()?; // RESPECT
            self.advance()?; // NULLS
            Some(NullTreatment::RespectNulls)
        } else {
            None
        };
        Ok(if from_first_last.is_none() && null_treatment.is_none() {
            None
        } else {
            Some(WindowFunctionTail {
                from_first_last,
                null_treatment,
            })
        })
    }
    /// Parse an optional MySQL `GROUP_CONCAT(... SEPARATOR '<sep>')` delimiter tail,
    /// gated by [`AggregateCallSyntax::group_concat_separator`]. The `SEPARATOR` keyword
    /// sits inside the call parentheses, after any in-parenthesis `ORDER BY` and before
    /// the closing `)` — mirroring where the in-call `ORDER BY` is admitted. When the
    /// dialect leaves the flag off, `SEPARATOR` is left unconsumed and the unmatched `)`
    /// surfaces as a clean parse error. The delimiter is always a string constant, so it
    /// is captured as a bare [`Literal`], not a general expression.
    ///
    /// [`AggregateCallSyntax::group_concat_separator`]: crate::ast::dialect::ExpressionSyntax
    fn parse_group_concat_separator(&mut self) -> ParseResult<Option<Literal>> {
        if !(self.features().aggregate_call_syntax.group_concat_separator
            && self.peek_is_contextual_keyword("SEPARATOR")?)
        {
            return Ok(None);
        }
        self.expect_contextual_keyword("SEPARATOR")?;
        let Some(token) = self.peek()? else {
            return Err(self.unexpected("a string literal after `SEPARATOR`"));
        };
        if token.kind != TokenKind::String {
            return Err(self.unexpected("a string literal after `SEPARATOR`"));
        }
        let Expr::Literal { literal, .. } = self.parse_string_literal(token)? else {
            unreachable!("parse_string_literal always yields Expr::Literal");
        };
        Ok(Some(literal))
    }
    /// Parse a non-empty comma-separated function-argument list, each a positional
    /// value or a PostgreSQL named argument (`name => value` / `name := value`).
    ///
    /// `trailing_comma` discards a single dangling comma before the closing `)` — set
    /// only for DuckDB's `COALESCE` special form (see [`DUCKDB_COALESCE_SPECIAL_FORM`]).
    /// The [`parse_comma_separated_trailing`](Parser::parse_comma_separated_trailing)
    /// combinator re-checks the dialect flag, so an ordinary call always parses as the
    /// plain `parse_comma_separated`.
    fn parse_function_args(
        &mut self,
        trailing_comma: bool,
    ) -> ParseResult<ThinVec<FunctionArg<D::Ext>>> {
        if trailing_comma {
            self.parse_comma_separated_trailing(Self::parse_function_arg, |p| {
                p.trailing_comma_at(Punctuation::RParen)
            })
        } else {
            self.parse_comma_separated(Self::parse_function_arg)
        }
    }
    /// Parse one function argument: a PostgreSQL named argument `name => value` /
    /// `name := value` when the dialect enables it and a name-then-arrow leads, else
    /// a bare positional value.
    ///
    /// One canonical [`FunctionArg`] shape carries both: a positional
    /// argument is `{ name: None, syntax: Positional, value }`, while a named one
    /// records the [`ArgSyntax`] arrow the source wrote so it round-trips. The name
    /// is a `ColId`; the `name =>` lookahead is unambiguous because a positional
    /// expression can never have `=>` / `:=` in infix position.
    ///
    /// A leading `VARIADIC` array-spread marker (gated by
    /// [`CallSyntax::variadic_argument`](crate::ast::dialect::CallSyntax)) is consumed here
    /// and recorded on the argument; it precedes the optional named-argument arrow
    /// (`VARIADIC name => value` is a valid engine form). The last-position and
    /// quantifier-exclusion rules the marker carries are enforced once the whole list is
    /// known, in [`parse_function_call`](Self::parse_function_call). When the flag is off
    /// the `VARIADIC` keyword is left for the expression grammar, where it surfaces as a
    /// clean parse error.
    fn parse_function_arg(&mut self) -> ParseResult<FunctionArg<D::Ext>> {
        let arg_start = self.current_span()?;
        let variadic =
            self.features().call_syntax.variadic_argument && self.eat_keyword(Keyword::Variadic)?;
        if let Some(syntax) = self.peek_named_arg_separator()? {
            let name = self.parse_ident()?.sym;
            self.advance()?; // the `=>` / `:=` separator
            let value = self.parse_expr()?;
            let span = arg_start.union(value.span());
            let meta = self.make_meta(span);
            return Ok(FunctionArg {
                name: Some(name),
                variadic,
                syntax,
                value,
                meta,
            });
        }
        let value = self.parse_expr()?;
        let span = arg_start.union(value.span());
        let meta = self.make_meta(span);
        Ok(FunctionArg {
            name: None,
            variadic,
            syntax: ArgSyntax::Positional,
            value,
            meta,
        })
    }
    /// Peek a named-argument separator: `Some` when the dialect enables named
    /// arguments and the cursor is a name immediately followed by `=>` (current) or
    /// `:=` (deprecated). The cursor is not advanced.
    fn peek_named_arg_separator(&mut self) -> ParseResult<Option<ArgSyntax>> {
        if !self.features().call_syntax.named_argument {
            return Ok(None);
        }
        let Some(name) = self.peek()? else {
            return Ok(None);
        };
        if !self.token_can_be_column_name(name) {
            return Ok(None);
        }
        Ok(match self.peek_nth(1)? {
            Some(token) if token.kind == TokenKind::Operator(Operator::Arrow) => {
                Some(ArgSyntax::Arrow)
            }
            Some(token) if token.kind == TokenKind::Operator(Operator::ColonEquals) => {
                Some(ArgSyntax::ColonEquals)
            }
            _ => None,
        })
    }
    /// Parse the optional `ALL` / `DISTINCT` quantifier on an aggregate's argument
    /// list, as in `count(DISTINCT x)` or `sum(ALL x)`; `None` when none is written.
    fn parse_aggregate_quantifier(&mut self) -> ParseResult<Option<SetQuantifier>> {
        if self.eat_keyword(Keyword::All)? {
            Ok(Some(SetQuantifier::All))
        } else if self.eat_keyword(Keyword::Distinct)? {
            Ok(Some(SetQuantifier::Distinct))
        } else {
            Ok(None)
        }
    }
    /// Parse an optional aggregate `FILTER (WHERE <predicate>)` clause, gated by
    /// [`AggregateCallSyntax::aggregate_filter`](crate::ast::dialect::CallSyntax).
    ///
    /// Only consumed when the dialect enables it and `FILTER` is immediately followed by
    /// `(`, so a bare `filter` after a call stays usable as an alias (it is a non-reserved
    /// keyword). Dialects without the clause (MySQL, SQLite) leave `FILTER` unconsumed, so
    /// the trailing `(WHERE …)` surfaces as a clean parse error.
    pub(super) fn parse_aggregate_filter(&mut self) -> ParseResult<ParsedAggregateFilter<D::Ext>> {
        if !(self.features().aggregate_call_syntax.aggregate_filter
            && self.peek_is_keyword(Keyword::Filter)?
            && self.peek_nth_is_punct(1, Punctuation::LParen)?)
        {
            return Ok((None, FilterWhereSpelling::Where));
        }
        self.advance()?; // FILTER
        self.expect_punct(Punctuation::LParen, "`(` after `FILTER`")?;
        // The SQL-standard body opens with `WHERE`; DuckDB drops it
        // (`filter_optional_where`). When the keyword is optional, a present `WHERE` is
        // still consumed so the two spellings converge on the same predicate, and the
        // omission is recorded so the render round-trips it.
        let where_spelling = if self.features().aggregate_call_syntax.filter_optional_where {
            if self.eat_keyword(Keyword::Where)? {
                FilterWhereSpelling::Where
            } else {
                FilterWhereSpelling::Omitted
            }
        } else {
            self.expect_keyword(Keyword::Where)?;
            FilterWhereSpelling::Where
        };
        let predicate = self.parse_expr()?;
        self.expect_punct(Punctuation::RParen, "`)` to close the `FILTER` clause")?;
        Ok((Some(Box::new(predicate)), where_spelling))
    }
    /// Parse an optional `WITHIN GROUP (ORDER BY <keys>)` ordered-set aggregate clause
    /// (SQL:2008 T612/T614), as in `percentile_cont(0.5) WITHIN GROUP (ORDER BY x)`.
    ///
    /// Only consumed when `WITHIN` is immediately followed by `GROUP`, so a bare
    /// `within` after a call stays usable as an alias (it is a non-reserved `AS_LABEL`
    /// keyword), mirroring the `FILTER` guard. The sort key is required: the standard's
    /// `within_group_clause` wraps a non-empty `sort_clause`, so an empty list is a
    /// syntax error rather than an absent clause.
    fn parse_within_group(&mut self) -> ParseResult<Option<ThinVec<OrderByExpr<D::Ext>>>> {
        if !self.features().aggregate_call_syntax.within_group {
            // Dialects without ordered-set aggregates (SQLite) leave `WITHIN` unconsumed,
            // so the trailing clause surfaces as a clean parse error.
            return Ok(None);
        }
        if !(self.peek_is_keyword(Keyword::Within)?
            && self.peek_nth_is_keyword(1, Keyword::Group)?)
        {
            return Ok(None);
        }
        self.advance()?; // WITHIN
        self.advance()?; // GROUP
        self.expect_punct(Punctuation::LParen, "`(` after `WITHIN GROUP`")?;
        let order_by = self.parse_aggregate_order_by()?;
        if order_by.is_empty() {
            return Err(self.unexpected("`ORDER BY` inside `WITHIN GROUP (…)`"));
        }
        self.expect_punct(
            Punctuation::RParen,
            "`)` to close the `WITHIN GROUP` clause",
        )?;
        Ok(Some(order_by))
    }
}

#[cfg(test)]
mod tests {
    use super::{MYSQL_WINDOW_FUNCTION_ARITY, MYSQL_WINDOW_FUNCTIONS};

    #[test]
    fn window_function_arity_table_covers_exactly_the_window_functions() {
        // The membership list (`MYSQL_WINDOW_FUNCTIONS`, used by the OVER-restriction gate
        // and mirrored by the ast crate's reserved-word carve-out) and the arity table must
        // stay in lockstep: a name in one but not the other would either admit a window
        // function with no arity rule (over-acceptance) or attach an arity rule to a
        // non-window name.
        for &(name, min_args, max_args) in MYSQL_WINDOW_FUNCTION_ARITY {
            assert!(
                MYSQL_WINDOW_FUNCTIONS.contains(&name),
                "{name} has an arity entry but is not a window function",
            );
            assert!(min_args <= max_args, "{name} has an inverted arity bound");
        }
        for &name in MYSQL_WINDOW_FUNCTIONS {
            assert!(
                MYSQL_WINDOW_FUNCTION_ARITY
                    .iter()
                    .any(|&(candidate, _, _)| candidate == name),
                "{name} is a window function with no arity entry",
            );
        }
    }
}
