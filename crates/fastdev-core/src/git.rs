//! Thin wrapper over the `git` CLI (user's git, credentials and config).

use std::path::Path;
use std::process::{Command, Stdio};

use crate::error::{Error, Result};
use crate::toolchain;

/// Tags of published versions: `v1.2.3`.
pub const TAG_PREFIX: &str = "v";

fn command(cwd: &Path) -> Command {
    let mut cmd = Command::new("git");
    cmd.current_dir(cwd)
        .env_clear()
        .envs(toolchain::child_env())
        // Never wait for a password prompt; fail instead.
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null());
    cmd
}

/// Runs git in `cwd` and returns trimmed stdout; stderr becomes the error message.
pub fn run(cwd: &Path, args: &[&str]) -> Result<String> {
    let output = command(cwd)
        .args(args)
        .output()
        .map_err(|err| Error::new(crate::ErrorCode::Toolchain, format!("cannot run git: {err}")))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let message = stderr.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("git failed").trim();
        Err(Error::command(format!("git {}: {message}", args.first().copied().unwrap_or_default())))
    }
}

pub fn is_work_tree(dir: &Path) -> bool {
    dir.join(".git").exists()
}

pub fn is_bare(dir: &Path) -> bool {
    dir.join("HEAD").is_file() && dir.join("objects").is_dir()
}

/// `git clone --mirror url dest` (a bare copy with every branch and tag).
pub fn clone_mirror(url: &str, dest: &Path) -> Result<()> {
    let parent = dest.parent().ok_or_else(|| Error::invalid("mirror path has no parent"))?;
    std::fs::create_dir_all(parent)?;
    run(parent, &["clone", "--mirror", "--quiet", url, &dest.to_string_lossy()])?;
    Ok(())
}

/// Updates a mirror from `url` (also when the URL changed, e.g. moved to GitHub).
pub fn fetch_mirror(dir: &Path, url: &str) -> Result<()> {
    if remote_url(dir, "origin").as_deref() != Some(url) {
        run(dir, &["remote", "set-url", "origin", url])?;
    }
    // Tags are forced so a moved tag is noticed (validation warns; exports keep their commit).
    run(dir, &["fetch", "--quiet", "--prune", "origin", "+refs/heads/*:refs/heads/*", "+refs/tags/*:refs/tags/*"])?;
    Ok(())
}

pub fn clone(url: &str, dest: &Path) -> Result<()> {
    let parent = dest.parent().ok_or_else(|| Error::invalid("clone path has no parent"))?;
    std::fs::create_dir_all(parent)?;
    run(parent, &["clone", "--quiet", url, &dest.to_string_lossy()])?;
    Ok(())
}

pub fn remote_url(dir: &Path, remote: &str) -> Option<String> {
    run(dir, &["remote", "get-url", remote]).ok().filter(|u| !u.is_empty())
}

/// Published versions (`vX.Y.Z` tags) with the commit each one points to, newest first.
pub fn version_tags(dir: &Path) -> Vec<(semver::Version, String)> {
    let Ok(out) = run(dir, &["for-each-ref", "refs/tags", "--format=%(refname:short) %(objectname) %(*objectname)"])
    else {
        return Vec::new();
    };
    let mut tags: Vec<(semver::Version, String)> = out
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let name = parts.next()?;
            let object = parts.next()?;
            // Annotated tags point to a tag object; the third field is the commit.
            let commit = parts.next().unwrap_or(object);
            let version = semver::Version::parse(name.strip_prefix(TAG_PREFIX)?).ok()?;
            Some((version, commit.to_string()))
        })
        .collect();
    tags.sort_by(|a, b| b.0.cmp(&a.0));
    tags
}

/// Writes the tree of `rev` into `dest` (created, must not exist).
pub fn export(dir: &Path, rev: &str, dest: &Path) -> Result<()> {
    let tmp = tempfile::Builder::new().prefix("fastdev-export-").tempdir()?;
    let archive = tmp.path().join("tree.tar");
    run(dir, &["archive", "--format=tar", &format!("--output={}", archive.to_string_lossy()), rev])?;
    std::fs::create_dir_all(dest)?;
    let status = Command::new("/usr/bin/tar")
        .arg("-xf")
        .arg(&archive)
        .arg("-C")
        .arg(dest)
        .status()
        .map_err(|err| Error::internal(format!("tar: {err}")))?;
    if !status.success() {
        return Err(Error::internal(format!("cannot extract {rev}")));
    }
    Ok(())
}

/// Content of `path` at `rev`, e.g. `show(dir, "v1.0.0", "template.toml")`.
pub fn show(dir: &Path, rev: &str, path: &str) -> Result<String> {
    run(dir, &["show", &format!("{rev}:{path}")])
}

pub fn head(dir: &Path) -> Option<String> {
    run(dir, &["rev-parse", "HEAD"]).ok()
}

/// No staged, unstaged or untracked changes (ignored files do not count).
pub fn is_clean(dir: &Path) -> bool {
    run(dir, &["status", "--porcelain"]).map(|out| out.is_empty()).unwrap_or(false)
}

#[cfg(test)]
pub(crate) fn init_repo(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    run(dir, &["init", "--quiet", "-b", "main"]).unwrap();
    run(dir, &["config", "user.name", "fastDev tests"]).unwrap();
    run(dir, &["config", "user.email", "tests@fastdev.local"]).unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mirrors_tags_and_exports() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("repo");
        init_repo(&repo);
        std::fs::write(repo.join("a.txt"), "one").unwrap();
        run(&repo, &["add", "-A"]).unwrap();
        run(&repo, &["commit", "-qm", "one"]).unwrap();
        run(&repo, &["tag", "-a", "v1.0.0", "-m", "1.0.0"]).unwrap();
        std::fs::write(repo.join("a.txt"), "two").unwrap();
        run(&repo, &["commit", "-qam", "two"]).unwrap();
        run(&repo, &["tag", "v1.1.0"]).unwrap();

        let mirror = tmp.path().join("cache/repo.git");
        clone_mirror(&repo.to_string_lossy(), &mirror).unwrap();
        let tags = version_tags(&mirror);
        assert_eq!(tags.iter().map(|t| t.0.to_string()).collect::<Vec<_>>(), ["1.1.0", "1.0.0"]);
        assert_eq!(tags[0].1, head(&repo).unwrap());
        assert_eq!(show(&mirror, "v1.0.0", "a.txt").unwrap(), "one");

        let out = tmp.path().join("export");
        export(&mirror, "v1.0.0", &out).unwrap();
        assert_eq!(std::fs::read_to_string(out.join("a.txt")).unwrap(), "one");

        std::fs::write(repo.join("a.txt"), "three").unwrap();
        run(&repo, &["commit", "-qam", "three"]).unwrap();
        run(&repo, &["tag", "v2.0.0"]).unwrap();
        fetch_mirror(&mirror, &repo.to_string_lossy()).unwrap();
        assert_eq!(version_tags(&mirror)[0].0.to_string(), "2.0.0");
        assert!(is_clean(&repo));
    }
}
