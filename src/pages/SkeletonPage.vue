<script setup lang="ts">
import { FolderOpen, GitFork, Star } from '@lucide/vue'
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { api } from '@/api'
import PageHeader from '@/components/layout/PageHeader.vue'
import ChangelogList from '@/components/skeleton/ChangelogList.vue'
import CreateProjectForm from '@/components/skeleton/CreateProjectForm.vue'
import SkeletonMaintenance from '@/components/skeleton/SkeletonMaintenance.vue'
import SkeletonOverview from '@/components/skeleton/SkeletonOverview.vue'
import UiBadge from '@/components/ui/UiBadge.vue'
import UiEmpty from '@/components/ui/UiEmpty.vue'
import UiIconButton from '@/components/ui/UiIconButton.vue'
import UiSegmented from '@/components/ui/UiSegmented.vue'
import UiSelect from '@/components/ui/UiSelect.vue'
import UiSpinner from '@/components/ui/UiSpinner.vue'
import { st, t } from '@/i18n'
import { useLibraryStore } from '@/stores/library'
import { useToastStore } from '@/stores/toasts'

const props = defineProps<{ id: string }>()
const route = useRoute()
const router = useRouter()
const library = useLibraryStore()
const toasts = useToastStore()

type Tab = 'create' | 'overview' | 'versions' | 'maintenance'
const tab = ref<Tab>((route.query.tab as Tab | undefined) ?? 'create')
const version = ref<string | undefined>(route.query.version as string | undefined)
const error = ref<string | null>(null)

const summary = computed(() => library.skeletons.find((s) => s.id === props.id))
/** Default: latest published version, or the draft of an unpublished skeleton. */
const target = computed(() => version.value ?? summary.value?.latest ?? 'draft')
const details = computed(() => library.cached(props.id, target.value))
const info = computed(() => details.value?.summary ?? summary.value)
const title = computed(() => st(info.value?.name, info.value?.translations) || props.id)
const description = computed(() => st(info.value?.description, info.value?.translations))

async function load(): Promise<void> {
  try {
    if (!library.loaded) await library.load()
    await library.loadDetails(props.id, target.value)
    error.value = null
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e)
  }
}

watch(
  target,
  (value, old) => {
    if (old) library.forget(props.id, old)
    void load()
  },
  { immediate: true },
)
watch([tab, version], () => void router.replace({ query: { tab: tab.value, version: version.value } }))
onBeforeUnmount(() => library.forget(props.id, target.value))

const versionOptions = computed(() => {
  const s = details.value?.summary ?? summary.value
  const options = (s?.versions ?? []).map((v) => ({ value: v, label: `v${v}` }))
  if (s?.draft) options.unshift({ value: 'draft', label: t('Draft') })
  return options
})

async function onChanged(next?: string): Promise<void> {
  await library.load()
  if (next && next !== target.value) version.value = next
  else await load()
}

async function favorite(): Promise<void> {
  if (!details.value) return
  try {
    await library.toggleFavorite(props.id, !details.value.favorite)
    await load()
  } catch (e) {
    toasts.error(e)
  }
}

async function openFolder(): Promise<void> {
  if (!details.value) return
  try {
    const workspace = details.value.summary.workspacePath
    await api.openPath(workspace ?? details.value.paths.target)
  } catch (e) {
    toasts.error(e)
  }
}
</script>

<template>
  <div class="flex h-full flex-col">
    <PageHeader :title="title" :subtitle="info?.stack.join(' · ')" back="/library">
      <template #badges>
        <UiBadge v-if="details?.summary.forkedFrom" :icon="GitFork">
          {{
            t('fork of {id} {version}', {
              id: details.summary.forkedFrom.id,
              version: details.summary.forkedFrom.version,
            })
          }}
        </UiBadge>
      </template>
      <UiSelect
        v-if="versionOptions.length"
        :model-value="target"
        :options="versionOptions"
        @update:model-value="version = $event"
      />
      <UiIconButton :icon="Star" :label="t('Favorite')" :active="details?.favorite" @click="favorite" />
      <UiIconButton :icon="FolderOpen" :label="t('Open skeleton folder')" @click="openFolder" />
    </PageHeader>

    <div class="@container flex-1 overflow-y-auto px-7 pb-10">
      <UiEmpty v-if="error" :icon="GitFork" :title="t('Cannot open this skeleton')" :text="error" />
      <div v-else-if="!details" class="flex justify-center py-20 text-fg-3"><UiSpinner :size="22" /></div>
      <template v-else>
        <p
          v-if="description"
          class="selectable mb-6 max-w-4xl text-[14px] leading-relaxed whitespace-pre-line text-fg-2"
        >
          {{ description }}
        </p>
        <div class="mb-6 flex flex-wrap items-center gap-4">
          <UiSegmented
            v-model="tab"
            :options="[
              { value: 'create', label: t('Create project') },
              { value: 'overview', label: t('Overview') },
              { value: 'versions', label: t('Versions') },
              { value: 'maintenance', label: t('Maintenance') },
            ]"
          />
          <UiBadge v-if="details.parent?.newer" tone="warning" class="ml-auto">
            {{
              t('{id} has a newer version {version}', {
                id: details.parent.id,
                version: details.parent.latest ?? '',
              })
            }}
          </UiBadge>
        </div>

        <CreateProjectForm v-if="tab === 'create'" :details="details" />
        <SkeletonOverview v-else-if="tab === 'overview'" :details="details" />
        <div v-else-if="tab === 'versions'" class="flex flex-col gap-8">
          <div v-if="details.parent" class="rounded-card bg-fill px-5 py-4 text-[13px] text-fg-2">
            {{ t('Forked from {id} {version}.', { id: details.parent.id, version: details.parent.version }) }}
            <template v-if="details.parent.newer">
              {{ t('Newer versions of the parent (apply what you need in a draft):') }}
              <ChangelogList :entries="details.parent.changelog" class="mt-4" />
            </template>
          </div>
          <ChangelogList v-if="details.changelog.length" :entries="details.changelog" />
          <p v-else class="text-[13px] text-fg-3">{{ t('No published versions yet.') }}</p>
        </div>
        <SkeletonMaintenance v-else :details="details" @changed="onChanged" />
      </template>
    </div>
  </div>
</template>
