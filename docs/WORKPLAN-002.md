# v0.2.0 work plan

Read: PR-002.md, PR-001.md, WORKPLAN-001.md.

1. Implement a shared translation catalog, per-application language preference, localized interface and persisted settings. Keep existing document compatibility.
2. Add version display from Cargo metadata; verify both languages, narrow layout, immediate switching, restart persistence and regressions.
3. Prepare bilingual README and release notes. Initialize local Git, inspect the exact public file list, exclude saves/builds/test artifacts, then create the authorized public repository and publish v0.2.0.

Verification: format check, Clippy, unit tests, release build, native UI observation where available, archive integrity, remote repository visibility and tag/release metadata.

Status: implementation and local verification complete (33 tests, bilingual layout checks, native English UI). Current target: publish the verified source and v0.2.0 release.
