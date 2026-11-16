//! Importable art packs: PNG files that replace the built-in procedural art of map objects.
//!
//! Layout (in the data folder, `%APPDATA%\DND55\asset_packs` on Windows):
//! ```text
//! asset_packs/
//!   manifest.template.json      ← written by the game: every object id, ready to copy
//!   my_pack/manifest.json       ← { "name", "license", "author", "source", "objects": [{ "id", "file" }] }
//!   my_pack/oak.png
//! ```
//! A pack must state its license; packs without one are ignored (only use art you may
//! redistribute). Unknown ids, missing files and bad PNGs are reported in the log and skipped,
//! so the built-in art is always the fallback.

use bevy::image::Image;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub name: String,
    #[serde(default)]
    pub license: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub objects: Vec<Entry>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    pub file: String,
}

#[derive(Debug, Default)]
pub struct Packs {
    /// Object id → PNG file that replaces its art.
    pub art: HashMap<String, PathBuf>,
    /// (pack name, license) of every loaded pack.
    pub loaded: Vec<(String, String)>,
    pub problems: Vec<String>,
}

/// Reads every `*/manifest.json` under `root`. `known` are the valid object ids.
pub fn scan(root: &Path, known: &[String]) -> Packs {
    let mut out = Packs::default();
    let Ok(dirs) = std::fs::read_dir(root) else {
        return out;
    };
    let mut dirs: Vec<PathBuf> = dirs
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    for dir in dirs {
        let mf = dir.join("manifest.json");
        let Ok(text) = std::fs::read_to_string(&mf) else {
            continue;
        };
        let m: Manifest = match serde_json::from_str(&text) {
            Ok(m) => m,
            Err(e) => {
                out.problems.push(format!("{}: {e}", mf.display()));
                continue;
            }
        };
        if m.license.trim().is_empty() {
            out.problems
                .push(format!("{}: no license stated, pack ignored", m.name));
            continue;
        }
        for e in m.objects {
            let file = dir.join(&e.file);
            if !known.contains(&e.id) {
                out.problems
                    .push(format!("{}: unknown object id '{}'", m.name, e.id));
            } else if e.file.contains("..") || !file.is_file() {
                out.problems
                    .push(format!("{}: missing file '{}'", m.name, e.file));
            } else {
                out.art.insert(e.id, file);
            }
        }
        out.loaded.push((m.name, m.license));
    }
    out
}

/// Writes `manifest.template.json` listing every object id (refreshed when ids change).
pub fn write_template(root: &Path, known: &[String]) {
    let m = Manifest {
        name: "My art pack".into(),
        license: "CC-BY-4.0 (state the real license of your art)".into(),
        author: String::new(),
        source: String::new(),
        objects: known
            .iter()
            .map(|id| Entry {
                id: id.clone(),
                file: format!("{id}.png"),
            })
            .collect(),
    };
    let text = serde_json::to_string_pretty(&m).unwrap_or_default();
    let path = root.join("manifest.template.json");
    if std::fs::read_to_string(&path).ok().as_deref() != Some(text.as_str()) {
        let _ = std::fs::create_dir_all(root);
        let _ = std::fs::write(path, text);
    }
}

/// Decodes a pack PNG (at most 1024 px per side).
pub fn load_png(path: &Path) -> Option<Image> {
    let bytes = std::fs::read(path).ok()?;
    let img = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png).ok()?;
    let img = if img.width() > 1024 || img.height() > 1024 {
        img.thumbnail(1024, 1024)
    } else {
        img
    };
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    Some(crate::textures::new_image(w, h, rgba.into_raw()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packs_need_a_license_and_valid_ids_and_files() {
        let root = std::env::temp_dir().join(format!("dnd55-packs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let good = root.join("good");
        let bad = root.join("nolicense");
        std::fs::create_dir_all(&good).unwrap();
        std::fs::create_dir_all(&bad).unwrap();
        let png = image::RgbaImage::from_pixel(4, 4, image::Rgba([200, 10, 10, 255]));
        png.save(good.join("oak.png")).unwrap();
        std::fs::write(good.join("manifest.json"), r#"{"name":"Good","license":"CC0","objects":[
            {"id":"tree","file":"oak.png"},{"id":"nope","file":"oak.png"},{"id":"barrel","file":"missing.png"}]}"#).unwrap();
        std::fs::write(
            bad.join("manifest.json"),
            r#"{"name":"Bad","objects":[{"id":"tree","file":"x.png"}]}"#,
        )
        .unwrap();
        let known = vec!["tree".to_string(), "barrel".to_string()];
        let p = scan(&root, &known);
        assert_eq!(p.art.len(), 1);
        assert!(p.art.contains_key("tree"));
        assert_eq!(p.loaded, vec![("Good".to_string(), "CC0".to_string())]);
        assert_eq!(p.problems.len(), 3, "{:?}", p.problems);
        assert!(load_png(&p.art["tree"]).is_some());
        write_template(&root, &known);
        let t: Manifest = serde_json::from_str(
            &std::fs::read_to_string(root.join("manifest.template.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(t.objects.len(), 2);
        let _ = std::fs::remove_dir_all(&root);
    }
}
