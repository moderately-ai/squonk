// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Emit the keyword inventory, lookup, and per-position reservation bitsets.
//!
//! ADR-0004 calls for dep-free, codegen-generated keyword recognition. This
//! module is that generator: it reads the checked-in objective keyword
//! inventories (`crates/squonk-sourcegen/keyword_data/*.csv` — PostgreSQL
//! `kwlist.h` and the SQL:2016 Part 2 keyword lists) and renders the full
//! `Keyword` enum, its `ALL`/`as_str` tables, the allocation-free
//! `lookup_keyword`, and the per-dialect / per-category reservation bitsets.
//!
//! ## Per-position reservation (prod-keyword-position-reserved-sets)
//!
//! PostgreSQL reserves keywords *per grammatical position*, via the four `kwlist.h`
//! classes (`unreserved` / `col_name` / `type_func_name` / `reserved`) plus an
//! independent `BARE_LABEL` / `AS_LABEL` axis. This generator emits a bitset for each
//! reject-relevant category (`POSTGRES_{COL_NAME,TYPE_FUNC_NAME,RESERVED}_KEYWORDS`)
//! plus the `POSTGRES_AS_LABEL_KEYWORDS` bare-label reject set, and the hand-written
//! `dialect/keyword.rs` composes them into the per-position gates the parser
//! consults (`ColId`, function name, type name, bare label). The `unreserved` class
//! needs no bitset: an unreserved keyword is admissible in every position, so no gate
//! rejects it.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;

/// One keyword's inventory row: canonical lower-case spelling, the Rust variant
/// name, and its per-dialect / per-position reservation data.
struct KeywordRow {
    spelling: String,
    variant: String,
    /// PostgreSQL `kwlist.h` reservation class, or `None` when the spelling is not
    /// a PostgreSQL keyword (it appears only in the SQL:2016 inventory, so
    /// PostgreSQL treats it as a plain identifier).
    postgres_class: Option<PgClass>,
    /// PostgreSQL `kwlist.h` `AS_LABEL` designation: `true` when the keyword cannot
    /// be a bare column alias (`SELECT a <kw>`). `false` for `BARE_LABEL` keywords
    /// and for non-PostgreSQL spellings.
    postgres_as_label: bool,
    /// MySQL 8.0 reservation class, or `None` when the spelling is not a MySQL
    /// reserved word (MySQL treats it as a plain identifier).
    mysql_class: Option<MysqlClass>,
}

/// PostgreSQL `kwlist.h` keyword reservation class.
#[derive(Clone, Copy, PartialEq, Eq)]
enum PgClass {
    Unreserved,
    ColName,
    TypeFuncName,
    Reserved,
}

impl PgClass {
    /// Parse a `postgres_keywords.csv` class column.
    fn parse(class: &str) -> Self {
        match class {
            "unreserved" => Self::Unreserved,
            "col_name" => Self::ColName,
            "type_func_name" => Self::TypeFuncName,
            "reserved" => Self::Reserved,
            other => panic!("unknown PostgreSQL reservation class {other:?}"),
        }
    }
}

/// MySQL 8.0 reservation class. MySQL has a single reserved set plus grammar
/// carve-outs for built-in functions, so it needs only three classes (vs
/// PostgreSQL's four) — see `mysql_keywords.csv` for the mapping rationale.
#[derive(Clone, Copy, PartialEq, Eq)]
enum MysqlClass {
    /// Never a plain identifier: rejected as column, function, type, and bare alias.
    Reserved,
    /// Reserved as a column/type/bare-alias name but admissible as a *function* name
    /// (MySQL parses `kw(...)` as a built-in call, e.g. `LEFT`, `IF`, `MOD`).
    TypeFuncName,
    /// The inverse of [`TypeFuncName`](Self::TypeFuncName): admissible as a
    /// column/type/bare-alias name but reserved as a *function* name, because MySQL
    /// parses the bare word as an identifier yet syntax-rejects `kw(...)` as a call
    /// (only `array` on 8.4). Contributes to the function-name reject set alone.
    FunctionOnly,
}

