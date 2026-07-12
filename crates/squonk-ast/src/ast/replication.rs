// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! MySQL replication-administration statement AST nodes.
//!
//! The five measured MySQL replication-control families, gated as one cohesive unit by
//! [`UtilitySyntax::replication_statements`](crate::dialect::UtilitySyntax):
//!
//! - `CHANGE REPLICATION SOURCE TO <option-list> [FOR CHANNEL '<ch>']` — configure the
//!   asynchronous-replication source connection (`sql_yacc.yy` `change_replication_stmt`,
//!   `source_defs`).
//! - `CHANGE REPLICATION FILTER <rule-list> [FOR CHANNEL '<ch>']` — set the replication
//!   filtering rules (`filter_defs`).
//! - `START REPLICA [<threads>] [UNTIL <cond>] [<connection>] [FOR CHANNEL '<ch>']` and
//!   `STOP REPLICA [<threads>] [FOR CHANNEL '<ch>']` — the replica-thread lifecycle
//!   (`start_replica_stmt` / `stop_replica_stmt`).
//! - `START GROUP_REPLICATION [<option-list>]` / `STOP GROUP_REPLICATION` — the Group
//!   Replication plugin lifecycle (`group_replication`).
//!
//! MySQL 8.4 removed the legacy `MASTER`/`SLAVE` spellings (`CHANGE MASTER TO`, `START
//! SLAVE`, the `MASTER_*` option names), so only the `REPLICATION`/`REPLICA`/`SOURCE_*`
//! grammar is modelled here — each is an `ER_PARSE_ERROR` on mysql:8.4.10 and never parsed.
//! No operand is an expression, so the whole family is non-generic (no extension parameter).

use super::{AccountName, Ident, Literal, ObjectName};
use crate::vocab::Meta;
use thin_vec::ThinVec;

