// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

import type * as Ast from "../js/ast.generated.js";
import { AST_FIELD_TYPES as RAW_AST_FIELD_TYPES } from "../js/ast-metadata.generated.js";

const AST_FIELD_TYPES = RAW_AST_FIELD_TYPES as Readonly<
  Record<string, Readonly<Record<string, string>>>
>;

const DIALECT_ALIASES = {
  ansi: ["ansi", "generic"],
  postgres: ["postgres", "postgresql", "pg"],
  mysql: ["mysql", "mariadb"],
  sqlite: ["sqlite", "sqlite3"],
  duckdb: ["duckdb", "duck"],
  bigquery: ["bigquery", "bq", "zetasql"],
  hive: ["hive", "hiveql"],
  clickhouse: ["clickhouse", "ch"],
  databricks: ["databricks", "dbx"],
  mssql: ["mssql", "tsql", "sqlserver"],
  snowflake: ["snowflake", "sf"],
  redshift: ["redshift", "amazonredshift"],
  lenient: ["lenient", "permissive"],
} as const;

declare const DIALECT_BRAND: unique symbol;

/** Canonical lower-case dialect names returned by parse and tokenize results. */
export type CanonicalDialectName = keyof typeof DIALECT_ALIASES;

/** All case-insensitive dialect spellings accepted by the Rust binding layer. */
export type DialectAlias = (typeof DIALECT_ALIASES)[CanonicalDialectName][number];

type DialectCanonicalMap = {
  [TCanonical in CanonicalDialectName as (typeof DIALECT_ALIASES)[TCanonical][number]]: TCanonical;
};

/** Canonical result dialect for a dialect literal or validated dynamic dialect. */
export type CanonicalDialect<TDialect> =
  TDialect extends ValidatedDialectName<infer TCanonical>
    ? TCanonical
    : TDialect extends DialectAlias
      ? DialectCanonicalMap[TDialect]
      : never;

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
export type ValidatedDialectName<
  TSupported extends CanonicalDialectName = CanonicalDialectName,
> = string & {
  readonly [DIALECT_BRAND]: TSupported;
};

/** Dialect value accepted by a package entrypoint for its compiled dialect set. */
export type DialectName<TSupported extends CanonicalDialectName = CanonicalDialectName> =
  | TSupported
  | DialectAliasesFor<TSupported>
  | ValidatedDialectName<TSupported>;

type DefaultDialect<
  TSupported extends CanonicalDialectName,
  TDefault extends TSupported,
> = TDefault;

/** SQL rendering mode. */
export type RenderMode =
  | "canonical"
  | "redacted"
  | "parenthesized"
  | "parenthesised";

/** Input accepted by the wasm-bindgen initializer. */
export type InitInput =
  | string
  | URL
  | Request
  | Response
  | BufferSource
  | WebAssembly.Module;

/** Runtime class constructor accepted by `findAll`. */
export type NodeType<TNode extends Node = Node> = {
  readonly prototype: TNode;
};

// `AstValue` is the loose recursive value type for untyped traversal (`Node.get`,
// `wrap`, `.data`). It deliberately does NOT inline the generated `Ast.AstNode`
// union: that union has ~630 members, and embedding it in a recursive type forces
// TypeScript to fully expand `AstValue`/`WrappedAstValue` past its union-complexity
// limit (TS2590) at every use site — notably the recursive `.map` in `wrap`. Every
// AST node is structurally an object, so `AstObject` (the index-signature member)
// already covers node values here; the discriminated node typing that consumers
// rely on lives on the typed surfaces (`ParseResult.statements: Ast.Statement[]`,
// `Node<TRaw extends Ast.AstNode>`), not on this traversal type.
/** Recursive JSON AST scalar/object value emitted by the parser. */
export type AstValue =
  | AstObject
  | Ast.ObjectName
  | Ast.Ident
  | Ast.Span
  | Ast.Meta
  | string
  | number
  | boolean
  | null
  | AstValueArray;

// Array members are interfaces, not inline `T[]` in the aliases: an interface
// extending `Array<…>` defers evaluation, keeping the recursive types within the
// compiler's complexity limit.
/** Array member of {@link AstValue}, deferred via an interface (see note). */
export interface AstValueArray extends Array<AstValue> {}

/** JSON object inside an AST payload. */
export interface AstObject {
  [field: string]: AstValue;
}

/** Value returned by `Node.get`, wrapping known AST object shapes in helper views. */
export type WrappedAstValue =
  | AstValue
  | Node
  | Ident
  | ObjectName
  | undefined
  | WrappedAstValueArray;

/** Array member of {@link WrappedAstValue}, deferred via an interface (see note). */
export interface WrappedAstValueArray extends Array<WrappedAstValue> {}

/** Source location derived from a byte offset. Lines and columns are zero-based. */
export interface SourceLocation {
  line: number;
  byteColumn: number;
  charColumn: number;
  utf16Column: number;
}

/** Options for fail-fast and recovering parse calls. */
export interface ParseConfig<
  TSupported extends CanonicalDialectName = CanonicalDialectName,
  TDialect extends DialectName<TSupported> = DialectName<TSupported>,
