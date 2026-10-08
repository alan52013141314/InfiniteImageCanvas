# v0.2.0 work plan

Read: PR-002.md, PR-001.md, WORKPLAN-001.md.

1. Implement a shared translation catalog, per-application language preference, localized interface and persisted settings. Keep existing document compatibility.
2. Add version display from Cargo metadata; verify both languages, narrow layout, immediate switching, restart persistence and regressions.
3. Prepare bilingual README and release notes. Initialize local Git, inspect the exact public file list, exclude saves/builds/test artifacts, then create the authorized public repository and publish v0.2.0.

Verification: format check, Clippy, unit tests, release build, native UI observation where available, archive integrity, remote repository visibility and tag/release metadata.

Status: complete.

Verified: 33 tests, format check, Clippy, release build, bilingual minimum-width/scale checks, native English UI and version title, old settings compatibility, immediate language persistence, ZIP contents and executable integrity.

Published:
- Public repository: https://github.com/alan52013141314/InfiniteImageCanvas
- Release: https://github.com/alan52013141314/InfiniteImageCanvas/releases/tag/v0.2.0
- GitHub asset SHA-256 matches the locally verified archive: 9aefa35d39d3d30101179a65cc3bda0e4e3befecb945deaac340b3b72cbfce4e.
- Personal saves, managed image assets, test output and build artifacts are excluded from source control. Release contains only the executable, two language guides and third-party notices; a checksum file is also published.

No remaining work for PR-002.
