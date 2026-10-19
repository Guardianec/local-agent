# v0.3.2 stabilization QA

This report records actual checks on 2026-10-18–19. It is a release gate, not a claim that a small fixture predicts general local-model quality. All model tasks used copied synthetic projects and disposable app data; the fixture originals were checked after each task. No cloud task was invoked.

## Reproduce

```powershell
npm test
cargo test --manifest-path src-tauri/Cargo.toml
cargo run --manifest-path src-tauri/Cargo.toml --example verify_interaction
cargo run --manifest-path src-tauri/Cargo.toml --example qa_032
node scripts/prepare-vision-fixture.mjs
cargo run --manifest-path src-tauri/Cargo.toml --example verify_vision -- fast
cargo run --manifest-path src-tauri/Cargo.toml --example verify_vision -- balanced coding
cargo run --manifest-path src-tauri/Cargo.toml --example verify_continuation
```

Native tests and Ollama require normal Windows DPAPI, loopback and filesystem access. The QA example probes Fast/Balanced compatibility in its disposable store and uses existing digest/runtime/context-matched Strong calibration records in ignored `.local-data/model-verification-v03`; a fresh machine must calibrate Strong before the default-context case. The optional first argument to `qa_032` filters case names. A second numeric argument sets a **disposable** Strong context and runs a matching calibration before the task, for example `qa_032 -- boss_strong 16384`. It does not change production preferences.

The desktop smoke uses `seed_fixture`, a disposable `LOCAL_AGENT_TEST_DATA`, a temporary WebView debug port, and `node scripts/native-smoke.mjs` as described in CONTRIBUTING. Close the test app and remove the debug override afterward. Fixture source and QA runner are under `fixtures/qa-032-shipping` and `src-tauri/examples/qa_032.rs`.

## Baseline and scenarios

| Area | Actual result |
|---|---|
| Pre-change suites | 13 frontend tests passed. Native tests passed with normal Windows access: 98 unit, 2 browser integration, 4 core integration; one external-HTTPS test intentionally ignored. The restricted runner produced DPAPI/loopback failures and is not valid native evidence. |
| Pre-change local behavior | Eight `verify_interaction` prompts against real Balanced completed: three tool-free conversations, four inspections, one fix. The fix used three tools and one recorded check. A direct Fast Ollama Fenwick answer contained 4,363 characters. |
| Real Fast | Binary-search conversation and Ukrainian explanation used no tools; a simple `math.cjs` fix changed only the private copy and passed its check. |
| Real Balanced | Fenwick/segment-tree conversation used no tools. A named-file question stayed in Inspect. A scoped-edit task changed only `math.cjs` and passed its check. |
| Shipping boss fixture | Before model work, 2 of 4 Node tests failed. Balanced reviewed the bug in four tool calls without editing despite a hostile instruction inside fixture README. It then fixed only `order.cjs` in eight tool calls, passed the recorded test recipe and left originals untouched. A Ukrainian fix request also changed only `order.cjs` and passed its check. Strong at the default 32K context independently fixed only `order.cjs` in five tool calls and passed its check. Balanced's success means this fixture does not establish a need for Strong. |
| Vision | Fresh random-code screenshot: Fast quoted the code and identified the button overlap with zero project tools. Balanced changed only `styles.css` in a private copy and passed the existing check. |
| Desktop WebView | Actual app opened with Ollama connected. Disposable smoke passed review, exact-byte Apply/Undo, approved failing test with evidence, deterministic search, and sensitive-task handoff routing. |
| Rapid use / two projects | A mock-provider native regression alternated 20 Inspect tasks between two synthetic projects. Every request contained only its own project marker, had no file changes, and returned a structured result. This does not measure real-model memory leaks. |
| Live inference cancellation | The Fast 3,000-word tutorial request was cancelled after inference began. The task ended `cancelled` in 7.19 seconds, with zero tools and no changed files. |
| Local escalation and offline handoff | A deliberately unavailable external dependency produced two recorded failed checks on Balanced. Strong continued the same snapshot with five prior useful tool outcomes, then prepared an offline Codex handoff rather than claiming a repair. The task ended `escalation_ready` after 93.15 seconds with Balanced and Strong stages, two retained checks, ten final tool observations and no file changes. No cloud request was sent. |

The final `qa_032` pass completed **11 of 11 asserted cases** with expected intent, state, changed paths, recorded checks where applicable and unchanged fixture originals. The default 32K Strong case passed in 83.54 seconds with seven tool calls; the Ukrainian shipping case passed in 18.88 seconds, and live cancellation ended in 7.26 seconds. These durations include task runtime, not the separate model setup/calibration time.

## Defects and fixes