impl MysqlClass {
    /// Parse a `mysql_keywords.csv` class column.
    fn parse(class: &str) -> Self {
        match class {
            "reserved" => Self::Reserved,
            "type_func_name" => Self::TypeFuncName,
            "function_only" => Self::FunctionOnly,
            other => panic!("unknown MySQL reservation class {other:?}"),
        }
    }
}

/// The directory holding the checked-in keyword source data.
fn keyword_data_dir() -> PathBuf {
    crate::workspace_root().join("crates/squonk-sourcegen/keyword_data")
}

/// Parse a `keyword,class` data file, skipping the `#` provenance header, blank
/// lines, and the `keyword,class` column header. Returns `(spelling, class)`.
fn load_csv(name: &str, valid_classes: &[&str]) -> Vec<(String, String)> {
    let path = keyword_data_dir().join(name);
    let text =
        fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
    let mut rows = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (keyword, class) = line
            .split_once(',')
            .unwrap_or_else(|| panic!("{}: malformed row {line:?}", path.display()));
        let (keyword, class) = (keyword.trim(), class.trim());
        if !valid_classes.contains(&class) {
            // The `keyword,class` header row (and only it) is skipped here.
            assert_eq!(
                keyword,
                "keyword",
                "{}: unknown reservation class {class:?}",
                path.display(),
            );
            continue;
        }
        rows.push((keyword.to_owned(), class.to_owned()));
    }
    rows
}

/// Convert a lower-case, `_`-separated keyword spelling to a PascalCase Rust
/// variant name (`current_date` -> `CurrentDate`).
///
/// PascalCasing turns most reserved words (`as` -> `As`) into ordinary
/// identifiers, but `Self` stays reserved and cannot be a bare variant — so any
/// PascalCase result that is still a Rust keyword gets a `_` suffix (`self` ->
/// `Self_`). The variant name is internal (only `as_str`/`lookup` map back to the
/// spelling), so the suffix is harmless.
fn variant_name(spelling: &str) -> String {
    let mut out = String::new();
    for segment in spelling.split('_') {
        let mut chars = segment.chars();
        if let Some(first) = chars.next() {
            out.extend(first.to_uppercase());
            out.push_str(chars.as_str());
        }
    }
    if is_rust_keyword(&out) {
        out.push('_');
    }
    out
}

/// Whether `ident` is a Rust keyword that cannot be used as a bare variant name.
///
/// `Self`/`self`/`super`/`crate` are the keywords reserved even after PascalCasing
/// (and `r#` raw identifiers cannot escape them); the rest are listed for
/// robustness against future inventory changes.
fn is_rust_keyword(ident: &str) -> bool {
    const KEYWORDS: &[&str] = &[
        "as", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern", "false",
        "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
        "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true", "type",
        "union", "unsafe", "use", "where", "while", "async", "await", "abstract", "become", "box",
        "do", "final", "macro", "override", "priv", "typeof", "unsized", "virtual", "yield", "try",
        "gen",
    ];
    KEYWORDS.contains(&ident)
}

/// Parse a single-column keyword list file (`# provenance`, a `keyword` header,
/// then one lower-case spelling per line).
fn load_keyword_list(name: &str) -> Vec<String> {
    let path = keyword_data_dir().join(name);
    let text =
        fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#') && *line != "keyword")
        .map(str::to_owned)
        .collect()
}

