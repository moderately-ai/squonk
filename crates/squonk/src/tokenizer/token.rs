// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Lexical token types: the M1 token categories and their operator/punctuation
//! sub-kinds.

use crate::ast::Span;
use crate::ast::dialect::Keyword;

/// A single lexical token: a category plus its byte range in the source.
///
/// `Token` is `Copy` and deliberately carries no borrow. The token *text* is
/// recovered later as `&source[span]` (zero-copy tokens): keeping the
/// token free of a `&str` makes the token stream cache-dense and decouples it
/// from the source buffer's lifetime, so a `Vec<Token>` can outlive the borrow
/// the cursor held while scanning.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Token {
    /// The lexical category of this token.
    pub kind: TokenKind,
    /// The half-open byte range `[start, end)` this token occupies in the source.
    pub span: Span,
}

impl Token {
    /// Pair a category with the source range it covers.
    pub const fn new(kind: TokenKind, span: Span) -> Self {
        Self { kind, span }
    }
}

/// The M1 lexical categories.
///
/// Literal *unescaping* is deferred to lazy materialization: a
/// [`String`] or [`QuotedIdent`] span still includes its delimiters and raw
/// escapes.
///
/// [`String`]: TokenKind::String
/// [`QuotedIdent`]: TokenKind::QuotedIdent
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TokenKind {
    /// An unquoted identifier or unrecognized word: `users`, `_c1`, `café`.
    Word,
    /// A recognized SQL keyword, with its exact source spelling recoverable
    /// through the token span.
    Keyword(Keyword),
    /// A numeric literal, integer or float: `42`, `3.14`, `.5`, `1e10`.
    Number,
    /// A string literal: standard single-quoted (`'…'`), or a dialect-enabled
    /// PostgreSQL escape (`E'…'`) / dollar-quoted (`$tag$…$tag$`) string. The
    /// span covers the delimiters and any prefix.
    String,
    /// A double-quoted (`"…"`) delimited identifier. The span covers the quotes.
    QuotedIdent,
    /// A prepared-statement parameter placeholder: PostgreSQL positional `$1` (the
    /// span covers `$` and its digits) or an anonymous `?`. Which forms lex is
    /// dialect data ([`FeatureSet::parameters`](crate::ast::dialect::FeatureSet));
    /// the positional/anonymous split is recovered from the span at parse time, like
    /// the integer/float split of a [`Number`](TokenKind::Number).
    Parameter,
    /// A DuckDB `#n` positional column reference: `#` followed by one or more ASCII
    /// digits (the span covers the sigil and the digits). Lexed only under a dialect that
    /// sets [`ExpressionSyntax::positional_column`](crate::ast::dialect::ExpressionSyntax)
    /// (DuckDB), so elsewhere `#` stays a stray byte, a MySQL line comment, or
    /// PostgreSQL's XOR [`Operator::Hash`] per that dialect's data. The
    /// 1-based index is recovered from the span at parse time, like the integer of a
    /// [`Number`](TokenKind::Number).
    PositionalColumn,
    /// A MySQL session variable read as a value expression: a user variable `@name`
    /// or a system variable `@@name` / `@@global.name` / `@@session.name` (the span
    /// covers the sigil, any scope prefix, and the name). Distinct from a
    /// [`Parameter`](TokenKind::Parameter) placeholder — the sigil-and-scope split is
    /// recovered from the span at parse time. Which forms lex is dialect data
    /// ([`FeatureSet::session_variables`](crate::ast::dialect::FeatureSet)).
    Variable,
    /// A Snowflake stage reference: `@stage`, `@~`, `@%table`, optionally with
    /// `/path` segments. Gated by [`UtilitySyntax::stage_references`](crate::ast::dialect::UtilitySyntax).
    StageReference,
    /// An operator spelling, e.g. `+`, `<>`, `||`.
    Operator(Operator),
    /// Structural punctuation, e.g. `(`, `,`, `;`.
    Punctuation(Punctuation),
    /// A byte that begins no known token.
    ///
    /// Reserved for the resilience-ready tokenizer path the ADR foreshadows: the
    /// M1 eager [`tokenize`] is fail-fast and reports such a byte as a
    /// [`LexError`] instead of emitting this kind. It is part of the closed enum
    /// so a later error-recovering driver can surface a placeholder token
    /// without widening the public type.
    ///
    /// [`tokenize`]: crate::tokenizer::tokenize
    /// [`LexError`]: crate::tokenizer::LexError
    Unknown,
}

