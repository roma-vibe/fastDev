<script setup lang="ts">
import type { Component } from 'vue'
import UiSwitch from './UiSwitch.vue'

defineProps<{ label: string; description?: string; icon?: Component; disabled?: boolean }>()
const model = defineModel<boolean>({ required: true })
</script>

<template>
  <label
    class="flex items-center gap-3 rounded-2xl px-3 py-2.5 transition-colors hover:bg-fill"
    :class="disabled ? 'opacity-50' : ''"
  >
    <span v-if="icon" class="flex size-8 shrink-0 items-center justify-center rounded-xl bg-fill text-fg-2">
      <component :is="icon" :size="16" :stroke-width="1.9" />
    </span>
    <span class="min-w-0 flex-1">
      <span class="block text-[13.5px] font-medium">{{ label }}</span>
      <span v-if="description" class="block text-[12px] leading-snug text-fg-3">{{ description }}</span>
    </span>
    <UiSwitch v-model="model" :disabled="disabled" :label="label" />
  </label>
</template>
