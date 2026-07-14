# npm distribution

Squonk publishes seven focused scoped packages and one batteries-included umbrella at the same workspace version:

- `@squonk-sql/ansi`
- `@squonk-sql/postgres`
- `@squonk-sql/mysql`
- `@squonk-sql/sqlite`
- `@squonk-sql/duckdb`
- `@squonk-sql/quiltdb`
- `@squonk-sql/lenient`
- `squonk`

The focused packages contain ANSI plus their named dialect and default to the named dialect. `squonk` defaults to ANSI and contains all 14 built-in presets. There is no `@squonk-sql/all` package and no public `full` or `lite` build mode.

## Runtime contract

Bare package imports use ordered conditional exports. Node 22+ and Bun prefer an ABI-stable Node-API addon and synchronously fall back to colocated WASM when the platform package is absent or addons are disabled. Deno imports wasm-bindgen's module-target output without read or FFI permission. Wrangler/workerd and edge-light bundlers receive static WebAssembly modules. Consumers call `parse()` directly; `init()` and `defaultWasmUrl` are not public.

Browser consumers import `<package>/browser` and await `createSquonk()`. The optional `{ wasm }` input supports custom URLs, responses, bytes, promises, and precompiled modules. Concurrent calls coalesce, a failed load can be retried, and the first successful source owns the package runtime.

Every package includes parsing, recovery, typed AST views, mutation-aware rendering, formatting, tokenization, redaction, and transpilation.

## Build and verification

The private `crates/squonk-wasm/package.json` is build tooling, never a publish artifact. One descriptor table drives the eight Rust feature sets, TypeScript entrypoints, staging paths, smoke tests, budgets, and release order.

```sh
cd crates/squonk-wasm
npm install --ignore-scripts --no-package-lock
npm run build
npm run typecheck
npm run smoke:variants
npm run size:check:tarball
npm run pack:check
npm run smoke:install
npm run smoke:runtimes
npm run smoke:workerd
```

`npm run build` stages exact facade and platform-package trees in `dist/npm/<label>`. Facades contain browser/edge WASM, Deno module-target WASM, conditional runtime entrypoints, the shared typed runtime, generated AST declarations, README, license, and manifest. Eight script-free optional platform packages contain one Node-API addon each; there is no install-time compilation or download.

Size ceilings are per facade package. Raw and gzip browser-WASM sizes plus packed and unpacked tarball sizes have 10% measured headroom; the 35-entry facade inventory is exact.

## Release

Every facade and platform package must trust the `moderately-ai/squonk` GitHub repository,
`release-npm.yml` workflow, and protected `npm` environment. Publishing uses npm OIDC after
each package's one-time registry bootstrap; no install or runtime token ships in an artifact.

The release workflow builds each WebAssembly facade in its own one-worker matrix job, then assembles and verifies every package before uploading one immutable artifact containing all staged trees. WASM jobs share registry and pinned-tool caches without duplicating feature-specific target trees; native jobs use target-specific dependency caches. The protected publish job enables publishing only in those ephemeral manifests, repeats every dry-run, publishes platform packages first, then focused facades and `squonk` last.

Publishing is resumable. If an exact version already exists, its registry integrity must match the verified local tarball or the job stops. npm attaches signed provenance to every new package version.

No local implementation or rehearsal command publishes externally. A real workflow publish requires an explicit dispatch choice and protected-environment approval.

## Post-publish smoke

Install at least one focused package and the umbrella in clean Node 22 and Node 24 projects, test ESM and CommonJS, repeat under `--no-addons`, and exercise parse, format, recovery, dialect metadata, and `runtimeInfo()`. Packed-install gates also execute Bun, permissionless Deno, and a live Wrangler/workerd request. Run the browser example against packed or published artifacts and verify default loading plus a custom WASM URL.
