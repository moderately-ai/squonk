# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Moderately AI Inc.

from __future__ import annotations

import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


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


if __name__ == "__main__":
    unittest.main()
