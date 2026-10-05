<script setup lang="ts">
import { ExternalLink, Play, RotateCw, Square } from '@lucide/vue'
import { computed } from 'vue'
import type { CommandView, RunInfo, Translations } from '@/api'
import UiButton from '@/components/ui/UiButton.vue'
import UiIconButton from '@/components/ui/UiIconButton.vue'
import UiStatusDot from '@/components/ui/UiStatusDot.vue'
import { st, t } from '@/i18n'

const props = defineProps<{
  command: CommandView
  run?: RunInfo
  disabled?: boolean
  translations?: Translations
}>()
const emit = defineEmits<{ run: []; stop: []; open: [url: string] }>()

const running = computed(() => props.run?.status === 'running')
const status = computed(() => {
  if (running.value) return 'running'
  if (props.run?.status === 'exited' && props.run.exitCode !== 0) return 'failed'
  return 'idle'
})
const lastResult = computed(() => {
  const run = props.run
  if (!run || run.status === 'running') return ''
  if (run.status === 'stopped') return t('stopped')
  if (run.status === 'failed') return t('failed to start')
  return run.exitCode === 0 ? t('succeeded') : t('exit code {code}', { code: run.exitCode ?? '?' })
})
</script>

<template>
  <div
    class="flex flex-col gap-2.5 rounded-card border p-4 transition-colors"
    :class="
      command.primary
        ? 'border-accent/25 bg-accent/[0.06]'
        : 'border-hairline bg-surface shadow-[var(--fd-highlight)]'
    "
  >
    <div class="flex items-center gap-2">
      <UiStatusDot :status="status" />
      <span class="truncate text-[14px] font-semibold">{{ st(command.label, translations) }}</span>
      <span class="ml-auto text-[11.5px] text-fg-3">{{ lastResult }}</span>
    </div>
    <code class="selectable truncate font-mono text-[11.5px] text-fg-2" :title="command.run">{{
      command.run
    }}</code>
    <div class="mt-auto flex items-center gap-1.5 pt-1">
      <UiButton v-if="running" size="sm" variant="danger" :icon="Square" @click="emit('stop')">
        {{ t('Stop') }}
      </UiButton>
      <UiButton
        v-else
        size="sm"
        :variant="command.primary ? 'primary' : 'secondary'"
        :icon="run ? RotateCw : Play"
        :disabled="disabled"
        @click="emit('run')"
      >
        {{ run ? t('Run again') : t('Run') }}{{ command.inputs?.length ? '…' : '' }}
      </UiButton>
      <UiIconButton
        v-if="command.url"
        :icon="ExternalLink"
        :label="command.url"
        size="sm"
        :disabled="!running"
        @click="emit('open', command.url)"
      />
    </div>
  </div>
</template>
