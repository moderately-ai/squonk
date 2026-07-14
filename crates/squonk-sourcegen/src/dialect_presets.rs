// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Drift gate for shipped dialect declarations.

use std::fs;

use syn::visit::{self, Visit};
use syn::{ExprStruct, ImplItem, Item, Visibility};

use crate::workspace_root;

const NON_PRESET_MODULES: &[&str] = &[
    "conflict.rs",
    "feature_set_generated.rs",
    "head_contention.rs",
    "keyword.rs",
    "lex_class.rs",
    "mod.rs",
    "standard_catalog.rs",
    "support_tier.rs",
];

#[derive(Default)]
struct StructUpdateFinder {
    found: bool,
}

impl<'ast> Visit<'ast> for StructUpdateFinder {
    fn visit_expr_struct(&mut self, node: &'ast ExprStruct) {
        self.found |= node.rest.is_some();
        visit::visit_expr_struct(self, node);
    }
}

/// Reject struct-update inheritance in every public associated preset constant.
///
/// Test-only ad hoc feature sets remain free to use updates because they live inside
/// `#[cfg(test)]` modules, not public associated constants at the module root.
pub(crate) fn assert_explicit() {
    let directory = workspace_root().join("crates/squonk-ast/src/dialect");
    let mut violations = Vec::new();

    for entry in fs::read_dir(&directory).expect("read dialect source directory") {
        let path = entry.expect("read dialect directory entry").path();
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs")
            || NON_PRESET_MODULES.contains(&file_name)
        {
            continue;
        }

        let source = fs::read_to_string(&path).expect("read dialect preset source");
        let syntax = syn::parse_file(&source).expect("parse dialect preset source");
        for item in syntax.items {
            match item {
                Item::Impl(item_impl) => {
                    for item in item_impl.items {
                        let ImplItem::Const(item_const) = item else {
                            continue;
                        };
                        if !matches!(item_const.vis, Visibility::Public(_)) {
                            continue;
                        }
                        let mut finder = StructUpdateFinder::default();
                        finder.visit_expr(&item_const.expr);
                        if finder.found {
                            violations.push(format!("{file_name}::{}", item_const.ident));
                        }
                    }
                }
                Item::Const(item_const) if matches!(item_const.vis, Visibility::Public(_)) => {
                    let mut finder = StructUpdateFinder::default();
                    finder.visit_expr(&item_const.expr);
                    if finder.found {
                        violations.push(format!("{file_name}::{}", item_const.ident));
                    }
                }
                _ => {}
            }
        }
    }

    violations.sort();
    assert!(
        violations.is_empty(),
        "shipped dialect presets must enumerate every field; struct-update inheritance found in: {}",
        violations.join(", "),
    );
}

#[cfg(test)]
mod tests {
    #[test]
    fn shipped_presets_enumerate_every_field() {
        super::assert_explicit();
    }
}
