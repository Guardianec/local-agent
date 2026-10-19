# Three model profiles and measured routing (0.2.0)

The original single-model preference migrated to Balanced. Fast and Strong are independently configurable in Local models, also reached through Settings → Configure model profiles. Assignments persist; Auto is the default for every new task. Task-specific overrides use explicit profile IDs and do not change the saved assignments. Existing project/task/snapshot/permission records are retained.

## Routing contract

- L0 deterministic actions do not load a model. If lint auto-fix leaves errors, the remaining task is routed again before requesting inference.
- L1 tries verified Fast, then verified Balanced. L2 tries Balanced, then Fast. L3 tries Strong only when enabled and currently calibrated, then Balanced/Fast. Missing/unverified choices have an explicit Activity explanation.
- L4 remains a hard handoff decision even with a manual override. A model cannot lower this gate.
- Strong or another large model requires a passing coding calibration keyed by digest, runtime version and context, confirmed full GPU placement, sampled free VRAM of at least 512 MiB, and explicit enablement. Models with ≥8 GiB of weights are conservatively treated as large for this 12 GB target even when assigned to another slot.
- Every Strong attempt requires confirmation and the existing six-file limit. A manual Fast/Balanced choice does not relax an L3 scope limit. A missing or ineligible manual choice errors visibly rather than silently using another model.
- Model loading rechecks live GPU placement and free headroom. Auto prefers full GPU allocation when the NVIDIA monitor is available. Load/memory failure can downgrade Strong → Balanced → Fast, never revisiting the same model. A mid-inference runtime failure stops with retained evidence; it does not silently replay potentially completed actions.
- A Fast tool-protocol failure may upgrade to Balanced once, using the same context, task copy and approvals. The global two-error protocol repair budget, turn/tool/command/token budgets and active-time deadline are retained. Failed validation/no-progress limits still stop/escalate; switching is not a reset mechanism.

## Sequential inference and memory

One app operation reserves the controller at a time. It preloads the chosen model at the configured context; switches unload the app's previous model, and completion/cancellation releases the selected model. The app never kills shared Ollama or unloads an unrelated model by enumerating and clearing the runtime. Ollama is shared, so concurrent external clients can affect memory or reload a released model. This limitation is visible in Local models.

The optional monitor invokes a fixed Windows System32 `nvidia-smi.exe` query with no model-controlled arguments, a two-second timeout and hidden window. Unknown monitor data stays unknown; Strong fails closed. Full GPU placement compares Ollama's loaded total versus VRAM allocation with only a 1 MiB reporting tolerance. VRAM measurements are samples, not a promise about future memory pressure. Context changes disable Strong until recalibration.

## Real calibration

The user explicitly starts Test coding after a host-execution disclosure. A separate controller/data folder creates an app-owned Node fixture, asks the actual model to read and repair addition, and runs real tests. Only the exact built-in command is approved, and only while package/test bytes remain unchanged and the fixture contains its three allowed files. Generated JavaScript executes with the user's account; this is not an OS sandbox. No user project is used or modified.

Passing requires one eligible math-file diff, unchanged test/config files, actual passing checks tied to the final source manifest, and an unchanged original fixture. Reports retain real check/activity evidence, generated-token evaluation throughput, coding-task elapsed time, loaded allocation, GPU placement and minimum sampled free VRAM. A failed test cannot enable Strong. Cancellation cancels the child controller and retains disposable evidence.

Compatibility uses a random punctuation-free proof token returned through a synthetic native tool and requires its exact appearance in a final response without another tool call. The probe tool takes one required enum argument (`request: "proof"`) and rejects extra keys; the empty-object form caused a false negative with the installed Balanced model despite successful real project tool use. The same tool definitions are retained across the follow-up. This tests strict tool arguments and tool-result consumption without turning UUID punctuation copying into the compatibility criterion; the coding fixture still requires exact full file hashes.

## Ollama interoperability findings

Real testing on Ollama 0.40.5 exposed unresolved nested JSON-schema references and empty object schemas lacking `properties`. Tool schemas now inline local definitions and explicitly supply object properties, while Rust still strictly parses every argument. Error observations include bounded/redacted schema details so a model can repair an invalid call.

Thinking is disabled only when model metadata advertises the Boolean false option. The default `qwen3:4b` tag was thinking-only on the verified runtime and exhausted the bounded response budget. The explicit 4B Instruct variant passed the unchanged coding fixture repeatedly and is the chosen Fast model. The selected tags and measurements are documented in `../MODEL_CALIBRATION.md`; they are replaceable assignments, not hard-coded inference routes.
