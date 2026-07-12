# ADR-0009: AST extensibility — the Other(X) seam & Dialect::Ext

- **Status:** Accepted (2026-06-26)
- **Atoms:** A21, A22

## Context

The prior art's `Statement`/`Expr` are closed, non-generic enums, and its parse hooks return only bundled types — so a downstream consumer cannot add a node kind without forking. We need an open AST without resurrecting the closed-enum wall or paying a per-node cost when unused.

## Decision

- **A single `Other(X = NoExt)` extension variant** (simplified "Trees that Grow") on a *small* set of openable enums (`Statement`, `Expr`, `TableFactor`, `ColumnOption`, `TableConstraint`, and — since the 2026-07-06 amendment — `DataType`). `NoExt` is uninhabited, so `Statement` (= `Statement<NoExt>`) keeps its node sizes and the `Other` arm is statically dead for the stock parser — **zero cost when unused**.
- **`X: Extension`** — the trait itself bounds `Clone + Debug + Eq + Hash + Spanned` (`crates/squonk-ast/src/ast/ext.rs`); the `Render` and `Visit` bounds this ADR's spirit wants are applied at the renderer/visitor **call sites**, not on the trait, to avoid an AST → renderer/generated-code layering cycle. Round-trip/traversal safety is thus enforced where an extension is rendered or visited (so the Display-panics / round-trip-blindness failures still can't recur) rather than blanket on the trait.
- **Blanket impl, on purpose.** `Extension` is blanket-implemented for every qualifying type (`impl<T: Clone + Debug + Eq + Hash + Spanned> Extension for T {}` in `crates/squonk-ast/src/ast/ext.rs`), so a consumer's node type needs zero boilerplate and both the `DynExt` hatch and the typed path just work. The accepted, permanent cost: a blanket impl forecloses ever adding a required item to `Extension` (every downstream type would break) and conflicts with any future specific impl, so `Extension` can never grow behaviour — it is a **bound-alias forever**. This is the layering above taken to its conclusion: behaviour lives on the `Render`/`Visit` call-site bounds, never on `Extension`, and any future per-extension behaviour is a *new* opt-in trait, not a required item here.
- **Hybrid:** *popular* dialect-specific constructs (`QUALIFY`, `PIVOT`, `RETURNING`, `CONNECT BY`, …) are first-class variants in the shared AST; `Other(X)` is for the truly custom tail. `Box<dyn AstExt>` is the *blessed* dynamic instantiation of `X`, paying vtable/alloc only then.
- **`Dialect::Ext`** — the dialect's associated `type Ext: Extension` *is* the AST's `X`, so `{ features, Ext }` is the entire dialect surface.

## Consequences

- The associated `Ext` type makes `&dyn Dialect` non-object-safe, which only bites *runtime* selection. The realistic cases are covered: compile-time-known dialect → monomorphized `Parser<D>`; runtime selection among **builtins** (all `Ext = NoExt`) → a `BuiltinDialect` value-enum + object-safe `dyn Dialect<Ext = NoExt>` → plain `Statement`.
- **Documented limitation (not handled now):** runtime-selecting among *custom* dialects with *different* `Ext`, or composing multiple independent custom node-sets in one build (a dialect-plugin platform). Escape hatch if ever needed: `X = Box<dyn AstExt>`. The intended downstream consumer does not hit this (one consumer, one `Ext`, runtime-selects builtins).
- **Contingent on codegen (ADR-0013):** threading `<X>` by hand through the per-variant walks would resurrect the unergonomic Trees-that-Grow tax, so the generated walks thread it.

## Alternatives considered

Full per-constructor Trees-that-Grow (rejected — where-clause explosion in Rust); à-la-carte / frunk coproducts (research-grade); a closed union-superset (the prior art — unextendable without forking).

## Interconnects

- code: `crates/squonk-ast/src/ast/ext.rs` — `Extension`, `NoExt`; the `Other { ext: X, meta }` seam on `Statement`, `Expr`, `TableFactor`, `ColumnOption`, `TableConstraint`, `DataType`
- invariant: the `Extension` supertrait bound is exactly `Clone + Debug + Eq + Hash + Spanned` (Render/Visit are applied at their call sites), and it is blanket-implemented for every qualifying type (`impl<T: Clone + Debug + Eq + Hash + Spanned> Extension for T {}`), so the trait is a bound-alias that can never grow a required item; `NoExt` is uninhabited; exactly six enums carry the `ext: X` seam.
- xtask: `cargo xtask extension-seam`.

## References

Atoms A21–A22. Najd & Peyton Jones "Trees that Grow"; DataFusion `UserDefinedLogicalNode` (the `Box<dyn>` precedent).

## Amendment (2026-06-30): the de-facto extension-seam model, and `BuiltinDialect` built

Two structural properties of the `Other(X)` seam surfaced during the dialect-packaging work (`spike-structural-dialect-extensibility-stress-…`). We record the **de-facto model** so the asymmetry is intentional, not a surprise; we deliberately did **not** add finer-grained hooks.

