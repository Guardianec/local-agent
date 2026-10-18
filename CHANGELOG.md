# Changelog

Meaningful changes are grouped by application version, using Keep a Changelog-style sections. Earlier milestones are reconstructed from existing architecture/acceptance documents, not invented Git commits or tags. The first public source release is 0.3.0.

## [Unreleased]

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

[Unreleased]: https://github.com/Guardianec/local-agent/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/Guardianec/local-agent/releases/tag/v0.3.0
