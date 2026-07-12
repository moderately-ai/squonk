// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Release-contract support-tier assignment for every shipped preset and optional
//! product surface.
//!
//! The value types ([`SupportTier`], [`SupportEvidence`]) live in `squonk-ast`
//! beside [`Maturity`](crate::ast::dialect::Maturity); this module holds the
//! *assignment*, which is release-specific and references identities this crate owns:
//! [`BuiltinDialect`] for presets and [`ProductSurface`] for the feature-gated
//! product APIs.
//!
//! Two rules keep the assignment honest, both enforced by the release-tier gate in
//! the conformance crate (`squonk-conformance`, `support_tiers` module):
//!
//! 1. Every entry has an explicit tier and a named [`SupportEvidence`].
//! 2. A [`SupportTier::Stable`] entry must cite
//!    [authoritative](SupportEvidence::is_authoritative) evidence — no preset can be
//!    advertised as stable on documentation or comparison alone.
//!
//! The *live* backing for the engine-differential stable claims (that the named
//! oracle actually ran at the pinned version) is the nightly parity guard
//! (`oracle-nightly.yml`); this table names the source of truth, and that guard
//! keeps it true at runtime.

use crate::ast::dialect::{SupportEvidence, SupportTier};
use crate::dialect::BuiltinDialect;

impl BuiltinDialect {
    /// This preset's release-contract [`SupportTier`].
    ///
    /// See [`support_evidence`](Self::support_evidence) for the named source of truth
    /// behind the tier, and the module docs for the two invariants the conformance
    /// gate enforces over the pair.
    pub const fn support_tier(self) -> SupportTier {
        match self {
            // Standard baseline: authoritative via the standard text + enforced
            // round-trip property + accept corpus (no single vendor engine).
            Self::Ansi => SupportTier::Stable,
            // Engine-differential parity at bar A, held by the nightly oracle guard.
            #[cfg(feature = "postgres")]
            Self::Postgres => SupportTier::Stable,
            #[cfg(feature = "mysql")]
            Self::MySql => SupportTier::Stable,
            #[cfg(feature = "sqlite")]
            Self::Sqlite => SupportTier::Stable,
            #[cfg(feature = "duckdb")]
            Self::DuckDb => SupportTier::Stable,
            // Real engine oracle wired but partial coverage and not yet in the default
            // gate — a promotion candidate, not a stable claim.
            #[cfg(feature = "clickhouse")]
            Self::ClickHouse => SupportTier::Preview,
            // Non-authoritative comparison oracle (sqlglot) over a modelled surface.
            #[cfg(feature = "bigquery")]
            Self::BigQuery => SupportTier::Preview,
            // Permissive union: constructed to match no single engine by design.
            #[cfg(feature = "lenient")]
            Self::Lenient => SupportTier::Preview,
            // Documentation-derived; engine-oracle acquisition blocked.
            #[cfg(feature = "hive")]
            Self::Hive => SupportTier::Experimental,
            #[cfg(feature = "databricks")]
            Self::Databricks => SupportTier::Experimental,
            #[cfg(feature = "mssql")]
            Self::Mssql => SupportTier::Experimental,
            #[cfg(feature = "snowflake")]
            Self::Snowflake => SupportTier::Experimental,
            #[cfg(feature = "redshift")]
            Self::Redshift => SupportTier::Experimental,
        }
    }

