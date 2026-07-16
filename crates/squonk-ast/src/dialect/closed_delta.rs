// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Shared closed-delta helpers for preset honesty tests.
//!
//! A satellite preset claims "ANSI + documented deltas". These helpers make that
//! claim checkable: list every top-level [`FeatureSet`] axis that differs from a
//! base, then assert the list matches the documented closed set.

use super::FeatureSet;

/// Names of top-level [`FeatureSet`] axes whose values differ between `base` and `other`.
///
/// Used by preset unit tests so "everything else equals the base" is a single
/// assert against an explicit allowlist rather than a hand-maintained field laundry list.
pub fn divergent_axes(base: &FeatureSet, other: &FeatureSet) -> Vec<&'static str> {
    let mut out = Vec::new();
    macro_rules! check {
        ($field:ident) => {
            if base.$field != other.$field {
                out.push(stringify!($field));
            }
        };
    }
    check!(identifier_casing);
    check!(identifier_quotes);
    check!(default_null_ordering);
    check!(reserved_column_name);
    check!(reserved_function_name);
    check!(reserved_type_name);
    check!(reserved_bare_alias);
    check!(reserved_as_label);
    check!(catalog_qualified_names);
    check!(byte_classes);
    check!(binding_powers);
    check!(set_operation_powers);
    check!(string_literals);
    check!(numeric_literals);
    check!(parameters);
    check!(session_variables);
    check!(identifier_syntax);
    check!(table_expressions);
    check!(join_syntax);
    check!(table_factor_syntax);
    check!(expression_syntax);
    check!(operator_syntax);
    check!(call_syntax);
    check!(string_func_forms);
    check!(aggregate_call_syntax);
    check!(predicate_syntax);
    check!(pipe_operator);
    check!(double_ampersand);
    check!(keyword_operators);
    check!(caret_operator);
    check!(hash_bitwise_xor);
    check!(comment_syntax);
    check!(mutation_syntax);
    check!(statement_ddl_gates);
    check!(view_sequence_clause_syntax);
    check!(create_table_clause_syntax);
    check!(column_definition_syntax);
    check!(constraint_syntax);
    check!(index_alter_syntax);
    check!(existence_guards);
    check!(select_syntax);
    check!(query_tail_syntax);
    check!(grouping_syntax);
    check!(utility_syntax);
    check!(transaction_syntax);
    check!(show_syntax);
    check!(maintenance_syntax);
    check!(access_control_syntax);
    check!(type_name_syntax);
    check!(target_spelling);
    out
}

/// Assert `other` differs from `base` on exactly the axes in `expected` (order-independent).
pub fn assert_closed_delta(base: &FeatureSet, other: &FeatureSet, expected: &[&'static str]) {
    let mut got = divergent_axes(base, other);
    let mut exp: Vec<&str> = expected.to_vec();
    got.sort_unstable();
    exp.sort_unstable();
    assert_eq!(
        got, exp,
        "closed-delta mismatch: preset differs from base on {got:?}, expected {exp:?}"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ansi_has_no_delta_against_itself() {
        assert_closed_delta(&FeatureSet::ANSI, &FeatureSet::ANSI, &[]);
    }

    #[test]
    fn redshift_closed_delta_is_casing_and_table_json_path() {
        assert_closed_delta(
            &FeatureSet::ANSI,
            &FeatureSet::REDSHIFT,
            &["identifier_casing", "table_expressions"],
        );
    }
}
