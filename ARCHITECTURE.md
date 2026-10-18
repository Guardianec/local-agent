# Local Agent 0.3 architecture

The Windows application remains Tauri 2, React/TypeScript and a Rust authority core, using Ollama's native loopback API. [HANDOFF.md](HANDOFF.md) is the original baseline; [decision 003](docs/decisions/003-multimodal-continuation.md) supersedes its model, context and escalation rules. It does not replace the filesystem, permission, snapshot or process boundaries.

```mermaid
flowchart TB
  UI[React task input / images / models / review] --> IPC[Typed Tauri IPC]
  IPC --> Core[Rust task controller]
  IPC --> Images[Bounded image decode / DPAPI blobs]
  IPC --> Setup[Explicit install / cleanup / benchmark]
  Core --> Router[Deterministic initial route]
  Router --> Registry[Central model registry]
  Core --> Lifecycle[Sequential load / unload / memory checks]
  Lifecycle --> Ollama[Local Ollama HTTP provider]
  Core --> Continuation[Bounded task continuation]
  Continuation --> Lifecycle
  Core --> Tools[Shared tool registry]
  Tools --> Policy[Permissions / exact operation approvals]
  Policy --> Files[Confined private task copy]
  Policy --> Commands[Fixed recipes / Windows Job Objects]
  Files --> Review[Journaled Apply / Undo]
  Core --> Store[SQLite history / encrypted blobs]
  Images --> Store
  Continuation --> Handoff[Offline Codex handoff]
```

## Ownership of decisions

- `assets/model-registry.json`: the three default physical identities, quantization, display text, preferred contexts, capabilities and expected placement. Installed capabilities come from Ollama metadata; registry descriptions do not override missing runtime support.
- `src-tauri/src/registry.rs`: registry parsing, tier context selection and non-destructive settings migration. Arbitrary custom assignments remain intact.
- `models.rs`: selects a verified, installed local model. Strong additionally needs an enabled profile and a passing digest/runtime/context-matched coding benchmark. Full GPU residency is **not** required.
- `lifecycle.rs` and the controller's activation/release methods: system-memory observations, conservative allocation preflight, sequential transitions and sustained low-memory detection. Unrelated loaded Ollama models are preserved and block loading another tier.
- `continuation.rs`: builds bounded evidence from the existing task; it neither grants permissions nor persists hidden reasoning. `agent.rs` decides when a deterministic escalation signal is sufficient and retains global budgets.
- `images.rs`: validates and normalizes inert image inputs, stores encrypted source/normalized bytes once, provides bounded previews and assembles transient Ollama `images` payloads. Task history holds references, not base64 copies.
- `model_management.rs`: explicit installation and cleanup operations. Download progress is a service record. Ownership is established by a matching installation receipt, not by a model-name guess.
- Existing `security`, `fs`, `commands`, `snapshots` and `browser` modules retain their authority. New models never receive a different or more permissive tool harness.

## Persistence and compatibility

Additive task fields use Serde defaults: structured inputs, frozen escalation preference, stage history, continuation and metrics. Existing task, approval, snapshot and journal records remain readable. The generic SQLite record schema and encrypted blob format are unchanged. Old global context/model fields remain for compatibility; normal inference uses per-tier context settings.

An in-task tier transition keeps the same task ID, snapshot, working files, check records, grants, denied operations and remaining limits. An explicit **Continue task** remains a new user-started task, as before. On restart, interrupted tasks do not resume commands or approvals automatically.

There is no cloud inference, cloud model subscription, automatic external image transmission, new browser operator, desktop controller, MCP ecosystem or background autonomy in this update.

## 0.3.1 stabilization

[Decision 005](docs/decisions/005-conversational-outcomes.md) defines the interaction and terminal-result contracts. `interaction.rs` separates Converse/Inspect/Execute from model selection. Explicit constraints, project/prior-task context and a bounded optional local classification feed intent-specific prompts and schemas. `agent_tools.rs` enforces intent again at dispatch, alongside existing permissions.

The controller publishes `TaskResult` after cleanup on success, failure, escalation or cancellation; restart reconciliation supplies interrupted results. `Task::transition` records lifecycle edges and unexpected-edge diagnostics. The frontend renders the result separately from Activity, with a terminal fallback and allowlisted Copy diagnostics. See the decision for transition edges, failure semantics and the host-code limitation on diagnostic commands.
