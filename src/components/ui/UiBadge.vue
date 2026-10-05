<script setup lang="ts">
import type { Component } from 'vue'

withDefaults(
  defineProps<{
    tone?: 'neutral' | 'accent' | 'success' | 'warning' | 'danger'
    icon?: Component
    mono?: boolean
  }>(),
  {
    tone: 'neutral',
    icon: undefined,
  },
)
const tones = {
  neutral: 'bg-fill text-fg-2',
  accent: 'bg-accent/12 text-accent',
  success: 'bg-success/12 text-success',
  warning: 'bg-warning/14 text-warning',
  danger: 'bg-danger/12 text-danger',
}
</script>

<template>
  <span
    class="inline-flex h-[22px] shrink-0 items-center gap-1 rounded-full px-2.5 text-[11.5px] font-medium whitespace-nowrap"
    :class="[tones[tone], mono ? 'font-mono text-[11px]' : '']"
  >
    <component :is="icon" v-if="icon" :size="12" :stroke-width="2.2" />
    <slot />
  </span>
</template>
