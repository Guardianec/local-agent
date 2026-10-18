# Implementation decisions and measured constraints

The root HANDOFF.md remains the architectural baseline. These implementation details preserve its authority and recovery boundaries.

## Browser: bounded scenarios and a Windows named pipe

The first capability is `browser_check_page`: one explicitly approved origin and up to ten typed steps, executed in a new Playwright context. It supports unauthenticated public reads (JavaScript off) and disposable loopback app tests (JavaScript on). It does not expose arbitrary selectors, evaluation, a personal browser profile, or an agent-controlled browser RPC server.

The fixed helper exchanges newline-framed JSON over a random **local Windows named pipe**, authenticated by a per-run nonce, rather than stdin/stdout. This allows reuse of the already tested suspended-process/Job executor, whose input is intentionally closed. Standard output contains one bounded result. There is no listening TCP RPC or CDP port. Rust brokers page HTTP requests, checks origin and DNS/IP, and pins the chosen address. Main-document same-origin redirects restart navigation in a fresh page; subresource/cross-origin redirects are blocked. The resource cap is reduced to 512 KiB, avoiding multi-frame body transport in v1. Request count is 100 and total base64 transport budget 14 MiB per scenario.

A scenario reduces approval/state complexity compared with a long-lived interactive browser session. Local models can call it as a first-class tool when the optional component is installed and the original user request contains the exact URL. Deterministic browser tasks work without a model. Public links that need another origin require a new explicit check.

Tests use Playwright's matching Chrome for Testing, installed only in `.local-data/browsers`. A test using personal installed Chrome failed because that Chrome rejected automation under the stripped environment; no personal profile is used as a fallback. Production setup explicitly downloads matching Chromium and copies the selected Node runtime into per-user app storage. It does not download a model.

Request interception controls HTTP application data. Chromium may create an empty speculative TCP connection before a blocked navigation. The request-level test therefore asserts **zero HTTP bytes**, not absence of all socket creation. This is not an OS egress sandbox; the UI and security documentation must not claim one.

## Review and recovery

Apply binds the task revision and a hash of the exact reviewed file deltas. A changed proposal requires another review. Windows handle-based mutation, encrypted before/after blobs, a durable SQLite journal and conflict-aware Undo remain unchanged. Empty directories created by Apply are currently retained after Undo; source bytes/absence are restored, but directories are never recursively removed.

The store uses a versioned SQLite `records(kind,id,json)` table instead of many small relational tables. Writes remain parameterized and durable (`WAL`, `synchronous=FULL`), with independent record types for tasks, snapshots, journals, handoffs and probes. This keeps schema changes manageable without weakening journal ordering. SQL never reaches the renderer. There is no automatic recovery-data pruning in this version; keeping recovery data is safer until retention UI exists.

Startup reconciles incomplete task UI states with the latest root file journal, including the gap after file commit and before task-state persistence. Recovery journals identify the root operation so interrupted recovery is repeatable. Recovering an interrupted Undo returns to the applied state; recovering an interrupted Apply returns to the proposed state. A Windows exclusive data-folder lock permits one desktop controller per storage directory; an old lock file without a live handle does not block restart.

Source eligibility freezes captured `.gitignore` rules and retains tracked files even when ignored. Per-user/global Git excludes are intentionally not imported into snapshots; app artifact/secret exclusions always apply. Commands may not hide source changes by rewriting `.gitignore`.

## Windows command paths

Filesystem operations retain canonical extended paths and pinned handles. Child-process working directories are converted to ordinary drive paths after validation: npm's internal Windows script runner treats `\\?\C:\...` as UNC and loses the working directory. Child tool paths longer than 240 characters are refused with an actionable message. This was found by actual desktop command testing, not assumed from static code.

## UI and model evidence

Semantic CSS variables and accessible Radix primitives implement the specified design without a Tailwind dependency. Monaco's editor-only module is lazy-loaded; language services/catalogs are not shipped as eager imports. Browser assets and local workers require no remote CDN.

Completed coding-task summaries are derived from file deltas and actual check records. Model completion text cannot fabricate passed-test totals. Explanation tasks retain the model's explanation, visibly identified as such. Tool compatibility is probed with synthetic data before source is sent. Models are discovered, not hard-coded. At the initial 0.1 verification, Ollama was absent and protocol fixtures were not counted as a hardware benchmark. This limitation is superseded by decision 002 and the actual measurements in ../MODEL_CALIBRATION.md.

## Conservative capabilities

Host execution always requires a concrete recipe approval; task-scoped recipe reuse is supported. Persistent automatic recipe grants and a separate persistent host-trust settings screen are not enabled yet. Project permission mode automatically allows eligible **proposed edits**, never original-file application. Dependency preparation physically copies an ordinary existing npm tree after explicit approval; dependency installation/package additions remain unavailable. Missing Python modules are reported rather than installed automatically.

No stack, authority boundary, snapshot baseline or cloud-upload decision was changed.
