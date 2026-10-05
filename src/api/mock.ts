// In-browser mock of the fastDev API, used only when the UI runs outside Tauri
// (for design work in a normal browser). It is never part of the app's data flow.

import type {
  AppEvent,
  JobResponse,
  JobSnapshot,
  Manifest,
  Project,
  RunInfo,
  Settings,
  SkeletonDetails,
  SkeletonSummary,
} from './types'

type Handler = (event: AppEvent) => void
const handlers = new Set<Handler>()
const emit = (event: AppEvent) => handlers.forEach((h) => h(event))
export function subscribe(handler: Handler): () => void {
  handlers.add(handler)
  return () => handlers.delete(handler)
}

const now = () => new Date().toISOString()
let counter = 0
const nextId = (prefix: string) => `${prefix}-${(++counter).toString(16).padStart(6, '0')}`

const settings: Settings = {
  theme: 'system',
  language: 'system',
  registries: ['/Users/you/Documents/fastDev-library/registry'],
  skeletonSources: [],
  workspacePath: '/Users/you/Documents/fastDev-library/skeletons',
  projectsDir: '/Users/you/Documents',
  editor: 'vscode',
  editorCommand: '',
  terminal: 'terminal',
  keepInMenuBar: true,
  pushOnPublish: true,
  favorites: ['node-vue'],
}

const nodeVueDescription =
  'A full-stack web app in TypeScript: a Fastify API with SQLite and a Vue 3 single-page app styled with Tailwind CSS. ' +
  'A Notes example goes through every layer, with tests, linting and type checking that pass with zero warnings. ' +
  'Choose it for small web apps and internal tools that run anywhere Node.js runs.'

const nodeVueManifest: Manifest = {
  schema: 1,
  id: 'node-vue',
  name: 'Node + Vue',
  description: nodeVueDescription,
  category: 'web',
  languages: ['node'],
  tags: ['fullstack', 'typescript', 'sqlite', 'tailwind'],
  stack: ['Node.js 24+', 'Fastify 5', 'Vue 3', 'Vite', 'Pinia', 'Tailwind CSS 4', 'SQLite'],
  requirements: { node: '>=24', npm: '>=10' },
  features: {},
  choices: {
    docker: {
      label: 'Docker',
      description: 'How the project uses Docker.',
      default: 'run',
      options: {
        none: {
          label: 'No Docker',
          description: 'Everything runs on this Mac with Node.js.',
          files: [],
          requirements: {},
          env: {},
          commands: {},
        },
        run: {
          label: 'Docker to run the app',
          description:
            'Develop locally with Node.js; build and run the production container with Docker Compose.',
          files: ['Dockerfile', 'docker-compose.yml', '.dockerignore'],
          requirements: { docker: '>=24' },
          env: {},
          commands: {},
        },
        full: {
          label: 'Everything in Docker',
          description:
            'Dependencies, the dev server, tests and builds run in containers; only Docker is needed on this Mac.',
          files: ['Dockerfile', 'docker-compose.yml', '.dockerignore'],
          requirements: { docker: '>=24' },
          env: {},
          commands: {},
        },
      },
    },
    data: {
      label: 'Data storage',
      description: 'Where containers keep the SQLite database.',
      default: 'volume',
      when: { docker: ['run', 'full'] },
      options: {
        volume: {
          label: 'Docker volume',
          description: 'Managed by Docker, survives container rebuilds; `docker compose down -v` deletes it.',
          files: [],
          requirements: {},
          env: {},
          commands: {},
        },
        local: {
          label: 'Project folder',
          description:
            'The data/ folder of the project is mounted into containers; easy to inspect and back up.',
          files: [],
          requirements: {},
          env: {},
          commands: {},
        },
      },
    },
  },
  env: {
    APP_NAME: '{{ project.name }}',
    APP_SLUG: '{{ project.slug }}',
    DB_PATH: 'data/{{ project.slug }}.sqlite',
  },
  ports: { APP_PORT: 3000, WEB_PORT: 5173 },
  secrets: {},
  set: [{ file: 'package.json', path: ['name'], value: '{{ project.slug }}' }],
  setup: [{ label: 'Install dependencies', run: 'npm install' }],
  commands: {
    dev: { label: 'Dev', run: 'npm run dev', long: true, url: 'http://localhost:${WEB_PORT}', primary: true },
    build: { label: 'Build', run: 'npm run build' },
    start: { label: 'Start', run: 'npm start', long: true, url: 'http://localhost:${APP_PORT}' },
    test: { label: 'Test', run: 'npm test' },
    check: { label: 'Check', run: 'npm run check' },
    'docker-up': { label: 'Docker up', run: 'docker compose up --build', long: true, feature: 'docker' },
    'docker-down': { label: 'Docker down', run: 'docker compose down', feature: 'docker' },
    'new-page': {
      label: 'New page',
      run: 'npm run new:page -- "$NAME"',
      description: 'Creates a page, a route and a test.',
      inputs: [{ name: 'NAME', label: 'Page name', placeholder: 'About' }],
    },
  },
  verify: { commands: ['check', 'build'] },
  translations: {
    ru: {
      [nodeVueDescription]:
        'Полноценное веб-приложение на TypeScript: API на Fastify с SQLite и одностраничное приложение на Vue 3 со стилями Tailwind CSS. ' +
        'Пример «Заметки» проходит через все слои, с тестами, линтером и проверкой типов без единого предупреждения. ' +
        'Подходит для небольших веб-приложений и внутренних инструментов.',
      'Everything runs on this Mac with Node.js.': 'Всё запускается на этом Mac через Node.js.',
      'Where containers keep the SQLite database.': 'Где контейнеры хранят базу SQLite.',
    },
  },
  updates: { ecosystem: 'npm' },
}

