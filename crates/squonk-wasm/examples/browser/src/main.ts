// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

import "./styles.css";

import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
import { sql as sqlLanguage } from "@codemirror/lang-sql";
import {
  bracketMatching,
  defaultHighlightStyle,
  indentOnInput,
  syntaxHighlighting,
} from "@codemirror/language";
import {
  EditorState,
  StateEffect,
  StateField,
  type Extension,
} from "@codemirror/state";
import {
  Decoration,
  type DecorationSet,
  drawSelection,
  EditorView,
  highlightActiveLine,
  highlightActiveLineGutter,
  keymap,
  lineNumbers,
} from "@codemirror/view";
import { hierarchy, tree } from "d3-hierarchy";

import {
  createSquonk,
  SqlParseError,
  type CanonicalDialectName,
  type Diagnostic,
  type Document as SqlDocument,
  type Node as AstNode,
  type RecoveredDocument,
  type Span,
  type Token,
  type TokenizeResult,
} from "../../../js/browser.js";

interface QueryPreset {
  id: string;
  label: string;
  dialect: CanonicalDialectName;
  targetDialect: CanonicalDialectName;
  recovering: boolean;
  sql: string;
}

interface PositionMap {
  byteToCodeUnit: number[];
}

interface AstVisualNode {
  id: number;
  kind: string;
  span: Span | null;
  from: number | null;
  to: number | null;
  depth: number;
  parentId: number | null;
  childIds: number[];
  path: string[];
  node: AstNode;
}

interface VisualDiagnostic {
  diagnostic: Diagnostic | SqlParseError;
  from: number | null;
  to: number | null;
}

interface RunMetrics {
  wasmInitMs: number;
  parseMs: number;
  tokenizeMs: number;
  renderMs: number;
  targetMs: number;
  sqlBytes: number;
  astBytes: number;
  nodeCount: number;
  maxDepth: number;
  tokenCount: number;
  triviaCount: number;
  wasmDecodedBytes: number | null;
  wasmTransferBytes: number | null;
  heapUsedBytes: number | null;
}

interface ViewState {
  source: string;
  positionMap: PositionMap;
  document: SqlDocument | RecoveredDocument;
  nodes: AstVisualNode[];
  diagnostics: VisualDiagnostic[];
  activeNodeId: number | null;
  treeClipped: boolean;
}

interface TreeDatum {
  label: string;
  nodeId: number | null;
  children?: TreeDatum[];
}

interface BrowserMemory {
  readonly usedJSHeapSize: number;
}

type BrowserPerformance = Performance & {
  readonly memory?: BrowserMemory;
};

const PRESETS: QueryPreset[] = [
  {
    id: "join",
    label: "Join + params",
    dialect: "postgres",
    targetDialect: "postgres",
    recovering: false,
    sql: `SELECT u.id, u.email, COUNT(o.id) AS order_count
FROM public.users AS u
LEFT JOIN public.orders AS o ON o.user_id = u.id
WHERE u.status = $1 AND o.created_at >= '2026-01-01'
GROUP BY u.id, u.email
ORDER BY order_count DESC
LIMIT 20`,
  },
  {
    id: "cte",
    label: "CTE rollup",
    dialect: "postgres",
    targetDialect: "postgres",
    recovering: false,
    sql: `WITH regional_sales AS (
  SELECT region, SUM(amount) AS total_sales
  FROM sales.orders
  WHERE order_date >= '2026-01-01'
  GROUP BY region
)
SELECT region, total_sales
FROM regional_sales
WHERE total_sales > 1000
ORDER BY total_sales DESC`,
  },
  {
    id: "ddl",
    label: "DDL schema",
    dialect: "postgres",
    targetDialect: "postgres",
    recovering: false,
    sql: `CREATE TABLE public.users (
  id INTEGER,
  email TEXT NOT NULL,
  created_at TIMESTAMP
);
CREATE INDEX users_email_idx ON public.users (email)`,
  },
  {
    id: "mysql",
    label: "MySQL limit",
    dialect: "mysql",
    targetDialect: "mysql",
    recovering: false,
    sql: `SELECT u.id, u.email
FROM users AS u
WHERE u.email LIKE '%@example.com' AND u.deleted_at IS NULL
ORDER BY u.id DESC
LIMIT 5, 10`,
  },
  {
    id: "duckdb",
    label: "DuckDB star",
    dialect: "duckdb",
    targetDialect: "lenient",
    recovering: false,
    sql: `SELECT * EXCLUDE (internal_notes)
FROM orders
WHERE total > 100
ORDER BY total DESC
LIMIT 10`,
  },
  {
    id: "recovery",
    label: "Recovery",
    dialect: "ansi",
    targetDialect: "ansi",
    recovering: true,
    sql: `SELECT 1 AS ok;
FROM broken;
SELECT 2 AS still_ok`,
  },
];

