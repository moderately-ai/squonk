// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Feature-set self-consistency registries. Three sibling checkers surface the
//! combinations the per-feature model cannot make independent:
//!
//! - [`FeatureSet::lexical_conflict`] — two features that both claim the *same*
//!   context-free tokenizer trigger (a soundness hazard: fixed precedence silently
//!   *mis-parses* one of them).
//! - [`FeatureSet::feature_dependencies`] — a grammar flag set without the base flag it
//!   rides on (an *inertness* hazard: the dependent flag is simply unreachable, never a
//!   wrong parse).
//! - [`FeatureSet::grammar_conflict`] — two features that both read the *same*
//!   parser-position head (a soundness hazard like a lexical conflict, but one layer up: a
//!   fixed parser branch order silently shadows one reading, with the tokenizer innocent).
//!
//! The three are MECE by construction: a lexical trigger never appears in the dependency or
//! grammar registries, a pure grammar dependency never appears in the other two, and a
//! parser-position contention has no tokenizer trigger of its own and rides no base flag.
//! All are test/debug-time properties — every shipped preset and any custom [`FeatureDelta`]
//! should be clean under all three.
//!
//! These registries record the contentions with *no* defined resolution. Their positive
//! counterpart — the multi-claimant statement heads a preset *does* union with a documented,
//! deterministic resolution (a lookahead split, a dispatch precedence, or a deliberate
//! one-reading exclusion), which is why they earn no variant here — is the enumerable
//! [`MULTI_CLAIMANT_STATEMENT_HEADS`] ledger in the `head_contention` module. A new
//! head-level `LENIENT` exclusion is recorded there, not as a registry variant.

use super::*;

impl FeatureSet {
    /// The first lexical conflict in this feature set, or `None` when it is
    /// self-consistent.
    ///
    /// A dialect is modelled as independent feature data, but a few features
    /// contend for the *same* tokenizer trigger, and the tokenizer is context-free:
    /// each trigger resolves to exactly one token kind by a fixed precedence. Enabling
    /// two claimants of one trigger is not an error today — the precedence silently
    /// keeps one and shadows the other (e.g. `double_quoted_strings` on top of a `"`
    /// identifier quote makes `"x"` a string, never the identifier). This predicate
    /// turns that implicit precedence into an explicit, testable property: every
    /// shipped preset — and any custom [`FeatureDelta`] — should return `None`.
    ///
    /// The contested triggers form a MECE partition: each [`LexicalConflict`] variant
    /// governs one distinct trigger — `"`, `[`, `:`+identifier (the `a[x:y]` slice
    /// bound / `{a: b}` collection separator), `$`+digit, `#` (whose claimants — a line
    /// comment, the XOR operator, a `#`-led identifier byte class, and DuckDB's `#n`
    /// positional reference — pair up as comment-vs-identifier, XOR-vs-comment,
    /// positional-vs-XOR, and positional-vs-comment, none of which both fire in a shipped
    /// preset), `@name`, `<@`, `?` (the `jsonb` key-existence operator vs the anonymous
    /// placeholder), and `@@` (the `jsonb`/text-search match operator vs the MySQL
    /// system-variable sigil) — with no overlap, plus the byte-class
    /// hygiene variants that keep a feature-claimed sigil byte (`$`, `@`, `:`) out of a
    /// custom table's identifier-start class. The set is exhaustive over the tokenizer's
    /// *shared-trigger* hazards. The other either/or features are already single-valued
    /// *by type* — [`PipeOperator`] for `||` and [`DoubleAmpersand`] for `&&` are enums,
    /// not overlapping booleans, so an invalid combination is unrepresentable and needs
    /// no runtime check. The remaining multi-meaning sigils are disambiguated by
    /// lookahead, not by enabling rival features, so they are MECE by context: `::`
    /// (typecast) and a lone `:` before a non-identifier byte stay disjoint from `:name`
    /// by their follow byte — only a `:` before a bare identifier (the `a[x:y]` slice
    /// bound / `{a: b}` collection separator) contends,
    /// which is the tracked variant above; `@@` is disjoint from `@name` by its second
    /// `@` (its only contention is the `jsonb`-vs-system-variable trigger above), and the
    /// `@?` operator is disjoint from every `@` claimant by its second byte; the non-digit
    /// `$` forms and the `_charset'…'` introducer split by their own
    /// follow-sets; `/*!` (a versioned-comment region under
    /// [`CommentSyntax::versioned_comments`]) is disjoint from a plain `/*` block
    /// comment by its third byte, so the two never contend for the `/*` trigger; and the
    /// two `U&`-prefixed [`StringLiteralSyntax::unicode_strings`] surfaces — the `U&'…'`
    /// string constant and the `U&"…"` delimited identifier — are disjoint from each
    /// other by their third byte (`'` vs `"`) and from a plain `U`-led identifier by the
    /// three-byte `U&'`/`U&"` lead (`U` is an ordinary identifier-start letter, so a bare
    /// `U&1` stays a `U` word, a `&` operator, and `1`), so neither steals the other nor a
    /// plain identifier and no variant is minted for them.
    ///
    /// Grammar-level gates ([`MutationSyntax`], [`StatementDdlGates`], [`SelectSyntax`],
    /// [`TableExpressionSyntax`], …) never *contend for a trigger* — each is introduced by
    /// a disjoint keyword (or occupies a distinct clause position) with no ordering
    /// contention, so enabling any combination resolves to one production. They have their
    /// own, MECE-disjoint hazard instead: a *dependency*, where a refinement flag rides a
    /// base flag and is inert without it. That family is the sibling
    /// [`feature_dependencies`](Self::feature_dependencies) registry; contention here stays
    /// a purely *lexical* property covering only shared tokenizer triggers.
    pub const fn lexical_conflict(&self) -> Option<LexicalConflict> {
        // `"`: a string constant and a quoted identifier cannot both claim the byte.
        if self.string_literals.double_quoted_strings
            && identifier_quotes_open_with(self.identifier_quotes, '"')
        {
            return Some(LexicalConflict::DoubleQuoteStringVersusIdentifier);
        }
        // `[`: a bracket identifier quote claims `[` at lex time, so the `[`-punctuation
        // expression grammar (subscript, array constructor, DuckDB list literal) — and the
        // table-position PartiQL / SUPER path root (`FROM src[0].a`) — can never receive it.
        if identifier_quotes_open_with(self.identifier_quotes, '[')
            && (self.expression_syntax.subscript
                || self.expression_syntax.array_constructor
                || self.expression_syntax.collection_literals
                || self.table_expressions.table_json_path)
        {
            return Some(LexicalConflict::BracketIdentifierVersusArraySyntax);
        }
        // `:`+identifier: a colon-named parameter, array slicing with a bare-identifier
        // upper bound, and a collection literal's `key: value` separator before a
        // bare-identifier value all claim it. The `:name` scanner binds `:`+identifier
        // as one parameter, so inside `a[x:y]` it swallows `:y` (the slice's upper
        // bound), inside `{a: b}` it swallows `:b` (the entry's value), and inside
        // `a:b` it swallows the path key; a feature set enabling any of those grammars
        // with colon parameters must pick one meaning.
        // (`::` and a lone `:` before a non-identifier byte stay the typecast and
        // separator regardless.)
        if self.parameters.named_colon
            && (self.expression_syntax.subscript
                || self.expression_syntax.collection_literals
                || self.expression_syntax.semi_structured_access)
        {
            return Some(LexicalConflict::ColonParameterVersusSliceBound);
        }
        // `$`+digit: a money literal and a positional parameter cannot both claim it
        // (the scanner tries money first, shadowing the parameter).
        if self.numeric_literals.money_literals && self.parameters.positional_dollar {
            return Some(LexicalConflict::MoneyVersusPositionalDollar);
        }
        // `$`+identifier-start: a SQLite `$name` parameter and a PostgreSQL
        // `$tag$…$tag$` dollar-quote opener both lead with `$` then a tag/identifier
        // byte (the classes overlap), so a feature set enabling both resolves the byte
        // by fixed scan precedence, shadowing one. `$`+digit stays disjoint (that is
        // the money-vs-positional trigger above), so only the identifier-lead contends.
        if self.parameters.named_dollar && self.string_literals.dollar_quoted_strings {
            return Some(LexicalConflict::NamedDollarParameterVersusDollarQuotedString);
        }
        // `#`: a line comment and a `#`-led identifier cannot both claim it (the comment
        // branch wins, so a `#`-start byte class never lexes a `#name` word).
        if self.comment_syntax.line_comment_hash
            && self
                .byte_classes
                .has_class(b'#', lex_class::CLASS_IDENTIFIER_START)
        {
            return Some(LexicalConflict::HashCommentVersusHashIdentifier);
        }
        // `#`: PostgreSQL's bitwise-XOR operator and a `#` line comment cannot both claim
        // it (the comment is skipped as trivia before the operator scan can see `#`, so the
        // XOR operator is silently shadowed). The third claimant of the `#` trigger, after
        // the comment-vs-identifier pair above.
        if self.hash_bitwise_xor && self.comment_syntax.line_comment_hash {
            return Some(LexicalConflict::HashXorOperatorVersusHashComment);
        }
        // `#`: DuckDB's `#n` positional column reference and PostgreSQL's `#` bitwise-XOR
        // operator both claim the `#` trigger. The positional scan arm is placed before
        // the XOR arm, so `#`+digit lexes as a positional reference, silently shadowing
        // the XOR reading of that byte. The fourth claimant of the `#` trigger.
        if self.expression_syntax.positional_column && self.hash_bitwise_xor {
            return Some(LexicalConflict::HashXorOperatorVersusPositionalColumn);
        }
        // `#`: DuckDB's `#n` positional column reference and a `#` line comment both claim
        // the `#` trigger. The comment is consumed as trivia before the positional scan
        // sees `#`, silently shadowing the `#n` reference, so a feature set enabling both
        // must pick one meaning for `#`. (A `#`-led identifier byte class instead resolves
        // by scan order — the identifier scan precedes the positional arm — so it does not
        // contend here, mirroring the XOR-vs-identifier case.)
        if self.expression_syntax.positional_column && self.comment_syntax.line_comment_hash {
            return Some(LexicalConflict::HashCommentVersusPositionalColumn);
        }
        // `@name`: a named-at parameter and a user-variable read cannot both claim it
        // (the scanner tries the user variable first, shadowing the parameter). The
        // `@@` system-variable form is disjoint from both — it needs a second `@` —
        // so it never enters this conflict.
        if self.parameters.named_at && self.session_variables.user_variables {
            return Some(LexicalConflict::AtNameParameterVersusUserVariable);
        }
        // `<@`: PostgreSQL's "contained by" operator and an abutting `@name` sigil both
        // claim `<`+`@`. The scanner munches `<@` to the containment operator whenever
        // containment is on, shadowing the abutting `a<@x` (meaning `a < @x`) that a
        // `@name` parameter or user-variable read would otherwise lex. (`@>` is safe —
        // its second byte `>` is not identifier-start — so only `<@` contends.)
        if self.operator_syntax.containment_operators
            && (self.parameters.named_at || self.session_variables.user_variables)
        {
            return Some(LexicalConflict::ContainmentOperatorVersusAtName);
        }
        // `?`: PostgreSQL's `jsonb` key-existence operator (also the lead byte of `?|`/`?&`)
        // and the anonymous `?` placeholder both claim the byte. The anonymous-parameter
        // dispatch arm precedes the operator arm, so `?` lexes as the placeholder whenever
        // that is on, silently shadowing the operator. No shipped preset pairs them
        // (PostgreSQL enables the operators and has no `?` parameter; the placeholder
        // dialects leave the operators off).
        if self.operator_syntax.jsonb_operators && self.parameters.anonymous_question {
            return Some(LexicalConflict::JsonbKeyExistsVersusAnonymousParameter);
        }
        // `@@`: PostgreSQL's `jsonb`/text-search match operator and MySQL's `@@name`
        // system-variable sigil both claim `@`+`@`. The system-variable dispatch arm
        // precedes the operator arm, so `@@name` lexes as the variable whenever that is on,
        // silently shadowing the operator. (`@?` is disjoint from every `@` claimant by its
        // second byte, so it never contends.) No shipped preset pairs them.
        if self.operator_syntax.jsonb_operators && self.session_variables.system_variables {
            return Some(LexicalConflict::JsonbSearchOperatorVersusSystemVariable);
        }
        // `@`: the general bare-`@` operator (`custom_operators`) and an abutting `@name`
        // sigil (`named_at`/`user_variables`) both claim `@`+identifier. The sigil dispatch
        // arms precede the bare-`@` operator arm, so `@x` lexes as the sigil, shadowing the
        // operator. (The `<@`/`@@` two-byte triggers are the containment/jsonb pairs above;
        // this is the single `@`.) No shipped preset pairs them.
        if self.operator_syntax.custom_operators
            && (self.parameters.named_at || self.session_variables.user_variables)
        {
            return Some(LexicalConflict::CustomOperatorVersusAtName);
        }
        // `@@`: the general `@@` operator (`custom_operators`, when the `jsonb` family is off
        // so `@@` is not already claimed by the pair above) and MySQL's `@@name` system
        // variable both claim `@`+`@`. The system-variable arm precedes the operator arm, so
        // `@@name` lexes as the variable, shadowing the operator. No shipped preset pairs them.
        if self.operator_syntax.custom_operators
            && !self.operator_syntax.jsonb_operators
            && self.session_variables.system_variables
        {
            return Some(LexicalConflict::CustomOperatorVersusSystemVariable);
        }
        // Byte-class hygiene: a feature that leads with a sigil byte assumes that byte
        // dispatches to its sigil scan, so a custom `ByteClasses` must not also mark the
        // byte `CLASS_IDENTIFIER_START` — the same either/or the `#` check above enforces
        // for a `#`-led comment. Every shipped preset uses a table that marks none of these
        // bytes identifier-start (`STANDARD_BYTE_CLASSES`, or `POSTGRES_BYTE_CLASSES` which
        // only adds the vertical tab to the whitespace class), so only a custom table trips
        // these.
        if (self.string_literals.dollar_quoted_strings
            || self.parameters.positional_dollar
            || self.numeric_literals.money_literals)
            && self
                .byte_classes
                .has_class(b'$', lex_class::CLASS_IDENTIFIER_START)
        {
            return Some(LexicalConflict::DollarSigilVersusIdentifierByte);
        }
        if (self.parameters.named_at
            || self.session_variables.user_variables
            || self.session_variables.system_variables)
            && self
                .byte_classes
                .has_class(b'@', lex_class::CLASS_IDENTIFIER_START)
        {
            return Some(LexicalConflict::AtSigilVersusIdentifierByte);
        }
        if self.parameters.named_colon
            && self
                .byte_classes
                .has_class(b':', lex_class::CLASS_IDENTIFIER_START)
        {
            return Some(LexicalConflict::ColonSigilVersusIdentifierByte);
        }
        None
    }