/// Load all source files and merge them into the sorted keyword inventory.
fn load_inventory() -> Vec<KeywordRow> {
    let postgres = load_csv(
        "postgres_keywords.csv",
        &["unreserved", "col_name", "type_func_name", "reserved"],
    );
    let ansi = load_csv("ansi_keywords.csv", &["reserved", "non_reserved"]);
    let mysql = load_csv(
        "mysql_keywords.csv",
        &["reserved", "type_func_name", "function_only"],
    );
    let sqlite = load_keyword_list("sqlite_keywords.csv");
    let duckdb = load_keyword_list("duckdb_keywords.csv");
    let bigquery = load_keyword_list("bigquery_keywords.csv");
    let mssql = load_keyword_list("mssql_keywords.csv");
    let as_label_list = load_keyword_list("postgres_as_label.csv");

    // Union the spellings, then attach each dialect's reservation. A spelling
    // absent from a dialect's list is non-reserved there (it may still be a
    // keyword via another dialect, which is why the enum is the union). MySQL
    // contributes its distinctive reserved words (`RLIKE`, `DIV`, `XOR`, …) to the
    // shared universe; they stay non-reserved under ANSI/PostgreSQL. SQLite,
    // DuckDB, BigQuery, and MSSQL contribute only the spellings absent from the other
    // inventories (`GLOB`, `QUALIFY`, `EXTEND`, `APPLY`) — carrying no reservation
    // class, so they are unreserved everywhere until a dialect's hand-composed
    // per-position set names them.
    let mut spellings: Vec<String> = postgres
        .iter()
        .map(|(keyword, _)| keyword.clone())
        .chain(ansi.iter().map(|(keyword, _)| keyword.clone()))
        .chain(mysql.iter().map(|(keyword, _)| keyword.clone()))
        .chain(sqlite.iter().cloned())
        .chain(duckdb.iter().cloned())
        .chain(bigquery.iter().cloned())
        .chain(mssql.iter().cloned())
        .collect();
    spellings.sort();
    spellings.dedup();

    let postgres_class: BTreeMap<&str, PgClass> = postgres
        .iter()
        .map(|(keyword, class)| (keyword.as_str(), PgClass::parse(class)))
        .collect();
    let mysql_class: BTreeMap<&str, MysqlClass> = mysql
        .iter()
        .map(|(keyword, class)| (keyword.as_str(), MysqlClass::parse(class)))
        .collect();
    let as_label: std::collections::BTreeSet<&str> =
        as_label_list.iter().map(String::as_str).collect();

    // Every AS_LABEL spelling must be a PostgreSQL keyword: the bare-label axis is
    // a `kwlist.h` designation, so a stray entry would be a transcription error.
    for keyword in &as_label {
        assert!(
            postgres_class.contains_key(keyword),
            "postgres_as_label.csv lists {keyword:?}, which is not a PostgreSQL keyword",
        );
    }

    let mut variants = std::collections::BTreeSet::new();
    let rows: Vec<KeywordRow> = spellings
        .into_iter()
        .map(|spelling| {
            let variant = variant_name(&spelling);
            assert!(
                variants.insert(variant.clone()),
                "keyword spelling {spelling:?} collides on Rust variant {variant:?}",
            );
            KeywordRow {
                postgres_class: postgres_class.get(spelling.as_str()).copied(),
                postgres_as_label: as_label.contains(spelling.as_str()),
                mysql_class: mysql_class.get(spelling.as_str()).copied(),
                variant,
                spelling,
            }
        })
        .collect();
    rows
}

/// The keyword inventory as `(canonical lower-case spelling, Rust variant name)`
/// pairs, in the same order as the generated `Keyword::ALL`.
///
/// Exposed for the bench-only ADR-0004 `phf` comparison: it builds an alternative
/// perfect-hash lookup over the exact same source-backed inventory the generated
/// `lookup_keyword` uses, so the two are measured on identical data rather than a
/// re-keyed re-implementation.
pub(crate) fn inventory_pairs() -> Vec<(String, String)> {
    load_inventory()
        .into_iter()
        .map(|row| (row.spelling, row.variant))
        .collect()
}