const MAX_TREE_NODES = 240;
const SVG_NS = "http://www.w3.org/2000/svg";
const encoder = new TextEncoder();
const decoder = new TextDecoder();

const setAstDecorations = StateEffect.define<DecorationSet>();
const astDecorationField = StateField.define<DecorationSet>({
  create() {
    return Decoration.none;
  },
  update(decorations, transaction) {
    for (const effect of transaction.effects) {
      if (effect.is(setAstDecorations)) {
        return effect.value;
      }
    }
    return decorations.map(transaction.changes);
  },
  provide: (field) => EditorView.decorations.from(field),
});

const presetList = byId<HTMLDivElement>("presets");
const editorMount = byId<HTMLDivElement>("editor");
const dialectSelect = byId<HTMLSelectElement>("dialect");
const targetDialectSelect = byId<HTMLSelectElement>("target-dialect");
const recoveringInput = byId<HTMLInputElement>("recovering");
const statusOutput = byId<HTMLOutputElement>("status");
const selectionSummary = byId<HTMLOutputElement>("selection-summary");
const treeSummary = byId<HTMLOutputElement>("tree-summary");
const diagnosticsNode = byId<HTMLDivElement>("diagnostics");
const metricsNode = byId<HTMLDivElement>("metrics");
const resourcesNode = byId<HTMLDivElement>("resources");
const nodeDetails = byId<HTMLDivElement>("node-details");
const astTree = byId<HTMLDivElement>("ast-tree");
const renderedNode = byId<HTMLPreElement>("rendered");
const redactedNode = byId<HTMLPreElement>("redacted");
const transpiledNode = byId<HTMLPreElement>("transpiled");
const rawNode = byId<HTMLPreElement>("raw");

let activePresetId: string | null = PRESETS[0]?.id ?? null;
let currentState: ViewState | null = null;
let programmaticEditorChange = false;
let pendingUpdate = 0;

const editor = new EditorView({
  parent: editorMount,
  state: EditorState.create({
    doc: PRESETS[0]?.sql ?? "",
    extensions: editorExtensions(),
  }),
});

const initStart = performance.now();
const {
  canonicalDialectName,
  parse,
  parseRecovering,
  redact,
  render,
  supportedDialects,
  tokenize,
  transpile,
} = await createSquonk();
const wasmInitMs = performance.now() - initStart;

for (const dialect of supportedDialects()) {
  dialectSelect.add(new Option(dialect.name, dialect.name));
  targetDialectSelect.add(new Option(dialect.name, dialect.name));
}

renderPresetButtons();
installEventHandlers();
applyPreset(activePresetId);

function editorExtensions(): Extension[] {
  return [
    lineNumbers(),
    highlightActiveLineGutter(),
    history(),
    drawSelection(),
    indentOnInput(),
    bracketMatching(),
    sqlLanguage(),
    syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
    EditorView.lineWrapping,
    highlightActiveLine(),
    keymap.of([...defaultKeymap, ...historyKeymap]),
    astDecorationField,
    EditorView.domEventHandlers({
      click(event, view) {
        const position = view.posAtCoords({
          x: event.clientX,
          y: event.clientY,
        });
        if (position === null || currentState === null) {
          return false;
        }
        const node = innermostNodeAt(position, currentState.nodes);
        if (node === null) {
          return false;
        }
        selectNode(node.id);
        return true;
      },
    }),
    EditorView.updateListener.of((update) => {
      if (!update.docChanged || programmaticEditorChange) {
        return;
      }
      activePresetId = null;
      paintPresetButtons();
      scheduleUpdate();
    }),
    EditorView.theme({
      "&": {
        height: "100%",
        minHeight: "360px",
      },
      ".cm-scroller": {
        fontFamily:
          'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", monospace',
        fontSize: "13px",
        lineHeight: "1.58",
      },
      ".cm-content": {
        padding: "14px 0",
      },
      ".cm-line": {
        padding: "0 16px",
      },
      ".cm-gutters": {
        backgroundColor: "#111714",
        borderRight: "1px solid #253129",
        color: "#8b9890",
      },
      ".cm-activeLineGutter": {
        backgroundColor: "#1a231e",
        color: "#d9e1dc",
      },
      ".cm-activeLine": {
        backgroundColor: "rgba(255, 255, 255, 0.035)",
      },
      ".cm-selectionBackground": {
        backgroundColor: "rgba(126, 176, 255, 0.28) !important",
      },
      "&.cm-focused": {
        outline: "2px solid #2f6fed",
        outlineOffset: "-2px",
      },
    }),
  ];
}

