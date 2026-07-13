// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

import type * as Ast from "../js/ast.generated.js";
declare const DIALECT_ALIASES: {
    readonly ansi: readonly ["ansi", "generic"];
    readonly postgres: readonly ["postgres", "postgresql", "pg"];
    readonly mysql: readonly ["mysql", "mariadb"];
    readonly sqlite: readonly ["sqlite", "sqlite3"];
    readonly duckdb: readonly ["duckdb", "duck"];
    readonly bigquery: readonly ["bigquery", "bq", "zetasql"];
    readonly hive: readonly ["hive", "hiveql"];
    readonly clickhouse: readonly ["clickhouse", "ch"];
    readonly databricks: readonly ["databricks", "dbx"];
    readonly mssql: readonly ["mssql", "tsql", "sqlserver"];
    readonly snowflake: readonly ["snowflake", "sf"];
    readonly redshift: readonly ["redshift", "amazonredshift"];
    readonly lenient: readonly ["lenient", "permissive"];
};
declare const DIALECT_BRAND: unique symbol;
/** Canonical lower-case dialect names returned by parse and tokenize results. */
export type CanonicalDialectName = keyof typeof DIALECT_ALIASES;
/** All case-insensitive dialect spellings accepted by the Rust binding layer. */
export type DialectAlias = (typeof DIALECT_ALIASES)[CanonicalDialectName][number];
type DialectCanonicalMap = {
    [TCanonical in CanonicalDialectName as (typeof DIALECT_ALIASES)[TCanonical][number]]: TCanonical;
};
/** Canonical result dialect for a dialect literal or validated dynamic dialect. */
export type CanonicalDialect<TDialect> = TDialect extends ValidatedDialectName<infer TCanonical> ? TCanonical : TDialect extends DialectAlias ? DialectCanonicalMap[TDialect] : never;
/** Accepted alias literals for a canonical dialect or dialect union. */
export type DialectAliasesFor<TSupported extends CanonicalDialectName> = {
    [TAlias in DialectAlias]: DialectCanonicalMap[TAlias] extends TSupported ? TAlias : never;
}[DialectAlias];
/**
 * Runtime-validated dynamic dialect string.
 *
 * Plain `string` is intentionally not assignable to parse/render options. Use
 * `isDialectName`, `assertDialectName`, or `canonicalDialectName` when the value
 * comes from UI, config, or another dynamic source.
 */
