# Verified local model profiles — 0.3.0

Measured on 2026-10-17 using Local Agent 0.3.0 development builds and Ollama 0.40.5 on Windows, RTX 4070 Super (12,282 MiB VRAM), i5-12400F and 32 GB RAM. Release checks were repeated on 2026-10-18. The registry in `assets/model-registry.json` supersedes the 0.2 lineup; no Fast Q4 substitution is used.

| Tier | Exact Ollama model | Context | Coding fixture | Tokens/s | Model GPU allocation | Minimum free GPU | Minimum free RAM | First token after preparation |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| Fast | `qwen3.5:4b-q8_0` | 16,384 | 11.57 s | 79.2 | 5.11 GiB | 5.21 GiB | 19.79 GiB | 0.61 s |
| Balanced | `qwen3.5:9b-q4_K_M` | 32,768 | 13.71 s | 71.4 | 6.12 GiB | 3.96 GiB | 19.73 GiB | 0.41 s |
| Strong | `qwen3.6:35b-a3b-q4_K_M` | 32,768 | 49.63 s | 42.8 | 9.05 GiB | 0.89 GiB | 7.21 GiB | 3.66 s |

All three passed real read/edit/Node-test execution on an isolated addition fixture. Fast and Balanced were fully GPU resident. **Strong intentionally used GPU plus system RAM** and passed at 32K context; full GPU residency is not an eligibility requirement. Its minimum observed free system RAM was 7.21 GiB; peak sampled total machine RAM use was 24.61 GiB. Fast/Balanced peak total RAM use was approximately 12.02/12.08 GiB. These are whole-machine samples including other applications, not model-process allocations or allocator peaks.

Coding time includes preparation and tool/check steps after the separate compatibility probe. First-token time starts after explicit model preparation and records the first emitted content/thinking/tool chunk. Generation speed is Ollama eval_count/eval_duration, not wall-clock task speed. The tiny benchmark requests minimal reasoning where supported; it does not predict difficult-task quality, cold-start time or normal Strong reasoning latency. Strong's separate probe/loading took several minutes. Coding retries/escalations were zero; Balanced's preceding synthetic tool probe needed one bounded argument correction, which is not a coding retry. Invalid synthetic calls never executed.

## Real workflow evidence

- Fast described a new screenshot containing a random five-digit code (22741) and identified a button overlapping the Email field, using actual local image inference in 13.75 seconds. The code was absent from project text.
- Native WebView testing exercised paste/drop/preview/removal, Balanced image-guided CSS editing, a real approved Node check, diff review, Apply and exact-byte Undo.
- Fast completed three exact-byte file renames and retained a written summary, then passed Apply/Undo in 15.69 seconds. An earlier model attempt copied displayed line numbers when recreating files; that failed acceptance and led to the deterministic `rename_file` tool. The original files were never damaged.
- A real Balanced → Strong → Codex continuation fixture passed in 79.01 seconds. It deliberately has an unavailable external dependency. Strong received the same snapshot, two failed checks and four useful tool outcomes, emitted an actual response, and exported accumulated failure evidence. This verifies orchestration, not successful repair of an impossible dependency or a difficult-reasoning quality benchmark.

## Memory and runtime behavior

Only the current app-owned model is unloaded at a switch/completion. A model already resident for another client, including the same tag, is preserved and blocks a new app load with an actionable error. Ollama is shared; the app does not kill it or its runners. Sequential loading reduces competing allocations.

Before load, the app estimates weights/context demand against available GPU and RAM, keeping a conservative Windows/app reserve. Strong requires known RAM telemetry. Less than 2 GiB available after load rejects use; eight consecutive 250 ms samples below 1.5 GiB during inference stop safely with recoverable evidence. Short dips can recover. These boundaries are covered by controlled tests. Actual catastrophic RAM exhaustion was not induced on this PC.

Keep other GPU/RAM-heavy programs quiet when using Strong. Use Balanced for everyday work. Per-tier contexts can be reduced in Advanced settings; changing Strong's context, digest or runtime requires fresh matching coding evidence before enabling it. Missing/changed models are never silently replaced.

## Provenance

- `qwen3.5:4b-q8_0`: digest `e15635ee93129fd2086640926e900c37589c5167561104abc3914bd6644c7e07`; measured `2026-10-17T07:45:37.170526800+00:00`; Ollama `0.40.5`.
- `qwen3.5:9b-q4_K_M`: digest `56671c2ab9385f9cfcb404638e32cd62d88e3501d44822208363c010179a3c90`; measured `2026-10-17T07:53:09.153333500+00:00`; Ollama `0.40.5`.
- `qwen3.6:35b-a3b-q4_K_M`: digest `07d35212591fc27746f0a317c975a6d68754fb38e9053d82e25f06057af28522`; measured `2026-10-17T08:26:01.351032300+00:00`; Ollama `0.40.5`.

Full local reports are persisted in the app calibration records; ignored development evidence is `.local-data/model-verification-v03/results.json`. The UI shows actual reports rather than hard-coded performance claims. Re-run **Local models → Test coding** after a relevant change.

[Fast official tag](https://ollama.com/library/qwen3.5:4b-q8_0), [Balanced official tag](https://ollama.com/library/qwen3.5:9b-q4_K_M), [Strong official tag](https://ollama.com/library/qwen3.6:35b-a3b-q4_K_M). Model size on disk is not total memory demand. The prior 0.2 reports remain in local history but do not qualify these different models/contexts.