function installEventHandlers(): void {
  for (const element of [dialectSelect, targetDialectSelect, recoveringInput]) {
    element.addEventListener("input", () => {
      activePresetId = null;
      paintPresetButtons();
      update();
    });
  }

  presetList.addEventListener("click", (event) => {
    const target = event.target;
    if (!(target instanceof Element)) {
      return;
    }
    const button = target.closest<HTMLButtonElement>("[data-preset-id]");
    if (button === null) {
      return;
    }
    applyPreset(button.dataset.presetId ?? null);
  });

  astTree.addEventListener("click", (event) => {
    const target = event.target;
    if (!(target instanceof Element)) {
      return;
    }
    const treeNode = target.closest<SVGGElement>("[data-tree-node-id]");
    if (treeNode === null) {
      return;
    }
    selectNode(Number(treeNode.dataset.treeNodeId));
  });
}

function renderPresetButtons(): void {
  const fragment = document.createDocumentFragment();
  for (const preset of PRESETS) {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "preset-button";
    button.dataset.presetId = preset.id;
    button.textContent = preset.label;
    fragment.append(button);
  }
  presetList.replaceChildren(fragment);
  paintPresetButtons();
}

function paintPresetButtons(): void {
  for (const button of presetList.querySelectorAll<HTMLButtonElement>("[data-preset-id]")) {
    const active = button.dataset.presetId === activePresetId;
    button.classList.toggle("is-active", active);
    button.setAttribute("aria-pressed", String(active));
  }
}

function applyPreset(id: string | null): void {
  const preset = PRESETS.find((candidate) => candidate.id === id) ?? PRESETS[0];
  if (preset === undefined) {
    return;
  }

  activePresetId = preset.id;
  setEditorText(preset.sql);
  setSelectValue(dialectSelect, preset.dialect, "ansi");
  setSelectValue(targetDialectSelect, preset.targetDialect, preset.dialect);
  recoveringInput.checked = preset.recovering;
  paintPresetButtons();
  update();
}

function setEditorText(source: string): void {
  programmaticEditorChange = true;
  editor.dispatch({
    changes: {
      from: 0,
      to: editor.state.doc.length,
      insert: source,
    },
  });
  programmaticEditorChange = false;
}

function scheduleUpdate(): void {
  if (pendingUpdate !== 0) {
    cancelAnimationFrame(pendingUpdate);
  }
  pendingUpdate = requestAnimationFrame(() => {
    pendingUpdate = 0;
    update();
  });
}