    /// Whether this feature set has no [`lexical_conflict`](Self::lexical_conflict) —
    /// every shared tokenizer trigger has exactly one claimant.
    pub const fn is_lexically_consistent(&self) -> bool {
        self.lexical_conflict().is_none()
    }

    /// The first grammar-dependency violation in this feature set — a refinement flag set
    /// without the base flag it rides on — or `None` when every dependent flag has its
    /// prerequisite.
    ///
    /// The grammar sibling of [`lexical_conflict`](Self::lexical_conflict). Several flags
    /// only *refine* a production another flag opens: the extra slice bound needs the
    /// bracket subscript, the `MERGE` extensions need `MERGE` dispatch, the `[FORCE]
    /// CHECKPOINT <db>` operands need the bare `CHECKPOINT` statement, and so on. Each
    /// documents the relationship in its field doc ("rides on" / "only reachable where" /
    /// "inert without"); this predicate turns that prose into an explicit, testable
    /// property.
    ///
    /// The severity is *inertness*, not unsoundness: a dependent flag whose base is off is
    /// simply unreachable — the parser never reaches the grammar position, so the flag has
    /// no effect and no wrong parse results. That is why this is a debug/test-time property
    /// (every shipped preset and any custom [`FeatureDelta`] should return `None`) rather
    /// than a runtime reject, and why [`try_with`](Self::try_with) — whose contract is the
    /// *lexical*-soundness gate — deliberately does not fold it in.
    ///
    /// MECE with [`lexical_conflict`](Self::lexical_conflict): every variant here is a pure
    /// grammar dependency with no tokenizer trigger of its own, and no shared-trigger
    /// hazard appears here. A flag that both rides a base grammar flag *and* claims a
    /// tokenizer trigger belongs in each registry for its respective axis.
    ///
    /// The predicate reads the *whole* [`FeatureSet`], so a dependent flag may name a base on
    /// a different syntax axis (a cross-axis dependency) as freely as a same-axis one; the
    /// machinery does not restrict a dependency to one sub-struct. What it *does* require is
    /// the inertness contract above — a registrable dependent must be fully inert when its
    /// base is off. That requirement, not the axis, is what excludes the one measured
    /// near-miss:
    ///
    /// * [`SessionVariableSyntax::variable_assignment`] — a two-facet flag.
    ///   Its *parser* facet (the MySQL `SET` variable-assignment grammar) is unreachable
    ///   without [`ShowSyntax::session_statements`], which gates all `SET`/`RESET`/`SHOW`
    ///   dispatch — a genuine cross-axis grammar dependency. But its *lexer* facet (`:=`
    ///   munching to one `ColonEquals` operator token) fires independently of that gate, so
    ///   the flag is **not** inert when `session_statements` is off: it still changes
    ///   tokenization (measured — `:=` lexes to one token with the flag on vs two, `:` then
    ///   `=`, with it off). A dependent that observably changes tokenization while its base is
    ///   off violates the inertness contract *and* the "no tokenizer trigger of its own" MECE
    ///   line, so registering it as a variant would falsely certify it safe-to-dangle.
    ///   `variable_assignment` is therefore a documented exemption, not a variant: its field
    ///   doc names the independent lexer facet as the reason the flag is not fully inert, and
    ///   its cross-axis parser dependency is recorded there rather than here.
    pub const fn feature_dependencies(&self) -> Option<FeatureDependencyViolation> {
        use FeatureDependencyViolation as V;

        // QueryTailSyntax: the row-locking refinements ride the base `locking_clauses` gate
        // (they refine the strength keyword / repeat the shared clause).
        if self.query_tail_syntax.key_lock_strengths && !self.query_tail_syntax.locking_clauses {
            return Some(V::KeyLockStrengthsWithoutLockingClauses);
        }
        if self.query_tail_syntax.stacked_locking_clauses && !self.query_tail_syntax.locking_clauses
        {
            return Some(V::StackedLockingClausesWithoutLockingClauses);
        }

        // TableFactorSyntax: the `WITH OFFSET` tail rides an `UNNEST` table factor.
        if self.table_factor_syntax.unnest_with_offset && !self.table_factor_syntax.unnest {
            return Some(V::UnnestWithOffsetWithoutUnnest);
        }

        // ExpressionSyntax: the third slice bound rides the bracket `subscript`; the
        // multidimensional bare-bracket sub-row rides the `ARRAY[…]` constructor.
        if self.expression_syntax.slice_step && !self.expression_syntax.subscript {
            return Some(V::SliceStepWithoutSubscript);
        }
        if self.expression_syntax.multidim_array_literals
            && !self.expression_syntax.array_constructor
        {
            return Some(V::MultidimArrayLiteralsWithoutArrayConstructor);
        }

        // OperatorSyntax: the list-operand and arbitrary-operator quantifier forms ride the
        // base `quantified_comparisons` reading of `ANY`/`ALL`/`SOME`; the DuckDB lambda is
        // inert without the `->` lexeme `json_arrow_operators` munches.
        if self.operator_syntax.quantified_comparison_lists
            && !self.operator_syntax.quantified_comparisons
        {
            return Some(V::QuantifiedComparisonListsWithoutQuantifiedComparisons);
        }
        if self.operator_syntax.quantified_arbitrary_operator
            && !self.operator_syntax.quantified_comparisons
        {
            return Some(V::QuantifiedArbitraryOperatorWithoutQuantifiedComparisons);
        }
        if self.operator_syntax.lambda_expressions && !self.operator_syntax.json_arrow_operators {
            return Some(V::LambdaExpressionsWithoutJsonArrowOperators);
        }

        // MutationSyntax: the CTE-before-MERGE clause and the three MERGE-action extensions
        // are only reachable where `merge` dispatches `MERGE` at all.
        if self.mutation_syntax.cte_before_merge && !self.mutation_syntax.merge {
            return Some(V::CteBeforeMergeWithoutMerge);
        }
        if self.mutation_syntax.merge_when_not_matched_by && !self.mutation_syntax.merge {
            return Some(V::MergeWhenNotMatchedByWithoutMerge);
        }
        if self.mutation_syntax.merge_insert_default_values && !self.mutation_syntax.merge {
            return Some(V::MergeInsertDefaultValuesWithoutMerge);
        }
        if self.mutation_syntax.merge_insert_overriding && !self.mutation_syntax.merge {
            return Some(V::MergeInsertOverridingWithoutMerge);
        }

        // IndexAlterSyntax: the extended-`ALTER TABLE` actions and guards are only
        // reachable through the `alter_table_extended` path.
        if self.index_alter_syntax.alter_existence_guards
            && !self.index_alter_syntax.alter_table_extended
        {
            return Some(V::AlterExistenceGuardsWithoutAlterTableExtended);
        }
        if self.index_alter_syntax.alter_column_set_data_type
            && !self.index_alter_syntax.alter_table_extended
        {
            return Some(V::AlterColumnSetDataTypeWithoutAlterTableExtended);
        }

        // UtilitySyntax: each operand/guard refinement rides the base statement gate.
        if self.maintenance_syntax.checkpoint_database && !self.maintenance_syntax.checkpoint {
            return Some(V::CheckpointDatabaseWithoutCheckpoint);
        }
        if self.maintenance_syntax.analyze_columns && !self.maintenance_syntax.analyze {
            return Some(V::AnalyzeColumnsWithoutAnalyze);
        }
        if self.utility_syntax.load_bare_name && !self.utility_syntax.load_extension {
            return Some(V::LoadBareNameWithoutLoadExtension);
        }
        if self.utility_syntax.call_bare_name && !self.utility_syntax.call {
            return Some(V::CallBareNameWithoutCall);
        }
        if self.utility_syntax.detach_if_exists && !self.utility_syntax.attach {
            return Some(V::DetachIfExistsWithoutAttach);
        }
        if self.utility_syntax.use_qualified_name && !self.utility_syntax.use_statement {
            return Some(V::UseQualifiedNameWithoutUseStatement);
        }
        if self.access_control_syntax.access_control_extended_objects
            && !self.access_control_syntax.access_control
        {
            return Some(V::AccessControlExtendedObjectsWithoutAccessControl);
        }
        if self.access_control_syntax.access_control_account_grants
            && !self.access_control_syntax.access_control
        {
            return Some(V::AccountGrantsWithoutAccessControl);
        }
        if self.utility_syntax.prepare_typed_parameters && !self.utility_syntax.prepared_statements
        {
            return Some(V::PrepareTypedParametersWithoutPreparedStatements);
        }

        None
    }

