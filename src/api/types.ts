// Types of the fastDev API (fastdev_core::api). Arguments are snake_case, results camelCase,
// except the raw manifest, which mirrors template.toml keys.

export type Theme = 'system' | 'light' | 'dark'
export type Language = 'system' | 'en' | 'ru'
export type EditorKind = 'vscode' | 'cursor' | 'zed' | 'custom'
export type TerminalApp = 'terminal' | 'iterm' | 'warp' | 'ghostty'
/** Texts translated by a skeleton: language → English text → translation. */
export type Translations = Record<string, Record<string, string>>

export interface Settings {
  theme: Theme
  language: Language
  registries: string[]
  skeletonSources: string[]
  workspacePath: string
  projectsDir: string
  editor: EditorKind
  editorCommand: string
  terminal: TerminalApp
  keepInMenuBar: boolean
  pushOnPublish: boolean
  favorites: string[]
}

export interface Verification {
  hash: string
  passed: boolean
  at: string
  jobId: string
  /** Selections skipped because their tools are missing on this Mac. */
  skipped?: string[]
}

export type DraftKind = 'new' | 'edit' | 'fork'

export interface DraftInfo {
  kind: DraftKind
  basedOn: string | null
  source: string | null
  createdAt: string
  path: string
  verification: Verification | null
}

export interface ForkOrigin {
  id: string
  version: string
}

export interface SkeletonSummary {
  id: string
  name: string
  description: string
  category: string
  languages: string[]
  tags: string[]
  stack: string[]
  versions: string[]
  latest: string | null
  draft: DraftInfo | null
  forkedFrom: ForkOrigin | null
  source: 'registry' | 'custom' | 'workspace'
  repo: string | null
  downloaded: boolean
  workspacePath: string | null
  /** Translations of name and description. */
  translations?: Translations
  error: string | null
  favorite?: boolean
}

/** A value a command asks for before it runs (passed as an environment variable). */
export interface CommandInput {
  name: string
  label: string
  placeholder?: string
  default?: string
  optional?: boolean
}

export interface ManifestCommand {
  inputs?: CommandInput[]
  label: string
  run: string
  description?: string
  long?: boolean
  url?: string
  primary?: boolean
  feature?: string
}

export interface ManifestFeature {
  label: string
  description: string
  default: boolean
  files: string[]
  requirements: Record<string, string>
  env: Record<string, string>
}

export interface ManifestChoiceOption {
  label: string
  description: string
  files: string[]
  requirements: Record<string, string>
  drop_requirements?: string[]
  env: Record<string, string>
  setup?: SetupStep[]
  commands: Record<string, Partial<ManifestCommand>>
  cleanup?: string
  preview?: Partial<ManifestPreview>
}

export interface ManifestPreview {
  command?: string
  url?: string
  path?: string
  message: string
}

export interface ManifestChoice {
  label: string
  description: string
  default: string
  when?: Record<string, string | string[]>
  options: Record<string, ManifestChoiceOption>
}

export interface SetupStep {
  label: string
  run: string
  feature?: string
}

export interface Manifest {
  schema: number
  id: string
  name: string
  description: string
  category: string
  languages: string[]
  tags: string[]
  stack: string[]
  forked_from?: ForkOrigin
  requirements: Record<string, string>
  features: Record<string, ManifestFeature>
  choices: Record<string, ManifestChoice>
  env: Record<string, string>
  ports: Record<string, number>
  secrets: Record<string, number>
  set: { file: string; path: string[]; value: string; format?: string; feature?: string }[]
  setup: SetupStep[]
  commands: Record<string, ManifestCommand>
  verify: { commands: string[]; variants?: Record<string, string>[] }
  preview?: ManifestPreview
  translations?: Translations
  updates?: { ecosystem: string }
}

export interface RequirementStatus {
  tool: string
  label: string
  requirement: string
  found: boolean
  version: string | null
  satisfied: boolean
  /** Why it is not satisfied, e.g. "Docker is installed but not running". */
  problem: string | null
  /** How to install or fix it. */
  hint: string
  /** Only in skeleton_requirements: a missing tool blocks setup. */
  neededForSetup?: boolean
}

export interface ResolvedRequirements {
  choices: Record<string, string>
  requirements: RequirementStatus[]
  setup: SetupStep[]
  commands: string[]
}

export interface ChangelogEntry {
  version: string
  date: string
  body: string
}

export interface Issue {
  level: 'error' | 'warning'
  file: string | null
  message: string
}

