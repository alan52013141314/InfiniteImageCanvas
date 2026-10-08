# InfiniteImageCanvas v0.2.0

Extract the Windows x64 release archive into a writable folder, then run the executable. No server, account, or installation is needed. Windows 11 has been tested; Windows 10 is a compatibility target, not a tested environment. An OpenGL 3.3 graphics driver is required.

## Language and version

Open Settings (設定), then choose English or 繁體中文. The selection applies immediately and is remembered across restarts. Existing installations retain Traditional Chinese by default. The app version appears in the title bar and Settings; it is separate from the canvas file format version.

## Import and arrange

Drag PNG, JPEG, WebP, BMP or animated GIF files onto the canvas. Use Import to select files, Folder to recursively scan a folder, or Paste / Ctrl+V for a clipboard image. A folder can also be dropped directly. Scanning can be cancelled.

Batches with more than 100 images require confirmation before any images are added. Up to 100 are added directly. Cancelling leaves the canvas unchanged.

A dropped image starts at the pointer's position (its top-left corner). Multiple images form touching rows with ceil(sqrt(count)) columns: 3 images use 2+1 rows; 5 use 3+2. Each row's tallest image determines the next row's vertical position. Original proportions and dimensions are retained. Settings lets you choose sequential or random import order.

## Controls

| Action | Control |
|---|---|
| Move an image or selected group | Left drag |
| Pan | Middle-button drag |
| Zoom the canvas | Mouse wheel, centered on the pointer |
| Resize proportionally | Drag a selected image's corner |
| Multi-select | Shift/Ctrl click, or drag empty space |
| Select all | Ctrl+A |
| Delete selection | Delete or Edit menu; original files are not deleted |
| Change stacking | Edit → Bring to front / Send to back |
| Undo / redo | Ctrl+Z / Ctrl+Y or Ctrl+Shift+Z |
| Fit all | Home or Fit all |
| Fullscreen | F11 or Fullscreen |
| Save / Save as | Ctrl+S / Ctrl+Shift+S |
| Open / New | Ctrl+O / Ctrl+N |

In fullscreen, controls hide until the pointer reaches their top or bottom region. They overlay the canvas without shifting images. Open menus keep the top controls visible. F11 always returns to windowed mode.

## Save and restore

Canvas files use `.icanvas`. The default directory is `save file` beside the executable. The first manual save uses `canvas_yyyyddMM_01.icanvas` (year, day, month), increasing the suffix if needed. For example, October 9, 2026 becomes `canvas_20260910_01.icanvas`.

Changes autosave every minute. Normal close saves to the current file; an unnamed canvas uses `previous_canvas.icanvas`. Autosaving an unnamed canvas still permits a date-based name on its first manual save. Startup restores the last canvas. New/Open first saves the current canvas.

Failed saves keep the window and unsaved data available for Retry or another save location. Forced termination can only recover the last successful save.

Reference mode stores original file paths, so keep the originals in place. Embed mode packs originals into one portable canvas file. Clipboard images and extracted embedded images use the managed `assets` directory; keep these assets when using reference mode. If the executable directory is unwritable, select another save location; preference recovery may use the user's local application-data directory.

There is no configured image-count or original-resolution limit. Available memory and graphics hardware still constrain capacity. Display previews may be downsampled (up to 4096 pixels on the longest side); original and embedded file contents are preserved.
