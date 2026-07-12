// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! SQL type-name AST nodes shared by casts, DDL, parameters, and temporal syntax.

use super::{Extension, Ident, Literal, NoExt, ObjectName};
use crate::vocab::Meta;
use thin_vec::ThinVec;

/// SQL data types in the M1 AST surface.
///
/// Variants intentionally keep spelling tags where common dialects spell the
/// same canonical type differently (`INT` vs `INTEGER`, `VARCHAR` vs
/// `CHARACTER VARYING`, `BOOL` vs `BOOLEAN`). Tier-1 rendering can therefore
/// round-trip the parsed surface, while dialect-target rendering can normalize
/// to a target spelling.
///
/// Dialect-only type names live in the same closed enum (a type is one
/// canonical shape, not a per-dialect tree). Their *recognition* is gated
/// by [`TypeNameSyntax`](crate::dialect::TypeNameSyntax) data, so a name like
/// `TINYINT` resolves to its built-in variant only under a dialect that opts in;
/// elsewhere the same word falls through to [`UserDefined`](Self::UserDefined).
///
/// The [`Other(X)`](Self::Other) seam (ADR-0009) lets an out-of-tree consumer carry a
/// host-owned type node produced by the parser crate's `Dialect::parse_data_type_hook`
/// without spelling it as a stock variant or forcing it into
/// [`UserDefined`](Self::UserDefined). Stock builtins use `X = NoExt` (uninhabited), so
/// the arm is statically dead and `DataType` (= `DataType<NoExt>`) keeps its node size.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum DataType<X: Extension = NoExt> {
    /// A boolean type (`BOOLEAN`/`BOOL`).
    Boolean {
        /// Exact source spelling retained for faithful rendering.
        spelling: BooleanTypeName,
        /// Source location and node identity.
        meta: Meta,
    },
    /// MySQL `TINYINT` (1-byte integer); recognized only under
    /// [`TypeNameSyntax::extended_scalar_type_names`](crate::dialect::TypeNameSyntax).
    TinyInt {
        /// Optional integer display width `(M)`, e.g. `TINYINT(1)`. This is *display
        /// metadata* — it governs the left-pad width under `ZEROFILL` — never the
        /// stored value's precision or range (`INT(1)` still stores the full 32-bit
        /// range). Canonically MySQL's (`INT(11)`-style, deprecated in 8.0.17+ but
        /// ubiquitous in dumps); SQLite accepts it through affinity type-name
        /// absorption. Gated by
        /// [`TypeNameSyntax::integer_display_width`](crate::dialect::TypeNameSyntax),
        /// so ANSI/PostgreSQL reject the parenthesized form on a built-in integer.
        display_width: Option<u32>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `SMALLINT` (2-byte integer).
    SmallInt {
        /// Optional display width for this syntax.
        display_width: Option<u32>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// MySQL `MEDIUMINT` (3-byte integer); recognized only under
    /// [`TypeNameSyntax::extended_scalar_type_names`](crate::dialect::TypeNameSyntax).
    MediumInt {
        /// Optional display width for this syntax.
        display_width: Option<u32>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// An `INT`/`INTEGER` (4-byte integer).
    Integer {
        /// Exact source spelling retained for faithful rendering.
        spelling: IntegerTypeName,
        /// Optional display width for this syntax.
        display_width: Option<u32>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `BIGINT` (8-byte integer).
    BigInt {
        /// Optional display width for this syntax.
        display_width: Option<u32>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// An exact `DECIMAL`/`NUMERIC(p, s)` type.
    Decimal {
        /// Exact source spelling retained for faithful rendering.
        spelling: DecimalTypeName,
        /// The precision modifier. Signed because PostgreSQL parses the `numeric`/`decimal`
        /// type-modifier arguments as a general expression list at raw-parse time, so a signed
        /// modifier (`numeric(-3, 6)`) is accepted and validated only later — the sign is gated
        /// for acceptance by
        /// [`TypeNameSyntax::signed_type_modifier`](crate::dialect::TypeNameSyntax); off
        /// elsewhere a leading `-` is a clean parse error.
        precision: Option<i32>,
        /// The scale modifier. Signed for the same reason as [`precision`](Self::Decimal::precision):
        /// PostgreSQL accepts a negative scale (`numeric(5, -2)` rounds to tens), which the
        /// standard/MySQL reject.
        scale: Option<i32>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `FLOAT[(p)]` approximate numeric type.
    Float {
        /// Numeric precision specified by this syntax.
        precision: Option<u32>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `REAL` (single-precision floating-point) type.
    Real {
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `DOUBLE PRECISION` (double-precision floating-point) type.
    Double {
        /// Exact source spelling retained for faithful rendering.
        spelling: DoubleTypeName,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A variable-length character LOB. The spelling tag carries the MySQL size
    /// family (`TINYTEXT`/`MEDIUMTEXT`/`LONGTEXT`), which differ only in declared
    /// maximum length; the bare PostgreSQL/MySQL `TEXT` is the default spelling.
    Text {
        /// Exact source spelling retained for faithful rendering.
        spelling: TextTypeName,
        /// MySQL's character-set type annotation — the whole TEXT size family admits it in
        /// column position (engine-measured on mysql:8.4: `TEXT CHARACTER SET utf8mb4`,
        /// `TINYTEXT ASCII`, `LONGTEXT BINARY`, `TEXT BYTE` all prepare). See the field
        /// docs on [`Character`](Self::Character); `None` outside MySQL.
        charset: Option<Box<CharsetAnnotation>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A binary LOB family: MySQL `BLOB`/`TINYBLOB`/`MEDIUMBLOB`/`LONGBLOB`. The
    /// binary analog of [`Text`](Self::Text); recognized only under
    /// [`TypeNameSyntax::extended_scalar_type_names`](crate::dialect::TypeNameSyntax).
    Blob {
        /// Exact source spelling retained for faithful rendering.
        spelling: BlobTypeName,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A fixed/variable-length character type (`CHAR`/`VARCHAR`/`CHARACTER`/…).
    Character {
        /// Exact source spelling retained for faithful rendering.
        spelling: CharacterTypeName,
        /// Optional size for this syntax.
        size: Option<u32>,
        /// MySQL's character-set type annotation — the grammar's
        /// `opt_charset_with_opt_binary` production: `CHARACTER SET <name>` (or the
        /// `CHARSET` synonym), the `ASCII`/`UNICODE`/`BYTE` shortcuts, and/or the trailing
        /// `BINARY` binary-collation modifier. Part of the *type*, not a column attribute:
        /// it must directly follow the type (and its length) and is rejected once any
        /// column attribute intervenes (`CHAR(5) NOT NULL CHARACTER SET x` is an
        /// `ER_PARSE_ERROR` on mysql:8), which is why it lives on the type node — unlike
        /// `COLLATE`, a free-floating column attribute. Recognized only under
        /// [`TypeNameSyntax::character_set_annotation`](crate::dialect::TypeNameSyntax),
        /// and only on the non-national spellings (`CHAR`/`CHARACTER`/`VARCHAR`): the
        /// national forms (`NCHAR`/`NATIONAL CHAR`) fix their own charset and reject the
        /// annotation, so they always carry `None`. `None` for every non-MySQL char type.
        ///
        /// Boxed: the annotation is a rare, fat payload (MySQL-only, and only on annotated
        /// char types), so ADR-0007 keeps it off the hot [`DataType`] node's inline width —
        /// the box is paid only on the rare annotated path.
        charset: Option<Box<CharsetAnnotation>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A fixed/variable-length binary type (`BINARY`/`VARBINARY`).
    Binary {
        /// Exact source spelling retained for faithful rendering.
        spelling: BinaryTypeName,
        /// Optional size for this syntax.
        size: Option<u32>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A bit-string type: `BIT [VARYING] [(n)]` (PostgreSQL `bit`/`varbit`).
    Bit {
        /// `BIT VARYING` (variable-length) vs fixed-length `BIT`.
        varying: bool,
        /// Optional size for this syntax.
        size: Option<u32>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// The PostgreSQL `JSON` type (distinct from the `jsonb` user-defined name).
    Json {
        /// Source location and node identity.
        meta: Meta,
    },
    /// The `UUID` type (PostgreSQL `uuid`, DuckDB `UUID`). A single-keyword scalar
    /// carrying no parameters, shaped exactly like [`Json`](Self::Json). Recognition is
    /// *ungated* — the word resolves to this canonical variant wherever a type name is
    /// admitted, so a type planner reads one UUID identity regardless of dialect rather
    /// than a per-dialect [`UserDefined`](Self::UserDefined) name. This is *identity*, not
    /// *acceptance*: dialects that reject `UUID` in a given position still reject it there
    /// (e.g. MySQL's narrow `CAST` target set excludes `UUID`), the same way `JSON` is a
    /// first-class variant yet remains subject to each dialect's positional rules.
    Uuid {
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `DATE` type.
    Date {
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `TIME [WITH|WITHOUT TIME ZONE]` type.
    Time {
        /// Exact source spelling retained for faithful rendering.
        spelling: TimeTypeName,
        /// Numeric precision specified by this syntax.
        precision: Option<u32>,
        /// The `WITH`/`WITHOUT TIME ZONE` qualifier; see [`TimeZone`].
        time_zone: TimeZone,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `TIMESTAMP [WITH|WITHOUT TIME ZONE]` type.
    Timestamp {
        /// Exact source spelling retained for faithful rendering.
        spelling: TimestampTypeName,
        /// Numeric precision specified by this syntax.
        precision: Option<u32>,
        /// The `WITH`/`WITHOUT TIME ZONE` qualifier; see [`TimeZone`].
        time_zone: TimeZone,
        /// Source location and node identity.
        meta: Meta,
    },
    /// An `INTERVAL` type, with an optional field qualifier and precision.
    Interval {
        /// Optional fields for this syntax.
        fields: Option<IntervalFields>,
        /// Numeric precision specified by this syntax.
        precision: Option<u32>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `ENUM('a', 'b', ...)`: a string type constrained to a value list — MySQL's column
    /// type and DuckDB's `x::ENUM('a', 'b')` cast target. Structural (it carries the value
    /// list), so it is one canonical variant rather than a spelling tag; recognized under
    /// [`TypeNameSyntax::enum_type`](crate::dialect::TypeNameSyntax). The values are string
    /// [`Literal`]s whose spelling round-trips from their span.
    Enum {
        /// Values in source order.
        values: ThinVec<Literal>,
        /// MySQL's character-set type annotation, which `ENUM` admits in column position
        /// (engine-measured on mysql:8.4: `ENUM('a') CHARACTER SET utf8mb4` / `ASCII` /
        /// `BYTE` / `BINARY` all prepare). See the field docs on
        /// [`Character`](Self::Character); always `None` for DuckDB (whose grammar has no
        /// such annotation) and outside MySQL.
        charset: Option<Box<CharsetAnnotation>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// MySQL `SET('a', 'b', ...)`: a string type whose value is a subset of the
    /// listed members, recognized under
    /// [`TypeNameSyntax::set_type`](crate::dialect::TypeNameSyntax). Shares
    /// [`Enum`](Self::Enum)'s value-list shape; the two stay distinct variants because the
    /// membership semantics differ.
    Set {
        /// Values in source order.
        values: ThinVec<Literal>,
        /// MySQL's character-set type annotation — same surface as [`Enum`](Self::Enum)
        /// (engine-measured on mysql:8.4).
        charset: Option<Box<CharsetAnnotation>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A numeric type carrying MySQL's `SIGNED`/`UNSIGNED`/`ZEROFILL` modifiers,
    /// gated by [`TypeNameSyntax::numeric_modifiers`](crate::dialect::TypeNameSyntax).
    ///
    /// One wrapper models the modifier for every numeric inner type, so the flag is
    /// kept once rather than duplicated onto each numeric variant — the
    /// same wrap-the-inner-type shape as [`Array`](Self::Array). `element` is
    /// optional because MySQL's `CAST(x AS UNSIGNED)` / `CAST(x AS SIGNED)` use the
    /// modifier as a standalone integer cast target that names no base type.
    NumericModifier {
        /// Optional element for this syntax.
        element: Option<Box<DataType<X>>>,
        /// The `SIGNED`/`UNSIGNED` modifier; see [`Signedness`].
        signedness: Signedness,
        /// Whether the zerofill form was present in the source.
        zerofill: bool,
        /// Source location and node identity.
        meta: Meta,
    },
    /// An array type (`T[]`, `T[n]`, or `T ARRAY[n]`).
    Array {
        /// The array element type.
        element: Box<DataType<X>>,
        /// Fixed cardinality for the DuckDB fixed-size `ARRAY` (`INTEGER[3]` /
        /// `INTEGER ARRAY[3]`), or `None` for the variable-length list (`INTEGER[]` /
        /// `INTEGER ARRAY`). The two are distinct DuckDB types (`ARRAY` enforces the
        /// count, `LIST` is unbounded); PostgreSQL accepts the bound syntactically but
        /// ignores it (`int[3]` binds identically to `int[]`), so the value is retained
        /// for round-trip regardless of dialect and its enforcement is a consumer concern.
        size: Option<u32>,
        /// Bracket `T[]`/`T[n]` vs keyword `T ARRAY`/`T ARRAY[n]` surface. One canonical
        /// array-type shape covers both spellings; the tag round-trips the
        /// written form.
        spelling: ArrayTypeSpelling,
        /// Source location and node identity.
        meta: Meta,
    },
    /// An anonymous composite (record) type: DuckDB `STRUCT(a INTEGER, b VARCHAR)` or
    /// the standard `ROW(...)` spelling of the same shape. One canonical named-field
    /// list covers both spellings; `spelling` records the written keyword.
    /// Recognized only under
    /// [`TypeNameSyntax::composite_types`](crate::dialect::TypeNameSyntax); ANSI/
    /// PostgreSQL reject the anonymous form (PostgreSQL has only *named* composite
    /// types and spells `ROW` as a value constructor, never a type).
    Struct {
        /// fields in source order.
        fields: ThinVec<StructTypeField<X>>,
        /// Exact source spelling retained for faithful rendering.
        spelling: StructTypeSpelling,
        /// Source location and node identity.
        meta: Meta,
    },
    /// An anonymous tagged-union type: DuckDB `UNION(tag1 T1, tag2 T2, ...)`. Shares the
    /// named-member shape of [`Struct`](Self::Struct) but stays a distinct variant
    /// because the semantics differ (a tagged sum type vs a product type), exactly as
    /// [`Enum`](Self::Enum)/[`Set`](Self::Set) share a value-list shape yet stay
    /// distinct. Gated by the same `composite_types` flag.
    Union {
        /// members in source order.
        members: ThinVec<StructTypeField<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// An anonymous map type: DuckDB `MAP(K, V)`, where `K` and `V` are themselves
    /// types (`MAP(VARCHAR, STRUCT(x INTEGER))`, `MAP(INTEGER[], VARCHAR)`). DuckDB
    /// desugars a map to a `LIST` of `STRUCT(key, value)` in its serialized tree; the
    /// canonical shape keeps the written key/value types directly (the desugaring is a
    /// representation-equivalent difference the structural oracle normalizes, like the
    /// `list_value`/`struct_pack` value desugars). Gated by `composite_types`.
    Map {
        /// The map's key type.
        key: Box<DataType<X>>,
        /// Value supplied by this syntax.
        value: Box<DataType<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A ClickHouse parametric type combinator wrapping exactly one inner type:
    /// `Nullable(T)` (and the sibling `LowCardinality(T)`, which shares this exact
    /// single-inner-type shape). One canonical variant with a [`WrappedTypeKind`] axis
    /// models the whole wrapper-shaped family — the same wrap-one-inner-type idiom as
    /// [`NumericModifier`](Self::NumericModifier), which keeps its modifier flag once
    /// rather than duplicating a variant per numeric inner type — so a further
    /// wrapper keyword is a new [`WrappedTypeKind`] arm plus a parser branch, never a
    /// new `DataType` variant or `Render` arm.
    ///
    /// `inner` is a full, recursively-nested type, so `Nullable(DECIMAL(10, 2))` and
    /// `Nullable(String)[]` both parse. ClickHouse constrains *composability* only at
    /// bind time — it rejects `Nullable(Nullable(T))` and `Nullable(Array(T))` with a
    /// type-resolution `DB::Exception`, not a grammar error — so those nestings
    /// parse-accept here and the constraint is a binder concern, the same parse-vs-bind
    /// split as ragged array literals.
    Wrapped {
        /// Which wrapper (`Nullable`/`LowCardinality`); see [`WrappedTypeKind`].
        kind: WrappedTypeKind,
        /// The wrapped inner type.
        inner: Box<DataType<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// ClickHouse `FixedString(N)`: a fixed-length byte string of exactly `N` bytes.
    ///
    /// Deliberately its own variant rather than a [`WrappedTypeKind`](Self::Wrapped) arm:
    /// its argument is a scalar length `N`, not a nested inner type, so it does not share
    /// the single-inner-type wrapper shape. It instead reuses the length-carrying string
    /// idiom of [`Character`](Self::Character)/[`Binary`](Self::Binary), but with the
    /// length as a **non-optional** `u32` rather than `Option<u32>`: ClickHouse's grammar
    /// makes `N` mandatory (a bare `FixedString` with no parens is a different, invalid
    /// spelling that falls through to the user-defined-type path), so the required argument
    /// is encoded in the type itself instead of a runtime `None` those spelling-tagged
    /// variants must tolerate. That mandatory-scalar shape is why it is not folded onto
    /// [`Character`](Self::Character) under a spelling tag — and `FixedString` is a *byte*
    /// string carrying no charset annotation, unlike the character variants.
    ///
    /// `N` is a positive integer in ClickHouse; the grammar admits any `u32` literal (`0`
    /// included) and ClickHouse rejects `FixedString(0)` only at type resolution, the same
    /// parse-vs-bind split as the `Wrapped` composability rejects. Recognized only under
    /// [`TypeNameSyntax::fixed_string_type`](crate::dialect::TypeNameSyntax); the canonical
    /// render emits ClickHouse's mixed-case spelling (`FixedString`, never `FIXEDSTRING`).
    FixedString {
        /// Length bound specified by this syntax.
        length: u32,
        /// Source location and node identity.
        meta: Meta,
    },
    /// ClickHouse `DateTime64(P[, 'timezone'])`: a sub-second timestamp with `P` fractional
    /// digits of precision and an optional IANA time-zone name.
    ///
    /// Its own variant rather than a spelling tag on [`Timestamp`](Self::Timestamp), for two
    /// shape reasons that mirror [`FixedString`](Self::FixedString)'s mandatory-scalar
    /// argument. First, precision is **mandatory**: ClickHouse's `DateTime64` has no bare
    /// spelling (the `(P)` is required; a bare `DateTime64` with no parens is a different
    /// spelling that falls through to the user-defined-type path), so `precision` is a
    /// non-optional `u32` rather than the `Option<u32>` that the ANSI/MySQL
    /// [`Timestamp`](Self::Timestamp)/[`Time`](Self::Time) variants carry. Second, its time
    /// zone is a single-quoted string-literal *argument*, not the ANSI `WITH TIME ZONE`
    /// flag those variants encode as a [`TimeZone`] tag — a different surface that would not
    /// fit the tag. Those two differences are why it is not folded onto `Timestamp`.
    ///
    /// `timezone` is the optional second argument, held as the source-spelled string
    /// [`Literal`] so its exact quoting round-trips (the [`Enum`](Self::Enum) value-member
    /// idiom). `P` is parsed as any `u32` literal; ClickHouse's documented `0..=9` range is a
    /// bind-time reject, not a grammar error — the same parse-vs-bind split as
    /// [`FixedString`](Self::FixedString)'s positive-length rule. Recognized only under
    /// [`TypeNameSyntax::datetime64_type`](crate::dialect::TypeNameSyntax); the canonical
    /// render emits ClickHouse's mixed-case spelling (`DateTime64`, never `DATETIME64`).
    ///
    /// The timezone-only sibling `DateTime('timezone')` (no precision, on the plain
    /// `DateTime` type) is a distinct surface and out of scope here — see the deferral on
    /// the owning ticket.
    DateTime64 {
        /// Numeric precision specified by this syntax.
        precision: u32,
        /// Optional timezone for this syntax.
        timezone: Option<Box<Literal>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// ClickHouse `Nested(name1 Type1, name2 Type2, ...)`: a named-field composite that
    /// models a repeated group — semantically a set of parallel same-length arrays sharing
    /// a common column prefix, not a single record value.
    ///
    /// Shares the named-field [`StructTypeField`] shape of [`Struct`](Self::Struct)/
    /// [`Union`](Self::Union) but stays a distinct variant because the semantics differ (a
    /// repeated nested structure vs a product or a tagged sum), exactly as
    /// [`Union`](Self::Union) shares `Struct`'s field shape yet stays distinct and
    /// [`Enum`](Self::Enum)/[`Set`](Self::Set) share a value-list shape yet stay distinct.
    /// A spelling tag on `Struct` would conflate those semantics and force the
    /// `composite_types` gate; `Nested` instead carries its own
    /// [`TypeNameSyntax::nested_type`](crate::dialect::TypeNameSyntax) flag
    /// (one behaviour = one flag) and its canonical render re-emits `Nested`, never `STRUCT`.
    ///
    /// A field type is a full, recursively-nested type, so `Nested(x Nested(y UInt8))`
    /// parses: ClickHouse permits arbitrary nesting levels (under `flatten_nested=0`), and
    /// any level limit is a setting/bind concern, not a grammar error — the same parse-vs-bind
    /// split as the [`Wrapped`](Self::Wrapped) composability rejects. At least one field is
    /// required (a bare `Nested()` is a syntax error), enforced by the one-or-more field list.
    Nested {
        /// fields in source order.
        fields: ThinVec<StructTypeField<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// ClickHouse's fixed-bit-width integer type names: the signed `Int8`/`Int16`/`Int32`/
    /// `Int64`/`Int128`/`Int256` family and their unsigned `UInt8`…`UInt256` siblings.
    ///
    /// One canonical variant carries the signedness ([`signed`](Self::FixedWidthInt::signed))
    /// and the width ([`IntWidth`]) as *data*, rather than a variant per width — the whole
    /// bit-width family travels together in a dialect exactly as MySQL's
    /// `TINYINT`/`MEDIUMINT`/… ride one recognition gate, so it is one behaviour and one
    /// [`DataType`] shape (the [`extended_scalar_type_names`](crate::dialect::TypeNameSyntax)
    /// precedent), gated by
    /// [`TypeNameSyntax::bit_width_integer_names`](crate::dialect::TypeNameSyntax). Signedness
    /// and width are the only axes, so the whole family is one `DataType` shape and one
    /// `Render` arm rather than twelve.
    ///
    /// Kept distinct from [`Integer`](Self::Integer)/[`BigInt`](Self::BigInt) rather than
    /// folded under a spelling tag: `Int32`/`Int64` name the same underlying types as `INT`
    /// and `BIGINT` but round-trip their own written surface, and the narrower/wider widths
    /// (`Int8`/`Int128`/`Int256`) have no ANSI integer variant at all. The names take no
    /// arguments, so a bare `Int256` off-gate is an ordinary user-defined type name (the
    /// trivial off-gate boundary, like a bare `Nullable`), never a parse error. The canonical
    /// render emits ClickHouse's mixed-case spelling (`Int256`/`UInt256`, never `INT256`).
    FixedWidthInt {
        /// `true` for the signed `Int*` spellings, `false` for the unsigned `UInt*`.
        signed: bool,
        /// The integer bit width (`Int8`…`Int256`); see [`IntWidth`].
        width: IntWidth,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A user-defined (or otherwise non-built-in) type reference — a dot-separated
    /// qualified [`ObjectName`] with an optional parenthesized modifier list. Every type
    /// name no typed variant claims lands here (the FALLBACK path), so a bare `GEOMETRY`,
    /// `citext`, or `schema.mytype` is a `UserDefined`.
    UserDefined {
        /// Name referenced by this syntax.
        name: ObjectName,
        /// The parenthesized modifier arguments, as constant [`Literal`]s whose spelling
        /// round-trips from their span. An unsigned-integer modifier (`FOO(3)`) parses
        /// under every dialect; a string-literal modifier (`GEOMETRY('OGC:CRS84')`) is
        /// DuckDB's, admitted only under
        /// [`TypeNameSyntax::string_type_modifiers`](crate::dialect::TypeNameSyntax).
        /// Empty when the name carries no parenthesized list.
        modifiers: ThinVec<Literal>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// SQLite's liberal affinity type name — a free run of one-or-more space-separated
    /// words (`UNSIGNED BIG INT`, `LONG INTEGER`, `NATIVE CHARACTER`, and even the
    /// misspelled `INTEGEB PRIMARI KEY`) with an optional one-or-two-argument parenthesized
    /// modifier (`VARCHAR(123,456)`, `FLOATING POINT(5,10)`). SQLite has no fixed type
    /// vocabulary — a column/cast type is any `ids ...` token run terminated by a
    /// column-constraint keyword, a comma, or a close paren — so a multi-word or two-argument
    /// name that no typed variant can faithfully hold lands here instead of over-rejecting.
    /// Recognized only under
    /// [`TypeNameSyntax::liberal_type_names`](crate::dialect::TypeNameSyntax) (SQLite +
    /// Lenient).
    ///
    /// # Why its own variant, not [`UserDefined`](Self::UserDefined)
    ///
    /// `UserDefined` carries an [`ObjectName`] — a *dot*-separated qualified name
    /// (`schema.type`) that renders with `.` — and its `modifiers` are unsigned `u32`s. A
    /// liberal affinity name is a *space*-separated word run with a distinct render, so it is
    /// a genuinely different canonical shape (ADR-0011: one type = one shape), not a spelling
    /// of the same `UserDefined` reference. Folding it in would force a render branch and a
    /// separator tag onto the common single-word user-defined path and grow its hot node; a
    /// dedicated variant keeps `UserDefined` untouched. A bare single-word affinity name
    /// (`BANANA`) still parses to `UserDefined` — this variant is reached only when a second
    /// word or a two-argument paren list makes the typed / user-defined parse insufficient
    /// (the FALLBACK ordering: typed variants win wherever they can faithfully represent the
    /// input, so `INT`, `DOUBLE PRECISION`, `VARCHAR(255)`, `NATIONAL CHARACTER(15)` keep
    /// their typed variants under SQLite).
    ///
    /// # Boundary (engine-probed, rusqlite/sqlite3 3.53.2 & 3.43.2)
    ///
    /// The word run terminates at a column-constraint keyword — `PRIMARY`, `NOT`, `NULL`,
    /// `UNIQUE`, `CHECK`, `DEFAULT`, `COLLATE`, `REFERENCES`, `CONSTRAINT`, `AS`, `GENERATED`
    /// (each engine-probed as a terminator, e.g. `x MY PRIMARY` rejects because `PRIMARY`
    /// starts `PRIMARY KEY`, not the type) — or a comma / close paren; every other
    /// identifier-or-non-reserved-keyword word is absorbed (`x FOO BAR BAZ QUX` accepts). A
    /// reserved word (`SELECT`) can never be a type word (`x SELECT` rejects). The optional
    /// paren list holds at most two arguments (`FOO(1,2)` accepts, `FOO(1,2,3)` rejects).
    Liberal {
        /// The one-or-more space-separated affinity words, each an [`Ident`] whose quote
        /// style round-trips (a quoted `"foo bar"` word counts as one). Rendered
        /// space-separated.
        words: ThinVec<Ident>,
        /// The zero-, one-, or two-element parenthesized modifier list (`VARCHAR(123,456)`
        /// carries `[123, 456]`). Empty when the name has no parens. Unsigned like
        /// [`UserDefined::modifiers`](Self::UserDefined) — SQLite's grammar also admits a
        /// signed or fractional argument, but the corpus surface is unsigned and the
        /// signed/float forms are left to a follow-up (their absence under-accepts, never
        /// over-accepts).
        args: ThinVec<u32>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A host-owned type node an out-of-tree dialect parses through the parser crate's
    /// `Dialect::parse_data_type_hook` (ADR-0009). The sixth `Other(X)` seam:
    /// `X = NoExt` for every shipped builtin leaves this arm uninhabited and statically
    /// dead, so stock parsing never produces it and `DataType<NoExt>` keeps its
    /// byte-identical size.
    Other {
        /// The dialect extension node value.
        ext: X,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// One named field of an anonymous composite type: `name TYPE`, as in a DuckDB
/// `STRUCT(a INTEGER)` field or a `UNION(tag T)` member.
///
/// The field name is a genuine identifier position (bare `a`, or double-quoted
/// `"key"`), so it is an [`Ident`] that round-trips its quote style, not a bare
/// symbol. Mirrors the Expr-side `StructField`: a spanned sub-node with its own
/// [`Meta`].
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct StructTypeField<X: Extension = NoExt> {
    /// Name referenced by this syntax.
    pub name: Ident,
    /// Data type named by this syntax.
    pub ty: DataType<X>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// How an anonymous composite type ([`DataType::Struct`]) was spelled.
///
/// DuckDB folds `STRUCT(...)` and the standard `ROW(...)` to the same canonical
/// composite type; this tag records the written keyword so each round-trips,
/// mirroring [`CastSyntax`](crate::ast::CastSyntax).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum StructTypeSpelling {
    /// The `STRUCT(...)` keyword.
    Struct,
    /// The standard `ROW(...)` keyword.
    Row,
    /// BigQuery angle-bracket type form `STRUCT<field TYPE, …>` (type position).
    AngleBracket,
}

/// The written surface of an array-type suffix ([`DataType::Array`]).
///
/// Bracket `T[]`/`T[n]` and keyword `T ARRAY`/`T ARRAY[n]` name the same canonical
/// array-type shape; the tag round-trips the written form.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ArrayTypeSpelling {
    /// The bracket suffix `T[]` / `T[n]`.
    Bracket,
    /// The `ARRAY` keyword suffix `T ARRAY` / `T ARRAY[n]`.
    Keyword,
    /// BigQuery angle-bracket type form `ARRAY<T>` (type position, prefix).
    AngleBracket,
}

/// The parametric wrapper keyword of a [`DataType::Wrapped`] type combinator.
///
/// Each variant is a distinct ClickHouse type combinator that shares the
/// single-inner-type wrapper shape; recognition of each is gated on its own
/// [`TypeNameSyntax`](crate::dialect::TypeNameSyntax) flag (one behaviour = one flag),
/// so a preset opts into each keyword independently rather than through one bundled
/// gate. The canonical render emits ClickHouse's mixed-case spelling (`Nullable`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum WrappedTypeKind {
    /// `Nullable(T)` — the inner type extended with a `NULL` value.
    Nullable,
    /// `LowCardinality(T)` — a dictionary-encoding wrapper over the inner type,
    /// transparent to query semantics (ClickHouse constrains which `T` at type
    /// resolution; the grammar accepts any single inner type).
    LowCardinality,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// The SQL boolean type name forms represented by the AST.
pub enum BooleanTypeName {
    /// The `BOOLEAN` spelling.
    Boolean,
    /// The `BOOL` spelling.
    Bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// The SQL integer type name forms represented by the AST.
pub enum IntegerTypeName {
    /// The `INT` spelling.
    Int,
    /// The `INTEGER` spelling.
    Integer,
}

/// The bit width spelled in a ClickHouse [`DataType::FixedWidthInt`] type name — the `N`
/// of `Int<N>`/`UInt<N>`. ClickHouse defines exactly these six widths; the width is a
/// closed axis (a precise enum, not a raw bit count) so only the real spellings are
/// representable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum IntWidth {
    /// An 8-bit integer width.
    W8,
    /// A 16-bit integer width.
    W16,
    /// A 32-bit integer width.
    W32,
    /// A 64-bit integer width.
    W64,
    /// A 128-bit integer width.
    W128,
    /// A 256-bit integer width.
    W256,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// The SQL decimal type name forms represented by the AST.
pub enum DecimalTypeName {
    /// The `DECIMAL` spelling.
    Decimal,
    /// The `DEC` spelling.
    Dec,
    /// The `NUMERIC` spelling.
    Numeric,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// The SQL double type name forms represented by the AST.
pub enum DoubleTypeName {
    /// The standard `DOUBLE PRECISION` spelling.
    DoublePrecision,
    /// MySQL's bare `DOUBLE` floating-point type (PostgreSQL leaves bare `double`
    /// unreserved, so it is recognized only under
    /// [`TypeNameSyntax::extended_scalar_type_names`](crate::dialect::TypeNameSyntax)).
    Double,
}

/// The MySQL character-LOB size family spelled by [`DataType::Text`]. They differ
/// only in declared maximum length, so they share one variant tagged by spelling.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum TextTypeName {
    /// The bare `TEXT` type (PostgreSQL; MySQL's mid-size member).
    Text,
    /// MySQL `TINYTEXT`.
    TinyText,
    /// MySQL `MEDIUMTEXT`.
    MediumText,
    /// MySQL `LONGTEXT`.
    LongText,
}

/// The MySQL binary-LOB size family spelled by [`DataType::Blob`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum BlobTypeName {
    /// MySQL `BLOB`.
    Blob,
    /// MySQL `TINYBLOB`.
    TinyBlob,
    /// MySQL `MEDIUMBLOB`.
    MediumBlob,
    /// MySQL `LONGBLOB`.
    LongBlob,
}

/// MySQL's numeric sign modifier, carried by [`DataType::NumericModifier`].
///
/// `ZEROFILL` (a separate flag on the wrapper) implies unsigned semantics in
/// MySQL, but the written sign is preserved here so the surface round-trips: a
/// bare `INT ZEROFILL` keeps [`Unspecified`](Self::Unspecified), not a synthesized
/// `UNSIGNED`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum Signedness {
    /// No `SIGNED`/`UNSIGNED` keyword was written.
    Unspecified,
    /// `SIGNED`.
    Signed,
    /// `UNSIGNED`.
    Unsigned,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// The SQL character type name forms represented by the AST.
pub enum CharacterTypeName {
    /// The fixed-length `CHAR` spelling.
    Char,
    /// The fixed-length `CHARACTER` spelling.
    Character,
    /// The variable-length `CHAR VARYING` spelling.
    CharVarying,
    /// The variable-length `CHARACTER VARYING` spelling.
    CharacterVarying,
    /// The variable-length `VARCHAR` spelling.
    Varchar,
    /// `NCHAR` (national character).
    Nchar,
    /// `NCHAR VARYING`.
    NcharVarying,
    /// `NATIONAL CHAR`.
    NationalChar,
    /// `NATIONAL CHAR VARYING`.
    NationalCharVarying,
    /// `NATIONAL CHARACTER`.
    NationalCharacter,
    /// `NATIONAL CHARACTER VARYING`.
    NationalCharacterVarying,
}

/// A MySQL character-set type annotation carried by the string-typed [`DataType`]
/// variants — [`Character`](DataType::Character), the [`Text`](DataType::Text) LOB family,
/// [`Enum`](DataType::Enum), and [`Set`](DataType::Set) (each engine-measured to admit it
/// on mysql:8.4) — the grammar's `opt_charset_with_opt_binary` production, part of the
/// *type* rather than a column attribute (see the field docs on
/// [`DataType::Character`]).
///
/// The canonical render emits the charset selector first, then `BINARY`
/// (`CHAR CHARACTER SET utf8mb4 BINARY`); MySQL's reversed spellings
/// (`CHAR BINARY CHARACTER SET utf8mb4`, `CHAR BINARY ASCII`) fold onto this one shape,
/// their exact written order recovered from the node span (an ADR-0011 spelling trade,
/// mirroring the plural-interval and `SIGNED INTEGER` folds). The `CHARSET` synonym
/// likewise folds to the canonical `CHARACTER SET`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct CharsetAnnotation {
    /// The character-set selector, or `None` when only the `BINARY` collation modifier was
    /// written (`CHAR BINARY`, with no charset). At least one of `charset` / `binary` is
    /// always present — an empty annotation is `None` on the [`DataType::Character`] node.
    pub charset: Option<Charset>,
    /// The charset name for [`Charset::Named`] (`CHARACTER SET <name>` / `CHARSET <name>`);
    /// `None` for the keyword shortcuts ([`Charset::Ascii`]/[`Charset::Unicode`]/
    /// [`Charset::Byte`]) and the bare-`BINARY` form. Held on this meta-bearing struct
    /// rather than inside the [`Charset`] enum so the tag stays a plain `Copy` kind. The
    /// name is a MySQL `ident_or_text`: a bare or backtick-quoted identifier, or a quoted
    /// string (`CHARACTER SET 'utf8mb4'`), whose spelling round-trips from the [`Ident`]'s
    /// quote style.
    pub name: Option<Ident>,
    /// Whether the `BINARY` binary-collation modifier accompanies the annotation
    /// (`CHAR BINARY`, `CHAR CHARACTER SET x BINARY`, `CHAR ASCII BINARY`).
    pub binary: bool,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The character-set selector kind of a [`CharsetAnnotation`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum Charset {
    /// `CHARACTER SET <name>` or the `CHARSET <name>` synonym (folded to the canonical
    /// `CHARACTER SET`). The name is carried by [`CharsetAnnotation::name`].
    Named,
    /// `ASCII` — MySQL shorthand for `CHARACTER SET latin1`.
    Ascii,
    /// `UNICODE` — MySQL shorthand for `CHARACTER SET ucs2`.
    Unicode,
    /// `BYTE` — MySQL/Oracle-compatibility shorthand producing a binary `CHAR`.
    Byte,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// The SQL binary type name forms represented by the AST.
pub enum BinaryTypeName {
    /// The fixed-length `BINARY` spelling.
    Binary,
    /// The variable-length `BINARY VARYING` spelling.
    BinaryVarying,
    /// The variable-length `VARBINARY` spelling.
    Varbinary,
    /// PostgreSQL's `BYTEA` spelling.
    Bytea,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// The SQL time type name forms represented by the AST.
pub enum TimeTypeName {
    /// The `TIME` spelling.
    Time,
    /// PostgreSQL's `TIMETZ` (`TIME WITH TIME ZONE`) spelling.
    Timetz,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// The SQL timestamp type name forms represented by the AST.
pub enum TimestampTypeName {
    /// The `TIMESTAMP` spelling.
    Timestamp,
    /// PostgreSQL's `TIMESTAMPTZ` (`TIMESTAMP WITH TIME ZONE`) spelling.
    Timestamptz,
    /// MySQL `DATETIME`: a timestamp-shaped type without time-zone semantics,
    /// recognized only under
    /// [`TypeNameSyntax::extended_scalar_type_names`](crate::dialect::TypeNameSyntax). It
    /// reuses the timestamp variant (one canonical shape) and pins
    /// [`TimeZone::Unspecified`](TimeZone), since `DATETIME` takes no zone suffix.
    Datetime,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// The SQL time zone forms represented by the AST.
pub enum TimeZone {
    /// No time-zone qualifier was written.
    Unspecified,
    /// `WITH TIME ZONE` — a time-zone-aware type.
    WithTimeZone,
    /// `WITHOUT TIME ZONE` — a time-zone-naive type.
    WithoutTimeZone,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// The SQL interval fields forms represented by the AST.
pub enum IntervalFields {
    /// `YEAR` — a year-precision interval qualifier.
    Year,
    /// `MONTH` — a month-precision interval qualifier.
    Month,
    /// `DAY` — a day-precision interval qualifier.
    Day,
    /// `HOUR` — an hour-precision interval qualifier.
    Hour,
    /// `MINUTE` — a minute-precision interval qualifier.
    Minute,
    /// `SECOND` — a second-precision interval qualifier.
    Second,
    /// `YEAR TO MONTH` — a year-and-month interval qualifier.
    YearToMonth,
    /// `DAY TO HOUR` — a day-to-hour interval qualifier.
    DayToHour,
    /// `DAY TO MINUTE` — a day-to-minute interval qualifier.
    DayToMinute,
    /// `DAY TO SECOND` — a day-to-second interval qualifier.
    DayToSecond,
    /// `HOUR TO MINUTE` — an hour-to-minute interval qualifier.
    HourToMinute,
    /// `HOUR TO SECOND` — an hour-to-second interval qualifier.
    HourToSecond,
    /// `MINUTE TO SECOND` — a minute-to-second interval qualifier.
    MinuteToSecond,
    // DuckDB-only extended units, admitted as `INTERVAL <amount> <unit>` multipliers
    // (both singular and plural spellings) solely under
    // [`ExpressionSyntax::relaxed_interval_syntax`](crate::dialect::ExpressionSyntax).
    // ANSI/PostgreSQL never produce these — their interval grammar has no such
    // qualifier — so the shared `DataType::Interval` render/accept path is unchanged.
    // Each is a whole, simple qualifier: DuckDB has no `TO` composite for them and
    // rejects a trailing precision, so (like the plural forms) they carry no precision.
    /// `WEEK` — a DuckDB week interval qualifier (non-standard).
    Week,
    /// `QUARTER` — a DuckDB quarter interval qualifier (non-standard).
    Quarter,
    /// `DECADE` — a DuckDB decade interval qualifier (non-standard).
    Decade,
    /// `CENTURY` — a DuckDB century interval qualifier (non-standard).
    Century,
    /// `MILLENNIUM` — a DuckDB millennium interval qualifier (non-standard).
    Millennium,
    /// `MILLISECOND` — a DuckDB millisecond interval qualifier (non-standard).
    Millisecond,
    /// `MICROSECOND` — a DuckDB microsecond interval qualifier (non-standard).
    Microsecond,
    // MySQL-only microsecond composite units, admitted solely in the MySQL `interval`
    // vocabulary (the `EVERY <expr> <unit>` event schedule and `INTERVAL <expr> <unit>`
    // arithmetic). MySQL spells them with an underscore (`DAY_MICROSECOND`), not the ANSI
    // `TO` composite; the standard `DataType::Interval` render never produces them (no
    // dialect's INTERVAL type grammar admits a microsecond composite), so they extend the
    // shared vocabulary without touching the ANSI interval type. The other MySQL composites
    // (`DAY_HOUR`, `MINUTE_SECOND`, `YEAR_MONTH`, …) reuse the existing `*To*` variants.
    /// `DAY_MICROSECOND` — a MySQL day-to-microsecond composite interval unit.
    DayToMicrosecond,
    /// `HOUR_MICROSECOND` — a MySQL hour-to-microsecond composite interval unit.
    HourToMicrosecond,
    /// `MINUTE_MICROSECOND` — a MySQL minute-to-microsecond composite interval unit.
    MinuteToMicrosecond,
    /// `SECOND_MICROSECOND` — a MySQL second-to-microsecond composite interval unit.
    SecondToMicrosecond,
}
