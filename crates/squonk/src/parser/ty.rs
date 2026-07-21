// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! SQL type-name grammar shared by casts, DDL, parameters, and temporal syntax.
//!
//! The AST keeps one canonical type shape plus compact spelling tags.
//! This parser therefore accepts the common ANSI/PostgreSQL surface spellings and
//! preserves the spelling choice when multiple spellings name the same meaning.

use crate::ast::{
    ArrayTypeSpelling, BinaryTypeName, BlobTypeName, BooleanTypeName, CharacterTypeName, Charset,
    CharsetAnnotation, DataType, DecimalTypeName, DoubleTypeName, Expr, Extension, Ident, IntWidth,
    IntegerTypeName, IntervalFields, Keyword, Literal, LiteralKind, QuoteStyle, Signedness, Span,
    Spanned, StructTypeField, StructTypeSpelling, TextTypeName, TimeTypeName, TimeZone,
    TimestampTypeName, WrappedTypeKind,
};
use crate::error::ParseResult;
use crate::tokenizer::{Operator, Punctuation, TokenKind};
use thin_vec::ThinVec;

use super::engine::Parser;
use super::expr::{number_literal_kind, string_literal_is_sconst};
use super::{Dialect, HookResult};

impl<'a, D: Dialect> Parser<'a, D> {
    /// Parse a SQL data type and any suffix array markers.
    ///
    /// The single entry every type position funnels through, so the dialect's
    /// [`parse_data_type_hook`](Dialect::parse_data_type_hook) is consulted here first:
    /// a `Handled` node claims the production, `NotHandled` leaves the cursor untouched
    /// and the built-in grammar runs (`Err` surfaces the hook's diagnostic).
    pub(super) fn parse_data_type(&mut self) -> ParseResult<DataType<D::Ext>> {
        match D::parse_data_type_hook(self) {
            HookResult::Handled(data_type) => return Ok(data_type),
            HookResult::NotHandled => {}
            HookResult::Err(error) => return Err(error),
        }

        if self.features().type_name_syntax.liberal_type_names {
            return self.parse_liberal_or_typed_data_type();
        }
        self.parse_typed_data_type()
    }

    /// SQLite's liberal affinity type-name dispatch (the
    /// [`TypeNameSyntax::liberal_type_names`](crate::ast::dialect::TypeNameSyntax) gate).
    ///
    /// A strict FALLBACK: it runs the ordinary typed / user-defined parse first — a typed
    /// variant (or the single-word user-defined path) wins wherever it can faithfully hold
    /// the input, so a bare `INT`, `DOUBLE PRECISION`, `VARCHAR(255)`, `NATIONAL
    /// CHARACTER(15)`, or single-word `BANANA` keep their existing shapes — and drops to
    /// [`DataType::Liberal`] only when the surface exceeds what that parse can hold: a
    /// trailing type-word (`LONG INTEGER`, `FLOATING POINT`) or a typed-parse failure on a
    /// two-argument built-in modifier (`VARCHAR(123,456)`), which is then re-read as a liberal
    /// word run from the start.
    fn parse_liberal_or_typed_data_type(&mut self) -> ParseResult<DataType<D::Ext>> {
        let start = self.current_span()?;
        let checkpoint = self.checkpoint();
        // On a clean typed parse with no trailing type-word, keep the typed variant. On a
        // trailing word (multi-word name) or a typed-parse error (e.g. a two-argument built-in
        // modifier) fall through to the liberal reparse.
        if let Ok(typed) = self.parse_typed_data_type() {
            if !self.peek_is_liberal_type_word()? {
                return Ok(typed);
            }
        }
        self.rewind(checkpoint);
        self.parse_liberal_type_name(start)
    }

    /// Parse a SQLite liberal affinity type name at `start`: a run of one-or-more type-name
    /// words followed by an optional one-or-two-argument parenthesized modifier list.
    fn parse_liberal_type_name(&mut self, start: Span) -> ParseResult<DataType<D::Ext>> {
        let reserved = self.features().reserved_type_name;
        let mut words = ThinVec::new();
        while self.peek_is_liberal_type_word()? {
            words.push(self.parse_ident_admitting(reserved, "a type name word")?);
        }
        if words.is_empty() {
            return Err(self.unexpected("a data type"));
        }
        let args = self.parse_optional_liberal_type_args()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(DataType::Liberal { words, args, meta })
    }

    /// True when the next token continues a SQLite liberal affinity type name: a plain
    /// identifier or a keyword admissible as a type-name word
    /// ([`token_admissible`](Self::token_admissible) against
    /// [`reserved_type_name`](crate::ast::dialect::FeatureSet::reserved_type_name), SQLite's
    /// `ids`-class reject set — so the column-constraint keywords `PRIMARY`/`NOT`/`NULL`/
    /// `UNIQUE`/`CHECK`/`DEFAULT`/`COLLATE`/`REFERENCES`/`CONSTRAINT`/`AS` terminate the run).
    /// `GENERATED` is excluded explicitly: it is the one non-reserved keyword that begins a
    /// real column constraint (`GENERATED ALWAYS AS`), so absorbing it as a type word would
    /// shadow the generated-column parse (engine edge `x FOO GENERATED bar` is left an
    /// accepted gap, never over-accepted).
    fn peek_is_liberal_type_word(&mut self) -> ParseResult<bool> {
        let column_def = self.features().column_definition_syntax;
        // The auto-increment spellings are excluded exactly when their
        // attribute gate admits them (the IDENTITY pattern below): absorbing
        // the word as a type word would shadow the attribute parse — and,
        // worse, force a liberal reparse whose first word is a reserved type
        // name, turning `a INTEGER AUTO_INCREMENT` into a parse error at
        // `INTEGER`. With the gate off the word stays an ordinary affinity
        // word (SQLite's engine-measured reading).
        if self.peek_is_contextual_keyword("GENERATED")?
            || (column_def.compact_identity_columns
                && self.peek_is_contextual_keyword("IDENTITY")?)
            || (column_def.joined_autoincrement_attribute
                && self.peek_is_contextual_keyword("AUTOINCREMENT")?)
            || (column_def.underscored_autoincrement_attribute
                && self.peek_is_contextual_keyword("AUTO_INCREMENT")?)
        {
            return Ok(false);
        }
        let reserved = self.features().reserved_type_name;
        Ok(match self.peek()? {
            Some(token) => self.token_admissible(token, reserved),
            None => false,
        })
    }

    /// Parse SQLite's optional liberal type-argument list: `( <u32> [, <u32>] )`, at most two
    /// arguments (engine-probed on rusqlite/sqlite3 3.53.2 & 3.43.2: `FOO(1,2)` accepts,
    /// `FOO(1,2,3)` rejects — the `typetoken` grammar's `LP signed [COMMA signed] RP`).
    /// Absent parens → empty. Unsigned only for now (SQLite also admits a signed/fractional
    /// argument; the corpus surface is unsigned and the wider form under-accepts, never
    /// over-accepts).
    fn parse_optional_liberal_type_args(&mut self) -> ParseResult<ThinVec<u32>> {
        if !self.eat_punct(Punctuation::LParen)? {
            return Ok(ThinVec::new());
        }
        let mut args = ThinVec::new();
        args.push(self.parse_u32_type_modifier()?);
        if self.eat_punct(Punctuation::Comma)? {
            args.push(self.parse_u32_type_modifier()?);
        }
        self.expect_punct(Punctuation::RParen, "`)` to close the type modifier")?;
        Ok(args)
    }

    /// The stock typed / user-defined type grammar (the pre-liberal path). Every non-SQLite
    /// dialect calls this directly; SQLite/Lenient reach it through
    /// [`parse_liberal_or_typed_data_type`](Self::parse_liberal_or_typed_data_type).
    fn parse_typed_data_type(&mut self) -> ParseResult<DataType<D::Ext>> {
        let start = self.current_span()?;
        let data_type = if self.eat_contextual_keyword("BOOLEAN")? {
            let meta = self.make_meta(start.union(self.preceding_span()));
            DataType::Boolean {
                spelling: BooleanTypeName::Boolean,
                meta,
            }
        } else if self.eat_contextual_keyword("BOOL")? {
            let meta = self.make_meta(start.union(self.preceding_span()));
            DataType::Boolean {
                spelling: BooleanTypeName::Bool,
                meta,
            }
        } else if self.eat_contextual_keyword("SMALLINT")? {
            let display_width = self.parse_optional_integer_display_width()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            DataType::SmallInt {
                display_width,
                meta,
            }
        } else if self.eat_contextual_keyword("INT")? {
            let display_width = self.parse_optional_integer_display_width()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            DataType::Integer {
                spelling: IntegerTypeName::Int,
                display_width,
                meta,
            }
        } else if self.eat_contextual_keyword("INTEGER")? {
            let display_width = self.parse_optional_integer_display_width()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            DataType::Integer {
                spelling: IntegerTypeName::Integer,
                display_width,
                meta,
            }
        } else if self.eat_contextual_keyword("BIGINT")? {
            let display_width = self.parse_optional_integer_display_width()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            DataType::BigInt {
                display_width,
                meta,
            }
        } else if self.eat_contextual_keyword("DECIMAL")? {
            let (precision, scale) = self.parse_optional_precision_scale()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            DataType::Decimal {
                spelling: DecimalTypeName::Decimal,
                precision,
                scale,
                meta,
            }
        } else if self.eat_contextual_keyword("DEC")? {
            let (precision, scale) = self.parse_optional_precision_scale()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            DataType::Decimal {
                spelling: DecimalTypeName::Dec,
                precision,
                scale,
                meta,
            }
        } else if self.eat_contextual_keyword("NUMERIC")? {
            let (precision, scale) = self.parse_optional_precision_scale()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            DataType::Decimal {
                spelling: DecimalTypeName::Numeric,
                precision,
                scale,
                meta,
            }
        } else if self.eat_contextual_keyword("FLOAT")? {
            let precision = self.parse_optional_single_type_modifier()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            DataType::Float { precision, meta }
        } else if self.eat_contextual_keyword("REAL")? {
            let meta = self.make_meta(start.union(self.preceding_span()));
            DataType::Real { meta }
        } else if self.peek_is_contextual_keyword("DOUBLE")?
            && self.peek_nth_is_contextual_keyword(1, "PRECISION")?
        {
            // Only `DOUBLE PRECISION` is the built-in; a bare `double` falls through
            // to the user-defined type name (it is unreserved), matching PostgreSQL.
            self.advance()?; // DOUBLE
            self.advance()?; // PRECISION
            let meta = self.make_meta(start.union(self.preceding_span()));
            DataType::Double {
                spelling: DoubleTypeName::DoublePrecision,
                meta,
            }
        } else if self.eat_contextual_keyword("TEXT")? {
            let charset = self.parse_optional_charset_annotation()?.map(Box::new);
            let meta = self.make_meta(start.union(self.preceding_span()));
            DataType::Text {
                spelling: TextTypeName::Text,
                charset,
                meta,
            }
        } else if self.eat_contextual_keyword("CHAR")? {
            let spelling = if self.eat_contextual_keyword("VARYING")? {
                CharacterTypeName::CharVarying
            } else {
                CharacterTypeName::Char
            };
            let size = self.parse_optional_single_type_modifier()?;
            let charset = self.parse_optional_charset_annotation()?.map(Box::new);
            let meta = self.make_meta(start.union(self.preceding_span()));
            DataType::Character {
                spelling,
                size,
                charset,
                meta,
            }
        } else if self.eat_contextual_keyword("CHARACTER")? {
            let spelling = if self.eat_contextual_keyword("VARYING")? {
                CharacterTypeName::CharacterVarying
            } else {
                CharacterTypeName::Character
            };
            let size = self.parse_optional_single_type_modifier()?;
            let charset = self.parse_optional_charset_annotation()?.map(Box::new);
            let meta = self.make_meta(start.union(self.preceding_span()));
            DataType::Character {
                spelling,
                size,
                charset,
                meta,
            }
        } else if self.eat_contextual_keyword("VARCHAR")? {
            let size = self.parse_optional_single_type_modifier()?;
            self.reject_missing_varchar_length(size)?;
            let charset = self.parse_optional_charset_annotation()?.map(Box::new);
            let meta = self.make_meta(start.union(self.preceding_span()));
            DataType::Character {
                spelling: CharacterTypeName::Varchar,
                size,
                charset,
                meta,
            }
        } else if self.eat_contextual_keyword("BINARY")? {
            let spelling = if self.eat_contextual_keyword("VARYING")? {
                BinaryTypeName::BinaryVarying
            } else {
                BinaryTypeName::Binary
            };
            let size = self.parse_optional_single_type_modifier()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            DataType::Binary {
                spelling,
                size,
                meta,
            }
        } else if self.eat_contextual_keyword("VARBINARY")? {
            let size = self.parse_optional_single_type_modifier()?;
            self.reject_missing_varchar_length(size)?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            DataType::Binary {
                spelling: BinaryTypeName::Varbinary,
                size,
                meta,
            }
        } else if self.eat_contextual_keyword("BYTEA")? {
            let meta = self.make_meta(start.union(self.preceding_span()));
            DataType::Binary {
                spelling: BinaryTypeName::Bytea,
                size: None,
                meta,
            }
        } else if self.eat_contextual_keyword("BIT")? {
            let varying = self.eat_contextual_keyword("VARYING")?;
            let size = self.parse_optional_single_type_modifier()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            DataType::Bit {
                varying,
                size,
                meta,
            }
        } else if self.eat_contextual_keyword("JSON")? {
            let meta = self.make_meta(start.union(self.preceding_span()));
            DataType::Json { meta }
        } else if self.eat_contextual_keyword("UUID")? {
            // Ungated like `JSON`: the canonical UUID identity is admitted wherever a type
            // name is (PostgreSQL/DuckDB have the type; a planner still wants the identity
            // elsewhere). Positional acceptance stays each dialect's concern — MySQL's
            // narrow `CAST` target gate rejects a `DataType::Uuid` the same as it did the
            // former `UserDefined` name.
            let meta = self.make_meta(start.union(self.preceding_span()));
            DataType::Uuid { meta }
        } else if self.eat_contextual_keyword("NCHAR")? {
            let spelling = if self.eat_contextual_keyword("VARYING")? {
                CharacterTypeName::NcharVarying
            } else {
                CharacterTypeName::Nchar
            };
            let size = self.parse_optional_single_type_modifier()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            // The national char forms fix their own charset, so MySQL rejects the
            // charset annotation on them (`NCHAR CHARACTER SET x` is `ER_PARSE_ERROR` on
            // mysql:8): the annotation parser is not consulted here, so a trailing
            // `CHARACTER SET` is left for the caller to reject.
            DataType::Character {
                spelling,
                size,
                charset: None,
                meta,
            }
        } else if self.eat_contextual_keyword("NATIONAL")? {
            let spelling = self.parse_national_character_spelling()?;
            let size = self.parse_optional_single_type_modifier()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            DataType::Character {
                spelling,
                size,
                charset: None,
                meta,
            }
        } else if self.eat_contextual_keyword("DATE")? {
            let meta = self.make_meta(start.union(self.preceding_span()));
            DataType::Date { meta }
        } else if self.eat_contextual_keyword("TIME")? {
            let precision = self.parse_optional_single_type_modifier()?;
            let time_zone = self.parse_optional_time_zone()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            DataType::Time {
                spelling: TimeTypeName::Time,
                precision,
                time_zone,
                meta,
            }
        } else if self.eat_contextual_keyword("TIMETZ")? {
            let precision = self.parse_optional_single_type_modifier()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            DataType::Time {
                spelling: TimeTypeName::Timetz,
                precision,
                time_zone: TimeZone::WithTimeZone,
                meta,
            }
        } else if self.eat_contextual_keyword("TIMESTAMP")? {
            let precision = self.parse_optional_single_type_modifier()?;
            let time_zone = self.parse_optional_time_zone()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            DataType::Timestamp {
                spelling: TimestampTypeName::Timestamp,
                precision,
                time_zone,
                meta,
            }
        } else if self.eat_contextual_keyword("TIMESTAMPTZ")? {
            let precision = self.parse_optional_single_type_modifier()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            DataType::Timestamp {
                spelling: TimestampTypeName::Timestamptz,
                precision,
                time_zone: TimeZone::WithTimeZone,
                meta,
            }
        } else if self.eat_contextual_keyword("INTERVAL")? {
            self.parse_interval_type(start)?
        } else if let Some(dialect_type) = self.try_parse_dialect_type_name(start)? {
            dialect_type
        } else {
            self.parse_user_defined_data_type(start)?
        };

        self.reject_zoned_temporal_type(&data_type, start)?;

        // MySQL's `SIGNED`/`UNSIGNED`/`ZEROFILL` are a postfix on the numeric type;
        // applying them here (not per numeric branch) keeps the modifier modelled
        // once. The standalone cast targets (`CAST(x AS UNSIGNED)`) are already a
        // `NumericModifier`, which `is_numeric` excludes, so they are never re-wrapped.
        let data_type = self.parse_optional_numeric_modifiers(data_type)?;
        self.parse_array_suffixes(data_type)
    }

