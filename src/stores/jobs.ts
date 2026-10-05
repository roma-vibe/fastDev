import { defineStore } from 'pinia'
import { ref } from 'vue'
import { api, type JobResponse, type JobSnapshot } from '@/api'
import { type LogBuffer, mergeLog } from '@/lib/logBuffer'

export interface JobSheetOptions {
  /** Called once when the job succeeds. */
  onSuccess?: (job: JobSnapshot) => void
  /** Show the job in the global job sheet (default true). */
  sheet?: boolean
}

export const useJobsStore = defineStore('jobs', () => {
  const jobs = ref<Record<string, JobSnapshot>>({})
  const logs = ref<Record<string, LogBuffer>>({})
  const activeJobId = ref<string | null>(null)
  const callbacks = new Map<string, JobSheetOptions>()

  function update(job: JobSnapshot): void {
    jobs.value = { ...jobs.value, [job.id]: job }
    if (job.status === 'succeeded') {
      const options = callbacks.get(job.id)
      callbacks.delete(job.id)
      options?.onSuccess?.(job)
    }
  }

  function append(jobId: string, start: number, lines: string[]): void {
    logs.value[jobId] = mergeLog(logs.value[jobId], start, lines)
  }

  function lines(jobId: string | null | undefined): string[] {
    return jobId ? (logs.value[jobId]?.lines ?? []) : []
  }

  /** Registers a job started by the UI and shows it in the job sheet. */
  async function track(response: JobResponse, options: JobSheetOptions = {}): Promise<void> {
    callbacks.set(response.jobId, options)
    if (options.sheet !== false) activeJobId.value = response.jobId
    // The job may have progressed before events started arriving; take the full state.
    const full = await api.getJob(response.jobId)
    append(response.jobId, full.logStart, full.log)
    update(full)
  }

  function close(): void {
    activeJobId.value = null
  }

  return { jobs, logs, activeJobId, update, append, lines, track, close }
})
