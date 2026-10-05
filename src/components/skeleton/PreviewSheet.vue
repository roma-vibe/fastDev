<script setup lang="ts">
import { CircleCheck, CircleDashed, CircleX, ExternalLink, FolderOpen, Square, Trash } from '@lucide/vue'
import { computed, onMounted, ref, watch } from 'vue'
import { api, type PreviewView, type SkeletonDetails } from '@/api'
import UiButton from '@/components/ui/UiButton.vue'
import UiConsole from '@/components/ui/UiConsole.vue'
import UiSheet from '@/components/ui/UiSheet.vue'
import UiSpinner from '@/components/ui/UiSpinner.vue'
import UiStatusDot from '@/components/ui/UiStatusDot.vue'
import { st, t } from '@/i18n'
import { useJobsStore } from '@/stores/jobs'
import { useRunsStore } from '@/stores/runs'
import { useToastStore } from '@/stores/toasts'

const props = defineProps<{
  details: SkeletonDetails
  features: Record<string, boolean>
  choices: Record<string, string>
  /** Show the preview that is already running instead of starting a new one. */
  attach?: boolean
}>()
const emit = defineEmits<{ close: [] }>()

const jobs = useJobsStore()
const runs = useRunsStore()
const toasts = useToastStore()
const id = computed(() => props.details.summary.id)
const jobId = ref<string | null>(null)
const view = ref<PreviewView | null>(null)
const busy = ref(false)

const job = computed(() => (jobId.value ? jobs.jobs[jobId.value] : undefined))
const jobLog = computed(() => jobs.lines(jobId.value))
const runId = computed(() => view.value?.preview?.runId ?? null)
const run = computed(() => (runId.value ? (runs.runs[runId.value] ?? view.value?.run ?? null) : null))
const running = computed(() => run.value?.status === 'running')
const runLines = computed(() => runs.lines(runId.value))

function show(result: PreviewView): void {
  view.value = result
  if (result.run) {
    runs.update(result.run)
    runs.append(result.run.id, result.start, result.lines)
  }
}

async function start(): Promise<void> {
  try {
    const response = await api.previewSkeleton(id.value, props.details.target, props.features, props.choices)
    jobId.value = response.jobId
    await jobs.track(response, { sheet: false, onSuccess: (done) => show(done.result as PreviewView) })
  } catch (e) {
    toasts.error(e)
    emit('close')
  }
}

onMounted(async () => {
  if (props.attach) show(await api.getPreview(id.value))
  else await start()
})

// A job that finished before its result arrived through events.
watch(job, (value) => {
  if (value?.status === 'succeeded' && !view.value && value.result) show(value.result as PreviewView)
})

async function stop(): Promise<void> {
  busy.value = true
  try {
    await api.stopPreview(id.value)
  } catch (e) {
    toasts.error(e)
  } finally {
    busy.value = false
  }
}

async function remove(): Promise<void> {
  busy.value = true
  try {
    await api.deletePreview(id.value)
    toasts.success(t('Preview files deleted'))
    emit('close')
  } catch (e) {
    toasts.error(e)
  } finally {
    busy.value = false
  }
}

async function open(target: string, kind: 'url' | 'path'): Promise<void> {
  try {
    if (kind === 'url') await api.openUrl(target)
    else await api.openPath(target)
  } catch (e) {
    toasts.error(e)
  }
}

const nextSteps = computed(() => {
  const preview = view.value?.preview
  if (!preview) return ''
  if (preview.message) return st(preview.message, preview.translations)
  if (preview.long)
    return preview.url
      ? t('Open the link to try it. Stop the preview when you are done.')
      : t('The preview is running.')
  return preview.path ? t('Finished. Open the result below.') : t('Finished.')
})
</script>

<template>
  <UiSheet
    :title="t('Preview of {name}', { name: st(details.summary.name, details.summary.translations) })"
    :subtitle="t('A throwaway copy in fastDev’s data folder; no project is created.')"
    width="lg"
    @close="emit('close')"
  >
    <div class="flex flex-col gap-4">
      <!-- Preparing -->
      <template v-if="!view">
        <ol v-if="job" class="grid grid-cols-2 gap-x-6 gap-y-1.5">
          <li v-for="(step, index) in job.steps" :key="index" class="flex items-center gap-2 text-[13px]">
            <UiSpinner v-if="step.status === 'running'" :size="15" class="text-accent" />
            <CircleCheck v-else-if="step.status === 'done'" :size="15" class="text-success" />
            <CircleX v-else-if="step.status === 'failed'" :size="15" class="text-danger" />
            <CircleDashed v-else :size="15" class="text-fg-3" />
            <span :class="step.status === 'pending' ? 'text-fg-3' : ''">{{
              st(step.label, details.manifest?.translations)
            }}</span>
          </li>
        </ol>
        <div v-else class="flex justify-center py-6 text-fg-3"><UiSpinner :size="20" /></div>
        <div v-if="job?.error" class="selectable rounded-2xl bg-danger/10 px-4 py-3 text-[13px] text-danger">
          {{ job.error.message }}
        </div>
        <UiConsole :lines="jobLog" height="260px" :placeholder="t('Preparing the preview…')" />
      </template>

      <!-- Running or finished -->
      <template v-else-if="view.preview">
        <div class="flex flex-col gap-3 rounded-card bg-accent/8 px-5 py-4">
          <div class="flex items-center gap-2 text-[13px] font-semibold">
            <UiStatusDot :status="running ? 'running' : run?.exitCode ? 'failed' : 'idle'" />
            {{
              running
                ? t('{label} is running', { label: st(view.preview.label, view.preview.translations) })
                : t('{label} finished', { label: st(view.preview.label, view.preview.translations) })
            }}
          </div>
          <p class="selectable text-[13.5px] leading-relaxed">{{ nextSteps }}</p>
          <div v-if="view.preview.url" class="flex items-center gap-2">
            <code
              class="selectable min-w-0 flex-1 truncate rounded-xl bg-surface-strong px-3 py-2 font-mono text-[12.5px]"
              >{{ view.preview.url }}</code
            >
            <UiButton
              variant="primary"
              :icon="ExternalLink"
              :disabled="!running && view.preview.long"
              @click="open(view.preview.url, 'url')"
            >
              {{ t('Open in browser') }}
            </UiButton>
          </div>
          <div v-if="view.preview.path" class="flex items-center gap-2">
            <code
              class="selectable min-w-0 flex-1 truncate rounded-xl bg-surface-strong px-3 py-2 font-mono text-[12.5px]"
              >{{ view.preview.path }}</code
            >
            <UiButton :icon="FolderOpen" @click="open(view.preview.path, 'path')">{{
              t('Show in Finder')
            }}</UiButton>
          </div>
        </div>
        <UiConsole :lines="runLines" height="240px" :placeholder="t('Waiting for output…')" />
      </template>
      <p v-else class="text-[13px] text-fg-3">{{ t('No preview is running.') }}</p>
    </div>

    <template #footer>
      <UiButton v-if="view?.preview" variant="ghost" :icon="Trash" :disabled="busy" @click="remove">
        {{ t('Delete preview files') }}
      </UiButton>
      <span class="flex-1" />
      <UiButton v-if="running" variant="danger" :icon="Square" :loading="busy" @click="stop">{{
        t('Stop')
      }}</UiButton>
      <UiButton variant="secondary" @click="emit('close')">{{ running ? t('Hide') : t('Close') }}</UiButton>
    </template>
  </UiSheet>
</template>
