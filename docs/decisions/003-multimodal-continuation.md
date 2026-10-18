# 003 — Local multimodal tiers and continuation (0.3.0)

This decision replaces 0.2's physical model lineup, full-GPU eligibility gate and one-time Fast upgrade. It preserves the Rust authority, tool schemas, permission decisions, private working copies, durable Apply/Undo, history and offline handoff design.

## Registry and migration

`assets/model-registry.json` is the physical-model source of truth:

| Tier | Exact local Ollama tag | Quantization | Initial context | Placement |
|---|---|---|---:|---|
| Fast | `qwen3.5:4b-q8_0` | Q8_0 | 16,384 | Prefer GPU; do not substitute Q4 |
| Balanced | `qwen3.5:9b-q4_K_M` | Q4_K_M | 32,768 | Default substantial work |
| Strong | `qwen3.6:35b-a3b-q4_K_M` | Q4_K_M | 32,768 | GPU + system RAM expected |

These tags were checked against the official Ollama library and local Ollama 0.40.5. [Fast](https://ollama.com/library/qwen3.5:4b-q8_0), [Balanced](https://ollama.com/library/qwen3.5:9b-q4_K_M), [Strong](https://ollama.com/library/qwen3.6:35b-a3b-q4_K_M). Published download sizes are estimates, not memory reservations or quality guarantees.

Normal routing knows tier names rather than model-family strings. Installed metadata/digests, native tool probes and real coding reports establish actual readiness. A future lineup changes the registry and explicit migration entries, not tool executors or the agent loop. Advanced assignments can preserve a user's custom installed models.

Registry version 1 migrates only unassigned slots or the exact previous configured defaults. An arbitrary custom assignment is preserved. The legacy single-model field still migrates into Balanced. New per-tier contexts take effect; Strong is disabled during this migration until current benchmark evidence is confirmed and the user enables it. Old calibration/history records remain available; an older digest/runtime/context cannot qualify a different build.

## Deterministic initial routing

No model is loaded merely to classify a task. Explicit search/check/review/browser presets remain deterministic. Simple explanation, summary, translation, description and bounded renaming requests prefer Fast. Ordinary code fixes start at Balanced, even for one matching file. Higher existing scope scores can select Strong when eligible and confirmed. Direct cloud requests and the existing hard gates for sensitive modifications, broad rewrites and destructive infrastructure/data work still recommend Codex.

An investigation/explanation of authentication is distinguishable from a request to redesign security-sensitive behavior. Runtime discovery of protected/sensitive paths still hits the same mutation policy. Manual Auto/Fast/Balanced/Strong selection is a starting preference. New tasks capture the advanced automatic-escalation switch (default on); Strong confirmation and safety limits apply regardless of that switch.

## Continuation instead of a fresh run

The controller escalates at an iteration boundary after evidence of repeated failure: two incompatible/incomplete responses, repeated unchanged actions, three turns without measurable progress, multiple failed validations, repeated completion requests while checks still fail, or context pressure after bounded compaction. One ordinary imperfect response is insufficient. A model's request to stop is not a grant to act; a failure-backed escalation can try the next tier, while a plain refusal may finish as a handoff.

The ladder is Fast → Balanced → Strong → offline Codex handoff. It never cycles through previously tried physical models. A missing/unverified/disabled next tier produces a useful handoff/setup result rather than substituting a different physical model without explanation. Automatic escalation off retains the same evidence and stops locally.

Before switching, persist `TaskContinuationContext` with original structured input text, constraints, project/recipe summary, relevant file names, bounded tool outcomes and excerpts, exact hash references, check outcomes, current diff, snapshot ID, accepted/rejected operation references, observations, unresolved issues, prior model, reason, image IDs and consumed counters. Accepted permissions in this record are descriptive; only the same live controller session can authorize execution. Recovered/externally exported records do not recreate grants.

The next model receives a fresh, compact evidence prompt and the original relevant image references, not a raw transcript. Hidden provider thinking is never persisted in this record or exported. Unchanged read/search/inspection results are cached by tool arguments and current manifest; an eligible repeated read can reuse exact evidence. Edits still require exact current hashes and normal policy validation.

Public model summaries are retained as explicitly unverified Agent notes; final completion claims still come from actual diff/check evidence. Deterministic `rename_file` copies exact source bytes into a new confined path and deletes the pinned source in the private task copy only, after concrete approval and hash revalidation. It refuses overwrite, traversal and case-only renames. Failed source deletion rolls back the new copy; an unsuccessful rollback leaves both paths with an explicit review error. This is not an atomic filesystem rename. Original-project Apply/Undo continues to use the existing journal and conflict checks.

Task identity, baseline, working copy, grants, denials, command count, tool count, generated-token count and active-time deadline survive switching. Limits remain 16 inference turns, 40 tool calls, six commands, 16,000 generated tokens and 12 minutes active time, excluding approval waits. The total tool-protocol repair budget remains two; escalation does not buy extra malformed calls. Strong confirms a six-file scope and checks the existing proposal against it before loading. Security/scope hard stops never get another local attempt merely because a larger model exists.

Completion requests trigger actual final validation when eligible. A failed test does not become success because a model says it finished. Checks preserve their original outcome independently of a later `stale` freshness label; this prevents losing failed-approach evidence after edits.

## Lifecycle and memory

One application operation owns inference at a time. A tier change stops requesting the old model, unloads the app-owned model through Ollama, observes release, checks available memory, preloads the next model at its configured context and continues. Completion, cancellation and errors release the selected model. A failed release is retained as pending ownership; a subsequent load cannot knowingly overlap it.

Do not kill Ollama, change its global settings, or enumerate/unload unrelated models. If another model is already loaded, the app asks the user to stop its use first. Ollama is shared: another program can race the app and change placement or reload a model. This is not an exclusive OS/runtime lease.

Use Windows `GlobalMemoryStatusEx` and the existing bounded fixed NVIDIA query. Strong's preflight requires known available RAM. The conservative estimate accounts for all model weights, approximately 80% of available GPU capacity, context overhead (0.5 GiB per 8K, minimum 0.5 GiB) and a 4 GiB Windows/app reserve. It is deliberately an estimate, not a precise KV-cache or allocator model. Below the estimate, do not allocate; suggest closing heavy apps, reducing tier context, using Balanced or preparing Codex. After load, available RAM below 2 GiB rejects the load. Sustained available RAM below 1.5 GiB for eight 250 ms samples during inference cancels the task and retains its work.

Partial GPU placement is a supported Strong result. It is shown as GPU + system RAM, not a compatibility error. Full GPU placement is still useful benchmark evidence for Fast/Balanced. Context is configured per tier (4K/8K/16K/32K); an advertised 256K window is never enabled automatically. Context pressure uses retrieval/compaction and continuation rather than concatenating entire repositories or transcripts.

Thinking controls remain provider-aware. Fast and ordinary bounded Balanced work request minimal reasoning only when the runtime explicitly supports it. Investigative/uncertain Balanced tasks and normal Strong attempts use the runtime default, with a 4,096-token response cap rather than requesting maximum reasoning. Other minimal responses retain the 2,048-token cap. Total generation remains 16,000 per task. The tiny coding benchmark uses minimal reasoning when supported; its timing is not a prediction for difficult Strong work.

## Image inputs

Inputs are tagged text/image/file-reference blocks. The native decoder accepts PNG, JPEG and WebP by content, not filename. Limits: four images per task, eight blocks, 8 MiB compressed bytes per image, 40 megapixels, 16,000 per dimension and a decoder allocation bound. Corrupt/unsupported/oversized inputs fail before inference. Apply image orientation, downscale the longest edge to 2,048 when necessary, normalize to a metadata-free PNG and produce a bounded JPEG thumbnail. The UI identifies resizing; text-heavy images can lose fine detail, so users can crop before attaching.

Original and normalized bytes are content-addressed DPAPI blobs. SQLite/task records store opaque IDs, dimensions and names, not repeated base64 bodies. Clipboard paste and HTML file drop use the same native ingestion path as the picker. Removing an attachment removes its draft reference; shared/recoverable blobs are retained. This release adds no automatic image/history pruning.

The provider assembles native Ollama user-message `images` arrays only for a request to the fixed loopback endpoint. Internal `image_refs` never go over the wire. The serialized-context estimate reserves a conservative per-image allowance; provider token accounting is still authoritative when available. A non-vision model is rejected before any image is sent to it. Image references survive local tier continuation. Future browser/desktop screenshot producers can create the same input type, but no new operator or screenshot-control authority is introduced.

Image text is untrusted input, subject to the same tools and permissions as repository content. Structured project files, commands and browser observations remain preferable where available. Images cannot grant command, filesystem, browser-origin or account permissions.

## Installation, cleanup and benchmarks

Install is an explicit UI action against a catalog tier; Strong's confirmation shows its approximate 24 GB download and RAM/offload expectations. Native `/api/pull` NDJSON progress is bounded and persisted for display; cancellation drops the request and preserves reusable Ollama layers. Errors release the operation reservation and offer retry. Quantization is checked after pulling. No Q4 substitution for Fast, paid cloud tag, remote model host, arbitrary download URL or automatic model download is allowed.

Cleanup considers only registry-listed superseded, currently unassigned models. A matching app-install receipt and digest establish ownership; previous assignment/calibration alone does not. Owned entries may be preselected, ambiguous entries remain unchecked, unrelated models are excluded, and removal is explicitly confirmed. Loaded models cannot be removed. Report logical removed model sizes as approximate: shared layers and unrelated disk activity prevent claiming that their sum is exact space recovered.

The existing real coding calibration is the small benchmark foundation: synthetic native tool-result consumption, an isolated built-in edit, actual Node tests, immutable fixture tests/config, final-manifest validation and original-file preservation. A synthetic argument error permits one bounded correction; invalid calls never execute. Reports include elapsed task time, first emitted token/chunk timing where observed, eval throughput, sampled model VRAM, minimum free RAM/VRAM, sampled peak **total system RAM used**, retries and escalation count. These are not process-specific RAM measurements, allocator peaks or broad quality rankings. The fixed fixture runs generated JavaScript with explicit host-execution consent; it is not a sandbox.

## Handoff and privacy

Strong exhaustion preserves the accumulated local investigation in the existing handoff preview. Export adds sanitized `evidence/continuation.json`, stage identities and failure evidence. Exact snapshots/patch bases and Apply status remain visible. JSON stays valid after privacy filtering; a patch remains complete or omitted.

Image pixels are excluded from copy/export by default, with an explicit omission notice. The app has no cloud upload endpoint. If the user wants a cloud agent to see an image, they must review it and deliberately attach it there. A local image ID does not imply that Codex can read a private app artifact. Descriptions/snippets are still best-effort redacted and previewed; automatic detection cannot guarantee removal of every visual/text secret.

## Verification boundaries

Unit/contract tests exercise registry migration, hardware gates, cancellation/failed loads, continuation, cached evidence, disabled escalation, image validation and rejection by a non-vision model. Deterministic protocol fixtures verify the model-independent harness, not model intelligence. Separate real-model coding/vision/native UI observations are recorded in `../MODEL_CALIBRATION.md` and `../ACCEPTANCE.md`. A deliberately unsatisfiable test can verify real tier switching without pretending to benchmark difficult reasoning quality.
