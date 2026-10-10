# v0.3.2 image context menu work plan

Read PR-005.md and WORKPLAN-004.md first.

1. Clarify menu, shared-source and managed-asset behavior — complete; all three answers recorded in PR-005.
2. Context menu for topmost right-clicked image — complete; real egui secondary-button event test.
3. Confirmed source deletion across open tabs — complete; failure retention, background-work exclusion and history cleanup implemented.
4. Verification — 47 tests pass, formatting and strict lint pass; Windows release built. Tests use temporary files only. Two optional load benchmarks not rerun. Native window interaction was not tested in this round.
5. Version, bilingual guides and release archive — prepared for v0.3.2 and publication to the existing public repository.

Tests cover cancel/confirm, overlapping-image targeting, real source removal, multi-tab cleanup and restart, undo/redo, retained unrelated file, missing-source failure, packed-asset deletion and pending-save exclusion.
