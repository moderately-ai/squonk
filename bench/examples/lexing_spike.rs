// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Lexer inner-loop throughput spike for `prod-perf-simd-branchless-lexing-spike`.
//!
//! The perf-ceiling note (`docs/performance.md`) measures the
//! scalar byte-at-a-time lexer at ~9-13% of parse CPU — `next_token` dispatch
//! ~44% of tokenize-only time, the run-scanners (`scan_word` +
//! `eat_identifier_continue` + number runs) ~25% — the single biggest UNTAPPED
//! compute lever now that keyword lookup is at floor. This example asks, WITHOUT
//! touching the live lexer (ADR-0005): how much faster can the four hot scan inner
//! loops go, and by which technique?
//!
//! The four inner loops it models, lifted from `tokenizer/scan.rs`:
//!   - whitespace / trivia skip   (`skip_trivia` -> `eat_while(WHITESPACE)`)
//!   - identifier-continue run     (`eat_identifier_continue`)
//!   - decimal-digit run           (`eat_decimal_digits`)
//!   - quoted-string body skip     (`scan_quoted_body`, the `'...'` common case)
//!
//! Each loop is implemented in up to five forms over the SAME bytes, asserted to
//! produce IDENTICAL scan results before any timing (see `verify`):
//! - `naive` — a per-byte predicate with no class table (`is_ascii_*`), the shape a from-scratch lexer writes before a LUT exists.
//! - `live` — the deployed shape: the real 256-byte class LUT (`STANDARD_BYTE_CLASSES`, reused verbatim) consulted per byte through an `Option`-returning peek, plus the live secondary branches (the identifier `byte < 0x80` gate). This is what `scan.rs` runs TODAY for the run loops.
//! - `branchless` — the same LUT classification but over the `&[u8]` slice with no per-byte bounds-checked `Option` and no secondary branch: one load + mask, `take_while`. Safe, dep-free.
//! - `swar` — SIMD-within-a-register: 8 bytes per `u64`, class membership by bit-tricks (`haszero` plus a high-bit-cleared range test), scalar tail. Portable, no intrinsics, no `unsafe`.
//! - `simd` — explicit `core::arch` SSE2/NEON, 16 bytes per vector (digit run + string-body memchr only). The ONLY `unsafe` here, in the isolated `simd` module — the cost the recommendation weighs.
//!
//! For the string-body loop the live shape is NOT a LUT (it is a per-byte `match`
//! hunting one delimiter), so its candidates are scalar / SwAR-memchr / SIMD-memchr.
//!
//! Inputs come in SHORT-run and LONG-run variants on purpose: real SQL tokens are
//! short (identifiers ~4-12 B, numbers ~1-4 B), often shorter than one 8-byte SwAR
//! word or 16-byte vector, so the wide methods pay setup/tail with little parallel
//! benefit — the crossover this spike exists to quantify. Throughput is reported as
//! ns/pass and MB/s, with the speedup vs the LIVE shape (the honest baseline).
//!
//! Endianness: words are loaded with `from_le_bytes`, so byte i of a chunk lands in
//! bits `[8i, 8i+8)` on ANY host and `trailing_zeros()/8` is the first matching byte
//! regardless of host endianness. Inputs are kept ASCII so the byte-level class
//! models match the live lexer exactly (a non-ASCII identifier byte takes the live
//! char-decode path, which no byte-parallel method changes).
//!
//! Deterministic by construction (fixed inputs, fixed iteration counts, no RNG, no
//! timing-derived control flow). Build OPTIMIZED — NEVER measure in a debug build
//! (the workspace `opt-level=1` optimizes deps but leaves our code at `opt-level=0`,
//! pitting an unoptimized scanner against an optimized allocator/std):
//!
//! ```text
//! cargo run --profile profiling --example lexing_spike -p squonk-bench
//! cargo run --release          --example lexing_spike -p squonk-bench
//! ```

use squonk_ast::dialect::lex_class::{
    CLASS_DIGIT, CLASS_IDENTIFIER_CONTINUE, CLASS_IDENTIFIER_START, CLASS_WHITESPACE,
    STANDARD_BYTE_CLASSES,
};
use squonk_bench::time_ns;

// ---------------------------------------------------------------------------
// LUT classification — the live lexer's exact classifier, reused verbatim.
// ---------------------------------------------------------------------------

