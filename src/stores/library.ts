import { defineStore } from 'pinia'
import { ref } from 'vue'
import { api, type SkeletonDetails, type SkeletonSummary } from '@/api'

export const useLibraryStore = defineStore('library', () => {
  const skeletons = ref<SkeletonSummary[]>([])
  const workspacePath = ref<string | null>(null)
  const registries = ref<string[]>([])
  const loaded = ref(false)
  const error = ref<string | null>(null)
  /** Details by "id@version" (version "" = default target). */
  const details = ref<Record<string, SkeletonDetails>>({})
  const open = new Map<string, { id: string; version?: string }>()

  async function load(): Promise<void> {
    try {
      const result = await api.listSkeletons()
      skeletons.value = result.skeletons
      workspacePath.value = result.workspacePath
      registries.value = result.registries
      error.value = null
    } catch (e) {
      skeletons.value = []
      error.value = e instanceof Error ? e.message : String(e)
    } finally {
      loaded.value = true
    }
  }

  const key = (id: string, version?: string) => `${id}@${version ?? ''}`

  async function loadDetails(id: string, version?: string): Promise<SkeletonDetails> {
    open.set(key(id, version), { id, version })
    const result = await api.getSkeleton(id, version)
    details.value = { ...details.value, [key(id, version)]: result }
    return result
  }

  function cached(id: string, version?: string): SkeletonDetails | undefined {
    return details.value[key(id, version)]
  }

  function forget(id: string, version?: string): void {
    open.delete(key(id, version))
  }

  async function refresh(): Promise<void> {
    await load()
    await Promise.all(
      [...open.values()].map(({ id, version }) => loadDetails(id, version).catch(() => undefined)),
    )
  }

  async function toggleFavorite(id: string, favorite: boolean): Promise<void> {
    await api.setFavorite(id, favorite)
    skeletons.value = skeletons.value.map((s) => (s.id === id ? { ...s, favorite } : s))
  }

  return {
    skeletons,
    workspacePath,
    registries,
    loaded,
    error,
    details,
    load,
    loadDetails,
    cached,
    forget,
    refresh,
    toggleFavorite,
  }
})
