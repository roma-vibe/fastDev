<script setup lang="ts">
import { CircleX, TriangleAlert } from '@lucide/vue'
import { computed } from 'vue'
import type { RequirementStatus } from '@/api'
import { t } from '@/i18n'

const props = defineProps<{ items: RequirementStatus[]; install: boolean }>()

/** Blocking: setup needs the tool. Later: only needed for some commands (e.g. Docker up). */
const blocking = computed(() =>
  props.items.filter((r) => !r.satisfied && props.install && r.neededForSetup !== false),
)
const later = computed(() => props.items.filter((r) => !r.satisfied && !blocking.value.includes(r)))

function state(item: RequirementStatus): string {
  if (item.problem) return item.problem
  return item.version ? t('found {version}', { version: item.version }) : t('not installed')
}
</script>

<template>
  <div v-if="blocking.length" class="flex flex-col gap-2 rounded-card bg-danger/10 px-4 py-3 text-[12.5px]">
    <p class="flex items-center gap-2 font-semibold text-danger">
      <CircleX :size="15" />
      {{ t('Missing tools needed to create the project') }}
    </p>
    <div v-for="item in blocking" :key="item.tool" class="selectable pl-6 leading-snug">
      <span class="font-semibold">{{ item.label }} {{ item.requirement }}</span>
      <span class="text-fg-2"> — {{ state(item) }}. {{ item.hint }}</span>
    </div>
    <p class="pl-6 text-fg-3">
      {{ t('Or turn off “Install dependencies” to create the project without setup.') }}
    </p>
  </div>
  <div v-if="later.length" class="flex flex-col gap-2 rounded-card bg-warning/10 px-4 py-3 text-[12.5px]">
    <p class="flex items-center gap-2 font-semibold text-warning">
      <TriangleAlert :size="15" />
      {{ t('Needed later') }}
    </p>
    <div v-for="item in later" :key="item.tool" class="selectable pl-6 leading-snug">
      <span class="font-semibold">{{ item.label }} {{ item.requirement }}</span>
      <span class="text-fg-2"> — {{ state(item) }}. {{ item.hint }}</span>
    </div>
  </div>
</template>
