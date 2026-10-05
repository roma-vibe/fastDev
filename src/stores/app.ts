import { defineStore } from 'pinia'
import { ref } from 'vue'
import { api, shell, type AppInfo, type ShellInfo } from '@/api'

export const useAppStore = defineStore('app', () => {
  const shellInfo = ref<ShellInfo | null>(null)
  const appInfo = ref<AppInfo | null>(null)
  const quitRequest = ref<number | null>(null)

  async function load(): Promise<void> {
    const [s, a] = await Promise.all([shell.info(), api.appInfo()])
    shellInfo.value = s
    appInfo.value = a
    document.documentElement.classList.toggle('no-glass', !s.glassSupported)
  }

  return { shellInfo, appInfo, quitRequest, load }
})
