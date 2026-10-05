import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { api, type Project } from '@/api'

export const useProjectsStore = defineStore('projects', () => {
  const projects = ref<Project[]>([])
  const loaded = ref(false)
  let timer: ReturnType<typeof setTimeout> | undefined

  async function load(): Promise<void> {
    const result = await api.listProjects()
    projects.value = result.projects
    loaded.value = true
  }

  /** Coalesces bursts of events (runs starting and stopping) into one reload. */
  function reloadSoon(): void {
    clearTimeout(timer)
    timer = setTimeout(() => void load(), 150)
  }

  function byId(id: string): Project | undefined {
    return projects.value.find((p) => p.id === id)
  }

  const runningCount = computed(() => projects.value.reduce((sum, p) => sum + p.running.length, 0))

  return { projects, loaded, load, reloadSoon, byId, runningCount }
})