function update(): void {
  const source = editor.state.doc.toString();
  const dialect = readDialectSelect(dialectSelect);
  const targetDialect = readDialectSelect(targetDialectSelect);
  const recovering = recoveringInput.checked;
  const positionMap = buildPositionMap(source);

  try {
    const parseStart = performance.now();
    const document = recovering
      ? parseRecovering(source, { dialect, captureTrivia: true })
      : parse(source, { dialect, captureTrivia: true });
    const parseMs = performance.now() - parseStart;

    const tokenizeStart = performance.now();
    const tokenized = tokenize(source, { dialect, includeTrivia: true });
    const tokenizeMs = performance.now() - tokenizeStart;

    const renderStart = performance.now();
    const rendered = render(document);
    const redacted = redact(document);
    const renderMs = performance.now() - renderStart;

    const targetStart = performance.now();
    const targetSql = recovering
      ? render(document, { dialect: targetDialect })
      : transpile(source, { sourceDialect: dialect, targetDialect });
    const targetMs = performance.now() - targetStart;

    const nodes = buildAstNodes(document, positionMap);
    const diagnostics = document.errors.map((diagnostic) =>
      visualDiagnostic(diagnostic, positionMap),
    );
    const astJson = JSON.stringify(document.toJSON());
    const wasmResource = currentWasmResource();

    currentState = {
      source,
      positionMap,
      document,
      nodes,
      diagnostics,
      activeNodeId: defaultNodeId(nodes),
      treeClipped: nodes.length > MAX_TREE_NODES,
    };

    const metrics: RunMetrics = {
      wasmInitMs,
      parseMs,
      tokenizeMs,
      renderMs,
      targetMs,
      sqlBytes: byteLength(source),
      astBytes: byteLength(astJson),
      nodeCount: nodes.length,
      maxDepth: maxDepth(nodes),
      tokenCount: tokenized.tokens.length,
      triviaCount: tokenized.trivia?.length ?? 0,
      wasmDecodedBytes: wasmResource.decodedBytes,
      wasmTransferBytes: wasmResource.transferBytes,
      heapUsedBytes: currentHeapBytes(),
    };

    statusOutput.value = `${formatCount(document.statements.length, "stmt")} | ${formatCount(nodes.length, "node")} | parse ${formatMs(parseMs)} | AST ${formatBytes(byteLength(astJson))}`;
    renderDiagnostics(document.errors);
    renderMetrics(metrics);
    renderTree();
    renderNodeDetails();
    updateEditorDecorations();
    renderedNode.textContent = rendered;
    redactedNode.textContent = redacted;
    transpiledNode.textContent = targetSql;
    rawNode.textContent = JSON.stringify(document.toJSON(), null, 2);
  } catch (error) {
    renderFailure(source, positionMap, error);
  }
}

function buildAstNodes(
  document: SqlDocument | RecoveredDocument,
  positionMap: PositionMap,
): AstVisualNode[] {
  const nodes: AstVisualNode[] = [];
  for (const [index, statement] of document.statements.entries()) {
    visitNode(statement, positionMap, 0, null, [`statement ${index + 1}`], nodes);
  }
  return nodes;
}

function visitNode(
  node: AstNode,
  positionMap: PositionMap,
  depth: number,
  parentId: number | null,
  path: string[],
  nodes: AstVisualNode[],
): number {
  const id = nodes.length;
  const editorSpan = node.span === null ? null : editorSpanFromByteSpan(node.span, positionMap);
  nodes.push({
    id,
    kind: node.kind,
    span: node.span,
    from: editorSpan?.from ?? null,
    to: editorSpan?.to ?? null,
    depth,
    parentId,
    childIds: [],
    path,
    node,
  });

  for (const child of node.children()) {
    const childId = visitNode(child, positionMap, depth + 1, id, [...path, child.kind], nodes);
    nodes[id]!.childIds.push(childId);
  }

  return id;
}

