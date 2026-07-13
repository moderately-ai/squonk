# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Moderately AI Inc.

"""Run the portable publication benchmark and preserve every raw observation."""

from __future__ import annotations

import argparse
from collections.abc import Sequence
import json
import os
from pathlib import Path
import platform
import random
import statistics
import subprocess
import sys
import time
from typing import Any


ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
CORPUS = HERE / "corpus" / "portable.json"
RUST_ADAPTER = ROOT / "target" / "release" / "examples" / "publication_adapter"
RESULTS = HERE / "results"

TOOLS = (
    ("rust", "squonk"),
    ("rust", "datafusion-sqlparser-rs"),
    ("python", "squonk"),
    ("python", "sqlglot"),
    ("node", "squonk"),
    ("node", "node-sql-parser"),
)


def command_version(argv: Sequence[str]) -> str:
    return subprocess.run(
        argv, check=True, text=True, capture_output=True
    ).stdout.strip()


def host_environment() -> dict[str, Any]:
    cpu_model = platform.processor()
    cpu_info = Path("/proc/cpuinfo")
    if cpu_info.exists():
        for line in cpu_info.read_text().splitlines():
            if line.startswith("model name"):
                cpu_model = line.partition(":")[2].strip()
                break
    governors = sorted(
        {
            path.read_text().strip()
            for path in Path("/sys/devices/system/cpu").glob(
                "cpu[0-9]*/cpufreq/scaling_governor"
            )
        }
    )
    return {
        "platform": platform.platform(),
        "machine": platform.machine(),
        "cpu_model": cpu_model,
        "logical_cpus": os.cpu_count(),
        "cpu_governors": governors,
        "load_average_at_start": list(os.getloadavg()),
        "python": platform.python_version(),
        "node": command_version(["node", "--version"]),
        "rustc": command_version(["rustc", "--version"]),
        "cargo": command_version(["cargo", "--version"]),
    }


def command(ecosystem: str, tool: str, mode: str, count: int = 0) -> list[str]:
    if ecosystem == "rust":
        result = [str(RUST_ADAPTER), mode, "--tool", tool]
    elif ecosystem == "python":
        result = [sys.executable, str(HERE / "python_adapter.py"), mode, "--tool", tool]
    else:
        result = ["node"]
        if mode == "retain":
            result.append("--expose-gc")
        result.extend([str(HERE / "node_adapter.mjs"), mode, "--tool", tool])
    if mode == "retain":
        result.extend(["--count", str(count)])
    return result


def run_json(argv: Sequence[str]) -> dict[str, Any]:
    completed = subprocess.run(
        argv, cwd=ROOT, check=True, text=True, capture_output=True
    )
    return json.loads(completed.stdout)


def cold_observation(ecosystem: str, tool: str) -> float:
    started = time.perf_counter()
    run_json(command(ecosystem, tool, "cold"))
    return (time.perf_counter() - started) * 1000


def current_rss_bytes(pid: int) -> int:
    completed = subprocess.run(
        ["ps", "-o", "rss=", "-p", str(pid)], check=True, text=True, capture_output=True
    )
    return int(completed.stdout.strip()) * 1024


