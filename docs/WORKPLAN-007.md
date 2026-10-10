# v0.4.1 navigation and reading fit work plan

Read PR-007.md and WORKPLAN-006.md first.

1. Confirm interaction semantics — complete; three answers incorporated in PR-007.
2. Middle hold/release state machine — complete; both modes tested for threshold, speed by distance, toggle-off/release and focus loss. Existing short-drag test remains passing; tab/mode switches reset transient navigation.
3. Fit controls and wheel settings — complete; real context-menu event test, fit-axis geometry, separate multiplier behavior and global persistence verified.
4. Verification — 55 tests passed; formatting, strict lint and Windows release build passed. Two opt-in load benchmarks not rerun. Native window/physical long-hold testing was not performed this round; verification uses egui input events with controlled timestamps.
5. Delivery — v0.4.1 guides and archive prepared for the existing public GitHub repository; no personal saves included.
