# Local models and images

**Ollama is offline:** open Ollama and refresh Local models. The app talks only to `127.0.0.1:11434`; it does not start a daemon, use your cloud account or require a paid subscription. Deterministic project tools and existing review remain available.

**A model is missing:** choose Install for its tier. The dialog shows the approximate download size; Strong is about 24 GB. Stop cancels the current request; retry can reuse completed Ollama layers. Check disk space/connectivity if the download fails. A cached completion message without verified local metadata does not qualify a model. Fast must be the Q8 tag; there is no silent Q4 replacement.

**Installed, but tools are unchecked:** choose Check tools, then Test coding for real edit/test evidence. A single invalid synthetic call gets one strict correction; if compatibility still fails, no source is sent. A failed model is not marked verified just because it was downloaded.

**Strong cannot be enabled:** run its coding benchmark at the selected Strong context. The digest, Ollama version and context must match. Full GPU placement is not required. Changing the model build/runtime/context may require another benchmark.

**Strong needs more memory:** stop other inference apps and close memory-heavy applications. Lower Strong's context under its Advanced details, rerun its benchmark, or continue with Balanced/Prepare for Codex. The app uses estimates before loading and monitors available RAM while working; it cannot guarantee Windows or another application will not change memory use afterward.

**Another model is loaded:** the app preserves unrelated models instead of unloading them. Stop their use in the other application/Ollama, then retry. Avoid running multiple inference clients during benchmarks. The app never kills shared Ollama.

**Partial GPU/system RAM placement:** expected for Qwen3.6 35B-A3B on a 12 GB GPU. Its resident weights are much larger than its approximately 3B active parameters. Slower loading and responses are normal. Compare actual task results, not just advertised parameter counts or weight-file size.

**A task escalates:** Activity shows the reason and the next tier. The same task copy, findings, checks and image references continue. Strong asks for confirmation. If the next tier is unavailable or Strong stalls, review the preserved changes or prepare a Codex handoff. Switching does not reset the task's time/tool/command/token limits. Turn off automatic escalation in Settings for new tasks if you want the selected tier to stop locally.

**An image is rejected:** use a valid PNG, JPEG or WebP up to 8 MiB, at most four per task. Dimensions/decoded memory are bounded too. Export a smaller image if it exceeds the limit. The native decoder checks actual bytes; renaming an unsupported file is insufficient. Images above 2,048 pixels on the longest edge are resized. Crop dense text to keep it readable.

**Paste/drop does not show a preview:** focus the task input and paste an image rather than a clipboard file path. Drop an image file into the task composer or use Attach images. Unsupported clipboard formats are not silently converted through a cloud service. Removing a draft attachment does not delete blobs referenced by other tasks or recovery data.

**The selected model has no vision:** select a compatible tier or remove the image. The app does not send images to a text-only model or to an external fallback.

**Sharing a screenshot with Codex:** exported handoffs exclude image pixels and say so. Inspect the screenshot for keys, passwords or private information, then deliberately attach it in the cloud agent if needed. An opaque local image reference is not accessible to that agent. The app uploads nothing.

**Old models remain:** cleanup preselects only a superseded model with a matching app installation receipt. Existing assignment or coding calibration is not proof that the app originally installed it. Ambiguous models stay unchecked; unrelated models remain preserved. Removal is explicit and loaded models are rejected. Reported logical model sizes are approximate because Ollama may share layers.

**Tests/builds need permission:** changing a model does not change host-code execution rules. Approved project commands run as your Windows account, not inside an OS sandbox. The app still requires review and explicit Apply before original source files change; Undo rejects newer user edits rather than overwriting them.