/// Render the full contents of `dialect/keyword/generated.rs`.
pub(crate) fn render() -> String {
    let rows = load_inventory();
    let count = rows.len();
    let max_len = rows
        .iter()
        .map(|row| row.spelling.len())
        .max()
        .expect("the keyword inventory is non-empty");

    let mut out = crate::license_header::block(crate::license_header::Comment::Slash);
    out.push_str(HEADER);

    // --- enum ---
    out.push_str("/// A recognized SQL keyword in canonical, case-insensitive form.\n");
    out.push_str("#[repr(u16)]\n");
    out.push_str("#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]\n");
    out.push_str("pub enum Keyword {\n");
    for row in &rows {
        let _ = writeln!(
            out,
            "    /// The `{}` keyword.",
            row.spelling.to_uppercase()
        );
        let _ = writeln!(out, "    {},", row.variant);
    }
    out.push_str("}\n\n");

    // --- ALL ---
    out.push_str("impl Keyword {\n");
    let _ = writeln!(
        out,
        "    /// Every keyword in fixed discriminant order; `Keyword::ALL[d] as usize == d`.",
    );
    let _ = writeln!(out, "    pub const ALL: [Self; {count}] = [");
    for row in &rows {
        let _ = writeln!(out, "        Self::{},", row.variant);
    }
    out.push_str("    ];\n\n");

    // --- as_str ---
    out.push_str("    /// Canonical lower-case spelling, used for pre-interned keyword symbols.\n");
    out.push_str("    pub const fn as_str(self) -> &'static str {\n");
    out.push_str("        match self {\n");
    for row in &rows {
        let _ = writeln!(
            out,
            "            Self::{} => {:?},",
            row.variant, row.spelling
        );
    }
    out.push_str("        }\n    }\n}\n\n");

    // --- lookup ---
    render_lookup(&mut out, &rows, max_len);

    // --- per-position reservation bitsets ---
    //
    // The reject-relevant PostgreSQL `kwlist.h` classes (col_name, type_func_name,
    // reserved) plus the independent AS_LABEL axis; `dialect/keyword.rs` composes
    // these into the per-position gates (a ColId rejects type_func_name ∪ reserved, a
    // type name rejects col_name ∪ reserved, a bare label rejects AS_LABEL, …).
    render_reserved(
        &mut out,
        "POSTGRES_COL_NAME_KEYWORDS",
        "PostgreSQL `COL_NAME_KEYWORD` class from kwlist.h — usable as a column \
         name/ColId but not as a bare type or function name (those have dedicated \
         grammar productions in PostgreSQL).",
        rows.iter()
            .filter(|row| row.postgres_class == Some(PgClass::ColName)),
    );
    render_reserved(
        &mut out,
        "POSTGRES_TYPE_FUNC_NAME_KEYWORDS",
        "PostgreSQL `TYPE_FUNC_NAME_KEYWORD` class from kwlist.h — usable as a \
         type or function name but not as a bare ColId (it would be ambiguous in \
         the FROM clause, e.g. `JOIN`, `LEFT`).",
        rows.iter()
            .filter(|row| row.postgres_class == Some(PgClass::TypeFuncName)),
    );
    render_reserved(
        &mut out,
        "POSTGRES_RESERVED_KEYWORDS",
        "PostgreSQL `RESERVED_KEYWORD` class from kwlist.h — never a plain \
         identifier, though still admissible as an `AS` label (ColLabel).",
        rows.iter()
            .filter(|row| row.postgres_class == Some(PgClass::Reserved)),
    );
    render_reserved(
        &mut out,
        "POSTGRES_AS_LABEL_KEYWORDS",
        "PostgreSQL `AS_LABEL` keywords from kwlist.h — the bare-label reject set: \
         these cannot be a column alias without `AS` (`SELECT a <kw>`), so `OVER` \
         and `FILTER` reject as bare aliases while `SELECT` accepts.",
        rows.iter().filter(|row| row.postgres_as_label),
    );

    // --- MySQL per-position reservation bitsets ---
    //
    // MySQL's reserved-word set (mysql_keywords.csv, from the MySQL 8.0 manual)
    // differs from the shared ANSI/PostgreSQL one in both directions, so it gets its
    // own bitsets. `dialect/keyword.rs` composes them into the four MySQL gates the
    // same way it composes the PostgreSQL categories: a ColId rejects
    // `type_func_name ∪ reserved`, a function name rejects only `reserved`.
    render_reserved(
        &mut out,
        "MYSQL_RESERVED_KEYWORDS",
        "MySQL 8.0 fully-reserved words (mysql_keywords.csv) — never a plain \
         identifier: rejected as a column, function, type, and bare-alias name.",
        rows.iter()
            .filter(|row| row.mysql_class == Some(MysqlClass::Reserved)),
    );
    render_reserved(
        &mut out,
        "MYSQL_TYPE_FUNC_NAME_KEYWORDS",
        "MySQL 8.0 reserved words that are built-in function names — reserved as a \
         column/type/bare-alias name but admissible as a function name, because \
         MySQL parses `kw(...)` as a call (e.g. `LEFT`, `IF`, `MOD`). Mirrors the \
         PostgreSQL `type_func_name` carve-out.",
        rows.iter()
            .filter(|row| row.mysql_class == Some(MysqlClass::TypeFuncName)),
    );
    render_reserved(
        &mut out,
        "MYSQL_FUNCTION_ONLY_KEYWORDS",
        "MySQL 8.4 words reserved *only* as a function name — the inverse of \
         `type_func_name`: admissible as a column/type/bare-alias name but rejected \
         as a call head, because MySQL parses the bare word as an identifier yet \
         syntax-rejects `kw(...)` (only `array`, engine-verified on 8.4.10). \
         `dialect/keyword.rs` unions it into the MySQL function-name reject set alone.",
        rows.iter()
            .filter(|row| row.mysql_class == Some(MysqlClass::FunctionOnly)),
    );

    out
}

