# Acceptance evidence — Local Agent 0.3.0

Verification: 2026-10-17–18 on Windows x64, RTX 4070 Super 12 GB, i5-12400F, 32 GB RAM, Ollama 0.40.5, Node 22 and Rust MSVC. The original architecture is root `HANDOFF.md`; decision `decisions/003-multimodal-continuation.md` supersedes its model and continuation sections. Production exposes no simulated provider.

## Current update

| Capability | Evidence and boundary |
|---|---|
| Three exact local tiers | Fast `qwen3.5:4b-q8_0` at 16K; Balanced `qwen3.5:9b-q4_K_M` at 32K; Strong `qwen3.6:35b-a3b-q4_K_M` at 32K. All passed real isolated read/edit/Node-test calibration. Exact digests, timing and memory are in [MODEL_CALIBRATION.md](MODEL_CALIBRATION.md). |
| Central selection and migration | One physical registry, deterministic routing, custom-assignment preservation, additive history migration and matching digest/runtime/context evidence. Fast Q8 never silently becomes Q4. Strong is disabled on registry migration until current evidence is confirmed. |
| Model installation and cleanup | Native fixed-catalog downloads with bounded progress/cancellation and quantization verification. UI tests verify explicit size consent, stop on close, app-owned preselection and ambiguous entries unchecked. Exact receipt/digest ownership and unrelated-model exclusion are native-tested. Actual machine cleanup is recorded below. |
| Dynamic continuation | A real Balanced → Strong → Codex run passed in 79.01 s with one task/snapshot, two failed checks and four useful outcomes carried into Strong. An actual Strong response and accumulated handoff were verified. The deliberately unavailable external dependency remains failed; the app does not invent a fix. Unit fixtures cover disabled escalation, budgets, state, cached evidence and malformed tools. |
| Sequential model lifecycle | Live partial-GPU Strong passed. Native tests verify failed release/load handling, sustained versus transient low RAM and preservation of a preexisting foreign model, including the selected tag. No shared runtime process is killed. Catastrophic RAM exhaustion was not intentionally induced. |
| Local vision | Actual Fast inference read a random code visible only in a screenshot and identified overlap. Actual Balanced image-guided CSS edits passed approved Node checks, diff review, Apply and exact Undo in native WebView. No cloud upload occurred. |
| Multimodal controls | Picker, real paste/drop, thumbnails and removal exercised; UI tests cover input behavior. Native tests cover decoded-byte formats/limits, metadata normalization, persisted opaque IDs, continuation references and rejection before sending to a non-vision model. |
| Fast deterministic file actions | Three exact-byte renames, retained summary, Apply and Undo passed with real Fast inference in 15.69 s. The new rename tool preserves BOM/CRLF/final newline and rejects collision, changed source and path escapes. Original files stay untouched before Apply. |
| Offline Codex handoff | Real continued task exports original request, checks, relevant outcomes, stages and sanitized structured continuation. Images are explicitly omitted; the user must review and attach them separately. No cloud-send endpoint. JSON remains parseable after bounded privacy filtering; patches are complete or omitted. |

The first real three-rename attempt failed because the model reconstructed numbered file output incorrectly. It was not accepted. The deterministic rename tool fixed the underlying byte-preservation problem; the successful repeat included actual Apply/Undo. One synthetic Balanced compatibility call required a bounded correction. Calibration coding retries count only the coding task, not that preceding probe.

## Preserved safety and workflows

The existing project picker, deterministic search/checks, controlled tools, concrete permissions, independent task copies, diff review and durable Apply/Undo remain in place. Tests verify dirty Git staged/unstaged user changes and ignored-but-tracked source survive, while non-Git create/edit/delete round trips restore exact bytes. A newer external edit causes an Undo conflict and is preserved.

Every project-code recipe still requires approval describing host-account filesystem/network authority. It is not a security sandbox. Strict schemas reject unrestricted shell/encoded PowerShell; protected paths, credentials and confinement checks remain native authority. Prompt/image/repository content cannot grant permissions. A broad live adversarial-model campaign is not claimed.

The durable journal precedes original-project mutation. Targeted interruption tests verify recovery after writes, interrupted Undo, repeated recovery and startup after commit before UI-state persistence. Unknown/newer content produces an explicit conflict. This does not claim an exhaustive power-loss campaign or protection from failed storage. A per-data-folder instance lock prevents competing controllers.

