use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    #[default]
    #[serde(rename = "zh-Hant")]
    TraditionalChinese,
    #[serde(rename = "en")]
    English,
}

impl Language {
    /// Translate only application-owned errors, never arbitrary OS text or paths.
    pub fn error(self, error: &str) -> String {
        if self == Self::TraditionalChinese {
            return error.into();
        }
        let translated = match error {
            "沒有保存資料夾" => "No save directory",
            "不支援的版面版本" => "Unsupported canvas format version",
            "無效的畫布座標" => "Invalid canvas coordinates",
            "無效的圖片資料" => "Invalid image data",
            "圖片解碼失敗；請檢查來源檔案或可用記憶體" => {
                "Image decoding failed. Check the source file or available memory."
            }
            "GIF 沒有影格" => "GIF contains no frames",
            _ => {
                if let Some(path) = error.strip_suffix("：無效尺寸") {
                    return format!("{path}: Invalid dimensions");
                }
                if let Some(rest) = error.strip_prefix("可用記憶體不足：此圖片解碼預估需要 ")
                    && let Some((required, rest)) = rest.split_once(" MB，目前作業預算 ")
                    && let Some(available) = rest.strip_suffix(" MB；請釋放記憶體後重試")
                {
                    return format!(
                        "Insufficient memory: decoding needs approximately {required} MB; current budget is {available} MB. Free memory and reopen the canvas to retry."
                    );
                }
                return error.into();
            }
        };
        translated.into()
    }

