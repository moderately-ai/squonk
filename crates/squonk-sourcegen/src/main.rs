// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Writer entry point for the `squonk` AST code generator.
//!
//! `cargo run -p squonk-sourcegen` (re)writes the checked-in traversal files
//! in `crates/squonk-ast/src/generated/`. The generation logic lives in the
//! library crate so the drift test can reuse it; this binary only persists it.

fn main() -> std::io::Result<()> {
    // Pre-flight the runtime root derivation (cwd walk-up / override — resolved at
    // invocation, never baked at compile time) so a misinvocation outside any
    // workspace exits with the one-line error instead of a panic from the first
    // path helper. Generation below re-derives the same root per path.
    if let Err(err) = squonk_sourcegen::try_workspace_root() {
        eprintln!("error: {err}");
        std::process::exit(2);
    }
    for file in squonk_sourcegen::generate_all() {
        std::fs::write(&file.path, file.contents)?;
        println!("wrote {}", file.path.display());
    }
    Ok(())
}