function renderTree(): void {
  if (currentState === null || currentState.nodes.length === 0) {
    astTree.replaceChildren(empty("No AST"));
    treeSummary.value = "";
    return;
  }

  const root = tree<TreeDatum>()
    .nodeSize([30, 175])(
      hierarchy(treeData(currentState.nodes)),
    );

  const descendants = root.descendants();
  const links = root.links();
  const minX = Math.min(...descendants.map((node) => node.x));
  const maxX = Math.max(...descendants.map((node) => node.x));
  const maxY = Math.max(...descendants.map((node) => node.y));
  const width = Math.max(520, maxY + 260);
  const height = Math.max(280, maxX - minX + 80);
  const activeAncestors = activeAncestorIds();

  const svg = createSvg("svg");
  svg.setAttribute("viewBox", `0 ${minX - 40} ${width} ${height}`);
  svg.setAttribute("role", "img");
  svg.setAttribute("aria-label", "AST tree");

  const linkGroup = createSvg("g");
  linkGroup.setAttribute("class", "tree-links");
  for (const link of links) {
    const path = createSvg("path");
    const midpoint = (link.source.y + link.target.y) / 2;
    path.setAttribute(
      "d",
      `M${link.source.y + 16},${link.source.x} C${midpoint},${link.source.x} ${midpoint},${link.target.x} ${link.target.y - 10},${link.target.x}`,
    );
    linkGroup.append(path);
  }
  svg.append(linkGroup);

  const nodeGroup = createSvg("g");
  nodeGroup.setAttribute("class", "tree-nodes");
  for (const treeNode of descendants) {
    const nodeId = treeNode.data.nodeId;
    const group = createSvg("g");
    group.setAttribute("class", "tree-node");
    group.setAttribute("transform", `translate(${treeNode.y},${treeNode.x})`);
    if (nodeId !== null) {
      const visualNode = currentState.nodes[nodeId];
      group.dataset.treeNodeId = String(nodeId);
      group.dataset.nodeKind = visualNode?.kind ?? "";
      group.classList.toggle("is-active", nodeId === currentState.activeNodeId);
      group.classList.toggle("is-ancestor", activeAncestors.has(nodeId));
      group.setAttribute("tabindex", "0");
      group.setAttribute("role", "button");
    }

    const dot = createSvg("circle");
    dot.setAttribute("r", nodeId === null ? "5" : "4");
    group.append(dot);

    const text = createSvg("text");
    text.setAttribute("x", "12");
    text.setAttribute("y", "4");
    text.textContent = treeNode.data.label;
    group.append(text);
    nodeGroup.append(group);
  }
  svg.append(nodeGroup);

  astTree.replaceChildren(svg);
  treeSummary.value = currentState.treeClipped
    ? `${MAX_TREE_NODES} of ${currentState.nodes.length} nodes`
    : `${currentState.nodes.length} nodes`;
}

function treeData(nodes: AstVisualNode[]): TreeDatum {
  let rendered = 0;
  const rootIds = nodes.filter((node) => node.parentId === null).map((node) => node.id);
  return {
    label: "Document",
    nodeId: null,
    children: rootIds
      .map((id) => nodeTreeData(id, nodes, () => rendered++))
      .filter((datum): datum is TreeDatum => datum !== null),
  };
}

function nodeTreeData(
  id: number,
  nodes: AstVisualNode[],
  countRendered: () => number,
): TreeDatum | null {
  if (countRendered() >= MAX_TREE_NODES) {
    return null;
  }
  const node = nodes[id];
  if (node === undefined) {
    return null;
  }
  const children = node.childIds
    .map((childId) => nodeTreeData(childId, nodes, countRendered))
    .filter((datum): datum is TreeDatum => datum !== null);
  return {
    label: node.kind,
    nodeId: id,
    children: children.length === 0 ? undefined : children,
  };
}

function renderNodeDetails(): void {
  if (currentState === null || currentState.activeNodeId === null) {
    selectionSummary.value = "";
    nodeDetails.replaceChildren(empty("No selected node"));
    return;
  }

  const node = currentState.nodes[currentState.activeNodeId];
  if (node === undefined) {
    selectionSummary.value = "";
    nodeDetails.replaceChildren(empty("No selected node"));
    return;
  }

  const spanText = node.span === null ? "no source span" : `${node.span.start}:${node.span.end}`;
  selectionSummary.value = `${node.kind} ${spanText}`;

  const details = document.createElement("div");
  details.className = "node-card";

  const title = document.createElement("strong");
  title.id = "selected-node-kind";
  title.textContent = node.kind;
  details.append(title);

  const list = document.createElement("dl");
  addDatum(list, "Span", spanText);
  addDatum(list, "Depth", String(node.depth));
  addDatum(list, "Children", String(node.childIds.length));
  addDatum(list, "Path", node.path.join(" / "));
  details.append(list);

  if (node.span !== null) {
    const source = document.createElement("pre");
    source.className = "node-source";
    source.textContent = clipText(sliceBytes(currentState.source, node.span.start, node.span.end), 320);
    details.append(source);
  }

  nodeDetails.replaceChildren(details);
}

function selectNode(id: number): void {
  if (currentState === null || Number.isNaN(id) || currentState.nodes[id] === undefined) {
    return;
  }
  currentState.activeNodeId = id;
  renderNodeDetails();
  renderTree();
  updateEditorDecorations();
}