    /// Reject a `VARCHAR`/`VARBINARY` written without an explicit length under
    /// [`TypeNameSyntax::varchar_requires_length`](crate::ast::dialect::TypeNameSyntax)
    /// (MySQL): a length-less form is an `ER_PARSE_ERROR` there, so the missing `(N)`
    /// surfaces as a clean parse error rather than a `size: None` type.
    fn reject_missing_varchar_length(&mut self, size: Option<u32>) -> ParseResult<()> {
        if size.is_none() && self.features().type_name_syntax.varchar_requires_length {
            return Err(self.unexpected("an explicit length `(N)` on `VARCHAR`/`VARBINARY`"));
        }
        Ok(())
    }

    /// Reject a time-zone-qualified temporal type (`TIMESTAMPTZ`, `TIMESTAMP`/`TIME`
    /// `WITH`/`WITHOUT TIME ZONE`, `TIMETZ`) under a dialect whose
    /// [`TypeNameSyntax::zoned_temporal_types`](crate::ast::dialect::TypeNameSyntax) is off
    /// (MySQL, which has no zoned temporal type — its `TIMESTAMP` carries no zone qualifier).
    /// The zone-less `TIMESTAMP`/`TIME`/`DATETIME` forms ([`TimeZone::Unspecified`]) are
    /// unaffected.
    fn reject_zoned_temporal_type(
        &mut self,
        ty: &DataType<D::Ext>,
        start: Span,
    ) -> ParseResult<()> {
        if self.features().type_name_syntax.zoned_temporal_types {
            return Ok(());
        }
        let zoned = matches!(
            ty,
            DataType::Timestamp {
                time_zone: TimeZone::WithTimeZone | TimeZone::WithoutTimeZone,
                ..
            } | DataType::Time {
                time_zone: TimeZone::WithTimeZone | TimeZone::WithoutTimeZone,
                ..
            }
        );
        if zoned {
            let span = start.union(self.preceding_span());
            return Err(self.error_at(
                span,
                "a temporal type without a time-zone qualifier (this dialect has no zoned temporal type)",
                self.span_text(span).to_owned(),
            ));
        }
        Ok(())
    }

    /// Recognize a dialect-only type name (for example `TINYINT`/`ENUM`/`UNSIGNED`),
    /// gated by [`TypeNameSyntax`](crate::ast::dialect::TypeNameSyntax) data.
    ///
    /// Returns `None` without consuming input when no gated form matches, so the
    /// caller falls through to the user-defined-type path — this is exactly how a
    /// dialect that leaves a knob off declines to recognize the name. Each gate is
    /// read as data; there is no `match dialect` here.
    fn try_parse_dialect_type_name(
        &mut self,
        start: Span,
    ) -> ParseResult<Option<DataType<D::Ext>>> {
        let types = self.features().type_name_syntax;
        if types.extended_scalar_type_names {
            if let Some(data_type) = self.try_parse_extended_scalar_type_name(start)? {
                return Ok(Some(data_type));
            }
        }
        if types.bit_width_integer_names {
            if let Some(data_type) = self.try_parse_bit_width_integer_name(start)? {
                return Ok(Some(data_type));
            }
        }
        if types.enum_type || types.set_type {
            if let Some(data_type) = self.try_parse_enum_set_type(start)? {
                return Ok(Some(data_type));
            }
        }
        if types.numeric_modifiers {
            if let Some(data_type) = self.try_parse_standalone_sign(start)? {
                return Ok(Some(data_type));
            }
        }
        if types.composite_types {
            if let Some(data_type) = self.try_parse_composite_type(start)? {
                return Ok(Some(data_type));
            }
        }
        if types.angle_bracket_types {
            if let Some(data_type) = self.try_parse_angle_bracket_type(start)? {
                return Ok(Some(data_type));
            }
        }
        if types.fixed_string_type {
            if let Some(data_type) = self.try_parse_fixed_string_type(start)? {
                return Ok(Some(data_type));
            }
        }
        if types.datetime64_type {
            if let Some(data_type) = self.try_parse_datetime64_type(start)? {
                return Ok(Some(data_type));
            }
        }
        if types.nested_type {
            if let Some(data_type) = self.try_parse_nested_type(start)? {
                return Ok(Some(data_type));
            }
        }
        // The wrapper family reads its own per-keyword gates inside; the call is cheap
        // (a `(`-lookahead) and returns `None` when no enabled wrapper keyword leads.
        if let Some(data_type) = self.try_parse_wrapped_type(start)? {
            return Ok(Some(data_type));
        }
        Ok(None)
    }

    /// Parse a ClickHouse parametric type combinator: `Nullable(T)` (and future
    /// wrapper-shaped siblings such as `LowCardinality(T)`, each sharing this
    /// single-inner-type shape). Like the composite constructors, only the
    /// keyword-immediately-followed-by-`(` form is the combinator — a bare `Nullable` (no
    /// `(`) stays an ordinary type/column name — so the `(` lookahead is the
    /// disambiguation, never a tokenizer change. Each keyword reads its own
    /// [`TypeNameSyntax`](crate::ast::dialect::TypeNameSyntax) gate, so a preset opts into
    /// each independently; returns `None` without consuming input when the lookahead fails
    /// or no enabled wrapper keyword leads, so the caller falls through to the
    /// user-defined-type path.
    fn try_parse_wrapped_type(&mut self, start: Span) -> ParseResult<Option<DataType<D::Ext>>> {
        if !self.peek_nth_is_punct(1, Punctuation::LParen)? {
            return Ok(None);
        }
        let types = self.features().type_name_syntax;
        if types.nullable_type && self.peek_is_contextual_keyword("NULLABLE")? {
            self.advance()?; // Nullable
            return Ok(Some(
                self.parse_wrapped_type(start, WrappedTypeKind::Nullable)?,
            ));
        }
        if types.low_cardinality_type && self.peek_is_contextual_keyword("LOWCARDINALITY")? {
            self.advance()?; // LowCardinality
            return Ok(Some(
                self.parse_wrapped_type(start, WrappedTypeKind::LowCardinality)?,
            ));
        }
        Ok(None)
    }

    /// Parse the `( <inner_type> )` body of a wrapper combinator, its keyword already
    /// consumed. The inner is a full (recursively nested) type, so
    /// `Nullable(DECIMAL(10, 2))` recurses through
    /// [`parse_data_type`](Self::parse_data_type) — the same called-and-returned recursion
    /// idiom as the `MAP(K, V)` key/value types, so it introduces no new self-recursion
    /// frame beyond the one those constructors already have.
    fn parse_wrapped_type(
        &mut self,
        start: Span,
        kind: WrappedTypeKind,
    ) -> ParseResult<DataType<D::Ext>> {
        self.expect_punct(Punctuation::LParen, "`(` to open the wrapped inner type")?;
        let inner = self.parse_data_type()?;
        self.expect_punct(Punctuation::RParen, "`)` to close the wrapped type")?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(DataType::Wrapped {
            kind,
            inner: Box::new(inner),
            meta,
        })
    }

    /// Parse ClickHouse's `FixedString(N)` type constructor — a fixed-length byte string
    /// of exactly `N` bytes. Like the wrapper/composite constructors, only the
    /// keyword-immediately-followed-by-`(` form is the type; a bare `FixedString` (no `(`)
    /// is an ordinary type/column name, so the `(` lookahead is the disambiguation. `N` is
    /// mandatory (ClickHouse has no bare `FixedString`), so the `(N)` is required, not
    /// optional. The length is any `u32` literal; ClickHouse's positive-length requirement
    /// is a bind-time reject, not a grammar error. Returns `None` without consuming input
    /// when the lookahead fails or the leading word is not `FixedString`, so the caller
    /// falls through to the user-defined-type path.
    fn try_parse_fixed_string_type(
        &mut self,
        start: Span,
    ) -> ParseResult<Option<DataType<D::Ext>>> {
        if !self.peek_nth_is_punct(1, Punctuation::LParen)? {
            return Ok(None);
        }
        if !self.peek_is_contextual_keyword("FIXEDSTRING")? {
            return Ok(None);
        }
        self.advance()?; // FixedString
        self.expect_punct(Punctuation::LParen, "`(` to open the FixedString length")?;
        let length = self.parse_u32_type_modifier()?;
        self.expect_punct(Punctuation::RParen, "`)` to close the FixedString length")?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(Some(DataType::FixedString { length, meta }))
    }

    /// Parse ClickHouse's `DateTime64(P[, 'timezone'])` type constructor — a sub-second
    /// timestamp with `P` fractional digits and an optional IANA time-zone string. Like the
    /// wrapper/composite constructors, only the keyword-immediately-followed-by-`(` form is
    /// the type; a bare `DateTime64` (no `(`) is an ordinary type/column name, so the `(`
    /// lookahead is the disambiguation. `P` is mandatory (ClickHouse has no bare `DateTime64`
    /// spelling), parsed as any `u32` literal — the documented `0..=9` range is a bind-time
    /// reject, not a grammar error. The timezone is an optional single-quoted string literal
    /// second argument, held source-spelled so its exact quoting round-trips. Returns `None`
    /// without consuming input when the lookahead fails or the leading word is not
    /// `DateTime64`, so the caller falls through to the user-defined-type path.
    fn try_parse_datetime64_type(&mut self, start: Span) -> ParseResult<Option<DataType<D::Ext>>> {
        if !self.peek_nth_is_punct(1, Punctuation::LParen)? {
            return Ok(None);
        }
        if !self.peek_is_contextual_keyword("DATETIME64")? {
            return Ok(None);
        }
        self.advance()?; // DateTime64
        self.expect_punct(Punctuation::LParen, "`(` to open the DateTime64 arguments")?;
        let precision = self.parse_u32_type_modifier()?;
        let timezone = if self.eat_punct(Punctuation::Comma)? {
            Some(Box::new(self.expect_string_literal(
                "a single-quoted DateTime64 timezone string",
            )?))
        } else {
            None
        };
        self.expect_punct(Punctuation::RParen, "`)` to close the DateTime64 arguments")?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(Some(DataType::DateTime64 {
            precision,
            timezone,
            meta,
        }))
    }

    /// Parse ClickHouse's `Nested(name1 Type1, name2 Type2, ...)` named-field composite type.
    /// Like the wrapper/composite constructors, only the keyword-immediately-followed-by-`(`
    /// form is the type; a bare `Nested` (no `(`) is an ordinary type/column name, so the `(`
    /// lookahead is the disambiguation. The field list reuses the shared
    /// [`parse_struct_type_fields`](Self::parse_struct_type_fields) production (a `name Type`
    /// one-or-more comma list, at least one field required), so a field type is a full
    /// recursively-nested type and `Nested(x Nested(...))` parses — ClickHouse's nesting-level
    /// limit is a setting/bind concern, not a grammar error. Returns `None` without consuming
    /// input when the lookahead fails or the leading word is not `Nested`, so the caller falls
    /// through to the user-defined-type path.
    fn try_parse_nested_type(&mut self, start: Span) -> ParseResult<Option<DataType<D::Ext>>> {
        if !self.peek_nth_is_punct(1, Punctuation::LParen)? {
            return Ok(None);
        }
        if !self.peek_is_contextual_keyword("NESTED")? {
            return Ok(None);
        }
        self.advance()?; // Nested
        let fields = self.parse_struct_type_fields()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(Some(DataType::Nested { fields, meta }))
    }

