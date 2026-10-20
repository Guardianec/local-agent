# Development history

## 0.4.0 browser operator — 2026-10-19–20

Inspected and ran the existing 0.3.2 app before extending its helper. Baseline frontend suite passed 13 tests; two existing browser integrations passed, with external HTTPS intentionally ignored. The disposable desktop smoke passed exact-byte Apply/Undo, command approval/failure evidence, deterministic search and sensitive-scope handoff.

Added a task-lived browser protocol over the existing nonce-authenticated named pipe and Windows Job executor, semantic references, stale recovery, tabs, diagnostics, screenshot ingestion, projectless browser workspaces, static retrieval, origin grants, reviewed static forms and development-server ownership. Existing Rust authority, private-copy review and model escalation remain in place; see decision 006.

The final native pass completed 108 unit tests and eight local integration tests; the separately enabled external HTTPS test also passed. Fourteen frontend tests, strict Clippy and production frontend build passed. Native desktop QA retained exact-byte Apply/Undo, approval/failure evidence, search and handoff. Browser-only workspace and completed evidence UI checks passed. Canonical-field validation fixed extra-field acceptance on Serde tagged unit actions. Password observation/masking, WebRTC/WebTransport blocking, static-form checkbox safety and sensitive-query rejection gained regression coverage.

Real Fast completed static public text, Ukrainian browser reading and screenshot/canvas interpretation. Final Balanced reproduced HTTP 500, edited only the intended source, passed the fixture test and verified the repaired page in 51.06 seconds with 13 tools. Initial model failures informed compact schemas, same-session navigation and selective initial visual capture. Strong passed an 8K coding calibration, but browser acceptance remained blocked by the unchanged available-RAM gate; no Strong browser success is claimed. [QA 0.4.0](QA_0.4.0.md) distinguishes real models from mock-inference escalation coverage and records deferrals.

Acceptance inspection found browser HOME caches surviving process termination. Added confined cleanup after process exit and during interrupted-task reconciliation, preserving encrypted evidence and refusing junctions. Existing completed caches are not retroactively deleted. Reviewed an actual synthetic desktop screenshot for the README; no private task records or logs are included. All application manifests and lockfiles align at 0.4.0; helper version remains independent. Publication is gated on the clean committed production installer and exact downloaded checksum, not the debug QA build.

Headroom was attempted; its configured local proxy was unreachable. Focused reads were used. No automatic traffic compression is claimed.

This journal records engineering milestones, not a fictional commit history. The source checkout had no commits or remote before public-release preparation. Earlier versions are reconstructed from the architecture handoff, implementation decisions, acceptance notes and calibration records. Dates are given only where supported. The initial public Git commit imports 0.3.0; old tags/releases are not fabricated.

## Architecture and 0.1.0 — documented 2026-10-16

**Goal:** make bounded local coding work accessible through a Windows GUI, with permissions, review and recovery.

Tauri 2 provides the desktop shell, React/TypeScript the interface and Rust the authority boundary. Typed tools separate model requests from policy and execution. Private working copies keep agent edits away from originals until explicit Apply. SQLite WAL/FULL journals and encrypted content-addressed blobs support exact-byte Undo and conflict-aware recovery, including dirty Git projects.

Project discovery, deterministic search/check recipes, a bounded agent loop, diff/check views, activity and offline handoff were implemented. Optional browser automation became typed Playwright scenarios: one approved origin, bounded steps, fresh profiles and an authenticated Windows named pipe. Rust brokers requests and checks DNS/IP boundaries.

**Failures/fixes:** actual Windows tests showed npm loses its working folder with extended path syntax. Confined file paths stay extended/pinned, while validated child folders use ordinary drive paths. Personal Chrome rejected the stripped automation environment; matching Chromium is used without personal profiles. Browser tests assert zero forbidden HTTP bytes rather than claiming no speculative sockets.

**Safety/decisions:** fixed recipes, concrete host-code consent, Windows Job Objects, handle confinement, protected secrets, review-hash binding and durable recovery. Root-journal identification made interrupted Undo/recovery repeatable. Startup reconciles a crash after file commit but before task-state persistence. An exclusive data-folder lock prevents competing controllers.

**Limits then:** Ollama was initially absent, so protocol fixtures were not called model benchmarks. Host code was not sandboxed; no automatic pruning, unsigned installer and Windows-first scope were explicit.