/// Operator spellings recognized in M1.
///
/// The set is *closed over the operator bytes* the shared lexer-class table
/// marks (`+ - * / % = < > ! | & ^ ~`): every such byte maps to a variant here,
/// either alone or as the lead of a fixed two-byte operator (`<=`, `>=`, `<>`,
/// `!=`, `||`, and the always-munched shifts `<<`/`>>`). That keeps the enum small
/// and total without rejecting a byte the dialect data calls an operator. Operator
/// *precedence and semantics* live in the AST precedence data and the parser, not here.
///
/// A handful of spellings are recognised only when their dialect feature is on, so
/// they are scanned with the feature data rather than the byte-class table:
/// - the named-argument arrows [`Arrow`](Self::Arrow) (`=>`) and
///   [`ColonEquals`](Self::ColonEquals) (`:=`) under named arguments (the `:=` lead
///   byte is punctuation-class, so it is munched in the punctuation scanner);
/// - the PostgreSQL containment and JSON operators [`AtGt`](Self::AtGt) (`@>`),
///   [`LtAt`](Self::LtAt) (`<@`), [`MinusGt`](Self::MinusGt) (`->`), and
///   [`MinusGtGt`](Self::MinusGtGt) (`->>`), gated by the containment / JSON-arrow
///   expression-syntax flags, and PostgreSQL's XOR [`Hash`](Self::Hash) (`#`), gated by
///   [`FeatureSet::hash_bitwise_xor`](crate::ast::dialect::FeatureSet). The `@` and `#` lead bytes are
///   not in the lexer-class table, so `@>` / `#` reach the operator scanner through a
///   feature-gated dispatch arm. The PostgreSQL prefix `@` absolute-value operator is a
///   scoped follow-up (its `@` lexeme contends with the T-SQL/MySQL `@name` sigils,
///   needing a tracked conflict).
/// - the PostgreSQL `jsonb` operators [`Question`](Self::Question) (`?`),
///   [`QuestionPipe`](Self::QuestionPipe) (`?|`), [`QuestionAmp`](Self::QuestionAmp) (`?&`),
///   [`AtQuestion`](Self::AtQuestion) (`@?`), [`AtAt`](Self::AtAt) (`@@`),
///   [`HashGt`](Self::HashGt) (`#>`), [`HashGtGt`](Self::HashGtGt) (`#>>`), and
///   [`HashMinus`](Self::HashMinus) (`#-`), gated by
///   [`OperatorSyntax::jsonb_operators`](crate::ast::dialect::OperatorSyntax::jsonb_operators).
///   The `?` and `@?`/`@@` forms reach the scanner through feature-gated dispatch arms (like
///   `@>`); the `#`-led forms are munched inside the scanner ahead of the bare `#` XOR.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Operator {
    /// `+`
    Plus,
    /// `-`
    Minus,
    /// `*`
    Star,
    /// `/`
    Slash,
    /// `//` — DuckDB's doubled-`/` integer-division spelling. Lexed only under a dialect
    /// that enables it ([`OperatorSyntax::integer_divide_slash`](crate::ast::dialect::OperatorSyntax::integer_divide_slash));
    /// elsewhere the bytes stay a lone `/` followed by another `/`.
    SlashSlash,
    /// `%`
    Percent,
    /// `=`
    Eq,
    /// `==` — SQLite's doubled-`=` equality spelling. Lexed only under a dialect that
    /// enables it ([`OperatorSyntax::double_equals`](crate::ast::dialect::ExpressionSyntax));
    /// elsewhere the bytes stay a lone `=` followed by another `=`.
    EqEq,
    /// `<`
    Lt,
    /// `<=`
    LtEq,
    /// `>`
    Gt,
    /// `>=`
    GtEq,
    /// `<>` or `!=` (both spell "not equal"; the span recovers which was written).
    NotEq,
    /// `<=>` — MySQL's null-safe equality operator (`a <=> b` ≡ `a IS NOT DISTINCT FROM b`).
    /// Lexed only under a dialect that enables it
    /// ([`OperatorSyntax::null_safe_equals`](crate::ast::dialect::ExpressionSyntax));
    /// elsewhere the bytes stay `<=` followed by `>`. Munched ahead of `<=` (maximal munch
    /// over three bytes).
    LtEqGt,
    /// `||` (string concatenation in standard SQL / PostgreSQL).
    Concat,
    /// `&&` (logical AND in MySQL; array-overlap in PostgreSQL). The lexeme is
    /// always recognised; whether it is an operator is dialect *meaning* data
    /// ([`FeatureSet::double_ampersand`](crate::ast::dialect::FeatureSet)).
    AmpAmp,
    /// A lone `!` (not part of `!=`).
    Bang,
    /// A lone `|` (not part of `||`).
    Pipe,
    /// `&`
    Amp,
    /// `^`
    Caret,
    /// `^@` — DuckDB's "starts with" string operator. Lexed only under
    /// [`OperatorSyntax::starts_with_operator`](crate::ast::dialect::OperatorSyntax).
    CaretAt,
    /// `~`
    Tilde,
    /// `<<` — bitwise left shift. Maximal-munched over `<` `<` unconditionally (no dialect
    /// spells two adjacent `<` any other way), like [`Concat`](Self::Concat); whether it is
    /// an infix operator is dialect *meaning* data
    /// ([`OperatorSyntax::bitwise_operators`](crate::ast::dialect::ExpressionSyntax)).
    ShiftLeft,
    /// `>>` — bitwise right shift. Maximal-munched over `>` `>` unconditionally, like
    /// [`ShiftLeft`](Self::ShiftLeft).
    ShiftRight,
    /// `#` — PostgreSQL's bitwise-XOR operator. Lexed only under a dialect that sets
    /// [`FeatureSet::hash_bitwise_xor`](crate::ast::dialect::FeatureSet); elsewhere `#` is a stray byte
    /// (or a MySQL line comment). The `#` lead byte is not in the lexer-class table, so it
    /// reaches the operator scanner through a feature-gated dispatch arm, like `@>`.
    Hash,
    /// `=>` — the PostgreSQL named-argument arrow (`f(name => value)`). Lexed only
    /// under a dialect that enables named arguments
    /// ([`CallSyntax::named_argument`](crate::ast::dialect::ExpressionSyntax));
    /// elsewhere the bytes stay a lone `=` followed by `>`.
    Arrow,
    /// `:=` — the deprecated PostgreSQL named-argument separator (`f(name := value)`).
    /// Lexed only under a dialect that enables named arguments; elsewhere the bytes
    /// stay a lone `:` followed by `=`.
    ColonEquals,
    /// `@>` — the PostgreSQL "contains" containment operator. Lexed only under a dialect
    /// that enables the containment operators; elsewhere the bytes stay a lone `@` (a
    /// stray byte outside PostgreSQL) followed by `>`.
    AtGt,
    /// `<@` — the PostgreSQL "contained by" containment operator. Gated with
    /// [`AtGt`](Self::AtGt); elsewhere the bytes stay a lone `<` followed by `@`.
    LtAt,
    /// `->` — the PostgreSQL JSON field/element access operator. Lexed only under a
    /// dialect that enables the JSON-arrow operators; elsewhere the bytes stay a lone
    /// `-` followed by `>`.
    MinusGt,
    /// `->>` — the PostgreSQL JSON access-as-text operator. Gated with
    /// [`MinusGt`](Self::MinusGt).
    MinusGtGt,
    /// `|>` — the BigQuery/ZetaSQL query pipe separator (`FROM t |> WHERE …`). Lexed only
    /// under a dialect that enables query pipe syntax
    /// ([`QueryTailSyntax::pipe_syntax`](crate::ast::dialect::SelectSyntax)); elsewhere the
    /// bytes stay a lone `|` followed by `>`, so no other dialect's `|`/`||` lexing shifts.
    /// Munched in the `|` arm beside `||`, over contiguous bytes only (`a | > b` spaced
    /// stays `|` then `>`). Not the `||` concat/OR operator — see
    /// [`Concat`](Self::Concat).
    PipeArrow,
    /// `?` — the PostgreSQL `jsonb` key/element-existence operator. Lexed only under a
    /// dialect that enables the `jsonb` operators
    /// ([`OperatorSyntax::jsonb_operators`](crate::ast::dialect::OperatorSyntax::jsonb_operators));
    /// elsewhere `?` is the anonymous placeholder or a stray byte. Reaches the operator
    /// scanner through a feature-gated dispatch arm (the `?` byte is not lexer-class).
    Question,
    /// `?|` — the PostgreSQL `jsonb` any-key-existence operator. Gated with
    /// [`Question`](Self::Question); the `?|` munch takes the `|` ahead of a lone `?`.
    QuestionPipe,
    /// `?&` — the PostgreSQL `jsonb` all-keys-existence operator. Gated with
    /// [`Question`](Self::Question).
    QuestionAmp,
    /// `@?` — the PostgreSQL `jsonb` `@?` path-existence operator. Gated with the `jsonb`
    /// operators; reaches the operator scanner through a feature-gated `@` dispatch arm, like
    /// `@>`. Its second byte `?` keeps it disjoint from every other `@` claimant.
    AtQuestion,
    /// `@@` — the PostgreSQL `jsonb`/text-search match operator. Gated with the `jsonb`
    /// operators; reaches the operator scanner through the same `@` dispatch arm. Shares the
    /// `@@` spelling with MySQL's system-variable sigil, resolved by feature precedence.
    AtAt,
    /// `#>` — the PostgreSQL `jsonb` extract-at-path operator. Munched inside the operator
    /// scanner ahead of the bare `#` bitwise-XOR, under the `jsonb` operators.
    HashGt,
    /// `#>>` — the PostgreSQL `jsonb` extract-at-path-as-text operator. Gated with
    /// [`HashGt`](Self::HashGt); the `#>>` munch takes the second `>` ahead of `#>`.
    HashGtGt,
    /// `#-` — the PostgreSQL `jsonb` delete-at-path operator. Munched over the two contiguous
    /// bytes ahead of the bare `#` (engine-verified: `5#-3` is `5 #- 3`; a space splits it).
    HashMinus,
    /// A general symbolic operator that matches no built-in spelling — a maximal-munch run of
    /// `Op`-class bytes (`~ ! @ # ^ & | ? + - * / % < > =`) under the general operator
    /// surface
    /// ([`OperatorSyntax::custom_operators`](crate::ast::dialect::OperatorSyntax::custom_operators),
    /// which any dialect can enable; PostgreSQL is the current enabler): the regex `!~`/`~*`/
    /// `!~*`, the geometric/network/text-search ops (`&<`, `&>`, `<->`, `<<|`, `|>>`, `^@`,
    /// `##`, `<^`, `<%`, `@-@`, …), the negator spellings (`*<>`, `*>=`), the prefix operators
    /// (`@`, `|/`, `||/`, `!!`, `@#@`), and any user-defined operator. Payloadless — the exact
    /// spelling is the token's span, interned at parse time onto an
    /// [`Expr::NamedOperator`](crate::ast::Expr::NamedOperator) /
    /// [`Expr::PrefixOperator`](crate::ast::Expr::PrefixOperator) — so a new custom spelling
    /// costs no new variant and [`Operator`] stays `Copy`. Only produced under the
    /// maximal-munch operator scanner (the `custom_operators` gate); every built-in operator
    /// (single or multi-char) still lexes to its own dedicated variant, so this is exactly the
    /// residue.
    Custom,
}

