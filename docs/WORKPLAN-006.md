# v0.4.0 work plan

Read PR-006.md and WORKPLAN-005.md first.

1. Clarify behavior — complete; four user answers incorporated in PR-006.
2. Bounded optional preload and Settings — complete; real background decode, nearest priority, budget decrease/zero, retention and preference persistence tests.
3. Blank-canvas Settings and Gather — complete; real pointer-menu events, mixed-size geometry, anchor, Undo tests.
4. Per-tab comic reading — complete; geometry, wheel/zoom, arrow navigation, original-layout preservation, save/restart and Source delete/tab-switch integration tests.
5. Verification — complete: 52 tests pass, formatting, strict lint and release build pass. Two opt-in load benchmarks not rerun. Native isolated Windows test confirms reading submenu, vertical layout, next-page key, blank-context Settings and preload controls. Horizontal mode, gathering and Ctrl+wheel are covered automatically rather than native end-to-end.
6. Guides, version and delivery — v0.4.0 prepared for publication to the existing public repository. No user data in package.

No remaining implementation targets. Last native inspection preceded the final mode-aware footer text adjustment; the final build is covered by automated tests.
