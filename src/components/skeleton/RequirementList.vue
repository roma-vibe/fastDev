<script setup lang="ts">
import { CircleAlert, CircleCheck, CircleX } from '@lucide/vue'
import type { RequirementStatus } from '@/api'
import { t } from '@/i18n'

defineProps<{ items: RequirementStatus[] }>()
</script>

<template>
  <ul class="flex flex-col gap-1">
    <li
      v-for="item in items"
      :key="item.tool"
      class="flex items-center gap-2 text-[13px]"
      :title="item.satisfied ? undefined : `${item.problem ?? t('not installed')}. ${item.hint}`"
    >
      <CircleCheck v-if="item.satisfied" :size="15" class="shrink-0 text-success" />
      <CircleAlert
        v-else-if="item.neededForSetup === false"
        :size="15"
        class="shrink-0 text-warning"
        :title="t('Not needed to create the project; needed later')"
      />
      <CircleX v-else :size="15" class="shrink-0 text-danger" />
      <span class="font-medium">{{ item.label || item.tool }}</span>
      <span class="font-mono text-[11.5px] text-fg-3">{{ item.requirement }}</span>
      <span
        class="ml-auto font-mono text-[11.5px]"
        :class="item.satisfied ? 'text-fg-2' : item.neededForSetup === false ? 'text-warning' : 'text-danger'"
      >
        {{ item.problem ? t('not running') : (item.version ?? t('not installed')) }}
      </span>
    </li>
  </ul>
</template>