export type ValidatedDialectName<TSupported extends CanonicalDialectName = CanonicalDialectName> = string & {
    readonly [DIALECT_BRAND]: TSupported;
};
/** Dialect value accepted by a package entrypoint for its compiled dialect set. */
export type DialectName<TSupported extends CanonicalDialectName = CanonicalDialectName> = TSupported | DialectAliasesFor<TSupported> | ValidatedDialectName<TSupported>;
type DefaultDialect<TSupported extends CanonicalDialectName, TDefault extends TSupported> = TDefault;
/** SQL rendering mode. */
export type RenderMode = "canonical" | "redacted" | "parenthesized" | "parenthesised";
/** Input accepted by the wasm-bindgen initializer. */
export type InitInput = string | URL | Request | Response | BufferSource | WebAssembly.Module;
/** Runtime class constructor accepted by `findAll`. */
export type NodeType<TNode extends Node = Node> = {
    readonly prototype: TNode;
};
/** Recursive JSON AST scalar/object value emitted by the parser. */
export type AstValue = AstObject | Ast.ObjectName | Ast.Ident | Ast.Span | Ast.Meta | string | number | boolean | null | AstValueArray;
/** Array member of {@link AstValue}, deferred via an interface (see note). */
export interface AstValueArray extends Array<AstValue> {
}
/** JSON object inside an AST payload. */
export interface AstObject {
    [field: string]: AstValue;
}
/** Value returned by `Node.get`, wrapping known AST object shapes in helper views. */
export type WrappedAstValue = AstValue | Node | Ident | ObjectName | undefined | WrappedAstValueArray;
/** Array member of {@link WrappedAstValue}, deferred via an interface (see note). */
export interface WrappedAstValueArray extends Array<WrappedAstValue> {
}
/** Source location derived from a byte offset. Lines and columns are zero-based. */
export interface SourceLocation {
    line: number;
    byteColumn: number;
    charColumn: number;
    utf16Column: number;
}
/** Options for fail-fast and recovering parse calls. */
export interface ParseConfig<TSupported extends CanonicalDialectName = CanonicalDialectName, TDialect extends DialectName<TSupported> = DialectName<TSupported>> {
    /** Dialect name or alias. Defaults to the active package's dialect. */
    dialect?: TDialect;
    /** Optional parser recursion-depth limit for untrusted input. */
    recursionLimit?: number;
    /** Include whitespace and comments in `Document.trivia` when true. */
    captureTrivia?: boolean;
    /** Parse floating-point literals as exact decimal values when true. */
    parseFloatAsDecimal?: boolean;
}
/** Options for rendering a SQL string or parsed document. */
export interface RenderOptions<TSupported extends CanonicalDialectName = CanonicalDialectName, TDialect extends DialectName<TSupported> = DialectName<TSupported>> {
    /** Target dialect. Defaults to the document dialect, or the package default for strings. */
    dialect?: TDialect;
    /** Renderer mode. Defaults to `"canonical"`. */
    mode?: RenderMode;
}
/** Options for pretty-print formatting. */
export interface FormatOptions<TSupported extends CanonicalDialectName = CanonicalDialectName, TDialect extends DialectName<TSupported> = DialectName<TSupported>> {
    /** Dialect used for parsing and formatting. Defaults to the package dialect. */
    dialect?: TDialect;
    /** Spaces per indentation level. Defaults to 2. */
    indentWidth?: number;
    /** Preferred maximum line width. Defaults to 80. */
    maxWidth?: number;
    /** Keyword casing in the formatted output. Defaults to `"upper"`. */
    keywordCase?: "upper" | "lower" | "preserve";
}
/** Options for parse-then-render transpilation. */
export interface TranspileOptions<TSupported extends CanonicalDialectName = CanonicalDialectName, TSourceDialect extends DialectName<TSupported> = DialectName<TSupported>, TTargetDialect extends DialectName<TSupported> = DialectName<TSupported>> {
    /** Source dialect used for parsing. Defaults to the package dialect. */
    sourceDialect?: TSourceDialect;
    /** Target dialect used for rendering. Defaults to the package dialect. */
    targetDialect?: TTargetDialect;
}
/** Dialect-controlled string literal feature metadata captured with a parse result. */
export interface StringLiteralSyntax {
    escape_strings: boolean;
    dollar_quoted_strings: boolean;
    national_strings: boolean;
    double_quoted_strings: boolean;
    backslash_escapes: boolean;
    unicode_strings: boolean;
    bit_string_literals: boolean;
    charset_introducers: boolean;
    same_line_adjacent_concat: boolean;
}
/** Metadata needed to resolve serialized AST symbol ids. */
export interface ResolverMetadata {
    dynamic_base: number;
    keyword_symbols: KeywordSymbol[];
}
/** Fixed keyword-backed symbol entry. */
export interface KeywordSymbol {
    symbol: number;
    text: string;
}
/** Known diagnostic categories emitted by the wasm binding layer. */
export type DiagnosticKind = "syntax" | "recursion_limit_exceeded" | "unknown_dialect" | "unknown_render_mode" | "unknown_keyword_case" | "lex" | "render" | "deserialize" | "serialization" | "binding" | (string & {});
/** Serializable diagnostic object thrown by fail-fast APIs or returned by recovery. */
export interface DiagnosticJson {
    message: string;
    kind: DiagnosticKind;
    span: Ast.Span | null;
    span_start?: number;
    span_end?: number;
    expected?: string;
    found?: string;
}
/** Raw JSON parse result emitted by `parseJson`. */
export interface ParseResult<TDialect extends CanonicalDialectName = CanonicalDialectName> {
    dialect: TDialect;
    source: string;
    symbols: string[];
    trivia?: Trivia[];
    resolver: ResolverMetadata;
    string_literals: StringLiteralSyntax;
    statements: Ast.Statement[];
}
/** Raw recovering parse result emitted by `parseRecoveringJson`. */
export interface RecoveringParseResult<TDialect extends CanonicalDialectName = CanonicalDialectName> extends ParseResult<TDialect> {
    errors: DiagnosticJson[];
}
/** Supported dialect metadata for the active package entrypoint. */
export interface DialectInfo<TDialect extends CanonicalDialectName = CanonicalDialectName> {
    name: TDialect;
    aliases: DialectAliasesFor<TDialect>[];
}
/** Operator token variant. */
export type OperatorKind = "Plus" | "Minus" | "Star" | "Slash" | "SlashSlash" | "Percent" | "Eq" | "EqEq" | "Lt" | "LtEq" | "Gt" | "GtEq" | "NotEq" | "LtEqGt" | "Concat" | "AmpAmp" | "Bang" | "Pipe" | "Amp" | "Caret" | "Tilde" | "ShiftLeft" | "ShiftRight" | "Hash" | "Arrow" | "ColonEquals" | "AtGt" | "LtAt" | "MinusGt" | "MinusGtGt";
/** Punctuation token variant. */
export type PunctuationKind = "LParen" | "RParen" | "Comma" | "Semicolon" | "Dot" | "LBracket" | "RBracket" | "LBrace" | "RBrace" | "Colon" | "DoubleColon";
/** Discriminated lexical token category. */
export type TokenKind = {
    kind: "Word";
} | {
    kind: "Keyword";
    keyword: string;
} | {
    kind: "Number";
} | {
    kind: "String";
} | {
    kind: "QuotedIdent";
} | {
    kind: "Parameter";
} | {
    kind: "PositionalColumn";
} | {
    kind: "Variable";
} | {
    kind: "Operator";
    operator: OperatorKind;
} | {
    kind: "Punctuation";
    punctuation: PunctuationKind;
} | {
    kind: "Unknown";
};
/** Captured trivia category. */
export type TriviaKind = "LineComment" | "BlockComment" | "Whitespace";
/** Non-trivia token with exact source span and text. */
export type Token = TokenKind & {
    span: Ast.Span;
    text: string;
};
/** Whitespace or comment trivia with exact source span and text. */
export interface Trivia {
    kind: TriviaKind;
    span: Ast.Span;
    text: string;
}
/** Tokenizer output for the active dialect. */
export interface TokenizeResult<TDialect extends CanonicalDialectName = CanonicalDialectName> {
    source: string;
    dialect: TDialect;
    tokens: Token[];
    trivia?: Trivia[];
}
/** Native Node-API or wasm-bindgen document handle consumed by the typed facade. */
export interface NativeDocumentHandle {
    readonly source: string;
    readonly dialect: string;
    to_value(): unknown;
    render(dialect: string, mode: string): string;
    render_fragment(nodeId: number, dialect: string, mode: string): string;
}
/** Backend-neutral low-level binding contract consumed by the typed facade. */
export interface WasmBindings {
    parse_document_with(sql: string, dialect: string, recursionLimit: number | null | undefined, captureTrivia: boolean, parseFloatAsDecimal: boolean): NativeDocumentHandle;
    parse_recovering_document_with(sql: string, dialect: string, recursionLimit: number | null | undefined, captureTrivia: boolean, parseFloatAsDecimal: boolean): NativeDocumentHandle;
    parse_with(sql: string, dialect: string, recursionLimit: number | null | undefined, captureTrivia: boolean, parseFloatAsDecimal: boolean): unknown;
    parse_recovering_with(sql: string, dialect: string, recursionLimit: number | null | undefined, captureTrivia: boolean, parseFloatAsDecimal: boolean): unknown;
    render_sql(sql: string, dialect: string, mode: string): string;
    render_document?(document: unknown, dialect: string, mode: string): string;
    render_fragment?(document: unknown, nodeId: number, dialect: string, mode: string): string;
    format?(sql: string, dialect: string, indentWidth: number, maxWidth: number, keywordCase: string): string;
    supported_dialects(): unknown;
    tokenize(sql: string, dialect: string, includeTrivia: boolean): unknown;
    transpile(sql: string, sourceDialect: string, targetDialect: string): string;
    version(): string;
    schema_version(): number;
}
type WasmInit<TInitOutput> = (input?: {
    module_or_path: InitInput | Promise<InitInput>;
} | InitInput | Promise<InitInput>) => Promise<TInitOutput>;
/** Options for loading a browser package. */
export interface CreateSquonkOptions {
    /** Custom wasm source. Defaults to the package's colocated `.wasm` file. */
    wasm?: InitInput | Promise<InitInput>;
}
declare const WRAPPER_TOKEN: unique symbol;
/**
 * Structured parser error thrown by fail-fast APIs.
 *
 * Recovering parse APIs return SQL syntax diagnostics as data, but still throw
 * this error for binding-boundary failures such as unknown dialect names.
 */
