<script setup lang="ts">
import { CircleCheck, CircleDashed, CircleMinus, CircleX, TriangleAlert } from '@lucide/vue'
import { computed } from 'vue'
import { useRouter } from 'vue-router'
import UiButton from '@/components/ui/UiButton.vue'
import UiConsole from '@/components/ui/UiConsole.vue'
import UiSheet from '@/components/ui/UiSheet.vue'
import UiSpinner from '@/components/ui/UiSpinner.vue'
import { st, t } from '@/i18n'
import { useJobsStore } from '@/stores/jobs'

const jobs = useJobsStore()
const router = useRouter()

const job = computed(() => (jobs.activeJobId ? jobs.jobs[jobs.activeJobId] : undefined))
const log = computed(() => jobs.lines(jobs.activeJobId))
const createdProjectId = computed(() => {
  const result = job.value?.result as { project?: { id: string } } | null | undefined
  return result?.project?.id
})

const kindTitle = computed(() => {
  switch (job.value?.kind) {
    case 'create_project':
      return t('Creating project')
    case 'verify_skeleton':
      return t('Verifying skeleton')
    case 'check_skeleton_updates':
      return t('Checking dependency updates')
    case 'apply_skeleton_updates':
      return t('Updating dependencies')
    case 'run_project_setup':
      return t('Running setup')
    case 'preview_skeleton':
      return t('Preparing the preview…')
    default:
      return job.value?.title ?? ''
  }
})

const statusText = computed(() =>
  job.value?.status === 'running'
    ? t('Working…')
    : job.value?.status === 'succeeded'
      ? t('Finished')
      : t('Failed'),
)

function openProject(): void {
  const id = createdProjectId.value
  jobs.close()
  if (id) void router.push(`/projects/${id}`)
}
</script>

<template>
  <UiSheet
    v-if="job"
    :title="kindTitle"
    :subtitle="`${job.title} · ${statusText}`"
    width="lg"
    @close="jobs.close()"
  >
    <div class="flex flex-col gap-4">
      <ol class="grid grid-cols-2 gap-x-6 gap-y-1.5">
        <li v-for="(step, index) in job.steps" :key="index" class="flex items-center gap-2 text-[13px]">
          <UiSpinner v-if="step.status === 'running'" :size="15" class="text-accent" />
          <CircleCheck v-else-if="step.status === 'done'" :size="15" class="text-success" />
          <CircleX v-else-if="step.status === 'failed'" :size="15" class="text-danger" />
          <CircleMinus v-else-if="step.status === 'skipped'" :size="15" class="text-fg-3" />
          <CircleDashed v-else :size="15" class="text-fg-3" />
          <span :class="step.status === 'pending' || step.status === 'skipped' ? 'text-fg-3' : 'text-fg'">
            {{ st(step.label, job.translations) }}
          </span>
        </li>
      </ol>

      <div v-if="job.error" class="selectable rounded-2xl bg-danger/10 px-4 py-3 text-[13px] text-danger">
        {{ job.error.message }}
      </div>
      <div
        v-for="(warning, index) in job.warnings"
        :key="index"
        class="flex gap-2 rounded-2xl bg-warning/12 px-4 py-2.5 text-[12.5px] text-warning"
      >
        <TriangleAlert :size="15" class="mt-0.5 shrink-0" />
        <span class="selectable">{{ warning }}</span>
      </div>

      <UiConsole :lines="log" height="300px" :placeholder="t('Waiting for output…')" />
    </div>

    <template #footer>
      <UiButton v-if="job.status === 'running'" variant="ghost" @click="jobs.close()">{{
        t('Hide')
      }}</UiButton>
      <template v-else>
        <UiButton variant="secondary" @click="jobs.close()">{{ t('Close') }}</UiButton>
        <UiButton v-if="createdProjectId" variant="primary" @click="openProject">{{
          t('Open project')
        }}</UiButton>
      </template>
    </template>
  </UiSheet>
</template>
