# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Moderately AI Inc.

"""Render the README throughput figure from the checked-in publication result."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_RESULT = ROOT / "bench" / "publication" / "results" / "headline.json"
OUTPUT = ROOT / "docs" / "assets" / "performance-summary.png"
INK = "#18212b"
MUTED = "#68737d"
GRID = "#d9dee3"
SQUONK = "#087f8c"
REFERENCE = "#718096"


def overlapping_labels(
    labels: list[plt.Text], fig: plt.Figure
) -> list[tuple[str, str]]:
    fig.canvas.draw()
    renderer = fig.canvas.get_renderer()
    boxes = [label.get_window_extent(renderer).expanded(1.02, 1.08) for label in labels]
    return [
        (labels[left_index].get_text(), labels[right_index].get_text())
        for left_index, left in enumerate(boxes)
        for right_index, right in enumerate(boxes[left_index + 1 :], left_index + 1)
        if left.overlaps(right)
    ]


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", type=Path, default=DEFAULT_RESULT)
    parser.add_argument("--output", type=Path, default=OUTPUT)
    args = parser.parse_args()
    result_bytes = args.input.read_bytes()
    result = json.loads(result_bytes)
    if result["schema"] != "squonk.publication-benchmark/1":
        raise ValueError("unsupported publication result schema")

    rows = []
    positions = (5.5, 4.5, 2.75, 1.75, 0.0, -1.0)
    ecosystems = ("rust", "python", "node")
    for ecosystem in ecosystems:
        tools = [tool for tool in result["tools"] if tool["ecosystem"] == ecosystem]
        if len(tools) != 2:
            raise ValueError(f"expected exactly two {ecosystem} tools")
        reference = next(tool for tool in tools if tool["tool"] != "squonk")
        if reference["status"] != "qualified" or not reference["timing"]["stable"]:
            raise ValueError(f"{ecosystem} reference is not publishable")
        baseline = reference["timing"]["median_mib_per_second"]
        for tool in tools:
            if tool["status"] != "qualified" or not tool["timing"]["stable"]:
                raise ValueError(f"{ecosystem} {tool['tool']} is not publishable")
            throughput = tool["timing"]["median_mib_per_second"]
            rows.append((ecosystem, tool["tool"], throughput / baseline, throughput))

    fig, ax = plt.subplots(figsize=(11.8, 6.2))
    fig.subplots_adjust(left=0.31, right=0.94, bottom=0.18, top=0.79)
    labels: list[plt.Text] = []
    for position, (ecosystem, tool, ratio, throughput) in zip(
        positions, rows, strict=True
    ):
        color = SQUONK if tool == "squonk" else REFERENCE
        ax.barh(
            position, ratio, height=0.66, color=color, edgecolor="white", linewidth=1.0
        )
        value = ax.text(
            ratio + 0.05,
            position,
            f"{ratio:.2f}×  ·  {throughput:.2f} MiB/s",
            va="center",
            ha="left",
            color=color,
            fontsize=10,
            fontweight="bold" if tool == "squonk" else "normal",
        )
        labels.append(value)

    display_names = {
        "datafusion-sqlparser-rs": "datafusion-sqlparser-rs",
        "sqlglot": "sqlglot",
        "node-sql-parser": "node-sql-parser",
        "squonk": "Squonk",
    }
    ax.set_yticks(
        positions,
        [
            f"{ecosystem.capitalize()}  ·  {display_names[tool]}"
            for ecosystem, tool, _, _ in rows
        ],
    )
    ax.axvline(1.0, color=INK, linewidth=1.0, linestyle="--", alpha=0.65)
    ax.text(
        1.0,
        6.05,
        "direct peer = 1.00×",
        ha="center",
        va="bottom",
        color=MUTED,
        fontsize=9,
    )
    ax.set_xlim(0, max(ratio for _, _, ratio, _ in rows) + 0.65)
    ax.set_ylim(-1.65, 6.35)
    ax.set_xlabel(
        "Throughput relative to the direct ecosystem peer  →", color=INK, labelpad=12
    )
    ax.grid(axis="x", color=GRID, linewidth=0.8)
    ax.set_axisbelow(True)
    ax.spines[["top", "right", "left"]].set_visible(False)
    ax.tick_params(axis="y", length=0, colors=INK, labelsize=10)
    ax.tick_params(axis="x", colors=MUTED)

    fig.suptitle(
        "Full-AST parsing throughput",
        y=0.965,
        fontsize=20,
        fontweight="bold",
        color=INK,
    )
    fig.text(
        0.5,
        0.885,
        "Same frozen 256-statement workload · median of 10 isolated processes · labels include raw throughput",
        ha="center",
        fontsize=10,
        color=MUTED,
    )
    fig.text(
        0.5,
        0.055,
        f"portable-full-ast-v1 · corpus {result['workload']['sha256'][:12]} · compare within each ecosystem",
        ha="center",
        fontsize=8.8,
        color=MUTED,
    )

    collisions = overlapping_labels(labels, fig)
    if collisions:
        raise RuntimeError(
            f"performance figure contains overlapping data labels: {collisions}"
        )
    args.output.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(
        args.output,
        dpi=190,
        bbox_inches="tight",
        facecolor="white",
        metadata={
            "Benchmark-SHA256": hashlib.sha256(result_bytes).hexdigest(),
            "Benchmark-Source-Commit": result["source_commit"],
        },
    )
    plt.close(fig)


if __name__ == "__main__":
    main()
