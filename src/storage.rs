use crate::model::Canvas;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

#[derive(Default, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub language: crate::i18n::Language,
    pub packed: bool,
    pub last: Option<PathBuf>,
    pub unnamed: bool,
    #[serde(default)]
    pub random_order: bool,
}

pub fn error(e: impl std::fmt::Display) -> String {
    e.to_string()
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path.parent().ok_or("沒有保存資料夾")?;
    fs::create_dir_all(parent).map_err(error)?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent).map_err(error)?;
    tmp.write_all(bytes).map_err(error)?;
    tmp.as_file().sync_all().map_err(error)?;
    tmp.persist(path).map_err(error)?;
    Ok(())
}

pub fn save(canvas: &Canvas, path: &Path) -> Result<(), String> {
    canvas.validate()?;
    let parent = path.parent().ok_or("沒有保存資料夾")?;
    fs::create_dir_all(parent).map_err(error)?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent).map_err(error)?;
    {
        let mut zip = ZipWriter::new(tmp.as_file_mut());
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored)
            .large_file(true);
        let mut manifest = canvas.clone();
        if canvas.packed {
            let mut mapped = std::collections::HashMap::new();
            for item in &mut manifest.items {
                if let Some(entry) = mapped.get(&item.source) {
                    item.source = PathBuf::from(entry);
                    continue;
                }
                let entry = format!("assets/{}", item.id);
                let mut file = File::open(&item.source)
                    .map_err(|e| format!("{}：{e}", item.source.display()))?;
                zip.start_file(&entry, options).map_err(error)?;
                std::io::copy(&mut file, &mut zip).map_err(error)?;
                mapped.insert(item.source.clone(), entry.clone());
                item.source = entry.into();
            }
        }
        zip.start_file("canvas.json", options).map_err(error)?;
        serde_json::to_writer(&mut zip, &manifest).map_err(error)?;
        zip.finish().map_err(error)?;
    }
    tmp.as_file().sync_all().map_err(error)?;
    tmp.persist(path).map_err(error)?;
    Ok(())
}

pub fn load(path: &Path, assets_dir: &Path) -> Result<Canvas, String> {
    let mut zip = ZipArchive::new(File::open(path).map_err(error)?).map_err(error)?;
    let mut canvas: Canvas =
        serde_json::from_reader(zip.by_name("canvas.json").map_err(error)?).map_err(error)?;
    canvas.validate()?;
    if canvas.packed {
        let dir = assets_dir.join(uuid::Uuid::new_v4().to_string());
        fs::create_dir_all(&dir).map_err(error)?;
        let mut mapped = std::collections::HashMap::new();
        for item in &mut canvas.items {
            if let Some(target) = mapped.get(&item.source) {
                item.source = PathBuf::from(target);
                continue;
            }
            let name = item.source.to_string_lossy().replace('\\', "/");
            let mut entry = zip.by_name(&name).map_err(error)?;
            // Never trust archive paths as output paths.
            let target = dir.join(format!("{}.image", item.id));
            let mut file = File::create(&target).map_err(error)?;
            std::io::copy(&mut entry, &mut file).map_err(error)?;
            file.sync_all().map_err(error)?;
            mapped.insert(item.source.clone(), target.clone());
            item.source = target;
        }
    } else {
        for item in &mut canvas.items {
            if item.source.is_relative() {
                item.source = path.parent().unwrap().join(&item.source);
            }
        }
    }
    Ok(canvas)
}

pub fn next_name(dir: &Path, date: &str) -> PathBuf {
    for n in 1u64.. {
        let p = dir.join(format!("canvas_{date}_{n:02}.icanvas"));
        if !p.exists() {
            return p;
        }
    }
    unreachable!()
}
pub fn read_settings(dir: &Path) -> Settings {
    let mut paths = [dir.join("settings.json"), fallback_settings(dir)];
    paths.sort_by_key(|p| std::cmp::Reverse(fs::metadata(p).and_then(|m| m.modified()).ok()));
    paths
        .iter()
        .find_map(|p| {
            fs::read(p)
                .ok()
                .and_then(|b| serde_json::from_slice(&b).ok())
        })
        .unwrap_or_default()
}
pub fn write_settings(dir: &Path, settings: &Settings) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(settings).map_err(error)?;
    atomic_write(&dir.join("settings.json"), &bytes)
        .or_else(|_| atomic_write(&fallback_settings(dir), &bytes))
}

fn fallback_settings(dir: &Path) -> PathBuf {
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    dir.hash(&mut hash);
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("InfiniteImageCanvas")
        .join(format!("{:x}-settings.json", hash.finish()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Item;
    #[test]
    fn reference_and_packed_roundtrip_with_source_removed() {
        let d = tempfile::tempdir().unwrap();
        let src = d.path().join("中文.png");
        fs::write(&src, b"original image bytes").unwrap();
        let mut c = Canvas::default();
        c.items.push(Item {
            id: 1,
            source: src.clone(),
            position: [-88.0, 99.0],
            size: [30.0, 20.0],
        });
        let path = d.path().join("test.icanvas");
        save(&c, &path).unwrap();
        assert_eq!(c, load(&path, d.path()).unwrap());
        c.packed = true;
        save(&c, &path).unwrap();
        fs::remove_file(src).unwrap();
        let loaded = load(&path, d.path()).unwrap();
        assert_eq!(
            fs::read(&loaded.items[0].source).unwrap(),
            b"original image bytes"
        );
        assert_eq!(loaded.items[0].position, [-88.0, 99.0]);
        let mut reference = loaded;
        reference.packed = false;
        save(&reference, &path).unwrap();
        assert_eq!(reference, load(&path, d.path()).unwrap());
    }
    #[test]
    fn failed_pack_preserves_previous_file() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("a.icanvas");
        let mut c = Canvas::default();
        save(&c, &p).unwrap();
        let old = fs::read(&p).unwrap();
        c.packed = true;
        c.items.push(Item {
            id: 1,
            source: d.path().join("missing"),
            position: [0.0; 2],
            size: [2.0; 2],
        });
        assert!(save(&c, &p).is_err());
        assert_eq!(old, fs::read(p).unwrap());
    }
    #[test]
    fn names_increment_and_versions_rejected() {
        let d = tempfile::tempdir().unwrap();
        let first = next_name(d.path(), "20260910");
        assert_eq!(first.file_stem().unwrap(), "canvas_20260910_01");
        fs::write(first, []).unwrap();
        assert_eq!(
            next_name(d.path(), "20260910").file_stem().unwrap(),
            "canvas_20260910_02"
        );
        assert!(
            Canvas {
                version: 99,
                ..Default::default()
            }
            .validate()
            .is_err()
        );
    }
}
