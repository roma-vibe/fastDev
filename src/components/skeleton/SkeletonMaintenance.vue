<script setup lang="ts">
import {
  BadgeCheck,
  CircleAlert,
  Code,
  FolderOpen,
  GitFork,
  ListChecks,
  PencilLine,
  RefreshCw,
  ShieldCheck,
  Trash,
  TriangleAlert,
  Upload,
} from '@lucide/vue'
import { computed, ref } from 'vue'
import { api, type OutdatedPackage, type SkeletonDetails, type ValidationReport } from '@/api'
import UiBadge from '@/components/ui/UiBadge.vue'
import UiButton from '@/components/ui/UiButton.vue'
import UiCard from '@/components/ui/UiCard.vue'
import UiSection from '@/components/ui/UiSection.vue'
import UiSheet from '@/components/ui/UiSheet.vue'
import { formatRelative, t, tn } from '@/i18n'
import { useJobsStore } from '@/stores/jobs'
import { useToastStore } from '@/stores/toasts'
import ForkSheet from './ForkSheet.vue'
import PublishSheet from './PublishSheet.vue'

const props = defineProps<{ details: SkeletonDetails }>()
const emit = defineEmits<{ changed: [target?: string] }>()
const jobs = useJobsStore()
const toasts = useToastStore()

const summary = computed(() => props.details.summary)
const draft = computed(() => summary.value.draft)
const report = ref<ValidationReport | null>(null)
const outdated = ref<OutdatedPackage[] | null>(null)
const selected = ref<Set<string>>(new Set())
const busy = ref<string | null>(null)
const forkOpen = ref(false)
const publishOpen = ref(false)
const discardOpen = ref(false)

const verification = computed(() => {
  if (!draft.value) return null
  if (props.details.target !== 'draft') return null
  if (props.details.draftVerified === true) return 'verified'
  if (draft.value.verification?.passed) return 'changed'
  if (draft.value.verification) return 'failed'
  return 'never'
})

const kindLabel = computed(() => {
  switch (draft.value?.kind) {
    case 'new':
      return t('New skeleton')
    case 'fork':
      return t('Fork of {id} {version}', { id: draft.value.source ?? '', version: draft.value.basedOn ?? '' })
    default:
      return t('Next version, based on {version}', { version: draft.value?.basedOn ?? '' })
  }
})

async function run<T>(key: string, action: () => Promise<T>): Promise<T | undefined> {
  busy.value = key
  try {
    return await action()
  } catch (e) {
    toasts.error(e)
    return undefined
  } finally {
    busy.value = null
  }
}

async function startDraft(): Promise<void> {
  const result = await run('draft', () => api.createDraft({ mode: 'edit', id: summary.value.id }))
  if (result) {
    toasts.success(t('Draft created'), result.draft.path)
    emit('changed', 'draft')
  }
}

async function validate(): Promise<void> {
  report.value =
    (await run('validate', () => api.validateSkeleton(summary.value.id, props.details.target))) ?? null
}

async function verify(): Promise<void> {
  const response = await run('verify', () => api.verifySkeleton(summary.value.id, props.details.target))
  if (response) await jobs.track(response, { onSuccess: () => emit('changed') })
}

async function discard(): Promise<void> {
  const result = await run('discard', () => api.discardDraft(summary.value.id))
  discardOpen.value = false
  if (result) {
    toasts.success(t('Draft discarded'))
    emit('changed', summary.value.latest ?? undefined)
  }
}

async function checkUpdates(): Promise<void> {
  const response = await run('updates', () => api.checkUpdates(summary.value.id, props.details.target))
  if (!response) return
  await jobs.track(response, {
    onSuccess: (job) => {
      const result = job.result as { packages?: OutdatedPackage[] }
      outdated.value = result.packages ?? []
      selected.value = new Set(outdated.value.filter((p) => p.level !== 'major' && !p.held).map(key))
      jobs.close()
    },
  })
}

const key = (p: OutdatedPackage) => `${p.workspace}:${p.name}`