> {
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
export interface RenderOptions<
  TSupported extends CanonicalDialectName = CanonicalDialectName,
  TDialect extends DialectName<TSupported> = DialectName<TSupported>,
> {
  /** Target dialect. Defaults to the document dialect, or the package default for strings. */
  dialect?: TDialect;
  /** Renderer mode. Defaults to `"canonical"`. */
  mode?: RenderMode;
}

/** Options for pretty-print formatting. */
export interface FormatOptions<
  TSupported extends CanonicalDialectName = CanonicalDialectName,
  TDialect extends DialectName<TSupported> = DialectName<TSupported>,
> {
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
export interface TranspileOptions<
  TSupported extends CanonicalDialectName = CanonicalDialectName,
  TSourceDialect extends DialectName<TSupported> = DialectName<TSupported>,
  TTargetDialect extends DialectName<TSupported> = DialectName<TSupported>,
> {
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
export type DiagnosticKind =
  | "syntax"
  | "recursion_limit_exceeded"
  | "unknown_dialect"
  | "unknown_render_mode"
  | "unknown_keyword_case"
  | "lex"
  | "render"
  | "deserialize"
  | "serialization"
  | "binding"
  | (string & {});

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
export interface RecoveringParseResult<
  TDialect extends CanonicalDialectName = CanonicalDialectName,
> extends ParseResult<TDialect> {
  errors: DiagnosticJson[];
}

/** Supported dialect metadata for the active package entrypoint. */
export interface DialectInfo<TDialect extends CanonicalDialectName = CanonicalDialectName> {
  name: TDialect;
  aliases: DialectAliasesFor<TDialect>[];
}

/** Operator token variant. */
export type OperatorKind =
  | "Plus"
  | "Minus"
  | "Star"
  | "Slash"
  | "SlashSlash"
  | "Percent"
  | "Eq"
  | "EqEq"
  | "Lt"
  | "LtEq"
  | "Gt"
  | "GtEq"
  | "NotEq"
  | "LtEqGt"
  | "Concat"
  | "AmpAmp"
  | "Bang"
  | "Pipe"
  | "Amp"
  | "Caret"
  | "Tilde"
  | "ShiftLeft"
  | "ShiftRight"
  | "Hash"
  | "Arrow"
  | "ColonEquals"
  | "AtGt"
  | "LtAt"
  | "MinusGt"
  | "MinusGtGt";

/** Punctuation token variant. */
export type PunctuationKind =
  | "LParen"
  | "RParen"
  | "Comma"
  | "Semicolon"
  | "Dot"
  | "LBracket"
  | "RBracket"
  | "LBrace"
  | "RBrace"
  | "Colon"
  | "DoubleColon";

/** Discriminated lexical token category. */
export type TokenKind =
  | { kind: "Word" }
  | { kind: "Keyword"; keyword: string }
  | { kind: "Number" }
  | { kind: "String" }
  | { kind: "QuotedIdent" }
  | { kind: "Parameter" }
  | { kind: "PositionalColumn" }
  | { kind: "Variable" }
  | { kind: "Operator"; operator: OperatorKind }
  | { kind: "Punctuation"; punctuation: PunctuationKind }
  | { kind: "Unknown" };

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
  parse_document_with(
    sql: string,
    dialect: string,
    recursionLimit: number | null | undefined,
    captureTrivia: boolean,
    parseFloatAsDecimal: boolean,
  ): NativeDocumentHandle;
  parse_recovering_document_with(
    sql: string,
    dialect: string,
    recursionLimit: number | null | undefined,
    captureTrivia: boolean,
    parseFloatAsDecimal: boolean,
  ): NativeDocumentHandle;
  parse_with(
    sql: string,
    dialect: string,
    recursionLimit: number | null | undefined,
    captureTrivia: boolean,
    parseFloatAsDecimal: boolean,
  ): unknown;
  parse_recovering_with(
    sql: string,
    dialect: string,
    recursionLimit: number | null | undefined,
    captureTrivia: boolean,
    parseFloatAsDecimal: boolean,
  ): unknown;
  render_sql(sql: string, dialect: string, mode: string): string;
  render_document?(document: unknown, dialect: string, mode: string): string;
  render_fragment?(document: unknown, nodeId: number, dialect: string, mode: string): string;
  format?(
    sql: string,
    dialect: string,
    indentWidth: number,
    maxWidth: number,
    keywordCase: string,
  ): string;
  supported_dialects(): unknown;
  tokenize(sql: string, dialect: string, includeTrivia: boolean): unknown;
  transpile(sql: string, sourceDialect: string, targetDialect: string): string;
  version(): string;
  schema_version(): number;
}

type WasmInit<TInitOutput> = (
  input?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>,
) => Promise<TInitOutput>;

/** Options for loading a browser package. */
export interface CreateSquonkOptions {
  /** Custom wasm source. Defaults to the package's colocated `.wasm` file. */
  wasm?: InitInput | Promise<InitInput>;
}

interface DocumentRuntime {
  wasm: WasmBindings;
}

const documentRuntimes = new WeakMap<object, DocumentRuntime>();
const WRAPPER_TOKEN: unique symbol = Symbol("squonk.wrapper");

/**
 * Structured parser error thrown by fail-fast APIs.
 *
 * Recovering parse APIs return SQL syntax diagnostics as data, but still throw
 * this error for binding-boundary failures such as unknown dialect names.
 */
export class SqlParseError extends Error {
  readonly kind: DiagnosticKind;
  readonly span: Ast.Span | null;
  readonly expected: string | null;
  readonly found: string | null;

  constructor(diagnostic: DiagnosticJson) {
    super(diagnostic.message);
    this.name = "SqlParseError";
    this.message = diagnostic.message;
    this.kind = diagnostic.kind ?? "syntax";
    this.span = diagnostic.span ?? null;
    this.expected = diagnostic.expected ?? null;
    this.found = diagnostic.found ?? null;
    Object.setPrototypeOf(this, SqlParseError.prototype);
  }
}

/** Parsed SQL document with convenience methods for traversal and rendering. */
export class Document<
  TParse extends ParseResult = ParseResult,
  TDialect extends CanonicalDialectName = TParse["dialect"],
  TSupported extends CanonicalDialectName = CanonicalDialectName,
> {
  #raw: TParse | null;
  #native: NativeDocumentHandle | null;
  readonly #source: string;
  readonly #dialect: TDialect;
  #keywordSymbols: Map<number, string> | null = null;
  #lineStarts: number[] | null = null;

  constructor(
    token: typeof WRAPPER_TOKEN,
    raw: TParse | null,
    native: NativeDocumentHandle | null = null,
    source?: string,
    dialect?: TDialect,
  ) {
    if (token !== WRAPPER_TOKEN) throw new TypeError("Document instances are created by parse()");
    if (raw === null && native === null) {
      throw new TypeError("Document requires a native handle or materialized payload");
    }
    this.#raw = raw;
    this.#native = native;
    this.#source = source ?? raw?.source ?? native?.source ?? "";
    this.#dialect = dialect ?? (raw?.dialect ?? native?.dialect ?? "ansi") as TDialect;
  }

  /** Raw JSON parse payload, materialized on first access. */
  get raw(): TParse {
    if (this.#raw === null) {
      const native = this.#native;
      if (native === null) {
        throw new Error("Document has no native or materialized representation");
      }
      this.#raw = unwrap(() => native.to_value()) as TParse;
      this.#native = null;
    }
    return this.#raw;
  }

  /** Original SQL source. */
  get source(): string {
    return this.#source;
  }

  /** Canonical dialect used to parse this document. */
  get dialect(): TDialect {
    return this.#dialect;
  }

  /** Top-level statements wrapped as traversal nodes. */
  get statements(): Array<Node<Ast.Statement>> {
    return (this.raw.statements ?? []).map(
      (value) => wrapNode(value, this, "Statement") as Node<Ast.Statement>,
    );
  }

  /** Recovering diagnostics. Empty for fail-fast parse documents. */
  get errors(): Diagnostic[] {
    return ((this.raw as ParseResult & { errors?: DiagnosticJson[] }).errors ?? []).map(
      (value) => new Diagnostic(WRAPPER_TOKEN, value, this),
    );
  }

  /** Captured whitespace/comment trivia, when `captureTrivia` was enabled. */
  get trivia(): Trivia[] {
    return this.raw.trivia ?? [];
  }

  /** Resolve a serialized AST symbol id to source or keyword text. */
  resolveSymbol(symbol: number): string {
    const resolver = this.raw.resolver ?? {};
    const dynamicBase = Number(resolver.dynamic_base ?? 1);
    if (symbol < dynamicBase) {
      if (this.#keywordSymbols === null) {
        this.#keywordSymbols = new Map(
          (resolver.keyword_symbols ?? []).map((entry) => [
            Number(entry.symbol),
            String(entry.text),
          ]),
        );
      }
      const text = this.#keywordSymbols.get(symbol);
      if (text === undefined) {
        throw new RangeError(`unknown keyword-backed symbol ${symbol}`);
      }
      return text;
    }

    const index = symbol - dynamicBase;
    const text = this.raw.symbols?.[index];
    if (text === undefined) {
      throw new RangeError(`unknown dynamic symbol ${symbol}`);
    }
    return String(text);
  }

  /** Return exact source text for a byte span. */
  sourceText(span: Ast.Span): string {
    const actual = normalizeSpan(span);
    return new TextDecoder().decode(
      new TextEncoder().encode(this.source).slice(actual.start, actual.end),
    );
  }

  /** Convert a byte offset into line, byte-column, Unicode scalar, and UTF-16 columns. */
  location(offset: number): SourceLocation {
    const starts = this.#lineStartBytes();
    let line = 0;
    let lo = 0;
    let hi = starts.length;
    while (lo < hi) {
      const mid = Math.floor((lo + hi) / 2);
      if (starts[mid] <= offset) {
        line = mid;
        lo = mid + 1;
      } else {
        hi = mid;
      }
    }
    const lineStart = starts[line] ?? 0;
    const prefix = new TextDecoder().decode(
      new TextEncoder().encode(this.source).slice(lineStart, offset),
    );
    return {
      line,
      byteColumn: offset - lineStart,
      charColumn: [...prefix].length,
      utf16Column: prefix.length,
    };
  }

  /** Walk every AST node in document order. */
  *walk(): IterableIterator<Node> {
    const stack: Node[] = [...this.statements].reverse();
    while (stack.length > 0) {
      const node = stack.pop();
      if (node === undefined) {
        continue;
      }
      yield node;
      stack.push(...node.children().reverse());
    }
  }

  /** Find wrapped nodes by runtime kind string. */
  findAll(kind: string): IterableIterator<Node>;
  /** Find wrapped nodes by helper class, for example `findAll(Ident)`. */
  findAll<TNode extends Node>(kind: NodeType<TNode>): IterableIterator<TNode>;
  *findAll<TNode extends Node>(kind: string | NodeType<TNode>): IterableIterator<Node | TNode> {
    for (const node of this.walk()) {
      if (typeof kind === "string" ? node.kind === kind : node instanceof toConstructor(kind)) {
        yield node;
      }
    }
  }

  /** Return the raw JSON parse payload. */
  toJSON(): TParse {
    return this.raw;
  }

  /** Render this document as SQL. */
  toSQL<
    const TTargetDialect extends DialectName<TSupported> = DialectName<TSupported>,
  >(options?: RenderOptions<TSupported, TTargetDialect>): string {
    const runtime = documentRuntimes.get(this);
    if (runtime === undefined) {
      throw new Error("Document.toSQL() requires a document returned by parse()");
    }
    const dialect = options?.dialect ?? this.dialect;
    const mode = options?.mode ?? "canonical";
    if (this.#raw === null && this.#native !== null) {
      return unwrap(() => this.#native?.render(dialect, mode) ?? "");
    }
    if (runtime.wasm.render_document === undefined) {
      throw new Error("Document.toSQL() requires the package's document renderer");
    }
    return unwrap(() => runtime.wasm.render_document?.(this.raw, dialect, mode) ?? "");
  }

  #lineStartBytes(): number[] {
    if (this.#lineStarts === null) {
      const bytes = new TextEncoder().encode(this.source);
      const starts = [0];
      for (let index = 0; index < bytes.length; index += 1) {
        if (bytes[index] === 0x0a) {
          starts.push(index + 1);
        }
      }
      this.#lineStarts = starts;
    }
    return this.#lineStarts;
  }
}

/** Recovering parse document with syntax diagnostics attached as data. */
export class RecoveredDocument<
  TParse extends RecoveringParseResult = RecoveringParseResult,
  TDialect extends CanonicalDialectName = TParse["dialect"],
  TSupported extends CanonicalDialectName = CanonicalDialectName,
> extends Document<TParse, TDialect, TSupported> {
  /** Render the recovered partial document. */
  override toSQL<
    const TTargetDialect extends DialectName<TSupported> = DialectName<TSupported>,
  >(options?: RenderOptions<TSupported, TTargetDialect>): string {
    const runtime = documentRuntimes.get(this);
    if (runtime === undefined) {
      throw new Error("RecoveredDocument.toSQL() requires a document returned by parseRecovering()");
    }
    return super.toSQL(options);
  }
}

/** Wrapped AST node with source spans, traversal, and field access helpers. */
export class Node<TRaw extends Ast.AstNode = Ast.AstNode> {
  readonly raw: TRaw;
  readonly document: Document;
  readonly typeName: string | null;
  kind: string;
  data: AstObject | AstValue;

  constructor(
    token: typeof WRAPPER_TOKEN,
    raw: TRaw,
    document: Document,
    typeName: string | null = null,
  ) {
    if (token !== WRAPPER_TOKEN) throw new TypeError("Node instances are created by parse()");
    this.raw = raw;
    this.document = document;
    this.typeName = typeName;
    const [kind, data, isVariant] = nodeKindAndData(raw);
    this.kind = typeName && !isVariant ? typeName : kind;
    this.data = data;
  }

  /** Source byte span for this node, when the AST payload carries metadata. */
  get span(): Ast.Span | null {
    const data = objectData(this.data) as Record<string, unknown> | null;
    const meta = data?.["meta"];
    return isRecord(meta) && isSpan(meta.span) ? normalizeSpan(meta.span) : null;
  }

  /** Stable parser-allocated node id, when present. */
  get nodeId(): number | null {
    const data = objectData(this.data) as Record<string, unknown> | null;
    const meta = data?.["meta"];
    return isRecord(meta) && typeof meta.node_id === "number" ? meta.node_id : null;
  }

  /** Whether this node can render without surrounding SQL context. */
  get isRenderable(): boolean {
    return this.typeName === "Statement" || this.typeName === "Query" ||
      this.typeName === "Expr" || this.typeName === "DataType";
  }

  /** Read a field from the node payload, wrapping known AST object shapes. */
  get(field: string): WrappedAstValue {
    const data = objectData(this.data) as Record<string, unknown> | null;
    return wrap(
      data?.[field],
      this.document,
      fieldType(this.typeName, this.kind, field),
    );
  }

  /** Exact SQL source text for this node span, or null for synthetic nodes. */
  sourceText(): string | null {
    return this.span ? this.document.sourceText(this.span) : null;
  }

  /** Source location at this node's start offset, or null for synthetic nodes. */
  location(): SourceLocation | null {
    return this.span ? this.document.location(this.span.start) : null;
  }

  /** Direct AST children wrapped as nodes. */
  children(): Node[] {
    const out: Node[] = [];
    for (const [field, value] of childEntries(this.data)) {
      collectNodes(
        wrap(value, this.document, fieldType(this.typeName, this.kind, field)),
        out,
      );
    }
    return out;
  }

  /** Walk this node and all descendants in document order. */
  *walk(): IterableIterator<Node> {
    const stack: Node[] = [this];
    while (stack.length > 0) {
      const node = stack.pop();
      if (node === undefined) {
        continue;
      }
      yield node;
      stack.push(...node.children().reverse());
    }
  }

  /** Find descendant nodes by runtime kind string. */
  findAll(kind: string): IterableIterator<Node>;
  /** Find descendant nodes by helper class, for example `findAll(Ident)`. */
  findAll<TNode extends Node>(kind: NodeType<TNode>): IterableIterator<TNode>;
  *findAll<TNode extends Node>(kind: string | NodeType<TNode>): IterableIterator<Node | TNode> {
    for (const node of this.walk()) {
      if (typeof kind === "string" ? node.kind === kind : node instanceof toConstructor(kind)) {
        yield node;
      }
    }
  }

  /** Return the raw JSON node payload. */
  toJSON(): TRaw {
    return this.raw;
  }

  /** Render this standalone node as a SQL fragment. */
  toSQL<const TTargetDialect extends DialectName = DialectName>(
    options?: RenderOptions<CanonicalDialectName, TTargetDialect>,
  ): string {
    const runtime = documentRuntimes.get(this.document);
    if (!this.isRenderable) {
      throw new SqlParseError({
        message: `${this.kind} requires surrounding SQL context`,
        kind: "unsupported_node_render",
        span: this.span,
      });
    }
    if (runtime === undefined || runtime.wasm.render_fragment === undefined) {
      throw new SqlParseError({
        message: "Node.toSQL() requires a node returned by parse() and fragment rendering support",
        kind: "binding",
        span: this.span,
      });
    }
    if (this.nodeId === null) {
      throw new SqlParseError({
        message: "Node.toSQL() requires a parser-assigned node id",
        kind: "unsupported_node_render",
        span: this.span,
      });
    }
    return unwrap(() => runtime.wasm.render_fragment?.(
      this.document.raw,
      this.nodeId as number,
      options?.dialect ?? this.document.dialect,
      options?.mode ?? "canonical",
    ) ?? "");
  }
}

/** Wrapped identifier with resolved source text. */
export class Ident extends Node<Ast.Ident> {
  constructor(token: typeof WRAPPER_TOKEN, raw: Ast.Ident, document: Document) {
    super(token, raw, document, "Ident");
    this.kind = "Ident";
    this.data = raw as unknown as AstObject;
  }

  /** Serialized symbol id. */
  get symbol(): number {
    return Number((this.data as Ast.Ident).sym);
  }

  /** Identifier text resolved through the document symbol table. */
  get text(): string {
    return this.document.resolveSymbol(this.symbol);
  }

  /** Quote style recorded by the parser. */
  get quote(): Ast.QuoteStyle {
    return String((this.data as Ast.Ident).quote) as Ast.QuoteStyle;
  }
}

/** Dotted object name helper that wraps each identifier part. */
export class ObjectName {
  readonly raw: Ast.ObjectName;
  readonly document: Document;
  readonly parts: Ident[];

  constructor(token: typeof WRAPPER_TOKEN, raw: Ast.ObjectName, document: Document) {
    if (token !== WRAPPER_TOKEN) throw new TypeError("ObjectName instances are created by parse()");
    this.raw = raw;
    this.document = document;
    this.parts = raw.map((part) => new Ident(WRAPPER_TOKEN, part, document));
  }

  /** Object name joined with dots. */
  get text(): string {
    return this.parts.map((part) => part.text).join(".");
  }

  /** Iterate identifier parts in source order. */
  [Symbol.iterator](): IterableIterator<Ident> {
    return this.parts[Symbol.iterator]();
  }

  /** Return the raw JSON object-name payload. */
  toJSON(): Ast.ObjectName {
    return this.raw;
  }
}

/** Diagnostic wrapper with source helpers. */
export class Diagnostic {
  readonly raw: DiagnosticJson;
  readonly document: Document;

  constructor(token: typeof WRAPPER_TOKEN, raw: DiagnosticJson, document: Document) {
    if (token !== WRAPPER_TOKEN) throw new TypeError("Diagnostic instances are created by parseRecovering()");
    this.raw = raw;
    this.document = document;
  }

  /** Human-readable diagnostic message. */
  get message(): string {
    return String(this.raw.message);
  }

  /** Stable diagnostic category. */
  get kind(): DiagnosticKind {
    return this.raw.kind ?? "syntax";
  }

  /** Diagnostic byte span, or null for synthetic/no-source diagnostics. */
  get span(): Ast.Span | null {
    return this.raw.span ? normalizeSpan(this.raw.span) : null;
  }

  /** Exact source text covered by this diagnostic, when it has a span. */
  sourceText(): string | null {
    return this.span ? this.document.sourceText(this.span) : null;
  }

  /** Source location at the diagnostic start offset, when it has a span. */
  location(): SourceLocation | null {
    return this.span ? this.document.location(this.span.start) : null;
  }
}

/** Typed API exported by each package entrypoint. */
export interface SquonkApi<
  TSupported extends CanonicalDialectName,
  TDefault extends TSupported,
> {
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
  parse<const TDialect extends DialectName<TSupported> = DefaultDialect<TSupported, TDefault>>(
    sql: string,
    options?: ParseConfig<TSupported, TDialect>,
  ): Document<ParseResult<CanonicalDialect<TDialect>>, CanonicalDialect<TDialect>, TSupported>;
  /** Fail-fast parse into the raw JSON payload. */
  parseJson<const TDialect extends DialectName<TSupported> = DefaultDialect<TSupported, TDefault>>(
    sql: string,
    options?: ParseConfig<TSupported, TDialect>,
  ): ParseResult<CanonicalDialect<TDialect>>;
  /** Parse with an explicit recursion-depth limit. */
  parseWithLimit<const TDialect extends DialectName<TSupported>>(
    sql: string,
    dialect: TDialect,
    limit: number,
  ): Document<ParseResult<CanonicalDialect<TDialect>>, CanonicalDialect<TDialect>, TSupported>;
  /** Recovering parse into a wrapped document with diagnostics. */
  parseRecovering<
    const TDialect extends DialectName<TSupported> = DefaultDialect<TSupported, TDefault>,
  >(
    sql: string,
    options?: ParseConfig<TSupported, TDialect>,
  ): RecoveredDocument<
    RecoveringParseResult<CanonicalDialect<TDialect>>,
    CanonicalDialect<TDialect>,
    TSupported
  >;
  /** Recovering parse into the raw JSON payload with diagnostics. */
  parseRecoveringJson<
    const TDialect extends DialectName<TSupported> = DefaultDialect<TSupported, TDefault>,
  >(
    sql: string,
    options?: ParseConfig<TSupported, TDialect>,
  ): RecoveringParseResult<CanonicalDialect<TDialect>>;
  /** Dialects compiled into the active wasm artifact. */
  supportedDialects(): DialectInfo<TSupported>[];
  /** Tokenize SQL under a supported dialect. */
  tokenize<const TDialect extends DialectName<TSupported> = DefaultDialect<TSupported, TDefault>>(
    sql: string,
    options?: { dialect?: TDialect; includeTrivia?: boolean },
  ): TokenizeResult<CanonicalDialect<TDialect>>;
  /** Render a SQL string or parsed document. */
  render<const TTargetDialect extends DialectName<TSupported> = DialectName<TSupported>>(
    sqlOrDocument: string | Document<ParseResult, CanonicalDialectName, TSupported>,
    options?: RenderOptions<TSupported, TTargetDialect>,
  ): string;
  /** Render using redaction mode. */
  redact<const TTargetDialect extends DialectName<TSupported> = DialectName<TSupported>>(
    sqlOrDocument: string | Document<ParseResult, CanonicalDialectName, TSupported>,
    options?: Omit<RenderOptions<TSupported, TTargetDialect>, "mode">,
  ): string;
  /** Pretty-print SQL. */
  format<const TDialect extends DialectName<TSupported> = DefaultDialect<TSupported, TDefault>>(
    sql: string,
    options?: FormatOptions<TSupported, TDialect>,
  ): string;
  /** Parse under one dialect and render under another. */
  transpile<
    const TSourceDialect extends DialectName<TSupported> = DefaultDialect<TSupported, TDefault>,
    const TTargetDialect extends DialectName<TSupported> = DefaultDialect<TSupported, TDefault>,
  >(
    sql: string,
    options?: TranspileOptions<TSupported, TSourceDialect, TTargetDialect>,
  ): string;
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
export function createSquonkApi<
  const TSupportedDialects extends readonly CanonicalDialectName[],
  const TDefaultDialect extends TSupportedDialects[number],
>(
  wasm: WasmBindings,
  options: {
    readonly defaultDialect: TDefaultDialect;
    readonly supportedDialects: TSupportedDialects;
    readonly runtime?: RuntimeInfo;
  },
): SquonkApi<TSupportedDialects[number], TDefaultDialect> {
  type TSupported = TSupportedDialects[number];

  const runtime: DocumentRuntime = {
    wasm,
  };

  class RuntimeDocument<
    TParse extends ParseResult = ParseResult,
    TDialect extends CanonicalDialectName = TParse["dialect"],
    TActiveSupported extends CanonicalDialectName = TSupported,
  > extends Document<TParse, TDialect, TActiveSupported> {
    constructor(
      raw: TParse | null,
      native: NativeDocumentHandle | null = null,
      source?: string,
      dialect?: TDialect,
    ) {
      super(WRAPPER_TOKEN, raw, native, source, dialect);
      documentRuntimes.set(this, runtime);
    }
  }

  class RuntimeRecoveredDocument<
    TParse extends RecoveringParseResult = RecoveringParseResult,
    TDialect extends CanonicalDialectName = TParse["dialect"],
    TActiveSupported extends CanonicalDialectName = TSupported,
  > extends RecoveredDocument<TParse, TDialect, TActiveSupported> {
    constructor(
      raw: TParse | null,
      native: NativeDocumentHandle | null = null,
      source?: string,
      dialect?: TDialect,
    ) {
      super(WRAPPER_TOKEN, raw, native, source, dialect);
      documentRuntimes.set(this, runtime);
    }
  }

  function requestedDialect(callOptions: { dialect?: string | undefined }): string {
    const dialect = callOptions.dialect ?? options.defaultDialect;
    assertDialectName(dialect);
    return dialect;
  }

  function canonicalDialectName(value: string): TSupported | null {
    const lower = value.toLowerCase();
    for (const canonical of options.supportedDialects) {
      if ((DIALECT_ALIASES[canonical] as readonly string[]).includes(lower)) {
        return canonical as TSupported;
      }
    }
    return null;
  }

  function isDialectName(value: string): value is ValidatedDialectName<TSupported> {
    return canonicalDialectName(value) !== null;
  }

  function assertDialectName(value: string): asserts value is ValidatedDialectName<TSupported> {
    if (!isDialectName(value)) {
      throw new SqlParseError({
        message: `unknown or unsupported dialect: ${JSON.stringify(value)}`,
        kind: "unknown_dialect",
        span: null,
      });
    }
  }

  function parse<
    const TDialect extends DialectName<TSupported> = DefaultDialect<TSupported, TDefaultDialect>,
  >(
    sql: string,
    parseOptions: ParseConfig<TSupported, TDialect> = {},
  ): Document<ParseResult<CanonicalDialect<TDialect>>, CanonicalDialect<TDialect>, TSupported> {
    const requested = requestedDialect(parseOptions);
    const dialect = canonicalDialectName(requested);
    if (dialect === null) {
      assertDialectName(requested);
      throw new Error("unreachable");
    }
    const native = unwrap(() =>
      wasm.parse_document_with(
        sql,
        requested,
        parseOptions.recursionLimit ?? undefined,
        parseOptions.captureTrivia ?? false,
        parseOptions.parseFloatAsDecimal ?? false,
      )
    );
    return new RuntimeDocument<
      ParseResult<CanonicalDialect<TDialect>>,
      CanonicalDialect<TDialect>
    >(
      null,
      native,
      sql,
      dialect as CanonicalDialect<TDialect>,
    );
  }

  function parseJson<
    const TDialect extends DialectName<TSupported> = DefaultDialect<TSupported, TDefaultDialect>,
  >(
    sql: string,
    parseOptions: ParseConfig<TSupported, TDialect> = {},
  ): ParseResult<CanonicalDialect<TDialect>> {
    return unwrap(() =>
      wasm.parse_with(
        sql,
        requestedDialect(parseOptions),
        parseOptions.recursionLimit ?? undefined,
        parseOptions.captureTrivia ?? false,
        parseOptions.parseFloatAsDecimal ?? false,
      ),
    ) as ParseResult<CanonicalDialect<TDialect>>;
  }

  function parseWithLimit<const TDialect extends DialectName<TSupported>>(
    sql: string,
    dialect: TDialect,
    limit: number,
  ): Document<ParseResult<CanonicalDialect<TDialect>>, CanonicalDialect<TDialect>, TSupported> {
    return parse(sql, { dialect, recursionLimit: limit });
  }

  function parseRecovering<
    const TDialect extends DialectName<TSupported> = DefaultDialect<TSupported, TDefaultDialect>,
  >(
    sql: string,
    parseOptions: ParseConfig<TSupported, TDialect> = {},
  ): RecoveredDocument<
    RecoveringParseResult<CanonicalDialect<TDialect>>,
    CanonicalDialect<TDialect>,
    TSupported
  > {
    const requested = requestedDialect(parseOptions);
    const dialect = canonicalDialectName(requested);
    if (dialect === null) {
      assertDialectName(requested);
      throw new Error("unreachable");
    }
    const native = unwrap(() =>
      wasm.parse_recovering_document_with(
        sql,
        requested,
        parseOptions.recursionLimit ?? undefined,
        parseOptions.captureTrivia ?? false,
        parseOptions.parseFloatAsDecimal ?? false,
      )
    );
    return new RuntimeRecoveredDocument<
      RecoveringParseResult<CanonicalDialect<TDialect>>,
      CanonicalDialect<TDialect>
    >(
      null,
      native,
      sql,
      dialect as CanonicalDialect<TDialect>,
    );
  }

  function parseRecoveringJson<
    const TDialect extends DialectName<TSupported> = DefaultDialect<TSupported, TDefaultDialect>,
  >(
    sql: string,
    parseOptions: ParseConfig<TSupported, TDialect> = {},
  ): RecoveringParseResult<CanonicalDialect<TDialect>> {
    return unwrap(() =>
      wasm.parse_recovering_with(
        sql,
        requestedDialect(parseOptions),
        parseOptions.recursionLimit ?? undefined,
        parseOptions.captureTrivia ?? false,
        parseOptions.parseFloatAsDecimal ?? false,
      ),
    ) as RecoveringParseResult<CanonicalDialect<TDialect>>;
  }

  function supportedDialects(): DialectInfo<TSupported>[] {
    const active = new Set<string>(options.supportedDialects);
    const dialects = unwrap(() => wasm.supported_dialects()) as DialectInfo<CanonicalDialectName>[];
    return dialects
      .filter((dialect) => active.has(dialect.name)) as DialectInfo<TSupported>[];
  }

  function tokenize<
    const TDialect extends DialectName<TSupported> = DefaultDialect<TSupported, TDefaultDialect>,
  >(
    sql: string,
    tokenizeOptions: { dialect?: TDialect; includeTrivia?: boolean } = {},
  ): TokenizeResult<CanonicalDialect<TDialect>> {
    return unwrap(() =>
      wasm.tokenize(
        sql,
        requestedDialect(tokenizeOptions),
        tokenizeOptions.includeTrivia ?? false,
      ),
    ) as TokenizeResult<CanonicalDialect<TDialect>>;
  }

  function render<
    const TTargetDialect extends DialectName<TSupported> = DialectName<TSupported>,
  >(
    sqlOrDocument: string | Document<ParseResult, CanonicalDialectName, TSupported>,
    renderOptions: RenderOptions<TSupported, TTargetDialect> = {},
  ): string {
    if (sqlOrDocument instanceof Document) {
      return sqlOrDocument.toSQL(renderOptions);
    }
    return unwrap(() =>
      wasm.render_sql(
        sqlOrDocument,
        requestedDialect(renderOptions),
        renderOptions.mode ?? "canonical",
      ),
    );
  }

  function redact<
    const TTargetDialect extends DialectName<TSupported> = DialectName<TSupported>,
  >(
    sqlOrDocument: string | Document<ParseResult, CanonicalDialectName, TSupported>,
    renderOptions: Omit<RenderOptions<TSupported, TTargetDialect>, "mode"> = {},
  ): string {
    return render(sqlOrDocument, { ...renderOptions, mode: "redacted" });
  }

  function format<
    const TDialect extends DialectName<TSupported> = DefaultDialect<TSupported, TDefaultDialect>,
  >(
    sql: string,
    formatOptions: FormatOptions<TSupported, TDialect> = {},
  ): string {
    if (wasm.format === undefined) {
      throw new Error("format() is unavailable in this package build");
    }
    return unwrap(() =>
      wasm.format?.(
        sql,
        requestedDialect(formatOptions),
        formatOptions.indentWidth ?? 2,
        formatOptions.maxWidth ?? 80,
        formatOptions.keywordCase ?? "upper",
      ) ?? "",
    );
  }

  function transpile<
    const TSourceDialect extends DialectName<TSupported> = DefaultDialect<TSupported, TDefaultDialect>,
    const TTargetDialect extends DialectName<TSupported> = DefaultDialect<TSupported, TDefaultDialect>,
  >(
    sql: string,
    transpileOptions: TranspileOptions<TSupported, TSourceDialect, TTargetDialect> = {},
  ): string {
    const sourceDialect = transpileOptions.sourceDialect ?? options.defaultDialect;
    const targetDialect = transpileOptions.targetDialect ?? options.defaultDialect;
    assertDialectName(sourceDialect);
    assertDialectName(targetDialect);
    return unwrap(() =>
      wasm.transpile(
        sql,
        sourceDialect,
        targetDialect,
      ),
    );
  }

  function version(): string {
    return wasm.version();
  }

  function schemaVersion(): number {
    return wasm.schema_version();
  }

  const runtimeInfoValue = Object.freeze<RuntimeInfo>(
    options.runtime ?? { backend: "wasm", host: "unknown" },
  );

  function runtimeInfo(): RuntimeInfo {
    return runtimeInfoValue;
  }

  return {
    SqlParseError,
    Document: RuntimeDocument as typeof Document,
    RecoveredDocument: RuntimeRecoveredDocument as typeof RecoveredDocument,
    Node,
    Ident,
    ObjectName,
    Diagnostic,
    isDialectName,
    assertDialectName,
    canonicalDialectName,
    parse,
    parseJson,
    parseWithLimit,
    parseRecovering,
    parseRecoveringJson,
    supportedDialects,
    tokenize,
    render,
    redact,
    format,
    transpile,
    version,
    schemaVersion,
    runtimeInfo,
  };
}

/** Build the one-shot asynchronous loader exposed by each browser entrypoint. */
export function createBrowserSquonk<
  const TSupportedDialects extends readonly CanonicalDialectName[],
  const TDefaultDialect extends TSupportedDialects[number],
  TInitOutput,
>(
  initWasm: WasmInit<TInitOutput>,
  wasm: WasmBindings,
  defaultWasmUrl: URL,
  options: {
    readonly defaultDialect: TDefaultDialect;
    readonly supportedDialects: TSupportedDialects;
  },
): (
  createOptions?: CreateSquonkOptions,
) => Promise<SquonkApi<TSupportedDialects[number], TDefaultDialect>> {
  let active: Promise<SquonkApi<TSupportedDialects[number], TDefaultDialect>> | undefined;

  return function createSquonk(
    createOptions: CreateSquonkOptions = {},
  ): Promise<SquonkApi<TSupportedDialects[number], TDefaultDialect>> {
    if (active === undefined) {
      const input = createOptions.wasm ?? defaultWasmUrl;
      active = initWasm({ module_or_path: input })
        .then(() => createSquonkApi(wasm, {
          ...options,
          runtime: { backend: "wasm", host: "browser" },
        }))
        .catch((error: unknown) => {
          active = undefined;
          throw error;
        });
    }
    return active;
  };
}

function unwrap<T>(call: () => T): T {
  try {
    return call();
  } catch (error) {
    throw toSqlParseError(error);
  }
}

function toSqlParseError(error: unknown): unknown {
  if (error instanceof SqlParseError) {
    return error;
  }
  if (typeof error === "string") {
    try {
      return new SqlParseError(JSON.parse(error) as DiagnosticJson);
    } catch {
      return new SqlParseError({ message: error, kind: "binding", span: null });
    }
  }
  if (isDiagnosticLike(error)) {
    return new SqlParseError(error);
  }
  if (isRecord(error) && typeof error.message === "string") {
    try {
      return new SqlParseError(JSON.parse(error.message) as DiagnosticJson);
    } catch {
      return error;
    }
  }
  return error;
}

function toConstructor<TNode extends Node>(kind: NodeType<TNode>): abstract new (
  ...args: never[]
) => TNode {
  return kind as abstract new (...args: never[]) => TNode;
}

function isDiagnosticLike(value: unknown): value is DiagnosticJson {
  return (
    isRecord(value) &&
    typeof value.message === "string" &&
    typeof value.kind === "string" &&
    ("span" in value ? isSpan(value.span) || value.span === null : true)
  );
}

function wrap(value: unknown, document: Document, typeSpec: string | null = null): WrappedAstValue {
  if (value == null) {
    return value;
  }
  if (typeSpec === "ObjectName" && Array.isArray(value)) {
    return new ObjectName(WRAPPER_TOKEN, value as Ast.ObjectName, document);
  }
  if (typeSpec === "Ident" && isIdent(value)) {
    return new Ident(WRAPPER_TOKEN, value, document);
  }
  if (Array.isArray(value)) {
    return value.map((item) => wrap(item, document, arrayElementType(typeSpec)));
  }
  if (isRecord(value)) {
    if (isIdent(value)) {
      return new Ident(WRAPPER_TOKEN, value, document);
    }
    return new Node(WRAPPER_TOKEN, value as Ast.AstNode, document, nodeType(typeSpec));
  }
  return value as AstValue;
}

function wrapNode(value: unknown, document: Document, typeSpec: string | null = null): Node {
  const wrapped = wrap(value, document, typeSpec);
  if (wrapped instanceof Node) {
    return wrapped;
  }
  throw new TypeError(`expected AST node object, got ${typeof value}`);
}

function nodeKindAndData(raw: Ast.AstNode): [string, AstObject | AstValue, boolean] {
  if (isRecord(raw)) {
    const entries = Object.entries(raw);
    if (entries.length === 1 && /^[A-Z]/.test(entries[0]?.[0] ?? "")) {
      return [entries[0]![0], entries[0]![1] as AstValue, true];
    }
    return ["Object", raw as AstObject, false];
  }
  return [String(raw), raw as AstValue, false];
}

function objectData(value: AstObject | AstValue): AstObject | null {
  return isRecord(value) ? (value as AstObject) : null;
}

function isIdent(value: unknown): value is Ast.Ident {
  return (
    isRecord(value) &&
    typeof value.sym === "number" &&
    typeof value.quote === "string" &&
    isRecord(value.meta)
  );
}

function childEntries(value: unknown): Array<[string, unknown]> {
  if (!isRecord(value) && !Array.isArray(value)) {
    return [];
  }
  if (Array.isArray(value)) {
    return value.map((item, index) => [String(index), item]);
  }
  return Object.entries(value).filter(([key]) => key !== "meta");
}

function fieldType(typeName: string | null, kind: string, field: string): string | null {
  if (typeName && kind) {
    const variantFields = AST_FIELD_TYPES[`${typeName}.${kind}`];
    if (variantFields && hasOwn(variantFields, field)) {
      return variantFields[field] ?? null;
    }
  }
  const fields = typeName ? AST_FIELD_TYPES[typeName] : null;
  if (fields && hasOwn(fields, field)) {
    return fields[field] ?? null;
  }
  return null;
}

function hasOwn(object: object, key: string): boolean {
  return Object.prototype.hasOwnProperty.call(object, key);
}

function arrayElementType(typeSpec: string | null): string | null {
  return typeof typeSpec === "string" && typeSpec.endsWith("[]")
    ? typeSpec.slice(0, -2)
    : null;
}

function nodeType(typeSpec: string | null): string | null {
  if (typeof typeSpec !== "string" || typeSpec === "NoExt" || typeSpec.endsWith("[]")) {
    return null;
  }
  return typeSpec;
}

function collectNodes(value: WrappedAstValue, out: Node[]): void {
  if (value instanceof Node) {
    out.push(value);
    return;
  }
  if (value instanceof ObjectName) {
    out.push(...value.parts);
    return;
  }
  if (Array.isArray(value)) {
    for (const item of value) {
      collectNodes(item, out);
    }
  }
}

function normalizeSpan(span: Ast.Span): Ast.Span {
  return { start: Number(span.start), end: Number(span.end) };
}

function isSpan(value: unknown): value is Ast.Span {
  return isRecord(value) && typeof value.start === "number" && typeof value.end === "number";
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}