#[inline]
fn is_ws(b: u8) -> bool {
    STANDARD_BYTE_CLASSES.has_class(b, CLASS_WHITESPACE)
}
#[inline]
fn is_idstart(b: u8) -> bool {
    STANDARD_BYTE_CLASSES.has_class(b, CLASS_IDENTIFIER_START)
}
#[inline]
fn is_idcont(b: u8) -> bool {
    STANDARD_BYTE_CLASSES.has_class(b, CLASS_IDENTIFIER_CONTINUE)
}
#[inline]
fn is_digit(b: u8) -> bool {
    STANDARD_BYTE_CLASSES.has_class(b, CLASS_DIGIT)
}

// ---------------------------------------------------------------------------
// Generic per-byte skip drivers: `live` peeks an `Option` per byte (the real
// `Cursor::eat_while` shape); `branchless` runs the predicate over the slice with
// no per-byte bounds check. Both take the SAME predicate, so the delta is purely
// loop structure + secondary-branch removal.
// ---------------------------------------------------------------------------

#[inline]
fn skip_peek(bytes: &[u8], start: usize, pred: impl Fn(u8) -> bool) -> usize {
    let mut i = start;
    while let Some(&b) = bytes.get(i) {
        if !pred(b) {
            break;
        }
        i += 1;
    }
    i
}

#[inline]
fn skip_slice(bytes: &[u8], start: usize, pred: impl Fn(u8) -> bool) -> usize {
    start + bytes[start..].iter().take_while(|&&b| pred(b)).count()
}

// ---------------------------------------------------------------------------
// SwAR (SIMD-within-a-register): 8 bytes per u64 via the classic bit-tricks.
// ---------------------------------------------------------------------------

const ONES: u64 = 0x0101_0101_0101_0101;
const HIGHS: u64 = 0x8080_8080_8080_8080;

#[inline]
fn bcast(b: u8) -> u64 {
    ONES.wrapping_mul(u64::from(b))
}

/// 0x80 in each lane equal to `c` (the `haszero` trick on `w ^ broadcast(c)`).
/// Exact per lane — equality is the one SwAR compare with no cross-lane interaction.
#[inline]
fn swar_eq(w: u64, c: u8) -> u64 {
    let x = w ^ bcast(c);
    x.wrapping_sub(ONES) & !x & HIGHS
}

/// 0x80 in each lane whose byte is in `[lo, hi]` inclusive, for `lo, hi < 0x80`;
/// bytes `>= 0x80` are reported NOT in range. EXACT per lane.
///
/// The naive `hasless`/`hasmore` bit-tricks are a whole-word *test*, not a per-lane
/// mask — a borrow from one lane corrupts its neighbour. Clearing the high bit first
/// keeps every lane below `0x80`, so adding `0x80 - lo` (itself `<= 0x80`) cannot
/// carry across a lane boundary, and the lane's high bit then reads out the compare
/// exactly. Verified against a brute-force per-byte oracle before use.
#[inline]
fn swar_in_range_ascii(w: u64, lo: u8, hi: u8) -> u64 {
    let x = w & !HIGHS; // low 7 bits per lane — no cross-lane carry below
    let ge_lo = x.wrapping_add(bcast(0x80 - lo)) & HIGHS; // low7 >= lo
    let le_hi = !x.wrapping_add(bcast(0x7f - hi)) & HIGHS; // low7 <= hi
    let high_clear = !w & HIGHS; // original byte < 0x80
    ge_lo & le_hi & high_clear
}

/// 0x80 in lanes where a whitespace skip must STOP (byte is NOT whitespace).
#[inline]
fn swar_ws_stop(w: u64) -> u64 {
    let in_ws = swar_eq(w, b' ')
        | swar_eq(w, b'\t')
        | swar_eq(w, b'\n')
        | swar_eq(w, 0x0c)
        | swar_eq(w, b'\r');
    !in_ws & HIGHS
}

/// 0x80 in lanes where a digit-run skip must STOP (byte is NOT `0-9`).
#[inline]
fn swar_digit_stop(w: u64) -> u64 {
    !swar_in_range_ascii(w, b'0', b'9') & HIGHS
}

