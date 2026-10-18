use crate::{domain::*, platform};
use rusqlite::{params, Connection};
use serde::{de::DeserializeOwned, Serialize};
use std::{
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
};

pub struct Store {
    conn: Mutex<Connection>,
    pub root: PathBuf,
}
impl Store {
    pub fn open(root: &Path) -> Result<Self> {
        std::fs::create_dir_all(root.join("blobs"))?;
        std::fs::create_dir_all(root.join("tasks"))?;
        let conn = Connection::open(root.join("local-agent.sqlite"))?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON;
          CREATE TABLE IF NOT EXISTS records(kind TEXT NOT NULL,id TEXT NOT NULL,json TEXT NOT NULL,PRIMARY KEY(kind,id));
          CREATE TABLE IF NOT EXISTS schema_migrations(version INTEGER PRIMARY KEY);
          INSERT OR IGNORE INTO schema_migrations VALUES(1);")?;
        Ok(Self {
            conn: Mutex::new(conn),
            root: root.to_path_buf(),
        })
    }
    pub fn put<T: Serialize>(&self, kind: &str, id: &str, data: &T) -> Result<()> {
        let json = serde_json::to_string(data)?;
        self.conn.lock().map_err(|_| AppError::new("STORAGE", "Task storage is busy."))?.execute("INSERT INTO records(kind,id,json) VALUES(?1,?2,?3) ON CONFLICT(kind,id) DO UPDATE SET json=excluded.json", params![kind,id,json])?;
        Ok(())
    }
    pub fn get<T: DeserializeOwned>(&self, kind: &str, id: &str) -> Result<T> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::new("STORAGE", "Task storage is busy."))?;
        let json: String = conn
            .query_row(
                "SELECT json FROM records WHERE kind=?1 AND id=?2",
                params![kind, id],
                |r| r.get(0),
            )
            .map_err(|e| {
                AppError::new("NOT_FOUND", "This item is no longer available.").detail(e)
            })?;
        Ok(serde_json::from_str(&json)?)
    }
    pub fn list<T: DeserializeOwned>(&self, kind: &str) -> Result<Vec<T>> {
        self.list_limit(kind, i64::MAX)
    }
    pub fn list_limit<T: DeserializeOwned>(&self, kind: &str, limit: i64) -> Result<Vec<T>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::new("STORAGE", "Task storage is busy."))?;
        let mut stmt =
            conn.prepare("SELECT json FROM records WHERE kind=?1 ORDER BY rowid DESC LIMIT ?2")?;
        let rows = stmt.query_map(params![kind, limit.max(0)], |r| r.get::<_, String>(0))?;
        rows.map(|r| Ok(serde_json::from_str(&r?)?)).collect()
    }
    pub fn visible_tasks(&self) -> Result<Vec<Task>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::new("STORAGE", "Task storage is busy."))?;
        // Never hide an outstanding proposal or an operation that can need Undo/recovery.
        let mut stmt = conn.prepare("SELECT json FROM records WHERE kind='tasks' AND (rowid IN (SELECT rowid FROM records WHERE kind='tasks' ORDER BY rowid DESC LIMIT 100) OR json_extract(json,'$.changeState') IN ('proposed','applied','recovery_required')) ORDER BY rowid DESC")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        rows.map(|r| Ok(serde_json::from_str(&r?)?)).collect()
    }
    pub fn blob(&self, bytes: &[u8]) -> Result<String> {
        let digest = hash(bytes);
        let path = self.root.join("blobs").join(&digest);
        if !path.exists() {
            let sealed = platform::seal(bytes)?;
            let temp = self.root.join("blobs").join(format!("{}.tmp", id()));
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)?;
            file.write_all(&sealed)?;
            file.sync_all()?;
            drop(file);
            match std::fs::rename(&temp, &path) {
                Ok(_) => {}
                Err(_) if path.exists() => {
                    std::fs::remove_file(temp)?;
                }
                Err(e) => return Err(e.into()),
            }
        }
        Ok(digest)
    }
    pub fn bytes(&self, digest: &str) -> Result<Vec<u8>> {
        if digest.len() != 64 || !digest.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(AppError::new(
                "BLOB_ID",
                "The recovery reference is invalid.",
            ));
        }
        let bytes = platform::unseal(&std::fs::read(self.root.join("blobs").join(digest))?)?;
        if hash(&bytes) != digest {
            return Err(AppError::new(
                "BLOB_CORRUPT",
                "Recovery data failed its integrity check. The project has not been overwritten.",
            ));
        }
        Ok(bytes)
    }
    pub fn settings(&self) -> Settings {
        let mut settings: Settings = self.get("settings", "main").unwrap_or_else(|_| Settings {
            theme: "dark".into(),
            context_size: 8192,
            ..Default::default()
        });
        if settings.model_profiles.balanced.is_none() {
            settings.model_profiles.balanced = settings.model.clone();
        }
        crate::registry::migrate(&mut settings);
        settings
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn encrypted_blobs_and_persistent_records() {
        let tmp = tempfile::tempdir().unwrap();
        let store = Store::open(tmp.path()).unwrap();
        let digest = store.blob(b"private source").unwrap();
        assert_eq!(store.bytes(&digest).unwrap(), b"private source");
        assert!(!std::fs::read(tmp.path().join("blobs").join(&digest))
            .unwrap()
            .windows(14)
            .any(|v| v == b"private source"));
        store.put("test", "x", &vec![1, 2]).unwrap();
        drop(store);
        let store = Store::open(tmp.path()).unwrap();
        assert_eq!(store.get::<Vec<i32>>("test", "x").unwrap(), vec![1, 2]);
    }
    #[test]
    fn desktop_instance_lock_is_exclusive_and_released() {
        let tmp = tempfile::tempdir().unwrap();
        let lock = platform::lock_instance(tmp.path()).unwrap();
        assert!(platform::lock_instance(tmp.path()).is_err());
        drop(lock);
        assert!(platform::lock_instance(tmp.path()).is_ok());
    }
    #[test]
    fn legacy_model_becomes_balanced_without_losing_preferences() {
        let tmp = tempfile::tempdir().unwrap();
        let store = Store::open(tmp.path()).unwrap();
        store.put("settings","main",&serde_json::json!({"theme":"light","model":"existing:8b","contextSize":8192,"nodePath":null,"pythonPath":null,"gitPath":null,"lastProjectId":"project"})).unwrap();
        let settings = store.settings();
        assert_eq!(
            settings.model_profiles.balanced.as_deref(),
            Some("existing:8b")
        );
        assert!(!settings.model_profiles.strong_enabled);
        assert_eq!(settings.last_project_id.as_deref(), Some("project"));
        assert_eq!(settings.theme, "light");
    }
}
