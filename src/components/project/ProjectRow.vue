<script setup lang="ts">
import { ChevronRight, Code, Play, Square } from '@lucide/vue'
import { computed } from 'vue'
import type { Project } from '@/api'
import UiBadge from '@/components/ui/UiBadge.vue'
import UiIconButton from '@/components/ui/UiIconButton.vue'
import UiStatusDot from '@/components/ui/UiStatusDot.vue'
import { st, t } from '@/i18n'

const props = defineProps<{ project: Project }>()
const emit = defineEmits<{ run: [command: string]; stop: [command: string]; editor: [] }>()

const primary = computed(
  () => props.project.commands.find((c) => c.primary) ?? props.project.commands.find((c) => c.long),
)
const primaryRunning = computed(() => !!primary.value && props.project.running.includes(primary.value.key))
const status = computed(() =>
  props.project.running.length
    ? 'running'
    : !props.project.exists || props.project.setupStatus === 'failed'
      ? 'warning'
      : 'idle',
)
</script>

<template>
  <RouterLink
    :to="`/projects/${project.id}`"
    class="focus-ring group flex items-center gap-4 rounded-2xl px-4 py-3 transition-colors hover:bg-fill"
  >
    <UiStatusDot :status="status" />
    <div class="min-w-0 flex-1">
      <div class="flex items-center gap-2">
        <span class="truncate text-[14px] font-semibold">{{ project.name }}</span>
        <UiBadge v-if="!project.exists" tone="warning">{{ t('Folder missing') }}</UiBadge>
        <UiBadge v-else-if="project.setupStatus === 'failed'" tone="danger">{{ t('Setup failed') }}</UiBadge>
        <UiBadge v-for="command in project.running" :key="command" tone="success">{{ command }}</UiBadge>
      </div>
      <p class="truncate font-mono text-[11.5px] text-fg-3">{{ project.path }}</p>
    </div>
    <UiBadge v-if="project.skeletonId" mono>{{ project.skeletonId }} {{ project.skeletonVersion }}</UiBadge>
    <UiBadge v-if="project.updateAvailable" tone="accent">{{ t('update') }}</UiBadge>
    <div class="flex items-center gap-1.5" @click.prevent.stop>
      <UiIconButton
        v-if="primary && project.exists"
        :icon="primaryRunning ? Square : Play"
        :label="
          primaryRunning
            ? t('Stop {command}', { command: st(primary.label, project.translations) })
            : t('Run {command}', { command: st(primary.label, project.translations) })
        "
        :active="primaryRunning"
        size="sm"
        @click="primaryRunning ? emit('stop', primary.key) : emit('run', primary.key)"
      />
      <UiIconButton
        v-if="project.exists"
        :icon="Code"
        :label="t('Open in editor')"
        size="sm"
        @click="emit('editor')"
      />
    </div>
    <ChevronRight :size="16" class="text-fg-3 transition-transform group-hover:translate-x-0.5" />
  </RouterLink>
</template>
