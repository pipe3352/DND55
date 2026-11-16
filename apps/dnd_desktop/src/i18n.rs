//! Localization. User-facing UI strings are looked up by key from `i18n/<lang>.json`;
//! missing keys fall back to English, then to the key itself (which makes gaps visible).

use bevy::prelude::*;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    En,
    It,
}

#[derive(Resource)]
pub struct I18n {
    pub lang: Lang,
    en: HashMap<String, String>,
    it: HashMap<String, String>,
}

impl Default for I18n {
    fn default() -> Self {
        let parse =
            |s: &str| serde_json::from_str::<HashMap<String, String>>(s).expect("valid i18n json");
        I18n {
            lang: Lang::En,
            en: parse(include_str!("../i18n/en.json")),
            it: parse(include_str!("../i18n/it.json")),
        }
    }
}

impl I18n {
    pub fn t<'a>(&'a self, key: &'a str) -> &'a str {
        let table = match self.lang {
            Lang::En => &self.en,
            Lang::It => &self.it,
        };
        table
            .get(key)
            .or_else(|| self.en.get(key))
            .map(|s| s.as_str())
            .unwrap_or(key)
    }

    #[cfg(test)]
    pub fn keys(&self, lang: Lang) -> Vec<&String> {
        match lang {
            Lang::En => self.en.keys().collect(),
            Lang::It => self.it.keys().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn italian_covers_every_english_key() {
        let i = I18n::default();
        let it: std::collections::HashSet<_> = i.keys(Lang::It).into_iter().collect();
        let missing: Vec<_> = i
            .keys(Lang::En)
            .into_iter()
            .filter(|k| !it.contains(k))
            .collect();
        assert!(missing.is_empty(), "missing Italian keys: {missing:?}");
    }

    #[test]
    fn every_key_used_in_source_exists() {
        let i = I18n::default();
        let src_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/src");
        let mut missing = vec![];
        for entry in std::fs::read_dir(src_dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|e| e != "rs") {
                continue;
            }
            let text = std::fs::read_to_string(path).unwrap();
            for part in text.split(".t(\"").skip(1) {
                let key = part.split('"').next().unwrap();
                if i.t(key) == key {
                    missing.push(key.to_string());
                }
            }
        }
        missing.sort();
        missing.dedup();
        assert!(missing.is_empty(), "keys missing from en.json: {missing:?}");
    }
}