/// Structural punctuation recognized in M1.
///
/// Closed over the punctuation bytes the shared lexer-class table marks
/// (`( ) , ; . [ ] { } :`), so the tokenizer never rejects a byte the dialect
/// data calls punctuation. The `:` byte is the one with two forms: `::` (the
/// PostgreSQL typecast operator) takes maximal-munch priority over a lone `:`
/// (the array-slice separator).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Punctuation {
    /// `(`
    LParen,
    /// `)`
    RParen,
    /// `,`
    Comma,
    /// `;`
    Semicolon,
    /// `.` (only when it does not begin a `.5`-style numeric literal).
    Dot,
    /// `[`
    LBracket,
    /// `]`
    RBracket,
    /// `{`
    LBrace,
    /// `}`
    RBrace,
    /// A lone `:` (the array-slice separator in `a[lo:hi]`); a `::` munches to
    /// [`DoubleColon`](Self::DoubleColon) first.
    Colon,
    /// `::` (the PostgreSQL typecast operator `expr::type`).
    DoubleColon,
    /// A standalone `@` separating `user@host` in a MySQL account name when the host is a
    /// quoted/backtick `ident_or_text` (`u@'h'`, `u@"h"`, `` u@`h` ``). The context-free
    /// lexer folds an *unquoted* `@host` into one [`Variable`](TokenKind::Variable) lexeme;
    /// a quoted host cannot fold there (a quote is not an identifier byte), so the `@` is
    /// emitted as its own token and the account-name parser reads the following
    /// string/quoted-ident as the host. Emitted only under
    /// [`SessionVariableSyntax::user_variables`](crate::ast::dialect::SessionVariableSyntax);
    /// elsewhere a bare `@` before a quote stays a stray byte.
    At,
}
