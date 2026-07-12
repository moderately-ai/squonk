// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//
// Warm end-to-end parse throughput for JSQLParser over the shared conformance corpus.
//
// WHAT THIS MEASURES: the warm, single-thread, end-to-end throughput — statements
// parsed per wall-clock second — of `CCJSqlParserUtil.parse(sql)` (build the
// JSQLParser `Statement` AST; pure syntactic parse, no validation) over the
// both-accept subset of the shared corpus.
//
// WHAT IT IS NOT: NOT apples-to-apples with the Rust parser (JVM startup/JIT,
// different memory model). The only honest cross-language metric is end-to-end
// throughput, read through the runtime caption it prints. Memory excluded by design.
//
// NOTE ON RIGOUR: "JMH-lite" manual harness — explicit warm-up to trigger C2, a
// printed blackhole to defeat DCE. JMH is the gold standard for JVM microbenchmarks;
// this trades rigour for a zero-dependency single-file runner.
//
// JSQLPARSER-SPECIFIC CAVEAT: JSQLParser has a single, broadly-permissive grammar
// (no dialect selection), so it tends to accept the most of the three tools. It is a
// STATEMENT parser, so bare-expression candidates fall out of its accept set (and
// thus the intersection) — correct. Recent versions route `CCJSqlParserUtil.parse`
// through an ExecutorService for timeout/backtracking control; if its throughput
// looks anomalously low, that per-call wrapper (not the grammar) is the cost — see
// the notes doc for the lower-overhead direct-parser alternative.
//
// This runner cannot be executed in the sandboxed worktree (no JVM, no Maven
// artifacts). It is written from the JSQLParser API and run later; see
// `docs/performance.md` for the exact classpath + run line.
//
// Class name matches the file so both `javac jsqlparser_throughput.java` and the
// Java 11+ single-file launcher `java -cp "$CP" jsqlparser_throughput.java <args>`
// work (lowercase name honours the ticket filename + javac's public-class rule).

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import java.util.TreeSet;

import net.sf.jsqlparser.parser.CCJSqlParserUtil;
import net.sf.jsqlparser.statement.Statement;

public final class jsqlparser_throughput {

    // ---- shared corpus loader: byte-for-byte port of corpus_loader.py / the Rust
    // ---- segmentation; keep all runners in lockstep so ids line up.
    static final String[][] CORPORA = {
        {"sqlglot_identity", "sqlglot/identity.sql", "line"},
        {"sqllogictest_statements", "sqllogictest/statements.sql", "line"},
        {"postgres_regress_supported", "postgres/regress-supported.sql", "semicolon"},
    };

    static final class Cand {
        final String corpus;
        final int index;
        final String sql;
        Cand(String corpus, int index, String sql) {
            this.corpus = corpus;
            this.index = index;
            this.sql = sql;
        }
        String id() { return corpus + ":" + index; }
    }

    static List<String> splitLine(String text) {
        List<String> out = new ArrayList<>();
        for (String line : text.split("\n", -1)) {
            if (!line.strip().isEmpty()) {
                out.add(line);
            }
        }
        return out;
    }

    static List<String> splitSemicolon(String text) {
        int pos = 0;
        int n = text.length();
        while (pos < n) {
            int eol = text.indexOf('\n', pos);
            int lineEnd = (eol == -1) ? n : eol + 1;
            String stripped = text.substring(pos, lineEnd).stripLeading();
            if (!stripped.isEmpty() && !stripped.startsWith("--")) {
                break;
            }
            pos = lineEnd;
        }
        List<String> out = new ArrayList<>();
        for (String chunk : text.substring(pos).split(";", -1)) {
            String t = chunk.strip();
            if (!t.isEmpty()) {
                out.add(t);
            }
        }
        return out;
    }

    static List<Cand> loadCandidates(Path root) throws IOException {
        List<Cand> out = new ArrayList<>();
        for (String[] spec : CORPORA) {
            String key = spec[0];
            String rel = spec[1];
            String shape = spec[2];
            String text = Files.readString(root.resolve(rel), StandardCharsets.UTF_8);
            List<String> stmts = shape.equals("line") ? splitLine(text) : splitSemicolon(text);
            for (int i = 0; i < stmts.size(); i++) {
                out.add(new Cand(key, i, stmts.get(i)));
            }
        }
        return out;
    }

