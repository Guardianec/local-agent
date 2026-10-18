# Architecture decisions

1. [Original architecture handoff](HANDOFF.md): Tauri/Rust authority, private task copies, bounded tools and explicit review.
2. [Implementation details](docs/decisions/001-implementation-details.md): measured Windows filesystem/process/browser constraints and retained recovery behavior.
3. [Original three-profile release](docs/decisions/002-model-profiles.md): historical 0.2 routing and calibration findings.
4. [Multimodal continuation](docs/decisions/003-multimodal-continuation.md): current 0.3 registry, lifecycle, escalation, image and migration contracts. Supersedes the older lineup and full-GPU Strong requirement.
5. [Public releases and build privacy](docs/decisions/004-public-release.md): clean source provenance, path remapping, source/artifact audits, MIT licensing and release obligations.

6. [Conversational outcomes](docs/decisions/005-conversational-outcomes.md): interaction intent, bounded ambiguity resolution, selective tools, continuation, lifecycle and terminal results.

The current module map is [ARCHITECTURE.md](ARCHITECTURE.md). Hardware observations and limits belong in [MODEL_CALIBRATION.md](docs/MODEL_CALIBRATION.md), separate from architecture promises.
