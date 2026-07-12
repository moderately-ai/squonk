// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Shared core for the iterative-vs-recursive Pratt parsing spike
//! (`spike-iterative-pratt`).
//!
//! A MINIMAL but faithful model of the production expression core
//! (`crates/squonk/src/parser/expr.rs`): primaries, parenthesized groups,
//! prefix-unary chains, and binary operators driven by a binding-power table
//! (ADR-0008). It exists in two forms that must produce byte-identical trees:
//!
//!   * [`rec_parse`] — recursive descent mirroring `parse_expr_bp` / `parse_prefix`
//!     one-for-one (precedence-climbing `loop`, prefix recursion at the operator's
//!     own binding power, parens re-entering at `0`). This is the shape that
//!     stack-overflows on deep nesting today.
//!   * [`IterParser`] — an iterative explicit-heap-stack reification of the *same*
//!     call graph: every recursive call in `rec_parse` becomes a pushed [`Frame`]
//!     on a reused `Vec`, so nesting depth costs heap, never call stack. It cannot
//!     overflow regardless of input depth.
//!
//! Faithfulness that matters for the cost model:
//!   * Parens are grouping only and store NO node (ADR-0008/ADR-0019), so both
//!     parsers drop them — a 100k-deep paren nest parses to a single leaf, which
//!     isolates *stack* behaviour from *tree* size.
//!   * Left-associative chains fold inside the loop (O(1) recursion depth in the
//!     recursive form too); only NESTING (parens, prefix, right-assoc RHS) grows
//!     the stack — exactly the production vectors.
//!   * The non-associative chain rejection (`a = b = c`) is reproduced, because it
//!     is the one correctness rule the explicit-stack rewrite must re-express.
//!
//! Lexically simplified vs real SQL (symbols for keyword operators, no comments,
//! single token classes) so the lexer is not the thing under test — the parse
//! strategy is. The binding-power ratios mirror `precedence/mod.rs`.

// Included via `#[path]` into the driver example, the parity test (and nothing
// else); each includer uses a different subset, so the module-level allow keeps
// `-D warnings` green exactly as the precedence-encoding ref does.
#![allow(dead_code, unused_imports)]

use std::fmt;

// ---------------------------------------------------------------------------
// Tokens + lexer
// ---------------------------------------------------------------------------

/// A token in the mini expression grammar. `Copy`, like the real `Token`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tok {
    /// Integer literal.
    Num(i64),
    /// Identifier, interned to a small id (distinct spellings get distinct ids).
    Ident(u32),
    Plus,
    Minus,
    Star,
    Slash,
    /// `^`: a RIGHT-associative binary, included to exercise the iterative fold's
    /// right-recursion path. Not an M1 operator (the real table has no right-assoc
    /// binary) — present purely as an associativity stress.
    Caret,
    Eq,
    Lt,
    Gt,
    /// `&` standing in for the `AND` keyword (lexer simplification).
    Amp,
    /// `|` standing in for the `OR` keyword.
    Pipe,
    /// `!` standing in for the prefix `NOT` keyword.
    Bang,
    LParen,
    RParen,
}

