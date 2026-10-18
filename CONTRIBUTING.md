# Contributing

Keep changes focused and preserve native policy boundaries. Read [README](README.md), [architecture](ARCHITECTURE.md) and [decisions](DECISIONS.md). Vulnerabilities belong in the private channel described in [SECURITY.md](SECURITY.md).

## Windows setup

Use Node 20.19+ (22 tested), Rust MSVC (1.99.1 tested), Visual Studio C++ desktop tools/Windows SDK and WebView2. Keep npm/Cargo lockfiles.

```powershell
npm ci
npm ci --prefix browser-helper
npm run desktop
```

Ollama/model weights are separate and unnecessary for deterministic protocol tests. Never commit weights, private screenshots/task exports, local data or credentials.

## Check a change

```powershell
npm test
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
npm run build
```

Install matching test Chromium before the two local browser integrations:

```powershell
$env:PLAYWRIGHT_BROWSERS_PATH = Join-Path $PWD '.local-data/browsers'
node browser-helper/node_modules/playwright-core/cli.js install chromium --no-shell
```

The external-HTTPS test is intentionally ignored by default. Native tests require normal Windows DPAPI, temporary-file, loopback and process/Job access. Restricted-runner permission errors are not passing security evidence. Real-model calibration is separate from protocol fixtures.

UI scripts use a temporary WebView endpoint, a debug build and an isolated LOCAL_AGENT_TEST_DATA folder under .local-data/. seed_fixture creates explicitly synthetic proposals. Inspect each script's fixture assumptions; never run it against personal production history. Close the test app and clear data/debug overrides afterward. Do not expose a normal user's debugging port.

## Required for meaningful updates

1. Decide whether a version bump is warranted. Keep package.json, package-lock.json, src-tauri/Cargo.toml, src-tauri/Cargo.lock and src-tauri/tauri.conf.json aligned. The helper package has an independent version.
2. Update root CHANGELOG.md and docs/DEVELOPMENT_HISTORY.md with actual work, decisions, fixes, runtime/model changes, safety impact, checks and limits. Keep README feature status accurate.
3. Record significant architecture decisions in docs/decisions/ and DECISIONS.md; update ARCHITECTURE.md when boundaries change.
4. Add focused regression coverage for dangerous/nontrivial changes. Run checks/build before release. Never weaken policy, recovery or budgets to make a model pass.
5. Audit the exact Git tree/history/assets. Never commit secrets, .env, personal paths, local logs, weights or task evidence. Use Gitleaks and the repository privacy checker. Review false positives narrowly.
6. For every authorized user-facing version, create/update a GitHub Release and attach the latest tested production Windows installer plus checksum. A source push or debug executable is not a release.

Documentation-only fixes usually need no version bump. Shared release tags/installers are immutable: use a new appropriate version for fixes, not a silent replacement.

## Production release

1. Finish docs/tests and install matching test Chromium. After committing, run Gitleaks on the proposed history: `gitleaks git --redact --no-banner`. Do not include secrets in logs/notes. Use a public GitHub no-reply author email if desired.
2. Commit all intended source. The release script refuses dirty/untracked files.
3. Run `powershell -ExecutionPolicy Bypass -File scripts/build-release.ps1`. It validates privacy/versions, runs checks, remaps build paths, invokes Tauri/NSIS and writes ignored release-artifacts/v<version>/ with installer, SHA256SUMS and local provenance manifest. Fix/commit failures before rebuilding.
4. Install/test that exact installer, using disposable data or a backup. Unattended upgrades use `/S /UPDATE` to make update semantics explicit. Verify launch/version/setup/retained data. Review any public screenshots for personal content/metadata.
5. Tag the tested commit v<version>, push branch/tag, then create a draft GitHub Release. Include highlights, fixes, limitations, install notes, source commit and checksum. Attach the exact installer and SHA256SUMS; inspect before publishing. Do not attach private logs/debug binaries/local manifests.
6. Verify visibility, default branch, license, rendered README/links, tag target and downloaded asset checksum. Record real results; do not overclaim signing or exhaustive testing.

Contributions are under the repository MIT license. Dependencies/models retain separate licenses.