    /// Whether this feature set has no
    /// [`feature_dependencies`](Self::feature_dependencies) violation — every dependent
    /// grammar flag has the base flag it rides on.
    pub const fn has_satisfied_feature_dependencies(&self) -> bool {
        self.feature_dependencies().is_none()
    }

    /// This feature set with every *inert* refinement flag cleared — the dependency
    /// registry's normal form, in which
    /// [`has_satisfied_feature_dependencies`](Self::has_satisfied_feature_dependencies)
    /// holds.
    ///
    /// Each [`feature_dependencies`](Self::feature_dependencies) violation is a refinement
    /// flag enabled without the base it rides on; the registry's own contract is that such
    /// a flag is *inert* (the parser never reaches its grammar position), so turning it off
    /// cannot change any parse. This walks the registry, clearing one dangling dependent
    /// per step until none remain, and is therefore an outcome-preserving projection onto
    /// the parser's self-consistency precondition (the parse-entry `debug_assert!`) — the
    /// tool a caller that assembles a [`FeatureDelta`] by toggling individual flags uses to
    /// drop the inert leftovers before handing the set to the parser, rather than reasoning
    /// out the dependency closure by hand.
    ///
    /// This is the dependency sibling of the lexical [`try_with`](Self::try_with): where
    /// `try_with` reports the lexical verdict as a value, this repairs the (benign)
    /// dependency one. It deliberately does *not* touch a
    /// [`lexical_conflict`](Self::lexical_conflict) or a
    /// [`grammar_conflict`](Self::grammar_conflict) — those are *soundness* hazards with no
    /// inert-and-safe reading to normalize away, so a set carrying one has no defined parse
    /// and this returns it unchanged.
    pub fn without_dangling_dependents(&self) -> FeatureSet {
        use FeatureDependencyViolation as V;

        let mut features = self.clone();
        // Each step clears exactly the dependent named by the first violation, so the
        // violation count strictly decreases and the loop terminates.
        while let Some(violation) = features.feature_dependencies() {
            match violation {
                V::KeyLockStrengthsWithoutLockingClauses => {
                    features.query_tail_syntax.key_lock_strengths = false;
                }
                V::StackedLockingClausesWithoutLockingClauses => {
                    features.query_tail_syntax.stacked_locking_clauses = false;
                }
                V::UnnestWithOffsetWithoutUnnest => {
                    features.table_factor_syntax.unnest_with_offset = false;
                }
                V::SliceStepWithoutSubscript => {
                    features.expression_syntax.slice_step = false;
                }
                V::MultidimArrayLiteralsWithoutArrayConstructor => {
                    features.expression_syntax.multidim_array_literals = false;
                }
                V::QuantifiedComparisonListsWithoutQuantifiedComparisons => {
                    features.operator_syntax.quantified_comparison_lists = false;
                }
                V::QuantifiedArbitraryOperatorWithoutQuantifiedComparisons => {
                    features.operator_syntax.quantified_arbitrary_operator = false;
                }
                V::LambdaExpressionsWithoutJsonArrowOperators => {
                    features.operator_syntax.lambda_expressions = false;
                }
                V::CteBeforeMergeWithoutMerge => {
                    features.mutation_syntax.cte_before_merge = false;
                }
                V::MergeWhenNotMatchedByWithoutMerge => {
                    features.mutation_syntax.merge_when_not_matched_by = false;
                }
                V::MergeInsertDefaultValuesWithoutMerge => {
                    features.mutation_syntax.merge_insert_default_values = false;
                }
                V::MergeInsertOverridingWithoutMerge => {
                    features.mutation_syntax.merge_insert_overriding = false;
                }
                V::AlterExistenceGuardsWithoutAlterTableExtended => {
                    features.index_alter_syntax.alter_existence_guards = false;
                }
                V::AlterColumnSetDataTypeWithoutAlterTableExtended => {
                    features.index_alter_syntax.alter_column_set_data_type = false;
                }
                V::CheckpointDatabaseWithoutCheckpoint => {
                    features.maintenance_syntax.checkpoint_database = false;
                }
                V::AnalyzeColumnsWithoutAnalyze => {
                    features.maintenance_syntax.analyze_columns = false;
                }
                V::LoadBareNameWithoutLoadExtension => {
                    features.utility_syntax.load_bare_name = false;
                }
                V::CallBareNameWithoutCall => {
                    features.utility_syntax.call_bare_name = false;
                }
                V::DetachIfExistsWithoutAttach => {
                    features.utility_syntax.detach_if_exists = false;
                }
                V::UseQualifiedNameWithoutUseStatement => {
                    features.utility_syntax.use_qualified_name = false;
                }
                V::AccessControlExtendedObjectsWithoutAccessControl => {
                    features
                        .access_control_syntax
                        .access_control_extended_objects = false;
                }
                V::AccountGrantsWithoutAccessControl => {
                    features.access_control_syntax.access_control_account_grants = false;
                }
                V::PrepareTypedParametersWithoutPreparedStatements => {
                    features.utility_syntax.prepare_typed_parameters = false;
                }
            }
        }
        features
    }

    /// The first grammar-position mutual exclusion in this feature set — two features whose
    /// grammars read the *same* token sequence at the *same* parser-position head with no
    /// lookahead to tell them apart — or `None` when no such pair is enabled together.
    ///
    /// The third self-consistency sibling, after
    /// [`lexical_conflict`](Self::lexical_conflict) (shared *tokenizer* triggers) and
    /// [`feature_dependencies`](Self::feature_dependencies) (a refinement flag without its
    /// base). This one covers the class the other two cannot: two features whose grammars
    /// claim the same head, where the tokenizer is innocent (each byte lexes to one fixed
    /// token) and neither flag rides the other. The parser resolves the head by a fixed
    /// branch order, so enabling both silently shadows one reading — a *soundness* hazard
    /// like a lexical conflict but at the grammar layer, which is exactly why neither sibling
    /// registry can catch it (the [`prefix_colon_alias`](SelectSyntax::prefix_colon_alias)
    /// field doc first named this gap).
    ///
    /// A collision is registrable here only when the contention has *no defined resolution*
    /// and *no shipped preset enables both*. A preset that pairs two head-claimants with a
    /// documented, deterministic resolution — DuckDB's `SUMMARIZE`-vs-MySQL-`DESCRIBE`
    /// dispatch order under Lenient, or the `GROUP BY ALL` lookahead split — is a
    /// conflict-*free* permissive union, not a mutual exclusion, and gets no variant. Like
    /// the siblings this is a debug/test-time property — every shipped preset and any custom
    /// [`FeatureDelta`] should return `None`.
    ///
    /// MECE with both siblings: every variant here is a pure parser-position contention —
    /// no shared tokenizer trigger (that is a [`LexicalConflict`]) and no base-flag
    /// dependency (that is a [`FeatureDependencyViolation`]).
    pub const fn grammar_conflict(&self) -> Option<GrammarConflict> {
        use GrammarConflict as G;

        // `<ident> :` head: DuckDB's prefix colon alias (`SELECT j : 42` / `FROM b : a`, the
        // alias written before its value) and semi-structured access (`base : key`, a postfix
        // path) both read a bare identifier then `:` at a value / select-item head. The `:`
        // always lexes as a lone `Colon` punctuation token (no tokenizer trigger), and
        // neither flag rides the other, so the hazard is purely grammatical: the prefix-alias
        // branch is tried first, binding `a : b` as an alias and silently shadowing the path
        // reading. No shipped preset pairs them (DuckDB/Lenient enable the prefix alias with
        // `semi_structured_access` off; Snowflake/Databricks enable the path with
        // `prefix_colon_alias` off).
        if self.select_syntax.prefix_colon_alias && self.expression_syntax.semi_structured_access {
            return Some(G::PrefixColonAliasVersusSemiStructuredAccess);
        }

        // Leading `DO` head: PostgreSQL's anonymous-code-block statement
        // ([`do_statement`](UtilitySyntax::do_statement)) and MySQL's evaluate-and-discard
        // expression list ([`do_expression_list`](UtilitySyntax::do_expression_list)) both
        // dispatch on a bare leading `DO`. The `DO` byte lexes to one contextual keyword (no
        // tokenizer trigger), and neither flag rides the other, so the hazard is purely
        // grammatical: the code-block branch is tried first, so `DO 'x'` (MySQL intent) mis-parses
        // as a PG block body and `DO 1, 2` over-rejects. No shipped preset pairs them (PostgreSQL
        // and Lenient arm the block with the expression list off; MySQL the reverse).
        if self.utility_syntax.do_statement && self.utility_syntax.do_expression_list {
            return Some(G::DoStatementVersusDoExpressionList);
        }

        // Leading `PREPARE`/`EXECUTE`/`DEALLOCATE` head: DuckDB's typed-`AS` lifecycle
        // ([`prepared_statements`](UtilitySyntax::prepared_statements)) and MySQL's
        // `FROM`/`USING` lifecycle
        // ([`prepared_statements_from`](UtilitySyntax::prepared_statements_from)) claim the same
        // three leading keywords with different grammars. The dispatch resolves the two heads
        // DuckDB-first, but the `DEALLOCATE` tail resolves MySQL-first (the `PREPARE` keyword is
        // mandatory whenever `prepared_statements_from` is on), so the combination is incoherent
        // across one lifecycle — see the parser sites in `query.rs`/`util.rs`. No shipped preset
        // pairs them (DuckDB/PostgreSQL/Lenient arm the typed-`AS` form; MySQL the `FROM`/`USING`
        // form). Registry-rejecting the combination is what lets the three keyword sites leave the
        // both-on semantics undefined.
        if self.utility_syntax.prepared_statements && self.utility_syntax.prepared_statements_from {
            return Some(G::PreparedStatementsVersusPreparedStatementsFrom);
        }

        // `GRANT`/`REVOKE` head, a ROUTE-flag conflict (see the enum doc): the MySQL account route
        // ([`access_control_account_grants`](AccessControlSyntax::access_control_account_grants))
        // dispatches the whole account-based grammar before the standard/PostgreSQL extended-object
        // grammar ([`access_control_extended_objects`](AccessControlSyntax::access_control_extended_objects))
        // is consulted, so enabling both silently deadens the extended-object reading. The two are
        // independent intents (a custom delta can set both), and the route's branch order picks the
        // account grammar with no lookahead — a contradiction with no defined resolution. No shipped
        // preset pairs them: MySQL arms the account route with extended objects off; ANSI/PostgreSQL/
        // DuckDB/Lenient keep the extended-object grammar with the account route off.
        if self.access_control_syntax.access_control_account_grants
            && self.access_control_syntax.access_control_extended_objects
        {
            return Some(G::AccountGrantsVersusExtendedObjects);
        }

        None
    }

    /// Whether this feature set has no [`grammar_conflict`](Self::grammar_conflict) — no two
    /// features contend for the same parser-position head.
    pub const fn has_no_grammar_conflict(&self) -> bool {
        self.grammar_conflict().is_none()
    }

