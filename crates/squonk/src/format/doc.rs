// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! A Wadler/Prettier-style layout IR (`Doc`) and a width-aware layouter.
//!
//! The pretty renderer never decides where lines break; it builds a `Doc` tree of
//! *intentions* — text, breakable lines, groups, and indentation — and this module's
//! [`layout`] resolves them against a target line width. A [`Group`](Doc::Group) is
//! printed flat (its [`Line`](Doc::Line)s become spaces) when it fits the remaining
//! columns, and broken (its lines become newline + indent) when it does not. This is
//! the same two-mode, fits-directed algorithm Wadler's *A prettier printer* and
//! Prettier's `printDocToString` use, implemented iteratively so a deeply nested
//! document cannot overflow the stack the way the naive recursion would.
//!
//! Indentation is carried in *absolute columns*: [`Nest`](Doc::Nest) adds to the
//! current indent, and the renderer passes the configured indent width as the
//! increment, so this module is width-only and unit-agnostic.

use std::borrow::Cow;

/// A layout intention. The renderer composes these; [`layout`] resolves them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Doc {
    /// The empty document. An identity for [`concat()`](concat()) / [`join`] so a missing clause
    /// contributes nothing.
    Nil,
    /// Literal text with no internal newlines. Keywords are `'static` borrows;
    /// rendered fragments are owned.
    Text(Cow<'static, str>),
    /// A space when the enclosing group is flat, a newline + indent when it is broken.
    Line,
    /// Nothing when flat, a newline + indent when broken (Prettier's `softline`).
    SoftLine,
    /// Always a newline + indent, and it forces every enclosing group to break
    /// (Prettier's `hardline`). A comment or a statement boundary emits one.
    HardLine,
    /// A sequence, laid out left to right in the enclosing mode.
    Concat(Vec<Doc>),
    /// Increase the indent of the contained document by the given number of columns.
    Nest(usize, Box<Doc>),
    /// A break group: laid out flat if it fits the remaining width, otherwise broken.
    Group(Box<Doc>),
}

impl Doc {
    /// Literal text (no internal newlines — use [`hardline`] for those).
    pub fn text(s: impl Into<Cow<'static, str>>) -> Doc {
        Doc::Text(s.into())
    }
}

/// The empty document.
pub fn nil() -> Doc {
    Doc::Nil
}

/// A soft line: space when flat, newline when broken.
pub fn line() -> Doc {
    Doc::Line
}

/// A soft line that vanishes when flat.
pub fn softline() -> Doc {
    Doc::SoftLine
}

/// A mandatory line break that also forces every enclosing group to break.
pub fn hardline() -> Doc {
    Doc::HardLine
}

/// Sequence `docs` left to right, dropping [`Doc::Nil`] so absent clauses vanish.
pub fn concat(docs: impl IntoIterator<Item = Doc>) -> Doc {
    let parts: Vec<Doc> = docs
        .into_iter()
        .filter(|d| !matches!(d, Doc::Nil))
        .collect();
    match parts.len() {
        0 => Doc::Nil,
        1 => parts.into_iter().next().expect("len checked"),
        _ => Doc::Concat(parts),
    }
}

/// Indent the contained document by `width` extra columns whenever it breaks.
pub fn nest(width: usize, doc: Doc) -> Doc {
    if matches!(doc, Doc::Nil) {
        return Doc::Nil;
    }
    Doc::Nest(width, Box::new(doc))
}

/// A break group: flat if it fits, broken otherwise.
pub fn group(doc: Doc) -> Doc {
    if matches!(doc, Doc::Nil) {
        return Doc::Nil;
    }
    Doc::Group(Box::new(doc))
}

/// Interleave `sep` between the non-empty `items`.
pub fn join(sep: Doc, items: impl IntoIterator<Item = Doc>) -> Doc {
    let mut out = Vec::new();
    for item in items {
        if matches!(item, Doc::Nil) {
            continue;
        }
        if !out.is_empty() {
            out.push(sep.clone());
        }
        out.push(item);
    }
    concat(out)
}

/// Whether a group is being printed flat (lines are spaces) or broken (lines are
/// newlines).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Flat,
    Break,
}

/// The display width of a text run. SQL is overwhelmingly ASCII; `char` count is a
/// good, cheap approximation of column width for the fits decision (it is not a
/// full Unicode east-asian-width measure, which the layout does not need).
fn text_width(s: &str) -> usize {
    s.chars().count()
}