    /// Parse a DuckDB anonymous composite / nested type constructor: `STRUCT(a INT, ...)`,
    /// the standard `ROW(...)` spelling of the same shape, `UNION(tag T, ...)`, or
    /// `MAP(K, V)`. Gated by
    /// [`TypeNameSyntax::composite_types`](crate::ast::dialect::TypeNameSyntax).
    ///
    /// Only the keyword-immediately-followed-by-`(` form is the constructor; a bare
    /// `STRUCT`/`MAP`/`UNION`/`ROW` (no `(`) is an ordinary type/column name, so the `(`
    /// lookahead is the disambiguation — never a tokenizer change, mirroring the
    /// `ARRAY[`/`ROW(` expression constructors. Returns `None` without consuming input
    /// when the leading word matches none of the four, so the caller falls through to the
    /// user-defined-type path.
    fn try_parse_composite_type(&mut self, start: Span) -> ParseResult<Option<DataType<D::Ext>>> {
        if !self.peek_nth_is_punct(1, Punctuation::LParen)? {
            return Ok(None);
        }
        if self.peek_is_contextual_keyword("STRUCT")? {
            self.advance()?; // STRUCT
            let fields = self.parse_struct_type_fields()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(DataType::Struct {
                fields,
                spelling: StructTypeSpelling::Struct,
                meta,
            }));
        }
        if self.peek_is_contextual_keyword("ROW")? {
            self.advance()?; // ROW
            let fields = self.parse_struct_type_fields()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(DataType::Struct {
                fields,
                spelling: StructTypeSpelling::Row,
                meta,
            }));
        }
        if self.peek_is_contextual_keyword("UNION")? {
            self.advance()?; // UNION
            let members = self.parse_struct_type_fields()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(DataType::Union { members, meta }));
        }
        if self.peek_is_contextual_keyword("MAP")? {
            self.advance()?; // MAP
            return Ok(Some(self.parse_map_type(start)?));
        }
        Ok(None)
    }

    /// Parse BigQuery angle-bracket type forms: `STRUCT<field TYPE, …>` and `ARRAY<T>`.
    /// Gated by [`TypeNameSyntax::angle_bracket_types`](crate::ast::dialect::TypeNameSyntax). Only keyword-immediately-followed-by
    /// `<` is the constructor (bare `STRUCT`/`ARRAY` stays a name). Nested closers that
    /// lex as `>>` (ShiftRight) are split via a pending-`>` counter so `STRUCT<a STRUCT<b INT>>`
    /// parses (shared-byte-trigger with bitwise `>>` — only armed under this gate).
    fn try_parse_angle_bracket_type(
        &mut self,
        start: Span,
    ) -> ParseResult<Option<DataType<D::Ext>>> {
        if self.peek_is_contextual_keyword("STRUCT")? && self.peek_nth_is_op(1, Operator::Lt)? {
            self.advance()?; // STRUCT
            let fields = self.parse_angle_struct_type_fields()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(DataType::Struct {
                fields,
                spelling: StructTypeSpelling::AngleBracket,
                meta,
            }));
        }
        if self.peek_is_contextual_keyword("ARRAY")? && self.peek_nth_is_op(1, Operator::Lt)? {
            self.advance()?; // ARRAY
            self.expect_op(Operator::Lt, "`<` to open ARRAY element type")?;
            let element = self.parse_data_type()?;
            self.expect_angle_gt("`>` to close ARRAY element type")?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(DataType::Array {
                element: Box::new(element),
                size: None,
                spelling: ArrayTypeSpelling::AngleBracket,
                meta,
            }));
        }
        Ok(None)
    }

    /// Parse `field TYPE, …` between `<` … `>` for angle-bracket STRUCT.
    fn parse_angle_struct_type_fields(&mut self) -> ParseResult<ThinVec<StructTypeField<D::Ext>>> {
        self.expect_op(Operator::Lt, "`<` to open STRUCT fields")?;
        let fields = self.parse_comma_separated(Self::parse_struct_type_field)?;
        self.expect_angle_gt("`>` to close STRUCT fields")?;
        Ok(fields)
    }

    /// Consume one type-position `>` close. Accepts a lone `>` or a `>>` (ShiftRight)
    /// that contributes one close now and one pending for the next call — so nested
    /// `STRUCT<a STRUCT<b INT>>` works without a tokenizer change.
    fn expect_angle_gt(&mut self, expected: &'static str) -> ParseResult<()> {
        if self.angle_gt_pending > 0 {
            self.angle_gt_pending -= 1;
            return Ok(());
        }
        if self.eat_op(Operator::Gt)? {
            return Ok(());
        }
        if self.eat_op(Operator::ShiftRight)? {
            self.angle_gt_pending = 1;
            return Ok(());
        }
        Err(self.unexpected(expected))
    }

    /// Parse the parenthesized `(name TYPE, ...)` field list shared by the
    /// `STRUCT`/`ROW`/`UNION` composite constructors. DuckDB requires at least one field
    /// (`STRUCT()` is a syntax error), which the one-or-more comma list enforces.
    fn parse_struct_type_fields(&mut self) -> ParseResult<ThinVec<StructTypeField<D::Ext>>> {
        self.expect_punct(Punctuation::LParen, "`(` to open the composite type fields")?;
        let fields = self.parse_comma_separated(Self::parse_struct_type_field)?;
        self.expect_punct(
            Punctuation::RParen,
            "`)` to close the composite type fields",
        )?;
        Ok(fields)
    }

    /// Parse one `name TYPE` composite-type field. The name is a `ColId` identifier
    /// (bare `a`, or quoted `"key"`), and the type is a full (recursively nested) type.
    fn parse_struct_type_field(&mut self) -> ParseResult<StructTypeField<D::Ext>> {
        let start = self.current_span()?;
        let name = self.parse_ident()?;
        let ty = self.parse_data_type()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(StructTypeField { name, ty, meta })
    }

    /// Parse a `MAP(K, V)` key/value type pair, the `MAP` keyword already consumed. The
    /// key and value are themselves types, so nested composites (`MAP(INT[], STRUCT(...))`)
    /// recurse through [`parse_data_type`](Self::parse_data_type).
    fn parse_map_type(&mut self, start: Span) -> ParseResult<DataType<D::Ext>> {
        self.expect_punct(Punctuation::LParen, "`(` to open the MAP key/value types")?;
        let key = self.parse_data_type()?;
        self.expect_punct(
            Punctuation::Comma,
            "`,` between the MAP key and value types",
        )?;
        let value = self.parse_data_type()?;
        self.expect_punct(Punctuation::RParen, "`)` to close the MAP type")?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(DataType::Map {
            key: Box::new(key),
            value: Box::new(value),
            meta,
        })
    }

    /// Parse extended scalar type names. Each maps onto the existing
    /// canonical [`DataType`] shape via a new variant or spelling tag — no bespoke
    /// per-type representation. `DOUBLE PRECISION` is matched earlier in
    /// [`parse_data_type`](Self::parse_data_type), so a bare `DOUBLE` reaching here
    /// is unambiguously the extended bare floating type.
    fn try_parse_extended_scalar_type_name(
        &mut self,
        start: Span,
    ) -> ParseResult<Option<DataType<D::Ext>>> {
        if self.eat_contextual_keyword("TINYINT")? {
            let display_width = self.parse_optional_integer_display_width()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(DataType::TinyInt {
                display_width,
                meta,
            }));
        }
        if self.eat_contextual_keyword("MEDIUMINT")? {
            let display_width = self.parse_optional_integer_display_width()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(DataType::MediumInt {
                display_width,
                meta,
            }));
        }
        if self.eat_contextual_keyword("DOUBLE")? {
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(DataType::Double {
                spelling: DoubleTypeName::Double,
                meta,
            }));
        }
        if self.eat_contextual_keyword("DATETIME")? {
            // `DATETIME[(fsp)]` takes an optional fractional-seconds precision and no
            // time-zone suffix, so it reuses the timestamp shape with a fixed zone.
            let precision = self.parse_optional_single_type_modifier()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(DataType::Timestamp {
                spelling: TimestampTypeName::Datetime,
                precision,
                time_zone: TimeZone::Unspecified,
                meta,
            }));
        }
        if let Some(spelling) = self.eat_text_spelling()? {
            let charset = self.parse_optional_charset_annotation()?.map(Box::new);
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(DataType::Text {
                spelling,
                charset,
                meta,
            }));
        }
        if let Some(spelling) = self.eat_blob_spelling()? {
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(DataType::Blob { spelling, meta }));
        }
        Ok(None)
    }

    /// Parse a ClickHouse fixed-bit-width integer type name — the signed `Int8`/`Int16`/
    /// `Int32`/`Int64`/`Int128`/`Int256` family and the unsigned `UInt*` siblings — onto the
    /// shared [`DataType::FixedWidthInt`] shape (signedness + [`IntWidth`]). Each name is a
    /// single argument-less word, so this is a bare-name match (the `TINYINT`/extended-scalar
    /// precedent), never the keyword-then-`(` lookahead the wrapper/constructor types use.
    /// Returns `None` without consuming input when the leading word is none of the twelve, so
    /// the caller falls through to the user-defined-type path (the trivial off-gate boundary).
    fn try_parse_bit_width_integer_name(
        &mut self,
        start: Span,
    ) -> ParseResult<Option<DataType<D::Ext>>> {
        // (spelling, signed, width) — matched case-insensitively; each spelling is a whole
        // identifier token, so ordering is irrelevant (no prefix ambiguity between widths).
        const NAMES: &[(&str, bool, IntWidth)] = &[
            ("INT8", true, IntWidth::W8),
            ("INT16", true, IntWidth::W16),
            ("INT32", true, IntWidth::W32),
            ("INT64", true, IntWidth::W64),
            ("INT128", true, IntWidth::W128),
            ("INT256", true, IntWidth::W256),
            ("UINT8", false, IntWidth::W8),
            ("UINT16", false, IntWidth::W16),
            ("UINT32", false, IntWidth::W32),
            ("UINT64", false, IntWidth::W64),
            ("UINT128", false, IntWidth::W128),
            ("UINT256", false, IntWidth::W256),
        ];
        for &(name, signed, width) in NAMES {
            if self.eat_contextual_keyword(name)? {
                let meta = self.make_meta(start.union(self.preceding_span()));
                return Ok(Some(DataType::FixedWidthInt {
                    signed,
                    width,
                    meta,
                }));
            }
        }
        Ok(None)
    }

    /// Match a MySQL character-LOB size keyword (`TINYTEXT`/`MEDIUMTEXT`/`LONGTEXT`).
    /// The bare `TEXT` spelling is matched in the shared chain (PostgreSQL has it).
    fn eat_text_spelling(&mut self) -> ParseResult<Option<TextTypeName>> {
        if self.eat_contextual_keyword("TINYTEXT")? {
            Ok(Some(TextTypeName::TinyText))
        } else if self.eat_contextual_keyword("MEDIUMTEXT")? {
            Ok(Some(TextTypeName::MediumText))
        } else if self.eat_contextual_keyword("LONGTEXT")? {
            Ok(Some(TextTypeName::LongText))
        } else {
            Ok(None)
        }
    }

    /// Match a MySQL binary-LOB keyword (`BLOB`/`TINYBLOB`/`MEDIUMBLOB`/`LONGBLOB`).
    fn eat_blob_spelling(&mut self) -> ParseResult<Option<BlobTypeName>> {
        if self.eat_contextual_keyword("TINYBLOB")? {
            Ok(Some(BlobTypeName::TinyBlob))
        } else if self.eat_contextual_keyword("BLOB")? {
            Ok(Some(BlobTypeName::Blob))
        } else if self.eat_contextual_keyword("MEDIUMBLOB")? {
            Ok(Some(BlobTypeName::MediumBlob))
        } else if self.eat_contextual_keyword("LONGBLOB")? {
            Ok(Some(BlobTypeName::LongBlob))
        } else {
            Ok(None)
        }
    }

    /// Parse an `ENUM('a', ...)` / `SET('a', ...)` value-list type. The two share one
    /// value-list shape but stay distinct variants (set-membership vs single-value
    /// semantics) and independent recognition gates: MySQL admits both, DuckDB admits only
    /// `ENUM` (its `x::ENUM('a','b')` cast target), so each keyword is gated on its own flag.
    fn try_parse_enum_set_type(&mut self, start: Span) -> ParseResult<Option<DataType<D::Ext>>> {
        let types = self.features().type_name_syntax;
        if types.enum_type && self.eat_contextual_keyword("ENUM")? {
            let values = self.parse_enum_set_values()?;
            let charset = self.parse_optional_charset_annotation()?.map(Box::new);
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(DataType::Enum {
                values,
                charset,
                meta,
            }));
        }
        if types.set_type && self.eat_contextual_keyword("SET")? {
            let values = self.parse_enum_set_values()?;
            let charset = self.parse_optional_charset_annotation()?.map(Box::new);
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(DataType::Set {
                values,
                charset,
                meta,
            }));
        }
        Ok(None)
    }

    /// Parse a parenthesized, comma-separated list of string-literal members for an
    /// `ENUM`/`SET` type. MySQL requires at least one member.
    fn parse_enum_set_values(&mut self) -> ParseResult<ThinVec<Literal>> {
        self.expect_punct(Punctuation::LParen, "`(` to open the value list")?;
        let values = self.parse_comma_separated(Self::parse_enum_set_value)?;
        self.expect_punct(Punctuation::RParen, "`)` to close the value list")?;
        Ok(values)
    }

    /// Parse one `ENUM`/`SET` member: a string literal whose spelling round-trips
    /// from its span, like any other string constant. Shared with DuckDB's
    /// `CREATE TYPE … AS ENUM(<labels>)` production, whose labels obey the same
    /// string-constant rule.
    pub(super) fn parse_enum_set_value(&mut self) -> ParseResult<Literal> {
        match self.peek()? {
            Some(token) if token.kind == TokenKind::String => {
                self.advance()?;
                Ok(Literal {
                    kind: LiteralKind::String,
                    meta: self.make_meta(token.span),
                })
            }
            Some(_) | None => Err(self.unexpected("a string literal value")),
        }
    }

    /// Parse a standalone MySQL `SIGNED`/`UNSIGNED [INTEGER]` integer cast target, e.g.
    /// `CAST(x AS UNSIGNED)` or `CAST(x AS SIGNED INTEGER)`. The modifier names no base
    /// type, so the wrapper's `element` is `None`.
    ///
    /// The optional trailing integer keyword — `INTEGER` or its `INT` synonym — is
    /// semantically inert: `CAST(x AS SIGNED INTEGER)` is identical to `CAST(x AS SIGNED)`
    /// on mysql:8 (engine-measured, mysql-faithful-cast-type-production), so it folds onto
    /// the same standalone [`DataType::NumericModifier`] and the canonical render emits the
    /// bare `SIGNED`/`UNSIGNED` — a documented spelling trade, mirroring the
    /// inert plural-interval-unit fold in [`parse_optional_interval_fields`](Self::parse_optional_interval_fields).
    fn try_parse_standalone_sign(&mut self, start: Span) -> ParseResult<Option<DataType<D::Ext>>> {
        let signedness = self.eat_optional_signedness()?;
        if signedness == Signedness::Unspecified {
            return Ok(None);
        }
        // Consume (and drop) the inert `[INTEGER|INT]` tail — its span is folded into the
        // node's meta below, so the whole target still round-trips with trivia capture.
        let _ = self.eat_contextual_keyword("INTEGER")? || self.eat_contextual_keyword("INT")?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(Some(DataType::NumericModifier {
            element: None,
            signedness,
            zerofill: false,
            meta,
        }))
    }

    /// Wrap a numeric `base` type in its MySQL `SIGNED`/`UNSIGNED`/`ZEROFILL`
    /// modifier suffix, when the dialect recognizes it and a modifier is written.
    ///
    /// Only numeric types take these attributes, so a non-numeric `base` (or no
    /// modifier keyword) is returned unchanged. The canonical written order is
    /// `[SIGNED|UNSIGNED] [ZEROFILL]`; the reverse/repeated MySQL orderings are out
    /// of scope (the canonical form is what generated DDL emits).
    fn parse_optional_numeric_modifiers(
        &mut self,
        base: DataType<D::Ext>,
    ) -> ParseResult<DataType<D::Ext>> {
        if !self.features().type_name_syntax.numeric_modifiers || !is_numeric_type(&base) {
            return Ok(base);
        }
        let signedness = self.eat_optional_signedness()?;
        let zerofill = self.eat_contextual_keyword("ZEROFILL")?;
        if signedness == Signedness::Unspecified && !zerofill {
            return Ok(base);
        }
        let span = base.span().union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(DataType::NumericModifier {
            element: Some(Box::new(base)),
            signedness,
            zerofill,
            meta,
        })
    }

    /// Consume an optional MySQL `UNSIGNED`/`SIGNED` keyword, defaulting to
    /// [`Signedness::Unspecified`] when neither is written (no input consumed).
    fn eat_optional_signedness(&mut self) -> ParseResult<Signedness> {
        Ok(if self.eat_contextual_keyword("UNSIGNED")? {
            Signedness::Unsigned
        } else if self.eat_contextual_keyword("SIGNED")? {
            Signedness::Signed
        } else {
            Signedness::Unspecified
        })
    }

    /// Speculatively parse a typed temporal literal opened by `keyword`:
    /// `DATE '...'`, `TIME [(p)] [WITH|WITHOUT TIME ZONE] '...'`,
    /// `TIMESTAMP [(p)] [WITH|WITHOUT TIME ZONE] '...'`, or
    /// `INTERVAL [(p)] '...' [<fields> [(p)]]`.
    ///
    /// These are PostgreSQL's `ConstDatetime`/`ConstInterval` constants: a type
    /// keyword whose literal reading is selected only when a string constant follows
    /// the type prefix. Detection is speculative — on a non-match the
    /// cursor rewinds so the keyword falls back to its ordinary column/function
    /// reading. A genuinely malformed temporal literal (e.g. two interval precisions)
    /// is a hard error, matching PostgreSQL. The precision on `TIME`/`TIMESTAMP` is
    /// consumed (so the whole literal round-trips from its span) but not retained on
    /// the kind tag: it constrains display, not the value, and stays recoverable from
    /// the source text, so only the value-bearing time-zone flag is kept.
    pub(super) fn try_parse_temporal_literal(
        &mut self,
        keyword: Keyword,
    ) -> ParseResult<Option<LiteralKind>> {
        if !self.features().expression_syntax.typed_string_literals {
            // The dialect has no prefix-typed literal (SQLite): return without advancing
            // so the caller falls back to the ordinary column/function reading, exactly
            // as the speculative non-match rewinds to.
            return Ok(None);
        }
        let checkpoint = self.checkpoint();
        self.advance()?; // the temporal type keyword

        if keyword == Keyword::Interval {
            // MySQL has no first-class interval literal: every prefix-typed `INTERVAL '…'`
            // form is `ER_PARSE_ERROR` on mysql:8, so the literal path is off there
            // (`typed_interval_literal`). The valid MySQL operator-position forms are read
            // before this by `try_parse_mysql_interval_operator`; the spellings it declines
            // (unit-less, ANSI `TO`/precision) reach here and must reject, so we rewind and
            // let the caller fall back to `INTERVAL`'s ordinary column/name reading.
            if !self.features().expression_syntax.typed_interval_literal {
                self.rewind(checkpoint);
                return Ok(None);
            }
            // DuckDB's relaxed amount forms (`relaxed_interval_syntax`): an unquoted
            // integer or a parenthesized expression stands in for the quoted amount
            // string, each requiring a trailing unit. The amount round-trips from the
            // literal's span like the quoted string it replaces (ADR-0006), so nothing is
            // retained beyond the unit qualifier. A non-match leaves the cursor right after
            // `INTERVAL`, so the standard quoted-string path (and Lenient's leading-precision
            // `INTERVAL(p) '...'`) still runs below.
            if self.features().expression_syntax.relaxed_interval_syntax {
                if let Some(kind) = self.try_parse_relaxed_interval_amount()? {
                    return Ok(Some(kind));
                }
            }
            let leading_precision = self.parse_optional_single_type_modifier()?;
            if !self.peek_is_sconst()? {
                self.rewind(checkpoint);
                return Ok(None);
            }
            self.consume_temporal_value_string()?;
            let (fields, field_precision) = self.parse_optional_interval_fields()?;
            let precision = self.coalesce_interval_precision(leading_precision, field_precision)?;
            return Ok(Some(LiteralKind::Interval { fields, precision }));
        }

        let kind = match keyword {
            Keyword::Date => LiteralKind::Date,
            Keyword::Time => {
                self.parse_optional_single_type_modifier()?;
                LiteralKind::Time {
                    time_zone: self.parse_optional_time_zone()?,
                }
            }
            Keyword::Timestamp => {
                self.parse_optional_single_type_modifier()?;
                LiteralKind::Timestamp {
                    time_zone: self.parse_optional_time_zone()?,
                }
            }
            _ => unreachable!("try_parse_temporal_literal called with a non-temporal keyword"),
        };
        if !self.peek_is_sconst()? {
            self.rewind(checkpoint);
            return Ok(None);
        }
        self.consume_temporal_value_string()?;
        Ok(Some(kind))
    }

    /// Speculatively parse the MySQL operator-position interval `INTERVAL <expr> <unit>`
    /// ([`ExpressionSyntax::mysql_interval_operator`](crate::ast::dialect::ExpressionSyntax::mysql_interval_operator)) into an
    /// [`Expr::Interval`] node — MySQL's `Item_date_add_interval` operand (`d - INTERVAL 3 DAY`).
    ///
    /// The caller (the expression-primary dispatch) guarantees the flag is on and the cursor is
    /// on the `INTERVAL` keyword. The amount is an arbitrary expression bounded by the trailing
    /// unit keyword (never an operator, so the expression parse stops before it); the unit is
    /// mandatory and uses MySQL's underscore vocabulary only — no ANSI `TO` composite and no
    /// `(p)` precision, both `ER_PARSE_ERROR` on mysql:8.4.10. A form that is not a valid MySQL
    /// operator interval — a unit-less amount, or a simple unit trailed by an ANSI `TO` /
    /// precision — rewinds to the cursor's start position and returns `None` so the caller falls
    /// through to the typed-string interval literal path. Under Lenient that path owns those
    /// spellings; under MySQL it is off
    /// ([`ExpressionSyntax::typed_interval_literal`](crate::ast::dialect::ExpressionSyntax::typed_interval_literal)),
    /// so they reject — MySQL has no first-class interval literal.
    pub(super) fn try_parse_mysql_interval_operator(
        &mut self,
    ) -> ParseResult<Option<Expr<D::Ext>>> {
        let checkpoint = self.checkpoint();
        let start = self.current_span()?;
        self.advance()?; // INTERVAL
        let Ok(value) = self.parse_expr() else {
            self.rewind(checkpoint);
            return Ok(None);
        };
        let Some(unit) = self.try_parse_mysql_interval_unit()? else {
            self.rewind(checkpoint);
            return Ok(None);
        };
        if self.peek_is_contextual_keyword("TO")? || self.peek_is_punct(Punctuation::LParen)? {
            self.rewind(checkpoint);
            return Ok(None);
        }
        let span = start.union(self.preceding_span());
        Ok(Some(Expr::Interval {
            value: Box::new(value),
            unit,
            meta: self.make_meta(span),
        }))
    }

    /// DuckDB's relaxed interval-amount forms (`relaxed_interval_syntax`): an unquoted
    /// integer amount `INTERVAL <int> <unit>` or a parenthesized-expression amount
    /// `INTERVAL (<expr>) <unit>`, each with a required trailing unit. Both round-trip the
    /// amount from the literal's span, so only the unit qualifier is retained.
    /// Returns `None` (no net cursor movement) when the token after `INTERVAL` opens
    /// neither form, so the caller falls through to the standard quoted-string path.
    fn try_parse_relaxed_interval_amount(&mut self) -> ParseResult<Option<LiteralKind>> {
        // Unquoted integer amount. A number immediately after `INTERVAL` is unambiguously
        // the amount — no infix operator can precede it — so this commits (a float, which
        // DuckDB rejects, is declined here and re-surfaces as a parse error). A bare
        // `INTERVAL <int>` with no unit is a DuckDB binding error, so the missing unit is a
        // hard error rather than a rewind.
        if let Some(token) = self.peek()? {
            if token.kind == TokenKind::Number
                && number_literal_kind(self.span_text(token.span), self.float_as_decimal_enabled())
                    == LiteralKind::Integer
            {
                self.advance()?;
                return Ok(Some(self.finish_relaxed_interval_unit()?));
            }
        }
        // Parenthesized-expression amount. DuckDB has no ANSI leading-precision
        // `INTERVAL(p) '...'`, but Lenient keeps it, so this is speculative: on a `(expr)`
        // not followed by a unit (or an unparseable body) the cursor rewinds and the
        // standard precision path reads it.
        if self.peek_is_punct(Punctuation::LParen)? {
            let checkpoint = self.checkpoint();
            self.advance()?; // `(`
            if self.parse_expr().is_ok() && self.eat_punct(Punctuation::RParen)? {
                let (fields, precision) = self.parse_optional_interval_fields()?;
                if fields.is_some() {
                    return Ok(Some(LiteralKind::Interval { fields, precision }));
                }
            }
            self.rewind(checkpoint);
        }
        Ok(None)
    }

    /// Require and consume the trailing unit of a relaxed interval-amount form. A bare
    /// `INTERVAL <amount>` parses in DuckDB but is a *binding* error (no unit to scale by),
    /// so a missing unit is rejected here.
    fn finish_relaxed_interval_unit(&mut self) -> ParseResult<LiteralKind> {
        let (fields, precision) = self.parse_optional_interval_fields()?;
        if fields.is_none() {
            return Err(self.unexpected("an interval unit"));
        }
        Ok(LiteralKind::Interval { fields, precision })
    }

    pub(super) fn peek_is_string(&mut self) -> ParseResult<bool> {
        Ok(self
            .peek()?
            .is_some_and(|token| token.kind == TokenKind::String))
    }

    /// Whether the current token is a `String` whose spelling is an `Sconst` — the
    /// character-string constant admitted in a prefix-typed / temporal literal's *value*
    /// position, excluding the bit-string (`B'…'`/`X'…'`), national (`N'…'`), and
    /// charset-introducer (`_utf8'…'`) kinds. PostgreSQL's `ConstTypename Sconst` /
    /// `ConstDatetime Sconst` productions take an `Sconst` only, and all three engines that
    /// arm the prefix-typed literal reject the non-`Sconst` kinds here — measured on
    /// pg_query 6.1.1 (`SELECT DATE X'ab'`, `SELECT float8 B'1'` reject), MySQL 8.4.10
    /// (`DATE X'ab'`/`DATE B'1'`/`DATE N'x'`/`DATE _utf8'x'` all `ER_PARSE_ERROR`), and
    /// DuckDB (`float8 B'1'`/`float8 N'x'` reject though the bare bit/national strings lex).
    /// So the value-kind restriction is dialect-independent, a plain
    /// [`string_literal_is_sconst`] call rather than dialect data.
    pub(super) fn peek_is_sconst(&mut self) -> ParseResult<bool> {
        let Some(token) = self.peek()? else {
            return Ok(false);
        };
        Ok(token.kind == TokenKind::String && string_literal_is_sconst(self.span_text(token.span)))
    }

    /// Consume a temporal literal's value string, folding any SQL-standard
    /// adjacent-string continuation segments into it. PostgreSQL continues the embedded
    /// constant exactly like a bare string primary — `DATE '1998'`⏎`'-12-01'` is the one
    /// value `1998-12-01` — so the value's span covers every segment and the concatenated
    /// text is recovered at [`Literal::as_temporal_text`](crate::ast::Literal::as_temporal_text).
    /// The caller has confirmed a string token is current.
    fn consume_temporal_value_string(&mut self) -> ParseResult<()> {
        let value = self
            .advance()?
            .expect("caller confirmed a string token is present");
        let value_text = self.span_text(value.span);
        self.consume_string_continuations(value.span, value_text)?;
        Ok(())
    }

    fn parse_user_defined_data_type(&mut self, start: Span) -> ParseResult<DataType<D::Ext>> {
        // A user-defined type name is `type_function_name`: `unreserved ∪
        // type_func_name`, rejecting `col_name` (so `CAST(x AS coalesce)` is
        // rejected, matching PostgreSQL). Built-in spellings are matched
        // contextually before this fallback, so they bypass the gate. The same
        // type-name reject set parses the (possibly qualified) name, so the gate
        // that admits the head is the one that parses it.
        let type_reserved = self.features().reserved_type_name;
        match self.peek()? {
            Some(token) if self.token_admissible(token, type_reserved) => {
                let name = self.parse_object_name_with(type_reserved)?;
                let modifiers = self.parse_optional_type_modifier_list()?;
                let meta = self.make_meta(start.union(self.preceding_span()));
                Ok(DataType::UserDefined {
                    name,
                    modifiers,
                    meta,
                })
            }
            Some(_) | None => Err(self.unexpected("a data type")),
        }
    }

    fn parse_interval_type(&mut self, start: Span) -> ParseResult<DataType<D::Ext>> {
        let interval_precision = self.parse_optional_single_type_modifier()?;
        let (fields, field_precision) = self.parse_optional_interval_fields()?;
        let precision = self.coalesce_interval_precision(interval_precision, field_precision)?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(DataType::Interval {
            fields,
            precision,
            meta,
        })
    }

    pub(super) fn parse_optional_interval_fields(
        &mut self,
    ) -> ParseResult<(Option<IntervalFields>, Option<u32>)> {
        // DuckDB's plural unit spellings (`INTERVAL '1' hours`, `INTERVAL 3 DAYS`) fold
        // onto the singular qualifier; the plural `s` round-trips from the span (the tag
        // has no plural variant — the documented spelling trade). DuckDB has no `TO`
        // composite, so a plural is always the whole, simple qualifier. Checked first
        // because `DAYS` is a distinct word from `DAY` (no shared-prefix ambiguity).
        if self.features().expression_syntax.relaxed_interval_syntax {
            const PLURAL_UNITS: &[(&str, IntervalFields)] = &[
                ("YEARS", IntervalFields::Year),
                ("MONTHS", IntervalFields::Month),
                ("DAYS", IntervalFields::Day),
                ("HOURS", IntervalFields::Hour),
                ("MINUTES", IntervalFields::Minute),
                ("SECONDS", IntervalFields::Second),
            ];
            for (spelling, fields) in PLURAL_UNITS {
                if self.eat_contextual_keyword(spelling)? {
                    return Ok((Some(*fields), None));
                }
            }
            // DuckDB's extended units — `WEEK`/`QUARTER`/`DECADE`/`CENTURY`/`MILLENNIUM`/
            // `MILLISECOND`/`MICROSECOND` — beyond the ANSI qualifiers, as `INTERVAL
            // <amount> <unit>` multipliers. Unlike the standard units (whose singular is
            // always accepted below), BOTH the singular and plural spellings are gated
            // here so ANSI/PostgreSQL never admit them. Each is a whole, simple qualifier
            // with no `TO` composite and no precision (DuckDB rejects a trailing `(p)`),
            // matching the plural-unit trade above: the exact spelling round-trips from
            // the span, the tag folds to one variant.
            const EXTENDED_UNITS: &[(&str, IntervalFields)] = &[
                ("WEEK", IntervalFields::Week),
                ("WEEKS", IntervalFields::Week),
                ("QUARTER", IntervalFields::Quarter),
                ("QUARTERS", IntervalFields::Quarter),
                ("DECADE", IntervalFields::Decade),
                ("DECADES", IntervalFields::Decade),
                ("CENTURY", IntervalFields::Century),
                ("CENTURIES", IntervalFields::Century),
                ("MILLENNIUM", IntervalFields::Millennium),
                ("MILLENNIA", IntervalFields::Millennium),
                ("MILLISECOND", IntervalFields::Millisecond),
                ("MILLISECONDS", IntervalFields::Millisecond),
                ("MICROSECOND", IntervalFields::Microsecond),
                ("MICROSECONDS", IntervalFields::Microsecond),
            ];
            for (spelling, fields) in EXTENDED_UNITS {
                if self.eat_contextual_keyword(spelling)? {
                    return Ok((Some(*fields), None));
                }
            }
        }
        if self.eat_contextual_keyword("YEAR")? {
            let precision = self.parse_optional_single_type_modifier()?;
            if self.eat_contextual_keyword("TO")? {
                self.expect_contextual_keyword("MONTH")?;
                return Ok((Some(IntervalFields::YearToMonth), precision));
            }
            return Ok((Some(IntervalFields::Year), precision));
        }
        if self.eat_contextual_keyword("MONTH")? {
            return Ok((
                Some(IntervalFields::Month),
                self.parse_optional_single_type_modifier()?,
            ));
        }
        if self.eat_contextual_keyword("DAY")? {
            let leading_precision = self.parse_optional_single_type_modifier()?;
            if self.eat_contextual_keyword("TO")? {
                if self.eat_contextual_keyword("HOUR")? {
                    return Ok((Some(IntervalFields::DayToHour), leading_precision));
                }
                if self.eat_contextual_keyword("MINUTE")? {
                    return Ok((Some(IntervalFields::DayToMinute), leading_precision));
                }
                if self.eat_contextual_keyword("SECOND")? {
                    let second_precision = self.parse_optional_single_type_modifier()?;
                    return self.finish_interval_to_second(
                        IntervalFields::DayToSecond,
                        leading_precision,
                        second_precision,
                    );
                }
                return Err(self.unexpected("`HOUR`, `MINUTE`, or `SECOND`"));
            }
            return Ok((Some(IntervalFields::Day), leading_precision));
        }
        if self.eat_contextual_keyword("HOUR")? {
            let leading_precision = self.parse_optional_single_type_modifier()?;
            if self.eat_contextual_keyword("TO")? {
                if self.eat_contextual_keyword("MINUTE")? {
                    return Ok((Some(IntervalFields::HourToMinute), leading_precision));
                }
                if self.eat_contextual_keyword("SECOND")? {
                    let second_precision = self.parse_optional_single_type_modifier()?;
                    return self.finish_interval_to_second(
                        IntervalFields::HourToSecond,
                        leading_precision,
                        second_precision,
                    );
                }
                return Err(self.unexpected("`MINUTE` or `SECOND`"));
            }
            return Ok((Some(IntervalFields::Hour), leading_precision));
        }
        if self.eat_contextual_keyword("MINUTE")? {
            let leading_precision = self.parse_optional_single_type_modifier()?;
            if self.eat_contextual_keyword("TO")? {
                self.expect_contextual_keyword("SECOND")?;
                let second_precision = self.parse_optional_single_type_modifier()?;
                return self.finish_interval_to_second(
                    IntervalFields::MinuteToSecond,
                    leading_precision,
                    second_precision,
                );
            }
            return Ok((Some(IntervalFields::Minute), leading_precision));
        }
        if self.eat_contextual_keyword("SECOND")? {
            return Ok((
                Some(IntervalFields::Second),
                self.parse_optional_single_type_modifier()?,
            ));
        }
        Ok((None, None))
    }

    fn finish_interval_to_second(
        &mut self,
        fields: IntervalFields,
        leading_precision: Option<u32>,
        second_precision: Option<u32>,
    ) -> ParseResult<(Option<IntervalFields>, Option<u32>)> {
        let precision = self.coalesce_interval_precision(leading_precision, second_precision)?;
        Ok((Some(fields), precision))
    }

    /// Combine the two optional `INTERVAL` precisions into the single retained one.
    /// At most one may be written; two precisions (a leading and a field precision)
    /// is a hard error, matching PostgreSQL.
    fn coalesce_interval_precision(
        &mut self,
        a: Option<u32>,
        b: Option<u32>,
    ) -> ParseResult<Option<u32>> {
        match (a, b) {
            (Some(precision), None) | (None, Some(precision)) => Ok(Some(precision)),
            (None, None) => Ok(None),
            (Some(_), Some(_)) => Err(self.unexpected("only one interval precision")),
        }
    }

    pub(super) fn parse_optional_time_zone(&mut self) -> ParseResult<TimeZone> {
        if self.eat_keyword(Keyword::With)? {
            self.expect_contextual_keyword("TIME")?;
            self.expect_contextual_keyword("ZONE")?;
            Ok(TimeZone::WithTimeZone)
        } else if self.eat_contextual_keyword("WITHOUT")? {
            self.expect_contextual_keyword("TIME")?;
            self.expect_contextual_keyword("ZONE")?;
            Ok(TimeZone::WithoutTimeZone)
        } else {
            Ok(TimeZone::Unspecified)
        }
    }

    /// Parse the trailing array-type suffixes: the keyword `T ARRAY` / `T ARRAY[n]` and
    /// the bracket `T[]` / `T[n]`. The optional `[n]` bound is the DuckDB fixed-size
    /// `ARRAY` (a distinct type from the unbounded `LIST`); PostgreSQL accepts the bound
    /// but ignores it. Both spellings fold onto one canonical [`DataType::Array`] shape
    /// tagged by [`ArrayTypeSpelling`]. Suffixes chain (`INTEGER[][3]`).
    fn parse_array_suffixes(
        &mut self,
        mut data_type: DataType<D::Ext>,
    ) -> ParseResult<DataType<D::Ext>> {
        loop {
            if self.eat_contextual_keyword("ARRAY")? {
                let size = self.parse_optional_array_keyword_size()?;
                data_type = self.wrap_array_type(data_type, size, ArrayTypeSpelling::Keyword);
                continue;
            }
            if self.eat_punct(Punctuation::LBracket)? {
                let size = if self.peek_is_punct(Punctuation::RBracket)? {
                    None
                } else {
                    Some(self.parse_u32_type_modifier()?)
                };
                self.expect_punct(Punctuation::RBracket, "`]` to close the array suffix")?;
                data_type = self.wrap_array_type(data_type, size, ArrayTypeSpelling::Bracket);
                continue;
            }
            return Ok(data_type);
        }
    }

    /// After the `ARRAY` keyword, parse its optional fixed-size bound `[n]`
    /// (`T ARRAY[3]`). A bare `ARRAY` is the unbounded list, so no `[` yields `None`
    /// without consuming input. An empty `[]` is not a valid `ARRAY` bound (DuckDB
    /// rejects `T ARRAY[]`), so it is left for the bracket-suffix path.
    fn parse_optional_array_keyword_size(&mut self) -> ParseResult<Option<u32>> {
        if !self.peek_is_punct(Punctuation::LBracket)?
            || self.peek_nth_is_punct(1, Punctuation::RBracket)?
        {
            return Ok(None);
        }
        self.expect_punct(Punctuation::LBracket, "`[`")?;
        let size = self.parse_u32_type_modifier()?;
        self.expect_punct(Punctuation::RBracket, "`]` to close the array size")?;
        Ok(Some(size))
    }

    fn wrap_array_type(
        &mut self,
        element: DataType<D::Ext>,
        size: Option<u32>,
        spelling: ArrayTypeSpelling,
    ) -> DataType<D::Ext> {
        let span = element.span().union(self.preceding_span());
        let meta = self.make_meta(span);
        DataType::Array {
            element: Box::new(element),
            size,
            spelling,
            meta,
        }
    }

    fn parse_optional_precision_scale(&mut self) -> ParseResult<(Option<i32>, Option<i32>)> {
        if !self.eat_punct(Punctuation::LParen)? {
            return Ok((None, None));
        }
        // DuckDB's empty type-parameter parens `DECIMAL()`/`DEC()`/`NUMERIC()`: an empty
        // modifier list means the default precision/scale, which DuckDB normalizes to the
        // same `DECIMAL(18,3)` as a bare `DECIMAL` (probed on 1.5.4). It carries no value, so
        // it folds onto the bare `precision: None, scale: None` shape (the canonical render
        // then drops the parens). Gated by `empty_type_parens`, so ANSI/PostgreSQL/MySQL keep
        // requiring a precision and reject the empty form. The closing `)` is consumed here so
        // the node span still covers the written `()`.
        if self.features().type_name_syntax.empty_type_parens
            && self.eat_punct(Punctuation::RParen)?
        {
            return Ok((None, None));
        }
        let precision = self.parse_numeric_type_modifier()?;
        let scale = if self.eat_punct(Punctuation::Comma)? {
            Some(self.parse_numeric_type_modifier()?)
        } else {
            None
        };
        self.expect_punct(Punctuation::RParen, "`)` to close the type modifier")?;
        Ok((Some(precision), scale))
    }

    /// Parse one `numeric`/`decimal` precision/scale modifier. When
    /// [`TypeNameSyntax::signed_type_modifier`](crate::ast::dialect::TypeNameSyntax) is on
    /// (PostgreSQL/Lenient), an optional leading `-`/`+` sign is consumed — PostgreSQL parses
    /// the modifier arguments as a general expression list at raw parse, accepting a signed
    /// value (`numeric(5, -2)`) it validates only later. Off-dialect the sign is left
    /// unconsumed and surfaces as a clean parse error, so the modifier stays an unsigned integer
    /// there. The magnitude reuses [`parse_u32_type_modifier`](Self::parse_u32_type_modifier)
    /// and is cast to `i32` (PostgreSQL's typmod domain is far narrower than `u32`, so the
    /// widen-then-narrow never truncates a real modifier — an out-of-range magnitude already
    /// errored inside the `u32` parse).
    fn parse_numeric_type_modifier(&mut self) -> ParseResult<i32> {
        let negative = if self.features().type_name_syntax.signed_type_modifier {
            if self.eat_op(Operator::Minus)? {
                true
            } else {
                let _ = self.eat_op(Operator::Plus)?;
                false
            }
        } else {
            false
        };
        let magnitude = self.parse_u32_type_modifier()?;
        let signed = magnitude as i64;
        let signed = if negative { -signed } else { signed };
        i32::try_from(signed).map_err(|_| {
            self.error_at(
                self.preceding_span(),
                "an i32 numeric type modifier",
                format!("out-of-range type modifier {signed}"),
            )
        })
    }

    /// Parse an optional integer display width `(M)` (`INT(11)`), gated by
    /// [`TypeNameSyntax::integer_display_width`](crate::ast::dialect::TypeNameSyntax).
    /// When the dialect leaves the knob off (ANSI/PostgreSQL/DuckDB), no input is
    /// consumed, so the trailing `(` on a built-in integer surfaces as a clean parse
    /// error — exactly the reject `pg_query` gives for `INT(11)`. The width is a prefix
    /// arg on the type name; MySQL's `UNSIGNED`/`ZEROFILL` postfix is applied afterward
    /// by [`parse_optional_numeric_modifiers`](Self::parse_optional_numeric_modifiers).
    fn parse_optional_integer_display_width(&mut self) -> ParseResult<Option<u32>> {
        if !self.features().type_name_syntax.integer_display_width {
            return Ok(None);
        }
        self.parse_optional_single_type_modifier()
    }

    pub(super) fn parse_optional_single_type_modifier(&mut self) -> ParseResult<Option<u32>> {
        if !self.eat_punct(Punctuation::LParen)? {
            return Ok(None);
        }
        let modifier = self.parse_u32_type_modifier()?;
        self.expect_punct(Punctuation::RParen, "`)` to close the type modifier")?;
        Ok(Some(modifier))
    }

    /// Parse a user-defined type name's optional `(modifier, ...)` list into constant
    /// [`Literal`]s. An unsigned-integer modifier parses under every dialect; a
    /// string-literal modifier (DuckDB's `GEOMETRY('OGC:CRS84')`) parses only under
    /// [`TypeNameSyntax::string_type_modifiers`](crate::ast::dialect::TypeNameSyntax).
    fn parse_optional_type_modifier_list(&mut self) -> ParseResult<ThinVec<Literal>> {
        if !self.eat_punct(Punctuation::LParen)? {
            return Ok(ThinVec::new());
        }
        let modifiers = self.parse_comma_separated(Self::parse_literal_type_modifier)?;
        self.expect_punct(Punctuation::RParen, "`)` to close the type modifier")?;
        Ok(modifiers)
    }

    /// One constant type modifier: an unsigned integer (every dialect), or — under
    /// [`TypeNameSyntax::string_type_modifiers`](crate::ast::dialect::TypeNameSyntax) — a
    /// string literal (DuckDB's coordinate-system annotation). Both round-trip from their
    /// span; a non-constant (a nested list, an expression) stays a clean parse error,
    /// matching DuckDB's `Expected a constant as type modifier`.
    fn parse_literal_type_modifier(&mut self) -> ParseResult<Literal> {
        match self.peek()? {
            Some(token) if token.kind == TokenKind::Number => {
                // Reuse the unsigned-integer validation (all-ASCII-digit, in-range) and
                // record the exact spelling as a `Literal` so it round-trips.
                let span = token.span;
                self.parse_u32_type_modifier()?;
                Ok(Literal {
                    kind: LiteralKind::Integer,
                    meta: self.make_meta(span),
                })
            }
            Some(token)
                if token.kind == TokenKind::String
                    && self.features().type_name_syntax.string_type_modifiers =>
            {
                let span = token.span;
                self.advance()?;
                Ok(Literal {
                    kind: LiteralKind::String,
                    meta: self.make_meta(span),
                })
            }
            Some(_) | None => Err(self.unexpected("an unsigned integer type modifier")),
        }
    }

    pub(super) fn parse_u32_type_modifier(&mut self) -> ParseResult<u32> {
        match self.peek()? {
            Some(token) if token.kind == TokenKind::Number => {
                let text = self.span_text(token.span);
                if !text.bytes().all(|byte| byte.is_ascii_digit()) {
                    return Err(self.error_at(
                        token.span,
                        "an unsigned integer type modifier",
                        text.to_owned(),
                    ));
                }
                self.advance()?;
                text.parse::<u32>().map_err(|_| {
                    self.error_at(
                        token.span,
                        "a u32 type modifier",
                        format!("out-of-range type modifier {text}"),
                    )
                })
            }
            Some(_) | None => Err(self.unexpected("an unsigned integer type modifier")),
        }
    }

    /// Parse the `CHAR[ACTER] [VARYING]` tail after `NATIONAL` into its spelling.
    fn parse_national_character_spelling(&mut self) -> ParseResult<CharacterTypeName> {
        let character = if self.eat_contextual_keyword("CHARACTER")? {
            true
        } else if self.eat_contextual_keyword("CHAR")? {
            false
        } else {
            return Err(self.unexpected("`CHARACTER` or `CHAR` after `NATIONAL`"));
        };
        let varying = self.eat_contextual_keyword("VARYING")?;
        Ok(match (character, varying) {
            (true, false) => CharacterTypeName::NationalCharacter,
            (true, true) => CharacterTypeName::NationalCharacterVarying,
            (false, false) => CharacterTypeName::NationalChar,
            (false, true) => CharacterTypeName::NationalCharVarying,
        })
    }

    /// Parse MySQL's optional character-set type annotation — the grammar's
    /// `opt_charset_with_opt_binary` production, gated by
    /// [`TypeNameSyntax::character_set_annotation`](crate::ast::dialect::TypeNameSyntax). It
    /// follows the type name and its length, and is a distinct concern from the free-floating
    /// `COLLATE` column attribute (engine-measured on mysql:8.4: the annotation is an
    /// `ER_PARSE_ERROR` once a column attribute intervenes — `CHAR(5) NOT NULL CHARACTER SET
    /// x` — so it belongs to the type grammar).
    ///
    /// The production is a charset selector and/or a `BINARY` modifier in either order:
    /// `CHARACTER SET x [BINARY]` / `BINARY [CHARACTER SET x]`, the `ASCII`/`UNICODE`
    /// shortcuts with `BINARY` on either side, the standalone `BYTE`, and the bare `BINARY`.
    /// At most one selector and one `BINARY`; the reversed spellings fold onto the canonical
    /// [`CharsetAnnotation`] shape (the exact order recovers from the node span). Returns
    /// `None` without consuming input when the dialect leaves the knob off or no annotation
    /// keyword follows, so the caller (a cast/column type position) is unaffected.
    fn parse_optional_charset_annotation(&mut self) -> ParseResult<Option<CharsetAnnotation>> {
        if !self.features().type_name_syntax.character_set_annotation {
            return Ok(None);
        }
        let start = self.current_span()?;
        // Leading `BINARY`: `BINARY [ASCII | UNICODE | CHARACTER SET x]`. `BYTE` is not a
        // `BINARY`-suffix charset (grammar), so it is excluded from `eat_optional_bin_charset`.
        if self.eat_contextual_keyword("BINARY")? {
            let (charset, name) = match self.eat_optional_bin_charset()? {
                Some((kind, name)) => (Some(kind), name),
                None => (None, None),
            };
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(CharsetAnnotation {
                charset,
                name,
                binary: true,
                meta,
            }));
        }
        // Leading selector, with an optional trailing `BINARY`. `ASCII`/`UNICODE`/a named
        // charset admit the trailing `BINARY`; `BYTE` does not (grammar).
        if self.eat_contextual_keyword("ASCII")? {
            let binary = self.eat_contextual_keyword("BINARY")?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(CharsetAnnotation {
                charset: Some(Charset::Ascii),
                name: None,
                binary,
                meta,
            }));
        }
        if self.eat_contextual_keyword("UNICODE")? {
            let binary = self.eat_contextual_keyword("BINARY")?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(CharsetAnnotation {
                charset: Some(Charset::Unicode),
                name: None,
                binary,
                meta,
            }));
        }
        if self.eat_contextual_keyword("BYTE")? {
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(CharsetAnnotation {
                charset: Some(Charset::Byte),
                name: None,
                binary: false,
                meta,
            }));
        }
        if let Some(name) = self.eat_optional_character_set_name()? {
            let binary = self.eat_contextual_keyword("BINARY")?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(CharsetAnnotation {
                charset: Some(Charset::Named),
                name: Some(name),
                binary,
                meta,
            }));
        }
        Ok(None)
    }

    /// Consume the optional charset that may follow a leading `BINARY` — `ASCII`, `UNICODE`,
    /// or `CHARACTER SET x` (the grammar's `opt_bin_charset`; `BYTE` is not admissible here).
    /// Returns the selector kind paired with its name (`Some` only for a named charset).
    fn eat_optional_bin_charset(&mut self) -> ParseResult<Option<(Charset, Option<Ident>)>> {
        if self.eat_contextual_keyword("ASCII")? {
            return Ok(Some((Charset::Ascii, None)));
        }
        if self.eat_contextual_keyword("UNICODE")? {
            return Ok(Some((Charset::Unicode, None)));
        }
        if let Some(name) = self.eat_optional_character_set_name()? {
            return Ok(Some((Charset::Named, Some(name))));
        }
        Ok(None)
    }

    /// Match a `CHARACTER SET <name>` clause or its `CHARSET <name>` synonym, returning the
    /// charset name. No input is consumed when neither keyword leads. MySQL's `charset_name`
    /// is `ident_or_text`, so the name is a bare/backtick identifier or a quoted string
    /// (`CHARACTER SET 'utf8mb4'`, engine-accepted), each round-tripping from the [`Ident`]'s
    /// quote style.
    fn eat_optional_character_set_name(&mut self) -> ParseResult<Option<Ident>> {
        if self.peek_is_contextual_keyword("CHARACTER")?
            && self.peek_nth_is_contextual_keyword(1, "SET")?
        {
            self.advance()?; // CHARACTER
            self.advance()?; // SET
            return Ok(Some(self.parse_charset_name()?));
        }
        if self.eat_contextual_keyword("CHARSET")? {
            return Ok(Some(self.parse_charset_name()?));
        }
        Ok(None)
    }

    /// Parse a MySQL `charset_name` (`ident_or_text`): a quoted string
    /// (`CHARACTER SET 'utf8mb4'`) folded to an [`Ident`], else a bare/backtick identifier.
    pub(in crate::parser) fn parse_charset_name(&mut self) -> ParseResult<Ident> {
        if let Some(ident) = self.parse_string_alias_ident()? {
            return Ok(ident);
        }
        self.parse_ident()
    }

    /// Whether `ty` is an admissible MySQL `CAST`/`CONVERT` target under
    /// [`CallSyntax::restricted_cast_targets`](crate::ast::dialect::CallSyntax) — the
    /// shape-recognized `cast_type` set ([`is_mysql_cast_target_shape`]) plus the extended
    /// spellings that reach the parser as a [`DataType::UserDefined`] and so escape the
    /// shape-only predicate: `YEAR` and the spatial `cast_type` names (`POINT`,
    /// `LINESTRING`, `POLYGON`, `MULTIPOINT`, `MULTILINESTRING`, `MULTIPOLYGON`,
    /// `GEOMETRYCOLLECTION`, and the `GEOMCOLLECTION` alias). Those take no argument in cast
    /// position, so only a single unquoted name part with no type modifiers matches; a bare
    /// `GEOMETRY` (not a `cast_type`) and every user-named type still reject
    /// (engine-measured on mysql:8: `CAST(x AS YEAR)` / `AS POINT` / … accept,
    /// `AS GEOMETRY` / `AS INT` / `AS VARCHAR` reject). The name check runs only for the
    /// UserDefined fallthrough, so it costs nothing for the built-in-spelling common path.
    pub(super) fn is_mysql_cast_target(&self, ty: &DataType<D::Ext>) -> bool {
        if is_mysql_cast_target_shape(ty) {
            return true;
        }
        let DataType::UserDefined {
            name, modifiers, ..
        } = ty
        else {
            return false;
        };
        let [part] = name.0.as_slice() else {
            return false;
        };
        if part.quote != QuoteStyle::None || !modifiers.is_empty() {
            return false;
        }
        let text = self.span_text(part.meta.span);
        MYSQL_EXTENDED_CAST_TARGETS
            .iter()
            .any(|target| text.eq_ignore_ascii_case(target))
    }
}

