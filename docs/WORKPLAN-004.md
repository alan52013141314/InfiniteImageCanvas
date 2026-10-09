# v0.3.1 work plan

Read PR-004.md and WORKPLAN-003.md first.

1. Continuous tab control and drag/drop reorder — complete; actual pointer-event test and native outline inspection.
2. Right-click rename/delete — complete; user confirmed file rename/deletion. Confirmation, collision protection and failure retention verified.
3. Restart, bilingual UI and regression verification — complete; 43 tests pass, strict lint, formatting and release build pass. Two optional load benchmarks not rerun.
4. Guides, version and Windows delivery archive — complete. Publish to the existing public repository.

Native visual evidence: isolated v0.3.1 window at 1280 × 800 shows labels and × inside a continuous tab background/outline. Native right-click and deletion were not fully exercised; actual egui pointer events and filesystem tests cover them. User interaction interrupted the native automation session.
