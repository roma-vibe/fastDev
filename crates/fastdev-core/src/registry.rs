//! Registry of created projects (SQLite, `fastdev.db` in the data folder).

use std::collections::{BTreeMap, HashSet};
use std::path::Path;
use std::sync::Mutex;

use indexmap::IndexMap;
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SetupStatus {
    Ok,
    Failed,
    Skipped,
}

impl SetupStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
        }
    }

    fn parse(value: &str) -> Self {
        match value {
            "failed" => Self::Failed,
            "skipped" => Self::Skipped,
            _ => Self::Ok,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRecord {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub path: String,
    pub skeleton_id: Option<String>,
    pub skeleton_version: Option<String>,
    pub features: BTreeMap<String, bool>,
    pub ports: IndexMap<String, u16>,
    pub setup_status: SetupStatus,
    pub created_at: String,
    pub opened_at: Option<String>,
}

pub struct Registry {
    conn: Mutex<Connection>,
}

const MIGRATIONS: &[&str] = &[r"
    CREATE TABLE projects (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        slug TEXT NOT NULL,
        path TEXT NOT NULL UNIQUE,
        skeleton_id TEXT,
        skeleton_version TEXT,
        features TEXT NOT NULL DEFAULT '{}',
        ports TEXT NOT NULL DEFAULT '{}',
        setup_status TEXT NOT NULL DEFAULT 'ok',
        created_at TEXT NOT NULL,
        opened_at TEXT
    );
"];

impl Registry {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        Self::init(conn)
    }

    pub fn in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
        for (index, sql) in MIGRATIONS.iter().enumerate().skip(usize::try_from(version).unwrap_or(0)) {
            conn.execute_batch(sql)?;
            conn.pragma_update(None, "user_version", i64::try_from(index + 1).unwrap_or(i64::MAX))?;
        }
        Ok(Self { conn: Mutex::new(conn) })
    }

    fn row(row: &Row<'_>) -> rusqlite::Result<ProjectRecord> {
        let features: String = row.get("features")?;
        let ports: String = row.get("ports")?;
        let setup: String = row.get("setup_status")?;
        Ok(ProjectRecord {
            id: row.get("id")?,
            name: row.get("name")?,
            slug: row.get("slug")?,
            path: row.get("path")?,
            skeleton_id: row.get("skeleton_id")?,
            skeleton_version: row.get("skeleton_version")?,
            features: serde_json::from_str(&features).unwrap_or_default(),
            ports: serde_json::from_str(&ports).unwrap_or_default(),
            setup_status: SetupStatus::parse(&setup),
            created_at: row.get("created_at")?,
            opened_at: row.get("opened_at")?,
        })
    }

    pub fn list(&self) -> Result<Vec<ProjectRecord>> {
        let conn = self.conn.lock().expect("registry");
        let mut stmt = conn.prepare("SELECT * FROM projects ORDER BY COALESCE(opened_at, created_at) DESC")?;
        let rows = stmt.query_map([], Self::row)?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn get(&self, id: &str) -> Result<Option<ProjectRecord>> {
        let conn = self.conn.lock().expect("registry");
        Ok(conn.query_row("SELECT * FROM projects WHERE id = ?1", [id], Self::row).optional()?)
    }

    /// Finds a project by id, slug or path.
    pub fn find(&self, reference: &str) -> Result<ProjectRecord> {
        let reference = reference.trim();
        let path = reference.trim_end_matches('/');
        let conn = self.conn.lock().expect("registry");
        let found = conn
            .query_row(
                "SELECT * FROM projects WHERE id = ?1 OR path = ?2 ORDER BY id = ?1 DESC LIMIT 1",
                params![reference, path],
                Self::row,
            )
            .optional()?;
        if let Some(record) = found {
            return Ok(record);
        }
        let mut stmt = conn.prepare("SELECT * FROM projects WHERE slug = ?1")?;
        let mut matches = stmt.query_map([reference], Self::row)?.collect::<rusqlite::Result<Vec<_>>>()?;
        match matches.len() {
            0 => Err(Error::not_found(format!("no project \"{reference}\"; use list_projects to see ids"))),
            1 => Ok(matches.remove(0)),
            _ => Err(Error::conflict(format!("several projects have slug \"{reference}\"; use the id or path"))),
        }
    }

    pub fn find_by_path(&self, path: &str) -> Result<Option<ProjectRecord>> {
        let conn = self.conn.lock().expect("registry");
        Ok(conn.query_row("SELECT * FROM projects WHERE path = ?1", [path], Self::row).optional()?)
    }

    pub fn insert(&self, record: &ProjectRecord) -> Result<()> {
        let conn = self.conn.lock().expect("registry");
        conn.execute(
            "INSERT INTO projects (id, name, slug, path, skeleton_id, skeleton_version, features, ports, setup_status, created_at, opened_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                record.id,
                record.name,
                record.slug,
                record.path,
                record.skeleton_id,
                record.skeleton_version,
                serde_json::to_string(&record.features)?,
                serde_json::to_string(&record.ports)?,
                record.setup_status.as_str(),
                record.created_at,
                record.opened_at,
            ],
        )
        .map_err(|err| match err {
            rusqlite::Error::SqliteFailure(e, _) if e.code == rusqlite::ErrorCode::ConstraintViolation => {
                Error::conflict(format!("{} is already registered", record.path))
            }
            other => other.into(),
        })?;
        Ok(())
    }

    pub fn set_setup_status(&self, id: &str, status: SetupStatus) -> Result<()> {
        let conn = self.conn.lock().expect("registry");
        conn.execute("UPDATE projects SET setup_status = ?2 WHERE id = ?1", params![id, status.as_str()])?;
        Ok(())
    }

    pub fn set_path(&self, id: &str, path: &str) -> Result<()> {
        let conn = self.conn.lock().expect("registry");
        conn.execute("UPDATE projects SET path = ?2 WHERE id = ?1", params![id, path])?;
        Ok(())
    }

    pub fn touch(&self, id: &str, at: &str) -> Result<()> {
        let conn = self.conn.lock().expect("registry");
        conn.execute("UPDATE projects SET opened_at = ?2 WHERE id = ?1", params![id, at])?;
        Ok(())
    }

    pub fn remove(&self, id: &str) -> Result<bool> {
        let conn = self.conn.lock().expect("registry");
        Ok(conn.execute("DELETE FROM projects WHERE id = ?1", [id])? > 0)
    }

    /// Ports assigned to registered projects (to avoid collisions between projects).
    pub fn taken_ports(&self) -> Result<HashSet<u16>> {
        Ok(self.list()?.into_iter().flat_map(|p| p.ports.into_values()).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(id: &str, slug: &str, path: &str) -> ProjectRecord {
        ProjectRecord {
            id: id.into(),
            name: slug.into(),
            slug: slug.into(),
            path: path.into(),
            skeleton_id: Some("node-vue".into()),
            skeleton_version: Some("1.0.0".into()),
            features: BTreeMap::new(),
            ports: IndexMap::from([("APP_PORT".into(), 3000)]),
            setup_status: SetupStatus::Ok,
            created_at: "2026-09-30T00:00:00Z".into(),
            opened_at: None,
        }
    }

    #[test]
    fn stores_and_finds_projects() {
        let reg = Registry::in_memory().unwrap();
        reg.insert(&record("a1", "shop", "/tmp/shop")).unwrap();
        reg.insert(&record("b2", "blog", "/tmp/blog")).unwrap();
        assert!(reg.insert(&record("c3", "dup", "/tmp/shop")).is_err());
        assert_eq!(reg.find("shop").unwrap().id, "a1");
        assert_eq!(reg.find("/tmp/blog/").unwrap().id, "b2");
        assert_eq!(reg.find("b2").unwrap().slug, "blog");
        assert!(reg.find("nope").is_err());
        assert!(reg.taken_ports().unwrap().contains(&3000));
        reg.set_setup_status("a1", SetupStatus::Failed).unwrap();
        assert_eq!(reg.get("a1").unwrap().unwrap().setup_status, SetupStatus::Failed);
        assert!(reg.remove("a1").unwrap());
        assert_eq!(reg.list().unwrap().len(), 1);
    }
}
