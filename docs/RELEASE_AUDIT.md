# Public release audit вЂ” 0.3.0

Prepared 2026-10-18. Scope: the exact initial Git tree/history and newly built Windows installer, not every ignored file on a developer's computer.

## Source/history

The checkout initially had no commits/remotes, so no hidden old Git history needed rewriting. Earlier milestones are documented as reconstructed history, not fabricated tags.

Targeted filename/content scans cover credentials/tokens/passwords, .env, private home paths/emails, logs/databases/model data, temporary files and binaries. The initial source scan's only email match was reserved synthetic fixture data. Gitleaks independently checks proposed tree/history. No broad application-source suppression is used. Public author metadata uses a GitHub no-reply address.

Ignores exclude local app/task data, dependencies/build output, credentials, personal tool settings, databases, weights, installers/debug files and temporary output. Lockfiles and generated IPC contracts intentionally remain source. Icons are small source assets. No private screenshots, logs, weights or benchmark databases are publication assets. The README image is a visually reviewed native screenshot of a disposable synthetic fixture; its PNG contains no text/EXIF metadata chunks. Automated binary checks do not perform OCR, so visual review remains required.

## Installer

The old local binary contained compiler/development paths. Public preparation removes the release-only development helper path and remaps compiler paths. The public installer uses Tauri/NSIS from a clean commit; release notes bind its checksum to that source commit. Only the installer and SHA256SUMS are user assets.

The privacy checker scans ASCII/UTF-16 production executable and installer strings. This complements Gitleaks/source review; none proves absence of every encoded secret. The installer is unsigned and contains no model weights. Private logs and machine paths are kept out of publication.

## Gate

Before publication: privacy/Gitleaks checks pass; versions agree; native/UI tests, formatting, strict Clippy and production packaging pass; exact installer is installed/launched; commit/tag/checksum and downloads are verified. Repository content becomes public only after the gate. Enable GitHub secret scanning/push protection and private vulnerability reporting where available.

Repeat the audit for every changed release under [CONTRIBUTING.md](../CONTRIBUTING.md) and [AGENTS.md](../AGENTS.md). This report does not replace rescanning.