Evidence: [architecture baseline](../HANDOFF.md), [decision 001](decisions/001-implementation-details.md), [acceptance](ACCEPTANCE.md).

## 0.2.0 — profiles and real model evidence, 2026-10-16

**Goal:** turn nominal integration into a tested three-tier workflow on a 12 GB GPU.

Fast/Balanced/Strong became configurable assignments with deterministic initial routing and explicit Strong gating. Calibration moved beyond synthetic compliance: an isolated addition task requires actual reading/editing and positive/negative/zero Node tests, immutable tests/config, final-manifest validation and unchanged originals. Historical Qwen3 4B Instruct Q4, 8B and 14B passed at 8K/full GPU on the reference PC. These are not today's defaults.

**Failures/fixes:** Ollama exposed unresolved nested JSON Schema references and empty object schemas lacking properties. The provider now flattens schemas while Rust strictly parses arguments. A random tool-result proof tests observation consumption. Thinking-only 4B exhausted the response budget; the explicit Instruct variant passed. Reasoning controls are used only when supported.

**Safety/decisions:** reports retain real evidence rather than trusting completion text. Digest/runtime/context identity prevents a different model inheriting readiness. All models share one policy harness.

**Limits then:** Strong initially required full GPU residency; continuation was narrower. Small fixtures were not a general quality ranking.

Evidence: [decision 002](decisions/002-model-profiles.md), [changelog](../CHANGELOG.md).

## 0.2.1 — packaged-app verification (exact date not independently retained)

**Goal:** verify embedded production assets/native IPC rather than only a development server.

The existing record describes persisted assignments/reports, Strong consent/rejection, automatic Fast inference, GUI calibration and cancellation. A real Balanced task exercised edit/command approvals, checks, Apply and exact Undo. Minimum-size, dark/light and rendering-scale checks covered profile controls. The installed executable was compared read-only against the build, accounting for Tauri's expected installer marker.

**Safety/fixes:** recovery/restart cases retain proposal/check evidence and refuse newer-file conflicts. The exact split of recovery work between 0.1 and 0.2.1 is not reconstructable from Git; this journal does not invent it.

**Limits then:** unsigned installer, shared Ollama ownership and no exhaustive crash/adversarial campaign. Exact older benchmark data is historical, not current recommendations.

Evidence: historical installed checks retained in [acceptance](ACCEPTANCE.md), [decisions](../DECISIONS.md).

## 0.3.0 — multimodal continuation, 2026-10-17–18

**Goal:** use three contemporary tiers effectively, continue investigations and support screenshots without weakening policy.

Physical models moved into one registry: Qwen3.5 4B Q8 (16K), Qwen3.5 9B Q4 (32K), Qwen3.6 35B-A3B Q4 (32K). Explicit downloads gained progress/cancellation and quantization checks. Matching install receipts establish cleanup ownership; ambiguous/unrelated models remain protected.

Sequential lifecycle control replaced Strong's full-GPU gate. Conservative memory preflight and sustained-RAM-pressure detection retain recoverable evidence. Foreign residency is preserved, even for the same tag. Deterministic failure/progress signals trigger compact continuation containing relevant tools, findings, checks, diff, image references, constraints and counters. Switching preserves task/snapshot/permissions/global budgets. Exhaustion prepares an accumulated Codex handoff.

Images gained byte-level validation/normalization, picker/paste/drop/previews/removal, opaque references and DPAPI blobs. Ollama gets transient native image payloads over loopback. Pixels are omitted from handoff export with an explicit notice.

**Failures/fixes:** a real rename attempt copied displayed line numbers during model reconstruction. It failed acceptance without changing originals. Approved deterministic `rename_file` now preserves exact bytes and refuses changed source/collisions/escapes; the repeat passed actual Apply/Undo. Stale checks retain their original outcome. Model notes remain unverified; completion claims use recorded evidence. Closing setup cancels downloads; late ingestion cannot target a different draft.

**Actual evidence:** all defaults passed isolated coding checks on RTX 4070 Super/i5-12400F/32 GB RAM, Ollama 0.40.5. Strong passed partial GPU/RAM placement at about 9.05 GiB GPU allocation, 7.21 GiB minimum free RAM. Actual vision read screenshot-only text and found overlap; Balanced image-guided CSS passed checks/review/Apply/Undo. An intentionally unavailable dependency fixture verified real Balanced → Strong → Codex continuation in 79 seconds, retaining two failed checks; it did not pretend to fix an impossible dependency.

