# 004 — Public releases and build privacy

The application stays at 0.3.0 for its first public release. Older milestones are reconstructed from existing documents; no old Git tags or commits are invented. MIT is the owner's selected license.

The public installer is built from a clean commit through `scripts/build-release.ps1`, which runs checks and invokes the existing Tauri/NSIS pipeline. Compiler path prefixes are remapped to stable nonpersonal locations. Release runtime setup supplies installed browser resources, so development-only manifest paths are excluded from release code. This changes build privacy, not the authority model.

Privacy/version/link checks and Gitleaks complement manual source/asset review. Local logs, task/model data, tools and generated release artifacts remain ignored. Shared tags are immutable. Release notes identify source commit and installer checksum; the only user assets are the production installer and SHA256SUMS. Signing/automatic updates remain deferred.

Windows CI uses pinned official actions, read-only permissions and no release authority. Human or agent release operators must follow CONTRIBUTING.md, update changelog/history, test the exact installer and verify public downloads. GitHub publication requires user authorization, not merely a repository instruction.