export interface ValidationReport {
  id: string
  target: string
  valid: boolean
  errors: Issue[]
  warnings: Issue[]
}

export interface ParentInfo {
  id: string
  version: string
  latest: string | null
  newer: boolean
  changelog: ChangelogEntry[]
}

export interface SkeletonDetails {
  summary: SkeletonSummary
  target: string
  manifest: Manifest | null
  requirements: RequirementStatus[]
  featureRequirements: Record<string, RequirementStatus[]>
  changelog: ChangelogEntry[]
  validation: ValidationReport
  draftVerified: boolean | null
  parent: ParentInfo | null
  favorite: boolean
  paths: { repo: string | null; workspace: string; target: string; files: string; manifest: string }
}

export interface PreviewInfo {
  id: string
  target: string
  choices: Record<string, string>
  dir: string
  command: string
  label: string
  long: boolean
  url: string | null
  path: string | null
  message: string
  runId: string | null
  /** Translations of label and message. */
  translations?: Translations
}

export interface PreviewView {
  preview: PreviewInfo | null
  run: RunInfo | null
  start: number
  lines: string[]
}

export type RunStatus = 'running' | 'exited' | 'stopped' | 'failed'

export interface RunInfo {
  id: string
  projectId: string
  command: string
  label: string
  run: string
  long: boolean
  status: RunStatus
  exitCode: number | null
  startedAt: string
  finishedAt: string | null
  url: string | null
}

export interface CommandView {
  inputs?: CommandInput[]
  key: string
  label: string
  run: string
  description: string
  long: boolean
  primary: boolean
  url: string | null
  lastRun: RunInfo | null
}

export type SetupStatus = 'ok' | 'failed' | 'skipped'

export interface Project {
  id: string
  name: string
  slug: string
  path: string
  skeletonId: string | null
  skeletonVersion: string | null
  features: Record<string, boolean>
  choices?: Record<string, string>
  ports: Record<string, number>
  setupStatus: SetupStatus
  createdAt: string
  openedAt: string | null
  exists: boolean
  commands: CommandView[]
  setup: SetupStep[]
  running: string[]
  latestVersion: string | null
  updateAvailable: boolean
  metaError: string | null
  /** Translations of command and setup labels, from the skeleton. */
  translations?: Translations
}

export type JobStatus = 'running' | 'succeeded' | 'failed'
export type StepStatus = 'pending' | 'running' | 'done' | 'failed' | 'skipped'

export interface JobSnapshot {
  id: string
  kind: string
  title: string
  status: JobStatus
  steps: { label: string; status: StepStatus }[]
  startedAt: string
  finishedAt: string | null
  result: unknown
  error: { code: string; message: string } | null
  warnings: string[]
  /** Translations of step labels that come from a skeleton. */
  translations?: Translations
}

export interface JobResponse extends JobSnapshot {
  jobId: string
  /** Absolute index of log[0]. */
  logStart: number
  log: string[]
}

export interface ToolInfo {
  name: string
  found: boolean
  version: string | null
  path: string | null
}

export interface AppInfo {
  version: string
  dataDir: string
  socketPath: string
  bridgePath: string | null
  bridgeExists: boolean
  claudeCommand: string
  codexConfig: string
}

export interface ShellInfo {
  glassSupported: boolean
  controlSocket: string | null
  controlError: string | null
  runningProcesses: number
}

export type UpdateLevel = 'patch' | 'minor' | 'major'

export interface OutdatedPackage {
  name: string
  workspace: string
  dependencyType: string
  current: string | null
  wanted: string | null
  latest: string
  level: UpdateLevel
  /** The `[updates] hold` range the latest version is outside of. */
  held?: string
}

export interface PublishResult {
  id: string
  version: string
  tag: string
  commit: string
  path: string
  verified: boolean
  pushed: boolean
  registry: string | null
  warnings: string[]
}

export interface SyncReport {
  registries: { name: string; ok: boolean; error: string | null }[]
  skeletons: { name: string; ok: boolean; error: string | null }[]
}

export type AppEvent =
  | { type: 'jobUpdated'; job: JobSnapshot }
  | { type: 'jobOutput'; jobId: string; start: number; lines: string[] }
  | { type: 'runUpdated'; run: RunInfo }
  | { type: 'runOutput'; runId: string; start: number; lines: string[] }
  | { type: 'projectsChanged' }
  | { type: 'libraryChanged' }
  | { type: 'settingsChanged'; settings: Settings }
