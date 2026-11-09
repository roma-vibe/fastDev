//! The shared lists of ports in use by projects on this Mac, `~/Library/Application Support/
//! dev-ports/`: every tool that gives projects ports keeps its own `<owner>.json` there and reads
//! the others' (My Simple Shell, which runs the projects, writes `my-simple-shell.json` and moves
//! a clashing port of a project before it starts). fastDev gives new projects ports that no file
//! lists, and publishes the ports of its registered projects in `fastdev.json`.
//!
//! File shape (all tools): `{ "version": 1, "owner": "fastdev", "updatedAt": <unix ms>,
//! "reserved": [[first, last], …], "claims": [{ "port", "project", "path", "key" }] }`.

use std::collections::HashSet;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::envfile;
use crate::error::Result;
use crate::registry::ProjectRecord;
use crate::util;

/// Our file in the folder.
pub const OWN_FILE: &str = "fastdev.json";

/// A larger "reserved" range is a broken file, not a reservation.
const MAX_RESERVED: usize = 1000;

/// Words of a key that name a server the project connects to rather than one it runs
/// (`POSTGRES_PORT`, `DB_PORT`, `REDIS_PORT`…): those ports are not the project's own.
const EXTERNAL_WORDS: &[&str] = &[
    "POSTGRES",
    "POSTGRESQL",
    "PG",
    "DB",
    "DATABASE",
    "MYSQL",
    "MARIADB",
    "MONGO",
    "MONGODB",
    "REDIS",
    "VALKEY",
    "MEMCACHED",
    "SMTP",
    "MAIL",
    "IMAP",
    "POP3",
    "AMQP",
    "RABBIT",
    "RABBITMQ",
    "KAFKA",
    "NATS",
    "ELASTIC",
    "ELASTICSEARCH",
    "OPENSEARCH",
    "CLICKHOUSE",
    "MINIO",
    "S3",
    "OLLAMA",
    "LDAP",
];

/// The folder under the user's home folder.
pub fn default_dir(home: &Path) -> PathBuf {
    home.join("Library/Application Support/dev-ports")
}

/// The folder for the app and the CLI: `None` with an isolated data folder (`FASTDEV_HOME`), whose
/// projects are not the user's.
pub fn shared_dir() -> Option<PathBuf> {
    std::env::var_os("FASTDEV_HOME").is_none().then(|| default_dir(&util::home_dir()))
}

/// True for `PORT` and `<NAME>_PORT` keys of a project's own servers.
pub fn is_own_port_key(key: &str) -> bool {
    if key == "PORT" {
        return true;
    }
    match key.strip_suffix("_PORT") {
        Some(name) if !name.is_empty() => !name.split('_').any(|word| EXTERNAL_WORDS.contains(&word)),
        _ => false,
    }
}

#[derive(Deserialize)]
struct OtherFile {
    #[serde(default)]
    reserved: Vec<[u16; 2]>,
    #[serde(default)]
    claims: Vec<OtherClaim>,
}

#[derive(Deserialize)]
struct OtherClaim {
    port: u16,
}

/// The ports the other tools' files in `dir` list, with their reserved ranges. Files that cannot
/// be read or parsed are skipped (logged): a broken neighbour must not stop project creation.
pub fn taken_by_others(dir: &Path) -> HashSet<u16> {
    let mut taken = HashSet::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return taken;
    };
    for path in entries.flatten().map(|entry| entry.path()) {
        if path.file_name() == Some(OsStr::new(OWN_FILE)) || path.extension() != Some(OsStr::new("json")) {
            continue;
        }
        let file = std::fs::read(&path)
            .map_err(|err| err.to_string())
            .and_then(|bytes| serde_json::from_slice::<OtherFile>(&bytes).map_err(|err| err.to_string()));
        let file = match file {
            Ok(file) => file,
            Err(err) => {
                log::warn!("skipping the port claims in {}: {err}", path.display());
                continue;
            }
        };
        for [first, last] in file.reserved {
            if first <= last && usize::from(last - first) < MAX_RESERVED {
                taken.extend(first..=last);
            }
        }
        taken.extend(file.claims.into_iter().map(|claim| claim.port));
    }
    taken
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OwnFile {
    version: u32,
    owner: &'static str,
    updated_at: i64,
    reserved: Vec<[u16; 2]>,
    claims: Vec<OwnClaim>,
}

#[derive(Serialize, Debug, PartialEq, Eq)]
struct OwnClaim {
    port: u16,
    project: String,
    path: String,
    key: String,
}

