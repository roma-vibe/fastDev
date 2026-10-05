import { defineStore } from 'pinia'
import { ref } from 'vue'
import { api, type RunInfo } from '@/api'
import { clearLog, type LogBuffer, mergeLog } from '@/lib/logBuffer'

export const useRunsStore = defineStore('runs', () => {
  const runs = ref<Record<string, RunInfo>>({})
  const logs = ref<Record<string, LogBuffer>>({})

  function update(run: RunInfo): void {
    runs.value = { ...runs.value, [run.id]: run }
  }

  function append(runId: string, start: number, lines: string[]): void {
    logs.value[runId] = mergeLog(logs.value[runId], start, lines)
  }

  function lines(runId: string | null | undefined): string[] {
    return runId ? (logs.value[runId]?.lines ?? []) : []
  }

  async function fetchLogs(runId: string): Promise<void> {
    const result = await api.runLogs(runId)
    update(result.run)
    append(runId, result.start, result.lines)
  }

  async function clear(runId: string): Promise<void> {
    await api.clearRunLogs(runId)
    logs.value[runId] = clearLog(logs.value[runId])
  }

  function forProject(projectId: string): RunInfo[] {
    return Object.values(runs.value)
      .filter((r) => r.projectId === projectId)
      .sort((a, b) => b.startedAt.localeCompare(a.startedAt))
  }

  return { runs, logs, update, append, lines, fetchLogs, clear, forProject }
})