/// Width thresholds for the packed-integer keyword key: a word matches only a
/// keyword of its own length, and a keyword of `<= 8` bytes packs into a `u64`,
/// `<= 16` into a `u128`. Wider keywords (a handful, never the hot path) keep the
/// byte-slice search.
const U64_MAX_LEN: usize = 8;
const U128_MAX_LEN: usize = 16;

/// Emit the allocation-free, case-insensitive `lookup_keyword`.
///
/// Recognition is length-bucketed (a word can only match a keyword of its own
/// length, ADR-0004). Within a bucket the lower-cased bytes are compared as a
/// *packed integer* rather than a byte slice: keywords are short, so a word of
/// `<= 8` bytes packs big-endian into a `u64` and `<= 16` into a `u128`, and the
/// bucket is binary-searched on that integer. A release-profile `samply` run put
/// the byte-slice `memcmp` at ~9% of parse CPU (keyword recognition ~13% total) —
/// every word token, overwhelmingly identifiers that *miss*, paid a per-step byte
/// compare; integer keys turn each step into one register compare. Big-endian
/// packing keeps the integer order equal to the byte-lexicographic order the
/// buckets are already sorted in, so the tables stay alphabetical (clean diffs)
/// and remain valid binary-search targets. No allocation, dep-free.
fn render_lookup(out: &mut String, rows: &[KeywordRow], max_len: usize) {
    use std::collections::BTreeMap;

    let mut by_len: BTreeMap<usize, Vec<&KeywordRow>> = BTreeMap::new();
    for row in rows {
        by_len.entry(row.spelling.len()).or_default().push(row);
    }
    let needs_u128 = by_len
        .keys()
        .any(|&len| (U64_MAX_LEN + 1..=U128_MAX_LEN).contains(&len));
    let needs_bytes = by_len.keys().any(|&len| len > U128_MAX_LEN);

    // --- lookup_keyword: length dispatch, packing per width ---
    out.push_str(
        "/// Case-insensitive, allocation-free keyword lookup over a borrowed source word.\n\
         ///\n\
         /// Length-bucketed (a word can only match a keyword of its own length); within a\n\
         /// bucket the lower-cased bytes are compared as a packed integer, so each\n\
         /// binary-search step is one register compare rather than a `memcmp` (the\n\
         /// byte-slice compare was ~9% of parse CPU on a release profile).\n\
         pub fn lookup_keyword(word: &str) -> Option<Keyword> {\n\
        \x20   let bytes = word.as_bytes();\n\
        \x20   match bytes.len() {\n",
    );
    for &len in by_len.keys() {
        if len <= U64_MAX_LEN {
            let _ = writeln!(
                out,
                "        {len} => search(KEYWORDS_LEN_{len}, pack_u64(bytes)),"
            );
        } else if len <= U128_MAX_LEN {
            let _ = writeln!(
                out,
                "        {len} => search(KEYWORDS_LEN_{len}, pack_u128(bytes)),"
            );
        } else {
            let _ = writeln!(
                out,
                "        {len} => search_bytes(KEYWORDS_LEN_{len}, bytes),"
            );
        }
    }
    out.push_str(
        "        _ => None,\n\
        \x20   }\n\
         }\n\n",
    );

    // --- pack_u64 / pack_u128: big-endian, ASCII-lower-cased ---
    out.push_str(
        "/// Pack up to eight lower-cased bytes big-endian into a `u64` lookup key.\n\
         ///\n\
         /// Big-endian so the integer order matches the byte-lexicographic order each\n\
         /// length bucket is sorted in, keeping the table a valid binary-search target.\n\
         /// Only `to_ascii_lowercase` folds case (A-Z only), so `_` and digits pass\n\
         /// through unchanged — the exact case-folding the byte-slice search used.\n\
         #[inline]\n\
         fn pack_u64(bytes: &[u8]) -> u64 {\n\
        \x20   let mut key = 0u64;\n\
        \x20   for &byte in bytes {\n\
        \x20       key = (key << 8) | byte.to_ascii_lowercase() as u64;\n\
        \x20   }\n\
        \x20   key\n\
         }\n\n",
    );
    if needs_u128 {
        out.push_str(
            "/// Pack up to sixteen lower-cased bytes big-endian into a `u128` lookup key —\n\
             /// the `u64` key's wider sibling, for keywords of 9..=16 bytes.\n\
             #[inline]\n\
             fn pack_u128(bytes: &[u8]) -> u128 {\n\
            \x20   let mut key = 0u128;\n\
            \x20   for &byte in bytes {\n\
            \x20       key = (key << 8) | byte.to_ascii_lowercase() as u128;\n\
            \x20   }\n\
            \x20   key\n\
             }\n\n",
        );
    }
    out.push_str(
        "/// Binary-search a same-length bucket sorted ascending by its packed-integer\n\
         /// key (`u64` or `u128`).\n\
         #[inline]\n\
         fn search<Key: Copy + Ord>(table: &[(Key, Keyword)], key: Key) -> Option<Keyword> {\n\
        \x20   table\n\
        \x20       .binary_search_by_key(&key, |&(packed, _)| packed)\n\
        \x20       .ok()\n\
        \x20       .map(|index| table[index].1)\n\
         }\n\n",
    );

    // --- byte-slice fallback for the few keywords wider than a u128 ---
    if needs_bytes {
        let _ = writeln!(
            out,
            "/// Longest keyword spelling; sizes the lower-casing buffer the byte-slice\n\
             /// fallback (keywords longer than sixteen bytes) lowers into.",
        );
        let _ = writeln!(out, "const MAX_KEYWORD_LEN: usize = {max_len};\n");
        out.push_str(
            "/// Byte-slice search for buckets too wide to pack into a `u128` (> 16 bytes):\n\
             /// lower-case into a fixed buffer, then binary-search the sorted byte tables.\n\
             fn search_bytes(table: &[(&[u8], Keyword)], word: &[u8]) -> Option<Keyword> {\n\
            \x20   let mut lowered = [0u8; MAX_KEYWORD_LEN];\n\
            \x20   for (slot, byte) in lowered.iter_mut().zip(word) {\n\
            \x20       *slot = byte.to_ascii_lowercase();\n\
            \x20   }\n\
            \x20   let key = &lowered[..word.len()];\n\
            \x20   table\n\
            \x20       .binary_search_by(|(candidate, _)| (*candidate).cmp(key))\n\
            \x20       .ok()\n\
            \x20       .map(|index| table[index].1)\n\
             }\n\n",
        );
    }

    // --- per-length bucket tables ---
    for (len, mut bucket) in by_len {
        bucket.sort_by(|a, b| a.spelling.cmp(&b.spelling));
        // Width is `len * 2` hex digits: the exact byte count of every key in the
        // bucket, zero-padded so the column aligns and shows leading-zero bytes.
        let hex_width = len * 2;
        if len <= U64_MAX_LEN {
            let _ = writeln!(out, "const KEYWORDS_LEN_{len}: &[(u64, Keyword)] = &[");
            for row in bucket {
                let packed = pack_be_u64(&row.spelling);
                let _ = writeln!(
                    out,
                    "    (0x{packed:0hex_width$x}, Keyword::{}), // {}",
                    row.variant, row.spelling,
                );
            }
        } else if len <= U128_MAX_LEN {
            let _ = writeln!(out, "const KEYWORDS_LEN_{len}: &[(u128, Keyword)] = &[");
            for row in bucket {
                let packed = pack_be_u128(&row.spelling);
                let _ = writeln!(
                    out,
                    "    (0x{packed:0hex_width$x}, Keyword::{}), // {}",
                    row.variant, row.spelling,
                );
            }
        } else {
            let _ = writeln!(out, "const KEYWORDS_LEN_{len}: &[(&[u8], Keyword)] = &[");
            for row in bucket {
                let _ = writeln!(out, "    (b{:?}, Keyword::{}),", row.spelling, row.variant);
            }
        }
        out.push_str("];\n\n");
    }
}

