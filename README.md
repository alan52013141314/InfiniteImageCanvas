# InfiniteImageCanvas

A portable Rust image canvas for Windows. Arrange images in an infinite 2D space, play animated GIFs, and save complete layouts. English and 繁體中文 interfaces are available in Settings. No server or account is needed.

## Download

Get **v0.3.0** from [GitHub Releases](https://github.com/alan52013141314/InfiniteImageCanvas/releases). Extract the archive into a writable folder and launch the executable. Close any older running version before replacing it. Keep your `save file` folder when upgrading.

Windows 11 x64 is tested. Windows 10 x64 is a compatibility target and has not been tested. An OpenGL 3.3 graphics driver is required. The CRT is statically linked; the executable directly depends only on Windows system DLLs.

## Features

- Drag files or entire folders into the canvas; recursively scan subfolders.
- PNG, JPEG, WebP, BMP and animated GIF; paste screenshots with Ctrl+V.
- Pan, zoom, drag, proportional resize, multi-select, layers and undo/redo.
- Compact batch layout; sequential or randomized import order.
- Confirm batches larger than 100 images before adding them.
- Reference original files or embed them into a portable `.icanvas` file.
- Autosave changed layouts every minute, save on normal close, restore at startup.
- Independent canvas tabs on the second row; restore all tabs and the active tab. Edit stays on the first row.
- F11 borderless fullscreen with controls revealed at the top/bottom edges.
- English / Traditional Chinese, remembered across restarts.

Original image count and resolution have no configured limit; capacity depends on hardware. Display previews may be downsampled; source files are preserved.

## Documentation

- [English guide](docs/USER-GUIDE.en.md) / [繁體中文操作說明](docs/USER-GUIDE.md)
- [Changelog](CHANGELOG.md)
- [Requirements](docs/PR-001.md) / [v0.3.0 requirements](docs/PR-003.md)
- [Work plan](docs/WORKPLAN-001.md) / [v0.3.0 plan](docs/WORKPLAN-003.md)
- [Verification and limitations](docs/VERIFICATION.md)
- [Third-party notices](THIRD-PARTY-NOTICES.txt)

## Build

Install Rust stable and the MSVC C++ build tools for Windows. Dependencies are pinned in `Cargo.lock`.

```powershell
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked --release
```

The executable is written to `target/release/infinite-image-canvas.exe`. A local `build.ps1` helper also supports `test`, `lint`, and `build`.

Optional load checks (generate synthetic local data):

```powershell
cargo test --locked --release benchmark_unique_images_and_large_canvas -- --ignored --nocapture
cargo test --locked --release sustained_gif_decoding -- --ignored --nocapture
```

## 繁體中文

Windows 免安裝無限圖片畫布，支援拖入圖片／整個資料夾、GIF 動畫、圖片移動及等比縮放、批次排列、保存與恢復。設定可切換 English／繁體中文；標題及設定會顯示版本號。

從 Releases 下載並解壓縮執行。升級前正常關閉舊版，保留 `save file`。引用模式需要保留原圖；要搬到其他電腦時，請選封裝原圖模式。
