# Changelog

## v0.3.1 — 2026-10-09

- Join the tab label and close control in one continuous outline.
- Drag tabs to reorder while retaining the active document; persist order on restart.
- Right-click Rename changes the canvas file name; reject name collisions.
- Right-click Delete canvas file asks for confirmation before deleting the layout file. The × control still only closes the tab.
- Keep tabs on file-operation failures and suspend autosave while confirming tab management.

## v0.3.0 — 2026-10-09

- Move Edit to the top toolbar and add a scrollable second-row tab strip.
- Keep canvas files, selection, views and undo history independent between tabs.
- Restore all tabs and the active tab on startup; migrate older settings.
- Ask Save / Don't save / Cancel when closing a tab; normal application close saves every tab.
- Autosave changed background tabs and give unnamed tabs separate previous_canvas recovery files.
- Prevent multiple open tabs from writing to the same save target.

## v0.2.0 — 2026-10-09

- Add English / Traditional Chinese interface switching with persistent preferences.
- Display the application version in the window title and Settings.
- Keep old settings and existing canvas files compatible.
- Check both languages at minimum window width and common UI scale factors.
- Make Settings scrollable in shorter windows.
- First public source and Windows release.

## v0.1.0 — local version

- Infinite image canvas, animated GIFs, recursive folder import and large-batch confirmation.
- Image placement, selection, resizing, layers, undo/redo, and clipboard import.
- Reference/embedded saves, autosave and startup restore.
- Borderless fullscreen with edge-revealed controls.