    /// Apply `delta` to this base, returning the customized set only when it is
    /// lexically consistent, or the first [`LexicalConflict`] it would introduce.
    ///
    /// The checked counterpart to [`with`](Self::with): a builder that wants the
    /// conflict verdict as a *value* uses `try_with`, while [`with`](Self::with) stays
    /// the documented-unchecked fast path the presets build on (its result feeds the
    /// construction-time `debug_assert!` at the parse/tokenize entry seam). See
    /// [`lexical_conflict`](Self::lexical_conflict) for what the shared triggers are.
    ///
    /// This is a *lexical* gate only — its contract is the tokenizer-soundness check, and
    /// it does not verify the sibling [`feature_dependencies`](Self::feature_dependencies)
    /// property (a dependent grammar flag without its base is inert, not unsound, so it
    /// need not block a `try_with`). A builder wanting both verdicts checks
    /// [`feature_dependencies`](Self::feature_dependencies) on the returned set.
    pub const fn try_with(&self, delta: FeatureDelta) -> Result<Self, LexicalConflict> {
        let candidate = self.with(delta);
        match candidate.lexical_conflict() {
            Some(conflict) => Err(conflict),
            None => Ok(candidate),
        }
    }
}

/// Whether any style in `quotes` opens with `open` (a `const` membership test, so the
/// [`FeatureSet::lexical_conflict`] checks fold at compile time).
const fn identifier_quotes_open_with(quotes: &[IdentifierQuote], open: char) -> bool {
    let mut index = 0;
    while index < quotes.len() {
        if quotes[index].open() == open {
            return true;
        }
        index += 1;
    }
    false
}

/// A mutual exclusion between two features that both claim the *same* context-free
/// tokenizer trigger — surfaced by [`FeatureSet::lexical_conflict`].
///
/// These are the dialect-data combinations the per-feature model cannot make
/// independent: the tokenizer resolves each trigger byte to one token kind, so two
/// claimants of one trigger are mutually exclusive even though each is its own feature
/// field. The variants partition the contested triggers MECE — one per trigger, no
/// overlap.
///
/// # Adding a tokenizer-trigger feature?
///
/// A feature that makes the scanner branch on a byte (a new sigil, quote, comment, or
/// operator lead) must be audited against this registry before it lands — the
/// "MECE and exhaustive" claim is hand-maintained, and both the `:`-slice and `<@`
/// hazards postdated an earlier version of it:
/// 1. Identify the lead byte(s) the feature's scan arm dispatches on.
/// 2. For each, list every *other* feature whose scan arm can lead with the same byte
///    (grep `scan.rs` for the byte and for its `features.` guards).
/// 3. If two can be enabled at once and the scanner resolves the byte to one by fixed
///    precedence, add a variant here and a per-variant detection test — never rely on
///    "no real dialect pairs them", which is exactly how untracked hazards accrue.
/// 4. If the byte instead stays disambiguated by lookahead (a second fixed byte or a
///    required follow-set), record *that* in the [`lexical_conflict`] doc's
///    lookahead-disjoint list rather than minting a variant.
///
/// Also confirm the lead byte is not marked [`CLASS_IDENTIFIER_START`] by any preset's
/// [`ByteClasses`] (the sigil-vs-identifier-byte variants below cover a *custom* table).
///
/// [`lexical_conflict`]: FeatureSet::lexical_conflict
/// [`CLASS_IDENTIFIER_START`]: crate::dialect::lex_class::CLASS_IDENTIFIER_START
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum LexicalConflict {
    /// `"` is claimed by both [`StringLiteralSyntax::double_quoted_strings`] (→ a
    /// string constant) and an [`identifier_quotes`](FeatureSet::identifier_quotes)
    /// style opening with `"` (→ a quoted identifier). The tokenizer makes `"…"` a
    /// string whenever `double_quoted_strings` is on, silently shadowing the quoted
    /// identifier, so a feature set must pick one meaning for `"`.
    DoubleQuoteStringVersusIdentifier,
    /// `[` is claimed by both an [`identifier_quotes`](FeatureSet::identifier_quotes)
    /// style opening with `[` (T-SQL bracket identifiers, → a quoted identifier) and
    /// the `[`-punctuation expression grammar that
    /// [`ExpressionSyntax::subscript`]/[`array_constructor`](ExpressionSyntax::array_constructor)/[`collection_literals`](ExpressionSyntax::collection_literals)
    /// need, or the table-position PartiQL / SUPER path root that
    /// [`TableExpressionSyntax::table_json_path`](TableExpressionSyntax::table_json_path)
    /// enters on a `[` after a table name (`FROM src[0].a`). The tokenizer claims `[` for the
    /// identifier before the parser sees `[` punctuation, so those forms (and any other
    /// `[`-led syntax, such as array-type suffixes or the DuckDB bare-bracket list literal)
    /// can never fire — pick bracket identifiers *or* `[` expression / table-path syntax.
    BracketIdentifierVersusArraySyntax,
    /// `$`+digit is claimed by both [`NumericLiteralSyntax::money_literals`] (T-SQL
    /// `$1234.56`, → a number) and [`ParameterSyntax::positional_dollar`] (PostgreSQL
    /// `$1`, → a parameter). The scanner tries money first, silently shadowing the
    /// positional parameter, so a feature set must pick one meaning for `$`+digit.
    MoneyVersusPositionalDollar,
    /// `@name` is claimed by both [`ParameterSyntax::named_at`] (T-SQL, → a named
    /// parameter placeholder) and [`SessionVariableSyntax::user_variables`] (MySQL, → a
    /// user-variable read). The two are the same surface with different meaning, so a
    /// feature set must pick one for `@name`; the scanner tries the user variable first,
    /// silently shadowing the parameter. The `@@` system-variable form is disjoint from
    /// both (its second `@` is no identifier byte), so it is not part of this conflict.
    AtNameParameterVersusUserVariable,
    /// `$`+identifier-start is claimed by both [`ParameterSyntax::named_dollar`]
    /// (SQLite `$name`, → a parameter) and [`StringLiteralSyntax::dollar_quoted_strings`]
    /// (PostgreSQL `$tag$…$tag$`, → a string constant): a dollar-quote tag byte is the
    /// same class as an identifier-start, so both scan arms lead with `$` then that
    /// byte. The scanner resolves it by fixed precedence — one shadows the other — so a
    /// feature set must pick one meaning for `$`+identifier. (`$`+digit stays the
    /// separate money-vs-positional trigger, disjoint by its follow byte.)
    NamedDollarParameterVersusDollarQuotedString,
    /// `#` is claimed by both [`CommentSyntax::line_comment_hash`] (→ a line comment)
    /// and a [`byte_classes`](FeatureSet::byte_classes) table marking `#` an
    /// identifier-start byte (→ a `#name` word). The comment branch wins, so `#` must
    /// stay out of the identifier-start class wherever it opens a comment.
    HashCommentVersusHashIdentifier,
    /// `#` is claimed by both [`FeatureSet::hash_bitwise_xor`] (PostgreSQL, → the bitwise-XOR
    /// operator) and [`CommentSyntax::line_comment_hash`] (MySQL, → a line comment). The
    /// comment is consumed as trivia before the operator scanner sees `#`, silently shadowing
    /// the XOR operator, so a feature set enabling both must pick one meaning for `#`. (A
    /// `#`-led identifier byte class instead resolves by scan order — the identifier scan
    /// precedes the XOR arm — so it does not contend here.)
    HashXorOperatorVersusHashComment,
    /// `#`+digit is claimed by both [`ExpressionSyntax::positional_column`] (DuckDB `#n`,
    /// → a positional column reference) and [`FeatureSet::hash_bitwise_xor`] (PostgreSQL, →
    /// the bitwise-XOR operator, which claims every `#`). The positional scan arm precedes the
    /// XOR arm, so `#`+digit lexes as the positional reference, silently shadowing the XOR
    /// reading of that byte — a feature set enabling both must pick one meaning for `#`.
    /// (No shipped preset pairs them: DuckDB has the positional form and spells nothing
    /// with `#`-XOR; PostgreSQL has `#`-XOR and no positional form.)
    HashXorOperatorVersusPositionalColumn,
    /// `#`+digit is claimed by both [`ExpressionSyntax::positional_column`] (DuckDB `#n`,
    /// → a positional column reference) and [`CommentSyntax::line_comment_hash`] (MySQL, →
    /// a line comment, which claims every `#`). The comment is consumed as trivia before
    /// the positional scan sees `#`, silently shadowing the `#n` reference, so a feature
    /// set enabling both must pick one meaning for `#`. (This is why Lenient, which keeps
    /// `#` a line comment, cannot also enable the positional form. A `#`-led identifier
    /// byte class instead resolves by scan order — the identifier scan precedes the
    /// positional arm — so it does not contend here, like the XOR case.)
    HashCommentVersusPositionalColumn,
    /// `:`+identifier-start is claimed by [`ParameterSyntax::named_colon`]
    /// (Oracle/SQLite `:name`, → a parameter) and by grammars that write a `:`
    /// before a bare-identifier operand: array slicing under
    /// [`ExpressionSyntax::subscript`] (`a[x:y]`, → the slice `:` separator then the
    /// bound `y`) and the DuckDB collection literals under
    /// [`ExpressionSyntax::collection_literals`] (`{a: b}` / `MAP {k: v}`, → the
    /// `key: value` separator then the value), plus semi-structured access under
    /// [`ExpressionSyntax::semi_structured_access`] (`a:b`, → a path key). The `:name`
    /// scanner binds `:`+identifier as one parameter token, so it swallows `:y` / `:b`
    /// and the operand is lost — pick colon-named parameters *or* the `:`-separated
    /// grammars.
    /// (`::` stays the typecast and a lone `:` before a non-identifier byte stays the
    /// separator regardless — only a `:` abutting an identifier contends.)
    ColonParameterVersusSliceBound,
    /// `<@` is claimed by both [`OperatorSyntax::containment_operators`] (PostgreSQL
    /// "contained by", → the `LtAt` operator) and an abutting `@name` sigil —
    /// [`ParameterSyntax::named_at`] or [`SessionVariableSyntax::user_variables`] (→ a
    /// `<` operator then an `@name` parameter / user-variable read). The scanner munches
    /// `<`+`@` to the containment operator whenever containment is on, silently shadowing
    /// the MySQL-legal abutting `a<@x` (meaning `a < @x`), so a feature set enabling both
    /// must pick one meaning for `<@`. (`@>` is safe — its second byte `>` is not
    /// identifier-start — so only `<@` contends.)
    ContainmentOperatorVersusAtName,
    /// `?` is claimed by both [`OperatorSyntax::jsonb_operators`] (PostgreSQL `jsonb`
    /// key-existence operator, and the lead byte of `?|`/`?&`) and
    /// [`ParameterSyntax::anonymous_question`] (ODBC/JDBC anonymous placeholder). The
    /// placeholder scan arm precedes the operator arm, so `?` lexes as the placeholder
    /// whenever it is on, silently shadowing the operator — a feature set enabling both must
    /// pick one meaning for `?`. (No shipped preset pairs them: PostgreSQL has the operators
    /// and no `?` parameter; the placeholder dialects leave the operators off.)
    JsonbKeyExistsVersusAnonymousParameter,
    /// `@@` is claimed by both [`OperatorSyntax::jsonb_operators`] (PostgreSQL's
    /// `jsonb`/text-search match operator) and [`SessionVariableSyntax::system_variables`]
    /// (MySQL `@@name`). The system-variable scan arm precedes the operator arm, so `@@name`
    /// lexes as the variable whenever it is on, silently shadowing the operator — a feature
    /// set enabling both must pick one meaning for `@@`. (`@?` is disjoint from every `@`
    /// claimant by its second byte, so only `@@` contends. No shipped preset pairs them.)
    JsonbSearchOperatorVersusSystemVariable,
    /// A bare `@` is claimed by both [`OperatorSyntax::custom_operators`] (the general
    /// symbolic operator, e.g. the prefix absolute-value `@ x`) and an abutting `@name`
    /// sigil — [`ParameterSyntax::named_at`] or [`SessionVariableSyntax::user_variables`]
    /// (→ an `@name` parameter / user-variable read). The sigil dispatch arms precede the
    /// bare-`@` operator arm, so `@x` lexes as the sigil whenever one is on, silently
    /// shadowing the operator — a feature set enabling both must pick one meaning for a
    /// `@`-then-identifier. (A `@ ` not abutting an identifier stays the operator, since the
    /// sigil arms require an identifier-start follow byte. No shipped preset pairs them:
    /// PostgreSQL enables the operators and has no `@name` sigil.) Distinct from
    /// [`ContainmentOperatorVersusAtName`](Self::ContainmentOperatorVersusAtName), which is
    /// the `<@` two-byte trigger, not the bare `@`.
    CustomOperatorVersusAtName,
    /// `@@` is claimed by both [`OperatorSyntax::custom_operators`] (the general `@@`
    /// operator — e.g. the prefix box-centre `@@ box` — when the `jsonb` family is off) and
    /// [`SessionVariableSyntax::system_variables`] (MySQL `@@name`). The system-variable scan
    /// arm precedes the operator arm, so `@@name` lexes as the variable whenever it is on,
    /// silently shadowing the operator — a feature set enabling both must pick one meaning for
    /// `@@`. (When the `jsonb` family is also on, the
    /// [`JsonbSearchOperatorVersusSystemVariable`](Self::JsonbSearchOperatorVersusSystemVariable)
    /// pair — checked first — owns the `@@` trigger instead. No shipped preset pairs these.)
    CustomOperatorVersusSystemVariable,
    /// `$` is claimed as a leading sigil by [`NumericLiteralSyntax::money_literals`],
    /// [`ParameterSyntax::positional_dollar`], or
    /// [`StringLiteralSyntax::dollar_quoted_strings`], yet a
    /// [`byte_classes`](FeatureSet::byte_classes) table also marks `$` an
    /// identifier-start byte. Two claimants for `$` is the same either/or as
    /// [`HashCommentVersusHashIdentifier`](Self::HashCommentVersusHashIdentifier): keep `$` out of the identifier-start class
    /// wherever a `$`-led feature is on.
    DollarSigilVersusIdentifierByte,
    /// `@` is claimed as a leading sigil by the `@`-family features
    /// ([`ParameterSyntax::named_at`], [`SessionVariableSyntax::user_variables`], or
    /// [`SessionVariableSyntax::system_variables`]), yet a
    /// [`byte_classes`](FeatureSet::byte_classes) table also marks `@` an
    /// identifier-start byte. Same either/or as [`HashCommentVersusHashIdentifier`](Self::HashCommentVersusHashIdentifier):
    /// keep `@` out of the identifier-start class wherever an `@`-family feature is on.
    AtSigilVersusIdentifierByte,
    /// `:` is claimed as a leading sigil by [`ParameterSyntax::named_colon`], yet a
    /// [`byte_classes`](FeatureSet::byte_classes) table also marks `:` an
    /// identifier-start byte — so a lone `:` would begin an identifier instead of the
    /// `:name` parameter or the slice separator. Same either/or as
    /// [`HashCommentVersusHashIdentifier`](Self::HashCommentVersusHashIdentifier): keep `:` out of the identifier-start class
    /// while `named_colon` is on.
    ColonSigilVersusIdentifierByte,
}