**Release work:** first public source import, MIT license, polished documentation, source/artifact audits, stronger ignores and compiler path remapping. Application version stays 0.3.0; release preparation does not invent a functional version. Release notes bind the installer checksum to its source commit.

The first private CI run caught scanner checksum parsing that depended on HTTP content type and trailing whitespace. CI now downloads the checksum as a file and parses trimmed text; checksum verification is retained rather than bypassed.

Private release rehearsal also exposed GitHub's normalization of spaces in uploaded asset names. The release pipeline now exports a hyphenated installer filename before creating SHA256SUMS, so published download names and checksum entries agree.

Independent Windows CI exposed protocol fixtures depending on real host RAM and a Node argument fixture using an empty environment. A compile-time test-only memory fixture now drives protocol observations, and the Node test uses the production stripped environment. Real runtime memory gates and execution consent are unchanged; test isolation does not simulate model quality.

The next CI run passed 87 of 88 native tests; the remaining real-command fixture exhausted a 20-second test deadline during an approval/check sequence. Its wait is now bounded at 60 seconds and CI limits parallel tests on the shared runner. Semantic assertions and production budgets remain intact.

**Limits now:** Windows x64, unsigned installer, slow Strong cold loading, host-code consent rather than sandboxing, best-effort redaction, no automatic data pruning. Targeted tests/fixtures do not constitute exhaustive quality, crash, adversarial or accessibility testing.

Evidence: [decision 003](decisions/003-multimodal-continuation.md), [model measurements](MODEL_CALIBRATION.md), [acceptance](ACCEPTANCE.md), [release audit](RELEASE_AUDIT.md).

## Documentation rendering correction - 2026-10-18

Reviewing the public GitHub page exposed Windows code-page corruption in punctuation written during release preparation. Known mojibake sequences were normalized back to UTF-8 and the rendered page was checked again. This is a documentation-only correction on main: application behavior, version 0.3.0, its tested source tag and installer remain unchanged.

## Future entries

Record version/date, motivation, changed architecture/features, bugs/failures fixed, runtime/model changes, safety effects, real validation and known limits. Link decisions and release notes. Distinguish measurements from plans. Never manufacture history or benchmark results.

## 0.3.1 conversational stabilization — 2026-10-18 (candidate)

Reproduced answer loss in the unchanged controller: a valid Fenwick explanation became a zero-file/zero-check summary. The audit also found global coding/tool pressure, accepted empty output, missing prior-answer context, and interrupted streams outside the recovery branch. Implemented intent-specific prompts/capabilities with native enforcement, compact follow-up context, explicit lifecycle diagnostics, bounded response recovery, structured terminal outcomes and frontend fallback/diagnostics. Preserved existing models, permission/host-code consent, snapshot/Apply/Undo and offline handoff boundaries.

Initial frontend baseline: 10 tests passed. Initial native sandbox run failed Windows DPAPI/loopback/filesystem fixtures; rerunning with normal host access passed 88 unit and 6 integration tests (one external-network test intentionally ignored). The answer-loss regression failed before implementation and passed afterward. Intermediate updated suites passed 95 native unit tests and 13 frontend tests; final verification is recorded separately below. Headroom's configured compression proxy was unreachable; targeted reads were used, with no claim of automatic traffic compression.

Real local acceptance compared direct Fast Ollama output (4,069 characters for the Fenwick question) with the corrected harness. Balanced answered Fenwick/architecture/MCP questions with zero tools; inspection requests returned visible reports, the diagnostic test request ran one failing check without entering a fix loop, and the fix request used four tools and one check. These synthetic observations are not general model-quality measurements. Acceptance source/data are isolated under ignored local test storage; no personal task content is committed.

See [decision 005](decisions/005-conversational-outcomes.md) for architecture and retained limitations. No future roadmap capabilities or cloud dispatch were added. Publication is a separate authorized step.