/// Tokenize a mini-grammar source string. Whitespace separates tokens and is
/// otherwise ignored; identifiers intern so equal spellings compare equal.
pub fn tokenize(src: &str) -> Result<Vec<Tok>, String> {
    let bytes = src.as_bytes();
    let mut idents: Vec<&str> = Vec::new();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        match b {
            b' ' | b'\t' | b'\n' | b'\r' => i += 1,
            b'+' => push(&mut out, Tok::Plus, &mut i),
            b'-' => push(&mut out, Tok::Minus, &mut i),
            b'*' => push(&mut out, Tok::Star, &mut i),
            b'/' => push(&mut out, Tok::Slash, &mut i),
            b'^' => push(&mut out, Tok::Caret, &mut i),
            b'=' => push(&mut out, Tok::Eq, &mut i),
            b'<' => push(&mut out, Tok::Lt, &mut i),
            b'>' => push(&mut out, Tok::Gt, &mut i),
            b'&' => push(&mut out, Tok::Amp, &mut i),
            b'|' => push(&mut out, Tok::Pipe, &mut i),
            b'!' => push(&mut out, Tok::Bang, &mut i),
            b'(' => push(&mut out, Tok::LParen, &mut i),
            b')' => push(&mut out, Tok::RParen, &mut i),
            b'0'..=b'9' => {
                let start = i;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
                let n: i64 = src[start..i]
                    .parse()
                    .map_err(|_| format!("integer literal out of range at byte {start}"))?;
                out.push(Tok::Num(n));
            }
            b if b.is_ascii_alphabetic() || b == b'_' => {
                let start = i;
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                let word = &src[start..i];
                let id = match idents.iter().position(|w| *w == word) {
                    Some(pos) => pos as u32,
                    None => {
                        idents.push(word);
                        (idents.len() - 1) as u32
                    }
                };
                out.push(Tok::Ident(id));
            }
            other => return Err(format!("unexpected byte {:?} at {i}", other as char)),
        }
    }
    Ok(out)
}

fn push(out: &mut Vec<Tok>, tok: Tok, i: &mut usize) {
    out.push(tok);
    *i += 1;
}

// ---------------------------------------------------------------------------
// AST — parens deliberately store no node (ADR-0008)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Or,
    And,
    Eq,
    Lt,
    Gt,
    Add,
    Sub,
    Mul,
    Div,
    Pow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Pos,
    Not,
}

/// The expression tree. Boxed children mirror `Expr::BinaryOp { left: Box<Expr>,
/// .. }`; both parsers build the identical shape so the trees are `==`.
///
/// NOTE: the derived `PartialEq` and `Clone` recurse, so they are used only on the
/// SHALLOW battery — never on the 100k-deep adversarial trees (which would
/// overflow). Teardown is made iterative below so building a deep tree (the whole
/// point of the iterative parser) is not undone by a deep recursive `Drop`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Expr {
    Num(i64),
    Var(u32),
    Unary(UnOp, Box<Expr>),
    Binary(Box<Expr>, BinOp, Box<Expr>),
}

/// Move `e`'s children onto `work`, leaving leaves behind — the one step shared by
/// the iterative `Drop` and any other iterative teardown.
fn take_children(e: &mut Expr, work: &mut Vec<Expr>) {
    match e {
        Expr::Unary(_, inner) => work.push(std::mem::replace(&mut **inner, Expr::Num(0))),
        Expr::Binary(l, _, r) => {
            work.push(std::mem::replace(&mut **l, Expr::Num(0)));
            work.push(std::mem::replace(&mut **r, Expr::Num(0)));
        }
        Expr::Num(_) | Expr::Var(_) => {}
    }
}

/// Iterative destructor: dismantle the tree onto a heap worklist so a 100k-deep
/// chain tears down in bounded call-stack depth. Without this, dropping a deep
/// tree the iterative parser just built would overflow exactly where the parser
/// did not — defeating the spike. Each popped node has its children replaced with
/// leaves before it drops, so its own re-entrant `Drop` sees only leaves.
impl Drop for Expr {
    fn drop(&mut self) {
        // A bare leaf (the overwhelmingly common case, incl. the leaves this very
        // routine leaves behind) needs no worklist — keep teardown allocation-free
        // for shallow trees.
        if matches!(self, Expr::Num(_) | Expr::Var(_)) {
            return;
        }
        let mut work: Vec<Expr> = Vec::new();
        take_children(self, &mut work);
        while let Some(mut node) = work.pop() {
            take_children(&mut node, &mut work);
            // `node` now holds leaf children; dropping it here is shallow.
        }
    }
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expr::Num(n) => write!(f, "{n}"),
            Expr::Var(id) => write!(f, "v{id}"),
            Expr::Unary(op, e) => write!(f, "({op:?} {e})"),
            Expr::Binary(l, op, r) => write!(f, "({l} {op:?} {r})"),
        }
    }
}