const skeletons: SkeletonSummary[] = [
  {
    id: 'node-vue',
    name: 'Node + Vue',
    description: nodeVueManifest.description,
    translations: nodeVueManifest.translations,
    category: 'web',
    languages: ['node'],
    tags: nodeVueManifest.tags,
    stack: nodeVueManifest.stack,
    versions: ['1.1.0', '1.0.0'],
    latest: '1.1.0',
    draft: null,
    forkedFrom: null,
    source: 'registry',
    repo: 'https://github.com/you/fastdev-node-vue.git',
    downloaded: true,
    workspacePath: null,
    error: null,
  },
  {
    id: 'rust-web',
    name: 'Rust Web',
    description: 'Axum API with SQLx, migrations and a tiny HTMX front end.',
    category: 'web',
    languages: ['rust'],
    tags: ['axum', 'sqlx'],
    stack: ['Rust', 'Axum', 'SQLx', 'HTMX'],
    versions: [],
    latest: null,
    draft: {
      kind: 'new',
      basedOn: null,
      source: null,
      createdAt: now(),
      path: '/Users/you/Documents/fastDev/library/rust-web/draft',
      verification: null,
    },
    forkedFrom: null,
    source: 'workspace',
    repo: '/Users/you/Documents/fastDev-library/skeletons/rust-web',
    downloaded: false,
    workspacePath: '/Users/you/Documents/fastDev-library/skeletons/rust-web',
    error: null,
  },
  {
    id: 'node-vue-auth',
    name: 'Node + Vue + Auth',
    description: 'Node + Vue with sessions, sign-up and sign-in pages.',
    category: 'web',
    languages: ['node'],
    tags: ['auth'],
    stack: ['Node.js 24+', 'Fastify 5', 'Vue 3', 'SQLite'],
    versions: ['1.0.0'],
    latest: '1.0.0',
    draft: null,
    forkedFrom: { id: 'node-vue', version: '1.0.0' },
    source: 'registry',
    repo: 'https://github.com/you/fastdev-node-vue-auth.git',
    downloaded: false,
    workspacePath: null,
    error: null,
  },
]

