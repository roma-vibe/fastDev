<script setup lang="ts">
import { FolderOpen, SquareTerminal } from '@lucide/vue'
import { computed } from 'vue'
import { api, type SkeletonDetails } from '@/api'
import UiBadge from '@/components/ui/UiBadge.vue'
import UiButton from '@/components/ui/UiButton.vue'
import UiCard from '@/components/ui/UiCard.vue'
import UiSection from '@/components/ui/UiSection.vue'
import { st, t } from '@/i18n'
import { useToastStore } from '@/stores/toasts'

const props = defineProps<{ details: SkeletonDetails }>()
const toasts = useToastStore()
const manifest = computed(() => props.details.manifest)
const tr = computed(() => manifest.value?.translations)
const env = computed(() => [
  ...Object.entries(manifest.value?.env ?? {}).map(([key, value]) => ({ key, value, kind: 'env' })),
  ...Object.entries(manifest.value?.ports ?? {}).map(([key, value]) => ({
    key,
    value: `${value}+`,
    kind: 'port',
  })),
  ...Object.entries(manifest.value?.secrets ?? {}).map(([key, value]) => ({
    key,
    value: `${value} bytes`,
    kind: 'secret',
  })),
])

async function open(path: string): Promise<void> {
  try {
    await api.openPath(path)
  } catch (e) {
    toasts.error(e)
  }
}
</script>

<template>
  <div v-if="manifest" class="flex flex-col gap-7">
    <UiSection :title="t('Stack')">
      <div class="flex flex-wrap gap-2">
        <span
          v-for="item in manifest.stack"
          :key="item"
          class="rounded-lg bg-fill px-2.5 py-1 text-[12.5px]"
          >{{ item }}</span
        >
        <span v-for="tag in manifest.tags" :key="tag" class="rounded-lg px-2 py-1 text-[12.5px] text-fg-3"
          >#{{ tag }}</span
        >
      </div>
    </UiSection>

    <UiSection
      :title="t('Commands')"
      :description="t('Buttons on the project page. ${VAR} is read from the project .env.')"
    >
      <UiCard padding="none" class="divide-y divide-hairline">
        <div
          v-for="(command, key) in manifest.commands"
          :key="key"
          class="flex items-center gap-3 px-5 py-2.5"
        >
          <SquareTerminal :size="15" class="shrink-0 text-fg-3" />
          <span class="w-32 shrink-0 truncate text-[13px] font-medium">{{
            st(command.label, tr) || key
          }}</span>
          <code class="selectable min-w-0 flex-1 truncate font-mono text-[12px] text-fg-2">{{
            command.run
          }}</code>
          <UiBadge v-if="command.primary" tone="accent">{{ t('primary') }}</UiBadge>
          <UiBadge v-if="command.long">{{ t('long-running') }}</UiBadge>
          <UiBadge v-if="command.inputs?.length">{{ t('asks for input') }}</UiBadge>
          <UiBadge v-if="command.feature" tone="warning">{{ command.feature }}</UiBadge>
        </div>
      </UiCard>
    </UiSection>

    <UiSection
      v-for="(choice, key) in manifest.choices ?? {}"
      :key="key"
      :title="st(choice.label, tr) || key"
      :description="st(choice.description, tr) || undefined"
    >
      <div class="grid grid-cols-1 gap-3 @3xl:grid-cols-3">
        <UiCard v-for="(option, value) in choice.options" :key="value" padding="sm" class="px-4">
          <div class="flex items-center gap-2">
            <p class="text-[13.5px] font-semibold">{{ st(option.label, tr) || value }}</p>
            <UiBadge v-if="value === choice.default" tone="accent">{{ t('default') }}</UiBadge>
          </div>
          <p class="mt-0.5 text-[12.5px] text-fg-3">{{ st(option.description, tr) }}</p>
          <p v-if="option.files.length" class="mt-2 font-mono text-[11.5px] text-fg-2">
            {{ option.files.join(', ') }}
          </p>
        </UiCard>
      </div>
    </UiSection>

    <UiSection v-if="Object.keys(manifest.features).length" :title="t('Optional features')">
      <div class="grid grid-cols-1 gap-3 @3xl:grid-cols-2">
        <UiCard v-for="(feature, key) in manifest.features" :key="key" padding="sm" class="px-4">
          <p class="text-[13.5px] font-semibold">{{ st(feature.label, tr) || key }}</p>
          <p class="mt-0.5 text-[12.5px] text-fg-3">{{ st(feature.description, tr) }}</p>
          <p class="mt-2 font-mono text-[11.5px] text-fg-2">{{ feature.files.join(', ') }}</p>
        </UiCard>
      </div>
    </UiSection>

    <UiSection
      :title="t('Environment')"
      :description="t('Written to .env and .env.example of every new project.')"
    >
      <UiCard padding="none" class="divide-y divide-hairline">
        <div v-for="item in env" :key="item.key" class="flex items-center gap-3 px-5 py-2">
          <code class="w-48 shrink-0 font-mono text-[12px] font-semibold">{{ item.key }}</code>
          <code class="selectable min-w-0 flex-1 truncate font-mono text-[12px] text-fg-2">{{
            item.value
          }}</code>
          <UiBadge v-if="item.kind !== 'env'">{{
            item.kind === 'port' ? t('free port') : t('secret')
          }}</UiBadge>
        </div>
      </UiCard>
    </UiSection>

    <UiSection :title="t('Repository')">
      <UiCard padding="none" class="divide-y divide-hairline text-[13px]">
        <div class="flex items-center gap-4 px-5 py-2.5">
          <span class="w-36 shrink-0 text-fg-3">{{ t('Repository') }}</span>
          <code class="selectable min-w-0 flex-1 truncate font-mono text-[12px]">{{
            details.paths.repo ?? '—'
          }}</code>
        </div>
        <div v-if="details.summary.workspacePath" class="flex items-center gap-4 px-5 py-2.5">
          <span class="w-36 shrink-0 text-fg-3">{{ t('Workspace clone') }}</span>
          <code class="selectable min-w-0 flex-1 truncate font-mono text-[12px]">{{
            details.summary.workspacePath
          }}</code>
          <UiButton size="sm" :icon="FolderOpen" @click="open(details.summary.workspacePath)">{{
            t('Open')
          }}</UiButton>
        </div>
        <div class="flex items-center gap-4 px-5 py-2.5">
          <span class="w-36 shrink-0 text-fg-3">{{
            t('Files of {version}', { version: details.target })
          }}</span>
          <code class="selectable min-w-0 flex-1 truncate font-mono text-[12px]">{{
            details.paths.files
          }}</code>
          <UiButton size="sm" :icon="FolderOpen" @click="open(details.paths.files)">{{ t('Open') }}</UiButton>
        </div>
      </UiCard>
    </UiSection>
  </div>
</template>
