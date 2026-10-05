<script setup lang="ts">
import { FolderInput, FolderKanban, LibraryBig, Search } from '@lucide/vue'
import { computed, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { api, shell } from '@/api'
import PageHeader from '@/components/layout/PageHeader.vue'
import ProjectRow from '@/components/project/ProjectRow.vue'
import UiButton from '@/components/ui/UiButton.vue'
import UiCard from '@/components/ui/UiCard.vue'
import UiEmpty from '@/components/ui/UiEmpty.vue'
import UiInput from '@/components/ui/UiInput.vue'
import { t, tn } from '@/i18n'
import { useProjectsStore } from '@/stores/projects'
import { useSettingsStore } from '@/stores/settings'
import { useToastStore } from '@/stores/toasts'

const projects = useProjectsStore()
const settings = useSettingsStore()
const toasts = useToastStore()
const router = useRouter()
const query = ref('')

onMounted(() => void projects.load())

const filtered = computed(() => {
  const q = query.value.trim().toLowerCase()
  return projects.projects.filter(
    (p) => !q || [p.name, p.slug, p.path, p.skeletonId ?? ''].some((f) => f.toLowerCase().includes(q)),
  )
})

async function act(action: () => Promise<unknown>): Promise<void> {
  try {
    await action()
  } catch (e) {
    toasts.error(e)
  }
}

async function importProject(): Promise<void> {
  const path = await shell.pickFolder(settings.settings?.projectsDir)
  if (!path) return
  await act(async () => {
    const project = await api.importProject(path)
    toasts.success(t('Project added'), project.path)
    void router.push(`/projects/${project.id}`)
  })
}
</script>

<template>
  <div class="flex h-full flex-col">
    <PageHeader
      :title="t('Projects')"
      :subtitle="tn(projects.projects.length, '{n} project', '{n} projects')"
    >
      <UiInput v-model="query" :icon="Search" :placeholder="t('Search projects')" class="w-56" />
      <UiButton :icon="FolderInput" @click="importProject">{{ t('Add existing') }}</UiButton>
      <RouterLink to="/library" custom>
        <template #default="{ navigate }">
          <UiButton variant="primary" :icon="LibraryBig" @click="navigate">{{ t('New project') }}</UiButton>
        </template>
      </RouterLink>
    </PageHeader>

    <div class="flex-1 overflow-y-auto px-7 pb-8">
      <UiEmpty
        v-if="projects.loaded && projects.projects.length === 0"
        :icon="FolderKanban"
        :title="t('No projects yet')"
        :text="t('Pick a skeleton in the library and create your first project in a few seconds.')"
      >
        <RouterLink to="/library" custom>
          <template #default="{ navigate }">
            <UiButton variant="primary" :icon="LibraryBig" @click="navigate">{{
              t('Browse the library')
            }}</UiButton>
          </template>
        </RouterLink>
      </UiEmpty>
      <UiCard v-else padding="sm" class="flex flex-col gap-0.5">
        <ProjectRow
          v-for="project in filtered"
          :key="project.id"
          :project="project"
          @run="(command) => act(() => api.runCommand(project.id, command))"
          @stop="(command) => act(() => api.stopCommand(project.id, command))"
          @editor="act(() => api.openProject(project.id, 'editor'))"
        />
        <p v-if="filtered.length === 0" class="px-4 py-6 text-center text-[13px] text-fg-3">
          {{ t('Nothing matches the search.') }}
        </p>
      </UiCard>
    </div>
  </div>
</template>