function updateEditorDecorations(): void {
  editor.dispatch({
    effects: setAstDecorations.of(buildDecorations()),
  });
}

function buildDecorations(): DecorationSet {
  if (currentState === null) {
    return Decoration.none;
  }

  const activeAncestors = activeAncestorIds();
  const ranges = [];
  for (const node of currentState.nodes) {
    if (node.from === null || node.to === null || node.from >= node.to) {
      continue;
    }
    const isActive = node.id === currentState.activeNodeId;
    const isAncestor = activeAncestors.has(node.id);
    if (!isActive && !isAncestor && node.depth > 3) {
      continue;
    }
    const classes = ["cm-ast-span", `cm-ast-depth-${node.depth % 6}`];
    if (isActive) {
      classes.push("cm-ast-active");
    } else if (isAncestor) {
      classes.push("cm-ast-ancestor");
    }
    ranges.push(
      Decoration.mark({
        class: classes.join(" "),
        attributes: {
          "data-ast-node-id": String(node.id),
          "data-ast-node-kind": node.kind,
        },
      }).range(node.from, node.to),
    );
  }

  for (const diagnostic of currentState.diagnostics) {
    if (diagnostic.from === null || diagnostic.to === null || diagnostic.from >= diagnostic.to) {
      continue;
    }
    ranges.push(
      Decoration.mark({
        class: "cm-diagnostic-span",
      }).range(diagnostic.from, diagnostic.to),
    );
  }

  return Decoration.set(ranges, true);
}

function renderDiagnostics(diagnostics: Diagnostic[]): void {
  const elements = diagnostics.map((diagnostic) => diagnosticElement(diagnostic));
  diagnosticsNode.replaceChildren(...(elements.length === 0 ? [empty("No diagnostics")] : elements));
}

function diagnosticElement(diagnostic: Diagnostic): HTMLElement {
  const element = document.createElement("div");
  element.className = "diagnostic";
  const location = diagnostic.location();
  element.textContent = location
    ? `${diagnostic.kind}: ${diagnostic.message} at line ${location.line + 1}`
    : `${diagnostic.kind}: ${diagnostic.message}`;
  return element;
}

function renderMetrics(metrics: RunMetrics): void {
  const maxTiming = Math.max(
    metrics.wasmInitMs,
    metrics.parseMs,
    metrics.tokenizeMs,
    metrics.renderMs,
    metrics.targetMs,
    0.01,
  );

  metricsNode.replaceChildren(
    metric("Init", formatMs(metrics.wasmInitMs), metrics.wasmInitMs / maxTiming),
    metric("Parse", formatMs(metrics.parseMs), metrics.parseMs / maxTiming),
    metric("Tokenize", formatMs(metrics.tokenizeMs), metrics.tokenizeMs / maxTiming),
    metric("Render", formatMs(metrics.renderMs), metrics.renderMs / maxTiming),
    metric("Target", formatMs(metrics.targetMs), metrics.targetMs / maxTiming),
  );

  const resources = [
    metric("SQL", formatBytes(metrics.sqlBytes), 0.3),
    metric("AST JSON", formatBytes(metrics.astBytes), 0.58),
    metric("Nodes", String(metrics.nodeCount), 0.74),
    metric("Depth", String(metrics.maxDepth), 0.44),
    metric("Tokens", String(metrics.tokenCount), 0.62),
    metric("Trivia", String(metrics.triviaCount), 0.36),
  ];

  if (metrics.wasmDecodedBytes !== null) {
    resources.push(metric("WASM", formatBytes(metrics.wasmDecodedBytes), 0.66));
  }
  if (metrics.wasmTransferBytes !== null && metrics.wasmTransferBytes > 0) {
    resources.push(metric("Transfer", formatBytes(metrics.wasmTransferBytes), 0.48));
  }
  if (metrics.heapUsedBytes !== null) {
    resources.push(metric("Heap", formatBytes(metrics.heapUsedBytes), 0.52));
  }

  resourcesNode.replaceChildren(...resources);
}

