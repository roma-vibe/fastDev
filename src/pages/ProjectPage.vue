<script setup lang="ts">
import {
  Code,
  Copy,
  Eraser,
  ExternalLink,
  FolderOpen,
  FolderSearch,
  Package,
  SquareTerminal,
  Trash,
  TriangleAlert,
  X,
} from '@lucide/vue'
import { computed, onMounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { api, shell, type CommandView, type Project } from '@/api'
import CommandCard from '@/components/project/CommandCard.vue'
import CommandInputsSheet from '@/components/project/CommandInputsSheet.vue'
import PageHeader from '@/components/layout/PageHeader.vue'
import UiBadge from '@/components/ui/UiBadge.vue'
import UiButton from '@/components/ui/UiButton.vue'
import UiCard from '@/components/ui/UiCard.vue'
import UiConsole from '@/components/ui/UiConsole.vue'
import UiEmpty from '@/components/ui/UiEmpty.vue'
import UiIconButton from '@/components/ui/UiIconButton.vue'
import UiInput from '@/components/ui/UiInput.vue'
import UiSection from '@/components/ui/UiSection.vue'
import UiSheet from '@/components/ui/UiSheet.vue'
import UiSpinner from '@/components/ui/UiSpinner.vue'
import UiStatusDot from '@/components/ui/UiStatusDot.vue'
import { formatDate, st, t } from '@/i18n'
import { useJobsStore } from '@/stores/jobs'
import { useProjectsStore } from '@/stores/projects'
import { useRunsStore } from '@/stores/runs'
import { useToastStore } from '@/stores/toasts'

const props = defineProps<{ id: string }>()
const router = useRouter()
const projects = useProjectsStore()
const runs = useRunsStore()
const jobs = useJobsStore()
const toasts = useToastStore()

const project = ref<Project | null>(null)
const error = ref<string | null>(null)
const selectedRun = ref<string | null>(null)
const deleteOpen = ref(false)
const deleteConfirm = ref('')

async function load(): Promise<void> {
  try {
    project.value = await api.getProject(props.id)
    error.value = null
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e)
  }
}

onMounted(async () => {
  await load()
  void api.touchProject(props.id).catch(() => undefined)
  const all = await api.listRuns()
  for (const run of all.runs.filter((r) => r.projectId === props.id)) {
    runs.update(run)
    await runs.fetchLogs(run.id)
  }
})

// Keep the page in sync with the list (events reload the list).
watch(
  () => projects.byId(props.id),
  (fresh) => {
    if (fresh) project.value = fresh
  },
)

const projectRuns = computed(() => runs.forProject(props.id))
watch(
  projectRuns,
  (list) => {
    if (!selectedRun.value || !list.some((r) => r.id === selectedRun.value))
      selectedRun.value = list[0]?.id ?? null
  },
  { immediate: true },
)
const currentRun = computed(() => (selectedRun.value ? runs.runs[selectedRun.value] : undefined))
const currentLines = computed(() => runs.lines(selectedRun.value))

function runFor(key: string) {
  return projectRuns.value.find((r) => r.command === key)
}

const commands = computed(() =>
  [...(project.value?.commands ?? [])].sort((a, b) => Number(b.primary) - Number(a.primary)),
)

async function act(action: () => Promise<unknown>): Promise<void> {
  try {
    await action()
  } catch (e) {
    toasts.error(e)
  }
}

/** The command whose inputs are being asked for. */
const asking = ref<CommandView | null>(null)

async function run(command: CommandView, inputs?: Record<string, string>): Promise<void> {
  if (command.inputs?.length && !inputs) {
    asking.value = command
    return
  }
  asking.value = null
  await act(async () => {
    const result = await api.runCommand(props.id, command.key, inputs)
    runs.update(result.run)
    selectedRun.value = result.run.id
  })
}

async function stop(key: string): Promise<void> {
  await act(() => api.stopCommand(props.id, key))
}

async function runSetup(): Promise<void> {
  await act(async () => jobs.track(await api.runSetup(props.id), { onSuccess: () => void load() }))
}

async function locate(): Promise<void> {
  const path = await shell.pickFolder()
  if (path) await act(async () => (project.value = await api.locateProject(props.id, path)))
}

async function remove(): Promise<void> {
  await act(async () => {
    await api.removeProject(props.id)
    toasts.success(t('Removed from the list'), t('Files on disk were not touched.'))
    void router.push('/projects')
  })
}

async function deleteFromDisk(): Promise<void> {
  await act(async () => {
    await api.deleteProjectFromDisk(props.id, deleteConfirm.value)
    deleteOpen.value = false
    toasts.success(t('Project deleted'))
    void router.push('/projects')
  })
}

async function openCurrentUrl(): Promise<void> {
  const url = currentRun.value?.url
  if (url) await act(() => api.openUrl(url))
}