/// 0x80 in lanes where an identifier-continue skip must STOP. In-class is
/// `[A-Z] | [a-z] | [0-9] | '_' | >=0x80`, matching the LUT's continue set so the
/// high-byte lane agrees with `is_idcont` even though the inputs here are ASCII.
#[inline]
fn swar_idcont_stop(w: u64) -> u64 {
    let in_class = swar_in_range_ascii(w, b'A', b'Z')
        | swar_in_range_ascii(w, b'a', b'z')
        | swar_in_range_ascii(w, b'0', b'9')
        | swar_eq(w, b'_')
        | (w & HIGHS);
    !in_class & HIGHS
}

/// Word-at-a-time run skip: process 8 bytes per `u64`, stop at the first lane the
/// `stop_word` bit-trick marks; a partial tail (< 8 bytes) falls back to `stop_byte`.
#[inline]
fn swar_skip(
    bytes: &[u8],
    start: usize,
    stop_word: fn(u64) -> u64,
    stop_byte: fn(u8) -> bool,
) -> usize {
    let mut i = start;
    while i + 8 <= bytes.len() {
        let w = u64::from_le_bytes(bytes[i..i + 8].try_into().unwrap());
        let stop = stop_word(w);
        if stop != 0 {
            return i + (stop.trailing_zeros() / 8) as usize;
        }
        i += 8;
    }
    while let Some(&b) = bytes.get(i) {
        if stop_byte(b) {
            break;
        }
        i += 1;
    }
    i
}

/// SwAR memchr: index of the first `needle` at or after `start`, else `None`.
#[inline]
fn swar_memchr(bytes: &[u8], start: usize, needle: u8) -> Option<usize> {
    let mut i = start;
    while i + 8 <= bytes.len() {
        let w = u64::from_le_bytes(bytes[i..i + 8].try_into().unwrap());
        let m = swar_eq(w, needle);
        if m != 0 {
            return Some(i + (m.trailing_zeros() / 8) as usize);
        }
        i += 8;
    }
    while let Some(&b) = bytes.get(i) {
        if b == needle {
            return Some(i);
        }
        i += 1;
    }
    None
}

// ---------------------------------------------------------------------------
// Quoted-string body scan (`'...'`, doubled `''` is an escaped quote, no backslash
// — the PostgreSQL default). Returns the index just past the closing quote. The
// scalar form mirrors `scan_quoted_body`; the SwAR/SIMD forms memchr to the next
// quote then resolve doubling at the boundary.
// ---------------------------------------------------------------------------

#[inline]
fn scan_string_scalar(bytes: &[u8], start: usize) -> usize {
    let mut i = start + 1; // past the opening quote
    loop {
        match bytes.get(i) {
            None => return i,
            Some(&b'\'') => {
                if bytes.get(i + 1) == Some(&b'\'') {
                    i += 2; // doubled: an escaped literal quote
                } else {
                    return i + 1; // closing quote
                }
            }
            Some(_) => i += 1,
        }
    }
}

#[inline]
fn scan_string_swar(bytes: &[u8], start: usize) -> usize {
    let mut i = start + 1;
    loop {
        match swar_memchr(bytes, i, b'\'') {
            None => return bytes.len(),
            Some(p) => {
                if bytes.get(p + 1) == Some(&b'\'') {
                    i = p + 2;
                } else {
                    return p + 1;
                }
            }
        }
    }
}