/// Pack a keyword spelling (`<= 8` bytes) big-endian into its `u64` lookup key,
/// the compile-time mirror of the runtime `pack_u64` the emitted code calls.
fn pack_be_u64(spelling: &str) -> u64 {
    let mut key = 0u64;
    for &byte in spelling.as_bytes() {
        key = (key << 8) | byte.to_ascii_lowercase() as u64;
    }
    key
}

/// Pack a keyword spelling (`9..=16` bytes) big-endian into its `u128` lookup key.
fn pack_be_u128(spelling: &str) -> u128 {
    let mut key = 0u128;
    for &byte in spelling.as_bytes() {
        key = (key << 8) | byte.to_ascii_lowercase() as u128;
    }
    key
}

/// Emit a per-dialect reserved bitset const built from the reserved keywords.
fn render_reserved<'a>(
    out: &mut String,
    name: &str,
    doc: &str,
    reserved: impl Iterator<Item = &'a KeywordRow>,
) {
    let _ = writeln!(out, "/// {doc}");
    let _ = writeln!(
        out,
        "pub const {name}: super::KeywordSet = super::KeywordSet::from_keywords(&[",
    );
    for row in reserved {
        let _ = writeln!(out, "    Keyword::{},", row.variant);
    }
    out.push_str("]);\n\n");
}

const HEADER: &str = "\
//! @generated by the `squonk-sourcegen` xtask - do not edit by hand.
//!
//! Keyword inventory, lookup, and per-position reservation bitsets,
//! generated from the checked-in objective source data in
//! `crates/squonk-sourcegen/keyword_data/`. Regenerate with:
//! `cargo run -p squonk-sourcegen`.

#![allow(clippy::all)]

";