/// A grammar-flag dependency left unsatisfied — a refinement feature enabled without the
/// base feature it rides on — surfaced by [`FeatureSet::feature_dependencies`].
///
/// The grammar sibling of [`LexicalConflict`], and MECE-disjoint from it: every variant
/// here is a pure grammar dependency (a flag inert without its base), never a shared
/// tokenizer trigger. Unlike a lexical conflict, an unsatisfied dependency is not a
/// soundness bug — the dependent flag is simply unreachable — so the registry is a
/// test/debug-time property, not a runtime reject.
///
/// Each variant is named `<Dependent>Without<Base>` and its doc names both the dependent
/// flag and the base flag it requires.
///
/// # Adding a dependent flag?
///
/// A flag that only *refines* a production another flag opens (a further clause, operand,
/// guard, or bound reachable only once the base grammar is admitted) must be registered
/// here before it lands — the "every preset is clean" claim is hand-maintained:
/// 1. Identify the base flag whose grammar position the new flag extends (the field doc's
///    "rides on" / "only reachable where" / "inert without" sentence names it).
/// 2. Add a `<Dependent>Without<Base>` variant, a guard in
///    [`feature_dependencies`](FeatureSet::feature_dependencies), and a per-variant
///    detection test.
/// 3. Point the field's prose sentence at the new variant so the doc and the registry stay
///    in lock-step.
///
/// If the flag instead claims its *own* tokenizer trigger, that hazard is a
/// [`LexicalConflict`], not a dependency — keep the two registries MECE.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum FeatureDependencyViolation {
    /// [`QueryTailSyntax::key_lock_strengths`] (the `FOR NO KEY UPDATE` / `FOR KEY SHARE`
    /// strengths) refines the strength keyword after `FOR`, so it requires
    /// [`QueryTailSyntax::locking_clauses`]. Without the base gate the `FOR` clause is never
    /// read, so the strength refinement is unreachable.
    KeyLockStrengthsWithoutLockingClauses,
    /// [`QueryTailSyntax::stacked_locking_clauses`] (multiple `FOR UPDATE`/`FOR SHARE` clauses
    /// on one query) repeats the shared locking clause, so it requires
    /// [`QueryTailSyntax::locking_clauses`]. Without the base gate no locking clause is read,
    /// so there is nothing to stack.
    StackedLockingClausesWithoutLockingClauses,
    /// [`TableFactorSyntax::unnest_with_offset`] (the BigQuery `WITH OFFSET` tail) sits
    /// on an `UNNEST` table factor, so it requires [`TableFactorSyntax::unnest`].
    /// Without the base gate `UNNEST(` is left to the named-table path and the tail is
    /// never reached.
    UnnestWithOffsetWithoutUnnest,
    /// [`ExpressionSyntax::slice_step`] (the third `base[lower:upper:step]` bound) is
    /// reachable only once the bracket subscript has opened, so it requires
    /// [`ExpressionSyntax::subscript`]. Without the base gate `[` is never read as a
    /// subscript and the extra bound is unreachable.
    SliceStepWithoutSubscript,
    /// [`ExpressionSyntax::multidim_array_literals`] (the bare-bracket sub-row) is only a
    /// value inside an `ARRAY[…]` constructor, so it requires
    /// [`ExpressionSyntax::array_constructor`]. Without the base gate there is no
    /// array-constructor element position for the sub-row to occupy.
    MultidimArrayLiteralsWithoutArrayConstructor,
    /// [`OperatorSyntax::quantified_comparison_lists`] (the scalar list/array operand of a
    /// quantified comparison) rides the base quantifier reading, so it requires
    /// [`OperatorSyntax::quantified_comparisons`]. Without the base gate `ANY`/`ALL`/`SOME`
    /// is not read as a quantifier and the list operand is unreachable.
    QuantifiedComparisonListsWithoutQuantifiedComparisons,
    /// [`OperatorSyntax::quantified_arbitrary_operator`] (extending the quantifier past the
    /// comparison operators) rides the base quantifier reading, so it requires
    /// [`OperatorSyntax::quantified_comparisons`]. Without the base gate the quantifier is
    /// unread and the arbitrary-operator extension is unreachable.
    QuantifiedArbitraryOperatorWithoutQuantifiedComparisons,
    /// [`OperatorSyntax::lambda_expressions`] (the DuckDB `x -> body` lambda) is a
    /// grammar-position reading of the `->` lexeme, which is munched only under
    /// [`OperatorSyntax::json_arrow_operators`], so it is inert without it. Without the base
    /// gate no `->` token is produced and the lambda reading never fires.
    LambdaExpressionsWithoutJsonArrowOperators,
    /// [`MutationSyntax::cte_before_merge`] (a leading `WITH` before `MERGE`) is only
    /// reachable where [`MutationSyntax::merge`] dispatches `MERGE`. Without the base gate
    /// the `MERGE` after the CTE list is never dispatched.
    CteBeforeMergeWithoutMerge,
    /// [`MutationSyntax::merge_when_not_matched_by`] (the `WHEN NOT MATCHED BY
    /// SOURCE | TARGET` arms) is only reachable where [`MutationSyntax::merge`] dispatches
    /// `MERGE`. Without the base gate the `MERGE` statement is never parsed.
    MergeWhenNotMatchedByWithoutMerge,
    /// [`MutationSyntax::merge_insert_default_values`] (the `INSERT DEFAULT VALUES` merge
    /// action) is only reachable where [`MutationSyntax::merge`] dispatches `MERGE`. Without
    /// the base gate the `MERGE` statement is never parsed.
    MergeInsertDefaultValuesWithoutMerge,
    /// [`MutationSyntax::merge_insert_overriding`] (the `OVERRIDING {SYSTEM | USER} VALUE`
    /// merge-insert override) is only reachable where [`MutationSyntax::merge`] dispatches
    /// `MERGE`. Without the base gate the `MERGE` statement is never parsed.
    MergeInsertOverridingWithoutMerge,
    /// [`IndexAlterSyntax::alter_existence_guards`] (the `IF [NOT] EXISTS` guards inside
    /// `ALTER TABLE`) is parsed only on the extended `ALTER TABLE` path, so it requires
    /// [`IndexAlterSyntax::alter_table_extended`]. Without the base gate the non-extended
    /// path parses no guard.
    AlterExistenceGuardsWithoutAlterTableExtended,
    /// [`IndexAlterSyntax::alter_column_set_data_type`] (the `ALTER COLUMN SET DATA TYPE`
    /// / `SET`/`DROP NOT NULL` actions) is reached only on the extended `ALTER TABLE` path,
    /// so it requires [`IndexAlterSyntax::alter_table_extended`]. Without the base gate the
    /// `ALTER COLUMN` action is never reached.
    AlterColumnSetDataTypeWithoutAlterTableExtended,
    /// [`MaintenanceSyntax::checkpoint_database`] (the DuckDB `[FORCE] CHECKPOINT <database>`
    /// operands) rides the base `CHECKPOINT` statement, so it requires
    /// [`MaintenanceSyntax::checkpoint`]. Without the base gate `CHECKPOINT` is not dispatched
    /// and the operands are unreachable.
    CheckpointDatabaseWithoutCheckpoint,
    /// [`MaintenanceSyntax::analyze_columns`] (the DuckDB `ANALYZE <table> (<cols>)` column
    /// list) rides the base `ANALYZE` statement, so it requires
    /// [`MaintenanceSyntax::analyze`]. Without the base gate `ANALYZE` is not dispatched and
    /// the column list is unreachable.
    AnalyzeColumnsWithoutAnalyze,
    /// [`UtilitySyntax::load_bare_name`] (the DuckDB bare-identifier `LOAD <name>` argument)
    /// rides the base `LOAD` statement, so it requires [`UtilitySyntax::load_extension`].
    /// Without the base gate `LOAD` is not dispatched and the bare-name argument is
    /// unreachable.
    LoadBareNameWithoutLoadExtension,
    /// [`UtilitySyntax::call_bare_name`] (MySQL's bare `CALL <name>` form, no parenthesized
    /// argument list) rides the base `CALL` statement, so it requires
    /// [`UtilitySyntax::call`]. Without the base gate `CALL` is not dispatched and the
    /// bare-name form is unreachable.
    CallBareNameWithoutCall,
    /// [`UtilitySyntax::detach_if_exists`] (the `DETACH DATABASE IF EXISTS` guard) rides the
    /// base `ATTACH`/`DETACH` statement, so it requires [`UtilitySyntax::attach`]. Without
    /// the base gate `DETACH` is not dispatched and the guard is unreachable.
    DetachIfExistsWithoutAttach,
    /// [`UtilitySyntax::use_qualified_name`] (DuckDB's dotted `USE <catalog> . <schema>`
    /// name, widening the accepted `USE` name arity from one to two parts) refines the name
    /// grammar of the base `USE` statement, so it requires [`UtilitySyntax::use_statement`].
    /// Without the base gate the leading `USE` is not dispatched, so the parser never reaches
    /// the arity check the flag widens and the flag is inert.
    UseQualifiedNameWithoutUseStatement,
    /// [`AccessControlSyntax::access_control_extended_objects`] (the extended `GRANT`/`REVOKE`
    /// object and prefix grammar) builds on the base `GRANT`/`REVOKE` statements, so it
    /// requires [`AccessControlSyntax::access_control`]. Without the base gate the access-control
    /// statements are not dispatched and the extended forms are unreachable.
    AccessControlExtendedObjectsWithoutAccessControl,
    /// [`AccessControlSyntax::access_control_account_grants`] (the MySQL account-based
    /// `GRANT`/`REVOKE` grammar) is a route of the base `GRANT`/`REVOKE` statements, so it
    /// requires [`AccessControlSyntax::access_control`]. Without the base gate the access-control
    /// statements are not dispatched and the account-based grammar is unreachable.
    AccountGrantsWithoutAccessControl,
    /// [`UtilitySyntax::prepare_typed_parameters`] (the PostgreSQL `PREPARE
    /// name(<type>, …)` parenthesized parameter-type list) widens the name position of the
    /// base `PREPARE` grammar, so it requires [`UtilitySyntax::prepared_statements`].
    /// Without the base gate `PREPARE` is not dispatched and the type list is unreachable.
    PrepareTypedParametersWithoutPreparedStatements,
}