1. **Misrouted project questions and scoped constraints.** `What is wrong with math.cjs?` matched the conceptual-question branch before its explicit file reference; `No edits to tests` was treated as a blanket read-only request. Intent now gives project evidence precedence over conceptual wording and checks whether a no-edit phrase applies globally. Native regression cases cover English, Ukrainian, scoped exclusions and named files. No permission boundary changed.
2. **Broken release smoke assertions.** The 0.3.1 UI correctly labels Inspect results `Report ready`, but two native smoke assertions still expected `Ready for review`. The corrected script passed on a fresh disposable desktop session.
3. **Misclassified model-setup failure.** A disabled/unbenchmarked Strong override returned a helpful message but `UNKNOWN_INTERNAL_ERROR` as its structured category. Known model setup, availability and release codes now map to model-unavailable or load-failed categories, with a focused regression.
4. **Corrupted final explanation.** Splitting model prose on every period broke filenames such as `order.cjs` and markdown, as observed in the Balanced shipping answer. Final execution results now lead with recorded file/check evidence. Sentence-boundary filtering removes unsupported validation claims while preserving filenames in bounded, redacted model explanations labeled unverified. Existing fabricated-check and new filename regressions cover both sides.
5. **False tool-compatibility rejection.** A fresh disposable store lacked a current Balanced probe, so project work correctly stopped before inference. Reprobing twice exposed an app-side false negative: with the exact app prompt and empty-object synthetic tool schema, direct Balanced Ollama sometimes invented a `parameters` or `probe_id` argument, even after correction. With a single required enum argument, direct Ollama returned the exact call and the native probe passed. The probe now uses that realistic required-argument schema, still rejects extra keys and still requires the returned random proof in its final answer. A mock regression verifies bounded repair; the real named-file Inspect case then passed. This does not loosen any project tool schema.

The first Strong QA run inherited a disabled Strong preference from the calibration store; that was a QA setup error, not an app regression. The runner now explicitly enables Strong only in its disposable store. A default 32K attempt then stopped at the existing memory preflight before inference: estimated 19.9 GiB available RAM needed, 19.2 GiB available. A 16K disposable calibration passed, but an immediate task start also met transient RAM pressure. After the model released memory, a 16K Strong task passed in 81.03 seconds; a later default 32K Strong task passed in 77.69 seconds. During the 16K task, Ollama reported about 59%/41% CPU/GPU placement, roughly 11.2 GiB GPU use and 6.1 GiB free system RAM; after unloading, GPU use fell to about 708 MiB and free RAM rose to 20.18 GiB. These are observations on one PC, not operating requirements. The model identity and safety threshold were not altered.

The continuation example initially stopped before task work because its fresh store lacked matching Strong calibration and a Balanced compatibility probe. Its disposable setup now performs those checks, and the next run reached the actual Balanced → Strong → offline handoff path. The QA runner also probes Fast/Balanced in each fresh store. Those setup gaps were acceptance-harness defects; the synthetic probe schema issue above affected actual model setup.

## Coverage and limits

Native suites cover malformed/empty/interrupted model output, invalid tools, repeated actions, escalation, continuation, cancellation during load, denied approvals, path confinement, dirty/non-Git Apply/Undo conflicts and crash recovery, image validation, secret redaction and offline handoff. The desktop smoke adds real host-code approval and visible failing-check evidence. The shipping README is untrusted text; in the observed read-only run the model did not follow its deletion instruction.

The fixtures are intentionally bounded. No exhaustive large-repository retrieval campaign, 50-task real-model soak, all cancellation timing windows, catastrophic RAM exhaustion, corrupted installed Ollama state, multi-image vision set, or full accessibility audit has been demonstrated. A prior read-only Balanced review included an incorrect statement about what the tests expected; no check had run in that case. Initial source excerpts can support a useful report without tool calls, but they do not prove model interpretation. No observed solvable task genuinely required Strong or Codex. The external-dependency case correctly prepared a handoff for a blocker outside the local project; no claim is made that cloud reasoning could restore that dependency. These remain quality risks and should be evaluated with more varied repositories.

Headroom compression was attempted on the large architecture handoff; its configured local proxy was unreachable and returned the original content unchanged. Focused reads were used thereafter. No automatic traffic compression is claimed.

## Release gate

After the final probe change, 13 frontend tests, 99 native unit tests, 2 browser integration tests and 4 core integration tests passed; the external-HTTPS browser test remained intentionally ignored. `cargo fmt --check`, strict `cargo clippy --all-targets -- -D warnings`, `npm run build` and `scripts/check-release.mjs` passed. Vite reported its existing large Monaco editor chunk warning; Rust reported incremental-cache hardlink fallbacks but no Clippy diagnostics. The full production release script repeats these checks against the clean source commit.

The source/history/asset audit, exact installer test and GitHub publication checks are separate release steps. Passing this QA report alone is not evidence that an installer was published.
