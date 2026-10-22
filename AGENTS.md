# Coding-agent instructions

Read README.md, CONTRIBUTING.md, ARCHITECTURE.md and DECISIONS.md. Keep scope focused; do not casually redesign working boundaries.

## Mandatory maintenance

For MCP work, read docs/MCP.md, docs/decisions/007-mcp-capabilities.md, docs/QA_0.5.0.md and HANDOFF_0.5.md. Verify current official SDK/spec behavior. Do not convert pending candidate gates into release claims or expose credentials through model tools, fixtures or acceptance examples.

Every meaningful update must:

- Bump the version when appropriate and align desktop manifests/lockfiles. Documentation-only corrections need not invent a version.
- Update root CHANGELOG.md and docs/DEVELOPMENT_HISTORY.md with actual work/checks. Never fabricate history or measurements.
- Document significant architectural decisions in docs/decisions/ and DECISIONS.md. Keep architecture and README feature status accurate.
- Preserve Rust authority, strict schemas, confinement, explicit host-code consent, bounded tasks, private copies and conflict-aware Apply/Undo.
- Run tests, strict linting and production build before release. Follow CONTRIBUTING.md and scripts/build-release.ps1; never publish a debug build or uncommitted source.
- Audit source/history/assets for secrets, private paths, logs, task evidence and weights. Never commit credentials or broadly disable secret checks. Use a public/no-reply author email.
- For an authorized user-facing version, create/update a GitHub Release and attach the exact latest tested Windows installer plus checksum. Publication requires user/session authorization. Do not silently replace a shared tag/asset; fixes need an appropriate new version.
- Verify visibility, default branch, tag commit, README rendering/links and downloaded checksums before declaring publication complete.

## Headroom for coding

Use local Headroom MCP tools for large logs/JSON/search/source excerpts when useful. Prefer targeted reads; retain compression hashes and retrieve exact originals before edits/conclusions depending on omitted details. Do not compress secrets, small inputs or exact patches. If unavailable, continue focused reads and mention the limitation. Never claim all Codex traffic is automatically compressed.

## Website animation tools

When website motion is in scope, prefer GSAP for precise timelines, Lenis only where smooth scrolling fits, and individual official TypeScript React Bits components. Install only needed project-local dependencies; preserve licenses and cleanup effects/listeners. Respect reduced motion and keyboard/anchor navigation. Tool availability alone is not a request for global motion or redesign.