    /// The named source of truth backing this preset's [`support_tier`](Self::support_tier).
    pub const fn support_evidence(self) -> SupportEvidence {
        match self {
            Self::Ansi => SupportEvidence::StandardReference {
                note: "ISO/IEC 9075:2016 baseline, held by the structural round-trip property \
                       (parse(render(x)) == x), the sqllogictest accept corpus, and a documented \
                       PostgreSQL nearest-engine delta ledger (oracle-parity-ansi)",
            },
            #[cfg(feature = "postgres")]
            Self::Postgres => SupportEvidence::EngineDifferential {
                engine: "libpg_query",
                version: "pg_query 6.1.1 (PostgreSQL 17)",
                method: "raw-parse-tree differential over the vendored corpus (ParseOnly)",
            },
            #[cfg(feature = "mysql")]
            Self::MySql => SupportEvidence::EngineDifferential {
                engine: "mysql",
                version: "8.4.10",
                method: "live-server prepare + parse differential (oracle-mysql)",
            },
            #[cfg(feature = "sqlite")]
            Self::Sqlite => SupportEvidence::EngineDifferential {
                engine: "sqlite",
                version: "rusqlite 0.40 (bundled SQLite)",
                method: "in-process prepare differential (oracle-engines)",
            },
            #[cfg(feature = "duckdb")]
            Self::DuckDb => SupportEvidence::EngineDifferential {
                engine: "libduckdb",
                version: "1.5.4",
                method: "in-process extract_statements differential (oracle-engines)",
            },
            #[cfg(feature = "clickhouse")]
            Self::ClickHouse => SupportEvidence::EngineDifferential {
                engine: "clickhouse-local",
                version: "25.5.1",
                method: "external-process EXPLAIN AST over a partial modelled surface; not yet in \
                         the default nightly gate (oracle-clickhouse, external-blocked)",
            },
            #[cfg(feature = "bigquery")]
            Self::BigQuery => SupportEvidence::Comparison {
                tool: "sqlglot",
                note: "ParseOnly cross-check over a modelled BigQuery surface; not authoritative \
                       (the ZetaSQL reference oracle is blocked — oracle-parity-bigquery)",
            },
            #[cfg(feature = "lenient")]
            Self::Lenient => SupportEvidence::Constructed {
                note: "permissive parse-anything union of every dialect surface; matches no single \
                       engine by design (oracle-parity-lenient)",
            },
            #[cfg(feature = "hive")]
            Self::Hive => SupportEvidence::DocumentationDerived {
                note: "dialect-reference library; engine-oracle acquisition blocked \
                       (oracle-parity-hive)",
            },
            #[cfg(feature = "databricks")]
            Self::Databricks => SupportEvidence::DocumentationDerived {
                note: "dialect-reference library; engine-oracle acquisition blocked \
                       (oracle-parity-databricks)",
            },
            #[cfg(feature = "mssql")]
            Self::Mssql => SupportEvidence::DocumentationDerived {
                note: "dialect-reference library; engine-oracle acquisition blocked \
                       (oracle-parity-mssql)",
            },
            #[cfg(feature = "snowflake")]
            Self::Snowflake => SupportEvidence::DocumentationDerived {
                note: "dialect-reference library; engine-oracle acquisition blocked \
                       (oracle-parity-snowflake)",
            },
            #[cfg(feature = "redshift")]
            Self::Redshift => SupportEvidence::DocumentationDerived {
                note: "dialect-reference library; engine-oracle acquisition blocked \
                       (oracle-parity-redshift)",
            },
        }
    }
}

/// An optional, feature-gated product surface that is part of the release contract
/// even though it is not a dialect. A gated API still ships, so it still needs a tier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ProductSurface {
    /// The pretty-printing formatter (`document-render` feature).
    DocumentRender,
    /// The serialized AST wire schema shared by the bindings (`serde` feature).
    SerdeAstSchema,
    /// The WebAssembly / JavaScript bindings (`squonk-wasm`).
    WasmBindings,
    /// The Python bindings (`squonk-python`).
    PythonBindings,
}

impl ProductSurface {
    /// Every product surface in the release contract, whether or not it is compiled
    /// into the current build — a gated API is still a shipped promise.
    pub const ALL: &'static [ProductSurface] = &[
        ProductSurface::DocumentRender,
        ProductSurface::SerdeAstSchema,
        ProductSurface::WasmBindings,
        ProductSurface::PythonBindings,
    ];

    /// Stable machine-readable id for this surface.
    pub const fn id(self) -> &'static str {
        match self {
            Self::DocumentRender => "document-render",
            Self::SerdeAstSchema => "serde-ast-schema",
            Self::WasmBindings => "wasm-bindings",
            Self::PythonBindings => "python-bindings",
        }
    }

    /// This surface's release-contract [`SupportTier`].
    pub const fn support_tier(self) -> SupportTier {
        match self {
            // Ships as a documented preview (see `format` module docs): parse-back and
            // spelling fidelity are guaranteed, but layout is not full-fidelity.
            Self::DocumentRender => SupportTier::Preview,
            // Frozen, versioned, drift-gated wire contract.
            Self::SerdeAstSchema => SupportTier::Stable,
            // Wrap the stable parser + wire schema, but distribution is not yet cut.
            Self::WasmBindings => SupportTier::Preview,
            Self::PythonBindings => SupportTier::Preview,
        }
    }

    /// The named source of truth backing this surface's [`support_tier`](Self::support_tier).
    pub const fn support_evidence(self) -> SupportEvidence {
        match self {
            Self::DocumentRender => SupportEvidence::Constructed {
                note: "documented v1 preview: parse-back + spelling fidelity guaranteed and pinned \
                       by the format::coverage fixtures; full-fidelity layout remains future \
                       work (see the format module docs)",
            },
            Self::SerdeAstSchema => SupportEvidence::ContractGate {
                artifact: "release/schema/wire-schema.v1.json",
                note: "wire schema v1, drift-gated by the wire_schema test and a frozen compat \
                       baseline (docs/schema-contract.md)",
            },
            Self::WasmBindings => SupportEvidence::Constructed {
                note: "wraps the stable parser and wire schema v1; npm distribution not yet cut",
            },
            Self::PythonBindings => SupportEvidence::Constructed {
                note: "wraps the stable parser and wire schema v1; wheel distribution not yet cut",
            },
        }
    }
}