function metric(label: string, value: string, ratio: number): HTMLElement {
  const item = document.createElement("div");
  item.className = "metric";

  const name = document.createElement("span");
  name.textContent = label;
  item.append(name);

  const strong = document.createElement("strong");
  strong.textContent = value;
  item.append(strong);

  const bar = document.createElement("span");
  bar.className = "metric-bar";
  bar.style.setProperty("--fill", `${Math.max(4, Math.min(100, ratio * 100))}%`);
  item.append(bar);

  return item;
}

function renderFailure(source: string, positionMap: PositionMap, error: unknown): void {
  currentState = null;
  statusOutput.value = "parse failed";
  selectionSummary.value = "";
  treeSummary.value = "";
  metricsNode.replaceChildren(empty("No run"));
  resourcesNode.replaceChildren(empty("No resources"));
  nodeDetails.replaceChildren(empty("No selected node"));
  astTree.replaceChildren(empty("No AST"));
  renderedNode.textContent = "";
  redactedNode.textContent = "";
  transpiledNode.textContent = "";
  rawNode.textContent = "";

  if (error instanceof SqlParseError) {
    const diagnostic = visualDiagnostic(error, positionMap);
    currentState = {
      source,
      positionMap,
      document: newEmptyDocument(source),
      nodes: [],
      diagnostics: [diagnostic],
      activeNodeId: null,
      treeClipped: false,
    };
    diagnosticsNode.replaceChildren(errorElement(source, error));
    updateEditorDecorations();
    return;
  }

  updateEditorDecorations();
  throw error;
}

function newEmptyDocument(source: string): SqlDocument {
  return {
    source,
    errors: [],
  } as unknown as SqlDocument;
}

function errorElement(source: string, error: SqlParseError): HTMLElement {
  const message = document.createElement("div");
  message.className = "diagnostic";
  message.textContent = error.span
    ? `${error.kind}: ${error.message} near "${sliceBytes(source, error.span.start, error.span.end)}"`
    : `${error.kind}: ${error.message}`;
  return message;
}

function buildPositionMap(source: string): PositionMap {
  const bytes = encoder.encode(source);
  const byteToCodeUnit = new Array<number>(bytes.length + 1);
  let byteOffset = 0;

  for (let codeUnitOffset = 0; codeUnitOffset < source.length;) {
    const codePoint = source.codePointAt(codeUnitOffset);
    if (codePoint === undefined) {
      break;
    }
    const text = String.fromCodePoint(codePoint);
    const byteLengthForChar = encoder.encode(text).length;
    const nextCodeUnitOffset = codeUnitOffset + text.length;
    for (let offset = 0; offset < byteLengthForChar; offset += 1) {
      byteToCodeUnit[byteOffset + offset] = codeUnitOffset;
    }
    byteOffset += byteLengthForChar;
    byteToCodeUnit[byteOffset] = nextCodeUnitOffset;
    codeUnitOffset = nextCodeUnitOffset;
  }

  byteToCodeUnit[0] = byteToCodeUnit[0] ?? 0;
  byteToCodeUnit[bytes.length] = source.length;
  return { byteToCodeUnit };
}

function editorSpanFromByteSpan(
  span: Span,
  positionMap: PositionMap,
): { from: number; to: number } | null {
  const from = byteOffsetToCodeUnit(span.start, positionMap);
  const to = byteOffsetToCodeUnit(span.end, positionMap);
  if (from === null || to === null) {
    return null;
  }
  return { from, to };
}

function byteOffsetToCodeUnit(offset: number, positionMap: PositionMap): number | null {
  if (offset < 0 || offset >= positionMap.byteToCodeUnit.length) {
    return null;
  }
  return positionMap.byteToCodeUnit[offset] ?? null;
}

function visualDiagnostic(
  diagnostic: Diagnostic | SqlParseError,
  positionMap: PositionMap,
): VisualDiagnostic {
  if (diagnostic.span === null) {
    return { diagnostic, from: null, to: null };
  }
  const editorSpan = editorSpanFromByteSpan(diagnostic.span, positionMap);
  return {
    diagnostic,
    from: editorSpan?.from ?? null,
    to: editorSpan?.to ?? null,
  };
}