async function clearCurrent(): Promise<void> {
  const runId = selectedRun.value
  if (runId) await act(() => runs.clear(runId))
}

async function copyLog(): Promise<void> {
  await navigator.clipboard.writeText(currentLines.value.join('\n'))
  toasts.success(t('Log copied'))
}

const ports = computed(() => Object.entries(project.value?.ports ?? {}))
const features = computed(() =>
  Object.entries(project.value?.features ?? {})
    .filter(([, on]) => on)
    .map(([name]) => name),
)
</script>

<template>
  <div class="flex h-full flex-col">
    <PageHeader :title="project?.name ?? t('Project')" :subtitle="project?.path" back="/projects">
      <template #badges>
        <UiBadge v-if="project?.skeletonId" mono
          >{{ project.skeletonId }} {{ project.skeletonVersion }}</UiBadge
        >
        <UiBadge v-for="feature in features" :key="feature" tone="accent">{{ feature }}</UiBadge>
        <UiBadge v-for="(value, key) in project?.choices ?? {}" :key="key" tone="accent" mono
          >{{ key }}: {{ value }}</UiBadge
        >
      </template>
      <template v-if="project?.exists">
        <UiIconButton
          :icon="Code"
          :label="t('Open in editor')"
          @click="act(() => api.openProject(id, 'editor'))"
        />
        <UiIconButton
          :icon="SquareTerminal"
          :label="t('Open in terminal')"
          @click="act(() => api.openProject(id, 'terminal'))"
        />
        <UiIconButton
          :icon="FolderOpen"
          :label="t('Show in Finder')"
          @click="act(() => api.openProject(id, 'finder'))"
        />
      </template>
    </PageHeader>

    <div class="flex-1 overflow-y-auto px-7 pb-10">
      <UiEmpty v-if="error" :icon="TriangleAlert" :title="t('Project not found')" :text="error" />
      <div v-else-if="!project" class="flex justify-center py-20 text-fg-3"><UiSpinner :size="22" /></div>
      <div v-else class="flex flex-col gap-7">
        <div
          v-if="!project.exists"
          class="flex flex-wrap items-center gap-3 rounded-card bg-warning/10 px-5 py-4 text-warning"
        >
          <TriangleAlert :size="18" />
          <span class="flex-1 text-[13px]">{{ t('The project folder was moved or deleted.') }}</span>
          <UiButton size="sm" :icon="FolderSearch" @click="locate">{{ t('Locate…') }}</UiButton>
          <UiButton size="sm" variant="danger" :icon="X" @click="remove">{{
            t('Remove from list')
          }}</UiButton>
        </div>
        <div
          v-else-if="project.setupStatus === 'failed'"
          class="flex flex-wrap items-center gap-3 rounded-card bg-danger/10 px-5 py-4 text-danger"
        >
          <TriangleAlert :size="18" />
          <span class="flex-1 text-[13px]">{{
            t('Setup did not finish (for example, npm install failed).')
          }}</span>
          <UiButton size="sm" :icon="Package" @click="runSetup">{{ t('Run setup again') }}</UiButton>
        </div>
        <div
          v-if="project.metaError"
          class="selectable rounded-card bg-warning/10 px-5 py-3 text-[12.5px] text-warning"
        >
          {{ project.metaError }}
        </div>
        <div
          v-if="project.updateAvailable"
          class="flex items-center gap-2 rounded-card bg-accent/8 px-5 py-3 text-[13px] text-fg-2"
        >
          <UiBadge tone="accent">{{ t('update') }}</UiBadge>
          {{
            t('Skeleton {id} {version} is available. Existing projects are not changed automatically.', {
              id: project.skeletonId ?? '',
              version: project.latestVersion ?? '',
            })
          }}
          <RouterLink
            :to="`/library/${project.skeletonId}?tab=versions`"
            class="ml-auto font-medium text-accent hover:underline"
          >
            {{ t('What changed') }}
          </RouterLink>
        </div>

        <UiSection :title="t('Commands')">
          <template #actions>
            <UiButton
              v-if="project.setup.length && project.exists"
              size="sm"
              variant="ghost"
              :icon="Package"
              @click="runSetup"
            >
              {{ t('Run setup') }}
            </UiButton>
          </template>
          <div v-if="commands.length" class="grid grid-cols-[repeat(auto-fill,minmax(210px,1fr))] gap-3">
            <CommandCard
              v-for="command in commands"
              :key="command.key"
              :command="command"
              :run="runFor(command.key)"
              :disabled="!project.exists"
              :translations="project.translations"
              @run="run(command)"
              @stop="stop(command.key)"
              @open="(url) => act(() => api.openUrl(url))"
            />
          </div>
          <p v-else class="text-[13px] text-fg-3">
            {{ t('This project has no commands. Add them to .fastdev.toml.') }}
          </p>
        </UiSection>

        <UiSection :title="t('Output')">
          <template #actions>
            <UiIconButton
              v-if="currentRun?.url && currentRun.status === 'running'"
              :icon="ExternalLink"
              :label="t('Open in browser')"
              size="sm"
              @click="openCurrentUrl"
            />
            <UiIconButton
              :icon="Copy"
              :label="t('Copy log')"
              size="sm"
              :disabled="!currentLines.length"
              @click="copyLog"
            />
            <UiIconButton
              :icon="Eraser"
              :label="t('Clear')"
              size="sm"
              :disabled="!selectedRun"
              @click="clearCurrent"
            />
          </template>
          <div v-if="projectRuns.length" class="flex flex-wrap gap-1.5">
            <button
              v-for="r in projectRuns"
              :key="r.id"
              type="button"
              class="focus-ring flex h-7 items-center gap-2 rounded-full px-3 text-[12.5px] transition-colors"
              :class="
                selectedRun === r.id
                  ? 'bg-accent-soft font-semibold text-accent-fg'
                  : 'bg-fill text-fg-2 hover:bg-fill-2'
              "
              @click="selectedRun = r.id"
            >
              <UiStatusDot
                :status="
                  r.status === 'running' ? 'running' : r.exitCode && r.status === 'exited' ? 'failed' : 'idle'
                "
              />
              {{ st(r.label, project.translations) }}
            </button>
          </div>
          <UiConsole
            :lines="currentLines"
            height="340px"
            :placeholder="t('Run a command to see its output here.')"
          />
        </UiSection>

        <UiSection :title="t('Details')">
          <UiCard padding="none" class="divide-y divide-hairline text-[13px]">
            <div class="flex gap-4 px-5 py-2.5">
              <span class="w-40 shrink-0 text-fg-3">{{ t('Created') }}</span>
              <span>{{ formatDate(project.createdAt) }}</span>
            </div>
            <div class="flex gap-4 px-5 py-2.5">
              <span class="w-40 shrink-0 text-fg-3">{{ t('Skeleton') }}</span>
              <RouterLink
                v-if="project.skeletonId"
                :to="`/library/${project.skeletonId}`"
                class="text-accent hover:underline"
              >
                {{ project.skeletonId }} {{ project.skeletonVersion }}
              </RouterLink>
              <span v-else class="text-fg-3">—</span>
            </div>
            <div v-if="ports.length" class="flex gap-4 px-5 py-2.5">
              <span class="w-40 shrink-0 text-fg-3">{{ t('Ports') }}</span>
              <span class="flex flex-wrap gap-3 font-mono text-[12px]">
                <span v-for="[key, port] in ports" :key="key">{{ key }}={{ port }}</span>
              </span>
            </div>
            <div class="flex gap-4 px-5 py-2.5">
              <span class="w-40 shrink-0 text-fg-3">{{ t('Id') }}</span>
              <span class="selectable font-mono text-[12px]">{{ project.id }}</span>
            </div>
          </UiCard>
        </UiSection>

        <UiSection :title="t('Danger zone')">
          <div class="flex flex-wrap gap-2">
            <UiButton :icon="X" @click="remove">{{ t('Remove from list') }}</UiButton>
            <UiButton
              v-if="project.exists"
              variant="danger"
              :icon="Trash"
              @click="((deleteConfirm = ''), (deleteOpen = true))"
            >
              {{ t('Delete from disk…') }}
            </UiButton>
          </div>
        </UiSection>
      </div>
    </div>

    <CommandInputsSheet
      v-if="asking && project"
      :command="asking"
      :translations="project.translations"
      @run="(inputs) => asking && run(asking, inputs)"
      @close="asking = null"
    />

    <UiSheet
      v-if="deleteOpen && project"
      :title="t('Delete {name}?', { name: project.name })"
      width="sm"
      @close="deleteOpen = false"
    >
      <div class="flex flex-col gap-3 pb-2 text-[13px] text-fg-2">
        <p>
          {{
            t('The folder {path} and everything in it will be deleted. This cannot be undone.', {
              path: project.path,
            })
          }}
        </p>
        <p>{{ t('Type {slug} to confirm.', { slug: project.slug }) }}</p>
        <UiInput v-model="deleteConfirm" mono :placeholder="project.slug" />
      </div>
      <template #footer>
        <UiButton variant="secondary" @click="deleteOpen = false">{{ t('Cancel') }}</UiButton>
        <UiButton variant="danger" :disabled="deleteConfirm !== project.slug" @click="deleteFromDisk">{{
          t('Delete')
        }}</UiButton>
      </template>
    </UiSheet>
  </div>
</template>
