// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Byte-offset source cursor (rustc_lexer style).
//!
//! `Cursor` is the low-level navigation layer: it peeks and advances over the
//! source's *bytes* and tracks a `u32` byte offset. It carries no notion of
//! tokens; the scanner ([`super::scan`]) drives it to recognize lexemes.

/// A byte-offset cursor over UTF-8 source text.
///
/// `pos` is a byte offset, so it lines up directly with [`Span`] offsets and the
/// `&source[..]` slicing used to recover token text. The cursor stops on byte
/// boundaries that are always char boundaries, because every scanner consumes a
/// multi-byte UTF-8 sequence whole — identifiers char-by-char ([`bump_char`]),
/// quoted bodies by running to their ASCII delimiter — so slicing the source at a
/// cursor offset never splits a `char`.
///
/// [`bump_char`]: Cursor::bump_char
///
/// # Source length invariant
///
/// Offsets are `u32`. The cursor therefore assumes `src.len() <= u32::MAX`;
/// [`tokenize`] enforces this before constructing a cursor. Constructing a
/// cursor directly over a longer source and advancing past `u32::MAX` would
/// overflow `pos`.
///
/// [`Span`]: crate::ast::Span
/// [`tokenize`]: crate::tokenizer::tokenize
#[derive(Clone, Copy, Debug)]
pub struct Cursor<'a> {
    src: &'a str,
    pos: u32,
}

impl<'a> Cursor<'a> {
    /// Create a cursor at the start of `src`.
    pub fn new(src: &'a str) -> Self {
        debug_assert!(
            u32::try_from(src.len()).is_ok(),
            "Cursor source length must fit in u32; tokenize() guards this",
        );
        Self { src, pos: 0 }
    }

