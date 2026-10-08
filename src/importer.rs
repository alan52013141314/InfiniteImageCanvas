use crate::media;
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

pub struct ImageFile {
    pub path: PathBuf,
    pub size: [f64; 2],
}
#[derive(Default)]
pub struct Batch {
    pub images: Vec<ImageFile>,
    pub warnings: Vec<String>,
}

fn supported(path: &Path) -> bool {
    path.extension().and_then(|s| s.to_str()).is_some_and(|s| {
        matches!(
            s.to_ascii_lowercase().as_str(),
            "png" | "jpg" | "jpeg" | "webp" | "bmp" | "gif"
        )
    })
}

pub fn scan(
    paths: Vec<PathBuf>,
    recursive: bool,
    cancelled: Arc<AtomicBool>,
    found: Arc<AtomicUsize>,
) -> Batch {
    let mut result = Batch::default();
    let mut visited_dirs = HashSet::new();
    let mut visited_files = HashSet::new();
    // Explicit files retain input order. Directory entries are sorted for reproducible sequential mode.
    let mut pending: Vec<(PathBuf, bool)> = paths.into_iter().rev().map(|p| (p, true)).collect();
    while let Some((path, explicit)) = pending.pop() {
        if cancelled.load(Ordering::Relaxed) {
            break;
        }
        let metadata = match std::fs::metadata(&path) {
            Ok(m) => m,
            Err(e) => {
                result.warnings.push(format!("{}：{e}", path.display()));
                continue;
            }
        };
        if metadata.is_dir() {
            if !explicit && !recursive {
                continue;
            }
            if !explicit
                && std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink())
            {
                continue;
            }
            let identity = std::fs::canonicalize(&path).unwrap_or(path.clone());
            if !visited_dirs.insert(identity) {
                continue;
            }
            match std::fs::read_dir(&path) {
                Ok(entries) => {
                    let mut children = Vec::new();
                    for entry in entries {
                        match entry {
                            Ok(e) => children.push(e.path()),
                            Err(e) => result.warnings.push(format!("{}：{e}", path.display())),
                        }
                    }
                    children.sort_by_cached_key(|p| p.to_string_lossy().to_lowercase());
                    pending.extend(children.into_iter().rev().map(|p| (p, false)));
                }
                Err(e) => result.warnings.push(format!("{}：{e}", path.display())),
            }
        } else if metadata.is_file() && (explicit || supported(&path)) {
            let identity = std::fs::canonicalize(&path).unwrap_or(path.clone());
            if !visited_files.insert(identity) {
                continue;
            }
            match media::dimensions(&path) {
                Ok((w, h)) if w > 0 && h > 0 => {
                    result.images.push(ImageFile {
                        path,
                        size: [w as f64, h as f64],
                    });
                    found.store(result.images.len(), Ordering::Relaxed);
                }
                Ok(_) => result
                    .warnings
                    .push(format!("{}：無效尺寸", path.display())),
                Err(e) => result.warnings.push(format!("{}：{e}", path.display())),
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn folders_filter_sort_recurse_and_deduplicate() {
        let dir = tempfile::tempdir().unwrap();
        let child = dir.path().join("child");
        std::fs::create_dir(&child).unwrap();
        for p in [
            dir.path().join("b.png"),
            dir.path().join("a.PNG"),
            child.join("c.png"),
        ] {
            image::RgbImage::new(10, 20)
                .save_with_format(p, image::ImageFormat::Png)
                .unwrap();
        }
        std::fs::write(dir.path().join("readme.txt"), "ignore").unwrap();
        std::fs::write(dir.path().join("broken.png"), "bad").unwrap();
        let scan_all = |recursive| {
            scan(
                vec![dir.path().into(), dir.path().join("a.PNG")],
                recursive,
                Arc::new(AtomicBool::new(false)),
                Arc::new(AtomicUsize::new(0)),
            )
        };
        let shallow = scan_all(false);
        assert_eq!(shallow.images.len(), 2);
        assert_eq!(shallow.images[0].path.file_name().unwrap(), "a.PNG");
        assert_eq!(shallow.warnings.len(), 1);
        assert_eq!(scan_all(true).images.len(), 3);
        assert!(
            scan(
                vec![dir.path().into()],
                true,
                Arc::new(AtomicBool::new(true)),
                Arc::new(AtomicUsize::new(0))
            )
            .images
            .is_empty()
        );
    }
}
