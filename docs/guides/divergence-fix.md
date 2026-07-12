<!-- SPDX-License-Identifier: MIT -->
<!-- Copyright (c) 2026 Moderately AI Inc. -->

# Fixing a differential divergence

1. Reproduce and minimize the failing input. Identify whether the mismatch is accept/reject,
   statement segmentation, structure, or an internal panic.
2. Probe the real engine and record its version, exact input, verdict, and any parse-versus-bind
   distinction in the permanent test or fixture.
3. Fix the behavior-named dialect data or the tokenizer/parser/harness contract responsible for
   the mismatch. Do not add dialect-identity branches.
4. Add the minimized input to the stable replay corpus and demonstrate fail-before/pass-after.
5. Run a bounded foreground fuzz soak and investigate every new finding.
6. A temporary divergence entry must include exact SQL and a concise rationale, and its replay
   must prove the divergence still exists. Remove the entry as soon as the implementations agree.
7. Run `cargo fmt --all` and `cargo xtask preflight`; run the relevant oracle lane when the
   changed behavior is engine-gated.
