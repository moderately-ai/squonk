<!-- SPDX-License-Identifier: MIT -->
<!-- Copyright (c) 2026 Moderately AI Inc. -->

# Dialect & syntax requests

A multi-dialect SQL parser attracts "add dialect X" and "accept syntax Y" requests from day one. We welcome them — this page states, up front, how we decide what to take and what evidence a request needs, so that filing one is worthwhile on both sides. It is the outward-facing view of a framework that otherwise lives in [`docs/architecture.md`](architecture.md) and the [ADRs](adr/); the per-preset tiers themselves are generated into [`docs/support-tiers.md`](support-tiers.md) and are not restated here.

## What a good request looks like

The minimal shape we can act on is **SQL input + dialect + engine citation** — the same three fields the [feature/dialect request form](../.github/ISSUE_TEMPLATE/feature_request.yml) collects and a subset of what the [bug form](../.github/ISSUE_TEMPLATE/bug_report.yml) asks:

- **SQL input** — a concrete snippet squonk should accept (and, where it sharpens the boundary, one it should keep rejecting). Not a prose description of a feature.
- **Dialect / engine** — which engine's grammar the syntax belongs to.
- **Engine citation** — a link to that engine's own documentation or grammar showing the syntax is real and accepted. This is the evidence bar; a request without it cannot be triaged, because we never model syntax we cannot point at.

A request that names the construct in prose but carries no SQL and no citation will be asked for those before it can move.

## Where a new dialect enters, and how it graduates

New dialects enter at the **experimental** tier: a conservative, documentation-derived preset with no differential oracle wired. See [`docs/support-tiers.md`](support-tiers.md) for what each tier promises and which presets currently sit where — the definitions are authoritative there and deliberately not duplicated here.

Graduating a preset to **stable** requires clearing the evidence bar that tier names: an *authoritative* source of truth — a real engine or reference-parser differential, or the SQL standard itself — wired behind an **enforced gate**, holding over-acceptance (accepting SQL the engine rejects) at zero over a vendored corpus. Documentation alone never reaches stable; without an acquirable engine oracle a preset stays experimental however well-cited, because there is nothing to hold it to. Engine acquisition being blocked is the usual reason a well-documented dialect cannot advance, and that blocker is tracked per preset in the support-tiers source of truth.

## Two rules that shape what a preset accepts

These are the gates a request runs into once accepted, and knowing them up front explains why a well-cited feature can still land "off by default" in every preset but `lenient`:

- **The conservative doc-derived preset rule.** A preset that has no differential oracle enables only the surface that already has a modelled, tested parser gate, and rejects unmodelled syntax cleanly. We ship a preset that under-accepts honestly rather than one that optimistically accepts syntax nobody can verify — an over-accepting preset would silently mis-trust structure, which is worse than a clean parse error.
- **The family rule as an outward gate.** A behaviour flag whose over-acceptance no oracle can measure stays **off across its whole family**, enabled only in `lenient` (the permissive parse-anything union that matches no single engine by design). We do not cherry-pick one un-oracled construct into a stable preset because it looks harmless; the grouping is the gate. So "engine X documents syntax Y" is necessary but not automatically sufficient for Y to be on in preset X — if X has no oracle to bound the blast radius, Y rides the family rule.

## Routing: issues, not Discussions

**File dialect and syntax requests as GitHub issues using the request form; GitHub Discussions is deliberately not enabled at launch.** Questions already route to opensource@moderately.ai and security reports to the [SECURITY.md](../SECURITY.md) private-advisory flow, so a second public forum would only fragment triage and invite unbounded "add dialect X" chatter with no maintainer bandwidth committed to answering it — the structured issue form plus the evidence bar above is the channel that produces actionable requests. Enabling Discussions later is a one-click, reversible repo toggle if demonstrated demand and capacity justify it; an abandoned Discussions tab is not.

## See also

- [`docs/support-tiers.md`](support-tiers.md) — the authoritative per-preset tier and source-of-truth table.
- [`SUPPORT.md`](../SUPPORT.md) — where to take questions, bugs, and security reports.
- [`docs/architecture.md`](architecture.md) — the dialects-as-data model and the full statement surface.
