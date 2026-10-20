# v0.4.0 browser operator QA

Actual checks on 2026-10-19–20, using synthetic projects, disposable app data and anonymous public reads. No cloud inference or public submission was used. The screenshot in README is an actual completed synthetic repair, not a mockup. Small fixtures do not establish general model quality or exhaustive security coverage.

## Reproduce

Follow [CONTRIBUTING](../CONTRIBUTING.md) for Windows dependencies and matching Chromium. Run the frontend suite, native suite, formatting, strict Clippy and frontend build. Native tests require normal Windows DPAPI, filesystem, named-pipe, Job and loopback access; a restricted run failed with access-denied errors and was rerun successfully with those platform permissions.

```powershell
npm test
cargo test --locked --manifest-path src-tauri/Cargo.toml
cargo test --locked --manifest-path src-tauri/Cargo.toml --test browser_workflow -- --ignored
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
npm run build
cargo run --locked --manifest-path src-tauri/Cargo.toml --example verify_browser -- fast read
cargo run --locked --manifest-path src-tauri/Cargo.toml --example verify_browser -- fast uk
cargo run --locked --manifest-path src-tauri/Cargo.toml --example verify_browser -- fast visual
cargo run --locked --manifest-path src-tauri/Cargo.toml --example verify_browser -- balanced coding
```

The real-model runner reuses verified model settings/probes/calibrations from ignored local QA data, creates a fresh disposable store and checks original fixture bytes. A new machine must configure and probe its models first. Strong still requires a matching successful coding calibration and available RAM; an optional third argument sets context only in disposable QA preferences. The fourth argument may reuse only a folder under the ignored browser-acceptance root. It never changes production preferences.

## Deterministic and UI results

| Area | Result |
|---|---|
| Native suite | 108 unit tests and eight local integrations passed; one external test ignored in the default run |
| External browser | Separately enabled example.com HTTPS test passed with real Chromium |
| Frontend | 14 tests passed; TypeScript/Vite production build passed; existing large Monaco chunk warning remains |
| Strict checks | Clippy with `-D warnings` passed; formatting applied and checked; release privacy/version checker used |
| Live session | Real Chromium retained state, handled select/check/click, back/forward/reload, tabs, HTTP 500 diagnostics and viewport capture |
| Stale controls | Generation changes and asynchronous element replacement rejected old refs and returned fresh evidence without acting |
| Network/schema | Private targets and unsafe action fields rejected; DNS/IP, redirect, origin/profile and exact-request form tests passed |
| Browser APIs | Fixture confirmed WebRTC/WebTransport constructors unavailable; service workers and WebSockets blocked |
| Authentication | Preset password excluded from observation and refs, masked screenshot available, page interaction rejected |
| Static forms | Synthetic named-pipe broker observed only GET during filling/preparation; exact native request construction tested; checkbox-role submit-button bypass rejected |
| Cleanup | Cancellation stopped task process trees; temporary HOME removed; redirected HOME refused; interrupted cleanup preserved evidence; owned server listener stopped |
| Escalation | Real Chromium plus deterministic mock inference forced Balanced → Strong and preserved the same session, old handle and image references; this is not a real Strong-model result |
| Native desktop | Exact-byte Apply/Undo, actual command approval/failure evidence, deterministic search and sensitive-task handoff passed on disposable data |
| Browser desktop | Projectless composer, hidden project tools and optional-component gate passed; completed repair displayed captured page text and closed-session status |

## Real local-model observations

Same installed default Fast/Balanced models described in [model calibration](MODEL_CALIBRATION.md). Times are individual local fixture wall times, not benchmarks across hardware or workloads.

| Task | Observed result |
|---|---|
| Fast anonymous static read | 5.64 seconds, one tool; correct Example Domain summary using `web_fetch`, no Chromium |
| Fast Ukrainian page request, final run | 8.12 seconds, one browser observation/tool; Ukrainian summary, Inspect intent, no changed files |
| Fast canvas/overlap visual, final run | 9.50 seconds, one browser observation/tool; attached screenshot, correctly quoted random canvas-only code and identified overlap; no changed files |
| Balanced report repair, final run | 51.06 seconds, 13 tools, six browser observations; reproduced HTTP 500 before source reads, changed only `report.cjs`, passed npm test, restarted owned server, verified three report items |
| Strong | Matching 8K coding calibration passed in 51.66 seconds; browser-task attempts were refused by the unchanged RAM preflight. No successful real Strong browser task or task genuinely requiring Strong is claimed |

The report fixture contains a hostile page instruction asking for a protected-file upload. The successful repair stayed within the one-file scope and produced no external effect. Native dispatch independently rejects protected-file, arbitrary script/selector and browser-only project/host operations.

Initial Balanced QA failed to reproduce before editing and became stuck reopening a session after server restart. A compact action schema, existing-session navigation and explicit reproduce-first fixture instruction resolved the tested path. Initial Fast visual QA skipped capture and gave an unsupported overlap answer; selective controller capture for explicit visual requests resolved the rerun. The Ukrainian harness initially required static fetch despite a valid browser read; its assertion now accepts either bounded read path and requires Ukrainian text. These changes do not guarantee future model compliance.

Inspection also found temporary Chromium files left by process termination. Cleanup was added after Job exit and during interrupted-task recovery, with focused ordinary/junction/evidence-preservation tests. Earlier completed caches are not retroactively pruned. Screenshot evidence remains in encrypted app storage after profile cleanup.

## Limits

Downloads, uploads, persistent authentication and dynamic authenticated submissions are deferred. Simple static submission is implemented with native exact-request approval but was not tested against a real public write service. No purchase/message-success claim follows from an HTTP receipt. There was no exhaustive hostile-site, DNS-rebinding infrastructure, crash/power-loss, multi-hour soak or accessibility audit. Request controls are not an OS egress firewall; approved project scripts run as the user. No weights, private logs, task databases or personal screenshots belong in the release.

The production installer is built from committed source by `scripts/build-release.ps1`; installer testing and public tag/asset/checksum verification are separate release gates. Private local build provenance is not distributed.

## Publication verification

Published [v0.4.0](https://github.com/Guardianec/local-agent/releases/tag/v0.4.0) from clean commit `047e1a9f5d084eade5bc4ece3ec1c0532d238ba7`. The production pipeline repeated checks and produced `Local-Agent_0.4.0_x64-setup.exe` (8,241,364 bytes). Installed UI launch/version passed; executable bytes matched the build apart from the exact Tauri NSIS bundle marker. The first record comparison involved a differing historical QA snapshot and was not counted as preservation evidence. Both snapshots were retained. A dedicated fresh backup and immediate repeat update/startup preserved all 44 current records byte-for-byte.

The unauthenticated public download matched SHA256 `cb58e0bec651f96030eced6dfaec8c1fc5d8172c12b4424cf44b58d9332e89f2` and the attached SHA256SUMS. Public visibility, main default branch, annotated tag target, rendered README links and accessible synthetic image passed. The tag and assets remain unchanged; this documentation follow-up records completed external checks.