Native UI smoke verified conversational output, diagnostic reports, Retry, Copy diagnostics and Auto/Ask controls in the actual WebView. A real vision execution run exposed a routing regression: the scoped constraint "Do not change tests, HTML or package.json" was interpreted as a blanket no-edit request. The model consequently repeated reads with no patch capability. Intent inference now distinguishes scoped exclusions from blanket prohibitions, with a regression case. The acceptance helper now copies verified tier records and uses disposable Strong settings; production calibration/consent gates remain unchanged. Repeated unchanged observations also receive explicit bounded recovery feedback before escalation.

After the scoped-constraint fix, real Balanced vision execution changed only styles.css and passed its recorded check (16.56 seconds). The same screenshot asked as a question returned the expected image-only error code and explained the overlap (6.30 seconds). The synthetic originals remained untouched. These are single-fixture observations, not latency guarantees or exhaustive visual understanding tests.

Candidate verification before packaging: 97 Rust unit tests plus six integration tests passed; one external-HTTPS browser test remained intentionally ignored. All 13 frontend tests passed. Strict Clippy, formatting, production frontend compilation, source/version/link privacy checks and Gitleaks history scanning passed. Vite retained the existing large Monaco chunk warning. The release script repeats its required checks against the clean candidate commit; installer provenance/checksum stay in ignored release artifacts. No GitHub publication is implied by this candidate entry.

Final review replaced a mutex-based terminal-publication guard with explicit terminal publication points. The guard could deadlock while continuing a legacy proposal whose finish timestamp was absent. A bounded continuation regression now covers that case. Terminal assembly also converts an unexpected still-active outcome into an explicit failure instead of leaving a spinner running. The first packaging run was stopped before delivery to include this correction.

## 0.3.2 stabilization QA — 2026-10-18–19

**Goal:** challenge the existing interaction and model workflow with real local models and disposable projects before the next user-facing release. No roadmap capability or model identity changed.

Real Fast, Balanced and Strong requests covered conversation, named-file inspection, scoped code edits, a coupon/shipping bug with two initially failing tests, Ukrainian prompts, hostile text inside a repository, vision questions and image-guided CSS work. Balanced fixed the shipping bug without escalation; Strong independently passed it at the default 32K context. Fast completed a simple code fix. A live inference cancellation ended cleanly. The actual desktop smoke exercised exact-byte Apply/Undo, approved failing-check evidence, deterministic search and sensitive handoff routing. Twenty mock-provider Inspect tasks alternated synthetic projects without context crossing. A deliberately unavailable dependency reached Balanced → Strong → offline Codex with two retained failed checks and no fabricated success.

**Defects fixed:** a conceptual-question cue overrode an explicit file question; broad matching of “no edits” wrongly turned scoped constraints into read-only work; model setup failures were reported as unknown internal errors; punctuation splitting corrupted filenames in final explanations; and two desktop smoke assertions expected an obsolete Inspect status. Regression coverage now protects intent precedence, structured failure categories, fabricated-check suppression, explanation fidelity and project context isolation. Recorded changed paths/check counts lead final execution results; model prose is bounded, redacted and labeled unverified.

**Acceptance setup:** the QA runner uses private copies and disposable app stores. Its first Strong attempt inherited a disabled preference. Later attempts met the existing RAM preflight during transient memory pressure; a 16K calibrated run and a subsequent default 32K run passed once memory was free. The continuation example now obtains its own disposable Strong calibration and Balanced compatibility probe before task work. These were test setup corrections, not relaxed production gates. The one observed 16K Strong run used mixed CPU/GPU placement. The model was unloaded afterward.

A fresh compatibility check also exposed a real false negative for Balanced: with the exact app prompt, direct Ollama sometimes invented arguments for the synthetic tool's empty-object schema. A required enum argument produced the exact direct call. The native probe now uses that ordinary schema, still rejects extra keys, allows only one bounded repair and requires the random tool-result proof. Mock and real-model repeats passed; project tool schemas and policy remain strict.

**Limits:** one synthetic shipping bug does not establish broad model reliability or any task that genuinely needs Strong. The unavailable-dependency handoff tests evidence preservation, not a cloud repair. No 50-task real-model soak, exhaustive large-repository retrieval, every cancellation window, catastrophic RAM failure, multi-image campaign or full accessibility audit was completed. One read-only model answer inferred a test expectation without running that test; this was recorded as a model-quality risk. See [the QA report](QA_0.3.2.md) for exact scenarios and measurements.
