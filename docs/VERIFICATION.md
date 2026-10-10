# 驗證記錄

日期：2026-10-09。發行檔：dist/InfiniteImageCanvas-latest.exe。

## v0.2.0 — 雙語與版本

- 33 項一般測試通過；格式及 Clippy 零警告，release 編譯成功。兩項既有負載測試未因語言變更重跑，其紀錄為 v0.1.0 實測。
- English／繁體中文切換立即套用，單獨保存語言偏好；重開及保存作業後仍保留選擇，舊設定預設繁中。
- 靜態介面字串翻譯完整性與應用程式錯誤翻譯測試通過，檔案路徑不被改寫。作業系統產生的錯誤文字沿用系統語言。
- 兩種語言在 720／960／1280 寬與 100%／125%／150%／200% 縮放的主要按鈕均完整顯示。設定使用捲動面板。
- 原生 Windows 英文主畫面及視窗標題 InfiniteImageCanvas v0.2.0 已目視檢查。
- Cargo 套件版本為 0.2.0；版面資料格式版本維持 1。

以下為 v0.1.0 基礎功能與負載驗證紀錄。

## 已完成

- Rust 格式化、Clippy 全目標零警告、release 編譯成功。
- 自動測試 30 項通過，0 失敗；兩項負載測試預設略過，均已另行執行成功。
- 保存涵蓋引用／封裝往返、來源移除後封裝重開、剪貼簿素材保存、日期序號、未命名自動保存後手動命名、正常退出、寫入失敗、保存中修改及切換版面。
- 操作涵蓋游標中心縮放、圖片拖曳、同一幀快速拖曳、中鍵平移、角點等比縮放、多選、層次及復原重做。
- 匯入涵蓋單張落點、1～5 張排列、不同尺寸排列、隨機順序、資料夾遞迴、去重、取消掃描、100 張直接加入及 101 張等待確認。
- PNG、JPEG、WebP、BMP、GIF 解碼；GIF 幀推進與循環；20,000 像素寬圖片測試通過。
- Windows 11 原生視窗實測：圖片顯示與 GIF 動畫、拖移、保存重開、全螢幕切換；新版工具列與設定面板已目視檢查。
- 新版原生資料夾選擇實測：子資料夾內 101 張圖片顯示確認提示；確認前仍為原有 6 張，確認後一次成為 107 張。
- 執行檔採靜態 CRT；相依檢查僅列 Windows 系統 DLL，記錄於 test-output/binary-dependencies.txt。

## 本輪修正與驗證

- Ctrl+V：同一剪貼簿圖片在舊版快捷鍵未加入、按鈕成功；改用視窗按鍵事件後，原生快速 Ctrl+V 由 108 張增加至 109 張。已保存的貼上圖片重開後仍顯示。
- 全螢幕：原生 1920×1080 視窗驗證中央隱藏、上方浮出工具列、下方浮出狀態列且上方隱藏；圖片位置一致。
- 新增全螢幕邊緣顯示與畫布範圍不變的回歸測試。修正首次實測發現的畫布覆蓋工具列繪製層次。

## 負載測量

Windows 11 本機，100／1,000／10,000 個獨立 480×320 PNG 檔；release 測試。時間為單次實測毫秒，不是容量上限或效能保證。

| 圖片數 | 掃描與加入 | 首幀 | 引用保存 | 讀取 |
|---|---:|---:|---:|---:|
| 100 | 9 | 4 | 82 | 9 |
| 1,000 | 94 | 4 | 119 | 14 |
| 10,000 | 917 | 4 | 896 | 30 |

首幀測試採視野裁切，並非同時解碼所有原圖。原始數據為 test-output/benchmark.csv。CPU 為 Intel Core i5-13400；RAM 總容量未取得。

## 持續 GIF 負載

128 個独立 128×128 GIF、各兩個影格，連續 60.01 秒共解碼 203,264 影格。逐幀驗證尺寸、延遲與交替顏色；串流狀態維持 128 份，估計狀態預算 33,554,432 bytes。程序工作集兩次取樣為 27,488,256 與 27,443,200 bytes，私有記憶體為 20,119,552 與 20,033,536 bytes。這是解碼器負載檢查，不等同所有圖片同時高解析度渲染或数小時實機壓力測試。原始結果：test-output/gif-stress.txt。

## 需求核對