def retained_observation(ecosystem: str, tool: str, count: int) -> dict[str, Any]:
    process = subprocess.Popen(
        command(ecosystem, tool, "retain", count),
        cwd=ROOT,
        text=True,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    assert process.stdout is not None
    assert process.stdin is not None
    line = process.stdout.readline()
    if not line:
        stderr = process.stderr.read() if process.stderr else ""
        raise RuntimeError(f"retain adapter exited before ready: {stderr}")
    ready = json.loads(line)
    rss = current_rss_bytes(ready["pid"])
    process.stdin.write("\n")
    process.stdin.flush()
    stdout, stderr = process.communicate(timeout=30)
    if process.returncode:
        raise RuntimeError(f"retain adapter failed: {stdout}\n{stderr}")
    return {"retained_documents": count, "rss_bytes": rss}


def fit(points: Sequence[dict[str, Any]]) -> dict[str, float]:
    xs = [float(point["retained_documents"]) for point in points]
    ys = [float(point["rss_bytes"]) for point in points]
    x_mean = statistics.mean(xs)
    y_mean = statistics.mean(ys)
    denominator = sum((x - x_mean) ** 2 for x in xs)
    slope = (
        sum((x - x_mean) * (y - y_mean) for x, y in zip(xs, ys, strict=True))
        / denominator
    )
    intercept = y_mean - slope * x_mean
    predicted = [intercept + slope * x for x in xs]
    residual = sum(
        (actual - estimate) ** 2 for actual, estimate in zip(ys, predicted, strict=True)
    )
    total = sum((actual - y_mean) ** 2 for actual in ys)
    return {
        "bytes_per_document": slope,
        "documents_per_mib": 1024 * 1024 / slope,
        "intercept_bytes": intercept,
        "r_squared": 1.0 - residual / total if total else 1.0,
    }


def coefficient_of_variation(values: Sequence[float]) -> float:
    mean = statistics.mean(values)
    return statistics.stdev(values) / mean if len(values) > 1 and mean else 0.0


def bootstrap_interval(values: Sequence[float], seed: int = 20260713) -> list[float]:
    randomizer = random.Random(seed)
    medians = []
    for _ in range(10_000):
        sample = [randomizer.choice(values) for _ in values]
        medians.append(statistics.median(sample))
    medians.sort()
    return [medians[249], medians[9749]]


def measure_memory(
    ecosystem: str, tool: str, repetitions: int, quick: bool
) -> dict[str, Any]:
    counts = [0, 8, 16, 32, 64] if quick else [0, 32, 64, 128, 256]
    observations: list[dict[str, Any]] = []
    while True:
        pending = [(repeat, count) for repeat in range(repetitions) for count in counts]
        random.Random(20260713 + len(counts)).shuffle(pending)
        observations = []
        for repeat, count in pending:
            observation = retained_observation(ecosystem, tool, count)
            observation["repetition"] = repeat
            observations.append(observation)
        baselines = [
            point["rss_bytes"]
            for point in observations
            if point["retained_documents"] == 0
        ]
        largest = [
            point["rss_bytes"]
            for point in observations
            if point["retained_documents"] == counts[-1]
        ]
        delta = statistics.median(largest) - statistics.median(baselines)
        if quick or delta >= 128 * 1024 * 1024 or counts[-1] >= 8192:
            break
        counts.append(counts[-1] * 2)

    fits = []
    for repeat in range(repetitions):
        fits.append(
            fit([point for point in observations if point["repetition"] == repeat])
        )
    slopes = [item["bytes_per_document"] for item in fits]
    r_squared = [item["r_squared"] for item in fits]
    summary = {
        "bytes_per_document": statistics.median(slopes),
        "documents_per_mib": 1024 * 1024 / statistics.median(slopes),
        "slope_cv": coefficient_of_variation(slopes),
        "minimum_r_squared": min(r_squared),
    }
    summary["stable"] = (
        summary["slope_cv"] <= 0.10 and summary["minimum_r_squared"] >= 0.98
    )
    return {
        "counts": counts,
        "observations": observations,
        "fits": fits,
        "summary": summary,
    }


def measure_timings(
    entries: list[dict[str, Any]], process_runs: int, quick: bool
) -> None:
    qualified = [entry for entry in entries if entry["status"] == "qualified"]
    throughput: dict[tuple[str, str], list[dict[str, Any]]] = {
        (entry["ecosystem"], entry["tool"]): [] for entry in qualified
    }
    cold: dict[tuple[str, str], list[float]] = {
        (entry["ecosystem"], entry["tool"]): [] for entry in qualified
    }
    randomizer = random.Random(20260713)
    settle_seconds = 0.0 if quick else 1.0

    for _ in range(process_runs):
        block = qualified.copy()
        randomizer.shuffle(block)
        for entry in block:
            key = (entry["ecosystem"], entry["tool"])
            throughput[key].append(
                run_json(command(entry["ecosystem"], entry["tool"], "throughput"))
            )
            if settle_seconds:
                time.sleep(settle_seconds)

    for _ in range(process_runs):
        block = qualified.copy()
        randomizer.shuffle(block)
        for entry in block:
            key = (entry["ecosystem"], entry["tool"])
            cold[key].append(cold_observation(entry["ecosystem"], entry["tool"]))

    for entry in qualified:
        key = (entry["ecosystem"], entry["tool"])
        runs = throughput[key]
        medians = [
            statistics.median(sample["mib_per_second"] for sample in run["samples"])
            for run in runs
        ]
        timing_cv = coefficient_of_variation(medians)
        entry["timing"] = {
            "process_runs": runs,
            "median_mib_per_second": statistics.median(medians),
            "confidence_interval_95": bootstrap_interval(medians),
            "process_median_cv": timing_cv,
            "stable": timing_cv <= 0.05,
        }
        cold_samples = cold[key]
        entry["cold_start"] = {
            "samples_ms": cold_samples,
            "median_ms": statistics.median(cold_samples),
            "confidence_interval_95": bootstrap_interval(cold_samples),
        }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--quick", action="store_true")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--timing-only", action="store_true")
    parser.add_argument("--cpu", type=int)
    args = parser.parse_args()
    process_runs = 2 if args.quick else 10
    memory_repetitions = 1 if args.quick else 3

    if args.cpu is not None:
        if not hasattr(os, "sched_setaffinity"):
            parser.error("--cpu requires an operating system with CPU affinity support")
        os.sched_setaffinity(0, {args.cpu})

    if not RUST_ADAPTER.exists():
        subprocess.run(
            [
                "cargo",
                "build",
                "--release",
                "-p",
                "squonk-bench",
                "--example",
                "publication_adapter",
            ],
            cwd=ROOT,
            check=True,
        )

    corpus = json.loads(CORPUS.read_text())
    output = args.output or RESULTS / ("smoke.json" if args.quick else "headline.json")
    if args.timing_only:
        if not output.exists():
            parser.error(f"--timing-only requires an existing result: {output}")
        result = json.loads(output.read_text())
        if result["workload"]["sha256"] != corpus["sha256"]:
            parser.error("existing result uses a different workload")
        entries = result["tools"]
    else:
        entries = []
        for ecosystem, tool in TOOLS:
            qualification = run_json(command(ecosystem, tool, "qualify"))
            entries.append(
                {
                    "ecosystem": ecosystem,
                    "tool": tool,
                    "version": qualification["version"],
                    "qualification": qualification,
                    "status": (
                        "qualified"
                        if qualification["accepted"] == qualification["requested"]
                        else "not_qualified"
                    ),
                }
            )

    measure_timings(entries, process_runs, args.quick)

    if not args.timing_only:
        memory_order = [entry for entry in entries if entry["status"] == "qualified"]
        random.Random(20260714).shuffle(memory_order)
        for entry in memory_order:
            entry["retained_memory"] = measure_memory(
                entry["ecosystem"], entry["tool"], memory_repetitions, args.quick
            )

    result = {
        "schema": "squonk.publication-benchmark/1",
        "generated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "source_commit": subprocess.run(
            ["git", "rev-parse", "HEAD"],
            cwd=ROOT,
            check=True,
            text=True,
            capture_output=True,
        ).stdout.strip(),
        "environment": host_environment(),
        "workload": {
            "name": corpus["name"],
            "sha256": corpus["sha256"],
            "statement_count": corpus["statement_count"],
            "input_bytes": sum(item["bytes"] for item in corpus["statements"]),
        },
        "policy": {
            "process_runs": process_runs,
            "memory_repetitions": memory_repetitions,
            "timing_cv_limit": 0.05,
            "memory_slope_cv_limit": 0.10,
            "memory_minimum_r_squared": 0.98,
            "timing_schedule": "blocked randomized interleaving",
            "timing_seed": 20260713,
            "settle_seconds": 0.0 if args.quick else 1.0,
            "cpu_affinity": sorted(os.sched_getaffinity(0))
            if hasattr(os, "sched_getaffinity")
            else None,
            "quick": args.quick,
        },
        "tools": entries,
    }
    if args.timing_only:
        result["generated_at"] = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
        result["policy"].update(
            {
                "process_runs": process_runs,
                "timing_cv_limit": 0.05,
                "timing_schedule": "blocked randomized interleaving",
                "timing_seed": 20260713,
                "settle_seconds": 0.0 if args.quick else 1.0,
                "cpu_affinity": (
                    sorted(os.sched_getaffinity(0))
                    if hasattr(os, "sched_getaffinity")
                    else None
                ),
            }
        )
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(result, indent=2) + "\n")
    print(output)


if __name__ == "__main__":
    main()
