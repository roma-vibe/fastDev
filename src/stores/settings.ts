import { defineStore } from 'pinia'
import { computed, ref, watch } from 'vue'
import { api, shell, type Settings } from '@/api'
import { detectLocale, locale, t } from '@/i18n'

export const useSettingsStore = defineStore('settings', () => {
  const settings = ref<Settings | null>(null)
  const media = window.matchMedia('(prefers-color-scheme: dark)')
  const systemDark = ref(media.matches)
  media.addEventListener('change', (e) => (systemDark.value = e.matches))

  const theme = computed(() => settings.value?.theme ?? 'system')
  const effectiveTheme = computed<'light' | 'dark'>(() =>
    theme.value === 'system' ? (systemDark.value ? 'dark' : 'light') : theme.value,
  )

  watch(
    effectiveTheme,
    (value) => {
      document.documentElement.dataset.theme = value
    },
    { immediate: true },
  )
  watch(theme, (value) => void shell.setWindowTheme(value))
  watch(
    () => settings.value?.language,
    (language) => {
      locale.value = !language || language === 'system' ? detectLocale() : language
      document.documentElement.lang = locale.value
      void shell.setTrayLabels(t('Open fastDev'), t('Quit fastDev'))
    },
  )

  async function load(): Promise<void> {
    settings.value = await api.getSettings()
  }

  async function update(patch: Partial<Settings>): Promise<void> {
    settings.value = await api.updateSettings(patch)
  }

  function apply(next: Settings): void {
    settings.value = { ...settings.value, ...next }
  }

  const favorites = computed(() => new Set(settings.value?.favorites ?? []))

  return { settings, effectiveTheme, favorites, load, update, apply }
})