/// The own ports of the projects: their port keys, with the value their `.env` has now (it may
/// have moved since the project was created) or else the one they were created with.
fn claims_of(projects: &[ProjectRecord]) -> Vec<OwnClaim> {
    let mut claims = Vec::new();
    for project in projects {
        let env = envfile::read_values(&Path::new(&project.path).join(".env"));
        for (key, port) in &project.ports {
            if !is_own_port_key(key) {
                continue;
            }
            let port = env.get(key).and_then(|value| value.trim().parse().ok()).unwrap_or(*port);
            claims.push(OwnClaim { port, project: project.name.clone(), path: project.path.clone(), key: key.clone() });
        }
    }
    claims.sort_by(|a, b| a.port.cmp(&b.port).then_with(|| a.project.cmp(&b.project)));
    claims
}

/// Writes `fastdev.json` in `dir` with the ports of the projects (atomically).
pub fn publish(dir: &Path, projects: &[ProjectRecord]) -> Result<()> {
    let file = OwnFile {
        version: 1,
        owner: "fastdev",
        updated_at: chrono::Utc::now().timestamp_millis(),
        reserved: Vec::new(),
        claims: claims_of(projects),
    };
    let json = serde_json::to_vec_pretty(&file).map_err(|err| crate::error::Error::internal(err.to_string()))?;
    util::write_atomic(&dir.join(OWN_FILE), &json)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use indexmap::IndexMap;

    use super::*;
    use crate::registry::SetupStatus;

    fn record(name: &str, path: &Path, ports: &[(&str, u16)]) -> ProjectRecord {
        ProjectRecord {
            id: name.into(),
            name: name.into(),
            slug: name.into(),
            path: path.display().to_string(),
            skeleton_id: None,
            skeleton_version: None,
            features: BTreeMap::new(),
            ports: ports.iter().map(|(k, p)| (k.to_string(), *p)).collect::<IndexMap<_, _>>(),
            setup_status: SetupStatus::Ok,
            created_at: String::new(),
            opened_at: None,
        }
    }

    #[test]
    fn own_ports_and_ports_of_other_servers() {
        for key in ["PORT", "EDGE_PORT", "WEB_PORT", "MCP_PORT", "APP_DEV_PORT"] {
            assert!(is_own_port_key(key), "{key}");
        }
        for key in ["POSTGRES_PORT", "DB_PORT", "REDIS_PORT", "PGPORT", "_PORT", "PORTS"] {
            assert!(!is_own_port_key(key), "{key}");
        }
    }

    #[test]
    fn reads_the_other_tools_and_skips_ours_and_broken_files() {
        let dir = tempfile::tempdir().unwrap();
        let write = |name: &str, text: &str| std::fs::write(dir.path().join(name), text).unwrap();
        write(
            "my-simple-shell.json",
            r#"{"owner":"my-simple-shell","reserved":[[47821,47823],[1024,65535],[9,1]],
               "claims":[{"port":8700,"project":"shop","path":"/p","key":"EDGE_PORT"}]}"#,
        );
        write(OWN_FILE, r#"{"claims":[{"port":3000}]}"#);
        write("broken.json", "{");
        write("notes.txt", r#"{"claims":[{"port":4000}]}"#);
        assert_eq!(taken_by_others(dir.path()), HashSet::from([8700, 47821, 47822, 47823]));
        assert!(taken_by_others(&dir.path().join("missing")).is_empty());
    }

    #[test]
    fn publishes_the_current_ports_of_the_projects() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("shop");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::write(project.join(".env"), "EDGE_PORT=8718\nPOSTGRES_PORT=5432\n").unwrap();
        let records = [
            record("shop", &project, &[("EDGE_PORT", 8700), ("WEB_PORT", 5177), ("POSTGRES_PORT", 5432)]),
            record("gone", &dir.path().join("gone"), &[("APP_PORT", 3000)]),
        ];
        let folder = dir.path().join("dev-ports");
        publish(&folder, &records).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&std::fs::read(folder.join(OWN_FILE)).unwrap()).unwrap();
        assert_eq!(value["owner"], "fastdev");
        let claims: Vec<(u64, &str, &str)> = value["claims"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| (c["port"].as_u64().unwrap(), c["project"].as_str().unwrap(), c["key"].as_str().unwrap()))
            .collect();
        assert_eq!(claims, [(3000, "gone", "APP_PORT"), (5177, "shop", "WEB_PORT"), (8718, "shop", "EDGE_PORT")]);
    }
}
