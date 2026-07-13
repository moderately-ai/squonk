# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Moderately AI Inc.

"""Python publication adapter for Squonk and sqlglot."""

from __future__ import annotations

import argparse
import gc
import hashlib
import json
import os
from pathlib import Path
import statistics
import sys
import time
from typing import Any, Callable


DEFAULT_CORPUS = Path(__file__).resolve().parent / "corpus" / "portable.json"


def load_cases(path: Path) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    corpus = json.loads(path.read_text())
    return corpus, corpus["statements"]


def adapter(tool: str) -> tuple[str, Callable[[str], object], Callable[[object], str]]:
    if tool == "squonk":
        import squonk

        return (
            squonk.__version__,
            lambda sql: squonk.parse(sql, dialect="ansi"),
            lambda document: document.to_json(),
        )
    if tool == "sqlglot":
        import sqlglot

        return (
            sqlglot.__version__,
            lambda sql: sqlglot.parse_one(sql),
            lambda expression: json.dumps(
                expression.dump(), sort_keys=True, separators=(",", ":")
            ),
        )
    raise ValueError(f"unknown tool: {tool}")


def digest_payloads(payloads: list[str]) -> str:
    digest = hashlib.sha256()
    for payload in payloads:
        encoded = payload.encode()
        digest.update(len(encoded).to_bytes(8, "big"))
        digest.update(encoded)
    return digest.hexdigest()


def qualify(tool: str, corpus_path: Path) -> dict[str, Any]:
    corpus, cases = load_cases(corpus_path)
    version, parse, serialize = adapter(tool)
    payloads: list[str] = []
    failures: list[dict[str, str]] = []
    for case in cases:
        try:
            payloads.append(serialize(parse(case["sql"])))
        except Exception as error:  # adapters must report every rejection
            failures.append({"id": case["id"], "error": str(error)})
    return {
        "schema": "squonk.publication-adapter/1",
        "ecosystem": "python",
        "tool": tool,
        "version": version,
        "mode": "qualify",
        "corpus_sha256": corpus["sha256"],
        "requested": len(cases),
        "accepted": len(cases) - len(failures),
        "ast_digest": digest_payloads(payloads),
        "failures": failures,
    }


def parse_batch(parse: Callable[[str], object], sql: list[str]) -> int:
    sink = 0
    for statement in sql:
        sink ^= id(parse(statement))
    return sink


def throughput(tool: str, corpus_path: Path) -> dict[str, Any]:
    corpus, cases = load_cases(corpus_path)
    version, parse, _ = adapter(tool)
    sql = [case["sql"] for case in cases]
    total_bytes = sum(len(statement.encode()) for statement in sql)

    warmup_started = time.perf_counter()
    while time.perf_counter() - warmup_started < 2.0:
        parse_batch(parse, sql)

    calibration_started = time.perf_counter()
    parse_batch(parse, sql)
    elapsed = time.perf_counter() - calibration_started
    passes = max(1, int(1.0 / elapsed) + 1)
    samples: list[dict[str, float]] = []
    sink = 0
    for _ in range(7):
        started = time.perf_counter()
        for _ in range(passes):
            sink ^= parse_batch(parse, sql)
        seconds = time.perf_counter() - started
        samples.append(
            {
                "seconds": seconds,
                "statements_per_second": len(sql) * passes / seconds,
                "mib_per_second": total_bytes * passes / seconds / (1024 * 1024),
            }
        )
    medians = [sample["mib_per_second"] for sample in samples]
    return {
        "schema": "squonk.publication-adapter/1",
        "ecosystem": "python",
        "tool": tool,
        "version": version,
        "mode": "throughput",
        "corpus_sha256": corpus["sha256"],
        "passes_per_sample": passes,
        "samples": samples,
        "median_mib_per_second": statistics.median(medians),
        "sink": sink,
    }


def retain(tool: str, corpus_path: Path, count: int) -> None:
    corpus, cases = load_cases(corpus_path)
    version, parse, _ = adapter(tool)
    sql = [case["sql"] for case in cases]
    gc.collect()
    retained = [parse(sql[index % len(sql)]) for index in range(count)]
    gc.collect()
    result = {
        "schema": "squonk.publication-adapter/1",
        "ecosystem": "python",
        "tool": tool,
        "version": version,
        "mode": "retain",
        "corpus_sha256": corpus["sha256"],
        "retained_documents": len(retained),
        "pid": os.getpid(),
        "ready": True,
    }
    print(json.dumps(result), flush=True)
    sys.stdin.readline()
    if len(retained) != count:
        raise AssertionError("retained roots must stay live")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("mode", choices=("qualify", "throughput", "retain", "cold"))
    parser.add_argument("--tool", required=True, choices=("squonk", "sqlglot"))
    parser.add_argument("--corpus", type=Path, default=DEFAULT_CORPUS)
    parser.add_argument("--count", type=int, default=0)
    args = parser.parse_args()
    if args.mode == "qualify":
        print(json.dumps(qualify(args.tool, args.corpus)))
    elif args.mode == "throughput":
        print(json.dumps(throughput(args.tool, args.corpus)))
    elif args.mode == "cold":
        corpus, cases = load_cases(args.corpus)
        version, parse, _ = adapter(args.tool)
        result = parse(cases[0]["sql"])
        print(
            json.dumps(
                {
                    "schema": "squonk.publication-adapter/1",
                    "ecosystem": "python",
                    "tool": args.tool,
                    "version": version,
                    "mode": "cold",
                    "corpus_sha256": corpus["sha256"],
                    "sink": id(result),
                }
            )
        )
    else:
        retain(args.tool, args.corpus, args.count)


if __name__ == "__main__":
    main()
