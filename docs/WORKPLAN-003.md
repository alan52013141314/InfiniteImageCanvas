# v0.3.0 work plan

Read PR-003.md first.

1. Move Edit to the top toolbar; verify both languages at minimum width.
2. After clarification, separate per-tab document state (canvas, selection, history, view, dirty status, save target and pending operations) from application preferences and window controls.
3. Build the second-row tab strip, tab activation and confirmed new/open/close behavior. Switching must not modify image positions or mix save targets/history.
4. Implement the confirmed save/recovery semantics; retain unsaved data on errors, prevent unnamed-file collisions, and preserve existing file compatibility.
5. Test multi-tab operations, save failures, restart, language/layout and fullscreen; build and update versioned delivery artifacts.

Targets 1–5 implemented. All three decisions are recorded in PR-003. Forty automated tests pass, including actual pointer events for tab creation, switching and closing; independent history/views; all-tab restoration; recovery naming; save failures; bilingual layout and fullscreen regressions. Formatting, strict lint and the Windows release build pass. Two opt-in load benchmarks were not rerun. Native visual verification was unavailable because the desktop tool's launch approval timed out; it is not recorded as passed.