- **The seam is for out-of-tree dialects only; builtins extend the canonical AST directly.** `Other(X)` is reachable only when `Ext` is inhabited, and every shipped builtin is `Ext = NoExt` (uninhabited), so a builtin literally cannot use the seam. This is by design: a builtin that needs a new statement/expression/clause adds a **first-class variant or field** to the shared AST (the *hybrid* policy above — popular dialect constructs are first-class; `Other(X)` is the truly-custom out-of-tree tail). The seam earns its keep only for a downstream consumer that adds node kinds without forking.

- **The seam is whole-node-coarse, by choice.** It opens exactly five whole-node enums (`Statement`, `Expr`, `TableFactor`, `ColumnOption`, `TableConstraint`) with no sub-clause granularity. A structurally-divergent feature that needs a new *field* on an existing shared node (T-SQL `SELECT … INTO`'s `Select.into`; an `OUTPUT` clause extending `Returning`) is therefore modelled as a **first-class field on the canonical shape**, gated as data (ADR-0011), not threaded through the seam. We rejected adding sub-clause extension hooks: they cut against the minimal-codebase value proposition, and ADR-0001/0007 (no-lifetime, fixed-size AST) make per-clause `Box<dyn …>` hooks costly for a payoff no shipped dialect needs.

So the in-tree extension axis is "add canonical variants/fields, compose dialect behaviour as `FeatureSet` data, and *package* per dialect behind a cargo feature"; the `Other(X)`/`X = Box<dyn AstExt>` axis is the separate out-of-tree affordance.

**Runtime selection is now built.** The `BuiltinDialect` value-enum + object-safe runtime path this ADR sketched (the `&dyn Dialect`-non-object-safe escape) is implemented as `squonk::dialect::{BuiltinDialect, parse_with_builtin}`: a value-enum over the `NoExt` builtins dispatched by value to the monomorphized parser (no `dyn Dialect` needed), each arm gated by the dialect's cargo feature, with a name for a disabled/unknown dialect resolving to a clean `None`.

## Amendment (2026-07-06): the sixth seam — `DataType::Other` + `Dialect::parse_data_type_hook`

A planner consumer needs to parse and carry a **host-owned type node** (a custom cast target, column type, function-signature type, or composite field) without spelling it as a stock variant or forcing it into `DataType::UserDefined`. This is the *type-production* analogue of the five whole-node seams, so `DataType` gains an `Other { ext: X, meta }` arm and the `Dialect` trait gains a sixth whole-node hook, `parse_data_type_hook` (same 3-state `Handled | NotHandled | Err` contract; consulted at the head of `Parser::parse_data_type`, the single entry every type position funnels through). The openable set is now **six**; the `cargo xtask extension-seam` gate and its count move to 6 in lock-step.

- **This does not reopen the sub-clause question the 2026-06-30 amendment closed.** `DataType` is a *whole node* (a first-class enum the AST already owns), reached in type position exactly as `Expr` is reached in value position — not a sub-clause hook on an existing node. The "whole-node-coarse, by choice" principle *permits* a new whole-node seam when a genuinely distinct node position is otherwise unreachable; it *forbids* per-clause `Box<dyn …>` granularity. A type position is unreachable through any of the five (a column/cast/`RETURNS` type is neither an `Expr` nor a whole `Statement`/`TableFactor`), and the `Expr::Other` route cannot cover column or function-signature type positions, which carry no enclosing `Expr`.

- **Why the typed generic seam and not a carrier variant.** Keeping `DataType` non-generic and adding an *inhabited* carrier (`Other(Box<dyn AstExt>)`) was rejected: it is present — and pattern-forced — in the stock enum, breaking the "`Other(NoExt)` statically dead / zero cost / byte-identical stock" invariant the five seams hold (verified: every size budget is unchanged after the change). The typed `Other { ext: X }` under the uninhabited `NoExt` is the only design that carries a host type *and* preserves that invariant. The `DynExt` hatch is not an alternative to it — a consumer that wants dynamic composition simply instantiates `X = DynExt`, exactly as for the other five seams.

- **The generic ripple is mechanical and reachability-following.** `DataType` becoming `DataType<X = NoExt>` threads `X` through every node that transitively owns a type: `StructTypeField`; the function-signature nodes (`CreateFunction`, `FunctionParam`, `RoutineSignature`) and the `GRANT`/`REVOKE` chain that references them (`GrantObject`, `AccessControlStatement`); `CommentTarget`/`CommentOnStatement`; and `TableFunctionColumn`. Every touched node genuinely contains a type a host could own, so the threading reflects real reachability rather than gratuitous generality. The generated walks thread `<X>` automatically (ADR-0013), and the `= NoExt` default keeps every stock call site — builtins, conformance, tests — compiling unchanged apart from the trivial uninhabited `Other { ext, .. } => match *ext {}` arm the exhaustive matchers gain.
