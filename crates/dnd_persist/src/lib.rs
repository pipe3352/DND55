//! # dnd_persist — campaign saves
//!
//! * One SQLite database file per installation (`campaigns.db`), WAL mode, `synchronous=FULL`.
//! * Every save is a single transaction: a crash mid-save leaves the previous saves intact.
//! * Each save stores the full serialized [`GameState`] plus a SHA-256 checksum, verified on load.
//! * Schema migrations use `PRAGMA user_version`; game-state format migrations run on JSON before decoding.
//! * Autosaves rotate (the newest [`AUTOSAVE_KEEP`] are kept); manual saves and checkpoints are never pruned.
//! * Portraits are stored content-addressed (SHA-256) and shared between saves.

use dnd_rules::character::CharacterSheet;
use dnd_session::assets::AssetStore;
use dnd_session::model::{GameState, STATE_FORMAT_VERSION};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

pub const AUTOSAVE_KEEP: usize = 5;
const SCHEMA_VERSION: i64 = 1;

#[derive(Debug, thiserror::Error)]
pub enum PersistError {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("serialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("save {0} failed its integrity check")]
    Corrupt(i64),
    #[error("save was written by a newer version of the game (format {0})")]
    TooNew(u32),
    #[error("database schema {0} is newer than this build supports")]
    SchemaTooNew(i64),
    #[error("not found")]
    NotFound,
}

pub type Result<T> = std::result::Result<T, PersistError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SaveKind {
    Manual,
    Auto,
    Checkpoint,
}

impl SaveKind {
    fn as_str(self) -> &'static str {
        match self {
            SaveKind::Manual => "manual",
            SaveKind::Auto => "auto",
            SaveKind::Checkpoint => "checkpoint",
        }
    }
    fn parse(s: &str) -> Self {
        match s {
            "auto" => SaveKind::Auto,
            "checkpoint" => SaveKind::Checkpoint,
            _ => SaveKind::Manual,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CampaignInfo {
    pub id: i64,
    pub name: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveInfo {
    pub id: i64,
    pub campaign_id: i64,
    pub kind: SaveKind,
    pub label: String,
    pub created_at: i64,
    pub format_version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredCharacter {
    pub id: i64,
    pub name: String,
    pub sheet: CharacterSheet,
    pub portrait_png: Option<Vec<u8>>,
    pub updated_at: i64,
}

/// Portable single-file export of a campaign (latest save + its assets).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CampaignExport {
    pub export_format: u32,
    pub app: String,
    pub ruleset: String,
    pub name: String,
    pub state: serde_json::Value,
    /// hash -> hex-encoded PNG
    pub assets: Vec<(String, String)>,
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn sha(s: &[u8]) -> String {
    hex::encode(Sha256::digest(s))
}

/// Upgrades a serialized `GameState` from an older format version to the current one.
pub fn migrate_state(mut value: serde_json::Value) -> Result<serde_json::Value> {
    let v = value
        .get("format_version")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    if v > STATE_FORMAT_VERSION {
        return Err(PersistError::TooNew(v));
    }
    // Each future format bump adds a step here, e.g. `if v < 2 { ...; v = 2 }`.
    if v < 1 {
        // Format 0 (pre-release) had no `settings`; default them.
        if let Some(obj) = value.as_object_mut() {
            obj.entry("settings").or_insert_with(|| {
                serde_json::to_value(dnd_session::model::Settings::default()).expect("serializable")
            });
            obj.insert("format_version".into(), 1.into());
        }
    }
    Ok(value)
}

pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let conn = Connection::open(path)?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "FULL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let mut s = Store { conn };
        s.migrate()?;
        Ok(s)
    }

    fn migrate(&mut self) -> Result<()> {
        let v: i64 = self
            .conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if v > SCHEMA_VERSION {
            return Err(PersistError::SchemaTooNew(v));
        }
        let tx = self.conn.transaction()?;
        if v < 1 {
            tx.execute_batch(
                "CREATE TABLE campaigns (
                    id INTEGER PRIMARY KEY,
                    name TEXT NOT NULL,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                 );
                 CREATE TABLE saves (
                    id INTEGER PRIMARY KEY,
                    campaign_id INTEGER NOT NULL REFERENCES campaigns(id) ON DELETE CASCADE,
                    kind TEXT NOT NULL CHECK (kind IN ('manual','auto','checkpoint')),
                    label TEXT NOT NULL,
                    created_at INTEGER NOT NULL,
                    format_version INTEGER NOT NULL,
                    state_json TEXT NOT NULL,
                    sha256 TEXT NOT NULL
                 );
                 CREATE INDEX saves_by_campaign ON saves(campaign_id, id);
                 CREATE TABLE assets (
                    hash TEXT PRIMARY KEY,
                    bytes BLOB NOT NULL
                 );
                 CREATE TABLE characters (
                    id INTEGER PRIMARY KEY,
                    name TEXT NOT NULL,
                    sheet_json TEXT NOT NULL,
                    portrait BLOB,
                    updated_at INTEGER NOT NULL
                 );",
            )?;
        }
        tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        tx.commit()?;
        Ok(())
    }

    // ---------------------------------------------------------------- campaigns

    pub fn create_campaign(&self, name: &str) -> Result<i64> {
        let t = now();
        self.conn.execute(
            "INSERT INTO campaigns (name, created_at, updated_at) VALUES (?1, ?2, ?2)",
            params![name, t],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn list_campaigns(&self) -> Result<Vec<CampaignInfo>> {
        let mut st = self.conn.prepare("SELECT id, name, created_at, updated_at FROM campaigns ORDER BY updated_at DESC, id DESC")?;
        let rows = st.query_map([], |r| {
            Ok(CampaignInfo {
                id: r.get(0)?,
                name: r.get(1)?,
                created_at: r.get(2)?,
                updated_at: r.get(3)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn delete_campaign(&self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM campaigns WHERE id = ?1", [id])?;
        Ok(())
    }

    // ---------------------------------------------------------------- saves

    /// Saves atomically. Autosaves beyond [`AUTOSAVE_KEEP`] are pruned in the same transaction.
    pub fn save(
        &mut self,
        campaign: i64,
        kind: SaveKind,
        label: &str,
        state: &GameState,
        assets: &AssetStore,
    ) -> Result<i64> {
        let json = serde_json::to_string(state)?;
        let hash = sha(json.as_bytes());
        let t = now();
        let tx = self.conn.transaction()?;
        let exists: Option<i64> = tx
            .query_row("SELECT id FROM campaigns WHERE id = ?1", [campaign], |r| {
                r.get(0)
            })
            .optional()?;
        if exists.is_none() {
            return Err(PersistError::NotFound);
        }
        // Store only assets referenced by the state.
        for c in state.creatures.values() {
            if let Some(h) = &c.portrait
                && let Some(bytes) = assets.get(h)
            {
                tx.execute(
                    "INSERT OR IGNORE INTO assets (hash, bytes) VALUES (?1, ?2)",
                    params![h, bytes],
                )?;
            }
        }
        tx.execute(
            "INSERT INTO saves (campaign_id, kind, label, created_at, format_version, state_json, sha256) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![campaign, kind.as_str(), label, t, state.format_version, json, hash],
        )?;
        let id = tx.last_insert_rowid();
        tx.execute(
            "UPDATE campaigns SET updated_at = ?2, name = ?3 WHERE id = ?1",
            params![campaign, t, state.campaign_name],
        )?;
        if kind == SaveKind::Auto {
            tx.execute(
                "DELETE FROM saves WHERE campaign_id = ?1 AND kind = 'auto' AND id NOT IN
                   (SELECT id FROM saves WHERE campaign_id = ?1 AND kind = 'auto' ORDER BY id DESC LIMIT ?2)",
                params![campaign, AUTOSAVE_KEEP as i64],
            )?;
        }
        tx.commit()?;
        Ok(id)
    }

    pub fn list_saves(&self, campaign: i64) -> Result<Vec<SaveInfo>> {
        let mut st = self.conn.prepare("SELECT id, campaign_id, kind, label, created_at, format_version FROM saves WHERE campaign_id = ?1 ORDER BY id DESC")?;
        let rows = st.query_map([campaign], |r| {
            Ok(SaveInfo {
                id: r.get(0)?,
                campaign_id: r.get(1)?,
                kind: SaveKind::parse(&r.get::<_, String>(2)?),
                label: r.get(3)?,
                created_at: r.get(4)?,
                format_version: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Loads and verifies one save.
    pub fn load(&self, save_id: i64) -> Result<(GameState, AssetStore)> {
        let (json, hash): (String, String) = self
            .conn
            .query_row(
                "SELECT state_json, sha256 FROM saves WHERE id = ?1",
                [save_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?
            .ok_or(PersistError::NotFound)?;
        if sha(json.as_bytes()) != hash {
            return Err(PersistError::Corrupt(save_id));
        }
        let value: serde_json::Value = serde_json::from_str(&json)?;
        let state: GameState = serde_json::from_value(migrate_state(value)?)?;
        let mut assets = AssetStore::default();
        for c in state.creatures.values() {
            if let Some(h) = &c.portrait {
                let bytes: Option<Vec<u8>> = self
                    .conn
                    .query_row("SELECT bytes FROM assets WHERE hash = ?1", [h], |r| {
                        r.get(0)
                    })
                    .optional()?;
                if let Some(b) = bytes
                    && sha(&b) == *h
                {
                    assets.blobs.insert(h.clone(), b);
                }
            }
        }
        Ok((state, assets))
    }

    /// Crash recovery: the newest save that passes its integrity check.
    pub fn load_latest_valid(&self, campaign: i64) -> Result<(SaveInfo, GameState, AssetStore)> {
        for info in self.list_saves(campaign)? {
            match self.load(info.id) {
                Ok((s, a)) => return Ok((info, s, a)),
                Err(PersistError::Corrupt(_)) | Err(PersistError::Json(_)) => continue,
                Err(e) => return Err(e),
            }
        }
        Err(PersistError::NotFound)
    }

    /// Consistent online backup of the whole database to `dest`.
    pub fn backup_to(&self, dest: &Path) -> Result<()> {
        if dest.exists() {
            std::fs::remove_file(dest)?;
        }
        self.conn
            .execute("VACUUM INTO ?1", [dest.to_string_lossy()])?;
        Ok(())
    }

    // ---------------------------------------------------------------- export / import

    pub fn export_campaign(&self, campaign: i64) -> Result<CampaignExport> {
        let (_, state, assets) = self.load_latest_valid(campaign)?;
        Ok(CampaignExport {
            export_format: 1,
            app: concat!("dnd55 ", env!("CARGO_PKG_VERSION")).into(),
            ruleset: state.ruleset.clone(),
            name: state.campaign_name.clone(),
            state: serde_json::to_value(&state)?,
            assets: assets
                .blobs
                .iter()
                .map(|(h, b)| (h.clone(), hex::encode(b)))
                .collect(),
        })
    }

    pub fn import_campaign(&mut self, export: &CampaignExport) -> Result<i64> {
        let state: GameState = serde_json::from_value(migrate_state(export.state.clone())?)?;
        let mut assets = AssetStore::default();
        for (h, hexed) in &export.assets {
            let bytes = hex::decode(hexed).map_err(|_| PersistError::Corrupt(0))?;
            if sha(&bytes) != *h {
                return Err(PersistError::Corrupt(0));
            }
            assets.blobs.insert(h.clone(), bytes);
        }
        let id = self.create_campaign(&state.campaign_name)?;
        self.save(id, SaveKind::Checkpoint, "Imported", &state, &assets)?;
        Ok(id)
    }

    // ---------------------------------------------------------------- character library

    pub fn save_character(
        &self,
        id: Option<i64>,
        sheet: &CharacterSheet,
        portrait_png: Option<&[u8]>,
    ) -> Result<i64> {
        let json = serde_json::to_string(sheet)?;
        let t = now();
        match id {
            Some(id) => {
                let n = self.conn.execute(
                    "UPDATE characters SET name = ?2, sheet_json = ?3, portrait = ?4, updated_at = ?5 WHERE id = ?1",
                    params![id, sheet.name, json, portrait_png, t],
                )?;
                if n == 0 {
                    return Err(PersistError::NotFound);
                }
                Ok(id)
            }
            None => {
                self.conn.execute(
                    "INSERT INTO characters (name, sheet_json, portrait, updated_at) VALUES (?1, ?2, ?3, ?4)",
                    params![sheet.name, json, portrait_png, t],
                )?;
                Ok(self.conn.last_insert_rowid())
            }
        }
    }

    pub fn list_characters(&self) -> Result<Vec<StoredCharacter>> {
        let mut st = self.conn.prepare("SELECT id, name, sheet_json, portrait, updated_at FROM characters ORDER BY updated_at DESC")?;
        let rows = st.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<Vec<u8>>>(3)?,
                r.get::<_, i64>(4)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, name, json, portrait_png, updated_at) = row?;
            // Skip rows that fail to decode rather than failing the whole list.
            if let Ok(sheet) = serde_json::from_str(&json) {
                out.push(StoredCharacter {
                    id,
                    name,
                    sheet,
                    portrait_png,
                    updated_at,
                });
            }
        }
        Ok(out)
    }

    pub fn delete_character(&self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM characters WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Test hook: direct SQL access (e.g. to simulate corruption).
    #[doc(hidden)]
    pub fn raw(&self) -> &Connection {
        &self.conn
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dnd_session::command::{Command, DmCommand};
    use dnd_session::demo::demo_campaign;
    use dnd_session::model::Principal;

    fn sample() -> dnd_session::Session {
        let mut s = demo_campaign();
        let p = s.add_player("Alice");
        s.apply(
            Principal::Player(p),
            Command::SubmitCharacter {
                sheet: Box::new(CharacterSheet::example_fighter("Alice")),
                portrait_png: Some(test_png()),
            },
        )
        .unwrap();
        s
    }

    fn test_png() -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(8, 8, image::Rgba([200, 30, 30, 255]));
        let mut out = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut out, image::ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }

    #[test]
    fn save_and_load_roundtrip_with_portraits() {
        let mut store = Store::open_in_memory().unwrap();
        let s = sample();
        let c = store.create_campaign(&s.state.campaign_name).unwrap();
        let id = store
            .save(c, SaveKind::Manual, "Session 1", &s.state, &s.assets)
            .unwrap();
        let (state, assets) = store.load(id).unwrap();
        assert_eq!(state, s.state);
        assert_eq!(assets, s.assets);
        assert_eq!(assets.blobs.len(), 1);
    }

    #[test]
    fn autosaves_rotate_but_manual_saves_stay() {
        let mut store = Store::open_in_memory().unwrap();
        let s = sample();
        let c = store.create_campaign("x").unwrap();
        store
            .save(c, SaveKind::Manual, "keep me", &s.state, &s.assets)
            .unwrap();
        for i in 0..12 {
            store
                .save(c, SaveKind::Auto, &format!("auto {i}"), &s.state, &s.assets)
                .unwrap();
        }
        let saves = store.list_saves(c).unwrap();
        assert_eq!(
            saves.iter().filter(|s| s.kind == SaveKind::Auto).count(),
            AUTOSAVE_KEEP
        );
        assert!(saves.iter().any(|s| s.label == "keep me"));
        assert_eq!(saves[0].label, "auto 11");
    }

    #[test]
    fn corrupted_save_is_detected_and_recovery_uses_previous() {
        let mut store = Store::open_in_memory().unwrap();
        let mut s = sample();
        let c = store.create_campaign("x").unwrap();
        let good = store
            .save(c, SaveKind::Manual, "good", &s.state, &s.assets)
            .unwrap();
        s.apply(Principal::Dm, Command::Dm(DmCommand::StartSession))
            .unwrap();
        let bad = store
            .save(c, SaveKind::Auto, "bad", &s.state, &s.assets)
            .unwrap();
        store.raw().execute("UPDATE saves SET state_json = replace(state_json, 'Sunken', 'Broken') WHERE id = ?1", [bad]).unwrap();
        assert!(matches!(store.load(bad), Err(PersistError::Corrupt(_))));
        let (info, state, _) = store.load_latest_valid(c).unwrap();
        assert_eq!(info.id, good);
        assert!(!state.session_started);
    }

    #[test]
    fn interrupted_transaction_leaves_no_partial_save() {
        let mut store = Store::open_in_memory().unwrap();
        let s = sample();
        let c = store.create_campaign("x").unwrap();
        store
            .save(c, SaveKind::Manual, "one", &s.state, &s.assets)
            .unwrap();
        {
            // Simulate a crash: start writing, never commit.
            let tx = store.conn.transaction().unwrap();
            tx.execute("INSERT INTO saves (campaign_id, kind, label, created_at, format_version, state_json, sha256) VALUES (?1,'manual','half',0,1,'{',  'x')", [c]).unwrap();
            drop(tx); // rollback
        }
        assert_eq!(store.list_saves(c).unwrap().len(), 1);
    }

    #[test]
    fn file_database_survives_reopen_and_backup() {
        let dir = std::env::temp_dir().join(format!("dnd55-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let db = dir.join("campaigns.db");
        let s = sample();
        let save_id;
        {
            let mut store = Store::open(&db).unwrap();
            let c = store.create_campaign("x").unwrap();
            save_id = store
                .save(c, SaveKind::Manual, "a", &s.state, &s.assets)
                .unwrap();
            store.backup_to(&dir.join("backup.db")).unwrap();
        }
        let store = Store::open(&db).unwrap();
        assert_eq!(store.load(save_id).unwrap().0, s.state);
        let backup = Store::open(&dir.join("backup.db")).unwrap();
        assert_eq!(backup.load(save_id).unwrap().0, s.state);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn export_import_roundtrip() {
        let mut store = Store::open_in_memory().unwrap();
        let s = sample();
        let c = store.create_campaign("x").unwrap();
        store
            .save(c, SaveKind::Manual, "a", &s.state, &s.assets)
            .unwrap();
        let exported = serde_json::to_string(&store.export_campaign(c).unwrap()).unwrap();
        let mut other = Store::open_in_memory().unwrap();
        let id = other
            .import_campaign(&serde_json::from_str(&exported).unwrap())
            .unwrap();
        let (_, state, assets) = other.load_latest_valid(id).unwrap();
        assert_eq!(state, s.state);
        assert_eq!(assets.blobs.len(), 1);
    }

    #[test]
    fn migrates_old_format_and_rejects_newer() {
        let s = sample();
        let mut v = serde_json::to_value(&s.state).unwrap();
        v.as_object_mut().unwrap().remove("settings");
        v["format_version"] = 0.into();
        let migrated: GameState = serde_json::from_value(migrate_state(v).unwrap()).unwrap();
        assert_eq!(migrated.format_version, 1);
        let mut v = serde_json::to_value(&s.state).unwrap();
        v["format_version"] = 99.into();
        assert!(matches!(migrate_state(v), Err(PersistError::TooNew(99))));
    }

    #[test]
    fn character_library() {
        let store = Store::open_in_memory().unwrap();
        let mut sheet = CharacterSheet::example_fighter("Brunhild");
        let id = store.save_character(None, &sheet, None).unwrap();
        sheet.biography = "Born in the mountains.".into();
        store
            .save_character(Some(id), &sheet, Some(&[1, 2, 3]))
            .unwrap();
        let list = store.list_characters().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].sheet.biography, "Born in the mountains.");
        assert_eq!(list[0].portrait_png.as_deref(), Some(&[1u8, 2, 3][..]));
    }
}
