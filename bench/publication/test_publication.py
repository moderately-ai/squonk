# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Moderately AI Inc.

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from PIL import Image


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]


class PublicationTests(unittest.TestCase):
    def test_corpus_generation_is_deterministic(self) -> None:
        before = (HERE / "corpus" / "portable.json").read_bytes()
        subprocess.run(
            [sys.executable, str(HERE / "build_portable_corpus.py")], check=True
        )
        self.assertEqual(before, (HERE / "corpus" / "portable.json").read_bytes())

    def test_publication_result_schema(self) -> None:
        path = HERE / "results" / "headline.json"
        if not path.exists():
            self.skipTest("run publication benchmark first")
        result = json.loads(path.read_text())
        self.assertEqual(result["schema"], "squonk.publication-benchmark/1")
        self.assertEqual(result["workload"]["statement_count"], 256)
        self.assertEqual(len(result["tools"]), 6)
        for tool in result["tools"]:
            qualification = tool["qualification"]
            self.assertEqual(qualification["requested"], 256)
            self.assertEqual(
                qualification["corpus_sha256"], result["workload"]["sha256"]
            )
            if tool["status"] == "qualified":
                self.assertTrue(tool["timing"]["process_runs"])
                self.assertGreaterEqual(len(tool["retained_memory"]["counts"]), 5)
                self.assertIn("stable", tool["timing"])
                self.assertIn("stable", tool["retained_memory"]["summary"])

    def test_plot_is_generated_from_result(self) -> None:
        source = HERE / "results" / "headline.json"
        if not source.exists():
            self.skipTest("run publication benchmark first")
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "plot.png"
            subprocess.run(
                [
                    sys.executable,
                    str(ROOT / "bench" / "plot_performance.py"),
                    "--input",
                    str(source),
                    "--output",
                    str(output),
                ],
                check=True,
            )
            self.assertGreater(output.stat().st_size, 10_000)
            self.assertEqual(
                output.read_bytes(),
                (ROOT / "docs" / "assets" / "full-ast-throughput.png").read_bytes(),
                "the committed graphic must be the exact generator output",
            )

        result = json.loads(source.read_text())
        with Image.open(ROOT / "docs" / "assets" / "full-ast-throughput.png") as image:
            self.assertEqual(
                image.info.get("Benchmark-SHA256"),
                hashlib.sha256(source.read_bytes()).hexdigest(),
            )
            self.assertEqual(
                image.info.get("Benchmark-Source-Commit"), result["source_commit"]
            )
            self.assertEqual(
                image.info.get("Benchmark-X-Axis"), "median_mib_per_second"
            )

        readme = (ROOT / "README.md").read_text()
        performance = (ROOT / "docs" / "performance.md").read_text()
        self.assertIn("./docs/assets/full-ast-throughput.png", readme)
        self.assertIn("(assets/full-ast-throughput.png)", performance)
        self.assertNotIn("performance-summary.png", readme)
        self.assertNotIn("performance-summary.png", performance)

    def test_published_headline_values_match_result(self) -> None:
        result = json.loads((HERE / "results" / "headline.json").read_text())
        tools = {
            (tool["ecosystem"], tool["tool"]): tool for tool in result["tools"]
        }
        ratios = {
            ecosystem: tools[(ecosystem, "squonk")]["timing"][
                "median_mib_per_second"
            ]
            / next(
                tool["timing"]["median_mib_per_second"]
                for (candidate_ecosystem, name), tool in tools.items()
                if candidate_ecosystem == ecosystem and name != "squonk"
            )
            for ecosystem in ("rust", "python", "node")
        }
        readme = (ROOT / "README.md").read_text()
        performance = (ROOT / "docs" / "performance.md").read_text()
        for ecosystem, ratio in ratios.items():
            formatted = f"{ratio:.2f}×"
            self.assertIn(formatted, readme, ecosystem)
            self.assertIn(formatted, performance, ecosystem)
        for tool in result["tools"]:
            median = tool["timing"]["median_mib_per_second"]
            self.assertIn(f"{median:.2f}", performance, tool["tool"])


if __name__ == "__main__":
    unittest.main()