/// MySQL `cast_type` names that reach the parser as a [`DataType::UserDefined`] — `YEAR`
/// (8.0.22+) and the spatial types added in 8.0.17. Bare `GEOMETRY` is deliberately absent:
/// it is a column type but not a `cast_type` (engine-measured-rejected on mysql:8), unlike
/// its `GEOMETRYCOLLECTION`/`GEOMCOLLECTION` sibling.
const MYSQL_EXTENDED_CAST_TARGETS: &[&str] = &[
    "YEAR",
    "POINT",
    "LINESTRING",
    "POLYGON",
    "MULTIPOINT",
    "MULTILINESTRING",
    "MULTIPOLYGON",
    "GEOMETRYCOLLECTION",
    "GEOMCOLLECTION",
];

/// Whether `data_type` is a numeric type that can carry MySQL's `SIGNED`/
/// `UNSIGNED`/`ZEROFILL` modifiers. The `NumericModifier` wrapper itself is
/// excluded so an already-modified type (or a standalone cast target) is never
/// wrapped twice.
fn is_numeric_type<X: Extension>(data_type: &DataType<X>) -> bool {
    matches!(
        data_type,
        DataType::TinyInt { .. }
            | DataType::SmallInt { .. }
            | DataType::MediumInt { .. }
            | DataType::Integer { .. }
            | DataType::BigInt { .. }
            | DataType::Decimal { .. }
            | DataType::Float { .. }
            | DataType::Real { .. }
            | DataType::Double { .. }
    )
}