const projects: Project[] = [
  {
    id: 'a1b2c3d4e5f6',
    name: 'Coffee Shop',
    slug: 'coffee-shop',
    path: '/Users/you/Documents/coffee-shop',
    skeletonId: 'node-vue',
    skeletonVersion: '1.0.0',
    features: { docker: true },
    ports: { APP_PORT: 3000, WEB_PORT: 5173 },
    setupStatus: 'ok',
    createdAt: now(),
    openedAt: now(),
    exists: true,
    commands: Object.entries(nodeVueManifest.commands).map(([key, c]) => ({
      key,
      label: c.label,
      run: c.run,
      description: c.description ?? '',
      long: !!c.long,
      primary: !!c.primary,
      url: c.url ? c.url.replace('${WEB_PORT}', '5173').replace('${APP_PORT}', '3000') : null,
      inputs: c.inputs,
      lastRun: null,
    })),
    setup: nodeVueManifest.setup,
    running: [],
    latestVersion: '1.1.0',
    updateAvailable: true,
    metaError: null,
  },
]

const runs = new Map<string, RunInfo>()
const runLines = new Map<string, string[]>()
const jobs = new Map<string, { snapshot: JobSnapshot; log: string[] }>()

function details(id: string): SkeletonDetails {
  const summary = skeletons.find((s) => s.id === id)
  if (!summary) throw { code: 'not_found', message: `skeleton "${id}" is not in the library` }
  const manifest = { ...nodeVueManifest, id, name: summary.name, description: summary.description }
  return {
    summary,
    target: summary.latest ?? 'draft',
    manifest,
    requirements: [
      {
        tool: 'node',
        label: 'Node.js',
        problem: null,
        hint: 'Install Node.js.',
        requirement: '>=24',
        found: true,
        version: '26.7.0',
        satisfied: true,
      },
      {
        tool: 'npm',
        label: 'npm',
        problem: null,
        hint: 'Install npm.',
        requirement: '>=10',
        found: true,
        version: '11.19.0',
        satisfied: true,
      },
    ],
    featureRequirements: {
      docker: [
        {
          tool: 'docker',
          label: 'Docker',
          problem: null,
          hint: 'Install Docker.',
          requirement: '>=24',
          found: true,
          version: '29.7.2',
          satisfied: true,
        },
      ],
    },
    changelog: [
      { version: '1.1.0', date: '2026-10-12', body: '- Updated Vue to 3.6\n- Added a health page' },
      { version: '1.0.0', date: '2026-09-30', body: '- Initial version' },
    ],
    validation: { id, target: summary.latest ?? 'draft', valid: true, errors: [], warnings: [] },
    draftVerified: summary.draft ? false : null,
    parent: summary.forkedFrom
      ? {
          id: summary.forkedFrom.id,
          version: summary.forkedFrom.version,
          latest: '1.1.0',
          newer: true,
          changelog: [],
        }
      : null,
    favorite: settings.favorites.includes(id),
    paths: {
      repo: summary.repo,
      workspace: `/Users/you/Documents/fastDev-library/skeletons/${id}`,
      target: `/Users/you/Library/Application Support/fastDev/skeletons/${id}/versions/1.1.0`,
      files: `/Users/you/Library/Application Support/fastDev/skeletons/${id}/versions/1.1.0/files`,
      manifest: '',
    },
  }
}