// ---------------------------------------------------------------------------
// Binding powers — mirror crates/squonk-ast/src/precedence/mod.rs ratios
// ---------------------------------------------------------------------------

// Mirrors the production `precedence::Assoc { Left, Right, NonAssoc }`. The
// `NonAssoc` variant trips `enum_variant_names` only because this module is
// `#[path]`-included into an example/test (non-library, so `pub` is not
// API-protected); the real enum is clean as exported API.
#[allow(clippy::enum_variant_names)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Assoc {
    Left,
    Right,
    NonAssoc,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bp {
    pub left: u8,
    pub right: u8,
    pub assoc: Assoc,
}

/// Binary binding power, same numbering as `BindingPowerTable::STANDARD`.
pub const fn binary_bp(op: BinOp) -> Bp {
    match op {
        BinOp::Or => Bp {
            left: 10,
            right: 11,
            assoc: Assoc::Left,
        },
        BinOp::And => Bp {
            left: 20,
            right: 21,
            assoc: Assoc::Left,
        },
        BinOp::Eq | BinOp::Lt | BinOp::Gt => Bp {
            left: 40,
            right: 41,
            assoc: Assoc::NonAssoc,
        },
        BinOp::Add | BinOp::Sub => Bp {
            left: 50,
            right: 51,
            assoc: Assoc::Left,
        },
        BinOp::Mul | BinOp::Div => Bp {
            left: 60,
            right: 61,
            assoc: Assoc::Left,
        },
        // Right-assoc: right < left, so the inner operator re-enters (matklad).
        BinOp::Pow => Bp {
            left: 71,
            right: 70,
            assoc: Assoc::Right,
        },
    }
}

/// Prefix binding power, mirroring `prefix_not = 30`, `prefix_sign = 80`.
pub const fn prefix_bp(op: UnOp) -> u8 {
    match op {
        UnOp::Not => 30,
        UnOp::Neg | UnOp::Pos => 80,
    }
}

/// Map a token in infix position to its [`BinOp`], or `None` if it ends the
/// expression (mirrors `peek_infix_operator`).
fn infix_op(tok: Tok) -> Option<BinOp> {
    Some(match tok {
        Tok::Pipe => BinOp::Or,
        Tok::Amp => BinOp::And,
        Tok::Eq => BinOp::Eq,
        Tok::Lt => BinOp::Lt,
        Tok::Gt => BinOp::Gt,
        Tok::Plus => BinOp::Add,
        Tok::Minus => BinOp::Sub,
        Tok::Star => BinOp::Mul,
        Tok::Slash => BinOp::Div,
        Tok::Caret => BinOp::Pow,
        _ => return None,
    })
}

/// Map a token in prefix position to its [`UnOp`] (mirrors `parse_unary`'s arms).
fn prefix_op(tok: Tok) -> Option<UnOp> {
    Some(match tok {
        Tok::Minus => UnOp::Neg,
        Tok::Plus => UnOp::Pos,
        Tok::Bang => UnOp::Not,
        _ => return None,
    })
}

