// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//
// Warm end-to-end parse throughput for Apache Calcite's SQL parser over the shared
// conformance corpus.
//
// WHAT THIS MEASURES: the warm, single-thread, end-to-end throughput — statements
// parsed per wall-clock second — of Calcite's `SqlParser.parseStmt()` (lex + build
// the `SqlNode` AST; NO validation, NO optimization — those are `SqlValidator` /
// the planner, separate stages) over the both-accept subset of the shared corpus.
//
// WHAT IT IS NOT: NOT apples-to-apples with the Rust parser. The JVM pays startup +
// JIT warm-up and has an entirely different memory model; the only honest
// cross-language metric is this end-to-end throughput, read through the runtime
// caption it prints. Memory is excluded by design (see the notes doc).
//
// NOTE ON RIGOUR: this is a "JMH-lite" manual harness — an explicit warm-up loop to
// trigger C2 compilation and a printed blackhole to defeat dead-code elimination.
// For publication-grade JVM microbenchmarks JMH is the gold standard; this trades a
// little rigour for a zero-dependency, single-file runner (see notes for wrapping
// it in JMH if desired).
//
// CALCITE-SPECIFIC CAVEAT: calcite-CORE's parser supports queries + INSERT/UPDATE/
// DELETE but NOT most DDL (CREATE TABLE lives in calcite-server's DDL parser) and
// NOT bare expressions. Those candidates simply fall out of Calcite's accept set and
// thus out of the both-accept subset — which is correct: the intersection is the
// comparable SQL. calcite-server's `SqlDdlParserImpl.FACTORY` would broaden DDL
// coverage; left out to keep the dependency to calcite-core only.
//
// This runner cannot be executed in the sandboxed worktree (no JVM, no Maven
// artifacts). It is written from the Calcite API and run later; see
// `docs/performance.md` for the exact classpath + run line.
//
// The class is named to match the file (`calcite_throughput.java`) so both
// `javac calcite_throughput.java` and the Java 11+ single-file launcher
// `java -cp "$CP" calcite_throughput.java <args>` work; the lowercase name is
// non-idiomatic Java but honours the ticket's filename + javac's public-class rule.

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

import org.apache.calcite.config.Lex;
import org.apache.calcite.sql.SqlNode;
import org.apache.calcite.sql.parser.SqlParser;
import org.apache.calcite.sql.validate.SqlConformanceEnum;

public final class calcite_throughput {

    // ---- shared corpus loader: a byte-for-byte port of corpus_loader.py, which in
    // ---- turn mirrors the Rust harness segmentation. Keep all three in lockstep so
    // ---- `<corpus>:<index>` ids line up across every runner.
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

    // Every non-blank line is one candidate, kept verbatim (Rust LinePerStatement).
    static List<String> splitLine(String text) {
        List<String> out = new ArrayList<>();
        for (String line : text.split("\n", -1)) {
            if (!line.strip().isEmpty()) {
                out.add(line);
            }
        }
        return out;
    }

    // Drop the leading `--`/blank header WHOLESALE (its prose contains a ';'), then
    // split the remainder on ';', trim, drop empties (Rust pg_regress_statements).
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

