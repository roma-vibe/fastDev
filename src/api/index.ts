// Typed API used by stores. Components never call the transport directly.

import { call } from './client'
import type {
  AppInfo,
  DraftInfo,
  JobResponse,
  PreviewView,
  Project,
  PublishResult,
  ResolvedRequirements,
  RunInfo,
  Settings,
  SkeletonDetails,
  SkeletonSummary,
  SyncReport,
  ToolInfo,
  UpdateLevel,
  ValidationReport,
} from './types'

export * from './types'
export { ApiError, isTauri, onEvent, onQuitRequested, shell } from './client'

export interface CreateProjectArgs {
  skeleton: string
  version?: string
  name: string
  slug?: string
  parent_dir?: string
  git: boolean
  install: boolean
  features: Record<string, boolean>
  choices: Record<string, string>
  agents_md: boolean
  spec_md: boolean
  claude_md: boolean
  brief: string
}

export const api = {
  appInfo: () => call<AppInfo>('app_info'),
  getSettings: () => call<Settings>('get_settings'),
  updateSettings: (patch: Partial<Settings>) => call<Settings>('update_settings', patch),
  setFavorite: (id: string, favorite: boolean) => call<Settings>('set_favorite', { id, favorite }),
  toolchain: (refresh = false) => call<{ tools: ToolInfo[]; path: string }>('toolchain', { refresh }),

  listSkeletons: () =>
    call<{ workspacePath: string; registries: string[]; skeletons: SkeletonSummary[] }>('list_skeletons'),
  syncLibrary: () => call<SyncReport>('sync_library'),
  previewSkeleton: (
    id: string,
    version: string,
    features: Record<string, boolean>,
    choices: Record<string, string>,
  ) => call<JobResponse>('preview_skeleton', { id, version, features, choices }),
  getPreview: (id: string) => call<PreviewView>('get_preview', { id }),
  stopPreview: (id: string) => call<{ stopped: boolean }>('stop_preview', { id }),
  deletePreview: (id: string) => call<{ deleted: boolean }>('delete_preview', { id }),
  addSkeletonSource: (url: string) => call<{ id: string }>('add_skeleton_source', { url }),
  removeSkeletonSource: (url: string) => call<Settings>('remove_skeleton_source', { url }),
  getSkeleton: (id: string, version?: string) => call<SkeletonDetails>('get_skeleton', { id, version }),
  skeletonRequirements: (
    id: string,
    version: string,
    features: Record<string, boolean>,
    choices: Record<string, string>,
  ) => call<ResolvedRequirements>('skeleton_requirements', { id, version, features, choices }),
  createDraft: (args: {
    mode: 'new' | 'edit' | 'fork'
    id: string
    source?: string
    from_version?: string
    name?: string
    description?: string
  }) => call<{ draft: DraftInfo; filesPath: string }>('create_skeleton_draft', args),
  validateSkeleton: (id: string, target?: string) =>
    call<ValidationReport>('validate_skeleton', { id, target }),
  verifySkeleton: (id: string, target?: string) => call<JobResponse>('verify_skeleton', { id, target }),
  publishSkeleton: (args: {
    id: string
    bump?: string
    version?: string
    changes: string[]
    allow_unverified: boolean
  }) => call<PublishResult>('publish_skeleton', args),
  discardDraft: (id: string) => call<{ discarded: boolean }>('discard_skeleton_draft', { id }),
  checkUpdates: (id: string, target?: string) => call<JobResponse>('check_skeleton_updates', { id, target }),
  applyUpdates: (id: string, packages: { name: string; workspace: string }[], level?: UpdateLevel) =>
    call<JobResponse>('apply_skeleton_updates', { id, packages, level }),

  listProjects: () => call<{ projects: Project[] }>('list_projects'),
  getProject: (project: string) => call<Project>('get_project', { project }),
  createProject: (args: CreateProjectArgs) => call<JobResponse>('create_project', { ...args }),
  importProject: (path: string) => call<Project>('import_project', { path }),
  removeProject: (project: string) => call<{ removed: boolean }>('remove_project', { project }),
  deleteProjectFromDisk: (project: string, confirm: string) =>
    call<{ deleted: boolean }>('delete_project_from_disk', { project, confirm }),
  locateProject: (project: string, path: string) => call<Project>('locate_project', { project, path }),
  touchProject: (project: string) => call<{ ok: boolean }>('touch_project', { project }),
  runCommand: (project: string, command: string, inputs: Record<string, string> = {}) =>
    call<{ run: RunInfo; output: string[]; url: string | null }>('run_project_command', {
      project,
      command,
      inputs,
      wait_seconds: 0,
    }),
  stopCommand: (project: string, command: string) =>
    call<{ run: RunInfo }>('stop_project_command', { project, command }),
  runSetup: (project: string) => call<JobResponse>('run_project_setup', { project }),
  openProject: (project: string, target: 'editor' | 'terminal' | 'finder') =>
    call<{ ok: boolean }>('open_project', { project, target }),
  openUrl: (url: string) => call<{ ok: boolean }>('open_url', { url }),
  openPath: (path: string, target: 'finder' | 'editor' | 'terminal' = 'finder') =>
    call<{ ok: boolean }>('open_path', { path, target }),

  listRuns: () => call<{ runs: RunInfo[] }>('list_runs'),
  runLogs: (runId: string) =>
    call<{ run: RunInfo; start: number; lines: string[] }>('get_run_logs', { run_id: runId }),
  clearRunLogs: (runId: string) => call<{ cleared: boolean }>('clear_run_logs', { run_id: runId }),
  getJob: (jobId: string) => call<JobResponse>('get_job', { job_id: jobId, log_lines: 5000 }),
}