    /// The source this cursor walks.
    ///
    /// Returns the `&'a str` (a `Copy` reborrow), so callers can hold the source
    /// and derive `&'a` slices from it while still mutating the cursor.
    pub fn src(&self) -> &'a str {
        self.src
    }

    /// The current byte offset.
    pub fn pos(&self) -> u32 {
        self.pos
    }

    /// True once the cursor has reached the end of the source.
    pub fn is_eof(&self) -> bool {
        self.pos as usize >= self.src.len()
    }

    /// The byte at the cursor, or `None` at end of input.
    pub fn peek(&self) -> Option<u8> {
        self.src.as_bytes().get(self.pos as usize).copied()
    }

    /// The byte `n` positions ahead of the cursor, without advancing.
    ///
    /// The index is computed in `usize` so a large `n` cannot overflow the `u32`
    /// offset; out-of-range lookups return `None`.
    pub fn peek_nth(&self, n: u32) -> Option<u8> {
        let index = self.pos as usize + n as usize;
        self.src.as_bytes().get(index).copied()
    }

    /// The not-yet-consumed bytes, `&source[pos..]`.
    ///
    /// Borrows the source for `'a`, not the cursor, so the caller may keep the
    /// slice while continuing to advance the cursor.
    pub fn rest(&self) -> &'a [u8] {
        &self.src.as_bytes()[self.pos as usize..]
    }

    /// Advance one byte, returning the byte moved past, or `None` at end of input.
    pub fn bump(&mut self) -> Option<u8> {
        let byte = self.peek()?;
        // `peek` returned `Some`, so `pos < len <= u32::MAX`; the increment
        // cannot overflow under the source-length invariant.
        self.pos += 1;
        Some(byte)
    }

    /// Advance the cursor by `n` bytes in one step.
    ///
    /// A bulk [`bump`](Self::bump) for a run the caller has already confirmed is
    /// present — a fixed operator width, a measured prefix, a whole multi-byte
    /// character — so the per-byte bounds check `bump` repeats is skipped on warm
    /// paths. The caller owes the in-bounds guarantee (`pos + n <= len`), which the
    /// `debug_assert!` verifies in debug builds; the direct `pos` write then mirrors
    /// [`bump_char`](Self::bump_char), staying within the source-length invariant.
    pub fn advance_bytes(&mut self, n: u32) {
        debug_assert!(
            self.pos as usize + n as usize <= self.src.len(),
            "advance_bytes must stay within the source",
        );
        self.pos += n;
    }

    /// Advance while `pred` holds for the byte at the cursor.
    ///
    /// Stops at the first non-matching byte or at end of input.
    pub fn eat_while(&mut self, mut pred: impl FnMut(u8) -> bool) {
        while let Some(byte) = self.peek() {
            if !pred(byte) {
                break;
            }
            self.pos += 1;
        }
    }

    /// The whole UTF-8 character beginning `n` bytes ahead of the cursor, or `None`
    /// at or past end of input.
    ///
    /// Used where a lexeme's class is a per-*character* Unicode property rather than
    /// a per-byte one (non-ASCII identifier characters). `pos + n` must fall on a
    /// char boundary; every caller offsets by whole ASCII bytes (the cursor itself,
    /// or one past an ASCII sigil), so it always does — and a non-boundary offset
    /// yields `None` rather than splitting a character, because the slice is taken
    /// with [`str::get`].
    pub fn char_at(&self, n: u32) -> Option<char> {
        self.src
            .get(self.pos as usize + n as usize..)?
            .chars()
            .next()
    }

    /// Advance past one whole UTF-8 character, returning it, or `None` at end of
    /// input.
    ///
    /// Unlike [`bump`](Self::bump), which moves a single byte, this consumes every
    /// byte of a multi-byte character, so the cursor always stops on a char boundary
    /// — the invariant that lets a token span slice back to valid UTF-8.
    pub fn bump_char(&mut self) -> Option<char> {
        let ch = self.char_at(0)?;
        // `pos < len <= u32::MAX` and `pos + ch.len_utf8() <= len`, so this stays
        // within the source-length invariant and cannot overflow.
        self.pos += ch.len_utf8() as u32;
        Some(ch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peek_and_bump_walk_bytes_and_track_offset() {
        let mut cursor = Cursor::new("ab");

        assert_eq!(cursor.pos(), 0);
        assert!(!cursor.is_eof());
        assert_eq!(cursor.peek(), Some(b'a'));
        assert_eq!(cursor.peek_nth(1), Some(b'b'));
        assert_eq!(cursor.peek_nth(2), None);

        assert_eq!(cursor.bump(), Some(b'a'));
        assert_eq!(cursor.pos(), 1);
        assert_eq!(cursor.bump(), Some(b'b'));
        assert_eq!(cursor.pos(), 2);
        assert!(cursor.is_eof());
        assert_eq!(cursor.bump(), None);
        assert_eq!(cursor.pos(), 2);
    }

    #[test]
    fn eat_while_stops_at_first_non_match_and_at_eof() {
        let mut cursor = Cursor::new("123abc");
        cursor.eat_while(|b| b.is_ascii_digit());
        assert_eq!(cursor.pos(), 3);
        assert_eq!(cursor.peek(), Some(b'a'));

        cursor.eat_while(|b| b.is_ascii_alphabetic());
        assert_eq!(cursor.pos(), 6);
        assert!(cursor.is_eof());
    }

    #[test]
    fn rest_returns_remaining_bytes_from_offset() {
        let mut cursor = Cursor::new("a$tag$");
        cursor.bump();
        assert_eq!(cursor.rest(), b"$tag$");
    }

    #[test]
    fn char_at_and_bump_char_walk_whole_characters() {
        // `é` is two UTF-8 bytes, `δ` is two, `🎉` is four: `bump_char` must advance
        // by the character's byte length so the offset stays on a char boundary.
        let mut cursor = Cursor::new("aé🎉");
        assert_eq!(cursor.char_at(0), Some('a'));
        assert_eq!(cursor.char_at(1), Some('é'));

        assert_eq!(cursor.bump_char(), Some('a'));
        assert_eq!(cursor.pos(), 1);
        assert_eq!(cursor.bump_char(), Some('é'));
        assert_eq!(cursor.pos(), 3); // skipped both bytes of `é`
        assert_eq!(cursor.bump_char(), Some('🎉'));
        assert_eq!(cursor.pos(), 7); // skipped all four bytes of `🎉`
        assert_eq!(cursor.bump_char(), None);

        // An offset that lands mid-character yields `None` rather than panicking.
        let cursor = Cursor::new("é");
        assert_eq!(cursor.char_at(1), None);
    }
}
