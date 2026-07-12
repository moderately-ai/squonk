<!-- SPDX-License-Identifier: MIT -->
<!-- Copyright (c) 2026 Moderately AI Inc. -->

# Adding a grammar family

1. Measure the uncovered surface from the pinned grammar and real engine. Treat grammar
   documentation as a hypothesis and the live engine as behavioral truth.
2. Model a family as one AST node with a behavior-named, mutually exclusive subcommand axis.
   Never branch product code on dialect identity.
3. Add the AST node and parser/render support in their family modules. Change sourcegen inputs
   and run `cargo run -p squonk-sourcegen`; never edit generated files directly.
4. Register curated conformance cases, including accept/reject and render round trips. Exercise
   both the default behavior and a dialect override.
5. Re-measure coverage and verdict pins from the final tree. Do not update measured values by
   arithmetic.
6. Audit the changed family's hand-written render implementations before updating its
   render-shape fingerprint.
7. Run `cargo fmt --all`, `cargo xtask preflight`, and `cargo xtask feature-matrix` when feature
   wiring changed.

Large families may land incrementally, but the node and subcommand axis should remain stable
throughout the series.