function startJob(title: string, kind: string, steps: string[], finish: () => unknown): JobResponse {
  const snapshot: JobSnapshot = {
    id: nextId('job'),
    kind,
    title,
    status: 'running',
    steps: steps.map((label) => ({ label, status: 'pending' })),
    startedAt: now(),
    finishedAt: null,
    result: null,
    error: null,
    warnings: [],
  }
  const entry = { snapshot, log: [] as string[] }
  jobs.set(snapshot.id, entry)
  let index = 0
  const tick = () => {
    const previous = snapshot.steps[index - 1]
    if (previous) previous.status = 'done'
    const step = snapshot.steps[index]
    if (!step) {
      snapshot.status = 'succeeded'
      snapshot.finishedAt = now()
      snapshot.result = finish()
      emit({ type: 'jobUpdated', job: structuredClone(snapshot) })
      return
    }
    step.status = 'running'
    emit({ type: 'jobUpdated', job: structuredClone(snapshot) })
    const lines = [`▸ ${step.label}`, `  working on ${step.label.toLowerCase()}…`, '  done']
    const start = entry.log.length
    entry.log.push(...lines)
    emit({ type: 'jobOutput', jobId: snapshot.id, start, lines })
    index++
    setTimeout(tick, 700)
  }
  setTimeout(tick, 150)
  return { ...structuredClone(snapshot), jobId: snapshot.id, logStart: 0, log: [] }
}

function project(ref: unknown): Project {
  const found = projects.find((p) => p.id === ref || p.slug === ref || p.path === ref)
  if (!found) throw { code: 'not_found', message: `no project "${String(ref)}"` }
  return found
}

export async function call<T>(method: string, args: Record<string, unknown>): Promise<T> {
  await new Promise((r) => setTimeout(r, 60))
  return handle(method, args) as T
}

