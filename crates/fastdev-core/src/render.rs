//! Rendering of `*.tmpl` files and manifest values with minijinja (strict undefined).

use std::collections::BTreeMap;

use indexmap::IndexMap;
use minijinja::{Environment, UndefinedBehavior};
use serde::Serialize;

use crate::error::{Error, Result};

pub const TEMPLATE_SUFFIX: &str = ".tmpl";

#[derive(Debug, Clone, Serialize)]
pub struct ProjectContext {
    pub name: String,
    pub slug: String,
    pub brief: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SkeletonContext {
    pub id: String,
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct OptionsContext {
    pub git: bool,
    pub install: bool,
    pub agents_md: bool,
    pub spec_md: bool,
    pub claude_md: bool,
}

/// Everything a template can use. See SPEC §5.5.
#[derive(Debug, Clone, Serialize)]
pub struct TemplateContext {
    pub project: ProjectContext,
    pub skeleton: SkeletonContext,
    pub features: BTreeMap<String, bool>,
    /// Selected option per choice ("" when the choice does not apply).
    pub choices: BTreeMap<String, String>,
    pub options: OptionsContext,
    pub env: IndexMap<String, String>,
    pub ports: IndexMap<String, u16>,
}

pub fn environment() -> Environment<'static> {
    let mut env = Environment::new();
    env.set_undefined_behavior(UndefinedBehavior::Strict);
    env.set_keep_trailing_newline(true);
    env
}

/// Renders a template string; `name` is used in error messages.
pub fn render_str(env: &Environment<'_>, name: &str, source: &str, ctx: &TemplateContext) -> Result<String> {
    env.render_named_str(name, source, ctx).map_err(|err| Error::validation(describe(name, &err)))
}

/// Only checks syntax (used by validation, where no context exists yet).
pub fn check_syntax(env: &Environment<'_>, name: &str, source: &str) -> Result<()> {
    env.template_from_named_str(name, source).map(|_| ()).map_err(|err| Error::validation(describe(name, &err)))
}

fn describe(name: &str, err: &minijinja::Error) -> String {
    let mut message = format!("{name}: {}", err.kind());
    if let Some(detail) = err.detail() {
        message.push_str(": ");
        message.push_str(detail);
    }
    if let Some(line) = err.line() {
        message.push_str(&format!(" (line {line})"));
    }
    message
}

#[cfg(test)]
pub(crate) fn sample_context() -> TemplateContext {
    TemplateContext {
        project: ProjectContext {
            name: "My Shop".into(),
            slug: "my-shop".into(),
            brief: String::new(),
            created_at: "2026-09-30".into(),
        },
        skeleton: SkeletonContext { id: "node-vue".into(), name: "Node + Vue".into(), version: "1.0.0".into() },
        features: BTreeMap::from([("docker".into(), true)]),
        choices: BTreeMap::from([("mode".into(), "full".into())]),
        options: OptionsContext { git: true, install: true, agents_md: true, spec_md: true, claude_md: true },
        env: IndexMap::new(),
        ports: IndexMap::from([("APP_PORT".into(), 3000)]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_and_fails_on_undefined() {
        let env = environment();
        let ctx = sample_context();
        let out = render_str(
            &env,
            "a",
            "{{ project.name }} {% if features.docker %}D{% endif %} {{ ports.APP_PORT }}\n",
            &ctx,
        )
        .unwrap();
        assert_eq!(out, "My Shop D 3000\n");
        let err = render_str(&env, "b.tmpl", "{{ project.nmae }}", &ctx).unwrap_err();
        assert!(err.message.starts_with("b.tmpl"), "{}", err.message);
    }
}
