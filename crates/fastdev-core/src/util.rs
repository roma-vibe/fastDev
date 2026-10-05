use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::error::{Error, IoContext, Result};

/// Folder names that never belong to a skeleton or a template copy.
pub const IGNORED_NAMES: &[&str] = &[".DS_Store", "node_modules", ".git"];

/// Build outputs and caches that validation rejects inside a skeleton.
pub const FORBIDDEN_DIRS: &[&str] =
    &["node_modules", "vendor", "target", ".venv", "venv", "__pycache__", ".git", "dist", ".next", ".nuxt"];

pub fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

pub fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

/// ASCII slug for folder and package names; Cyrillic and other scripts are transliterated.
pub fn slugify(name: &str) -> String {
    let slug = slug::slugify(name);
    slug.chars().take(64).collect::<String>().trim_matches('-').to_string()
}

/// Ids of skeletons: lowercase letters, digits and single dashes, starting with a letter.
pub fn is_valid_id(id: &str) -> bool {
    let bytes = id.as_bytes();
    !id.is_empty()
        && id.len() <= 64
        && bytes[0].is_ascii_lowercase()
        && !id.ends_with('-')
        && !id.contains("--")
        && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

pub fn is_valid_slug(slug: &str) -> bool {
    !slug.is_empty()
        && slug.len() <= 64
        && !slug.starts_with(['-', '.'])
        && slug.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_' || c == '.')
}

/// Writes through a temp file in the same folder, so readers never see half-written files.
pub fn write_atomic(path: &Path, content: &[u8]) -> Result<()> {
    let dir = path.parent().ok_or_else(|| Error::invalid(format!("{} has no parent", path.display())))?;
    fs::create_dir_all(dir).at(dir)?;
    let mut tmp = tempfile::NamedTempFile::new_in(dir).at(dir)?;
    tmp.write_all(content).at(path)?;
    // Temp files are created 0600; keep the mode of the file being replaced, or use the usual
    // 0644, so e.g. package.json stays readable by other users (Docker images run as `node`).
    let mode = fs::metadata(path).map(|m| m.permissions().mode() & 0o7777).unwrap_or(0o644);
    tmp.as_file().set_permissions(fs::Permissions::from_mode(mode)).at(path)?;
    tmp.as_file().sync_all().at(path)?;
    tmp.persist(path).map_err(|err| Error::io(path, err.error))?;
    Ok(())
}

pub fn random_hex(bytes: usize) -> String {
    let mut out = String::with_capacity(bytes * 2);
    while out.len() < bytes * 2 {
        out.push_str(&uuid::Uuid::new_v4().simple().to_string());
    }
    out.truncate(bytes * 2);
    out
}

pub fn short_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()[..12].to_string()
}

/// Stable 16-hex-digit FNV-1a hash of a string (cache folder names).
pub fn short_hash(text: &str) -> String {
    let hash =
        text.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3));
    format!("{hash:016x}")
}

pub fn sha256_file(path: &Path) -> Result<String> {
    let data = fs::read(path).at(path)?;
    Ok(hex::encode(Sha256::digest(&data)))
}

/// Relative paths (with `/` separators) of all files under `root`, sorted, skipping [`IGNORED_NAMES`].
pub fn list_files(root: &Path) -> Result<Vec<String>> {
    let mut files = Vec::new();
    let walker = WalkDir::new(root).follow_links(false).into_iter().filter_entry(|entry| {
        entry.depth() == 0 || !IGNORED_NAMES.contains(&entry.file_name().to_string_lossy().as_ref())
    });
    for entry in walker {
        let entry = entry.map_err(|err| Error::internal(format!("{}: {err}", root.display())))?;
        if entry.file_type().is_file() || entry.file_type().is_symlink() {
            files.push(relative(root, entry.path()));
        }
    }
    files.sort();
    Ok(files)
}

pub fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// Recursively copies `from` into `to` (created when missing), skipping [`IGNORED_NAMES`].
pub fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to).at(to)?;
    for rel in list_files(from)? {
        let src = from.join(&rel);
        let dst = to.join(&rel);
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent).at(parent)?;
        }
        fs::copy(&src, &dst).at(&src)?;
    }
    Ok(())
}

/// Combined hash of every file (path, executable bit and content) under `root`. The executable
/// bit is part of a file as much as its content: git keeps it, and a script without it fails.
pub fn hash_tree(root: &Path, skip: &[&str]) -> Result<String> {
    let mut hasher = Sha256::new();
    for rel in list_files(root)? {
        if skip.contains(&rel.as_str()) {
            continue;
        }
        let path = root.join(&rel);
        let executable = fs::metadata(&path).at(&path)?.permissions().mode() & 0o111 != 0;
        hasher.update(rel.as_bytes());
        hasher.update([0, u8::from(executable)]);
        hasher.update(fs::read(&path).at(&path)?);
        hasher.update([0]);
    }
    Ok(hex::encode(hasher.finalize()))
}

