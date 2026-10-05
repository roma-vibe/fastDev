import { createPinia } from 'pinia'
import { createApp } from 'vue'
import { isTauri, onEvent, onQuitRequested } from '@/api'
import App from './App.vue'
import { router } from './router'
import { useAppStore } from './stores/app'
import { useJobsStore } from './stores/jobs'
import { useLibraryStore } from './stores/library'
import { useProjectsStore } from './stores/projects'
import { useRunsStore } from './stores/runs'
import { useSettingsStore } from './stores/settings'
import './styles/main.css'

if (!isTauri) document.documentElement.classList.add('no-glass')

const app = createApp(App)
const pinia = createPinia()
app.use(pinia).use(router)

const settings = useSettingsStore()
const library = useLibraryStore()
const projects = useProjectsStore()
const runs = useRunsStore()
const jobs = useJobsStore()
const shellStore = useAppStore()

void onEvent((event) => {
  switch (event.type) {
    case 'jobUpdated':
      jobs.update(event.job)
      break
    case 'jobOutput':
      jobs.append(event.jobId, event.start, event.lines)
      break
    case 'runUpdated':
      runs.update(event.run)
      projects.reloadSoon()
      break
    case 'runOutput':
      runs.append(event.runId, event.start, event.lines)
      break
    case 'projectsChanged':
      projects.reloadSoon()
      break
    case 'libraryChanged':
      void library.refresh()
      break
    case 'settingsChanged':
      settings.apply(event.settings)
      break
  }
})
void onQuitRequested((running) => (shellStore.quitRequest = running))

// Disable the webview context menu and reload shortcuts in the packaged app.
if (isTauri && !import.meta.env.DEV) {
  window.addEventListener('contextmenu', (e) => {
    const target = e.target as HTMLElement
    if (!target.closest('input, textarea, .selectable')) e.preventDefault()
  })
}

await Promise.allSettled([settings.load(), shellStore.load(), library.load(), projects.load()])
app.mount('#app')