Owned command cancellation and timeout use real Windows Job Objects; a descendant fixture confirms it started and verifies its delayed write never occurs. Approval cancellation ends without execution. Inference HTTP cancellation and task budgets are bounded; GPU unloading remains controlled through shared Ollama. Checks retain real exit/output and tested manifest hashes; later edits mark freshness stale without losing the original result. Agent notes are labeled unverified; final summaries derive from persisted evidence.

The existing optional browser helper is unchanged: real pinned Chromium tests cover a disposable local fill/click/assert scenario, cross-origin denial and zero HTTP bytes from a public profile to a private endpoint. It is not an OS egress firewall. No new browser/desktop operator, MCP authority or autonomous shell was added.

## Final automated verification

- 88 native unit/export tests passed; binding exports check contracts and are not 88 separate security scenarios.
- Four core workflow integration tests and two local browser integration tests passed. The external static-HTTPS test remains explicitly ignored in the default suite; it passed separately in the earlier baseline, and was not repeated for this model-only update.
- Ten frontend tests passed: stale task state, approval closure, calibration/profile controls, image inputs and download/cleanup consent.
- Strict Clippy across all targets passed. Rust formatting passed after formatting the latest acceptance fixture and residency regression test.
- TypeScript/Vite production build passed. The large lazy Monaco asset still produces a size advisory; the editor is loaded on demand.
- Real-model evidence: `verify_models`, `verify_vision`, `verify_fast_files` and `verify_continuation` examples; `multimodal-smoke.mjs` exercises native WebView and actual image-guided work. Ignored evidence lives under `.local-data/`; no private project files or reasoning traces are committed.

## Desktop release verification

Final 0.3.0 Tauri/NSIS packaging succeeded (7.61 MiB). The installed product/file/uninstall versions are 0.3.0. A read-only executable comparison matches the release build except for Tauri's expected `BUNDLE_TYPE_VAR_UNK` → `BUNDLE_TYPE_VAR_NSS` installer marker. Verification used embedded production assets and native IPC, not Vite: all three assigned tags, 16K/32K/32K contexts, Strong enabled, and three actual matching coding reports passed.

The initial upgrade verification found missing copied reports; setup was repeated with the app closed, then verified in the installed interface. A subsequent explicit `/UPDATE` install was checked against a SQLite backup and an ordered hash of every persisted record: all records were unchanged. This is targeted upgrade persistence evidence, not a claim covering every old installer path or historical corruption.

Actual native cleanup removed only the two known app installations, `qwen3:4b-instruct-2507-q4_K_M` and `qwen3:14b`, after confirming their exact original digests and showing both names in the installed confirmation dialog. Their logical sizes total approximately 11.8 GB; shared layers mean this is not a measured disk-space recovery figure. Pre-existing `qwen3:8b` and all three new models were preserved with unchanged digests. Ambiguous candidates were initially unchecked.

The installed interface passed minimum 1000×700 layout, model controls, focus trapping/Escape, dark/light and 150% rendering checks; screenshots were visually inspected. The original theme was restored. These are targeted flows rather than a full assistive-technology audit.

After the explicit update, the installed profile/report verification passed again. The QA process was closed and the installed app relaunched normally, responsive with the Local Agent window title and no temporary 9223 debugging or 1420 development-server listener. Ollama retains exactly the three configured new tags plus the preserved `qwen3:8b` on this machine.

The installer is unsigned. Setup, selection, image input, permissions, review, rollback, escalation and local model management are GUI driven. Ollama itself is installed separately; model downloads are explicit. Automatic history/image pruning, broad assistive-technology auditing, multi-hour stress tests and exhaustive Windows crash testing remain outside this acceptance claim.

## Repeat on another machine or changed runtime

1. Open Ollama and Local models; install a selected catalog tier with the displayed size/memory consent. Use Balanced for normal tasks.
2. Check compatibility and Test coding with explicit consent. Record exact digest/runtime/context and actual memory placement; a synthetic probe alone is insufficient for Strong.
3. Open a disposable copy of `fixtures/node-small`, ask to fix addition and run tests, approve only the expected actions, and inspect actual checks/diff.
4. Verify originals are unchanged before Apply, then Apply/Undo exact bytes. Edit an original after Apply and confirm Undo refuses the conflict.
5. Attach a new screenshot with text absent from the project; verify local interpretation and removal controls. Stop an active task and inspect retained evidence.
6. Exercise a failing disposable task through continuation and handoff. Confirm failed checks stay failed, prior evidence is retained, and image omission is visible before export.

See `decisions/001-implementation-details.md` and `003-multimodal-continuation.md` for explicit simplifications and hard boundaries. Actual target-hardware results qualify these model builds; they do not guarantee all hardware/model combinations.
