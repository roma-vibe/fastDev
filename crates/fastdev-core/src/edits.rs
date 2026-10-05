//! Structured edits (`[[set]]`) of JSON and TOML files that keep the rest of the file intact.
//!
//! A path is a list of keys. A key may select an element of an array of objects/tables:
//! `package[name=app]` → the element of `package` whose `name` is `app` (e.g. `Cargo.lock`,
//! `uv.lock`); `[name=app]` selects inside the current array.

use std::path::Path;

use crate::error::{Error, IoContext, Result};
use crate::util::write_atomic;

/// Format of a `[[set]]` file: explicit, or from the extension/well-known lockfile names.
pub fn format_of(file: &str, explicit: Option<&str>) -> Option<&'static str> {
    match explicit {
        Some("json") => return Some("json"),
        Some("toml") => return Some("toml"),
        Some(_) => return None,
        None => {}
    }
    let name = file.rsplit('/').next().unwrap_or(file);
    match name {
        "Cargo.lock" | "uv.lock" | "poetry.lock" | "pdm.lock" => Some("toml"),
        "composer.lock" => Some("json"),
        _ if name.ends_with(".json") => Some("json"),
        _ if name.ends_with(".toml") => Some("toml"),
        _ => None,
    }
}

/// A path segment: a key, optionally selecting an array element by a field.
struct Segment<'a> {
    key: Option<&'a str>,
    select: Option<(&'a str, &'a str)>,
}

fn parse_segment(segment: &str) -> Result<Segment<'_>> {
    let Some(open) = segment.find('[').filter(|_| segment.ends_with(']')) else {
        return Ok(Segment { key: Some(segment), select: None });
    };
    let inner = &segment[open + 1..segment.len() - 1];
    let (field, value) = inner
        .split_once('=')
        .ok_or_else(|| Error::validation(format!("\"{segment}\": use key[field=value] to select an array element")))?;
    let key = &segment[..open];
    Ok(Segment { key: (!key.is_empty()).then_some(key), select: Some((field.trim(), value.trim())) })
}

/// Sets `path` in a JSON or TOML file to a string value. Missing objects are created;
/// selected array elements must exist.
pub fn set_value(file: &Path, path: &[String], value: &str, format: &str) -> Result<()> {
    if path.is_empty() {
        return Err(Error::validation(format!("{}: empty path in [[set]]", file.display())));
    }
    let text = std::fs::read_to_string(file).at(file)?;
    let mut updated = match format {
        "json" => set_json(&text, path, value),
        "toml" => set_toml(&text, path, value),
        other => Err(Error::validation(format!("[[set]] supports json and toml, not {other}"))),
    }
    .map_err(|err| Error::validation(format!("{}: {}", file.display(), err.message)))?;
    if matches!(file.file_name().and_then(|n| n.to_str()), Some("Cargo.lock" | "uv.lock")) {
        updated = sort_lock_packages(&updated);
    }
    write_atomic(file, updated.as_bytes())
}

/// Cargo and uv keep `[[package]]` entries sorted by name and rewrite the lockfile otherwise, so a
/// renamed package is moved to its place (a stable sort: same-name entries keep their order).
/// Text-based, because a package's sub-tables (`[package.metadata]`…) must move with it.
fn sort_lock_packages(text: &str) -> String {
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let is_package = |line: &str| line.trim_end() == "[[package]]";
    let belongs =
        |line: &str| !line.starts_with('[') || line.starts_with("[package.") || line.starts_with("[[package.");
    let Some(first) = lines.iter().position(|l| is_package(l)) else {
        return text.to_string();
    };
    let end = lines[first..].iter().position(|l| !is_package(l) && !belongs(l)).map_or(lines.len(), |p| first + p);
    let mut blocks: Vec<String> = Vec::new();
    for line in &lines[first..end] {
        if is_package(line) {
            blocks.push(String::new());
        }
        blocks.last_mut().expect("starts with [[package]]").push_str(line);
    }
    let name = |block: &str| {
        block
            .lines()
            .find_map(|l| l.strip_prefix("name = \"").and_then(|rest| rest.strip_suffix('"')))
            .unwrap_or_default()
            .to_string()
    };
    blocks.sort_by_key(|block| name(block));
    let mut out: String = lines[..first].concat();
    let body: Vec<String> = blocks.iter().map(|b| format!("{}\n", b.trim_end_matches('\n'))).collect();
    out.push_str(&body.join("\n"));
    if end < lines.len() {
        out.push('\n');
        out.push_str(&lines[end..].concat());
    }
    out
}

