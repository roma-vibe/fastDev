// Transport to the Rust core. In the Tauri window everything goes through the `call` command;
// in a plain browser (UI preview during development) a mock backend is used instead.

import type { AppEvent, ShellInfo } from './types'

export const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window

const EVENT = 'fastdev://event'
const QUIT_REQUESTED = 'fastdev://quit-requested'

export class ApiError extends Error {
  readonly code: string

  constructor(code: string, message: string) {
    super(message)
    this.code = code
  }
}

function toApiError(error: unknown): ApiError {
  if (error instanceof ApiError) return error
  if (error && typeof error === 'object' && 'message' in error) {
    const e = error as { code?: string; message: string }
    return new ApiError(e.code ?? 'internal', e.message)
  }
  return new ApiError('internal', String(error))
}

type MockModule = typeof import('./mock')
let mockModule: Promise<MockModule> | null = null
function mock(): Promise<MockModule> {
  mockModule ??= import('./mock')
  return mockModule
}

export async function call<T>(method: string, args: Record<string, unknown> = {}): Promise<T> {
  try {
    if (!isTauri) return (await mock()).call<T>(method, args)
    const { invoke } = await import('@tauri-apps/api/core')
    return await invoke<T>('call', { method, args })
  } catch (error) {
    throw toApiError(error)
  }
}

export async function onEvent(handler: (event: AppEvent) => void): Promise<() => void> {
  if (!isTauri) return (await mock()).subscribe(handler)
  const { listen } = await import('@tauri-apps/api/event')
  return listen<AppEvent>(EVENT, (e) => handler(e.payload))
}

export async function onQuitRequested(handler: (running: number) => void): Promise<() => void> {
  if (!isTauri) return () => {}
  const { listen } = await import('@tauri-apps/api/event')
  return listen<number>(QUIT_REQUESTED, (e) => handler(e.payload))
}

async function invokeShell<T>(command: string, args: Record<string, unknown> = {}, fallback: T): Promise<T> {
  if (!isTauri) return fallback
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<T>(command, args)
}

async function invokeVoid(command: string, args: Record<string, unknown> = {}): Promise<void> {
  await invokeShell<null>(command, args, null)
}

export const shell = {
  info: () =>
    invokeShell<ShellInfo>(
      'shell_info',
      {},
      {
        glassSupported: false,
        controlSocket: null,
        controlError: null,
        runningProcesses: 0,
      },
    ),
  setWindowTheme: (theme: string) => invokeVoid('set_window_theme', { theme }),
  setTrayLabels: (open: string, quit: string) => invokeVoid('set_tray_labels', { open, quit }),
  quit: () => invokeVoid('quit_app'),
  async pickFolder(defaultPath?: string): Promise<string | null> {
    if (!isTauri) return window.prompt('Folder path', defaultPath ?? '') || null
    const { open } = await import('@tauri-apps/plugin-dialog')
    const result = await open({ directory: true, multiple: false, defaultPath })
    return typeof result === 'string' ? result : null
  },
}
