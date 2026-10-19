# Changelog

Meaningful changes are grouped by application version, using Keep a Changelog-style sections. Earlier milestones are reconstructed from existing architecture/acceptance documents, not invented Git commits or tags. The first public source release is 0.3.0.

## [Unreleased]

## [0.3.2] - 2026-10-19

### Fixed

- Route explicit named-file questions to Inspect and preserve execution requests with scoped no-edit constraints, including Ukrainian wording.
- Preserve filenames and model explanations in final results while placing recorded changed files and checks first. Model prose is labeled unverified.
- Classify unavailable/unbenchmarked model setup and release failures in structured task outcomes.
- Use a required synthetic argument for strict tool compatibility checks, avoiding false rejection of the installed Balanced model on an empty-object probe.
- Refresh the desktop smoke script's Inspect status assertions.
- Carry forward the post-0.3.0 UTF-8 documentation punctuation correction; it did not change the 0.3.0 installer.

### Added

- Reproducible adversarial local-model QA cases on disposable projects, including hostile repository text, English/Ukrainian prompts, live cancellation and Strong comparison.
- A 20-task two-project context-isolation regression and updated Balanced → Strong → offline Codex continuation acceptance.

### Verified

- Real Fast, Balanced and Strong model work, image-guided coding, desktop Apply/Undo, host-code approval, and retained failed-check handoff. See [the QA report](docs/QA_0.3.2.md) for exact results and limits.

## [0.3.1] - Unreleased stabilization candidate

### Fixed

- Preserve conversational answers instead of replacing them with zero-file/check reports; remove global tool pressure.
- Separate Converse/Inspect/Execute intent, with contextual follow-ups, explicit read-only precedence and bounded classification for ambiguous wording.
- Enforce intent at native tool dispatch and Apply; diagnostics retain host-code consent and private-copy confinement for agent file operations.
- Recover once from empty, interrupted or invalid model responses before bounded escalation. Failed diagnostic tests remain findings rather than unsolicited fix requests.
- Publish structured terminal results after cleanup, acknowledge cancellation, recover interrupted tasks and render a frontend fallback for missing output.
- Continue legacy proposals without a finish timestamp without deadlocking; terminal assembly rejects unexpected active outcomes.
- Keep execution explanations with recorded validation evidence and retain existing local-tier continuation and offline Codex handoff.

### Added

- Auto / Ask / Act selection, Retry, sanitized Copy diagnostics, transition diagnostics and regression coverage for intent, recovery, continuity and visible outcomes.
- Reproducible local-model acceptance example using disposable synthetic projects.

## [0.3.0] - 2026-10-18

### Added

- Central registry: Fast Qwen3.5 4B Q8, Balanced Qwen3.5 9B, Strong Qwen3.6 35B-A3B Q4; per-tier contexts and custom assignments.
- Explicit catalog downloads, progress/cancellation and receipt/digest-aware cleanup.
- Bounded in-task escalation, compact continuation, cached evidence and accumulated offline Codex handoff.
- Local vision with picker/paste/drop, previews, normalization and encrypted image blobs.
- RAM-aware sequential loading; Strong supports expected partial GPU/system-RAM placement.
- First-token, throughput and memory metrics; actual coding/vision/continuation observations.
- Public documentation, MIT license, maintenance rules, privacy checks and clean-commit release pipeline.

### Fixed

- Approved deterministic exact-byte renames replace model reconstruction that could copy displayed line numbers or lose final newlines.
- Stale checks retain their original outcome; model notes are unverified and final claims use actual evidence.
- Tier switches preserve snapshot, permissions and consumed budgets; load/release failures retain recoverable evidence.
- Foreign Ollama residency, including the same tag, is preserved rather than unloaded as app-owned.
- Late image ingestion cannot attach to a different draft/project; closing model setup stops downloads.
- Public builds remap compiler paths and omit development-only browser source paths.
- Windows CI reads scanner checksum files explicitly as text, avoiding content-type and trailing-whitespace parsing failures.
- Public installer filenames avoid spaces so GitHub asset normalization cannot invalidate the checksum filename.
- Protocol fixtures use test-only deterministic memory telemetry; the Node argument test uses the same stripped environment as production. CI no longer assumes a large-RAM developer machine or a Node runtime that tolerates an empty environment.
- Real-command fixtures allow a bounded 60-second wait and CI limits test parallelism on shared Windows runners; production budgets and test assertions are unchanged.

### Changed

- Strong no longer requires full GPU residency; matching passing coding evidence and confirmation remain required.
- Additive migration preserves custom assignments and recoverable history.

## [0.2.1] - Historical milestone (exact date not independently retained)

### Changed

- Installed-profile verification/persistence for the original three-tier lineup.
- Verified automatic initial routing, Strong consent/rejection, GUI calibration/cancellation and real Balanced edit/check/Apply/Undo.
- Refined interrupted Apply/Undo recovery and installed-startup verification documented in the acceptance record.

## [0.2.0] - 2026-10-16

### Added

- Configurable Fast/Balanced/Strong assignments, deterministic initial routing and explicit Strong gating.
- Real isolated coding calibration beyond synthetic tool compatibility, keyed to digest/runtime/context.
- RTX 4070 Super measurements for Qwen3 4B Instruct Q4, 8B and 14B at 8K context; superseded by 0.3.0 defaults.

### Fixed

- Ollama schemas inline nested references and supply explicit object properties.
- Compatibility checks actual consumption of a random synthetic tool result.
- Unsupported thinking controls are avoided; thinking-only 4B was replaced with an explicit Instruct variant.

## [0.1.0] - Initial milestone documented 2026-10-16

### Added

- Windows Tauri/React/Rust app, project discovery, deterministic search/checks and controlled tools.
- Permissions, private task copies, diff review, journaled Apply/Undo and restart recovery.
- SQLite history, encrypted recovery blobs, Job Object command bounds and protected-file/path confinement.
- Optional bounded Playwright scenarios, authenticated local named-pipe helper and origin/request controls.
- Offline Codex handoff, activity/check evidence and dark/light desktop interface.

### Known limits then

- Ollama initially unavailable: protocol fixtures verified the harness, not local-model quality; real hardware evidence arrived in 0.2.
- Host code was not sandboxed; automatic retention cleanup was absent; installer unsigned.

[Unreleased]: https://github.com/Guardianec/local-agent/compare/v0.3.2...HEAD
[0.3.2]: https://github.com/Guardianec/local-agent/releases/tag/v0.3.2
[0.3.0]: https://github.com/Guardianec/local-agent/releases/tag/v0.3.0
