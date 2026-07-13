// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

import { AST_FIELD_TYPES as RAW_AST_FIELD_TYPES } from "../js/ast-metadata.generated.js";
const AST_FIELD_TYPES = RAW_AST_FIELD_TYPES;
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
};
const documentRuntimes = new WeakMap();
const WRAPPER_TOKEN = Symbol("squonk.wrapper");
/**
 * Structured parser error thrown by fail-fast APIs.
 *
 * Recovering parse APIs return SQL syntax diagnostics as data, but still throw
 * this error for binding-boundary failures such as unknown dialect names.
 */
export class SqlParseError extends Error {
    kind;
    span;
    expected;
    found;
    constructor(diagnostic) {
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
export class Document {
    #raw;
    #native;
    #source;
    #dialect;
    #keywordSymbols = null;
    #lineStarts = null;
    constructor(token, raw, native = null, source, dialect) {
        if (token !== WRAPPER_TOKEN)
            throw new TypeError("Document instances are created by parse()");
        if (raw === null && native === null) {
            throw new TypeError("Document requires a native handle or materialized payload");
        }
        this.#raw = raw;
        this.#native = native;
        this.#source = source ?? raw?.source ?? native?.source ?? "";
        this.#dialect = dialect ?? (raw?.dialect ?? native?.dialect ?? "ansi");
    }
    /** Raw JSON parse payload, materialized on first access. */
    get raw() {
        if (this.#raw === null) {
            const native = this.#native;
            if (native === null) {
                throw new Error("Document has no native or materialized representation");
            }
            this.#raw = unwrap(() => native.to_value());
            this.#native = null;
        }
        return this.#raw;
    }
    /** Original SQL source. */
    get source() {
        return this.#source;
    }
    /** Canonical dialect used to parse this document. */
    get dialect() {
        return this.#dialect;
    }
    /** Top-level statements wrapped as traversal nodes. */
    get statements() {
        return (this.raw.statements ?? []).map((value) => wrapNode(value, this, "Statement"));
    }
    /** Recovering diagnostics. Empty for fail-fast parse documents. */
    get errors() {
        return (this.raw.errors ?? []).map((value) => new Diagnostic(WRAPPER_TOKEN, value, this));
    }
    /** Captured whitespace/comment trivia, when `captureTrivia` was enabled. */
    get trivia() {
        return this.raw.trivia ?? [];
    }
    /** Resolve a serialized AST symbol id to source or keyword text. */
    resolveSymbol(symbol) {
        const resolver = this.raw.resolver ?? {};
        const dynamicBase = Number(resolver.dynamic_base ?? 1);
        if (symbol < dynamicBase) {
            if (this.#keywordSymbols === null) {
                this.#keywordSymbols = new Map((resolver.keyword_symbols ?? []).map((entry) => [
                    Number(entry.symbol),
                    String(entry.text),
                ]));
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
    sourceText(span) {
        const actual = normalizeSpan(span);
        return new TextDecoder().decode(new TextEncoder().encode(this.source).slice(actual.start, actual.end));
    }
    /** Convert a byte offset into line, byte-column, Unicode scalar, and UTF-16 columns. */
    location(offset) {
        const starts = this.#lineStartBytes();
        let line = 0;
        let lo = 0;
        let hi = starts.length;
        while (lo < hi) {
            const mid = Math.floor((lo + hi) / 2);
            if (starts[mid] <= offset) {
                line = mid;
                lo = mid + 1;
            }
            else {
                hi = mid;
            }
        }
        const lineStart = starts[line] ?? 0;
        const prefix = new TextDecoder().decode(new TextEncoder().encode(this.source).slice(lineStart, offset));
        return {
            line,
            byteColumn: offset - lineStart,
            charColumn: [...prefix].length,
            utf16Column: prefix.length,
        };
    }
    /** Walk every AST node in document order. */
    *walk() {
        const stack = [...this.statements].reverse();
        while (stack.length > 0) {
            const node = stack.pop();
            if (node === undefined) {
                continue;
            }
            yield node;
            stack.push(...node.children().reverse());
        }
    }
    *findAll(kind) {
        for (const node of this.walk()) {
            if (typeof kind === "string" ? node.kind === kind : node instanceof toConstructor(kind)) {
                yield node;
            }
        }
    }
    /** Return the raw JSON parse payload. */
    toJSON() {
        return this.raw;
    }
    /** Render this document as SQL. */
    toSQL(options) {
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
    #lineStartBytes() {
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
export class RecoveredDocument extends Document {
    /** Render the recovered partial document. */
    toSQL(options) {
        const runtime = documentRuntimes.get(this);
        if (runtime === undefined) {
            throw new Error("RecoveredDocument.toSQL() requires a document returned by parseRecovering()");
        }
        return super.toSQL(options);
    }
}
/** Wrapped AST node with source spans, traversal, and field access helpers. */
export class Node {
    raw;
    document;
    typeName;
    kind;
    data;
    constructor(token, raw, document, typeName = null) {
        if (token !== WRAPPER_TOKEN)
            throw new TypeError("Node instances are created by parse()");
        this.raw = raw;
        this.document = document;
        this.typeName = typeName;
        const [kind, data, isVariant] = nodeKindAndData(raw);
        this.kind = typeName && !isVariant ? typeName : kind;
        this.data = data;
    }
    /** Source byte span for this node, when the AST payload carries metadata. */
    get span() {
        const data = objectData(this.data);
        const meta = data?.["meta"];
        return isRecord(meta) && isSpan(meta.span) ? normalizeSpan(meta.span) : null;
    }
    /** Stable parser-allocated node id, when present. */
    get nodeId() {
        const data = objectData(this.data);
        const meta = data?.["meta"];
        return isRecord(meta) && typeof meta.node_id === "number" ? meta.node_id : null;
    }
    /** Whether this node can render without surrounding SQL context. */
    get isRenderable() {
        return this.typeName === "Statement" || this.typeName === "Query" ||
            this.typeName === "Expr" || this.typeName === "DataType";
    }
    /** Read a field from the node payload, wrapping known AST object shapes. */
    get(field) {
        const data = objectData(this.data);
        return wrap(data?.[field], this.document, fieldType(this.typeName, this.kind, field));
    }
    /** Exact SQL source text for this node span, or null for synthetic nodes. */
    sourceText() {
        return this.span ? this.document.sourceText(this.span) : null;
    }
    /** Source location at this node's start offset, or null for synthetic nodes. */
    location() {
        return this.span ? this.document.location(this.span.start) : null;
    }
    /** Direct AST children wrapped as nodes. */
    children() {
        const out = [];
        for (const [field, value] of childEntries(this.data)) {
            collectNodes(wrap(value, this.document, fieldType(this.typeName, this.kind, field)), out);
        }
        return out;
    }
    /** Walk this node and all descendants in document order. */
    *walk() {
        const stack = [this];
        while (stack.length > 0) {
            const node = stack.pop();
            if (node === undefined) {
                continue;
            }
            yield node;
            stack.push(...node.children().reverse());
        }
    }
    *findAll(kind) {
        for (const node of this.walk()) {
            if (typeof kind === "string" ? node.kind === kind : node instanceof toConstructor(kind)) {
                yield node;
            }
        }
    }
    /** Return the raw JSON node payload. */
    toJSON() {
        return this.raw;
    }
    /** Render this standalone node as a SQL fragment. */
    toSQL(options) {
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
        return unwrap(() => runtime.wasm.render_fragment?.(this.document.raw, this.nodeId, options?.dialect ?? this.document.dialect, options?.mode ?? "canonical") ?? "");
    }
}
/** Wrapped identifier with resolved source text. */
export class Ident extends Node {
    constructor(token, raw, document) {
        super(token, raw, document, "Ident");
        this.kind = "Ident";
        this.data = raw;
    }
    /** Serialized symbol id. */
    get symbol() {
        return Number(this.data.sym);
    }
    /** Identifier text resolved through the document symbol table. */
    get text() {
        return this.document.resolveSymbol(this.symbol);
    }
    /** Quote style recorded by the parser. */
    get quote() {
        return String(this.data.quote);
    }
}
/** Dotted object name helper that wraps each identifier part. */
export class ObjectName {
    raw;
    document;
    parts;
    constructor(token, raw, document) {
        if (token !== WRAPPER_TOKEN)
            throw new TypeError("ObjectName instances are created by parse()");
        this.raw = raw;
        this.document = document;
        this.parts = raw.map((part) => new Ident(WRAPPER_TOKEN, part, document));
    }
    /** Object name joined with dots. */
    get text() {
        return this.parts.map((part) => part.text).join(".");
    }
    /** Iterate identifier parts in source order. */
    [Symbol.iterator]() {
        return this.parts[Symbol.iterator]();
    }
    /** Return the raw JSON object-name payload. */
    toJSON() {
        return this.raw;
    }
}
/** Diagnostic wrapper with source helpers. */
export class Diagnostic {
    raw;
    document;
    constructor(token, raw, document) {
        if (token !== WRAPPER_TOKEN)
            throw new TypeError("Diagnostic instances are created by parseRecovering()");
        this.raw = raw;
        this.document = document;
    }
    /** Human-readable diagnostic message. */
    get message() {
        return String(this.raw.message);
    }
    /** Stable diagnostic category. */
    get kind() {
        return this.raw.kind ?? "syntax";
    }
    /** Diagnostic byte span, or null for synthetic/no-source diagnostics. */
    get span() {
        return this.raw.span ? normalizeSpan(this.raw.span) : null;
    }
    /** Exact source text covered by this diagnostic, when it has a span. */
    sourceText() {
        return this.span ? this.document.sourceText(this.span) : null;
    }
    /** Source location at the diagnostic start offset, when it has a span. */
    location() {
        return this.span ? this.document.location(this.span.start) : null;
    }
}
/** Construct a typed facade over a native Node-API or wasm-bindgen backend. */
export function createSquonkApi(wasm, options) {
    const runtime = {
        wasm,
    };
    class RuntimeDocument extends Document {
        constructor(raw, native = null, source, dialect) {
            super(WRAPPER_TOKEN, raw, native, source, dialect);
            documentRuntimes.set(this, runtime);
        }
    }
    class RuntimeRecoveredDocument extends RecoveredDocument {
        constructor(raw, native = null, source, dialect) {
            super(WRAPPER_TOKEN, raw, native, source, dialect);
            documentRuntimes.set(this, runtime);
        }
    }
    function requestedDialect(callOptions) {
        const dialect = callOptions.dialect ?? options.defaultDialect;
        assertDialectName(dialect);
        return dialect;
    }
    function canonicalDialectName(value) {
        const lower = value.toLowerCase();
        for (const canonical of options.supportedDialects) {
            if (DIALECT_ALIASES[canonical].includes(lower)) {
                return canonical;
            }
        }
        return null;
    }
    function isDialectName(value) {
        return canonicalDialectName(value) !== null;
    }
    function assertDialectName(value) {
        if (!isDialectName(value)) {
            throw new SqlParseError({
                message: `unknown or unsupported dialect: ${JSON.stringify(value)}`,
                kind: "unknown_dialect",
                span: null,
            });
        }
    }
    function parse(sql, parseOptions = {}) {
        const requested = requestedDialect(parseOptions);
        const dialect = canonicalDialectName(requested);
        if (dialect === null) {
            assertDialectName(requested);
            throw new Error("unreachable");
        }
        const native = unwrap(() => wasm.parse_document_with(sql, requested, parseOptions.recursionLimit ?? undefined, parseOptions.captureTrivia ?? false, parseOptions.parseFloatAsDecimal ?? false));
        return new RuntimeDocument(null, native, sql, dialect);
    }
    function parseJson(sql, parseOptions = {}) {
        return unwrap(() => wasm.parse_with(sql, requestedDialect(parseOptions), parseOptions.recursionLimit ?? undefined, parseOptions.captureTrivia ?? false, parseOptions.parseFloatAsDecimal ?? false));
    }
    function parseWithLimit(sql, dialect, limit) {
        return parse(sql, { dialect, recursionLimit: limit });
    }
    function parseRecovering(sql, parseOptions = {}) {
        const requested = requestedDialect(parseOptions);
        const dialect = canonicalDialectName(requested);
        if (dialect === null) {
            assertDialectName(requested);
            throw new Error("unreachable");
        }
        const native = unwrap(() => wasm.parse_recovering_document_with(sql, requested, parseOptions.recursionLimit ?? undefined, parseOptions.captureTrivia ?? false, parseOptions.parseFloatAsDecimal ?? false));
        return new RuntimeRecoveredDocument(null, native, sql, dialect);
    }
    function parseRecoveringJson(sql, parseOptions = {}) {
        return unwrap(() => wasm.parse_recovering_with(sql, requestedDialect(parseOptions), parseOptions.recursionLimit ?? undefined, parseOptions.captureTrivia ?? false, parseOptions.parseFloatAsDecimal ?? false));
    }
    function supportedDialects() {
        const active = new Set(options.supportedDialects);
        const dialects = unwrap(() => wasm.supported_dialects());
        return dialects
            .filter((dialect) => active.has(dialect.name));
    }
    function tokenize(sql, tokenizeOptions = {}) {
        return unwrap(() => wasm.tokenize(sql, requestedDialect(tokenizeOptions), tokenizeOptions.includeTrivia ?? false));
    }
    function render(sqlOrDocument, renderOptions = {}) {
        if (sqlOrDocument instanceof Document) {
            return sqlOrDocument.toSQL(renderOptions);
        }
        return unwrap(() => wasm.render_sql(sqlOrDocument, requestedDialect(renderOptions), renderOptions.mode ?? "canonical"));
    }
    function redact(sqlOrDocument, renderOptions = {}) {
        return render(sqlOrDocument, { ...renderOptions, mode: "redacted" });
    }
    function format(sql, formatOptions = {}) {
        if (wasm.format === undefined) {
            throw new Error("format() is unavailable in this package build");
        }
        return unwrap(() => wasm.format?.(sql, requestedDialect(formatOptions), formatOptions.indentWidth ?? 2, formatOptions.maxWidth ?? 80, formatOptions.keywordCase ?? "upper") ?? "");
    }
    function transpile(sql, transpileOptions = {}) {
        const sourceDialect = transpileOptions.sourceDialect ?? options.defaultDialect;
        const targetDialect = transpileOptions.targetDialect ?? options.defaultDialect;
        assertDialectName(sourceDialect);
        assertDialectName(targetDialect);
        return unwrap(() => wasm.transpile(sql, sourceDialect, targetDialect));
    }
    function version() {
        return wasm.version();
    }
    function schemaVersion() {
        return wasm.schema_version();
    }
    const runtimeInfoValue = Object.freeze(options.runtime ?? { backend: "wasm", host: "unknown" });
    function runtimeInfo() {
        return runtimeInfoValue;
    }
    return {
        SqlParseError,
        Document: RuntimeDocument,
        RecoveredDocument: RuntimeRecoveredDocument,
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
export function createBrowserSquonk(initWasm, wasm, defaultWasmUrl, options) {
    let active;
    return function createSquonk(createOptions = {}) {
        if (active === undefined) {
            const input = createOptions.wasm ?? defaultWasmUrl;
            active = initWasm({ module_or_path: input })
                .then(() => createSquonkApi(wasm, {
                ...options,
                runtime: { backend: "wasm", host: "browser" },
            }))
                .catch((error) => {
                active = undefined;
                throw error;
            });
        }
        return active;
    };
}
function unwrap(call) {
    try {
        return call();
    }
    catch (error) {
        throw toSqlParseError(error);
    }
}
function toSqlParseError(error) {
    if (error instanceof SqlParseError) {
        return error;
    }
    if (typeof error === "string") {
        try {
            return new SqlParseError(JSON.parse(error));
        }
        catch {
            return new SqlParseError({ message: error, kind: "binding", span: null });
        }
    }
    if (isDiagnosticLike(error)) {
        return new SqlParseError(error);
    }
    if (isRecord(error) && typeof error.message === "string") {
        try {
            return new SqlParseError(JSON.parse(error.message));
        }
        catch {
            return error;
        }
    }
    return error;
}
function toConstructor(kind) {
    return kind;
}
function isDiagnosticLike(value) {
    return (isRecord(value) &&
        typeof value.message === "string" &&
        typeof value.kind === "string" &&
        ("span" in value ? isSpan(value.span) || value.span === null : true));
}
function wrap(value, document, typeSpec = null) {
    if (value == null) {
        return value;
    }
    if (typeSpec === "ObjectName" && Array.isArray(value)) {
        return new ObjectName(WRAPPER_TOKEN, value, document);
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
        return new Node(WRAPPER_TOKEN, value, document, nodeType(typeSpec));
    }
    return value;
}
function wrapNode(value, document, typeSpec = null) {
    const wrapped = wrap(value, document, typeSpec);
    if (wrapped instanceof Node) {
        return wrapped;
    }
    throw new TypeError(`expected AST node object, got ${typeof value}`);
}
function nodeKindAndData(raw) {
    if (isRecord(raw)) {
        const entries = Object.entries(raw);
        if (entries.length === 1 && /^[A-Z]/.test(entries[0]?.[0] ?? "")) {
            return [entries[0][0], entries[0][1], true];
        }
        return ["Object", raw, false];
    }
    return [String(raw), raw, false];
}
function objectData(value) {
    return isRecord(value) ? value : null;
}
function isIdent(value) {
    return (isRecord(value) &&
        typeof value.sym === "number" &&
        typeof value.quote === "string" &&
        isRecord(value.meta));
}
function childEntries(value) {
    if (!isRecord(value) && !Array.isArray(value)) {
        return [];
    }
    if (Array.isArray(value)) {
        return value.map((item, index) => [String(index), item]);
    }
    return Object.entries(value).filter(([key]) => key !== "meta");
}
function fieldType(typeName, kind, field) {
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
function hasOwn(object, key) {
    return Object.prototype.hasOwnProperty.call(object, key);
}
function arrayElementType(typeSpec) {
    return typeof typeSpec === "string" && typeSpec.endsWith("[]")
        ? typeSpec.slice(0, -2)
        : null;
}
function nodeType(typeSpec) {
    if (typeof typeSpec !== "string" || typeSpec === "NoExt" || typeSpec.endsWith("[]")) {
        return null;
    }
    return typeSpec;
}
function collectNodes(value, out) {
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
function normalizeSpan(span) {
    return { start: Number(span.start), end: Number(span.end) };
}
function isSpan(value) {
    return isRecord(value) && typeof value.start === "number" && typeof value.end === "number";
}
function isRecord(value) {
    return value !== null && typeof value === "object" && !Array.isArray(value);
}