function handle(method: string, args: Record<string, unknown>): unknown {
  switch (method) {
    case 'app_info':
      return {
        version: '0.1.0 (preview)',
        dataDir: '/Users/you/Library/Application Support/fastDev',
        socketPath: '/Users/you/Library/Application Support/fastDev/fastdev.sock',
        bridgePath: '/Applications/fastDev.app/Contents/MacOS/fastdev-mcp',
        bridgeExists: true,
        claudeCommand:
          'claude mcp add --scope user fastdev -- "/Applications/fastDev.app/Contents/MacOS/fastdev-mcp"',
        codexConfig:
          '[mcp_servers.fastdev]\ncommand = "/Applications/fastDev.app/Contents/MacOS/fastdev-mcp"\n',
      }
    case 'get_settings':
      return { ...settings }
    case 'update_settings':
      Object.assign(settings, args)
      emit({ type: 'settingsChanged', settings: { ...settings } })
      return { ...settings }
    case 'set_favorite': {
      const id = String(args.id)
      settings.favorites = settings.favorites.filter((f) => f !== id)
      if (args.favorite) settings.favorites.push(id)
      emit({ type: 'settingsChanged', settings: { ...settings } })
      return { ...settings }
    }
    case 'toolchain':
      return {
        path: '/usr/local/bin:/usr/bin:/bin',
        tools: [
          { name: 'node', found: true, version: '26.7.0', path: '/Users/you/.local/bin/node' },
          { name: 'npm', found: true, version: '11.19.0', path: '/Users/you/.local/bin/npm' },
          { name: 'git', found: true, version: '2.50.1', path: '/usr/bin/git' },
          { name: 'docker', found: true, version: '29.7.2', path: '/usr/local/bin/docker' },
          { name: 'php', found: false, version: null, path: null },
          { name: 'composer', found: false, version: null, path: null },
          { name: 'cargo', found: true, version: '1.98.1', path: '/Users/you/.cargo/bin/cargo' },
          { name: 'python3', found: true, version: '3.9.6', path: '/usr/bin/python3' },
        ],
      }
    case 'list_skeletons':
      return {
        workspacePath: settings.workspacePath,
        registries: settings.registries,
        skeletons: skeletons.map((s) => ({ ...s, favorite: settings.favorites.includes(s.id) })),
      }
    case 'get_skeleton':
      return details(String(args.id))
    case 'skeleton_requirements': {
      const choices = (args.choices ?? {}) as Record<string, string>
      const full = choices.docker === 'full'
      const docker = choices.docker !== 'none'
      return {
        choices,
        requirements: [
          ...(full
            ? []
            : [
                {
                  tool: 'node',
                  requirement: '>=24',
                  found: true,
                  version: '26.7.0',
                  satisfied: true,
                  neededForSetup: true,
                },
                {
                  tool: 'npm',
                  requirement: '>=10',
                  found: true,
                  version: '11.19.0',
                  satisfied: true,
                  neededForSetup: true,
                },
              ]),
          ...(docker
            ? [
                {
                  tool: 'docker',
                  requirement: '>=24',
                  found: true,
                  version: '29.7.2',
                  satisfied: true,
                  neededForSetup: full,
                },
              ]
            : []),
        ],
        setup: [
          {
            label: 'Install dependencies',
            run: full ? 'docker compose run --rm dev npm install' : 'npm install',
          },
        ],
        commands: [],
      }
    }
    case 'preview_skeleton':
      return startJob(
        String(args.id),
        'preview_skeleton',
        ['Check requirements', 'Copy files', 'Install dependencies', 'Start Dev'],
        () => {
          const run: RunInfo = {
            id: nextId('run'),
            projectId: `preview:${String(args.id)}`,
            command: 'dev',
            label: 'Dev',
            run: 'npm run dev',
            long: true,
            status: 'running',
            exitCode: null,
            startedAt: now(),
            finishedAt: null,
            url: 'http://localhost:5174',
          }
          runs.set(run.id, run)
          runLines.set(run.id, ['$ npm run dev', '[web] VITE ready · http://localhost:5174'])
          emit({ type: 'runUpdated', run: { ...run } })
          return {
            preview: {
              id: String(args.id),
              target: '1.1.0',
              choices: {},
              dir: '/Users/you/Library/Application Support/fastDev/previews/node-vue/1.1.0-abc/preview-node-vue',
              command: 'dev',
              label: 'Dev',
              long: true,
              url: 'http://localhost:5174',
              path: null,
              message:
                'The API and the Vite dev server are running with hot reload. Open the link to try the app; the Notes page shows the example feature. Stop the preview when you are done.',
              runId: run.id,
            },
            run: { ...run },
            start: 0,
            lines: runLines.get(run.id),
          }
        },
      )
    case 'stop_preview': {
      for (const run of runs.values()) {
        if (run.projectId === `preview:${String(args.id)}` && run.status === 'running') {
          run.status = 'stopped'
          run.finishedAt = now()
          emit({ type: 'runUpdated', run: { ...run } })
        }
      }
      return { stopped: true }
    }
    case 'get_preview':
      return { preview: null, run: null, start: 0, lines: [] }
    case 'delete_preview':
      return { deleted: true }
    case 'sync_library':
      return { registries: [{ name: settings.registries[0], ok: true, error: null }], skeletons: [] }
    case 'add_skeleton_source':
      throw { code: 'not_allowed', message: 'mock: adding sources is not available in the browser preview' }
    case 'validate_skeleton':
      return { id: args.id, target: 'draft', valid: true, errors: [], warnings: [] }
    case 'verify_skeleton':
      return startJob(
        `Verify ${String(args.id)}`,
        'verify_skeleton',
        [
          'Validate',
          'Check requirements',
          'Create test project',
          'Install dependencies',
          'Run Check',
          'Run Build',
        ],
        () => ({ passed: true }),
      )
    case 'check_skeleton_updates':
      return startJob(
        `Check updates of ${String(args.id)}`,
        'check_skeleton_updates',
        ['Prepare copy', 'Install dependencies', 'Check outdated packages'],
        () => ({
          packages: [
            {
              name: 'vue',
              workspace: 'web',
              dependencyType: 'dependencies',
              current: '3.5.43',
              wanted: '3.5.43',
              latest: '3.6.1',
              level: 'minor',
            },
            {
              name: 'fastify',
              workspace: 'server',
              dependencyType: 'dependencies',
              current: '5.12.5',
              wanted: '5.12.5',
              latest: '5.12.7',
              level: 'patch',
            },
            {
              name: 'vite',
              workspace: 'web',
              dependencyType: 'devDependencies',
              current: '8.3.1',
              wanted: '8.3.1',
              latest: '9.0.0',
              level: 'major',
            },
          ],
        }),
      )
    case 'list_projects':
      return { projects: projects.map((p) => ({ ...p })) }
    case 'get_project':
      return { ...project(args.project) }
    case 'create_project': {
      const name = String(args.name)
      const slug =
        name
          .toLowerCase()
          .replace(/[^a-z0-9]+/g, '-')
          .replace(/^-|-$/g, '') || 'project'
      return startJob(
        `Create ${name}`,
        'create_project',
        ['Check requirements', 'Copy files', 'Install dependencies', 'Initialize git', 'Register project'],
        () => {
          const template = projects[0]
          if (!template) throw { code: 'internal', message: 'mock has no template project' }
          const created: Project = {
            ...structuredClone(template),
            id: nextId('p'),
            name,
            slug,
            path: `${settings.projectsDir}/${slug}`,
            running: [],
            updateAvailable: false,
            skeletonVersion: '1.1.0',
          }
          projects.unshift(created)
          emit({ type: 'projectsChanged' })
          return { project: created, setupStatus: 'ok' }
        },
      )
    }
    case 'run_project_command': {
      const p = project(args.project)
      const command = p.commands.find((c) => c.key === args.command)
      if (!command) throw { code: 'not_found', message: `no command ${String(args.command)}` }
      const run: RunInfo = {
        id: nextId('run'),
        projectId: p.id,
        command: command.key,
        label: command.label,
        run: command.run,
        long: command.long,
        status: 'running',
        exitCode: null,
        startedAt: now(),
        finishedAt: null,
        url: command.url,
      }
      runs.set(run.id, run)
      runLines.set(run.id, [`$ ${command.run}`])
      p.running = [...p.running, command.key]
      command.lastRun = run
      emit({ type: 'runUpdated', run: { ...run } })
      let n = 0
      const timer = setInterval(() => {
        if (run.status !== 'running') return clearInterval(timer)
        const line = command.long
          ? `[web] ready in ${120 + n} ms · http://localhost:5173`
          : `step ${n + 1}/5 ok`
        const buffer = runLines.get(run.id) ?? []
        buffer.push(line)
        emit({ type: 'runOutput', runId: run.id, start: buffer.length - 1, lines: [line] })
        if (!command.long && ++n >= 5) {
          run.status = 'exited'
          run.exitCode = 0
          run.finishedAt = now()
          p.running = p.running.filter((k) => k !== command.key)
          clearInterval(timer)
          emit({ type: 'runUpdated', run: { ...run } })
        } else if (command.long) n++
      }, 600)
      return { run: { ...run }, output: [], url: run.url }
    }
    case 'stop_project_command': {
      const p = project(args.project)
      const command = p.commands.find((c) => c.key === args.command)
      const run = command?.lastRun ? runs.get(command.lastRun.id) : undefined
      if (run) {
        run.status = 'stopped'
        run.finishedAt = now()
        p.running = p.running.filter((k) => k !== run.command)
        emit({ type: 'runUpdated', run: { ...run } })
      }
      return { run }
    }
    case 'list_runs':
      return { runs: [...runs.values()] }
    case 'get_run_logs':
      return {
        run: runs.get(String(args.run_id)),
        start: 0,
        lines: [...(runLines.get(String(args.run_id)) ?? [])],
      }
    case 'clear_run_logs':
      runLines.set(String(args.run_id), [])
      return { cleared: true }
    case 'get_job': {
      const job = jobs.get(String(args.job_id))
      if (!job) throw { code: 'not_found', message: 'job not found' }
      return { ...structuredClone(job.snapshot), jobId: job.snapshot.id, logStart: 0, log: [...job.log] }
    }
    case 'touch_project':
    case 'open_project':
    case 'open_url':
    case 'open_path':
      return { ok: true }
    case 'remove_project': {
      const p = project(args.project)
      projects.splice(projects.indexOf(p), 1)
      emit({ type: 'projectsChanged' })
      return { removed: true }
    }
    default:
      throw { code: 'not_found', message: `mock: ${method} is not implemented` }
  }
}