export declare class SqlParseError extends Error {
    readonly kind: DiagnosticKind;
    readonly span: Ast.Span | null;
    readonly expected: string | null;
    readonly found: string | null;
    constructor(diagnostic: DiagnosticJson);
}
/** Parsed SQL document with convenience methods for traversal and rendering. */
export declare class Document<TParse extends ParseResult = ParseResult, TDialect extends CanonicalDialectName = TParse["dialect"], TSupported extends CanonicalDialectName = CanonicalDialectName> {
    #private;
    constructor(token: typeof WRAPPER_TOKEN, raw: TParse | null, native?: NativeDocumentHandle | null, source?: string, dialect?: TDialect);
    /** Raw JSON parse payload, materialized on first access. */
    get raw(): TParse;
    /** Original SQL source. */
    get source(): string;
    /** Canonical dialect used to parse this document. */
    get dialect(): TDialect;
    /** Top-level statements wrapped as traversal nodes. */
    get statements(): Array<Node<Ast.Statement>>;
    /** Recovering diagnostics. Empty for fail-fast parse documents. */
    get errors(): Diagnostic[];
    /** Captured whitespace/comment trivia, when `captureTrivia` was enabled. */
    get trivia(): Trivia[];
    /** Resolve a serialized AST symbol id to source or keyword text. */
    resolveSymbol(symbol: number): string;
    /** Return exact source text for a byte span. */
    sourceText(span: Ast.Span): string;
    /** Convert a byte offset into line, byte-column, Unicode scalar, and UTF-16 columns. */
    location(offset: number): SourceLocation;
    /** Walk every AST node in document order. */
    walk(): IterableIterator<Node>;
    /** Find wrapped nodes by runtime kind string. */
    findAll(kind: string): IterableIterator<Node>;
    /** Find wrapped nodes by helper class, for example `findAll(Ident)`. */
    findAll<TNode extends Node>(kind: NodeType<TNode>): IterableIterator<TNode>;
    /** Return the raw JSON parse payload. */
    toJSON(): TParse;
    /** Render this document as SQL. */
    toSQL<const TTargetDialect extends DialectName<TSupported> = DialectName<TSupported>>(options?: RenderOptions<TSupported, TTargetDialect>): string;
}
/** Recovering parse document with syntax diagnostics attached as data. */
export declare class RecoveredDocument<TParse extends RecoveringParseResult = RecoveringParseResult, TDialect extends CanonicalDialectName = TParse["dialect"], TSupported extends CanonicalDialectName = CanonicalDialectName> extends Document<TParse, TDialect, TSupported> {
    /** Render the recovered partial document. */
    toSQL<const TTargetDialect extends DialectName<TSupported> = DialectName<TSupported>>(options?: RenderOptions<TSupported, TTargetDialect>): string;
}
/** Wrapped AST node with source spans, traversal, and field access helpers. */
export declare class Node<TRaw extends Ast.AstNode = Ast.AstNode> {
    readonly raw: TRaw;
    readonly document: Document;
    readonly typeName: string | null;
    kind: string;
    data: AstObject | AstValue;
    constructor(token: typeof WRAPPER_TOKEN, raw: TRaw, document: Document, typeName?: string | null);
    /** Source byte span for this node, when the AST payload carries metadata. */
    get span(): Ast.Span | null;
    /** Stable parser-allocated node id, when present. */
    get nodeId(): number | null;
    /** Whether this node can render without surrounding SQL context. */
    get isRenderable(): boolean;
    /** Read a field from the node payload, wrapping known AST object shapes. */
    get(field: string): WrappedAstValue;
    /** Exact SQL source text for this node span, or null for synthetic nodes. */
    sourceText(): string | null;
    /** Source location at this node's start offset, or null for synthetic nodes. */
    location(): SourceLocation | null;
    /** Direct AST children wrapped as nodes. */
    children(): Node[];
    /** Walk this node and all descendants in document order. */
    walk(): IterableIterator<Node>;
    /** Find descendant nodes by runtime kind string. */
    findAll(kind: string): IterableIterator<Node>;
    /** Find descendant nodes by helper class, for example `findAll(Ident)`. */
    findAll<TNode extends Node>(kind: NodeType<TNode>): IterableIterator<TNode>;
    /** Return the raw JSON node payload. */
    toJSON(): TRaw;
    /** Render this standalone node as a SQL fragment. */
    toSQL<const TTargetDialect extends DialectName = DialectName>(options?: RenderOptions<CanonicalDialectName, TTargetDialect>): string;
}
/** Wrapped identifier with resolved source text. */
export declare class Ident extends Node<Ast.Ident> {
    constructor(token: typeof WRAPPER_TOKEN, raw: Ast.Ident, document: Document);
    /** Serialized symbol id. */
    get symbol(): number;
    /** Identifier text resolved through the document symbol table. */
    get text(): string;
    /** Quote style recorded by the parser. */
    get quote(): Ast.QuoteStyle;
}
/** Dotted object name helper that wraps each identifier part. */
export declare class ObjectName {
    readonly raw: Ast.ObjectName;
    readonly document: Document;
    readonly parts: Ident[];
    constructor(token: typeof WRAPPER_TOKEN, raw: Ast.ObjectName, document: Document);
    /** Object name joined with dots. */
    get text(): string;
    /** Iterate identifier parts in source order. */
    [Symbol.iterator](): IterableIterator<Ident>;
    /** Return the raw JSON object-name payload. */
    toJSON(): Ast.ObjectName;
}
/** Diagnostic wrapper with source helpers. */
export declare class Diagnostic {
    readonly raw: DiagnosticJson;
    readonly document: Document;
    constructor(token: typeof WRAPPER_TOKEN, raw: DiagnosticJson, document: Document);
    /** Human-readable diagnostic message. */
    get message(): string;
    /** Stable diagnostic category. */
    get kind(): DiagnosticKind;
    /** Diagnostic byte span, or null for synthetic/no-source diagnostics. */
    get span(): Ast.Span | null;
    /** Exact source text covered by this diagnostic, when it has a span. */
    sourceText(): string | null;
    /** Source location at the diagnostic start offset, when it has a span. */
    location(): SourceLocation | null;
}
/** Typed API exported by each package entrypoint. */
export interface SquonkApi<TSupported extends CanonicalDialectName, TDefault extends TSupported> {
    /** Error class thrown by fail-fast APIs. */
    readonly SqlParseError: typeof SqlParseError;
    /** Document class bound to this entrypoint's wasm runtime. */
    readonly Document: typeof Document;
    /** Recovering document class bound to this entrypoint's wasm runtime. */
    readonly RecoveredDocument: typeof RecoveredDocument;
    /** AST node wrapper class. */
    readonly Node: typeof Node;
    /** Identifier wrapper class. */
    readonly Ident: typeof Ident;
    /** Object-name wrapper class. */
    readonly ObjectName: typeof ObjectName;
    /** Diagnostic wrapper class. */
    readonly Diagnostic: typeof Diagnostic;
    /** Validate a dynamic dialect string for this entrypoint. */
    isDialectName(value: string): value is ValidatedDialectName<TSupported>;
    /** Assert that a dynamic dialect string is valid for this entrypoint. */
    assertDialectName(value: string): asserts value is ValidatedDialectName<TSupported>;
    /** Return the canonical dialect for a dynamic string, or null if unsupported. */
    canonicalDialectName(value: string): TSupported | null;
    /** Fail-fast parse into a wrapped document. */
    parse<const TDialect extends DialectName<TSupported> = DefaultDialect<TSupported, TDefault>>(sql: string, options?: ParseConfig<TSupported, TDialect>): Document<ParseResult<CanonicalDialect<TDialect>>, CanonicalDialect<TDialect>, TSupported>;
    /** Fail-fast parse into the raw JSON payload. */
    parseJson<const TDialect extends DialectName<TSupported> = DefaultDialect<TSupported, TDefault>>(sql: string, options?: ParseConfig<TSupported, TDialect>): ParseResult<CanonicalDialect<TDialect>>;
    /** Parse with an explicit recursion-depth limit. */
    parseWithLimit<const TDialect extends DialectName<TSupported>>(sql: string, dialect: TDialect, limit: number): Document<ParseResult<CanonicalDialect<TDialect>>, CanonicalDialect<TDialect>, TSupported>;
    /** Recovering parse into a wrapped document with diagnostics. */
    parseRecovering<const TDialect extends DialectName<TSupported> = DefaultDialect<TSupported, TDefault>>(sql: string, options?: ParseConfig<TSupported, TDialect>): RecoveredDocument<RecoveringParseResult<CanonicalDialect<TDialect>>, CanonicalDialect<TDialect>, TSupported>;
    /** Recovering parse into the raw JSON payload with diagnostics. */
    parseRecoveringJson<const TDialect extends DialectName<TSupported> = DefaultDialect<TSupported, TDefault>>(sql: string, options?: ParseConfig<TSupported, TDialect>): RecoveringParseResult<CanonicalDialect<TDialect>>;
    /** Dialects compiled into the active wasm artifact. */
    supportedDialects(): DialectInfo<TSupported>[];
    /** Tokenize SQL under a supported dialect. */
    tokenize<const TDialect extends DialectName<TSupported> = DefaultDialect<TSupported, TDefault>>(sql: string, options?: {
        dialect?: TDialect;
        includeTrivia?: boolean;
    }): TokenizeResult<CanonicalDialect<TDialect>>;
    /** Render a SQL string or parsed document. */
    render<const TTargetDialect extends DialectName<TSupported> = DialectName<TSupported>>(sqlOrDocument: string | Document<ParseResult, CanonicalDialectName, TSupported>, options?: RenderOptions<TSupported, TTargetDialect>): string;
    /** Render using redaction mode. */
    redact<const TTargetDialect extends DialectName<TSupported> = DialectName<TSupported>>(sqlOrDocument: string | Document<ParseResult, CanonicalDialectName, TSupported>, options?: Omit<RenderOptions<TSupported, TTargetDialect>, "mode">): string;
    /** Pretty-print SQL. */
    format<const TDialect extends DialectName<TSupported> = DefaultDialect<TSupported, TDefault>>(sql: string, options?: FormatOptions<TSupported, TDialect>): string;
    /** Parse under one dialect and render under another. */
    transpile<const TSourceDialect extends DialectName<TSupported> = DefaultDialect<TSupported, TDefault>, const TTargetDialect extends DialectName<TSupported> = DefaultDialect<TSupported, TDefault>>(sql: string, options?: TranspileOptions<TSupported, TSourceDialect, TTargetDialect>): string;
    /** Crate/package version string. */
    version(): string;
    /** Serialized AST wire-schema version. */
    schemaVersion(): number;
    /** Describe the engine selected for this package instance. */
    runtimeInfo(): RuntimeInfo;
}
/** Runtime engine selected for a Squonk package instance. */
export interface RuntimeInfo {
    /** Execution backend. Native is Node-API; wasm is WebAssembly. */
    readonly backend: "native" | "wasm";
    /** Host family used to load the backend. */
    readonly host: "node" | "bun" | "deno" | "browser" | "workerd" | "edge-light" | "unknown";
}
/** Construct a typed facade over a native Node-API or wasm-bindgen backend. */
export declare function createSquonkApi<const TSupportedDialects extends readonly CanonicalDialectName[], const TDefaultDialect extends TSupportedDialects[number]>(wasm: WasmBindings, options: {
    readonly defaultDialect: TDefaultDialect;
    readonly supportedDialects: TSupportedDialects;
    readonly runtime?: RuntimeInfo;
}): SquonkApi<TSupportedDialects[number], TDefaultDialect>;
/** Build the one-shot asynchronous loader exposed by each browser entrypoint. */
export declare function createBrowserSquonk<const TSupportedDialects extends readonly CanonicalDialectName[], const TDefaultDialect extends TSupportedDialects[number], TInitOutput>(initWasm: WasmInit<TInitOutput>, wasm: WasmBindings, defaultWasmUrl: URL, options: {
    readonly defaultDialect: TDefaultDialect;
    readonly supportedDialects: TSupportedDialects;
}): (createOptions?: CreateSquonkOptions) => Promise<SquonkApi<TSupportedDialects[number], TDefaultDialect>>;
export {};