    static String argOf(String[] a, String name, String def) {
        for (int i = 0; i < a.length - 1; i++) {
            if (a[i].equals(name)) return a[i + 1];
        }
        return def;
    }
    static boolean hasFlag(String[] a, String name) {
        for (String s : a) if (s.equals(name)) return true;
        return false;
    }

    static Path resolveCorpusRoot(String[] args) {
        String env = System.getenv("SQUONK_CORPUS_ROOT");
        String fromArg = argOf(args, "--corpus-root", null);
        String chosen = fromArg != null ? fromArg : env;
        if (chosen == null) {
            for (String s : args) {
                if (!s.startsWith("--")) { chosen = s; break; }
            }
        }
        if (chosen == null) chosen = "../../conformance/corpus";
        return Path.of(chosen);
    }

    static long parseOnce(String sql) throws Exception {
        Statement stmt = CCJSqlParserUtil.parse(sql);
        // Blackhole: identityHashCode keeps the parsed tree observable so C2 cannot
        // dead-code-eliminate the parse. Folded into a sink printed at the end.
        return System.identityHashCode(stmt);
    }

    public static void main(String[] args) throws Exception {
        Path corpusRoot = resolveCorpusRoot(args);
        double warmupSecs = Double.parseDouble(argOf(args, "--warmup-secs", "2.0"));
        int reps = Integer.parseInt(argOf(args, "--reps", "7"));
        double minPassSecs = Double.parseDouble(argOf(args, "--min-pass-secs", "0.20"));
        String subsetPath = argOf(args, "--subset", null);
        String emitPath = argOf(args, "--emit-accepts", null);
        boolean rss = hasFlag(args, "--rss");

        String version = jsqlparser_throughput.class.getPackage().getImplementationVersion();
        if (version == null) version = "unknown (no jar manifest)";

        List<Cand> candidates = loadCandidates(corpusRoot);

        // ACCEPT-PROBE (outside every timed window): Throwable so deep-nesting
        // StackOverflowError is a reject, not a crash.
        Set<String> accepted = new HashSet<>();
        int[] covAcc = new int[CORPORA.length];
        int[] covTot = new int[CORPORA.length];
        for (Cand c : candidates) {
            int ci = corpusIndex(c.corpus);
            covTot[ci]++;
            try {
                parseOnce(c.sql);
                accepted.add(c.id());
                covAcc[ci]++;
            } catch (Throwable t) {
                // reject
            }
        }

        if (emitPath != null) {
            try (var w = Files.newBufferedWriter(Path.of(emitPath), StandardCharsets.UTF_8)) {
                for (String id : new TreeSet<>(accepted)) {
                    w.write(id);
                    w.write("\n");
                }
            }
            System.out.printf("wrote %d accepted ids to %s (JSQLParser %s)%n",
                accepted.size(), emitPath, version);
            return;
        }

        Set<String> requested = null;
        List<String> missing = new ArrayList<>();
        List<String> measuredSql = new ArrayList<>();
        var byId = new java.util.HashMap<String, String>();
        for (Cand c : candidates) byId.put(c.id(), c.sql);

        if (subsetPath != null) {
            requested = readIds(Path.of(subsetPath));
            for (String id : new TreeSet<>(requested)) {
                if (accepted.contains(id)) {
                    measuredSql.add(byId.get(id));
                } else {
                    missing.add(id);
                }
            }
        } else {
            for (String id : new TreeSet<>(accepted)) measuredSql.add(byId.get(id));
        }

        if (measuredSql.isEmpty()) {
            System.err.println("error: measured subset is empty (no accepted ids to time)");
            System.exit(1);
        }

        // WARM-UP: loop the subset until warmupSecs elapses (>= one pass) to let C2
        // compile the hot parse path before measuring.
        long warmEnd = System.nanoTime() + (long) (warmupSecs * 1e9);
        long sink = 0;
        int warmPasses = 0;
        do {
            for (String sql : measuredSql) sink += parseOnce(sql);
            warmPasses++;
        } while (System.nanoTime() < warmEnd);

        long t0 = System.nanoTime();
        for (String sql : measuredSql) sink += parseOnce(sql);
        double onePass = (System.nanoTime() - t0) / 1e9;
        int passes = onePass <= 0 ? 1024 : Math.max(1, (int) (minPassSecs / onePass) + 1);

        double best = 0;
        double[] rates = new double[reps];
        for (int r = 0; r < reps; r++) {
            long s0 = System.nanoTime();
            for (int p = 0; p < passes; p++) {
                for (String sql : measuredSql) sink += parseOnce(sql);
            }
            double dt = (System.nanoTime() - s0) / 1e9;
            double rate = (double) passes * measuredSql.size() / dt;
            rates[r] = rate;
            best = Math.max(best, rate);
        }
        Arrays.sort(rates);
        double median = rates[reps / 2];

        int totCand = 0, totAcc = 0;
        for (int i = 0; i < CORPORA.length; i++) { totCand += covTot[i]; totAcc += covAcc[i]; }

        System.out.println("# cross-language throughput: JSQLParser");
        System.out.printf("#   runtime         : JVM %s  (startup+JIT excluded via warm-up; single-thread)%n",
            System.getProperty("java.version"));
        System.out.printf("#   tool version    : jsqlparser %s%n", version);
        System.out.println("#   parse unit      : CCJSqlParserUtil.parse(sql) -> Statement AST (no validate)");
        System.out.println("#   dialect         : generic (JSQLParser has no dialect selection)");
        System.out.printf("#   corpus root     : %s%n", corpusRoot);
        System.out.println("#   metric          : parses/sec = statements / wall_seconds (warm, 1 thread, END-TO-END)");
        System.out.printf("#   method          : JMH-lite; warm-up >= %.3gs (%d passes), %d timed passes x %d inner loops (>= %.3gs each)%n",
            warmupSecs, warmPasses, reps, passes, minPassSecs);
        if (subsetPath != null) {
            System.out.printf("#   subset          : %s  (%d requested ids)%n", subsetPath, requested.size());
            if (!missing.isEmpty()) {
                System.out.printf("#   WARNING         : %d requested id(s) NOT accepted by JSQLParser -> excluded "
                    + "(subset/version drift; regenerate the intersection)%n", missing.size());
            }
        } else {
            System.out.println("#   subset          : SELF-COVERAGE (this tool's own accept set)");
            System.out.println("#   WARNING         : self-coverage is NOT the comparable both-accept subset;");
            System.out.println("#                     pass --subset both_accept.txt for a fair cross-tool number.");
        }
        System.out.println("#");
        System.out.println("# coverage (JSQLParser accepts / candidates), per corpus:");
        for (int i = 0; i < CORPORA.length; i++) {
            System.out.printf("#   %-28s %5d/%d%n", CORPORA[i][0], covAcc[i], covTot[i]);
        }
        System.out.printf("#   %-28s %5d/%d%n", "TOTAL", totAcc, totCand);
        System.out.println("#");
        System.out.printf("# throughput over the measured subset (%d statements):%n", measuredSql.size());
        System.out.printf("#   best   : %,12.0f parses/sec%n", best);
        System.out.printf("#   median : %,12.0f parses/sec%n", median);
        if (rss) {
            System.out.printf("#   %s%n", rssCaption());
        }
        System.out.printf("#   (blackhole 0x%x — ignore)%n", sink);
    }

    static int corpusIndex(String corpus) {
        for (int i = 0; i < CORPORA.length; i++) {
            if (CORPORA[i][0].equals(corpus)) return i;
        }
        return -1;
    }

    static Set<String> readIds(Path path) throws IOException {
        Set<String> ids = new HashSet<>();
        for (String line : Files.readAllLines(path, StandardCharsets.UTF_8)) {
            String t = line.strip();
            if (!t.isEmpty() && !t.startsWith("#")) ids.add(t);
        }
        return ids;
    }

    static String rssCaption() {
        try {
            for (String line : Files.readAllLines(Path.of("/proc/self/status"))) {
                if (line.startsWith("VmHWM:")) {
                    String[] parts = line.trim().split("\\s+");
                    long kb = Long.parseLong(parts[1]);
                    return String.format(
                        "peak RSS    : ~%d MiB  (WHOLE JVM: heap+metaspace+JIT+GC, NOT per-parse; "
                        + "not comparable to Rust dhat — see notes)", kb / 1024);
                }
            }
        } catch (Exception e) {
            // fall through
        }
        return "peak RSS    : unavailable on this platform (Linux /proc only; excluded by design)";
    }
}