    // ---- arg helpers (minimal; this is a runner, not a CLI framework) ----
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
            // First bare positional, else the script-relative default. The notes say
            // run from bench/cross-language/, so ../../conformance/corpus is correct.
            for (String s : args) {
                if (!s.startsWith("--")) { chosen = s; break; }
            }
        }
        if (chosen == null) chosen = "../../conformance/corpus";
        return Path.of(chosen);
    }

    // The Calcite parse config — the load-bearing "approximate dialect" choice. There
    // is no Postgres lex in Calcite; MYSQL_ANSI gives ANSI double-quoted identifiers,
    // and LENIENT conformance maximizes acceptance. Both are overridable so the
    // operator can name the surface they measured.
    static SqlParser.Config parseConfig(String[] args) {
        Lex lex = Lex.valueOf(argOf(args, "--lex", "MYSQL_ANSI").toUpperCase());
        SqlConformanceEnum conf =
            SqlConformanceEnum.valueOf(argOf(args, "--conformance", "LENIENT").toUpperCase());
        return SqlParser.config().withLex(lex).withConformance(conf);
    }

    static long parseOnce(String sql, SqlParser.Config config) throws Exception {
        SqlNode node = SqlParser.create(sql, config).parseStmt();
        // Blackhole: identityHashCode of the root keeps the parsed tree observable so
        // C2 cannot dead-code-eliminate the parse. Folded into a sink printed at end.
        return System.identityHashCode(node);
    }

    public static void main(String[] args) throws Exception {
        Path corpusRoot = resolveCorpusRoot(args);
        SqlParser.Config config = parseConfig(args);
        String lexName = argOf(args, "--lex", "MYSQL_ANSI").toUpperCase();
        String confName = argOf(args, "--conformance", "LENIENT").toUpperCase();
        double warmupSecs = Double.parseDouble(argOf(args, "--warmup-secs", "2.0"));
        int reps = Integer.parseInt(argOf(args, "--reps", "7"));
        double minPassSecs = Double.parseDouble(argOf(args, "--min-pass-secs", "0.20"));
        String subsetPath = argOf(args, "--subset", null);
        String emitPath = argOf(args, "--emit-accepts", null);
        boolean rss = hasFlag(args, "--rss");

        String version = calcite_throughput.class.getPackage().getImplementationVersion();
        if (version == null) version = "unknown (no jar manifest)";

        List<Cand> candidates = loadCandidates(corpusRoot);

        // ACCEPT-PROBE (outside every timed window): catch Throwable so a deep-nesting
        // StackOverflowError counts as a reject, not a crash.
        Set<String> accepted = new HashSet<>();
        int[] covAcc = new int[CORPORA.length];
        int[] covTot = new int[CORPORA.length];
        for (Cand c : candidates) {
            int ci = corpusIndex(c.corpus);
            covTot[ci]++;
            try {
                parseOnce(c.sql, config);
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
            System.out.printf("wrote %d accepted ids to %s (Calcite %s, lex=%s conformance=%s)%n",
                accepted.size(), emitPath, version, lexName, confName);
            return;
        }

        // Subset = requested (the both-accept manifest) intersected with our accepts.
        Set<String> requested = null;
        List<String> missing = new ArrayList<>();
        List<String> measuredSql = new ArrayList<>();
        // Index candidates by id for fast subset realization.
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
        // compile the hot parse path before any measurement.
        long warmEnd = System.nanoTime() + (long) (warmupSecs * 1e9);
        long sink = 0;
        int warmPasses = 0;
        do {
            for (String sql : measuredSql) sink += parseOnce(sql, config);
            warmPasses++;
        } while (System.nanoTime() < warmEnd);

        // Calibrate inner-loop passes so each timed measurement spans >= minPassSecs.
        long t0 = System.nanoTime();
        for (String sql : measuredSql) sink += parseOnce(sql, config);
        double onePass = (System.nanoTime() - t0) / 1e9;
        int passes = onePass <= 0 ? 1024 : Math.max(1, (int) (minPassSecs / onePass) + 1);

        double best = 0, sum = 0;
        double[] rates = new double[reps];
        for (int r = 0; r < reps; r++) {
            long s0 = System.nanoTime();
            for (int p = 0; p < passes; p++) {
                for (String sql : measuredSql) sink += parseOnce(sql, config);
            }
            double dt = (System.nanoTime() - s0) / 1e9;
            double rate = (double) passes * measuredSql.size() / dt;
            rates[r] = rate;
            best = Math.max(best, rate);
            sum += rate;
        }
        Arrays.sort(rates);
        double median = rates[reps / 2];

        int totCand = 0, totAcc = 0;
        for (int i = 0; i < CORPORA.length; i++) { totCand += covTot[i]; totAcc += covAcc[i]; }

        System.out.println("# cross-language throughput: Apache Calcite");
        System.out.printf("#   runtime         : JVM %s  (startup+JIT excluded via warm-up; single-thread)%n",
            System.getProperty("java.version"));
        System.out.printf("#   tool version    : calcite-core %s%n", version);
        System.out.println("#   parse unit      : SqlParser.parseStmt() -> SqlNode AST (NO validate, NO plan)");
        System.out.printf("#   dialect (approx): lex=%s conformance=%s  (no Postgres lex exists in Calcite)%n",
            lexName, confName);
        System.out.printf("#   corpus root     : %s%n", corpusRoot);
        System.out.println("#   metric          : parses/sec = statements / wall_seconds (warm, 1 thread, END-TO-END)");
        System.out.printf("#   method          : JMH-lite; warm-up >= %.3gs (%d passes), %d timed passes x %d inner loops (>= %.3gs each)%n",
            warmupSecs, warmPasses, reps, passes, minPassSecs);
        if (subsetPath != null) {
            System.out.printf("#   subset          : %s  (%d requested ids)%n", subsetPath, requested.size());
            if (!missing.isEmpty()) {
                System.out.printf("#   WARNING         : %d requested id(s) NOT accepted by Calcite -> excluded "
                    + "(subset/version drift; regenerate the intersection)%n", missing.size());
            }
        } else {
            System.out.println("#   subset          : SELF-COVERAGE (this tool's own accept set)");
            System.out.println("#   WARNING         : self-coverage is NOT the comparable both-accept subset;");
            System.out.println("#                     pass --subset both_accept.txt for a fair cross-tool number.");
        }
        System.out.println("#");
        System.out.println("# coverage (Calcite accepts / candidates), per corpus:");
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

    // Best-effort peak RSS via Linux /proc; never a per-parse figure. The JVM's RSS
    // includes the whole runtime (heap + metaspace + JIT code cache + GC), so it is
    // not comparable to Rust dhat numbers — caveat baked into the string.
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