/// A grammar-position mutual exclusion — two features that both read the *same*
/// parser-position head, which the parser resolves by a fixed branch order so enabling both
/// silently shadows one reading — surfaced by [`FeatureSet::grammar_conflict`].
///
/// The third self-consistency registry, MECE-disjoint from its two siblings: unlike a
/// [`LexicalConflict`] the contended surface is not a *tokenizer* trigger (each byte lexes to
/// one fixed token; the contention is which *grammar* claims the token sequence), and unlike
/// a [`FeatureDependencyViolation`] neither flag rides the other (both are independent
/// grammar positions). Like a lexical conflict the severity is *unsoundness* — the shadowed
/// reading is silently mis-parsed — but the shadow falls in the parser, not the tokenizer,
/// so no lexical precedence governs it.
///
/// Each variant is named `<A>Versus<B>` and its doc names both features and the shared head
/// they contend for.
///
/// # Two modelling species
///
/// The registered pairs fall into two shapes, both registrable under the same criteria:
///
/// - **Surface-overlap pairs** — two features that each *add* a reading at a shared head, so
///   enabling both leaves the head over-claimed. The `<ident> :` alias-vs-path pair and the two
///   `DO`-keyword readings ([`DoStatementVersusDoExpressionList`](Self::DoStatementVersusDoExpressionList))
///   are of this kind: each flag contributes one grammar to a position the other also reads.
/// - **Route flags** — a flag that selects *one of two mutually-exclusive whole grammars* for a
///   head rather than adding surface. When the route flag is on it dispatches its grammar
///   *before* the rival grammar is consulted, so the rival — even when explicitly enabled —
///   is silently deadened. [`AccountGrantsVersusExtendedObjects`](Self::AccountGrantsVersusExtendedObjects)
///   is the exemplar: [`access_control_account_grants`](AccessControlSyntax::access_control_account_grants)
///   routes `GRANT`/`REVOKE` to the MySQL account grammar, bypassing the
///   [`access_control_extended_objects`](AccessControlSyntax::access_control_extended_objects)
///   reading. A route flag is registrable **only when the grammar it displaces is itself a
///   feature flag** — that is what makes the both-on state independently expressible and its
///   resolution undefined-in-intent.
///
/// Not every route flag qualifies. MySQL's
/// [`variable_assignment`](SessionVariableSyntax::variable_assignment) is also a route flag — it
/// dispatches `SET` to the MySQL variable-assignment grammar before the standard `SET TIME ZONE`
/// / `SET SESSION AUTHORIZATION` forms are read — but the grammar it displaces is *unconditional
/// base grammar with no rival flag*. There is therefore no second flag to express "I also want
/// the standard `SET` forms", so no both-on contradiction exists, and MySQL (a shipped preset)
/// already exercises the route deterministically. Per the registrability criterion below, a
/// deterministic resolution a shipped preset relies on is a conflict-*free* union, not a mutual
/// exclusion (the same reasoning that leaves the `DESCRIBE`/`SUMMARIZE` leader unregistered), so
/// `variable_assignment` gets **no variant**. Registering it would require either promoting the
/// standard `SET` config grammar to its own flag or converting `variable_assignment` to an enum
/// axis (the `PipeOperator`/`DoubleAmpersand` either-by-type precedent) — an architectural
/// modelling change, not a registry addition.
///
/// # Registry coverage
///
/// The four entries cover the `<ident> :` lexical pair and three parser-position mutual
/// exclusions — the `DO`, prepared-statement, and `GRANT` heads. The grammar-level gates are
/// therefore *not* all pairwise-independent: a preset that unions two of these contenders
/// needs a registered resolution, which is why this registry exists rather than relying on
/// the gates being independent.
///
/// # Adding a grammar-position feature?
///
/// A feature whose grammar reads a token sequence at a position another feature's grammar
/// also reads must be audited against this registry before it lands — the "every preset is
/// clean" claim is hand-maintained:
/// 1. Identify the parser-position head the new grammar dispatches on (the field doc's "reads
///    the same … head" / "must not be enabled together" sentence names the rival).
/// 2. Confirm the contention is *undefined* — the two branches share no lookahead or fixed
///    dispatch order that keeps both readable — and that *no shipped preset enables both*. A
///    pair a preset unions with a documented deterministic resolution (the
///    `DESCRIBE`/`SUMMARIZE` dispatch, the `GROUP BY ALL` lookahead split) is a
///    conflict-*free* union, not a mutual exclusion, and gets no variant.
/// 3. Add an `<A>Versus<B>` variant, a guard in
///    [`grammar_conflict`](FeatureSet::grammar_conflict), and a per-variant detection test.
/// 4. Point each field's prose sentence at the new variant so the docs and the registry stay
///    in lock-step.
///
/// If the contended surface is instead a shared *tokenizer* trigger, that hazard is a
/// [`LexicalConflict`]; if one flag merely rides the other's base grammar, it is a
/// [`FeatureDependencyViolation`] — keep the three registries MECE.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum GrammarConflict {
    /// The `<ident> :` head is claimed by both [`SelectSyntax::prefix_colon_alias`] (DuckDB's
    /// alias-before-value `SELECT j : 42` / `FROM b : a`) and
    /// [`ExpressionSyntax::semi_structured_access`] (the `base : key` postfix path). The `:`
    /// always lexes as a lone `Colon` punctuation token — no [`LexicalConflict`] governs it —
    /// so the contention is purely grammatical: at a value / select-item head the prefix-alias
    /// branch is tried first, binding `a : b` as an alias and silently shadowing the path
    /// reading, so a feature set must pick one meaning for a leading `<ident> :`. No shipped
    /// preset pairs them (DuckDB/Lenient enable the prefix alias with
    /// [`semi_structured_access`](ExpressionSyntax::semi_structured_access) off;
    /// Snowflake/Databricks enable the path with
    /// [`prefix_colon_alias`](SelectSyntax::prefix_colon_alias) off).
    PrefixColonAliasVersusSemiStructuredAccess,
    /// The leading `DO` head is claimed by both [`UtilitySyntax::do_statement`] (PostgreSQL's
    /// `DO [LANGUAGE <lang>] '<body>'` anonymous code block) and
    /// [`UtilitySyntax::do_expression_list`] (MySQL's `DO <expr> [, <expr> …]`
    /// evaluate-and-discard statement). The `DO` byte lexes to one contextual keyword — no
    /// [`LexicalConflict`] governs it — so the contention is purely grammatical: the code-block
    /// branch is tried first, so under both-on `DO 'x'` (MySQL intent) mis-parses as a PostgreSQL
    /// block body and `DO 1, 2` over-rejects, the reading fixed only by dispatch order. No shipped
    /// preset pairs them (PostgreSQL and Lenient arm [`do_statement`](UtilitySyntax::do_statement)
    /// with [`do_expression_list`](UtilitySyntax::do_expression_list) off; MySQL the reverse).
    DoStatementVersusDoExpressionList,
    /// The leading `PREPARE`/`EXECUTE`/`DEALLOCATE` head is claimed by both
    /// [`UtilitySyntax::prepared_statements`] (DuckDB's typed-`AS` prepared-statement lifecycle)
    /// and [`UtilitySyntax::prepared_statements_from`] (MySQL's `FROM`/`USING` lifecycle) —
    /// different grammars on the same three leading keywords, each byte lexing to one contextual
    /// keyword (no [`LexicalConflict`]). The statement dispatch resolves the `PREPARE`/`EXECUTE`
    /// heads DuckDB-first by branch order, but the `DEALLOCATE` tail resolves MySQL-first (the
    /// `PREPARE` keyword becomes mandatory whenever `prepared_statements_from` is on), so the
    /// combination is *incoherent across one lifecycle*: `DEALLOCATE p` (valid under
    /// `prepared_statements` alone) errors while `PREPARE p AS …` keeps the DuckDB reading. No
    /// shipped preset pairs them (DuckDB/PostgreSQL/Lenient arm the typed-`AS` form; MySQL the
    /// `FROM`/`USING` form). Because the combination is registry-rejected, the three keyword sites
    /// (`query.rs` dispatch, `util.rs` `finish_deallocate_statement`) leave the both-on semantics
    /// deliberately undefined rather than reconciling the two winners.
    PreparedStatementsVersusPreparedStatementsFrom,
    /// The `GRANT`/`REVOKE` head is claimed by both
    /// [`AccessControlSyntax::access_control_account_grants`] (MySQL's account-based grammar) and
    /// [`AccessControlSyntax::access_control_extended_objects`] (the standard/PostgreSQL extended
    /// object and prefix grammar). A **route-flag** conflict (see the enum doc): the account route
    /// dispatches its whole grammar before the extended-object reading is consulted, so enabling
    /// both silently deadens the extended-object grammar even when it is explicitly on. The `GRANT`
    /// byte lexes to one keyword (no [`LexicalConflict`]) and neither flag rides the other, so the
    /// contention is purely grammatical, resolved by fixed branch order with no lookahead. No
    /// shipped preset pairs them: MySQL arms the account route with extended objects off; ANSI/
    /// PostgreSQL/DuckDB/Lenient keep the extended-object grammar with the account route off — the
    /// asymmetry the [`access_control_account_grants`](AccessControlSyntax::access_control_account_grants)
    /// field doc's "a dialect cannot enable both grant grammars at once" sentence records.
    AccountGrantsVersusExtendedObjects,
}