/// Whether `ty` is one of MySQL's narrow `CAST`/`CONVERT` target types (the `cast_type`
/// grammar): `SIGNED`/`UNSIGNED [INTEGER|INT]` (the inert trailing keyword consumed
/// upstream by `try_parse_standalone_sign`, so the shape it leaves is the bare standalone
/// modifier), the non-varying char forms
/// (`CHAR`/`NCHAR`/`CHARACTER`/`NATIONAL CHAR`), `BINARY`, `DATE`, `DATETIME`, `TIME`,
/// `DECIMAL`/`DEC`, `DOUBLE`/`DOUBLE PRECISION`, `FLOAT`, `REAL`, or `JSON`
/// (engine-measured on mysql:8).
///
/// Every other type name — `INT`/`INTEGER`/`SMALLINT`/`BIGINT`/`TINYINT`,
/// `VARCHAR`/`TEXT`, `TIMESTAMP`, `NUMERIC`, `BOOLEAN`, `VARBINARY`/`BLOB`/`BIT`, and any
/// user-defined name — is a MySQL cast-position syntax error (though still valid as a
/// column type), so gating on this predicate under
/// [`CallSyntax::restricted_cast_targets`](crate::ast::dialect::CallSyntax) sheds the
/// over-acceptance. This shape-only check cannot see through a [`DataType::UserDefined`]
/// name, so MySQL's `YEAR` and spatial (`POINT`, …) cast targets — which parse as
/// user-defined names — are admitted by the parser method
/// [`Parser::is_mysql_cast_target`], which layers a name allowlist over this predicate.
fn is_mysql_cast_target_shape<X: Extension>(ty: &DataType<X>) -> bool {
    match ty {
        // `CAST(x AS SIGNED)` / `AS UNSIGNED` — the standalone integer sign modifier that
        // names no base type. A wrapped numeric (`INT UNSIGNED`) is not a cast target.
        DataType::NumericModifier { element: None, .. } => true,
        // The non-varying character forms; `VARCHAR`/`CHAR VARYING`/… reject.
        DataType::Character { spelling, .. } => matches!(
            spelling,
            CharacterTypeName::Char
                | CharacterTypeName::Character
                | CharacterTypeName::Nchar
                | CharacterTypeName::NationalChar
                | CharacterTypeName::NationalCharacter
        ),
        // `BINARY` only; `VARBINARY`/`BYTEA` reject.
        DataType::Binary {
            spelling: BinaryTypeName::Binary,
            ..
        } => true,
        DataType::Date { .. } => true,
        // `DATETIME` only; `TIMESTAMP`/`TIMESTAMPTZ` reject.
        DataType::Timestamp {
            spelling: TimestampTypeName::Datetime,
            ..
        } => true,
        // `TIME` only; `TIME WITH TIME ZONE` (`Timetz`) rejects.
        DataType::Time {
            spelling: TimeTypeName::Time,
            ..
        } => true,
        // `DECIMAL`/`DEC`; `NUMERIC` rejects.
        DataType::Decimal {
            spelling: DecimalTypeName::Decimal | DecimalTypeName::Dec,
            ..
        } => true,
        DataType::Double { .. } | DataType::Float { .. } | DataType::Real { .. } => true,
        DataType::Json { .. } => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::dialect::{FeatureDelta, FeatureSet, TypeNameSyntax};
    use crate::ast::{
        ArrayTypeSpelling, DataType, Expr, IntWidth, NoExt, Resolver as _, SelectItem, SetExpr,
        Statement, WrappedTypeKind,
    };
    use crate::parser::{FeatureDialect, Parsed, parse_with};
    use crate::render::Renderer;

    /// ANSI plus the ClickHouse `nullable_type` flag alone, isolating the gate from the
    /// rest of the (feature-gated) Lenient preset. Implements `RenderDialect` for the
    /// exact-text round-trip checks (no ClickHouse Tier-1 render target exists).
    const NULLABLE_DIALECT: FeatureDialect = {
        const FEATURES: FeatureSet =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.type_name_syntax(TypeNameSyntax {
                nullable_type: true,
                ..TypeNameSyntax::ANSI
            }));
        FeatureDialect {
            features: &FEATURES,
        }
    };

    /// The data type of the sole `CAST(x AS <type>)` projection in a one-statement parse.
    fn cast_type(parsed: &Parsed) -> &DataType<NoExt> {
        let Statement::Query { query, .. } = &parsed.statements()[0] else {
            panic!("expected a query statement");
        };
        let SetExpr::Select { select, .. } = &query.body else {
            panic!("expected a SELECT body");
        };
        let SelectItem::Expr { expr, .. } = &select.projection[0] else {
            panic!("expected a bare projection expression");
        };
        match expr {
            Expr::Cast { data_type, .. } => data_type,
            other => panic!("expected a CAST expression, got {other:?}"),
        }
    }

    /// Parse `CAST(x AS <ty>)` under the gate and return the parse.
    fn parse_cast(ty: &str) -> Parsed {
        let sql = format!("SELECT CAST(x AS {ty})");
        parse_with(&sql, crate::ParseConfig::new(NULLABLE_DIALECT))
            .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"))
    }

    #[test]
    fn nullable_wraps_an_inner_type_and_round_trips() {
        // `Nullable(String)` is the combinator over a user-defined inner name; the inner
        // spelling round-trips and the wrapper renders its ClickHouse mixed-case keyword.
        let parsed = parse_cast("Nullable(String)");
        let DataType::Wrapped {
            kind: WrappedTypeKind::Nullable,
            inner,
            ..
        } = cast_type(&parsed)
        else {
            panic!("expected a Nullable wrapper, got {:?}", cast_type(&parsed));
        };
        let DataType::UserDefined { name, .. } = inner.as_ref() else {
            panic!("expected a user-defined inner type, got {inner:?}");
        };
        assert_eq!(parsed.resolver().resolve(name.0[0].sym), "String");
        assert_eq!(
            Renderer::new(NULLABLE_DIALECT)
                .render_parsed(&parsed)
                .expect("renders"),
            "SELECT CAST(x AS Nullable(String))",
        );
    }

    #[test]
    fn nullable_inner_type_recurses() {
        // The inner is a full recursive type: a parametrized builtin (`DECIMAL(10, 2)`)
        // and an array-of-nullable both parse and round-trip, proving the wrapper recurses
        // through the shared type grammar (the `MAP(K, V)` precedent).
        for (input, rendered) in [
            (
                "Nullable(DECIMAL(10, 2))",
                "SELECT CAST(x AS Nullable(DECIMAL(10, 2)))",
            ),
            // `Nullable(String)[]` — the array suffix applies to the whole wrapper.
            ("Nullable(String)[]", "SELECT CAST(x AS Nullable(String)[])"),
        ] {
            let parsed = parse_cast(input);
            assert_eq!(
                Renderer::new(NULLABLE_DIALECT)
                    .render_parsed(&parsed)
                    .expect("renders"),
                rendered,
                "round-trip for {input:?}",
            );
        }
        // The array case is an `Array` of a `Wrapped`, not the reverse.
        let arr = parse_cast("Nullable(String)[]");
        assert!(
            matches!(
                cast_type(&arr),
                DataType::Array { element, spelling: ArrayTypeSpelling::Bracket, .. }
                    if matches!(element.as_ref(), DataType::Wrapped { .. })
            ),
            "expected Array<Wrapped>, got {:?}",
            cast_type(&arr),
        );
    }

    #[test]
    fn constrained_composability_is_parse_accepted() {
        // ClickHouse binds-rejects `Nullable(Nullable(T))` and `Nullable(Array(T))` at
        // type resolution (a `DB::Exception`, not a grammar error), so the parser accepts
        // both nestings and leaves the constraint to the binder — the parse-vs-bind split.
        for input in ["Nullable(Nullable(String))", "Nullable(String[])"] {
            let parsed = parse_cast(input);
            assert!(
                matches!(cast_type(&parsed), DataType::Wrapped { .. }),
                "{input:?} must parse-accept as a wrapper",
            );
        }
    }

    #[test]
    fn bare_nullable_is_an_ordinary_type_name() {
        // With the gate on, `Nullable` with no `(` is an ordinary user-defined type name,
        // never the combinator — only the keyword-immediately-followed-by-`(` form diverts.
        let parsed = parse_cast("Nullable");
        assert!(
            matches!(cast_type(&parsed), DataType::UserDefined { .. }),
            "bare `Nullable` is a user-defined name, got {:?}",
            cast_type(&parsed),
        );
    }

    #[test]
    fn nullable_constructor_is_rejected_without_the_gate() {
        use crate::dialect::Ansi;
        // Off-gate, `Nullable(String)` resolves its head to a user-defined type name whose
        // `(String)` numeric-modifier list then fails to parse — a clean rejection, no
        // wrapper. (A bare `Nullable` name is still accepted, unaffected by the gate.)
        parse_with(
            "SELECT CAST(x AS Nullable(String))",
            crate::ParseConfig::new(Ansi),
        )
        .expect_err("ANSI has no Nullable(T) combinator");
        parse_with("SELECT CAST(x AS Nullable)", crate::ParseConfig::new(Ansi))
            .expect("a bare `Nullable` name is unaffected by the gate");
    }

    /// ANSI plus the `low_cardinality_type` flag alone, isolating this gate from the
    /// sibling `nullable_type` and the rest of the Lenient preset.
    const LOWCARD_DIALECT: FeatureDialect = {
        const FEATURES: FeatureSet =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.type_name_syntax(TypeNameSyntax {
                low_cardinality_type: true,
                ..TypeNameSyntax::ANSI
            }));
        FeatureDialect {
            features: &FEATURES,
        }
    };

    /// ANSI plus both wrapper gates, so the canonical `LowCardinality(Nullable(T))`
    /// composition — one combinator wrapping the other — parses end to end.
    const LOWCARD_NULLABLE_DIALECT: FeatureDialect = {
        const FEATURES: FeatureSet =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.type_name_syntax(TypeNameSyntax {
                low_cardinality_type: true,
                nullable_type: true,
                ..TypeNameSyntax::ANSI
            }));
        FeatureDialect {
            features: &FEATURES,
        }
    };

    /// Parse `CAST(x AS <ty>)` under `dialect` and return the parse.
    fn parse_cast_with(ty: &str, dialect: FeatureDialect) -> Parsed {
        let sql = format!("SELECT CAST(x AS {ty})");
        parse_with(&sql, crate::ParseConfig::new(dialect))
            .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"))
    }

    #[test]
    fn low_cardinality_wraps_an_inner_type_and_round_trips() {
        // `LowCardinality(String)` is the combinator over a user-defined inner name; the
        // inner spelling round-trips and the wrapper renders its ClickHouse mixed-case
        // keyword.
        let parsed = parse_cast_with("LowCardinality(String)", LOWCARD_DIALECT);
        let DataType::Wrapped {
            kind: WrappedTypeKind::LowCardinality,
            inner,
            ..
        } = cast_type(&parsed)
        else {
            panic!(
                "expected a LowCardinality wrapper, got {:?}",
                cast_type(&parsed)
            );
        };
        let DataType::UserDefined { name, .. } = inner.as_ref() else {
            panic!("expected a user-defined inner type, got {inner:?}");
        };
        assert_eq!(parsed.resolver().resolve(name.0[0].sym), "String");
        assert_eq!(
            Renderer::new(LOWCARD_DIALECT)
                .render_parsed(&parsed)
                .expect("renders"),
            "SELECT CAST(x AS LowCardinality(String))",
        );
    }

    #[test]
    fn low_cardinality_inner_type_recurses() {
        // The inner is a full recursive type: a parametrized builtin (`DECIMAL(10, 2)`) and
        // an array-of-lowcardinality both round-trip through the shared type grammar.
        for (input, rendered) in [
            (
                "LowCardinality(DECIMAL(10, 2))",
                "SELECT CAST(x AS LowCardinality(DECIMAL(10, 2)))",
            ),
            (
                "LowCardinality(String)[]",
                "SELECT CAST(x AS LowCardinality(String)[])",
            ),
        ] {
            let parsed = parse_cast_with(input, LOWCARD_DIALECT);
            assert_eq!(
                Renderer::new(LOWCARD_DIALECT)
                    .render_parsed(&parsed)
                    .expect("renders"),
                rendered,
                "round-trip for {input:?}",
            );
        }
        // The array case is an `Array` of a `Wrapped`, not the reverse.
        let arr = parse_cast_with("LowCardinality(String)[]", LOWCARD_DIALECT);
        assert!(
            matches!(
                cast_type(&arr),
                DataType::Array { element, spelling: ArrayTypeSpelling::Bracket, .. }
                    if matches!(element.as_ref(), DataType::Wrapped { .. })
            ),
            "expected Array<Wrapped>, got {:?}",
            cast_type(&arr),
        );
    }

    #[test]
    fn low_cardinality_of_nullable_is_the_canonical_composition() {
        // `LowCardinality(Nullable(String))` — the canonical ClickHouse composition — nests
        // one combinator inside the other and round-trips when both gates are on. The outer
        // wrapper is `LowCardinality`, its inner a `Nullable` wrapper.
        let parsed = parse_cast_with("LowCardinality(Nullable(String))", LOWCARD_NULLABLE_DIALECT);
        let DataType::Wrapped {
            kind: WrappedTypeKind::LowCardinality,
            inner,
            ..
        } = cast_type(&parsed)
        else {
            panic!(
                "expected an outer LowCardinality, got {:?}",
                cast_type(&parsed)
            );
        };
        assert!(
            matches!(
                inner.as_ref(),
                DataType::Wrapped {
                    kind: WrappedTypeKind::Nullable,
                    ..
                }
            ),
            "expected an inner Nullable wrapper, got {inner:?}",
        );
        assert_eq!(
            Renderer::new(LOWCARD_NULLABLE_DIALECT)
                .render_parsed(&parsed)
                .expect("renders"),
            "SELECT CAST(x AS LowCardinality(Nullable(String)))",
        );
    }

    #[test]
    fn bare_low_cardinality_is_an_ordinary_type_name() {
        // With the gate on, `LowCardinality` with no `(` is an ordinary user-defined type
        // name, never the combinator — only the keyword-immediately-followed-by-`(` diverts.
        let parsed = parse_cast_with("LowCardinality", LOWCARD_DIALECT);
        assert!(
            matches!(cast_type(&parsed), DataType::UserDefined { .. }),
            "bare `LowCardinality` is a user-defined name, got {:?}",
            cast_type(&parsed),
        );
    }

    #[test]
    fn low_cardinality_constructor_is_rejected_without_the_gate() {
        use crate::dialect::Ansi;
        // Off-gate, `LowCardinality(String)` resolves its head to a user-defined type name
        // whose `(String)` numeric-modifier list then fails to parse — a clean rejection, no
        // wrapper. (A bare `LowCardinality` name is still accepted, unaffected by the gate.)
        parse_with(
            "SELECT CAST(x AS LowCardinality(String))",
            crate::ParseConfig::new(Ansi),
        )
        .expect_err("ANSI has no LowCardinality(T) combinator");
        parse_with(
            "SELECT CAST(x AS LowCardinality)",
            crate::ParseConfig::new(Ansi),
        )
        .expect("a bare `LowCardinality` name is unaffected by the gate");
    }

    /// ANSI plus the `fixed_string_type` flag alone, isolating this gate from the wrapper
    /// siblings and the rest of the Lenient preset.
    const FIXEDSTRING_DIALECT: FeatureDialect = {
        const FEATURES: FeatureSet =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.type_name_syntax(TypeNameSyntax {
                fixed_string_type: true,
                ..TypeNameSyntax::ANSI
            }));
        FeatureDialect {
            features: &FEATURES,
        }
    };

    /// ANSI plus `fixed_string_type` and both wrapper gates, so `LowCardinality(FixedString(N))`
    /// and `Nullable(FixedString(N))` — a scalar-argument type nested inside a single-inner-type
    /// wrapper — parse end to end.
    const FIXEDSTRING_WRAPPED_DIALECT: FeatureDialect = {
        const FEATURES: FeatureSet =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.type_name_syntax(TypeNameSyntax {
                fixed_string_type: true,
                low_cardinality_type: true,
                nullable_type: true,
                ..TypeNameSyntax::ANSI
            }));
        FeatureDialect {
            features: &FEATURES,
        }
    };

    #[test]
    fn fixed_string_carries_its_length_and_round_trips() {
        // `FixedString(16)` is a scalar-argument type, not a wrapper: it carries a mandatory
        // length that is preserved and rendered under ClickHouse's mixed-case spelling.
        let parsed = parse_cast_with("FixedString(16)", FIXEDSTRING_DIALECT);
        let DataType::FixedString { length, .. } = cast_type(&parsed) else {
            panic!("expected a FixedString type, got {:?}", cast_type(&parsed));
        };
        assert_eq!(*length, 16);
        assert_eq!(
            Renderer::new(FIXEDSTRING_DIALECT)
                .render_parsed(&parsed)
                .expect("renders"),
            "SELECT CAST(x AS FixedString(16))",
        );
    }

    #[test]
    fn fixed_string_nests_inside_the_wrappers() {
        // `FixedString(N)` is a full type, so it nests as the inner of the ClickHouse
        // wrappers — the canonical `LowCardinality(FixedString(16))` and `Nullable(FixedString(8))`
        // both round-trip with the length preserved.
        for (input, rendered) in [
            (
                "LowCardinality(FixedString(16))",
                "SELECT CAST(x AS LowCardinality(FixedString(16)))",
            ),
            (
                "Nullable(FixedString(8))",
                "SELECT CAST(x AS Nullable(FixedString(8)))",
            ),
        ] {
            let parsed = parse_cast_with(input, FIXEDSTRING_WRAPPED_DIALECT);
            let DataType::Wrapped { inner, .. } = cast_type(&parsed) else {
                panic!(
                    "expected a wrapper over FixedString, got {:?}",
                    cast_type(&parsed)
                );
            };
            assert!(
                matches!(inner.as_ref(), DataType::FixedString { length: 16 | 8, .. }),
                "expected an inner FixedString, got {inner:?}",
            );
            assert_eq!(
                Renderer::new(FIXEDSTRING_WRAPPED_DIALECT)
                    .render_parsed(&parsed)
                    .expect("renders"),
                rendered,
                "round-trip for {input:?}",
            );
        }
    }

    #[test]
    fn bare_fixed_string_is_an_ordinary_type_name() {
        // With the gate on, `FixedString` with no `(` is an ordinary user-defined type name,
        // never the constructor — only the keyword-immediately-followed-by-`(` form diverts.
        let parsed = parse_cast_with("FixedString", FIXEDSTRING_DIALECT);
        assert!(
            matches!(cast_type(&parsed), DataType::UserDefined { .. }),
            "bare `FixedString` is a user-defined name, got {:?}",
            cast_type(&parsed),
        );
    }

    #[test]
    fn fixed_string_constructor_is_rejected_without_the_gate() {
        use crate::dialect::Ansi;
        // Off-gate, `FixedString(16)` resolves its head to a user-defined type name whose
        // `(16)` numeric-modifier list then... parses (`FixedString` is a valid user-defined
        // name with a `(16)` modifier), so off-gate it is a `UserDefined`, never the ClickHouse
        // constructor. A bare `FixedString` name is likewise unaffected by the gate.
        let parsed = parse_with(
            "SELECT CAST(x AS FixedString(16))",
            crate::ParseConfig::new(Ansi),
        )
        .expect("`FixedString(16)` off-gate is a user-defined name with a modifier");
        assert!(
            matches!(cast_type(&parsed), DataType::UserDefined { .. }),
            "off-gate `FixedString(16)` must be a user-defined type, got {:?}",
            cast_type(&parsed),
        );
        parse_with(
            "SELECT CAST(x AS FixedString)",
            crate::ParseConfig::new(Ansi),
        )
        .expect("a bare `FixedString` name is unaffected by the gate");
    }

    /// ANSI plus the `datetime64_type` flag alone, isolating this gate from the rest of the
    /// Lenient preset.
    const DATETIME64_DIALECT: FeatureDialect = {
        const FEATURES: FeatureSet =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.type_name_syntax(TypeNameSyntax {
                datetime64_type: true,
                ..TypeNameSyntax::ANSI
            }));
        FeatureDialect {
            features: &FEATURES,
        }
    };

    /// ANSI plus `datetime64_type` and `nullable_type`, so `Nullable(DateTime64(3))` — a
    /// scalar-argument type nested inside a single-inner-type wrapper — parses end to end.
    const DATETIME64_WRAPPED_DIALECT: FeatureDialect = {
        const FEATURES: FeatureSet =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.type_name_syntax(TypeNameSyntax {
                datetime64_type: true,
                nullable_type: true,
                ..TypeNameSyntax::ANSI
            }));
        FeatureDialect {
            features: &FEATURES,
        }
    };

    #[test]
    fn datetime64_round_trips_with_and_without_timezone() {
        // Precision-only: mandatory `P` is carried, no timezone.
        let parsed = parse_cast_with("DateTime64(3)", DATETIME64_DIALECT);
        let DataType::DateTime64 {
            precision,
            timezone,
            ..
        } = cast_type(&parsed)
        else {
            panic!("expected a DateTime64 type, got {:?}", cast_type(&parsed));
        };
        assert_eq!(*precision, 3);
        assert!(timezone.is_none(), "no timezone argument was written");
        assert_eq!(
            Renderer::new(DATETIME64_DIALECT)
                .render_parsed(&parsed)
                .expect("renders"),
            "SELECT CAST(x AS DateTime64(3))",
        );

        // Precision plus the optional single-quoted timezone, whose exact source spelling
        // round-trips.
        let parsed = parse_cast_with("DateTime64(3, 'UTC')", DATETIME64_DIALECT);
        let DataType::DateTime64 {
            precision,
            timezone,
            ..
        } = cast_type(&parsed)
        else {
            panic!("expected a DateTime64 type, got {:?}", cast_type(&parsed));
        };
        assert_eq!(*precision, 3);
        assert!(timezone.is_some(), "the timezone argument was written");
        assert_eq!(
            Renderer::new(DATETIME64_DIALECT)
                .render_parsed(&parsed)
                .expect("renders"),
            "SELECT CAST(x AS DateTime64(3, 'UTC'))",
        );
    }

    #[test]
    fn datetime64_nests_inside_nullable() {
        // `DateTime64(P[, 'tz'])` is a full type, so it nests as the inner of the ClickHouse
        // `Nullable(T)` wrapper — both the precision-only and the timezone forms round-trip.
        for (input, rendered) in [
            (
                "Nullable(DateTime64(3))",
                "SELECT CAST(x AS Nullable(DateTime64(3)))",
            ),
            (
                "Nullable(DateTime64(9, 'Europe/Moscow'))",
                "SELECT CAST(x AS Nullable(DateTime64(9, 'Europe/Moscow')))",
            ),
        ] {
            let parsed = parse_cast_with(input, DATETIME64_WRAPPED_DIALECT);
            let DataType::Wrapped { inner, .. } = cast_type(&parsed) else {
                panic!(
                    "expected a wrapper over DateTime64, got {:?}",
                    cast_type(&parsed)
                );
            };
            assert!(
                matches!(inner.as_ref(), DataType::DateTime64 { .. }),
                "expected an inner DateTime64, got {inner:?}",
            );
            assert_eq!(
                Renderer::new(DATETIME64_WRAPPED_DIALECT)
                    .render_parsed(&parsed)
                    .expect("renders"),
                rendered,
                "round-trip for {input:?}",
            );
        }
    }

    #[test]
    fn bare_datetime64_is_an_ordinary_type_name() {
        // With the gate on, `DateTime64` with no `(` is an ordinary user-defined type name,
        // never the constructor — only the keyword-immediately-followed-by-`(` form diverts.
        let parsed = parse_cast_with("DateTime64", DATETIME64_DIALECT);
        assert!(
            matches!(cast_type(&parsed), DataType::UserDefined { .. }),
            "bare `DateTime64` is a user-defined name, got {:?}",
            cast_type(&parsed),
        );
    }

    #[test]
    fn datetime64_constructor_off_gate_boundary_is_asymmetric() {
        use crate::dialect::Ansi;
        // The KEY off-gate boundary. `DateTime64(3)` resolves its head to a user-defined type
        // name whose `(3)` numeric-modifier list then parses, so off-gate it is a
        // `UserDefined`, exactly like `FixedString(16)`.
        let parsed = parse_with(
            "SELECT CAST(x AS DateTime64(3))",
            crate::ParseConfig::new(Ansi),
        )
        .expect("`DateTime64(3)` off-gate is a user-defined name with a numeric modifier");
        assert!(
            matches!(cast_type(&parsed), DataType::UserDefined { .. }),
            "off-gate `DateTime64(3)` must be a user-defined type, got {:?}",
            cast_type(&parsed),
        );
        // But `DateTime64(3, 'UTC')` off-gate parse-*rejects*: the user-defined modifier list
        // is `u32`-only, so the string second argument `'UTC'` has no grammar to fit — unlike
        // `FixedString(16)`, whose whole argument is numeric. This asymmetry is why the flag
        // buys real coverage on the timezone form even though the bare-precision form already
        // parse-accepts off-gate.
        parse_with(
            "SELECT CAST(x AS DateTime64(3, 'UTC'))",
            crate::ParseConfig::new(Ansi),
        )
        .expect_err(
            "off-gate `DateTime64(3, 'UTC')` must reject: a string does not fit the u32 \
             modifier grammar",
        );
        // A bare `DateTime64` name is likewise unaffected by the gate.
        parse_with(
            "SELECT CAST(x AS DateTime64)",
            crate::ParseConfig::new(Ansi),
        )
        .expect("a bare `DateTime64` name is unaffected by the gate");
    }

    /// ANSI plus the `nested_type` flag alone, isolating this gate from the rest of the
    /// Lenient preset.
    const NESTED_DIALECT: FeatureDialect = {
        const FEATURES: FeatureSet =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.type_name_syntax(TypeNameSyntax {
                nested_type: true,
                ..TypeNameSyntax::ANSI
            }));
        FeatureDialect {
            features: &FEATURES,
        }
    };

    /// ANSI plus the `bit_width_integer_names` flag alone, isolating this bare-name gate from
    /// the rest of the Lenient preset.
    const BIT_WIDTH_INT_DIALECT: FeatureDialect = {
        const FEATURES: FeatureSet =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.type_name_syntax(TypeNameSyntax {
                bit_width_integer_names: true,
                ..TypeNameSyntax::ANSI
            }));
        FeatureDialect {
            features: &FEATURES,
        }
    };

    /// ANSI plus `nested_type` and `nullable_type`, so `Nullable(Nested(...))` — the composite
    /// nested inside a single-inner-type wrapper — parses end to end (the wrapper accepts any
    /// single inner type; ClickHouse's composability constraints are a bind concern).
    const NESTED_WRAPPED_DIALECT: FeatureDialect = {
        const FEATURES: FeatureSet =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.type_name_syntax(TypeNameSyntax {
                nested_type: true,
                nullable_type: true,
                ..TypeNameSyntax::ANSI
            }));
        FeatureDialect {
            features: &FEATURES,
        }
    };

    /// ANSI plus `bit_width_integer_names` and `nullable_type`, so `Nullable(Int256)` — a
    /// bit-width integer name nested inside a single-inner-type wrapper — parses end to end.
    const BIT_WIDTH_INT_NULLABLE_DIALECT: FeatureDialect = {
        const FEATURES: FeatureSet =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.type_name_syntax(TypeNameSyntax {
                bit_width_integer_names: true,
                nullable_type: true,
                ..TypeNameSyntax::ANSI
            }));
        FeatureDialect {
            features: &FEATURES,
        }
    };

    #[test]
    fn nested_parses_named_fields_and_round_trips() {
        // `Nested(a UInt8, b String)` carries a named-field list; each field's name and its
        // (user-defined) inner type round-trip, and the wrapper renders its ClickHouse
        // mixed-case keyword.
        let parsed = parse_cast_with("Nested(a UInt8, b String)", NESTED_DIALECT);
        let DataType::Nested { fields, .. } = cast_type(&parsed) else {
            panic!("expected a Nested type, got {:?}", cast_type(&parsed));
        };
        assert_eq!(fields.len(), 2);
        assert_eq!(parsed.resolver().resolve(fields[0].name.sym), "a");
        assert_eq!(parsed.resolver().resolve(fields[1].name.sym), "b");
        assert!(
            matches!(fields[0].ty, DataType::UserDefined { .. }),
            "field type is the user-defined inner name, got {:?}",
            fields[0].ty,
        );
        assert_eq!(
            Renderer::new(NESTED_DIALECT)
                .render_parsed(&parsed)
                .expect("renders"),
            "SELECT CAST(x AS Nested(a UInt8, b String))",
        );
    }

    #[test]
    fn nested_field_types_recurse() {
        // A field type is a full recursive type: a parametrized builtin (`DECIMAL(10, 2)`), a
        // nested `Nested(...)` (ClickHouse's arbitrary nesting is a setting/bind concern, so it
        // parse-accepts), and an array-suffixed field all round-trip.
        for (input, rendered) in [
            (
                "Nested(d DECIMAL(10, 2))",
                "SELECT CAST(x AS Nested(d DECIMAL(10, 2)))",
            ),
            (
                "Nested(child Nested(y UInt8))",
                "SELECT CAST(x AS Nested(child Nested(y UInt8)))",
            ),
            (
                "Nested(tags String[])",
                "SELECT CAST(x AS Nested(tags String[]))",
            ),
        ] {
            let parsed = parse_cast_with(input, NESTED_DIALECT);
            assert!(
                matches!(cast_type(&parsed), DataType::Nested { .. }),
                "{input:?} must parse as a Nested type",
            );
            assert_eq!(
                Renderer::new(NESTED_DIALECT)
                    .render_parsed(&parsed)
                    .expect("renders"),
                rendered,
                "round-trip for {input:?}",
            );
        }
    }

    #[test]
    fn bit_width_integer_names_round_trip() {
        // The ticket-named `Int256`/`UInt256` pair plus a sample of the narrower widths — each
        // carries its signedness and width and round-trips under ClickHouse's mixed-case
        // spelling (`Int256`, never `INT256`). Mixed-case input is matched case-insensitively.
        for (input, signed, width, rendered) in [
            ("Int256", true, IntWidth::W256, "SELECT CAST(x AS Int256)"),
            (
                "UInt256",
                false,
                IntWidth::W256,
                "SELECT CAST(x AS UInt256)",
            ),
            ("Int8", true, IntWidth::W8, "SELECT CAST(x AS Int8)"),
            ("UInt64", false, IntWidth::W64, "SELECT CAST(x AS UInt64)"),
            // Lower-case input still canonicalizes to the mixed-case spelling.
            ("int128", true, IntWidth::W128, "SELECT CAST(x AS Int128)"),
        ] {
            let parsed = parse_cast_with(input, BIT_WIDTH_INT_DIALECT);
            let DataType::FixedWidthInt {
                signed: got_signed,
                width: got_width,
                ..
            } = cast_type(&parsed)
            else {
                panic!(
                    "expected a FixedWidthInt type for {input:?}, got {:?}",
                    cast_type(&parsed)
                );
            };
            assert_eq!(*got_signed, signed, "signedness for {input:?}");
            assert_eq!(*got_width, width, "width for {input:?}");
            assert_eq!(
                Renderer::new(BIT_WIDTH_INT_DIALECT)
                    .render_parsed(&parsed)
                    .expect("renders"),
                rendered,
                "round-trip for {input:?}",
            );
        }
    }

    #[test]
    fn nested_composes_with_wrapper_and_array_suffix() {
        // `Nested(...)` is a full type, so it nests as the inner of the `Nullable(T)` wrapper
        // and takes an array suffix — both parse-accept per the wrapper/suffix rules and
        // round-trip.
        let parsed = parse_cast_with("Nullable(Nested(a UInt8))", NESTED_WRAPPED_DIALECT);
        let DataType::Wrapped { inner, .. } = cast_type(&parsed) else {
            panic!(
                "expected a wrapper over Nested, got {:?}",
                cast_type(&parsed)
            );
        };
        assert!(
            matches!(inner.as_ref(), DataType::Nested { .. }),
            "expected an inner Nested, got {inner:?}",
        );
        assert_eq!(
            Renderer::new(NESTED_WRAPPED_DIALECT)
                .render_parsed(&parsed)
                .expect("renders"),
            "SELECT CAST(x AS Nullable(Nested(a UInt8)))",
        );

        // `Nested(a UInt8)[]` — the array suffix applies to the whole composite.
        let parsed = parse_cast_with("Nested(a UInt8)[]", NESTED_DIALECT);
        assert!(
            matches!(
                cast_type(&parsed),
                DataType::Array { element, spelling: ArrayTypeSpelling::Bracket, .. }
                    if matches!(element.as_ref(), DataType::Nested { .. })
            ),
            "expected Array<Nested>, got {:?}",
            cast_type(&parsed),
        );
    }

    #[test]
    fn int256_nests_inside_nullable() {
        // A bit-width integer name is a full type, so it nests as the inner of the ClickHouse
        // `Nullable(T)` wrapper — the canonical `Nullable(Int256)` round-trips.
        let parsed = parse_cast_with("Nullable(Int256)", BIT_WIDTH_INT_NULLABLE_DIALECT);
        let DataType::Wrapped { inner, .. } = cast_type(&parsed) else {
            panic!(
                "expected a wrapper over Int256, got {:?}",
                cast_type(&parsed)
            );
        };
        assert!(
            matches!(
                inner.as_ref(),
                DataType::FixedWidthInt {
                    signed: true,
                    width: IntWidth::W256,
                    ..
                }
            ),
            "expected an inner signed 256-bit integer, got {inner:?}",
        );
        assert_eq!(
            Renderer::new(BIT_WIDTH_INT_NULLABLE_DIALECT)
                .render_parsed(&parsed)
                .expect("renders"),
            "SELECT CAST(x AS Nullable(Int256))",
        );
    }

    #[test]
    fn bare_nested_is_an_ordinary_type_name() {
        // With the gate on, `Nested` with no `(` is an ordinary user-defined type name, never
        // the constructor — only the keyword-immediately-followed-by-`(` form diverts.
        let parsed = parse_cast_with("Nested", NESTED_DIALECT);
        assert!(
            matches!(cast_type(&parsed), DataType::UserDefined { .. }),
            "bare `Nested` is a user-defined name, got {:?}",
            cast_type(&parsed),
        );
    }

    #[test]
    fn nested_constructor_is_rejected_without_the_gate() {
        use crate::dialect::Ansi;
        // Off-gate, `Nested(a UInt8)` resolves its head to a user-defined type name whose
        // modifier list is `u32`-only, so the two-word `a UInt8` field has no grammar to fit —
        // a clean rejection, no composite (the wrapper off-gate reject, unlike the asymmetric
        // `DateTime64(3)` numeric-modifier accept). A bare `Nested` name is unaffected.
        parse_with(
            "SELECT CAST(x AS Nested(a UInt8))",
            crate::ParseConfig::new(Ansi),
        )
        .expect_err("ANSI has no Nested(...) constructor and `a UInt8` is not a u32 modifier");
        parse_with("SELECT CAST(x AS Nested)", crate::ParseConfig::new(Ansi))
            .expect("a bare `Nested` name is unaffected by the gate");
    }

    #[test]
    fn bit_width_integer_names_off_gate_are_user_defined() {
        use crate::dialect::Ansi;
        // The off-gate boundary is trivial: the names take no arguments, so with the gate off a
        // bare `Int256`/`UInt256` is simply an ordinary user-defined type name (like a bare
        // `Nullable`), never a parse error.
        for input in ["Int256", "UInt256"] {
            let parsed = parse_with(
                &format!("SELECT CAST(x AS {input})"),
                crate::ParseConfig::new(Ansi),
            )
            .unwrap_or_else(|_| panic!("off-gate `{input}` is a user-defined name"));
            assert!(
                matches!(cast_type(&parsed), DataType::UserDefined { .. }),
                "off-gate `{input}` must be a user-defined type, got {:?}",
                cast_type(&parsed),
            );
        }
    }

    #[test]
    fn bigquery_angle_bracket_types_parse_and_gate() {
        use crate::ast::{ArrayTypeSpelling, DataType, StructTypeSpelling};
        use crate::dialect::{Ansi, BigQuery};

        // ARRAY<T>
        let parsed = parse_with(
            "SELECT CAST(x AS ARRAY<INT64>)",
            crate::ParseConfig::new(BigQuery),
        )
        .expect("ARRAY<> type");
        assert!(
            matches!(
                cast_type(&parsed),
                DataType::Array {
                    spelling: ArrayTypeSpelling::AngleBracket,
                    ..
                }
            ),
            "expected ARRAY angle spelling, got {:?}",
            cast_type(&parsed),
        );

        // STRUCT<field type>
        let parsed = parse_with(
            "SELECT CAST(x AS STRUCT<a INT64, b STRING>)",
            crate::ParseConfig::new(BigQuery),
        )
        .expect("STRUCT<> type");
        assert!(
            matches!(
                cast_type(&parsed),
                DataType::Struct {
                    spelling: StructTypeSpelling::AngleBracket,
                    fields,
                    ..
                } if fields.len() == 2
            ),
            "expected STRUCT angle with 2 fields, got {:?}",
            cast_type(&parsed),
        );

        // Nested STRUCT with >> closer
        parse_with(
            "SELECT CAST(x AS STRUCT<a STRUCT<b INT64>>)",
            crate::ParseConfig::new(BigQuery),
        )
        .expect("nested STRUCT<> with >>");

        // Column definitions
        parse_with(
            "CREATE TABLE t (a ARRAY<STRING>)",
            crate::ParseConfig::new(BigQuery),
        )
        .expect("column ARRAY<>");
        parse_with(
            "CREATE TABLE t (s STRUCT<x INT64>)",
            crate::ParseConfig::new(BigQuery),
        )
        .expect("column STRUCT<>");

        // Gate: ANSI rejects
        parse_with(
            "SELECT CAST(x AS ARRAY<INT64>)",
            crate::ParseConfig::new(Ansi),
        )
        .expect_err("ANSI has no angle-bracket types");
        parse_with(
            "SELECT CAST(x AS STRUCT<a INT64>)",
            crate::ParseConfig::new(Ansi),
        )
        .expect_err("ANSI has no angle-bracket STRUCT");
    }
}