    pub fn text(self, chinese: &'static str) -> &'static str {
        if self == Self::TraditionalChinese {
            return chinese;
        }
        match chinese {
            "此檔案已在其他分頁使用，請另選保存位置。" => {
                "This file is used by another tab. Choose a different save location."
            }
            "關閉分頁" => "Close tab",
            "重新命名" => "Rename",
            "刪除版面檔案" => "Delete canvas file",
            "請輸入有效的檔名" => "Enter a valid file name",
            "此檔名已被使用" => "This file name is already in use",
            "確認刪除" => "Confirm deletion",
            "確認改名" => "Confirm rename",
            "將刪除版面檔案及放棄此分頁的變更，無法復原。來源圖片會保留。" => {
                "Delete this canvas file and discard this tab's changes permanently. Source images will be kept."
            }
            "新建分頁" => "New tab",
            "保存後關閉" => "Save and close",
            "不保存並關閉" => "Close without saving",
            "關閉前要保存此分頁嗎？" => "Save this tab before closing?",
            "不保存只會放棄最近一次保存後的變更。" => {
                "Discard only changes since the last successful save."
            }
            "正在保存所有分頁…" => "Saving all tabs…",
            "將圖片拖入畫布，開始整理" => "Drop images onto the canvas to begin",
            "正在保存…" => "Saving…",
            "正在開啟版面…" => "Opening canvas…",
            "新建未命名版面" => "New untitled canvas",
            "圖片掃描工作意外中止；尚未加入圖片" => {
                "Image scan stopped unexpectedly. No images were added."
            }
            "保存工作意外中止，請重試" => "Saving stopped unexpectedly. Please retry.",
            "載入工作意外中止" => "Loading stopped unexpectedly.",
            "正在掃描圖片與子資料夾…" => "Scanning images and subfolders…",
            "請確認本批匯入圖片數量" => "Confirm the number of images to import",
            "已取消匯入，畫布保持原狀" => "Import cancelled. Canvas unchanged.",
            "資料夾中沒有可匯入的圖片" => "No supported images found in the folder",
            "剪貼簿內沒有圖片，請先複製圖片或截圖" => {
                "No image on the clipboard. Copy an image or screenshot first."
            }
            "圖片" => "Images",
            "匯入資料夾（包含所有子資料夾）" => {
                "Import folder (including subfolders)"
            }
            "圖片版面" => "Image canvas",
            "無限圖片畫布" => "Image Canvas",
            "檔案" => "File",
            "新建版面" => "New canvas",
            "開啟版面…" => "Open canvas…",
            "另存新檔…" => "Save as…",
            "匯入圖片" => "Import",
            "資料夾" => "Folder",
            "匯入資料夾及所有子資料夾中的圖片" => {
                "Import images from a folder and all its subfolders"
            }
            "貼上" => "Paste",
            "貼上剪貼簿圖片 · Ctrl+V" => "Paste clipboard image · Ctrl+V",
            "退出全螢幕" => "Exit fullscreen",
            "全螢幕" => "Fullscreen",
            "設定" => "Settings",
            "保存" => "Save",
            "保存版面 · Ctrl+S" => "Save canvas · Ctrl+S",
            "未命名版面" => "Untitled canvas",
            "尚有變更" => "Unsaved changes",
            "已保存" => "Saved",
            "編輯" => "Edit",
            "復原    Ctrl+Z" => "Undo    Ctrl+Z",
            "重做    Ctrl+Y" => "Redo    Ctrl+Y",
            "移到最前" => "Bring to front",
            "移到最後" => "Send to back",
            "刪除選取圖片" => "Delete selected images",
            "顯示全部" => "Fit all",
            "中鍵平移  ·  滾輪縮放  ·  Shift 多選" => {
                "Middle drag: pan · Wheel: zoom · Shift: select"
            }
            "拖曳圖片角點等比縮放；在空白處拖曳框選。Ctrl+Z 復原，Ctrl+S 保存。" => {
                "Drag corners to resize proportionally; drag empty space to select. Ctrl+Z: undo; Ctrl+S: save."
            }
            "載入中…" => "Loading…",
            "掃描圖片" => "Scanning images",
            "取消掃描" => "Cancel scan",
            "確認大量圖片匯入" => "Confirm image import",
            "本批超過 100 張，確認後才會一次加入畫布。" => {
                "This batch exceeds 100 images. Confirm to add them together."
            }
            "取消" => "Cancel",
            "圖片保存" => "Image storage",
            "引用原圖路徑 · 檔案較小" => {
                "Reference original files · Smaller canvas file"
            }
            "封裝原圖 · 可攜至其他電腦" => "Embed original files · Portable canvas",
            "匯入順序" => "Import order",
            "依載入順序" => "Sequential",
            "隨機順序" => "Random",
            "圖片自動貼齊排列：3 張為 2＋1，5 張為 3＋2。" => {
                "Images touch side by side: 3 use 2+1 rows; 5 use 3+2."
            }
            "自動保存" => "Autosave",
            "每分鐘保存變更，啟動時恢復最後版面。" => {
                "Save changes every minute. Restore the last canvas on startup."
            }
            "引用模式需要保留原圖；剪貼簿圖片會保存至 assets。" => {
                "Keep original files in reference mode. Clipboard images are stored in assets."
            }
            "需要處理" => "Action required",
            "重試保存" => "Retry save",
            "另選保存位置" => "Choose another save location",
            "返回畫布" => "Back to canvas",
            "拖入圖片或資料夾" => "Drop images or folders",
            "自由排列，隨時保存。" => "Arrange freely. Save whenever you need.",
            "Ctrl + V 貼上圖片     ·     F11 全螢幕" => {
                "Ctrl + V to paste     ·     F11 for fullscreen"
            }
            _ => chinese,
        }
    }
}

// Both branches are checked by Rust's format parser, including named arguments.
#[macro_export]
macro_rules! localized {
    ($lang:expr, $zh:literal, $en:literal $(, $arg:expr)* $(,)?) => {
        if $lang == $crate::i18n::Language::English { format!($en $(, $arg)*) }
        else { format!($zh $(, $arg)*) }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_static_interface_keys_have_english_translations() {
        for source in [
            include_str!("app.rs"),
            include_str!("app/tabs.rs"),
            include_str!("theme.rs"),
        ] {
            let production = source.split("#[cfg(test)]\nmod tests").next().unwrap();
            for segment in production.split(".text(").skip(1) {
                if let Some(rest) = segment.trim_start().strip_prefix('"') {
                    let key = rest.split('"').next().unwrap();
                    if key.chars().any(|c| ('\u{3400}'..='\u{9fff}').contains(&c)) {
                        let translated = Language::English.text(key);
                        assert_ne!(translated, key, "untranslated: {key}");
                        assert!(
                            !translated
                                .chars()
                                .any(|c| ('\u{3400}'..='\u{9fff}').contains(&c))
                        );
                        assert_eq!(Language::TraditionalChinese.text(key), key);
                    }
                }
            }
        }
    }
    #[test]
    fn application_errors_translate_without_rewriting_paths() {
        assert_eq!(
            Language::English.error("不支援的版面版本"),
            "Unsupported canvas format version"
        );
        let path = "C:/images/不支援的版面版本.png: access denied";
        assert_eq!(Language::English.error(path), path);
        assert_eq!(Language::TraditionalChinese.error(path), path);
        let memory = Language::English.error(
            "可用記憶體不足：此圖片解碼預估需要 100 MB，目前作業預算 50 MB；請釋放記憶體後重試",
        );
        assert!(
            memory.contains("100 MB")
                && memory.contains("50 MB")
                && memory.starts_with("Insufficient memory")
        );
    }
}