fn set_json(text: &str, path: &[String], value: &str) -> Result<String> {
    let mut root: serde_json::Value = serde_json::from_str(text)?;
    set_json_in(&mut root, path, value)?;
    let indent = detect_json_indent(text);
    let mut out = Vec::new();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(indent.as_bytes());
    let mut ser = serde_json::Serializer::with_formatter(&mut out, formatter);
    serde::Serialize::serialize(&root, &mut ser)?;
    let mut out = String::from_utf8(out).map_err(|err| Error::internal(err.to_string()))?;
    if text.ends_with('\n') {
        out.push('\n');
    }
    Ok(out)
}

fn set_json_in(node: &mut serde_json::Value, path: &[String], value: &str) -> Result<()> {
    let (first, rest) = path.split_first().expect("non-empty path");
    let segment = parse_segment(first)?;
    if rest.is_empty() {
        let (Some(key), None) = (segment.key, segment.select) else {
            return Err(Error::validation("the last path segment must be a plain key".to_string()));
        };
        let obj = node
            .as_object_mut()
            .ok_or_else(|| Error::validation("parent of the last key is not an object".to_string()))?;
        obj.insert(key.to_string(), serde_json::Value::String(value.to_string()));
        return Ok(());
    }
    let mut target = node;
    if let Some(key) = segment.key {
        let obj =
            target.as_object_mut().ok_or_else(|| Error::validation(format!("\"{key}\" is not inside an object")))?;
        let default = if segment.select.is_some() { serde_json::Value::Array(vec![]) } else { serde_json::json!({}) };
        target = obj.entry(key.to_string()).or_insert(default);
    }
    if let Some((field, want)) = segment.select {
        let array = target.as_array_mut().ok_or_else(|| Error::validation(format!("\"{first}\" is not an array")))?;
        target = array
            .iter_mut()
            .find(|item| item.get(field).and_then(|v| v.as_str()) == Some(want))
            .ok_or_else(|| Error::validation(format!("no element with {field} = \"{want}\" in \"{first}\"")))?;
    }
    set_json_in(target, rest, value)
}

fn detect_json_indent(text: &str) -> String {
    text.lines()
        .nth(1)
        .map(|line| line.chars().take_while(|c| *c == ' ' || *c == '\t').collect::<String>())
        .filter(|indent| !indent.is_empty())
        .unwrap_or_else(|| "  ".to_string())
}

fn set_toml(text: &str, path: &[String], value: &str) -> Result<String> {
    let mut doc: toml_edit::DocumentMut =
        text.parse().map_err(|err: toml_edit::TomlError| Error::validation(err.to_string()))?;
    set_toml_in(doc.as_table_mut(), path, value)?;
    Ok(doc.to_string())
}

