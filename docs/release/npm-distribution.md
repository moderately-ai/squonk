# npm distribution

Squonk publishes six focused scoped packages and one batteries-included umbrella at the same workspace version:

- `@squonk/ansi`
- `@squonk/postgres`
- `@squonk/mysql`
- `@squonk/sqlite`
- `@squonk/duckdb`
- `@squonk/lenient`
- `squonk`

The focused packages contain ANSI plus their named dialect and default to the named dialect. `squonk` defaults to ANSI and contains all 13 built-in presets. There is no `@squonk/all` package and no public `full` or `lite` build mode.

## Runtime contract

Bare package imports target Node 22+ ESM and initialize their colocated artifact synchronously. Consumers call `parse()` directly; `init()` and `defaultWasmUrl` are not public.

Browser consumers import `<package>/browser` and await `createSquonk()`. The optional `{ wasm }` input supports custom URLs, responses, bytes, promises, and precompiled modules. Concurrent calls coalesce, a failed load can be retried, and the first successful source owns the package runtime.

Every package includes parsing, recovery, typed AST views, mutation-aware rendering, formatting, tokenization, redaction, and transpilation.

## Build and verification

The private `crates/squonk-wasm/package.json` is build tooling, never a publish artifact. One descriptor table drives the seven Rust feature sets, TypeScript entrypoints, staging paths, smoke tests, budgets, and release order.

```sh
cd crates/squonk-wasm
npm install --ignore-scripts --no-package-lock
npm run build
npm run typecheck
npm run smoke:variants
npm run size:check:tarball
npm run pack:check
npm run smoke:install
```

`npm run build` stages exact, dependency-free package trees in `dist/npm/<label>`. Each tree contains one WASM artifact, the matching Node and browser entrypoints, the shared typed runtime, generated AST declarations, README, license, and manifest.

Size ceilings are per package. Raw and gzip WASM sizes plus packed and unpacked tarball sizes have 10% measured headroom; the 18-entry package inventory is exact.

## Release

Before the first release:

1. Create or confirm control of the `@squonk` npm organization.
2. Confirm all seven names are still available.
3. Configure the protected GitHub `npm` environment.
4. Provide a short-lived granular bootstrap token permitted to create all seven packages.

The release workflow builds and verifies every package before uploading one immutable artifact containing all staged trees. The protected publish job enables publishing only in those ephemeral manifests, repeats every dry-run, then publishes focused packages first and `squonk` last.

Publishing is resumable. If an exact version already exists, its registry integrity must match the verified local tarball or the job stops. After the first release, configure npm Trusted Publishing for every package and delete the bootstrap token; later releases use OIDC provenance.

No local implementation or rehearsal command publishes externally. A real workflow publish requires an explicit dispatch choice and protected-environment approval.

## Post-publish smoke

Install at least one focused package and the umbrella in clean Node 22 and Node 24 projects, import without initialization, and exercise parse, format, recovery, and dialect metadata. Run the browser example against packed or published artifacts and verify default loading plus a custom WASM URL.