/// A MySQL replication-administration statement — the boxed payload of
/// [`Statement::Replication`](crate::ast::Statement::Replication).
///
/// Six verbs across the five measured families ride one enum because they are one dialect
/// unit reached through the replication-specific leading-keyword sequences. `START`/`STOP
/// GROUP_REPLICATION` are two variants (the two verbs carry different tails — only `START`
/// takes options) of the single `GROUP REPLICATION` family.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ReplicationStatement {
    /// `CHANGE REPLICATION SOURCE TO <option-list> [FOR CHANNEL '<ch>']`.
    ///
    /// The option list is non-empty (a bare `CHANGE REPLICATION SOURCE TO` is
    /// `ER_PARSE_ERROR` on mysql:8.4.10) and comma-separated; each element is a
    /// [`ChangeReplicationSourceOption`]. `FOR CHANNEL` is a trailing suffix, not a list
    /// member — an option after the channel (`… FOR CHANNEL 'c', SOURCE_PORT = 1`) rejects.
    ChangeSource {
        /// The `<name> = <value>` options in source order; always non-empty.
        options: ThinVec<ChangeReplicationSourceOption>,
        /// The optional `FOR CHANNEL '<name>'` replication-channel name.
        channel: Option<Literal>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `CHANGE REPLICATION FILTER <rule-list> [FOR CHANNEL '<ch>']`.
    ///
    /// The rule list is non-empty and comma-separated; each element is a
    /// [`ReplicationFilterRule`].
    ChangeFilter {
        /// The filter rules in source order; always non-empty.
        rules: ThinVec<ReplicationFilterRule>,
        /// The optional `FOR CHANNEL '<name>'` replication-channel name.
        channel: Option<Literal>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `START REPLICA [<thread-list>] [UNTIL <cond-list>] [USER='u'] [PASSWORD='p']
    /// [DEFAULT_AUTH='a'] [PLUGIN_DIR='d'] [FOR CHANNEL '<ch>']`.
    ///
    /// The connection options are four independent fixed-order optionals (MySQL's
    /// `opt_user_option` … `opt_plugin_dir_option`, space-separated — *not* a comma list,
    /// unlike [`StartGroupReplication`](Self::StartGroupReplication)). Each may appear alone
    /// (`START REPLICA PASSWORD = 'p'` is grammar-valid on mysql:8.4.10).
    StartReplica {
        /// The `SQL_THREAD`/`IO_THREAD` thread-type list (`opt_replica_thread_option_list`);
        /// empty when none was written (start both threads).
        threads: ThinVec<ReplicaThreadOption>,
        /// The `UNTIL <cond-list>` stop condition (`opt_replica_until`); empty when no
        /// `UNTIL` was written. See [`ReplicaUntilCondition`].
        until: ThinVec<ReplicaUntilCondition>,
        /// The `USER = '<u>'` connection user; `None` when absent.
        user: Option<Literal>,
        /// The `PASSWORD = '<p>'` connection password; `None` when absent.
        password: Option<Literal>,
        /// The `DEFAULT_AUTH = '<a>'` authentication plugin; `None` when absent.
        default_auth: Option<Literal>,
        /// The `PLUGIN_DIR = '<d>'` plugin directory; `None` when absent.
        plugin_dir: Option<Literal>,
        /// The optional `FOR CHANNEL '<name>'` replication-channel name.
        channel: Option<Literal>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `STOP REPLICA [<thread-list>] [FOR CHANNEL '<ch>']`.
    ///
    /// Unlike [`StartReplica`](Self::StartReplica), `STOP REPLICA` takes no `UNTIL` or
    /// connection tail — `STOP REPLICA UNTIL …` / `STOP REPLICA USER = …` is `ER_PARSE_ERROR`
    /// on mysql:8.4.10.
    StopReplica {
        /// The `SQL_THREAD`/`IO_THREAD` thread-type list; empty when none was written.
        threads: ThinVec<ReplicaThreadOption>,
        /// The optional `FOR CHANNEL '<name>'` replication-channel name.
        channel: Option<Literal>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `START GROUP_REPLICATION [<option-list>]`.
    ///
    /// The options are `USER`/`PASSWORD`/`DEFAULT_AUTH` in any order, *comma-separated*
    /// (`group_replication_start_options` — the distinguishing difference from
    /// [`StartReplica`](Self::StartReplica)'s space-separated fixed-order tail), so they ride
    /// an ordered list. Empty when none was written.
    StartGroupReplication {
        /// The connection options in source order; empty when none was written.
        options: ThinVec<GroupReplicationOption>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `STOP GROUP_REPLICATION` — takes no options (`STOP GROUP_REPLICATION USER = …` is
    /// `ER_PARSE_ERROR` on mysql:8.4.10).
    StopGroupReplication {
        /// Source location and node identity.
        meta: Meta,
    },
}

/// One `<name> = <value>` option of `CHANGE REPLICATION SOURCE TO` (`sql_yacc.yy`
/// `source_def` / `source_file_def`).
///
/// The [`name`](Self::name) is a closed, measured keyword set ([`SourceOption`]); the
/// [`value`](Self::value) carries the option's argument in one of the measured value shapes
/// ([`ChangeReplicationSourceOptionValue`]). The name→shape correspondence is fixed per
/// option and enforced by the parser (which reads the value shape the name dictates) rather
/// than the type — the [`CopyOption`](crate::ast::CopyOption) name/value idiom for a wide
/// flat option grammar.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct ChangeReplicationSourceOption {
    /// The option name; see [`SourceOption`].
    pub name: SourceOption,
    /// The option value; see [`ChangeReplicationSourceOptionValue`].
    pub value: ChangeReplicationSourceOptionValue,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The closed set of `CHANGE REPLICATION SOURCE TO` option names admitted by mysql:8.4.10.
///
/// A surface tag (no `meta`; the span rides the enclosing [`ChangeReplicationSourceOption`]).
/// The set is the 8.4 grammar's `source_def`/`source_file_def` alternatives, engine-narrowed:
/// the deprecated `MASTER_*` names were removed (each `ER_PARSE_ERROR`), and the compression
/// option is the *plural* `SOURCE_COMPRESSION_ALGORITHMS` — the singular
/// `SOURCE_COMPRESSION_ALGORITHM` (the yacc token's bare name) is `ER_PARSE_ERROR` on
/// mysql:8.4.10, so only the plural keyword is admitted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum SourceOption {
    /// `SOURCE_BIND` — the network interface to bind to.
    SourceBind,
    /// `SOURCE_HOST` — the source server host name.
    SourceHost,
    /// `SOURCE_USER` — the replication account user name.
    SourceUser,
    /// `SOURCE_PASSWORD` — the replication account password.
    SourcePassword,
    /// `SOURCE_PORT` — the source server TCP port.
    SourcePort,
    /// `SOURCE_CONNECT_RETRY` — seconds between reconnection attempts.
    SourceConnectRetry,
    /// `SOURCE_RETRY_COUNT` — the reconnection-attempt cap.
    SourceRetryCount,
    /// `SOURCE_DELAY` — the replication delay in seconds.
    SourceDelay,
    /// `SOURCE_HEARTBEAT_PERIOD` — the heartbeat interval (a fractional number of seconds).
    SourceHeartbeatPeriod,
    /// `SOURCE_LOG_FILE` — the source binary-log file name to read from.
    SourceLogFile,
    /// `SOURCE_LOG_POS` — the source binary-log position to read from.
    SourceLogPos,
    /// `SOURCE_AUTO_POSITION` — enable GTID auto-positioning (`0`/`1`).
    SourceAutoPosition,
    /// `RELAY_LOG_FILE` — the relay-log file name to resume from.
    RelayLogFile,
    /// `RELAY_LOG_POS` — the relay-log position to resume from.
    RelayLogPos,
    /// `SOURCE_SSL` — enable an encrypted connection (`0`/`1`).
    SourceSsl,
    /// `SOURCE_SSL_CA` — the certificate-authority file.
    SourceSslCa,
    /// `SOURCE_SSL_CAPATH` — the certificate-authority directory.
    SourceSslCapath,
    /// `SOURCE_SSL_CERT` — the client public-key certificate file.
    SourceSslCert,
    /// `SOURCE_SSL_CIPHER` — the permitted cipher list.
    SourceSslCipher,
    /// `SOURCE_SSL_KEY` — the client private-key file.
    SourceSslKey,
    /// `SOURCE_SSL_VERIFY_SERVER_CERT` — verify the source certificate (`0`/`1`).
    SourceSslVerifyServerCert,
    /// `SOURCE_SSL_CRL` — the certificate-revocation-list file.
    SourceSslCrl,
    /// `SOURCE_SSL_CRLPATH` — the certificate-revocation-list directory.
    SourceSslCrlpath,
    /// `SOURCE_TLS_VERSION` — the permitted TLS-protocol list.
    SourceTlsVersion,
    /// `SOURCE_TLS_CIPHERSUITES` — the permitted TLSv1.3 ciphersuite list (string, or `NULL`).
    SourceTlsCiphersuites,
    /// `SOURCE_PUBLIC_KEY_PATH` — the RSA public-key file for password exchange.
    SourcePublicKeyPath,
    /// `GET_SOURCE_PUBLIC_KEY` — request the RSA public key from the source (`0`/`1`).
    GetSourcePublicKey,
    /// `NETWORK_NAMESPACE` — the network namespace.
    NetworkNamespace,
    /// `IGNORE_SERVER_IDS` — the parenthesized server-id ignore list.
    IgnoreServerIds,
    /// `SOURCE_COMPRESSION_ALGORITHMS` — the permitted connection-compression algorithms.
    SourceCompressionAlgorithms,
    /// `SOURCE_ZSTD_COMPRESSION_LEVEL` — the zstd compression level.
    SourceZstdCompressionLevel,
    /// `PRIVILEGE_CHECKS_USER` — the account whose privileges gate applied transactions (an
    /// account, or `NULL`).
    PrivilegeChecksUser,
    /// `REQUIRE_ROW_FORMAT` — require row-based events (`0`/`1`).
    RequireRowFormat,
    /// `REQUIRE_TABLE_PRIMARY_KEY_CHECK` — the primary-key-check policy
    /// (`ON`/`OFF`/`STREAM`/`GENERATE`).
    RequireTablePrimaryKeyCheck,
    /// `SOURCE_CONNECTION_AUTO_FAILOVER` — enable asynchronous connection failover (`0`/`1`).
    SourceConnectionAutoFailover,
    /// `ASSIGN_GTIDS_TO_ANONYMOUS_TRANSACTIONS` — the anonymous-transaction GTID policy
    /// (`OFF`/`LOCAL`/`'<uuid>'`).
    AssignGtidsToAnonymousTransactions,
    /// `GTID_ONLY` — replicate using only GTIDs, storing no file/position (`0`/`1`).
    GtidOnly,
}

impl SourceOption {
    /// The exact keyword spelling this option renders as — the single source of truth
    /// shared by the renderer and the parser's option table.
    pub fn keyword(self) -> &'static str {
        match self {
            Self::SourceBind => "SOURCE_BIND",
            Self::SourceHost => "SOURCE_HOST",
            Self::SourceUser => "SOURCE_USER",
            Self::SourcePassword => "SOURCE_PASSWORD",
            Self::SourcePort => "SOURCE_PORT",
            Self::SourceConnectRetry => "SOURCE_CONNECT_RETRY",
            Self::SourceRetryCount => "SOURCE_RETRY_COUNT",
            Self::SourceDelay => "SOURCE_DELAY",
            Self::SourceHeartbeatPeriod => "SOURCE_HEARTBEAT_PERIOD",
            Self::SourceLogFile => "SOURCE_LOG_FILE",
            Self::SourceLogPos => "SOURCE_LOG_POS",
            Self::SourceAutoPosition => "SOURCE_AUTO_POSITION",
            Self::RelayLogFile => "RELAY_LOG_FILE",
            Self::RelayLogPos => "RELAY_LOG_POS",
            Self::SourceSsl => "SOURCE_SSL",
            Self::SourceSslCa => "SOURCE_SSL_CA",
            Self::SourceSslCapath => "SOURCE_SSL_CAPATH",
            Self::SourceSslCert => "SOURCE_SSL_CERT",
            Self::SourceSslCipher => "SOURCE_SSL_CIPHER",
            Self::SourceSslKey => "SOURCE_SSL_KEY",
            Self::SourceSslVerifyServerCert => "SOURCE_SSL_VERIFY_SERVER_CERT",
            Self::SourceSslCrl => "SOURCE_SSL_CRL",
            Self::SourceSslCrlpath => "SOURCE_SSL_CRLPATH",
            Self::SourceTlsVersion => "SOURCE_TLS_VERSION",
            Self::SourceTlsCiphersuites => "SOURCE_TLS_CIPHERSUITES",
            Self::SourcePublicKeyPath => "SOURCE_PUBLIC_KEY_PATH",
            Self::GetSourcePublicKey => "GET_SOURCE_PUBLIC_KEY",
            Self::NetworkNamespace => "NETWORK_NAMESPACE",
            Self::IgnoreServerIds => "IGNORE_SERVER_IDS",
            Self::SourceCompressionAlgorithms => "SOURCE_COMPRESSION_ALGORITHMS",
            Self::SourceZstdCompressionLevel => "SOURCE_ZSTD_COMPRESSION_LEVEL",
            Self::PrivilegeChecksUser => "PRIVILEGE_CHECKS_USER",
            Self::RequireRowFormat => "REQUIRE_ROW_FORMAT",
            Self::RequireTablePrimaryKeyCheck => "REQUIRE_TABLE_PRIMARY_KEY_CHECK",
            Self::SourceConnectionAutoFailover => "SOURCE_CONNECTION_AUTO_FAILOVER",
            Self::AssignGtidsToAnonymousTransactions => "ASSIGN_GTIDS_TO_ANONYMOUS_TRANSACTIONS",
            Self::GtidOnly => "GTID_ONLY",
        }
    }
}

/// The value of a [`ChangeReplicationSourceOption`] — the measured argument shapes of
/// mysql:8.4.10's `source_def` grammar.
///
/// Most options take a [`String`](Self::String) or [`Number`](Self::Number); the remaining
/// five carry the exotic shapes the grammar defines for specific options.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ChangeReplicationSourceOptionValue {
    /// A string-literal value (`SOURCE_HOST = 'h'`, `SOURCE_LOG_FILE = 'f'`). The spelling
    /// round-trips from the [`Literal`] span.
    String {
        /// The string value.
        value: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A numeric-literal value (`SOURCE_PORT = 3306`, `SOURCE_HEARTBEAT_PERIOD = 1.5`,
    /// the `0`/`1` boolean-ish flags). Integer and fractional spellings ride one shape;
    /// the [`Literal`] classifies its own kind and round-trips from its span.
    Number {
        /// The numeric value.
        value: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `SOURCE_TLS_CIPHERSUITES`'s `<string> | NULL` value: `Some` for a string,
    /// `None` for the bare `NULL` (`source_tls_ciphersuites_def`).
    NullableString {
        /// The string value; `None` for `NULL`.
        value: Option<Literal>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `PRIVILEGE_CHECKS_USER`'s `<user> | NULL` value: `Some` for a named account,
    /// `None` for the bare `NULL` (`privilege_check_def`).
    User {
        /// The account; `None` for `NULL`.
        account: Option<AccountName>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `IGNORE_SERVER_IDS`'s parenthesized server-id list (`ignore_server_id_list`). Empty
    /// for the `()` reset form.
    ServerIds {
        /// The server ids in source order; empty for `()`.
        ids: ThinVec<Literal>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `REQUIRE_TABLE_PRIMARY_KEY_CHECK`'s `ON | OFF | STREAM | GENERATE` keyword value
    /// (`table_primary_key_check_def`).
    PrimaryKeyCheck {
        /// The primary-key-check policy.
        check: RequirePrimaryKeyCheck,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `ASSIGN_GTIDS_TO_ANONYMOUS_TRANSACTIONS`'s `OFF | LOCAL | '<uuid>'` value
    /// (`assign_gtids_to_anonymous_transactions_def`). The `uuid` literal is present only
    /// when [`kind`](Self::AssignGtids::kind) is [`Uuid`](AssignGtidsKind::Uuid).
    AssignGtids {
        /// Which of the three forms was written.
        kind: AssignGtidsKind,
        /// The UUID string, present only for [`Uuid`](AssignGtidsKind::Uuid).
        uuid: Option<Literal>,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// The `REQUIRE_TABLE_PRIMARY_KEY_CHECK` policy keyword. A surface tag (no `meta`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum RequirePrimaryKeyCheck {
    /// `ON` — require a primary key on replicated tables.
    On,
    /// `OFF` — do not require a primary key.
    Off,
    /// `STREAM` — take the source's setting from the replication stream.
    Stream,
    /// `GENERATE` — generate a hidden primary key where the source table lacks one.
    Generate,
}

/// Which `ASSIGN_GTIDS_TO_ANONYMOUS_TRANSACTIONS` form was written. A surface tag (no
/// `meta`); the [`Uuid`](Self::Uuid) form's string rides
/// [`ChangeReplicationSourceOptionValue::AssignGtids::uuid`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum AssignGtidsKind {
    /// `OFF` — do not assign GTIDs to anonymous transactions.
    Off,
    /// `LOCAL` — assign GTIDs using the replica's own UUID.
    Local,
    /// `'<uuid>'` — assign GTIDs using the given UUID string.
    Uuid,
}

/// One rule of `CHANGE REPLICATION FILTER` (`sql_yacc.yy` `filter_def`).
///
/// Each rule's parenthesized argument list may be empty (the `()` reset form, engine-valid
/// on mysql:8.4.10). The table forms require *schema-qualified* names (`db.t` — a bare `t`
/// is `ER_PARSE_ERROR`), so they carry [`ObjectName`]s; the wild forms take string patterns;
/// `REPLICATE_REWRITE_DB` takes `(from, to)` database pairs.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ReplicationFilterRule {
    /// `REPLICATE_DO_DB = (<db>, …)` — replicate only the listed databases.
    DoDb {
        /// The database names; empty for the `()` reset form.
        databases: ThinVec<Ident>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `REPLICATE_IGNORE_DB = (<db>, …)` — ignore the listed databases.
    IgnoreDb {
        /// The database names; empty for the `()` reset form.
        databases: ThinVec<Ident>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `REPLICATE_DO_TABLE = (<db.t>, …)` — replicate only the listed (schema-qualified)
    /// tables.
    DoTable {
        /// The schema-qualified table names; empty for the `()` reset form.
        tables: ThinVec<ObjectName>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `REPLICATE_IGNORE_TABLE = (<db.t>, …)` — ignore the listed (schema-qualified) tables.
    IgnoreTable {
        /// The schema-qualified table names; empty for the `()` reset form.
        tables: ThinVec<ObjectName>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `REPLICATE_WILD_DO_TABLE = ('<pattern>', …)` — replicate tables matching the wildcard
    /// patterns.
    WildDoTable {
        /// The wildcard pattern strings; empty for the `()` reset form.
        patterns: ThinVec<Literal>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `REPLICATE_WILD_IGNORE_TABLE = ('<pattern>', …)` — ignore tables matching the wildcard
    /// patterns.
    WildIgnoreTable {
        /// The wildcard pattern strings; empty for the `()` reset form.
        patterns: ThinVec<Literal>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `REPLICATE_REWRITE_DB = ((<from>, <to>), …)` — rewrite database names. Each pair is
    /// doubly parenthesized (`filter_db_pair_list`); a single-paren `(a, b)` is
    /// `ER_PARSE_ERROR`.
    RewriteDb {
        /// The `(from, to)` rewrite pairs; empty for the `()` reset form.
        pairs: ThinVec<RewriteDbPair>,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// One `(<from>, <to>)` database-rewrite pair of a
/// [`ReplicationFilterRule::RewriteDb`] rule.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct RewriteDbPair {
    /// The source database name.
    pub from: Ident,
    /// The rewritten database name.
    pub to: Ident,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A replica-thread selector on `START`/`STOP REPLICA` (`sql_yacc.yy`
/// `replica_thread_option`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ReplicaThreadOption {
    /// `SQL_THREAD` — the applier thread.
    Sql {
        /// Source location and node identity.
        meta: Meta,
    },
    /// The I/O (receiver) thread. `IO_THREAD` and `RELAY_THREAD` are exact synonyms (both the
    /// yacc `RELAY_THREAD` token); an [`IoThreadKeyword`] surface tag records which was
    /// written so it round-trips.
    Io {
        /// Which of the `IO_THREAD` / `RELAY_THREAD` spellings was written.
        keyword: IoThreadKeyword,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// Which of the interchangeable `IO_THREAD` / `RELAY_THREAD` spellings names the receiver
/// thread. A surface tag (no `meta`); the two are exact synonyms.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum IoThreadKeyword {
    /// `IO_THREAD` — the documented spelling.
    Io,
    /// `RELAY_THREAD` — the synonym.
    Relay,
}

/// One condition of a `START REPLICA … UNTIL <cond-list>` clause (`sql_yacc.yy`
/// `replica_until`).
///
/// The grammar admits any single condition as the head of the list, but only the
/// file/position coordinates (`SOURCE_LOG_FILE`/`SOURCE_LOG_POS`/`RELAY_LOG_FILE`/
/// `RELAY_LOG_POS`, the `source_file_def` alternatives) may follow a comma — the parser
/// enforces that a GTID/gaps condition appears only as the first element. Which combinations
/// are *coherent* (a file needs its position, GTIDs and coordinates are mutually exclusive)
/// is a semantic check (`ER_BAD_REPLICA_UNTIL_COND` = 1277, grammar-positive), left to a
/// binding pass.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ReplicaUntilCondition {
    /// `SOURCE_LOG_FILE = '<f>'` — stop at the given source binary-log file.
    SourceLogFile {
        /// The source binary-log file name.
        value: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `SOURCE_LOG_POS = <n>` — stop at the given source binary-log position.
    SourceLogPos {
        /// The source binary-log position.
        value: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `RELAY_LOG_FILE = '<f>'` — stop at the given relay-log file.
    RelayLogFile {
        /// The relay-log file name.
        value: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `RELAY_LOG_POS = <n>` — stop at the given relay-log position.
    RelayLogPos {
        /// The relay-log position.
        value: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `SQL_BEFORE_GTIDS = '<gtid-set>'` — stop before applying the given GTIDs.
    SqlBeforeGtids {
        /// The GTID set.
        value: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `SQL_AFTER_GTIDS = '<gtid-set>'` — stop after applying the given GTIDs.
    SqlAfterGtids {
        /// The GTID set.
        value: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `SQL_AFTER_MTS_GAPS` — stop once the multi-threaded applier has filled its gaps.
    SqlAfterMtsGaps {
        /// Source location and node identity.
        meta: Meta,
    },
}

/// One option of `START GROUP_REPLICATION` (`sql_yacc.yy`
/// `group_replication_start_option`). The options are comma-separated and order-preserving.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum GroupReplicationOption {
    /// `USER = '<u>'` — the distributed-recovery account user.
    User {
        /// The user name.
        value: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `PASSWORD = '<p>'` — the distributed-recovery account password.
    Password {
        /// The password.
        value: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `DEFAULT_AUTH = '<a>'` — the distributed-recovery authentication plugin.
    DefaultAuth {
        /// The authentication plugin name.
        value: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
}