/// Whether `lhs` is itself a non-associative operator at precedence `level` — the
/// `a < b < c` rejection condition (mirrors `lhs_chains_nonassoc`). The real
/// version also maps the comparison-level predicates here; the mini-grammar has
/// none, so only the binary case applies.
fn chains_nonassoc(lhs: &Expr, level: u8) -> bool {
    match lhs {
        Expr::Binary(_, op, _) => {
            let bp = binary_bp(*op);
            bp.assoc == Assoc::NonAssoc && bp.left == level
        }
        _ => false,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseError {
    /// A token (or EOI) appeared where a primary/prefix was expected.
    ExpectedExpr,
    /// A `(` group was not closed by `)`.
    ExpectedRParen,
    /// `a < b < c`: a non-associative operator chained illegally.
    NonAssocChain,
    /// Tokens remained after a complete expression.
    TrailingTokens,
}

// ---------------------------------------------------------------------------
// Recursive parser — one-for-one with parse_expr_bp / parse_prefix
// ---------------------------------------------------------------------------

struct Rec<'t> {
    toks: &'t [Tok],
    pos: usize,
}

impl Rec<'_> {
    fn cur(&self) -> Option<Tok> {
        self.toks.get(self.pos).copied()
    }

    /// `parse_expr_bp(min_bp)`: a prefix/primary, then fold each infix whose left
    /// bp clears `min_bp`, recursing the RHS at the operator's own right bp.
    fn expr_bp(&mut self, min_bp: u8) -> Result<Expr, ParseError> {
        let (mut lhs, mut grouped) = self.prefix()?;
        while let Some(tok) = self.cur() {
            let Some(op) = infix_op(tok) else { break };
            let bp = binary_bp(op);
            if bp.left < min_bp {
                break;
            }
            if bp.assoc == Assoc::NonAssoc && !grouped && chains_nonassoc(&lhs, bp.left) {
                return Err(ParseError::NonAssocChain);
            }
            self.pos += 1; // consume the operator
            let rhs = self.expr_bp(bp.right)?;
            lhs = Expr::Binary(Box::new(lhs), op, Box::new(rhs));
            grouped = false;
        }
        Ok(lhs)
    }

    /// `parse_prefix`: a prefix-unary chain or a primary. Returns the `grouped`
    /// flag a parenthesized operand carries (suppresses the next non-assoc check).
    fn prefix(&mut self) -> Result<(Expr, bool), ParseError> {
        match self.cur() {
            Some(Tok::Num(n)) => {
                self.pos += 1;
                Ok((Expr::Num(n), false))
            }
            Some(Tok::Ident(id)) => {
                self.pos += 1;
                Ok((Expr::Var(id), false))
            }
            Some(tok) if prefix_op(tok).is_some() => {
                let op = prefix_op(tok).expect("guarded by prefix_op(tok).is_some()");
                self.pos += 1; // consume the prefix operator
                // parse_unary recurses at the operator's prefix binding power.
                let operand = self.expr_bp(prefix_bp(op))?;
                Ok((Expr::Unary(op, Box::new(operand)), false))
            }
            Some(Tok::LParen) => {
                self.pos += 1; // consume `(`
                // Parens reset binding fully: re-enter the climb at 0.
                let inner = self.expr_bp(0)?;
                if self.cur() != Some(Tok::RParen) {
                    return Err(ParseError::ExpectedRParen);
                }
                self.pos += 1; // consume `)`
                Ok((inner, true))
            }
            _ => Err(ParseError::ExpectedExpr),
        }
    }
}

/// Parse with the recursive descent parser (the shape that stack-overflows deep).
pub fn rec_parse(toks: &[Tok]) -> Result<Expr, ParseError> {
    let mut p = Rec { toks, pos: 0 };
    let expr = p.expr_bp(0)?;
    if p.pos != toks.len() {
        return Err(ParseError::TrailingTokens);
    }
    Ok(expr)
}

// ---------------------------------------------------------------------------
// Iterative parser — the recursive call graph reified onto an explicit Vec
// ---------------------------------------------------------------------------

/// A pending context on the explicit heap stack. Each variant is exactly one
/// suspended point of the recursive parser:
///
///   * [`Frame::BeginLoop`] — a `parse_expr_bp` that just obtained its `lhs` from
///     `parse_prefix` and is about to run the climbing loop at `min_bp`.
///   * [`Frame::BinaryRhs`] — a climbing loop suspended after consuming an
///     operator, waiting for the RHS sub-parse to fold `lhs op rhs`.
///   * [`Frame::PrefixWrap`] — a `parse_unary` waiting to wrap its operand.
///   * [`Frame::ParenClose`] — a `parse_grouped` waiting to consume `)`.
enum Frame {
    BeginLoop { min_bp: u8 },
    BinaryRhs { lhs: Expr, op: BinOp, min_bp: u8 },
    PrefixWrap { op: UnOp },
    ParenClose,
}