function toggle(p: OutdatedPackage): void {
  const next = new Set(selected.value)
  if (next.has(key(p))) next.delete(key(p))
  else next.add(key(p))
  selected.value = next
}

async function applyUpdates(): Promise<void> {
  const packages = (outdated.value ?? [])
    .filter((p) => selected.value.has(key(p)))
    .map((p) => ({ name: p.name, workspace: p.workspace }))
  const response = await run('apply', () => api.applyUpdates(summary.value.id, packages))
  if (response)
    await jobs.track(response, {
      onSuccess: () => {
        outdated.value = null
        emit('changed', 'draft')
      },
    })
}

async function openDraft(target: 'finder' | 'editor'): Promise<void> {
  const current = draft.value
  if (current) await run('open', () => api.openPath(current.path, target))
}

const levelTone = { patch: 'success', minor: 'accent', major: 'warning' } as const
</script>

<template>
  <div class="flex flex-col gap-7">
    <UiSection
      :title="t('Draft')"
      :description="t('Changes are made in a draft and published as a new immutable version.')"
    >
      <UiCard v-if="draft" class="flex flex-col gap-4">
        <div class="flex flex-wrap items-center gap-2">
          <UiBadge tone="warning" :icon="PencilLine">{{ t('Draft') }}</UiBadge>
          <span class="text-[13px] text-fg-2">{{ kindLabel }}</span>
          <span class="ml-auto text-[12px] text-fg-3">{{ formatRelative(draft.createdAt) }}</span>
        </div>
        <code class="selectable truncate rounded-xl bg-fill px-3 py-2 font-mono text-[12px]">{{
          draft.path
        }}</code>

        <div
          class="flex items-center gap-2 rounded-xl px-3 py-2 text-[12.5px]"
          :class="
            {
              verified: 'bg-success/10 text-success',
              changed: 'bg-warning/10 text-warning',
              failed: 'bg-danger/10 text-danger',
              never: 'bg-fill text-fg-2',
            }[verification ?? 'never']
          "
        >
          <component
            :is="
              verification === 'verified'
                ? BadgeCheck
                : verification === 'failed'
                  ? CircleAlert
                  : TriangleAlert
            "
            :size="15"
          />
          <span v-if="details.target !== 'draft'">{{
            t('Select the draft in the version menu to work with it.')
          }}</span>
          <span v-else-if="verification === 'verified'"
            >{{ t('Verified — ready to publish.') }}
            <template v-if="draft?.verification?.skipped?.length">
              {{
                t('Not verified on this Mac (tools missing): {list}', {
                  list: draft.verification.skipped.join('; '),
                })
              }}
            </template>
          </span>
          <span v-else-if="verification === 'changed'">{{
            t('Changed since the last verification. Verify again before publishing.')
          }}</span>
          <span v-else-if="verification === 'failed'">{{ t('The last verification failed.') }}</span>
          <span v-else>{{ t('Not verified yet.') }}</span>
        </div>

        <div class="flex flex-wrap gap-2">
          <UiButton :icon="FolderOpen" size="sm" @click="openDraft('finder')">{{ t('Finder') }}</UiButton>
          <UiButton :icon="Code" size="sm" @click="openDraft('editor')">{{ t('Editor') }}</UiButton>
          <span class="flex-1" />
          <template v-if="details.target === 'draft'">
            <UiButton :icon="ListChecks" size="sm" :loading="busy === 'validate'" @click="validate">{{
              t('Validate')
            }}</UiButton>
            <UiButton :icon="ShieldCheck" size="sm" :loading="busy === 'verify'" @click="verify">{{
              t('Verify')
            }}</UiButton>
            <UiButton :icon="Upload" size="sm" variant="primary" @click="publishOpen = true">{{
              t('Publish…')
            }}</UiButton>
          </template>
          <UiButton :icon="Trash" size="sm" variant="danger" @click="discardOpen = true">{{
            t('Discard')
          }}</UiButton>
        </div>
      </UiCard>

      <UiCard v-else class="flex flex-wrap items-center gap-3">
        <p class="min-w-0 flex-1 text-[13px] text-fg-2">
          {{ t('No draft. Start a new version or fork this skeleton into a separate one.') }}
        </p>
        <UiButton :icon="PencilLine" :loading="busy === 'draft'" @click="startDraft">{{
          t('New version')
        }}</UiButton>
        <UiButton :icon="GitFork" :disabled="!summary.latest" @click="forkOpen = true">{{
          t('Fork…')
        }}</UiButton>
        <UiButton :icon="ListChecks" :loading="busy === 'validate'" @click="validate">{{
          t('Validate')
        }}</UiButton>
      </UiCard>

      <UiCard v-if="report" padding="sm" class="flex flex-col gap-1.5 px-4">
        <p class="text-[13px] font-semibold" :class="report.valid ? 'text-success' : 'text-danger'">
          {{ report.valid ? t('Valid') : tn(report.errors.length, '{n} error', '{n} errors') }} ·
          {{ report.target }}
        </p>
        <p
          v-for="(issue, index) in [...report.errors, ...report.warnings]"
          :key="index"
          class="selectable text-[12.5px]"
          :class="issue.level === 'error' ? 'text-danger' : 'text-warning'"
        >
          <span v-if="issue.file" class="font-mono">{{ issue.file }}: </span>{{ issue.message }}
        </p>
      </UiCard>
    </UiSection>

    <UiSection
      v-if="details.manifest?.updates"
      :title="t('Dependency updates')"
      :description="t('Updates are applied to the draft; verify and publish them as a new version.')"
    >
      <template #actions>
        <UiButton :icon="RefreshCw" size="sm" :loading="busy === 'updates'" @click="checkUpdates">{{
          t('Check updates')
        }}</UiButton>
      </template>
      <UiCard v-if="outdated && outdated.length === 0" padding="sm" class="px-4 text-[13px] text-success">
        {{ t('All dependencies are up to date.') }}
      </UiCard>
      <UiCard v-else-if="outdated" padding="none" class="overflow-hidden">
        <div class="divide-y divide-hairline">
          <label v-for="p in outdated" :key="key(p)" class="flex items-center gap-3 px-5 py-2 hover:bg-fill">
            <input
              type="checkbox"
              class="size-4 accent-[var(--fd-accent)]"
              :checked="selected.has(key(p))"
              @change="toggle(p)"
            />
            <span class="w-48 truncate font-mono text-[12.5px] font-semibold">{{ p.name }}</span>
            <span class="w-20 truncate text-[12px] text-fg-3">{{ p.workspace || t('root') }}</span>
            <span class="flex-1 font-mono text-[12px] text-fg-2"
              >{{ p.current ?? '—' }} → {{ p.latest }}</span
            >
            <UiBadge v-if="p.held" tone="warning">{{ t('held at {range}', { range: p.held }) }}</UiBadge>
            <UiBadge :tone="levelTone[p.level]">{{ p.level }}</UiBadge>
          </label>
        </div>
        <div class="flex justify-end px-5 py-3">
          <UiButton
            variant="primary"
            size="sm"
            :disabled="selected.size === 0"
            :loading="busy === 'apply'"
            @click="applyUpdates"
          >
            {{ t('Apply {n} to the draft', { n: selected.size }) }}
          </UiButton>
        </div>
      </UiCard>
    </UiSection>

    <ForkSheet v-if="forkOpen" :details="details" @close="forkOpen = false" />
    <PublishSheet
      v-if="publishOpen"
      :details="details"
      @close="publishOpen = false"
      @published="(version) => ((publishOpen = false), emit('changed', version))"
    />
    <UiSheet v-if="discardOpen" :title="t('Discard the draft?')" width="sm" @close="discardOpen = false">
      <p class="pb-2 text-[13.5px] text-fg-2">
        {{ t('All changes in the draft will be deleted. Published versions stay untouched.') }}
      </p>
      <template #footer>
        <UiButton variant="secondary" @click="discardOpen = false">{{ t('Cancel') }}</UiButton>
        <UiButton variant="danger" :loading="busy === 'discard'" @click="discard">{{
          t('Discard')
        }}</UiButton>
      </template>
    </UiSheet>
  </div>
</template>
