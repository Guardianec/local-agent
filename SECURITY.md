# Security

## Private reports

Use this repository's **Security → Report a vulnerability** feature. Do not put exploit-ready details, credentials, private screenshots or task exports in public issues. If private reporting is unavailable, open a minimal request for a private channel without sensitive details.

This early 0.x project has no response SLA. The latest published version is the supported baseline.

## Boundaries

The 0.4.0 browser operator retains Rust as the browser network/action authority. Public and loopback profiles are separate; navigation and public-resource grants differ. Browser-only tasks cannot use project files or host commands. Static form submissions require an exact native request review; other public writes remain blocked. Password controls are excluded from observations and masked in screenshots; interaction on password pages is blocked. Downloads, uploads, personal profiles and persistent authentication are disabled. Temporary browser HOME is removed after process termination and during interrupted-task recovery, with junction refusal. See [browser operator boundaries](docs/BROWSER_OPERATOR.md), [decision 006](docs/decisions/006-browser-operator.md) and [QA limits](docs/QA_0.4.0.md).

Typed native IPC leads to Rust tool/policy validation, path/handle confinement, protected-file exclusions, fixed recipes and budgets. Models and repository/image text cannot grant authority. Original mutations require reviewed Apply; hash conflicts preserve newer user edits. DPAPI protects blobs for the Windows user, not against malware already acting as that user.

Approved tests/linters/build scripts execute as the user and can access files/network: **not a sandbox**. Optional browser origin/request controls are **not a firewall**. Execute trusted projects only.

Inference uses fixed Ollama loopback endpoints. Explicit model/component downloads need internet. Handoff export uploads nothing; redaction is best effort and images are omitted. Review artifacts before sharing.

Never commit .env, credentials, local data/logs, model weights or private screenshots. Use synthetic fixtures/reserved domains. Public builds must remove build-machine paths and scan source/history/assets. See CONTRIBUTING.md.

The installer is unsigned. Checksums identify bytes but are not independent publisher signatures. Automatic updates and exhaustive adversarial/power-loss testing are not provided.