/// The driver's control state. Mirrors the three reachable points of the
/// recursive code: entering a `parse_expr_bp`, sitting in its loop, and a
/// sub-parse having produced a value that must bubble to the enclosing frame.
enum State {
    /// Begin `parse_expr_bp(min_bp)`: push its loop frame, then parse a prefix.
    StartExpr(u8),
    /// Inside a climbing loop with the current `lhs`/`grouped` at `min_bp`.
    RunLoop {
        lhs: Expr,
        grouped: bool,
        min_bp: u8,
    },
    /// A prefix/primary (or a finished sub-parse) produced `val`; pop a frame.
    Produce { val: Expr, grouped: bool },
}

/// Iterative explicit-stack expression parser. Owns a reusable `Vec` so a
/// steady-state parse does no per-parse stack allocation (the production hot-path
/// concern); deep nesting just grows this `Vec` instead of the call stack.
pub struct IterParser {
    stack: Vec<Frame>,
}

impl Default for IterParser {
    fn default() -> Self {
        Self::new()
    }
}

impl IterParser {
    pub fn new() -> Self {
        Self { stack: Vec::new() }
    }

    /// Pre-size the reused stack so the common shallow case never reallocates.
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            stack: Vec::with_capacity(cap),
        }
    }

    /// The reused stack's current capacity (evidence it is not re-grown per parse).
    pub fn stack_capacity(&self) -> usize {
        self.stack.capacity()
    }

    /// Parse `toks` iteratively. Identical result to [`rec_parse`] for every
    /// input, but bounded by heap, not call stack.
    pub fn parse(&mut self, toks: &[Tok]) -> Result<Expr, ParseError> {
        self.stack.clear();
        let mut pos = 0usize;
        let mut state = State::StartExpr(0);

        loop {
            match state {
                State::StartExpr(min_bp) => {
                    // parse_expr_bp = push the loop frame, then parse a prefix.
                    self.stack.push(Frame::BeginLoop { min_bp });
                    match toks.get(pos).copied() {
                        Some(Tok::Num(n)) => {
                            pos += 1;
                            state = State::Produce {
                                val: Expr::Num(n),
                                grouped: false,
                            };
                        }
                        Some(Tok::Ident(id)) => {
                            pos += 1;
                            state = State::Produce {
                                val: Expr::Var(id),
                                grouped: false,
                            };
                        }
                        Some(tok) if prefix_op(tok).is_some() => {
                            let op = prefix_op(tok).expect("guarded by is_some()");
                            pos += 1;
                            // parse_unary: wrap the operand parsed at prefix bp.
                            self.stack.push(Frame::PrefixWrap { op });
                            state = State::StartExpr(prefix_bp(op));
                        }
                        Some(Tok::LParen) => {
                            pos += 1;
                            // parse_grouped: re-enter at 0, then expect `)`.
                            self.stack.push(Frame::ParenClose);
                            state = State::StartExpr(0);
                        }
                        _ => return Err(ParseError::ExpectedExpr),
                    }
                }

                State::RunLoop {
                    lhs,
                    grouped,
                    min_bp,
                } => {
                    let op = toks.get(pos).copied().and_then(infix_op);
                    match op {
                        None => state = State::Produce { val: lhs, grouped },
                        Some(op) => {
                            let bp = binary_bp(op);
                            if bp.left < min_bp {
                                // Operator belongs to an outer frame: end this loop.
                                state = State::Produce { val: lhs, grouped };
                            } else if bp.assoc == Assoc::NonAssoc
                                && !grouped
                                && chains_nonassoc(&lhs, bp.left)
                            {
                                return Err(ParseError::NonAssocChain);
                            } else {
                                pos += 1; // consume the operator
                                // RHS = parse_expr_bp(bp.right); fold on its return.
                                self.stack.push(Frame::BinaryRhs { lhs, op, min_bp });
                                state = State::StartExpr(bp.right);
                            }
                        }
                    }
                }

                State::Produce { val, grouped } => match self.stack.pop() {
                    None => {
                        // Top-level expression complete.
                        if pos != toks.len() {
                            return Err(ParseError::TrailingTokens);
                        }
                        return Ok(val);
                    }
                    Some(Frame::BeginLoop { min_bp }) => {
                        state = State::RunLoop {
                            lhs: val,
                            grouped,
                            min_bp,
                        };
                    }
                    Some(Frame::PrefixWrap { op }) => {
                        state = State::Produce {
                            val: Expr::Unary(op, Box::new(val)),
                            grouped: false,
                        };
                    }
                    Some(Frame::ParenClose) => {
                        if toks.get(pos).copied() != Some(Tok::RParen) {
                            return Err(ParseError::ExpectedRParen);
                        }
                        pos += 1; // consume `)`
                        state = State::Produce { val, grouped: true };
                    }
                    Some(Frame::BinaryRhs { lhs, op, min_bp }) => {
                        let folded = Expr::Binary(Box::new(lhs), op, Box::new(val));
                        state = State::RunLoop {
                            lhs: folded,
                            grouped: false,
                            min_bp,
                        };
                    }
                },
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Battery + deep-case builders
// ---------------------------------------------------------------------------

/// Realistic SHALLOW expressions — a handful of operators and a few parens, the
/// shape that dominates real SQL throughput. The deciding perf comparison runs
/// here. (`&` = AND, `|` = OR, `!` = NOT, `^` = right-assoc pow.)
pub const SHALLOW_CASES: &[(&str, &str)] = &[
    ("simple_add", "a + b * c - d"),
    ("grouped_mul", "(a + b) * (c - d)"),
    ("bool_chain", "a & b | c & d"),
    ("cmp_and", "a = 1 & b < 2 | c > 3 & d = 4"),
    ("prefix_sign", "- a * b + c"),
    ("prefix_not", "! a & ! b"),
    ("redundant_parens", "((a + b))"),
    ("nested_paren_div", "a + (b * (c - d)) / e"),
    ("right_assoc", "a ^ b ^ c + d"),
    ("literals", "1 + 2 * 3 - 4 / 2"),
    ("mixed_group", "(a) & (b | c)"),
    // The expression-heavy hot shape, modelled on perf_testbed's `nested_expr`:
    // balanced parens + arithmetic + a boolean tail. This is the representative
    // "common case" the throughput decision turns on.
    (
        "nested_expr",
        "((a + b) * (c - d)) / ((e + f) * (g - h)) + (((i * j) - (k / l)) + ((m + n) * (o - p)))",
    ),
    (
        "where_predicate",
        "(a > b & c < d) | (e = f & g < h) | (i > j & k < l)",
    ),
];

/// `N` nested parens around a single leaf: `(((…x…)))`. Parses to one `Var`
/// regardless of `N` (parens store no node), so this isolates STACK depth.
pub fn nested_parens(depth: usize) -> String {
    let mut s = String::with_capacity(depth * 2 + 1);
    for _ in 0..depth {
        s.push('(');
    }
    s.push('x');
    for _ in 0..depth {
        s.push(')');
    }
    s
}

/// A prefix-unary chain `N` deep: `- - - … x`. Parses to `N` nested `Unary`
/// nodes — depth in both stack AND tree.
pub fn prefix_chain(depth: usize) -> String {
    let mut s = String::with_capacity(depth * 2 + 1);
    for _ in 0..depth {
        s.push_str("- ");
    }
    s.push('x');
    s
}

/// Count all nodes in a tree, ITERATIVELY (a recursive count would itself
/// overflow on the deep adversarial trees this spike builds).
pub fn node_count(root: &Expr) -> usize {
    let mut count = 0usize;
    let mut work: Vec<&Expr> = vec![root];
    while let Some(e) = work.pop() {
        count += 1;
        match e {
            Expr::Num(_) | Expr::Var(_) => {}
            Expr::Unary(_, inner) => work.push(inner),
            Expr::Binary(l, _, r) => {
                work.push(l);
                work.push(r);
            }
        }
    }
    count
}