| 需求 | 當前證據 |
|---|---|
| R1 Rust／Windows／免安裝／繁中 | Cargo 專案、release exe、Windows 11 原生介面、系統 DLL 核對；Windows 10 驗證已豁免 |
| R2 無限畫布／無人為匯入上限 | f64 座標轉換、視野裁切、大圖與 10,000 張測試；硬體限制明列 |
| R3 匯入與 GIF | 格式解碼測試、原生檔案操作／Ctrl+V 實測、GIF 循環測試 |
| R4～R5 圖片與畫布操作 | 指標事件、快速拖曳、等比縮放、多選／層次／歷史測試及原生拖移 |
| R6～R7 保存內容與模式 | storage 往返／封裝移除來源／模式切換／剪貼簿持久化測試 |
| R8～R13 命名／位置／自動保存／失敗處理 | 保存狀態測試、原生正常關閉與重開、失敗保留原檔測試 |
| R14 規劃與授權範圍 | PR-001、WORKPLAN-001；無伺服器或額外服務 |
| R15 全螢幕／自動隱藏 | F11 指令測試、滿版不移位測試、原生上下工具列顯示實測 |
| R16～R17 排列與順序 | 1～5 張／異尺寸／落點／隨機置換與設定持久化測試 |
| R18～R19 資料夾／大量確認 | 遞迴與取消測試、100／101 張邊界、原生 101 張確認後整批加入 |
| R20 專業簡潔 UI | 原生主畫面／設定面板檢查、最小視窗與四種縮放渲染檢查 |

## 尚未驗證及限制

- Windows 10 尚未實測。使用者已回覆「noneed」，不再要求 Windows 10 實機驗證作為交付條件。乾淨系統未另行實測；免安裝相依性以靜態 CRT 與系統 DLL 檢查佐證。
- 已完成 720／960／1280 邏輯像素寬、100%／125%／150%／200% 縮放的渲染檢查，所有主要按鈕均在可視範圍。實體多顯示器切換與數小時負載未實測，不宣稱涵蓋所有硬體組合。
- 原生 Ctrl+V 截圖已實測成功加入；先前快速按鍵遺漏已改成 Windows 按鍵事件記錄。素材重開另由自動測試覆蓋。
- 原圖不設固定尺寸上限，但顯示預覽最長邊為 4096；原始檔與封裝內容不受改動。
- 斷電或強制終止只能恢復最近一次成功保存；目前不聲稱其間變更仍可恢復。

以上區分程式驗證、原生操作實測及待驗證環境；不將未執行的驗收項目列為通過。
## v0.3.0 — 2026-10-09

- 40 automated tests passed; two opt-in load benchmarks were not rerun.
- New coverage: independent document state and undo history; all-tab restoration and active tab; unique recovery/manual-save targets; old settings migration; duplicate-open selection; close Save/Discard/Cancel; save failure retention; all-tab exit saving.
- Actual egui pointer events exercise the +, tab selection, ×, Cancel and Save-and-close controls. Both languages render with nine tabs at 720 × 480; existing fullscreen and UI scale regressions pass.
- Formatting, strict Clippy lint and the Windows x64 release build passed.
- This version's native window visual inspection was not completed: the desktop tool's launch approval timed out. Earlier native verification in this document refers to earlier versions, not v0.3.0 tabs.

## v0.3.1 — 2026-10-09

43 automated tests passed; formatting, strict lint and release build passed. Two opt-in load benchmarks not rerun. New tests cover pointer drag reorder, right-click menu and deletion cancellation, actual file rename/deletion, collisions, missing-file rename and failed deletion retention, management-dialog autosave suspension, and restart order/active document.

The native Windows v0.3.1 test window was inspected: label and × now share one outline, with separate tabs and + spaced cleanly. Native right-click/deletion interaction was not fully exercised because user input interrupted the desktop session; these paths were verified with egui event and filesystem tests.

## v0.3.2 — 2026-10-10

47 automated tests pass; formatting and strict lint pass. New tests cover real egui image right-click, topmost-image targeting, cancel/confirm, actual temporary source deletion, all-open-tab cleanup, undo/redo and restart, failure retention, packed assets and pending-save exclusion. Source deletion tests touch temporary fixtures only. Native window interaction was not tested in this round. Two optional load benchmarks were not rerun.

## v0.4.0 — 2026-10-11

52 tests passed; formatting, strict lint and release build passed. New coverage includes actual offscreen decode, nearest priority, preview budget reduction/zero and settings persistence; blank-menu Settings/Gather; gather geometry and Undo; reading positions, arrow/wheel/Ctrl+wheel, original-layout save/restart, tab independence and source-delete integration. Two optional load benchmarks were not rerun.

Native isolated Windows inspection with generated page fixtures confirmed tab reading submenu, vertical layout, next-page arrow, blank-canvas Settings and preload controls. Horizontal mode, gathering and Ctrl+wheel were verified through automated events rather than a full native pass. Final mode-aware footer wording was adjusted after native inspection and then rebuilt/tested.