#[inline]
fn scan_string_simd(bytes: &[u8], start: usize) -> usize {
    let mut i = start + 1;
    loop {
        match simd::memchr(&bytes[i..], b'\'').map(|p| i + p) {
            None => return bytes.len(),
            Some(p) => {
                if bytes.get(p + 1) == Some(&b'\'') {
                    i = p + 2;
                } else {
                    return p + 1;
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Explicit SIMD (core::arch). The ONLY `unsafe` in this spike, scoped to this
// module behind `#![allow(unsafe_code)]` — the workspace denies `unsafe` (ADR-0017)
// and the live lexer + AST are unsafe-free, so this attribute IS the cost being
// weighed. SSE2 (x86_64) and NEON (aarch64) are baseline ISA, so there is no
// runtime feature detection; going wider (AVX2/AVX-512/SVE) WOULD need
// `is_*_feature_detected!` multiversioning — the further cost noted in the doc.
// ---------------------------------------------------------------------------

mod simd {
    #![allow(unsafe_code)]

    /// Which 16-wide backend was compiled for the running target.
    pub const BACKEND: &str = backend();

    const fn backend() -> &'static str {
        if cfg!(target_arch = "aarch64") {
            "NEON (aarch64, 16-wide)"
        } else if cfg!(target_arch = "x86_64") {
            "SSE2 (x86_64, 16-wide)"
        } else {
            "scalar fallback (no SIMD path for this arch)"
        }
    }

    /// Index of the first `needle` in `h`, else `None`.
    #[inline]
    pub fn memchr(h: &[u8], needle: u8) -> Option<usize> {
        #[cfg(target_arch = "aarch64")]
        let r = neon_memchr(h, needle);
        #[cfg(target_arch = "x86_64")]
        let r = sse2_memchr(h, needle);
        #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
        let r = h.iter().position(|&b| b == needle);
        r
    }

    /// Length of the leading `0-9` run in `h`.
    #[inline]
    pub fn skip_digits(h: &[u8]) -> usize {
        #[cfg(target_arch = "aarch64")]
        let r = neon_skip_digits(h);
        #[cfg(target_arch = "x86_64")]
        let r = sse2_skip_digits(h);
        #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
        let r = h.iter().take_while(|&&b| b.is_ascii_digit()).count();
        r
    }

    #[cfg(target_arch = "aarch64")]
    fn neon_memchr(h: &[u8], needle: u8) -> Option<usize> {
        use core::arch::aarch64::{vceqq_u8, vdupq_n_u8, vld1q_u8, vmaxvq_u8};
        let mut i = 0;
        // SAFETY: each load reads 16 bytes guarded by `i + 16 <= h.len()`; NEON is
        // a baseline aarch64 feature, so the intrinsics are always available.
        unsafe {
            let n = vdupq_n_u8(needle);
            while i + 16 <= h.len() {
                let chunk = vld1q_u8(h.as_ptr().add(i));
                if vmaxvq_u8(vceqq_u8(chunk, n)) != 0 {
                    return h[i..i + 16]
                        .iter()
                        .position(|&b| b == needle)
                        .map(|p| i + p);
                }
                i += 16;
            }
        }
        h[i..].iter().position(|&b| b == needle).map(|p| i + p)
    }

    #[cfg(target_arch = "aarch64")]
    fn neon_skip_digits(h: &[u8]) -> usize {
        use core::arch::aarch64::{vandq_u8, vcgeq_u8, vcleq_u8, vdupq_n_u8, vld1q_u8, vminvq_u8};
        let mut i = 0;
        // SAFETY: as in `neon_memchr` — every 16-byte load is length-guarded.
        unsafe {
            let lo = vdupq_n_u8(b'0');
            let hi = vdupq_n_u8(b'9');
            while i + 16 <= h.len() {
                let chunk = vld1q_u8(h.as_ptr().add(i));
                let is_dig = vandq_u8(vcgeq_u8(chunk, lo), vcleq_u8(chunk, hi));
                if vminvq_u8(is_dig) == 0 {
                    return i + h[i..i + 16]
                        .iter()
                        .position(|&b| !b.is_ascii_digit())
                        .unwrap_or(16);
                }
                i += 16;
            }
        }
        i + h[i..].iter().take_while(|&&b| b.is_ascii_digit()).count()
    }

    #[cfg(target_arch = "x86_64")]
    fn sse2_memchr(h: &[u8], needle: u8) -> Option<usize> {
        use core::arch::x86_64::{
            _mm_cmpeq_epi8, _mm_loadu_si128, _mm_movemask_epi8, _mm_set1_epi8,
        };
        let mut i = 0;
        // SAFETY: each load reads 16 bytes guarded by `i + 16 <= h.len()`; SSE2 is a
        // baseline x86_64 feature, so the intrinsics are always available.
        unsafe {
            let n = _mm_set1_epi8(needle as i8);
            while i + 16 <= h.len() {
                let chunk = _mm_loadu_si128(h.as_ptr().add(i).cast());
                let mask = _mm_movemask_epi8(_mm_cmpeq_epi8(chunk, n));
                if mask != 0 {
                    return Some(i + (mask as u32).trailing_zeros() as usize);
                }
                i += 16;
            }
        }
        h[i..].iter().position(|&b| b == needle).map(|p| i + p)
    }

    #[cfg(target_arch = "x86_64")]
    fn sse2_skip_digits(h: &[u8]) -> usize {
        use core::arch::x86_64::{
            _mm_and_si128, _mm_cmpgt_epi8, _mm_loadu_si128, _mm_movemask_epi8, _mm_set1_epi8,
            _mm_xor_si128,
        };
        let mut i = 0;
        // SAFETY: every 16-byte load is length-guarded; SSE2 is baseline. Bytes are
        // biased by 0x80 so the signed `cmpgt` realizes an unsigned range compare.
        unsafe {
            let bias = _mm_set1_epi8(-128_i8);
            let lo = _mm_set1_epi8((((b'0' as i32) - 1) ^ 0x80) as i8); // > '0'-1  => >= '0'
            let hi = _mm_set1_epi8((((b'9' as i32) + 1) ^ 0x80) as i8); // < '9'+1  => <= '9'
            while i + 16 <= h.len() {
                let c = _mm_xor_si128(_mm_loadu_si128(h.as_ptr().add(i).cast()), bias);
                let is_dig = _mm_and_si128(_mm_cmpgt_epi8(c, lo), _mm_cmpgt_epi8(hi, c));
                let mask = _mm_movemask_epi8(is_dig) as u32 & 0xffff;
                if mask != 0xffff {
                    return i + (!mask & 0xffff).trailing_zeros() as usize;
                }
                i += 16;
            }
        }
        i + h[i..].iter().take_while(|&&b| b.is_ascii_digit()).count()
    }
}

// ---------------------------------------------------------------------------
// Walk drivers — apply a run-skip across a whole input (the per-loop throughput
// shape), and a blended tokenize-shape walk (the realistic mix). Both fold run-end
// offsets into a checksum so the optimizer cannot elide the scan and so every
// technique can be asserted to walk the input identically.
// ---------------------------------------------------------------------------

/// Walk `bytes`, skipping each run `skip` recognizes and advancing one byte past any
/// non-run byte; fold run ends into a checksum. All techniques for one class return
/// the same checksum on the same input.
#[inline]
fn run_skip_walk(bytes: &[u8], mut skip: impl FnMut(&[u8], usize) -> usize) -> u64 {
    let mut i = 0;
    let mut acc = 0u64;
    while i < bytes.len() {
        let j = skip(bytes, i);
        if j > i {
            acc = acc.wrapping_add(j as u64);
            i = j;
        } else {
            i += 1;
        }
    }
    acc
}

/// Blended tokenize-shape walk: dispatch on the first byte's class (identical LUT
/// dispatch for every technique) and skip the matching run; the skip closures are
/// the only thing that varies. Returns `(end, checksum)`.
#[inline]
fn blended_walk(
    bytes: &[u8],
    mut skip_ws: impl FnMut(&[u8], usize) -> usize,
    mut skip_id: impl FnMut(&[u8], usize) -> usize,
    mut skip_dg: impl FnMut(&[u8], usize) -> usize,
    mut scan_str: impl FnMut(&[u8], usize) -> usize,
) -> (usize, u64) {
    let mut i = 0;
    let mut acc = 0u64;
    while i < bytes.len() {
        let b = bytes[i];
        let j = if is_ws(b) {
            skip_ws(bytes, i)
        } else if is_idstart(b) {
            skip_id(bytes, i)
        } else if is_digit(b) {
            skip_dg(bytes, i)
        } else if b == b'\'' {
            scan_str(bytes, i)
        } else {
            i + 1
        };
        acc = acc.wrapping_add(j as u64);
        i = if j > i { j } else { i + 1 };
    }
    (i, acc)
}

// ---------------------------------------------------------------------------
// Inputs — SHORT-run and LONG-run variants, ASCII, sized to ~16 KB per pass.
// ---------------------------------------------------------------------------

const TARGET_LEN: usize = 16_384;

fn repeat_to(pattern: &str, target: usize) -> String {
    let mut s = String::with_capacity(target + pattern.len());
    while s.len() < target {
        s.push_str(pattern);
    }
    s
}

/// A TPC-DS-shaped star join (the realistic blend; identical to the perf testbed).
const MIXED_SQL: &str = "SELECT i_item_id, AVG(ss_quantity) AS agg1, AVG(ss_list_price) AS agg2, AVG(ss_coupon_amt) AS agg3 FROM store_sales, customer_demographics, date_dim, item, promotion WHERE ss_sold_date_sk = d_date_sk AND ss_item_sk = i_item_sk AND ss_cdemo_sk = cd_demo_sk AND ss_promo_sk = p_promo_sk AND cd_gender = 'M' AND cd_marital_status = 'S' AND cd_education_status = 'College' AND d_year = 2000 AND (p_channel_email = 'N' OR p_channel_event = 'N') GROUP BY i_item_id ORDER BY i_item_id ";

// ---------------------------------------------------------------------------
// Correctness: every technique must agree on every input before timing.
// ---------------------------------------------------------------------------

fn verify() {
    let ws = repeat_to("\n            x", TARGET_LEN);
    let id_short = repeat_to("id,name,price,qty,total,user_id,", TARGET_LEN);
    let id_long = repeat_to("customer_demographics_extended_attribute_name,", TARGET_LEN);
    let dg_short = repeat_to("1,42,2024,7,128,9,", TARGET_LEN);
    let dg_long = repeat_to("123456789012345678901234567890,", TARGET_LEN);
    let str_short = repeat_to("'M','S','N','College','F',", TARGET_LEN);
    let str_long = repeat_to(
        "'the quick brown fox jumps over the lazy dog and on it goes',",
        TARGET_LEN,
    );

    // Whitespace.
    let b = ws.as_bytes();
    let want = run_skip_walk(b, |b, s| skip_peek(b, s, is_ws));
    assert_eq!(
        want,
        run_skip_walk(b, |b, s| skip_peek(b, s, |x| x.is_ascii_whitespace())),
        "ws naive"
    );
    assert_eq!(
        want,
        run_skip_walk(b, |b, s| skip_slice(b, s, is_ws)),
        "ws branchless"
    );
    assert_eq!(
        want,
        run_skip_walk(b, |b, s| swar_skip(b, s, swar_ws_stop, |x| !is_ws(x))),
        "ws swar"
    );

    // Identifier-continue.
    for input in [&id_short, &id_long] {
        let b = input.as_bytes();
        let want = run_skip_walk(b, |b, s| skip_peek(b, s, is_idcont));
        assert_eq!(
            want,
            run_skip_walk(b, |b, s| skip_peek(b, s, |x| x == b'_'
                || x.is_ascii_alphanumeric()
                || x >= 0x80)),
            "id naive"
        );
        assert_eq!(
            want,
            run_skip_walk(b, |b, s| skip_slice(b, s, is_idcont)),
            "id branchless"
        );
        assert_eq!(
            want,
            run_skip_walk(b, |b, s| swar_skip(b, s, swar_idcont_stop, |x| !is_idcont(
                x
            ))),
            "id swar"
        );
    }

    // Digit.
    for input in [&dg_short, &dg_long] {
        let b = input.as_bytes();
        let want = run_skip_walk(b, |b, s| skip_peek(b, s, is_digit));
        assert_eq!(
            want,
            run_skip_walk(b, |b, s| skip_peek(b, s, |x| x.is_ascii_digit())),
            "dg naive"
        );
        assert_eq!(
            want,
            run_skip_walk(b, |b, s| skip_slice(b, s, is_digit)),
            "dg branchless"
        );
        assert_eq!(
            want,
            run_skip_walk(b, |b, s| swar_skip(b, s, swar_digit_stop, |x| !is_digit(x))),
            "dg swar"
        );
        assert_eq!(
            want,
            run_skip_walk(b, |b, s| s + simd::skip_digits(&b[s..])),
            "dg simd"
        );
    }

    // Quoted-string body.
    for input in [&str_short, &str_long] {
        let b = input.as_bytes();
        let want = string_walk(b, scan_string_scalar);
        assert_eq!(want, string_walk(b, scan_string_swar), "str swar");
        assert_eq!(want, string_walk(b, scan_string_simd), "str simd");
    }

    // Blended walk: live / branchless / swar must agree.
    let b = repeat_to(MIXED_SQL, TARGET_LEN);
    let b = b.as_bytes();
    let live = blended_walk(
        b,
        |b, s| skip_peek(b, s, is_ws),
        |b, s| skip_peek(b, s, is_idcont),
        |b, s| skip_peek(b, s, is_digit),
        scan_string_scalar,
    );
    let branchless = blended_walk(
        b,
        |b, s| skip_slice(b, s, is_ws),
        |b, s| skip_slice(b, s, is_idcont),
        |b, s| skip_slice(b, s, is_digit),
        scan_string_scalar,
    );
    let swar = blended_walk(
        b,
        |b, s| swar_skip(b, s, swar_ws_stop, |x| !is_ws(x)),
        |b, s| swar_skip(b, s, swar_idcont_stop, |x| !is_idcont(x)),
        |b, s| swar_skip(b, s, swar_digit_stop, |x| !is_digit(x)),
        scan_string_swar,
    );
    assert_eq!(live, branchless, "blended branchless");
    assert_eq!(live, swar, "blended swar");
}

/// Walk a string-heavy input, scanning each `'...'` and advancing past other bytes;
/// fold string ends into a checksum (the string-body counterpart of `run_skip_walk`).
#[inline]
fn string_walk(bytes: &[u8], mut scan: impl FnMut(&[u8], usize) -> usize) -> u64 {
    let mut i = 0;
    let mut acc = 0u64;
    while i < bytes.len() {
        if bytes[i] == b'\'' {
            let j = scan(bytes, i);
            acc = acc.wrapping_add(j as u64);
            i = j;
        } else {
            i += 1;
        }
    }
    acc
}

// ---------------------------------------------------------------------------
// Timing
// ---------------------------------------------------------------------------

/// One technique's row: ns/pass, MB/s, and speedup vs the `baseline` ns.
fn row(label: &str, len: usize, ns: f64, baseline: f64) {
    let mb_s = len as f64 * 1000.0 / ns;
    println!(
        "  {label:<22} {ns:>10.1} {mb_s:>12.0} {:>10.2}x",
        baseline / ns
    );
}

/// A named scan technique: a label and a function that walks an input, returning a
/// checksum (so the optimizer cannot elide the scan).
type Technique = (&'static str, fn(&[u8]) -> u64);

/// Run every technique of a run-skip class over `input` and print the block. The
/// first technique (`live`) is the baseline every speedup is measured against.
fn bench_run_class(title: &str, input: &[u8], techniques: &[Technique], iters: u64) {
    println!(
        "\n## {title}  ({} B)   ns/pass | MB/s | x vs live",
        input.len()
    );
    println!(
        "  {:<22} {:>10} {:>12} {:>10}",
        "technique", "ns", "MB/s", "speedup"
    );
    let mut baseline = 0.0;
    for (idx, (label, f)) in techniques.iter().enumerate() {
        let ns = time_ns(iters, || f(input));
        if idx == 0 {
            baseline = ns;
        }
        row(label, input.len(), ns, baseline);
    }
}

fn main() {
    println!("# lexing inner-loop throughput spike (prod-perf-simd-branchless-lexing-spike)\n");
    println!("SIMD backend: {}", simd::BACKEND);
    println!(
        "Build: run under --profile profiling or --release ONLY; debug numbers are meaningless.\n\
         Baseline 'live' = the deployed shape (real class LUT via Option-peek). 'naive' = no LUT;\n\
         'branchless' = LUT over the slice; 'swar' = u64 bit-tricks; 'simd' = core::arch 16-wide."
    );

    verify();
    println!("\n[verify] all techniques agree on every input (scan results identical).");

    let ws = repeat_to("\n            x", TARGET_LEN);
    let id_short = repeat_to("id,name,price,qty,total,user_id,", TARGET_LEN);
    let id_long = repeat_to("customer_demographics_extended_attribute_name,", TARGET_LEN);
    let dg_short = repeat_to("1,42,2024,7,128,9,", TARGET_LEN);
    let dg_long = repeat_to("123456789012345678901234567890,", TARGET_LEN);
    let str_short = repeat_to("'M','S','N','College','F',", TARGET_LEN);
    let str_long = repeat_to(
        "'the quick brown fox jumps over the lazy dog and on it goes',",
        TARGET_LEN,
    );
    let mixed = repeat_to(MIXED_SQL, TARGET_LEN);

    let iters = 80_000u64;

    // Whitespace skip.
    bench_run_class(
        "whitespace skip — indent-heavy",
        ws.as_bytes(),
        &[
            ("live (LUT/peek)", |b| {
                run_skip_walk(b, |b, s| skip_peek(b, s, is_ws))
            }),
            ("naive (no LUT)", |b| {
                run_skip_walk(b, |b, s| skip_peek(b, s, |x| x.is_ascii_whitespace()))
            }),
            ("branchless (LUT)", |b| {
                run_skip_walk(b, |b, s| skip_slice(b, s, is_ws))
            }),
            ("swar (u64)", |b| {
                run_skip_walk(b, |b, s| swar_skip(b, s, swar_ws_stop, |x| !is_ws(x)))
            }),
        ],
        iters,
    );

    // Identifier-continue run, short and long.
    for (title, input) in [
        ("identifier run — SHORT (~3-7 B)", &id_short),
        ("identifier run — LONG (~44 B)", &id_long),
    ] {
        bench_run_class(
            title,
            input.as_bytes(),
            &[
                ("live (LUT/peek+gate)", |b| {
                    run_skip_walk(b, |b, s| skip_peek(b, s, is_idcont))
                }),
                ("naive (no LUT)", |b| {
                    run_skip_walk(b, |b, s| {
                        skip_peek(b, s, |x| {
                            x == b'_' || x.is_ascii_alphanumeric() || x >= 0x80
                        })
                    })
                }),
                ("branchless (LUT)", |b| {
                    run_skip_walk(b, |b, s| skip_slice(b, s, is_idcont))
                }),
                ("swar (u64)", |b| {
                    run_skip_walk(b, |b, s| {
                        swar_skip(b, s, swar_idcont_stop, |x| !is_idcont(x))
                    })
                }),
            ],
            iters,
        );
    }

    // Digit run, short and long (with SIMD).
    for (title, input) in [
        ("digit run — SHORT (~1-4 B)", &dg_short),
        ("digit run — LONG (~30 B)", &dg_long),
    ] {
        bench_run_class(
            title,
            input.as_bytes(),
            &[
                ("live (LUT/peek)", |b| {
                    run_skip_walk(b, |b, s| skip_peek(b, s, is_digit))
                }),
                ("naive (no LUT)", |b| {
                    run_skip_walk(b, |b, s| skip_peek(b, s, |x| x.is_ascii_digit()))
                }),
                ("branchless (LUT)", |b| {
                    run_skip_walk(b, |b, s| skip_slice(b, s, is_digit))
                }),
                ("swar (u64)", |b| {
                    run_skip_walk(b, |b, s| swar_skip(b, s, swar_digit_stop, |x| !is_digit(x)))
                }),
                ("simd (16-wide)", |b| {
                    run_skip_walk(b, |b, s| s + simd::skip_digits(&b[s..]))
                }),
            ],
            iters,
        );
    }

    // Quoted-string body, short and long (scalar / SwAR / SIMD memchr).
    for (title, input) in [
        ("string body — SHORT (~1-10 B)", &str_short),
        ("string body — LONG (~58 B)", &str_long),
    ] {
        bench_run_class(
            title,
            input.as_bytes(),
            &[
                ("scalar (per-byte match)", |b| {
                    string_walk(b, scan_string_scalar)
                }),
                ("swar memchr (u64)", |b| string_walk(b, scan_string_swar)),
                ("simd memchr (16-wide)", |b| {
                    string_walk(b, scan_string_simd)
                }),
            ],
            iters,
        );
    }

    // Blended tokenize-shape walk over realistic mixed SQL.
    bench_run_class(
        "blended tokenize-shape walk — mixed SQL",
        mixed.as_bytes(),
        &[
            ("live (LUT/peek)", |b| {
                blended_walk(
                    b,
                    |b, s| skip_peek(b, s, is_ws),
                    |b, s| skip_peek(b, s, is_idcont),
                    |b, s| skip_peek(b, s, is_digit),
                    scan_string_scalar,
                )
                .1
            }),
            ("branchless (LUT)", |b| {
                blended_walk(
                    b,
                    |b, s| skip_slice(b, s, is_ws),
                    |b, s| skip_slice(b, s, is_idcont),
                    |b, s| skip_slice(b, s, is_digit),
                    scan_string_scalar,
                )
                .1
            }),
            ("swar (u64)", |b| {
                blended_walk(
                    b,
                    |b, s| swar_skip(b, s, swar_ws_stop, |x| !is_ws(x)),
                    |b, s| swar_skip(b, s, swar_idcont_stop, |x| !is_idcont(x)),
                    |b, s| swar_skip(b, s, swar_digit_stop, |x| !is_digit(x)),
                    scan_string_swar,
                )
                .1
            }),
        ],
        iters,
    );

    println!(
        "\nReading the tables: MB/s is per-pass throughput over the input; 'x vs live' > 1 means\n\
         faster than the deployed shape. The lexer is ~9-13% of parse CPU, and the run loops are a\n\
         fraction of that, so multiply a per-loop speedup by the run loops' parse share to bound\n\
         the end-to-end win (see docs/performance.md)."
    );
}