/// Whether the group content `doc` (followed by the still-pending `rest` commands)
/// fits flat within `remaining` columns.
///
/// Measures `doc` in flat mode, then continues into the outer command stack in its
/// real modes, so trailing same-line content is counted too. Returns as soon as the
/// budget is exceeded (does not fit) or a real line break is reached (the line ends
/// within budget, so it fits). A [`HardLine`](Doc::HardLine) inside the flat content
/// cannot be flattened, so it forces the group to break.
fn fits(
    mut remaining: isize,
    group_indent: usize,
    group_doc: &Doc,
    rest: &[(usize, Mode, &Doc)],
) -> bool {
    // A local worklist for the group content, plus an index walking the outer stack
    // from its top (its last element) once the group content is exhausted.
    let mut local: Vec<(usize, Mode, &Doc)> = vec![(group_indent, Mode::Flat, group_doc)];
    let mut rest_top = rest.len();
    loop {
        if remaining < 0 {
            return false;
        }
        let (indent, mode, doc) = match local.pop() {
            Some(cmd) => cmd,
            None => {
                if rest_top == 0 {
                    return true;
                }
                rest_top -= 1;
                rest[rest_top]
            }
        };
        match doc {
            Doc::Nil => {}
            Doc::Text(s) => remaining -= text_width(s) as isize,
            Doc::Concat(parts) => {
                for part in parts.iter().rev() {
                    local.push((indent, mode, part));
                }
            }
            Doc::Nest(width, inner) => local.push((indent + width, mode, inner)),
            Doc::Group(inner) => local.push((indent, Mode::Flat, inner)),
            Doc::Line => match mode {
                Mode::Flat => remaining -= 1,
                Mode::Break => return true,
            },
            Doc::SoftLine => match mode {
                Mode::Flat => {}
                Mode::Break => return true,
            },
            Doc::HardLine => match mode {
                Mode::Flat => return false,
                Mode::Break => return true,
            },
        }
    }
}

/// Lay `doc` out to a string, breaking groups that do not fit within `width`
/// columns.
pub fn layout(doc: &Doc, width: usize) -> String {
    let mut out = String::new();
    let mut pos: isize = 0;
    let mut stack: Vec<(usize, Mode, &Doc)> = vec![(0, Mode::Break, doc)];

    while let Some((indent, mode, doc)) = stack.pop() {
        match doc {
            Doc::Nil => {}
            Doc::Text(s) => {
                out.push_str(s);
                pos += text_width(s) as isize;
            }
            Doc::Concat(parts) => {
                for part in parts.iter().rev() {
                    stack.push((indent, mode, part));
                }
            }
            Doc::Nest(w, inner) => stack.push((indent + w, mode, inner)),
            Doc::Group(inner) => {
                // Once flat, stay flat (a flat parent cannot re-break a child);
                // otherwise the group is flat iff its content fits the remaining width.
                let flat = mode == Mode::Flat || fits(width as isize - pos, indent, inner, &stack);
                let inner_mode = if flat { Mode::Flat } else { Mode::Break };
                stack.push((indent, inner_mode, inner));
            }
            Doc::Line => match mode {
                Mode::Flat => {
                    out.push(' ');
                    pos += 1;
                }
                Mode::Break => {
                    new_line(&mut out, indent);
                    pos = indent as isize;
                }
            },
            Doc::SoftLine => match mode {
                Mode::Flat => {}
                Mode::Break => {
                    new_line(&mut out, indent);
                    pos = indent as isize;
                }
            },
            Doc::HardLine => {
                new_line(&mut out, indent);
                pos = indent as isize;
            }
        }
    }
    out
}

/// Emit a newline followed by `indent` spaces, trimming any trailing spaces the
/// previous line accumulated (a broken group whose content was empty, or a clause
/// that emitted a `Line` right before the break).
fn new_line(out: &mut String, indent: usize) {
    while out.ends_with(' ') {
        out.pop();
    }
    out.push('\n');
    for _ in 0..indent {
        out.push(' ');
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list_doc() -> Doc {
        // group( "SELECT" nest(2, line() ++ "a," line "b," line "c") )
        group(concat([
            Doc::text("SELECT"),
            nest(
                2,
                concat([
                    line(),
                    Doc::text("a,"),
                    line(),
                    Doc::text("b,"),
                    line(),
                    Doc::text("c"),
                ]),
            ),
        ]))
    }

    #[test]
    fn group_stays_flat_when_it_fits() {
        assert_eq!(layout(&list_doc(), 80), "SELECT a, b, c");
    }

    #[test]
    fn group_breaks_when_too_wide() {
        assert_eq!(layout(&list_doc(), 10), "SELECT\n  a,\n  b,\n  c");
    }

    #[test]
    fn hardline_forces_enclosing_group_to_break() {
        let doc = group(concat([Doc::text("a"), hardline(), Doc::text("b")]));
        assert_eq!(layout(&doc, 80), "a\nb");
    }

    #[test]
    fn nested_group_breaks_independently() {
        // Outer always broken (hardline); inner list fits flat.
        let inner = group(concat([
            Doc::text("("),
            nest(
                2,
                concat([softline(), Doc::text("x"), line(), Doc::text("y")]),
            ),
            softline(),
            Doc::text(")"),
        ]));
        let doc = concat([Doc::text("SELECT"), hardline(), inner]);
        assert_eq!(layout(&doc, 80), "SELECT\n(x y)");
    }

    #[test]
    fn softline_vanishes_when_flat_and_breaks_when_wide() {
        let doc = group(concat([
            Doc::text("("),
            nest(2, concat([softline(), Doc::text("longcontent")])),
            softline(),
            Doc::text(")"),
        ]));
        assert_eq!(layout(&doc, 80), "(longcontent)");
        assert_eq!(layout(&doc, 5), "(\n  longcontent\n)");
    }

    #[test]
    fn trailing_spaces_are_trimmed_at_breaks() {
        // A Line right before a break must not leave a trailing space on the line.
        let doc = concat([Doc::text("a"), Doc::HardLine, Doc::text("b")]);
        assert_eq!(layout(&doc, 80), "a\nb");
    }
}
