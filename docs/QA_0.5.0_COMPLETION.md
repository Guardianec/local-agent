# 0.5.0 continuation completion — 2026-10-22

This supersedes the pending implementation/acceptance lists in the [initial QA record](QA_0.5.0.md) and [handoff](../HANDOFF_0.5.md). Production installer and publication verification are separate, recorded after they run.

## Regressions and security

The full updated native suite passed 124 unit tests and eight local integrations; the subsequently added connection-test cancellation/persistence regression passed separately (125 collected unit tests). The production pipeline repeats the complete suite. All 16 frontend tests and production TypeScript/Vite build passed. Existing browser, private-copy, Apply/Undo, model/intent and recovery regressions passed. The external HTTPS browser test is intentionally ignored in the default suite. Formatting/strict Clippy and exact-tree audits are repeated for packaging.

Synthetic peers cover modern/legacy stdio and Streamable HTTP, SSE responses, duplicate/colliding tool names, discovery counts, unsupported schemas/versions, malformed JSON, oversized JSON/SSE, schema changes, auth/rate/redirect errors, slow startup and cancellation. A real 30-second timeout followed by a separate call proved the timed-out call was not automatically repeated. Explicit connection close and Drop each terminated supervised descendant listeners. Settings cancellation releases its controller lease and safe diagnostics persist after reopening storage.

Controller fixtures verify mid-task acquisition/next-turn exposure, exact call denial with no effect, protected-file refusal, unknown capability denial, stale schema rejection, compact index selection and at most eight MCP schemas. Balanced → Strong retains the same task/connection: the fixture call count advances from one to two, with one connection grant and compact evidence retained.

Hostile fixture output cannot grant authority. Configured credential echoes are scrubbed before serialization, including quotes/backslashes and object keys. Task/handoff canary tests passed. A cancellation race initially returned protocol failure when SDK shutdown won the race; cancellation now retains its explicit outcome. Native UI passed invalid URL/executable, command consent, credential storage/clearing, bounded stdio discovery, actual process restart, lazy reconnect and persisted diagnostics. Generic redaction remains best effort, not proof against every transformed secret.

## Real local models

Single-fixture wall times; normal compatibility/calibration/RAM gates were retained. Tier-comparison automatic escalation was disabled.

| Scenario | Result |
|---|---|
| Fast Tauri docs | 15.21 s; two MCP calls; report ready; no changes |
| Balanced Tauri docs | 17.93 s; two MCP calls; report ready; no changes |
| Strong Tauri docs, 16K | 84.89 s; two MCP evidence calls/three total tools; report ready; no changes |
| Fast conversation | 7.55 s; zero tools/MCP |
| Balanced conversation | 8.14 s; zero tools/MCP |
| Fast arithmetic repair | 9.56 s; two native tools; only sample.ts proposed; zero MCP |
| Balanced arithmetic repair | 10.60 s; two native tools; only sample.ts proposed; zero MCP |
| Balanced GitHub CI investigation | 50.85 s; three MCP evidence calls/four total tools; no changes |

Strong passed matching 32K coding calibration in 56.48 s and 16K calibration in 55.50 s. Its 32K docs attempt and later coding/conversation loads were refused by unchanged RAM preflight; only the successful 16K integration task is claimed. Arithmetic fixtures register no test recipe, and their visible result explicitly says untested. Original bytes remained unchanged. Model documentation summaries and CI root-cause hypotheses are unverified prose.

## Real services and privacy

Context7 anonymously negotiated 2026-07-28. Resolver/query-docs passed for Tauri (4,477 evidence bytes), React (5,279), Ollama (11,450), and the official Rust SDK (7,125), without truncation in that run. Initial discovery indexed two tools in 0.88 s. Real models independently resolved/queryed Tauri.

The user explicitly entered a read-only GitHub credential in the isolated app. Official MCP repository search, issue search/comments, PR search/reviews and Actions listing passed. Lockdown refused an outsider's full issue body and remained enabled. A PR response exceeded the compact-evidence budget; narrower upstream fields/pagination succeeded. The real Balanced CI task retrieved a failed Windows-check run and identified the failing Rust test/waiting-approval evidence from a bounded job-log tail. It did not claim a new repair. The credential was asserted absent from serialized task and offline handoff records. No write tools were tested.

Automatic approval review required separate authorization for bounded CI logs entering local Ollama; the user granted it before the run. The earlier environment-token example was replaced with a strictly anonymous example. MCP acceptance read no unrelated token source. Git history scanner passed on the existing 12-commit history; proposed-tree/history scans repeat before publication.

## Retained limits

OAuth/refresh/elicitation, resources/prompts, standalone legacy SSE, schema references/regex, multiple independent credentials and encrypted-blob pruning are unsupported. Custom programs run as the Windows user, not an OS sandbox. External effects are outside Apply/Undo. No exhaustive every-shutdown-window, multi-hour soak, DNS-rebinding infrastructure or accessibility audit is claimed. REA, Penpot, Office, scheduling, self-installation and unrestricted GitHub writes remain deferred. No implementation follow-up for another model remains.

## Production release

Pending final clean-source production packaging, exact installer upgrade/data-retention acceptance, tag/push/publication and downloaded-checksum/README/default-branch verification. This paragraph is replaced only with actual results.