#[cfg(test)]
mod tests {
    use crate::dialect::lex_class::CLASS_IDENTIFIER_START;
    use crate::dialect::*;

    #[test]
    fn lexical_conflict_flags_a_sigil_byte_marked_identifier_start() {
        // The `#`-comment/`#`-identifier either/or, generalized: a feature that leads
        // with a sigil byte must not also have that byte in the identifier-start class of
        // a custom `ByteClasses`, or an identifier scan would shadow the sigil dispatch.
        // No shipped preset trips these — `STANDARD_BYTE_CLASSES` marks none of `$`/`@`/`:`
        // identifier-start — so each is reached only through a custom table.
        let dollar = FeatureSet::ANSI.with(
            FeatureDelta::EMPTY
                .parameters(ParameterSyntax {
                    positional_dollar: true,
                    ..ParameterSyntax::ANSI
                })
                .byte_classes(
                    FeatureSet::ANSI
                        .byte_classes
                        .with_class(b'$', CLASS_IDENTIFIER_START),
                ),
        );
        assert_eq!(
            dollar.lexical_conflict(),
            Some(LexicalConflict::DollarSigilVersusIdentifierByte),
        );

        let at = FeatureSet::ANSI.with(
            FeatureDelta::EMPTY
                .parameters(ParameterSyntax {
                    named_at: true,
                    ..ParameterSyntax::ANSI
                })
                .byte_classes(
                    FeatureSet::ANSI
                        .byte_classes
                        .with_class(b'@', CLASS_IDENTIFIER_START),
                ),
        );
        assert_eq!(
            at.lexical_conflict(),
            Some(LexicalConflict::AtSigilVersusIdentifierByte),
        );

        let colon = FeatureSet::ANSI.with(
            FeatureDelta::EMPTY
                .parameters(ParameterSyntax {
                    named_colon: true,
                    ..ParameterSyntax::ANSI
                })
                .byte_classes(
                    FeatureSet::ANSI
                        .byte_classes
                        .with_class(b':', CLASS_IDENTIFIER_START),
                ),
        );
        assert_eq!(
            colon.lexical_conflict(),
            Some(LexicalConflict::ColonSigilVersusIdentifierByte),
        );

        // The check is the pair, not the byte class alone: marking `$` identifier-start
        // with no `$`-led feature on is a coherent custom choice, not a conflict.
        let dollar_identifier_only = FeatureSet::ANSI.with(
            FeatureDelta::EMPTY.byte_classes(
                FeatureSet::ANSI
                    .byte_classes
                    .with_class(b'$', CLASS_IDENTIFIER_START),
            ),
        );
        assert_eq!(dollar_identifier_only.lexical_conflict(), None);
    }

    #[test]
    fn positional_column_conflicts_on_the_hash_trigger() {
        // DuckDB's `#n` positional reference claims `#`+digit; pairing it with either
        // other `#` claimant is a conflict, since the tokenizer resolves `#` to one
        // meaning and shadows the rest. No shipped preset pairs them (DuckDB has neither
        // rival), so each is reached only through a constructed combination.
        let with_xor = FeatureSet::DUCKDB.with(FeatureDelta::EMPTY.hash_bitwise_xor(true));
        assert_eq!(
            with_xor.lexical_conflict(),
            Some(LexicalConflict::HashXorOperatorVersusPositionalColumn),
        );

        let with_comment =
            FeatureSet::DUCKDB.with(FeatureDelta::EMPTY.comment_syntax(CommentSyntax {
                line_comment_hash: true,
                ..FeatureSet::DUCKDB.comment_syntax
            }));
        assert_eq!(
            with_comment.lexical_conflict(),
            Some(LexicalConflict::HashCommentVersusPositionalColumn),
        );

        // DuckDB itself — positional on (proven by the parser round-trip tests), no rival
        // `#` reading — is consistent.
        assert_eq!(FeatureSet::DUCKDB.lexical_conflict(), None);
    }

    #[test]
    fn every_shipped_preset_satisfies_its_feature_dependencies() {
        // The dependency sibling of the per-preset lexical assertions: every dependent
        // grammar flag a shipped preset sets also has the base flag it rides on, so no
        // preset ships an inert flag.
        for preset in [
            FeatureSet::ANSI,
            FeatureSet::POSTGRES,
            FeatureSet::MYSQL,
            FeatureSet::SQLITE,
            FeatureSet::DUCKDB,
            FeatureSet::BIGQUERY,
            FeatureSet::HIVE,
            FeatureSet::CLICKHOUSE,
            FeatureSet::DATABRICKS,
            FeatureSet::MSSQL,
            FeatureSet::SNOWFLAKE,
            FeatureSet::REDSHIFT,
            FeatureSet::LENIENT,
        ] {
            assert_eq!(preset.feature_dependencies(), None);
            assert!(preset.has_satisfied_feature_dependencies());
        }
    }

    #[test]
    fn feature_dependencies_detects_each_unsatisfied_dependency() {
        // ANSI is dependency-clean, so flipping exactly one base off while its dependent
        // stays on isolates that variant as the first (and only) violation. Sibling
        // dependents on the same multi-dependent base (`locking_clauses`,
        // `quantified_comparisons`, `merge`, `alter_table_extended`) are pinned off so the
        // intended variant is the one returned.
        use FeatureDependencyViolation as V;

        let key_lock =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.query_tail_syntax(QueryTailSyntax {
                locking_clauses: false,
                key_lock_strengths: true,
                stacked_locking_clauses: false,
                ..FeatureSet::ANSI.query_tail_syntax
            }));
        assert_eq!(
            key_lock.feature_dependencies(),
            Some(V::KeyLockStrengthsWithoutLockingClauses),
        );