function innermostNodeAt(position: number, nodes: AstVisualNode[]): AstVisualNode | null {
  return (
    nodes
      .filter(
        (node) =>
          node.from !== null &&
          node.to !== null &&
          node.from <= position &&
          position <= node.to,
      )
      .sort(
        (left, right) =>
          spanWidth(left) - spanWidth(right) || right.depth - left.depth,
      )[0] ?? null
  );
}

function defaultNodeId(nodes: AstVisualNode[]): number | null {
  const preferred =
    nodes.find((node) => node.kind === "Table" && node.from !== null && node.to !== null) ??
    nodes.find((node) => node.depth > 0 && node.from !== null && node.to !== null);
  return preferred?.id ?? nodes.find((node) => node.from !== null && node.to !== null)?.id ?? null;
}

function activeAncestorIds(): Set<number> {
  const ancestors = new Set<number>();
  if (currentState === null || currentState.activeNodeId === null) {
    return ancestors;
  }

  let parentId = currentState.nodes[currentState.activeNodeId]?.parentId ?? null;
  while (parentId !== null) {
    ancestors.add(parentId);
    parentId = currentState.nodes[parentId]?.parentId ?? null;
  }
  return ancestors;
}

function maxDepth(nodes: AstVisualNode[]): number {
  return nodes.reduce((depth, node) => Math.max(depth, node.depth), 0);
}

function spanWidth(node: AstVisualNode): number {
  if (node.from === null || node.to === null) {
    return Number.MAX_SAFE_INTEGER;
  }
  return node.to - node.from;
}

function setSelectValue(select: HTMLSelectElement, value: string, fallback: string): void {
  select.value = value;
  if (select.value !== value) {
    select.value = fallback;
  }
}

function readDialectSelect(select: HTMLSelectElement): CanonicalDialectName {
  const dialect = canonicalDialectName(select.value);
  if (dialect === null) {
    throw new Error(`unsupported dialect option ${JSON.stringify(select.value)}`);
  }
  return dialect;
}

function currentWasmResource(): { decodedBytes: number | null; transferBytes: number | null } {
  const entry = performance
    .getEntriesByType("resource")
    .find((resource) => resource.name.includes("squonk_wasm_bg.wasm"));
  if (!(entry instanceof PerformanceResourceTiming)) {
    return { decodedBytes: null, transferBytes: null };
  }
  return {
    decodedBytes: entry.decodedBodySize > 0 ? entry.decodedBodySize : null,
    transferBytes: entry.transferSize > 0 ? entry.transferSize : null,
  };
}

function currentHeapBytes(): number | null {
  const browserPerformance: BrowserPerformance = performance;
  return browserPerformance.memory?.usedJSHeapSize ?? null;
}

function byteLength(value: string): number {
  return encoder.encode(value).byteLength;
}

function sliceBytes(source: string, start: number, end: number): string {
  return decoder.decode(encoder.encode(source).slice(start, end));
}

function clipText(value: string, maxLength: number): string {
  return value.length <= maxLength ? value : `${value.slice(0, maxLength - 3)}...`;
}

function formatMs(value: number): string {
  if (value < 0.01) {
    return "<0.01 ms";
  }
  if (value < 10) {
    return `${value.toFixed(2)} ms`;
  }
  return `${value.toFixed(1)} ms`;
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  if (bytes < 1024 * 1024) {
    return `${(bytes / 1024).toFixed(1)} KiB`;
  }
  return `${(bytes / (1024 * 1024)).toFixed(2)} MiB`;
}

function formatCount(count: number, label: string): string {
  return `${count} ${label}${count === 1 ? "" : "s"}`;
}

function addDatum(list: HTMLDListElement, label: string, value: string): void {
  const term = document.createElement("dt");
  term.textContent = label;
  const definition = document.createElement("dd");
  definition.textContent = value;
  list.append(term, definition);
}

function empty(text: string): HTMLElement {
  const element = document.createElement("p");
  element.className = "empty";
  element.textContent = text;
  return element;
}

function createSvg<K extends keyof SVGElementTagNameMap>(tagName: K): SVGElementTagNameMap[K] {
  return document.createElementNS(SVG_NS, tagName);
}

function byId<T extends HTMLElement>(id: string): T {
  const element = document.getElementById(id);
  if (element === null) {
    throw new Error(`missing #${id}`);
  }
  return element as T;
}