pub fn home_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
}

/// Expands a leading `~/`.
pub fn expand_home(path: &str) -> PathBuf {
    if path == "~" {
        return home_dir();
    }
    match path.strip_prefix("~/") {
        Some(rest) => home_dir().join(rest),
        None => PathBuf::from(path),
    }
}

pub fn is_empty_dir(path: &Path) -> bool {
    fs::read_dir(path)
        .map(|mut entries| entries.all(|entry| entry.map(|e| e.file_name() == ".DS_Store").unwrap_or(false)))
        .unwrap_or(false)
}

/// Whether an `http://` URL answers with any bytes within a few seconds (a listening server,
/// not just a Docker port proxy that accepts and closes). Other schemes: whether the port accepts.
pub fn http_answers(url: &str) -> bool {
    use std::io::{Read, Write};
    use std::net::{TcpStream, ToSocketAddrs};
    use std::time::Duration;

    let (http, rest) = match url.split_once("://") {
        Some(("http", rest)) => (true, rest),
        Some((_, rest)) => (false, rest),
        None => (true, url),
    };
    let (authority, path) = rest.split_once('/').map_or((rest, "/".to_string()), |(a, p)| (a, format!("/{p}")));
    let default_port = if http { 80 } else { 443 };
    let host_port = if authority.rsplit_once(':').is_some_and(|(_, p)| p.parse::<u16>().is_ok()) {
        authority.to_string()
    } else {
        format!("{authority}:{default_port}")
    };
    let Ok(addrs) = host_port.to_socket_addrs() else { return false };
    addrs.into_iter().any(|addr| {
        let Ok(mut stream) = TcpStream::connect_timeout(&addr, Duration::from_millis(800)) else { return false };
        if !http {
            return true;
        }
        let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
        let request = format!("GET {path} HTTP/1.0\r\nHost: {authority}\r\n\r\n");
        stream.write_all(request.as_bytes()).is_ok() && matches!(stream.read(&mut [0u8; 1]), Ok(n) if n > 0)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_transliterates() {
        assert_eq!(slugify("My Shop"), "my-shop");
        assert_eq!(slugify("Мой магазин"), "moi-magazin");
        assert_eq!(slugify("  Hello,  World!! "), "hello-world");
    }

    #[test]
    fn validates_ids() {
        assert!(is_valid_id("node-vue"));
        assert!(is_valid_id("laravel-vue-sqlite2"));
        assert!(!is_valid_id("Node"));
        assert!(!is_valid_id("1abc"));
        assert!(!is_valid_id("a--b"));
        assert!(!is_valid_id("a-"));
    }

    #[test]
    fn atomic_writes_keep_readable_modes() {
        let dir = tempfile::tempdir().unwrap();
        let new_file = dir.path().join("new.json");
        write_atomic(&new_file, b"{}").unwrap();
        assert_eq!(fs::metadata(&new_file).unwrap().permissions().mode() & 0o777, 0o644);
        let script = dir.path().join("run.sh");
        fs::write(&script, "#!/bin/sh\n").unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        write_atomic(&script, b"#!/bin/sh\necho hi\n").unwrap();
        assert_eq!(fs::metadata(&script).unwrap().permissions().mode() & 0o777, 0o755);
    }

    #[test]
    fn tree_hash_covers_the_executable_bit() {
        let tmp = tempfile::tempdir().unwrap();
        let script = tmp.path().join("scripts/run.sh");
        fs::create_dir_all(script.parent().unwrap()).unwrap();
        fs::write(&script, "#!/bin/sh\n").unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        let executable = hash_tree(tmp.path(), &[]).unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o644)).unwrap();
        assert_ne!(hash_tree(tmp.path(), &[]).unwrap(), executable);
        // Copies (verification snapshots) keep the bit, so they hash like the original.
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        let copy = tmp.path().join("copy");
        copy_dir(&tmp.path().join("scripts"), &copy.join("scripts")).unwrap();
        assert_eq!(hash_tree(&copy, &[]).unwrap(), executable);
    }

    #[test]
    fn random_hex_has_length() {
        assert_eq!(random_hex(32).len(), 64);
        assert_eq!(random_hex(5).len(), 10);
    }

    #[test]
    fn probes_http_servers() {
        use std::io::{Read, Write};
        let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = server.local_addr().unwrap().port();
        let handle = std::thread::spawn(move || {
            // First connection: accepted and closed without an answer (like a Docker port proxy).
            drop(server.accept().unwrap());
            let (mut stream, _) = server.accept().unwrap();
            let _ = stream.read(&mut [0u8; 256]);
            stream.write_all(b"HTTP/1.0 200 OK\r\n\r\n").unwrap();
        });
        let url = format!("http://127.0.0.1:{port}/app");
        assert!(!http_answers(&url));
        assert!(http_answers(&url));
        handle.join().unwrap();
        assert!(!http_answers(&format!("http://127.0.0.1:{port}")));
    }
}
