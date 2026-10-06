//! MCP tool definitions. The bridge lists them even when the app is closed,
//! the app implements them (see `fastdev_core::api`).

use serde::Serialize;
use serde_json::{Value, json};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDefinition {
    pub name: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub input_schema: Value,
}

fn tool(name: &'static str, title: &'static str, description: &'static str, input_schema: Value) -> ToolDefinition {
    ToolDefinition { name, title, description, input_schema }
}

fn object(properties: Value, required: &[&str]) -> Value {
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false,
    })
}

fn wait_seconds(default: u32) -> Value {
    json!({
        "type": "integer",
        "minimum": 0,
        "maximum": 600,
        "description": format!(
            "How long to wait for the job to finish (default {default}). \
             If it is still running, the result contains job_id; poll it with get_job."
        ),
    })
}

/// Names of all tools, in listing order.
pub fn names() -> Vec<&'static str> {
    definitions().into_iter().map(|t| t.name).collect()
}

pub fn definitions() -> Vec<ToolDefinition> {
    let project_ref = json!({
        "type": "string",
        "description": "Project id, slug or absolute path.",
    });
    let target = json!({
        "type": "string",
        "description": "\"draft\" or a published version like \"1.2.0\". Default: the draft if it exists, otherwise the latest version.",
    });

    vec![
        tool(
            "fastdev_status",
            "fastDev status",
            "Overview of the running fastDev app: version, skeleton registries, workspace folder (git repositories \
             of skeletons being authored), default projects folder, installed toolchain (node, npm, git, docker, \
             php, cargo, python…) and running project processes.",
            object(json!({}), &[]),
        ),
        tool(
            "get_job",
            "Get job",
            "Status, steps, log tail and result of a long-running job (project creation, skeleton verification, \
             dependency updates). Optionally waits for it to finish.",
            object(
                json!({
                    "job_id": { "type": "string" },
                    "wait_seconds": wait_seconds(0),
                    "log_lines": { "type": "integer", "minimum": 0, "maximum": 2000, "description": "Log lines to return (default 80)." },
                }),
                &["job_id"],
            ),
        ),
        tool(
            "list_skeletons",
            "List skeletons",
            "List project skeletons in the library with their versions, draft state and fork origin.",
            object(
                json!({
                    "query": { "type": "string", "description": "Case-insensitive search in id, name, description, tags and stack." },
                    "language": { "type": "string", "description": "Filter by language, e.g. node, rust, php, python." },
                    "category": { "type": "string", "description": "Filter by category: web, desktop, cli, library, mobile, other." },
                }),
                &[],
            ),
        ),
        tool(
            "get_skeleton",
            "Get skeleton",
            "Full details of a skeleton version: manifest (features, choices such as the Docker mode, env, ports, \
             setup, commands), requirement check against the local toolchain, changelog, validation state and paths.",
            object(json!({ "id": { "type": "string" }, "version": target }), &["id"]),
        ),
        tool(
            "sync_library",
            "Sync library",
            "Update the skeleton registries and the downloaded skeleton repositories (git pull/fetch). \
             Use it when a skeleton or a version you expect is missing.",
            object(json!({}), &[]),
        ),
        tool(
            "get_authoring_guide",
            "Skeleton authoring guide",
            "Returns the guide for creating, changing, forking and updating skeletons (docs/skeleton-authoring.md), \
             the manifest reference (docs/manifest.md), the workspace folder with the skeleton repositories and the \
             registries. Read it before authoring skeletons.",
            object(json!({}), &[]),
        ),
        tool(
            "preview_skeleton",
            "Preview skeleton",
            "Try a skeleton without creating a project: a throwaway copy is set up once (per version and options) \
             in fastDev's data folder and its preview command runs there (a dev server, or a build). The result has \
             the URL or path to open and a message with what to do next — tell the owner. Stop it with stop_preview. Job.",
            object(
                json!({
                    "id": { "type": "string" },
                    "version": { "type": "string", "description": "Published version (default: latest) or \"draft\"." },
                    "features": { "type": "object", "additionalProperties": { "type": "boolean" } },
                    "choices": { "type": "object", "additionalProperties": { "type": "string" }, "description": "e.g. {\"docker\": \"none\"}" },
                    "wait_seconds": wait_seconds(300),
                }),
                &["id"],
            ),
        ),
        tool(
            "stop_preview",
            "Stop preview",
            "Stop the running preview of a skeleton.",
            object(json!({ "id": { "type": "string" } }), &["id"]),
        ),
        tool(
            "list_projects",
            "List projects",
            "All projects registered in fastDev with path, skeleton, status and running commands.",
            object(json!({}), &[]),
        ),
        tool(
            "get_project",
            "Get project",
            "Project details: path, skeleton version and available update, features, ports, commands and runs.",
            object(json!({ "project": project_ref }), &["project"]),
        ),
        tool(
            "create_project",
            "Create project",
            "Create a new project from a skeleton: copies files, injects the name into .env/.env.example, renders \
             AGENTS.md/SPEC.md (brief goes into SPEC.md), installs dependencies, initialises git and registers the project. \
             Runs as a job.",
            object(
                json!({
                    "skeleton": { "type": "string", "description": "Skeleton id, e.g. node-vue." },
                    "version": { "type": "string", "description": "Published version, or \"draft\" to test the workspace clone end to end. Default: latest." },
                    "name": { "type": "string", "description": "Human-readable project name (any language)." },
                    "slug": { "type": "string", "description": "Folder and package name; must not be used by another project. Default: derived from name (with a -2, -3… suffix when taken)." },
                    "parent_dir": { "type": "string", "description": "Absolute folder to create the project in. Default: the projects folder from settings." },
                    "git": { "type": "boolean", "description": "Initialise git and commit (default true)." },
                    "install": { "type": "boolean", "description": "Run setup steps such as npm install (default true)." },
                    "features": { "type": "object", "additionalProperties": { "type": "boolean" }, "description": "On/off skeleton features from the manifest [features]. Default: manifest defaults." },
                    "choices": { "type": "object", "additionalProperties": { "type": "string" }, "description": "Single-choice options from the manifest [choices], e.g. {\"docker\": \"full\", \"data\": \"local\"}. See get_skeleton → manifest.choices. Default: manifest defaults." },
                    "agents_md": { "type": "boolean", "description": "Render AGENTS.md (default true)." },
                    "spec_md": { "type": "boolean", "description": "Render SPEC.md (default true)." },
                    "claude_md": { "type": "boolean", "description": "Write CLAUDE.md importing AGENTS.md (default true)." },
                    "brief": { "type": "string", "description": "Initial idea / prompt for the agent; written into SPEC.md." },
                    "wait_seconds": wait_seconds(120),
                }),
                &["skeleton", "name"],
            ),
        ),
        tool(
            "import_project",
            "Import project",
            "Register an existing project folder. Uses its .fastdev.toml, or package.json scripts as commands.",
            object(
                json!({ "path": { "type": "string", "description": "Absolute path of the project folder." } }),
                &["path"],
            ),
        ),
        tool(
            "remove_project",
            "Remove project from list",
            "Remove a project from the fastDev list. Files on disk are never deleted by this tool.",
            object(json!({ "project": project_ref }), &["project"]),
        ),
        tool(
            "run_project_command",
            "Run project command",
            "Start one of the project's commands (see get_project, e.g. dev, build, test, docker-up). \
             Short commands are awaited and their output returned; long-running ones (dev servers) return after \
             wait_seconds with the first output and URL, and keep running until stopped.",
            object(
                json!({
                    "project": project_ref,
                    "command": { "type": "string" },
                    "inputs": { "type": "object", "additionalProperties": { "type": "string" }, "description": "Values for the command's inputs (see get_project → commands[].inputs), e.g. {\"NAME\": \"Tab Organizer\"}. Required inputs must be given." },
                    "wait_seconds": { "type": "integer", "minimum": 0, "maximum": 600, "description": "Default: 5 for long-running commands, 300 for others." },
                }),
                &["project", "command"],
            ),
        ),
        tool(
            "stop_project_command",
            "Stop project command",
            "Stop a running project command (its whole process group).",
            object(json!({ "project": project_ref, "command": { "type": "string" } }), &["project", "command"]),
        ),
        tool(
            "get_project_logs",
            "Get project logs",
            "Output of the latest run of a project command, or of all commands.",
            object(
                json!({
                    "project": project_ref,
                    "command": { "type": "string" },
                    "lines": { "type": "integer", "minimum": 1, "maximum": 5000, "description": "Default 200." },
                }),
                &["project"],
            ),
        ),
        tool(
            "run_project_setup",
            "Run project setup",
            "Run the skeleton setup steps again (e.g. npm install) in a project. Runs as a job.",
            object(json!({ "project": project_ref, "wait_seconds": wait_seconds(120) }), &["project"]),
        ),
        tool(
            "create_skeleton_draft",
            "Create skeleton draft",
            "Open the draft of a skeleton: its git repository in the workspace folder (cloned when needed); edit \
             template.toml and files/ there with your normal file tools. mode=new: new repository with a scaffold \
             for skeleton `id`. mode=edit: the repository of `id` at its latest version (for a new version). \
             mode=fork: a copy of `source` (at from_version) as a new independent skeleton `id` with its own versions.",
            object(
                json!({
                    "mode": { "type": "string", "enum": ["new", "edit", "fork"] },
                    "id": { "type": "string", "description": "Skeleton id (lowercase, digits, dashes). For fork: the NEW id." },
                    "source": { "type": "string", "description": "fork only: id of the skeleton to fork." },
                    "from_version": { "type": "string", "description": "edit/fork: version to copy. Default: latest." },
                    "name": { "type": "string", "description": "new/fork: display name." },
                    "description": { "type": "string", "description": "new/fork: one-sentence description." },
                }),
                &["mode", "id"],
            ),
        ),
        tool(
            "validate_skeleton",
            "Validate skeleton",
            "Check a draft or version: manifest schema, required files, forbidden folders, template syntax, \
             feature file lists, command references and checksums. Returns errors and warnings.",
            object(json!({ "id": { "type": "string" }, "target": target }), &["id"]),
        ),
        tool(
            "verify_skeleton",
            "Verify skeleton",
            "Create a throwaway project from a draft or version (all features on) in a temp folder, run setup and the \
             manifest's verify commands. A passing verification of the current draft content is required to publish. Job.",
            object(json!({ "id": { "type": "string" }, "target": target, "wait_seconds": wait_seconds(300) }), &["id"]),
        ),
        tool(
            "publish_skeleton",
            "Publish skeleton",
            "Publish the draft as a new immutable version: prepends CHANGELOG.md, commits, creates the vX.Y.Z tag, \
             pushes to origin (when configured) and updates the local registry. Use bump (patch|minor|major) or an \
             explicit version.",
            object(
                json!({
                    "id": { "type": "string" },
                    "bump": { "type": "string", "enum": ["patch", "minor", "major"] },
                    "version": { "type": "string", "description": "Explicit semver version instead of bump." },
                    "changes": { "type": "array", "items": { "type": "string" }, "minItems": 1, "description": "Changelog entries, one sentence each." },
                    "allow_unverified": { "type": "boolean", "description": "Publish without a passing verification (only when the toolchain is not installed locally). Default false." },
                }),
                &["id", "changes"],
            ),
        ),
        tool(
            "discard_skeleton_draft",
            "Discard skeleton draft",
            "Throw away the draft: the workspace repository is reset to the latest version (or removed when the \
             skeleton was never published). Published versions are not affected.",
            object(json!({ "id": { "type": "string" } }), &["id"]),
        ),
        tool(
            "check_skeleton_updates",
            "Check skeleton dependency updates",
            "List outdated dependencies of a skeleton version or draft (installs it in a temp folder first). Job.",
            object(json!({ "id": { "type": "string" }, "target": target, "wait_seconds": wait_seconds(300) }), &["id"]),
        ),
        tool(
            "apply_skeleton_updates",
            "Apply skeleton dependency updates",
            "Apply dependency updates into the draft (created from the latest version when missing). Either list \
             packages or give a level to update everything up to it. Then verify and publish. Job.",
            object(
                json!({
                    "id": { "type": "string" },
                    "packages": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "name": { "type": "string" },
                                "workspace": { "type": "string", "description": "Workspace folder, empty for the root." },
                                "version": { "type": "string", "description": "Target version. Default: latest." },
                            },
                            "required": ["name"],
                            "additionalProperties": false,
                        },
                    },
                    "level": { "type": "string", "enum": ["patch", "minor", "major"], "description": "Update every package whose update type is at most this level." },
                    "wait_seconds": wait_seconds(300),
                }),
                &["id"],
            ),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_names_are_unique() {
        let mut names = names();
        let count = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), count);
    }
}
