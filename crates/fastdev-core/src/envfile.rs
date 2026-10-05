//! Minimal `.env` reader/writer that keeps comments, blank lines and key order.

use std::collections::HashMap;
use std::path::Path;

use indexmap::IndexMap;

#[derive(Debug, Clone)]
enum Line {
    /// `raw` is the original line, written back unchanged until the value is set: other keys keep
    /// their quoting and `${VAR}` references (e.g. Laravel's `MAIL_FROM_NAME="${APP_NAME}"`).
    Entry {
        key: String,
        value: String,
        raw: Option<String>,
    },
    Other(String),
}

#[derive(Debug, Clone, Default)]
pub struct EnvFile {
    lines: Vec<Line>,
}

impl EnvFile {
    pub fn parse(text: &str) -> Self {
        let lines = text
            .lines()
            .map(|line| match parse_entry(line) {
                Some((key, value)) => Line::Entry { key, value, raw: Some(line.to_string()) },
                None => Line::Other(line.to_string()),
            })
            .collect();
        Self { lines }
    }

    pub fn read(path: &Path) -> Self {
        std::fs::read_to_string(path).map(|text| Self::parse(&text)).unwrap_or_default()
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.lines.iter().find_map(|line| match line {
            Line::Entry { key: k, value, .. } if k == key => Some(value.as_str()),
            _ => None,
        })
    }

    /// Replaces the value of `key` or appends it.
    pub fn set(&mut self, key: &str, value: &str) {
        for line in &mut self.lines {
            if let Line::Entry { key: k, value: v, raw } = line
                && k == key
            {
                if v != value {
                    *v = value.to_string();
                    *raw = None;
                }
                return;
            }
        }
        self.lines.push(Line::Entry { key: key.to_string(), value: value.to_string(), raw: None });
    }

    pub fn entries(&self) -> IndexMap<String, String> {
        self.lines
            .iter()
            .filter_map(|line| match line {
                Line::Entry { key, value, .. } => Some((key.clone(), value.clone())),
                Line::Other(_) => None,
            })
            .collect()
    }

    pub fn render(&self) -> String {
        let mut out = String::new();
        for line in &self.lines {
            match line {
                Line::Entry { raw: Some(raw), .. } => out.push_str(raw),
                Line::Entry { key, value, raw: None } => {
                    out.push_str(key);
                    out.push('=');
                    out.push_str(&quote(value));
                }
                Line::Other(text) => out.push_str(text),
            }
            out.push('\n');
        }
        out
    }
}

fn parse_entry(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim_start();
    if trimmed.starts_with('#') || trimmed.is_empty() {
        return None;
    }
    let trimmed = trimmed.strip_prefix("export ").unwrap_or(trimmed);
    let (key, raw) = trimmed.split_once('=')?;
    let key = key.trim();
    if key.is_empty() || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    Some((key.to_string(), unquote(raw.trim())))
}

fn unquote(raw: &str) -> String {
    if raw.len() >= 2 && raw.starts_with('"') && raw.ends_with('"') {
        let inner = &raw[1..raw.len() - 1];
        let mut out = String::with_capacity(inner.len());
        let mut chars = inner.chars();
        while let Some(c) = chars.next() {
            if c == '\\' {
                match chars.next() {
                    Some('n') => out.push('\n'),
                    Some(other) => out.push(other),
                    None => out.push('\\'),
                }
            } else {
                out.push(c);
            }
        }
        return out;
    }
    if raw.len() >= 2 && raw.starts_with('\'') && raw.ends_with('\'') {
        return raw[1..raw.len() - 1].to_string();
    }
    // Strip inline comments of unquoted values.
    match raw.find(" #") {
        Some(pos) => raw[..pos].trim_end().to_string(),
        None => raw.to_string(),
    }
}

fn quote(value: &str) -> String {
    let bare =
        value.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '/' | ':' | '@' | ',' | '+'));
    if bare {
        return value.to_string();
    }
    // Single quotes are literal in every common parser (Node parseEnv/loadEnvFile, dotenv,
    // phpdotenv, python-dotenv, Docker Compose): no escapes, no `$` interpolation.
    if !value.contains('\'') && !value.contains('\n') {
        return format!("'{value}'");
    }
    // Rare values with single quotes or newlines: double quotes; `\n` is understood widely,
    // an escaped `"` is best effort.
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n");
    format!("\"{escaped}\"")
}

/// Values of a project's `.env` for `${VAR}` interpolation.
pub fn read_values(path: &Path) -> HashMap<String, String> {
    EnvFile::read(path).entries().into_iter().collect()
}

/// Replaces `${VAR}` with values from `vars`; unknown variables stay as they are.
pub fn interpolate(text: &str, vars: &HashMap<String, String>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        match after.find('}') {
            Some(end) => {
                let key = &after[..end];
                match vars.get(key) {
                    Some(value) => out.push_str(value),
                    None => out.push_str(&rest[start..start + 3 + end]),
                }
                rest = &after[end + 1..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_comments_and_quotes_when_needed() {
        let mut env = EnvFile::parse("# App\nAPP_NAME=\"Skeleton\"\n\nAPP_PORT=3000 # port\n");
        assert_eq!(env.get("APP_PORT"), Some("3000"));
        env.set("APP_NAME", "Мой \"магазин\"");
        env.set("NEW_KEY", "value");
        assert_eq!(env.render(), "# App\nAPP_NAME='Мой \"магазин\"'\n\nAPP_PORT=3000 # port\nNEW_KEY=value\n");
        let reparsed = EnvFile::parse(&env.render());
        assert_eq!(reparsed.get("APP_NAME"), Some("Мой \"магазин\""));
        // Values with a single quote fall back to double quotes.
        env.set("APP_NAME", "Tom's \"shop\"");
        assert_eq!(EnvFile::parse(&env.render()).get("APP_NAME"), Some("Tom's \"shop\""));
    }

    #[test]
    fn keeps_lines_it_does_not_change() {
        let text =
            "APP_NAME=Laravel\nMAIL_FROM_NAME=\"${APP_NAME}\"\nVITE_APP_NAME=\"${APP_NAME}\" # for Vite\nPORT=80\n";
        let mut env = EnvFile::parse(text);
        env.set("APP_NAME", "Coffee Shop");
        env.set("PORT", "80");
        assert_eq!(
            env.render(),
            "APP_NAME='Coffee Shop'\nMAIL_FROM_NAME=\"${APP_NAME}\"\nVITE_APP_NAME=\"${APP_NAME}\" # for Vite\nPORT=80\n"
        );
    }

    #[test]
    fn interpolates() {
        let vars = HashMap::from([("PORT".to_string(), "5173".to_string())]);
        assert_eq!(interpolate("http://localhost:${PORT}/x", &vars), "http://localhost:5173/x");
        assert_eq!(interpolate("${MISSING}:${PORT}", &vars), "${MISSING}:5173");
    }
}