fn set_toml_in(table: &mut dyn toml_edit::TableLike, path: &[String], value: &str) -> Result<()> {
    let (first, rest) = path.split_first().expect("non-empty path");
    let segment = parse_segment(first)?;
    let key = segment
        .key
        .ok_or_else(|| Error::validation(format!("\"{first}\": TOML paths need a key before [field=value]")))?;
    if rest.is_empty() {
        if segment.select.is_some() {
            return Err(Error::validation("the last path segment must be a plain key".to_string()));
        }
        match table.get_mut(key) {
            Some(existing) if existing.is_value() => {
                let decor = existing.as_value().map(|v| v.decor().clone());
                let mut new_value = toml_edit::Value::from(value);
                if let Some(decor) = decor {
                    *new_value.decor_mut() = decor;
                }
                *existing = toml_edit::Item::Value(new_value);
            }
            _ => {
                table.insert(key, toml_edit::value(value));
            }
        }
        return Ok(());
    }
    match segment.select {
        None => {
            if table.get(key).is_none() {
                table.insert(key, toml_edit::table());
            }
            let item = table.get_mut(key).expect("inserted above");
            let sub = item.as_table_like_mut().ok_or_else(|| Error::validation(format!("\"{key}\" is not a table")))?;
            set_toml_in(sub, rest, value)
        }
        Some((field, want)) => {
            let item = table.get_mut(key).ok_or_else(|| Error::validation(format!("\"{key}\" does not exist")))?;
            let matches = |t: &dyn toml_edit::TableLike| t.get(field).and_then(|v| v.as_str()) == Some(want);
            if let Some(tables) = item.as_array_of_tables_mut() {
                let found = tables
                    .iter_mut()
                    .find(|t| matches(*t))
                    .ok_or_else(|| Error::validation(format!("no [[{key}]] with {field} = \"{want}\"")))?;
                return set_toml_in(found, rest, value);
            }
            if let Some(array) = item.as_array_mut() {
                let found = array
                    .iter_mut()
                    .filter_map(|v| v.as_inline_table_mut())
                    .find(|t| matches(*t))
                    .ok_or_else(|| Error::validation(format!("no element of {key} with {field} = \"{want}\"")))?;
                return set_toml_in(found, rest, value);
            }
            Err(Error::validation(format!("\"{key}\" is not an array of tables")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_json_keeping_order_and_indent() {
        let text = "{\n  \"name\": \"app\",\n  \"packages\": {\n    \"\": { \"name\": \"app\" }\n  }\n}\n";
        let out = set_json(text, &["packages".into(), "".into(), "name".into()], "my-shop").unwrap();
        let out = set_json(&out, &["name".into()], "my-shop").unwrap();
        assert_eq!(
            out,
            "{\n  \"name\": \"my-shop\",\n  \"packages\": {\n    \"\": {\n      \"name\": \"my-shop\"\n    }\n  }\n}\n"
        );
    }

    #[test]
    fn edits_toml_keeping_comments() {
        let text = "[package]\n# The crate name\nname = \"app\" # keep\nversion = \"0.1.0\"\n";
        let out = set_toml(text, &["package".into(), "name".into()], "my-shop").unwrap();
        assert_eq!(out, "[package]\n# The crate name\nname = \"my-shop\" # keep\nversion = \"0.1.0\"\n");
    }

    #[test]
    fn selects_array_elements() {
        let lock = "version = 4\n\n[[package]]\nname = \"anyhow\"\nversion = \"1.0.0\"\n\n[[package]]\nname = \"app\"\nversion = \"0.1.0\"\n";
        let out = set_toml(lock, &["package[name=app]".into(), "name".into()], "my-tool").unwrap();
        assert!(out.contains("name = \"my-tool\"") && out.contains("name = \"anyhow\""), "{out}");
        assert!(set_toml(lock, &["package[name=missing]".into(), "name".into()], "x").is_err());

        let json = "{\n  \"packages\": [\n    { \"name\": \"a\" },\n    { \"name\": \"app\", \"v\": \"1\" }\n  ]\n}\n";
        let out = set_json(json, &["packages[name=app]".into(), "name".into()], "shop").unwrap();
        assert!(out.contains("\"name\": \"shop\"") && out.contains("\"name\": \"a\""), "{out}");
    }

    #[test]
    fn keeps_lockfile_packages_sorted() {
        let lock = "version = 4\n\n[[package]]\nname = \"anyhow\"\nversion = \"1.0.0\"\n\n[[package]]\nname = \"app\"\nversion = \"0.1.0\"\ndependencies = [\n \"anyhow\",\n]\n\n[package.metadata]\nx = 1\n\n[[package]]\nname = \"zeta\"\nversion = \"2.0.0\"\n";
        let renamed = set_toml(lock, &["package[name=app]".into(), "name".into()], "zz-tool").unwrap();
        let sorted = sort_lock_packages(&renamed);
        assert_eq!(
            sorted,
            "version = 4\n\n[[package]]\nname = \"anyhow\"\nversion = \"1.0.0\"\n\n[[package]]\nname = \"zeta\"\nversion = \"2.0.0\"\n\n[[package]]\nname = \"zz-tool\"\nversion = \"0.1.0\"\ndependencies = [\n \"anyhow\",\n]\n\n[package.metadata]\nx = 1\n"
        );
        assert_eq!(sort_lock_packages(&sorted), sorted);
        // A section after the packages stays in place.
        let with_tail = format!("{lock}\n[metadata]\ny = 2\n");
        assert!(sort_lock_packages(&with_tail).ends_with("version = \"2.0.0\"\n\n[metadata]\ny = 2\n"));
    }

    #[test]
    fn infers_formats() {
        assert_eq!(format_of("Cargo.lock", None), Some("toml"));
        assert_eq!(format_of("server/uv.lock", None), Some("toml"));
        assert_eq!(format_of("composer.lock", None), Some("json"));
        assert_eq!(format_of("package.json", None), Some("json"));
        assert_eq!(format_of("yarn.lock", None), None);
        assert_eq!(format_of("data.cfg", Some("toml")), Some("toml"));
    }
}
