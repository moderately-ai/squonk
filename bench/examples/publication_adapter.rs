// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Machine-readable Rust adapter for the publication benchmark controller.

use std::hint::black_box;
use std::io::{self, BufRead, Write};
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlparser::{dialect::AnsiDialect, parser::Parser};
use squonk::{dialect::Ansi, parse_with};

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[derive(Deserialize)]
struct Corpus {
    sha256: String,
    statements: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    id: String,
    sql: String,
}

#[derive(Clone, Copy)]
enum Tool {
    Squonk,
    Sqlparser,
}

impl Tool {
    fn parse(value: &str) -> Self {
        match value {
            "squonk" => Self::Squonk,
            "datafusion-sqlparser-rs" => Self::Sqlparser,
            _ => panic!("unknown tool: {value}"),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Squonk => "squonk",
            Self::Sqlparser => "datafusion-sqlparser-rs",
        }
    }
}

struct Args {
    mode: String,
    tool: Tool,
    count: usize,
}

fn args() -> Args {
    let mut values = std::env::args().skip(1);
    let mode = values.next().expect("mode is required");
    let mut tool = None;
    let mut count = 0;
    while let Some(arg) = values.next() {
        match arg.as_str() {
            "--tool" => tool = values.next().map(|value| Tool::parse(&value)),
            "--count" => {
                count = values
                    .next()
                    .expect("count value")
                    .parse()
                    .expect("integer count")
            }
            _ => panic!("unknown argument: {arg}"),
        }
    }
    Args {
        mode,
        tool: tool.expect("--tool is required"),
        count,
    }
}

fn corpus() -> Corpus {
    serde_json::from_str(include_str!("../publication/corpus/portable.json"))
        .expect("checked-in publication corpus")
}

fn qualify(tool: Tool, corpus: &Corpus) {
    let mut digest = Sha256::new();
    let mut failures = Vec::new();
    for case in &corpus.statements {
        let rendered = match tool {
            Tool::Squonk => parse_with(&case.sql, Ansi)
                .map(|document| document.to_string())
                .map_err(|error| error.to_string()),
            Tool::Sqlparser => Parser::parse_sql(&AnsiDialect {}, &case.sql)
                .map(|statements| {
                    statements
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join("; ")
                })
                .map_err(|error| error.to_string()),
        };
        match rendered {
            Ok(payload) => {
                digest.update((payload.len() as u64).to_be_bytes());
                digest.update(payload.as_bytes());
            }
            Err(error) => failures.push(json!({"id": case.id, "error": error})),
        }
    }
    println!(
        "{}",
        json!({
            "schema": "squonk.publication-adapter/1",
            "ecosystem": "rust",
            "tool": tool.name(),
            "version": if matches!(tool, Tool::Squonk) { "1.0.0" } else { "0.62.0" },
            "mode": "qualify",
            "corpus_sha256": corpus.sha256,
            "requested": corpus.statements.len(),
            "accepted": corpus.statements.len() - failures.len(),
            "ast_digest": format!("{:x}", digest.finalize()),
            "failures": failures,
        })
    );
}

fn parse_batch(tool: Tool, sql: &[&str]) -> usize {
    sql.iter()
        .map(|statement| match tool {
            Tool::Squonk => black_box(parse_with(black_box(statement), Ansi).expect("qualified"))
                .statements()
                .len(),
            Tool::Sqlparser => black_box(
                Parser::parse_sql(&AnsiDialect {}, black_box(statement)).expect("qualified"),
            )
            .len(),
        })
        .sum()
}

fn throughput(tool: Tool, corpus: &Corpus) {
    let sql: Vec<_> = corpus
        .statements
        .iter()
        .map(|case| case.sql.as_str())
        .collect();
    let bytes = sql.iter().map(|statement| statement.len()).sum::<usize>();
    let warmup = Instant::now();
    while warmup.elapsed() < Duration::from_secs(2) {
        black_box(parse_batch(tool, &sql));
    }
    let calibration = Instant::now();
    black_box(parse_batch(tool, &sql));
    let passes = (1.0 / calibration.elapsed().as_secs_f64()).ceil().max(1.0) as usize;
    let mut samples = Vec::new();
    let mut sink = 0;
    for _ in 0..7 {
        let started = Instant::now();
        for _ in 0..passes {
            sink ^= parse_batch(tool, &sql);
        }
        let seconds = started.elapsed().as_secs_f64();
        samples.push(json!({
            "seconds": seconds,
            "statements_per_second": sql.len() as f64 * passes as f64 / seconds,
            "mib_per_second": bytes as f64 * passes as f64 / seconds / (1024.0 * 1024.0),
        }));
    }
    println!(
        "{}",
        json!({
            "schema": "squonk.publication-adapter/1",
            "ecosystem": "rust",
            "tool": tool.name(),
            "version": if matches!(tool, Tool::Squonk) { "1.0.0" } else { "0.62.0" },
            "mode": "throughput",
            "corpus_sha256": corpus.sha256,
            "passes_per_sample": passes,
            "samples": samples,
            "sink": sink,
        })
    );
}

fn wait_with_roots<T>(tool: Tool, corpus: &Corpus, roots: Vec<T>) {
    println!(
        "{}",
        json!({
            "schema": "squonk.publication-adapter/1",
            "ecosystem": "rust",
            "tool": tool.name(),
            "version": if matches!(tool, Tool::Squonk) { "1.0.0" } else { "0.62.0" },
            "mode": "retain",
            "corpus_sha256": corpus.sha256,
            "retained_documents": roots.len(),
            "pid": std::process::id(),
            "ready": true,
        })
    );
    io::stdout().flush().expect("flush ready signal");
    let mut line = String::new();
    io::stdin()
        .lock()
        .read_line(&mut line)
        .expect("wait for controller");
    black_box(roots);
}

fn retain(tool: Tool, corpus: &Corpus, count: usize) {
    match tool {
        Tool::Squonk => wait_with_roots(
            tool,
            corpus,
            (0..count)
                .map(|index| {
                    parse_with(
                        &corpus.statements[index % corpus.statements.len()].sql,
                        Ansi,
                    )
                    .expect("qualified")
                })
                .collect(),
        ),
        Tool::Sqlparser => wait_with_roots(
            tool,
            corpus,
            (0..count)
                .map(|index| {
                    Parser::parse_sql(
                        &AnsiDialect {},
                        &corpus.statements[index % corpus.statements.len()].sql,
                    )
                    .expect("qualified")
                })
                .collect(),
        ),
    }
}

fn cold(tool: Tool, corpus: &Corpus) {
    let statement = &corpus.statements[0].sql;
    let sink = match tool {
        Tool::Squonk => parse_with(statement, Ansi)
            .expect("qualified")
            .statements()
            .len(),
        Tool::Sqlparser => Parser::parse_sql(&AnsiDialect {}, statement)
            .expect("qualified")
            .len(),
    };
    println!(
        "{}",
        json!({
            "schema": "squonk.publication-adapter/1",
            "ecosystem": "rust",
            "tool": tool.name(),
            "version": if matches!(tool, Tool::Squonk) { "1.0.0" } else { "0.62.0" },
            "mode": "cold",
            "corpus_sha256": corpus.sha256,
            "sink": sink,
        })
    );
}

fn main() {
    let args = args();
    let corpus = corpus();
    match args.mode.as_str() {
        "qualify" => qualify(args.tool, &corpus),
        "throughput" => throughput(args.tool, &corpus),
        "retain" => retain(args.tool, &corpus, args.count),
        "cold" => cold(args.tool, &corpus),
        _ => panic!("mode must be qualify, throughput, retain, or cold"),
    }
}