        let stacked =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.query_tail_syntax(QueryTailSyntax {
                locking_clauses: false,
                key_lock_strengths: false,
                stacked_locking_clauses: true,
                ..FeatureSet::ANSI.query_tail_syntax
            }));
        assert_eq!(
            stacked.feature_dependencies(),
            Some(V::StackedLockingClausesWithoutLockingClauses),
        );

        let unnest_offset =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.table_factor_syntax(TableFactorSyntax {
                unnest: false,
                unnest_with_offset: true,
                ..FeatureSet::ANSI.table_factor_syntax
            }));
        assert_eq!(
            unnest_offset.feature_dependencies(),
            Some(V::UnnestWithOffsetWithoutUnnest),
        );

        let slice =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.expression_syntax(ExpressionSyntax {
                subscript: false,
                slice_step: true,
                ..FeatureSet::ANSI.expression_syntax
            }));
        assert_eq!(
            slice.feature_dependencies(),
            Some(V::SliceStepWithoutSubscript)
        );

        let multidim =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.expression_syntax(ExpressionSyntax {
                array_constructor: false,
                multidim_array_literals: true,
                ..FeatureSet::ANSI.expression_syntax
            }));
        assert_eq!(
            multidim.feature_dependencies(),
            Some(V::MultidimArrayLiteralsWithoutArrayConstructor),
        );

        let quant_lists =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.operator_syntax(OperatorSyntax {
                quantified_comparisons: false,
                quantified_comparison_lists: true,
                quantified_arbitrary_operator: false,
                ..FeatureSet::ANSI.operator_syntax
            }));
        assert_eq!(
            quant_lists.feature_dependencies(),
            Some(V::QuantifiedComparisonListsWithoutQuantifiedComparisons),
        );

        let quant_arbitrary =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.operator_syntax(OperatorSyntax {
                quantified_comparisons: false,
                quantified_comparison_lists: false,
                quantified_arbitrary_operator: true,
                ..FeatureSet::ANSI.operator_syntax
            }));
        assert_eq!(
            quant_arbitrary.feature_dependencies(),
            Some(V::QuantifiedArbitraryOperatorWithoutQuantifiedComparisons),
        );

        let lambda = FeatureSet::ANSI.with(FeatureDelta::EMPTY.operator_syntax(OperatorSyntax {
            json_arrow_operators: false,
            lambda_expressions: true,
            ..FeatureSet::ANSI.operator_syntax
        }));
        assert_eq!(
            lambda.feature_dependencies(),
            Some(V::LambdaExpressionsWithoutJsonArrowOperators),
        );

        // The four `MERGE` extensions, each isolated with `merge` off and its three
        // siblings pinned off.
        let merge_base = MutationSyntax {
            merge: false,
            cte_before_merge: false,
            merge_when_not_matched_by: false,
            merge_insert_default_values: false,
            merge_insert_overriding: false,
            merge_update_set_star: false,
            merge_insert_star_by_name: false,
            merge_error_action: false,
            ..FeatureSet::ANSI.mutation_syntax
        };
        let cte_merge =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.mutation_syntax(MutationSyntax {
                cte_before_merge: true,
                ..merge_base
            }));
        assert_eq!(
            cte_merge.feature_dependencies(),
            Some(V::CteBeforeMergeWithoutMerge),
        );
        let when_not_matched =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.mutation_syntax(MutationSyntax {
                merge_when_not_matched_by: true,
                ..merge_base
            }));
        assert_eq!(
            when_not_matched.feature_dependencies(),
            Some(V::MergeWhenNotMatchedByWithoutMerge),
        );
        let insert_defaults =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.mutation_syntax(MutationSyntax {
                merge_insert_default_values: true,
                ..merge_base
            }));
        assert_eq!(
            insert_defaults.feature_dependencies(),
            Some(V::MergeInsertDefaultValuesWithoutMerge),
        );
        let insert_overriding =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.mutation_syntax(MutationSyntax {
                merge_insert_overriding: true,
                merge_update_set_star: false,
                merge_insert_star_by_name: false,
                merge_error_action: false,
                ..merge_base
            }));
        assert_eq!(
            insert_overriding.feature_dependencies(),
            Some(V::MergeInsertOverridingWithoutMerge),
        );

        let alter_guards =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.index_alter_syntax(IndexAlterSyntax {
                alter_table_extended: false,
                alter_existence_guards: true,
                alter_column_set_data_type: false,
                ..FeatureSet::ANSI.index_alter_syntax
            }));
        assert_eq!(
            alter_guards.feature_dependencies(),
            Some(V::AlterExistenceGuardsWithoutAlterTableExtended),
        );
        let alter_set_type =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.index_alter_syntax(IndexAlterSyntax {
                alter_table_extended: false,
                alter_existence_guards: false,
                alter_column_set_data_type: true,
                ..FeatureSet::ANSI.index_alter_syntax
            }));
        assert_eq!(
            alter_set_type.feature_dependencies(),
            Some(V::AlterColumnSetDataTypeWithoutAlterTableExtended),
        );

        let checkpoint_db =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.maintenance_syntax(MaintenanceSyntax {
                checkpoint: false,
                checkpoint_database: true,
                ..FeatureSet::ANSI.maintenance_syntax
            }));
        assert_eq!(
            checkpoint_db.feature_dependencies(),
            Some(V::CheckpointDatabaseWithoutCheckpoint),
        );
        let analyze_cols =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.maintenance_syntax(MaintenanceSyntax {
                analyze: false,
                analyze_columns: true,
                ..FeatureSet::ANSI.maintenance_syntax
            }));
        assert_eq!(
            analyze_cols.feature_dependencies(),
            Some(V::AnalyzeColumnsWithoutAnalyze),
        );
        let load_bare = FeatureSet::ANSI.with(FeatureDelta::EMPTY.utility_syntax(UtilitySyntax {
            load_extension: false,
            load_bare_name: true,
            ..FeatureSet::ANSI.utility_syntax
        }));
        assert_eq!(
            load_bare.feature_dependencies(),
            Some(V::LoadBareNameWithoutLoadExtension),
        );
        let call_bare = FeatureSet::ANSI.with(FeatureDelta::EMPTY.utility_syntax(UtilitySyntax {
            call: false,
            call_bare_name: true,
            ..FeatureSet::ANSI.utility_syntax
        }));
        assert_eq!(
            call_bare.feature_dependencies(),
            Some(V::CallBareNameWithoutCall),
        );
        let detach = FeatureSet::ANSI.with(FeatureDelta::EMPTY.utility_syntax(UtilitySyntax {
            attach: false,
            detach_if_exists: true,
            ..FeatureSet::ANSI.utility_syntax
        }));
        assert_eq!(
            detach.feature_dependencies(),
            Some(V::DetachIfExistsWithoutAttach),
        );
        let use_qualified =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.utility_syntax(UtilitySyntax {
                use_statement: false,
                use_qualified_name: true,
                ..FeatureSet::ANSI.utility_syntax
            }));
        assert_eq!(
            use_qualified.feature_dependencies(),
            Some(V::UseQualifiedNameWithoutUseStatement),
        );
        let extended_ac = FeatureSet::ANSI.with(FeatureDelta::EMPTY.access_control_syntax(
            AccessControlSyntax {
                access_control: false,
                access_control_extended_objects: true,
                user_role_management: false,
                access_control_account_grants: false,
                alter_role_rename: false,
            },
        ));
        assert_eq!(
            extended_ac.feature_dependencies(),
            Some(V::AccessControlExtendedObjectsWithoutAccessControl),
        );
        let account_grants = FeatureSet::ANSI.with(FeatureDelta::EMPTY.access_control_syntax(
            AccessControlSyntax {
                access_control: false,
                access_control_extended_objects: false,
                user_role_management: false,
                access_control_account_grants: true,
                alter_role_rename: false,
            },
        ));
        assert_eq!(
            account_grants.feature_dependencies(),
            Some(V::AccountGrantsWithoutAccessControl),
        );
        let typed_params =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.utility_syntax(UtilitySyntax {
                prepared_statements: false,
                prepare_typed_parameters: true,
                ..FeatureSet::ANSI.utility_syntax
            }));
        assert_eq!(
            typed_params.feature_dependencies(),
            Some(V::PrepareTypedParametersWithoutPreparedStatements),
        );
    }

    #[test]
    fn feature_dependencies_and_lexical_conflict_stay_mece() {
        // A set can carry a lexical conflict and be dependency-clean, or vice versa —
        // the two registries never both claim the same combination. ANSI plus a bracket
        // slice with no subscript is a pure dependency violation and no lexical conflict.
        let dep_only =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.expression_syntax(ExpressionSyntax {
                subscript: false,
                slice_step: true,
                ..FeatureSet::ANSI.expression_syntax
            }));
        assert_eq!(
            dep_only.feature_dependencies(),
            Some(FeatureDependencyViolation::SliceStepWithoutSubscript),
        );
        assert_eq!(dep_only.lexical_conflict(), None);
    }

    #[test]
    fn without_dangling_dependents_clears_only_the_inert_refinements() {
        // A base flag turned off under a preset that had refinements riding it leaves those
        // refinements dangling; the normal form clears exactly them, satisfies the
        // dependency registry, and touches nothing else (the base flag it *did* keep, the
        // rest of the set). PostgreSQL enables `merge` plus its `cte_before_merge` /
        // `merge_when_not_matched_by` refinements — dropping `merge` dangles both.
        let dangling =
            FeatureSet::POSTGRES.with(FeatureDelta::EMPTY.mutation_syntax(MutationSyntax {
                merge: false,
                ..FeatureSet::POSTGRES.mutation_syntax
            }));
        assert!(dangling.feature_dependencies().is_some());

        let cleaned = dangling.without_dangling_dependents();
        assert_eq!(cleaned.feature_dependencies(), None);
        assert!(cleaned.has_satisfied_feature_dependencies());
        // The inert refinements were cleared…
        assert!(!cleaned.mutation_syntax.cte_before_merge);
        assert!(!cleaned.mutation_syntax.merge_when_not_matched_by);
        // …and nothing that was already consistent changed: the (still-off) base is
        // untouched, and an already-clean set is returned verbatim.
        assert_eq!(
            cleaned.mutation_syntax.merge,
            dangling.mutation_syntax.merge
        );
        assert_eq!(
            FeatureSet::POSTGRES.without_dangling_dependents(),
            FeatureSet::POSTGRES,
        );
    }

    #[test]
    fn every_shipped_preset_has_no_grammar_conflict() {
        // The parser-position sibling of the per-preset lexical and dependency assertions:
        // no shipped preset enables two features that contend for the same grammar head, so this
        // one loop covers every registered variant collectively (any preset tripping a new guard
        // would return `Some` here). The four registered pairs are each split across presets:
        // `prefix_colon_alias` vs `semi_structured_access` (DuckDB/Lenient have the prefix alias,
        // Snowflake/Databricks the path); `do_statement` vs `do_expression_list` (PostgreSQL/
        // Lenient the block, MySQL the expression list); `prepared_statements` vs
        // `prepared_statements_from` (DuckDB/PostgreSQL/Lenient the typed-`AS` form, MySQL the
        // `FROM`/`USING` form); and `access_control_account_grants` vs
        // `access_control_extended_objects` (MySQL the account route, everyone else the extended
        // objects). Lenient — the union preset that pairs `describe` with `describe_summarize` —
        // stays clean because that shared `DESCRIBE` leader is a deterministic dispatch-order
        // union, not a registered conflict.
        for preset in [
            FeatureSet::ANSI,
            FeatureSet::POSTGRES,
            FeatureSet::MYSQL,
            FeatureSet::SQLITE,
            FeatureSet::DUCKDB,
            FeatureSet::BIGQUERY,
            FeatureSet::HIVE,
            FeatureSet::CLICKHOUSE,
            FeatureSet::DATABRICKS,
            FeatureSet::MSSQL,
            FeatureSet::SNOWFLAKE,
            FeatureSet::REDSHIFT,
            FeatureSet::LENIENT,
        ] {
            assert_eq!(preset.grammar_conflict(), None);
            assert!(preset.has_no_grammar_conflict());
        }
    }

    #[test]
    fn grammar_conflict_detects_prefix_colon_versus_semi_structured() {
        // DuckDB ships the prefix colon alias with `semi_structured_access` off; forcing the
        // path grammar on alongside it pairs the two `<ident> :` head claimants, which the
        // registry must flag. (`named_colon` stays off, so this is a pure grammar contention
        // and not the sibling `ColonParameterVersusSliceBound` lexical conflict.)
        let both =
            FeatureSet::DUCKDB.with(FeatureDelta::EMPTY.expression_syntax(ExpressionSyntax {
                semi_structured_access: true,
                ..FeatureSet::DUCKDB.expression_syntax
            }));
        assert_eq!(
            both.grammar_conflict(),
            Some(GrammarConflict::PrefixColonAliasVersusSemiStructuredAccess),
        );
        assert!(!both.has_no_grammar_conflict());
        // Neither flag alone is a conflict.
        assert_eq!(FeatureSet::DUCKDB.grammar_conflict(), None);
        assert_eq!(FeatureSet::SNOWFLAKE.grammar_conflict(), None);
    }

    #[test]
    fn grammar_conflict_stays_mece_with_the_lexical_and_dependency_siblings() {
        // A pure grammar-position contention carries no tokenizer trigger and no base-flag
        // dependency: DuckDB plus `semi_structured_access` is a grammar conflict yet stays
        // lexically consistent (`named_colon` is off, so `:` has one claimant) and
        // dependency-clean.
        let grammar_only =
            FeatureSet::DUCKDB.with(FeatureDelta::EMPTY.expression_syntax(ExpressionSyntax {
                semi_structured_access: true,
                ..FeatureSet::DUCKDB.expression_syntax
            }));
        assert_eq!(
            grammar_only.grammar_conflict(),
            Some(GrammarConflict::PrefixColonAliasVersusSemiStructuredAccess),
        );
        assert_eq!(grammar_only.lexical_conflict(), None);
        assert_eq!(grammar_only.feature_dependencies(), None);
    }

    #[test]
    fn grammar_conflict_detects_do_statement_versus_do_expression_list() {
        // MySQL ships the `DO <expr-list>` statement (`do_expression_list` on) with the PostgreSQL
        // code block off; forcing `do_statement` on alongside it pairs the two `DO`-head readings.
        let both = FeatureSet::MYSQL.with(FeatureDelta::EMPTY.utility_syntax(UtilitySyntax {
            do_statement: true,
            ..FeatureSet::MYSQL.utility_syntax
        }));
        assert_eq!(
            both.grammar_conflict(),
            Some(GrammarConflict::DoStatementVersusDoExpressionList),
        );
        assert!(!both.has_no_grammar_conflict());
        // Neither dialect alone is a conflict.
        assert_eq!(FeatureSet::MYSQL.grammar_conflict(), None);
        assert_eq!(FeatureSet::POSTGRES.grammar_conflict(), None);
    }

    #[test]
    fn grammar_conflict_detects_prepared_statements_versus_prepared_statements_from() {
        // DuckDB ships the typed-`AS` prepared-statement lifecycle (`prepared_statements` on) with
        // MySQL's `FROM`/`USING` form off; forcing `prepared_statements_from` on alongside it pairs
        // the two lifecycles on the shared `PREPARE`/`EXECUTE`/`DEALLOCATE` keywords — the
        // combination whose `DEALLOCATE` tail is incoherent with the dispatch order.
        let both = FeatureSet::DUCKDB.with(FeatureDelta::EMPTY.utility_syntax(UtilitySyntax {
            prepared_statements_from: true,
            ..FeatureSet::DUCKDB.utility_syntax
        }));
        assert_eq!(
            both.grammar_conflict(),
            Some(GrammarConflict::PreparedStatementsVersusPreparedStatementsFrom),
        );
        assert!(!both.has_no_grammar_conflict());
        // Neither dialect alone is a conflict.
        assert_eq!(FeatureSet::DUCKDB.grammar_conflict(), None);
        assert_eq!(FeatureSet::MYSQL.grammar_conflict(), None);
    }

    #[test]
    fn grammar_conflict_detects_account_grants_versus_extended_objects() {
        // MySQL ships the account-based grant route (`access_control_account_grants` on) with the
        // extended-object grammar off; forcing `access_control_extended_objects` on alongside it
        // pairs the two `GRANT`/`REVOKE` grammars — the route-flag conflict, where the account
        // route deadens the explicitly-enabled extended-object reading. (`access_control` stays on,
        // so the extended-object flag is not a dependency violation.)
        let both = FeatureSet::MYSQL.with(FeatureDelta::EMPTY.access_control_syntax(
            AccessControlSyntax {
                access_control_extended_objects: true,
                ..FeatureSet::MYSQL.access_control_syntax
            },
        ));
        assert_eq!(
            both.grammar_conflict(),
            Some(GrammarConflict::AccountGrantsVersusExtendedObjects),
        );
        assert!(!both.has_no_grammar_conflict());
        assert_eq!(both.feature_dependencies(), None);
        // Neither route alone is a conflict.
        assert_eq!(FeatureSet::MYSQL.grammar_conflict(), None);
        assert_eq!(FeatureSet::POSTGRES.grammar_conflict(), None);
    }
}
