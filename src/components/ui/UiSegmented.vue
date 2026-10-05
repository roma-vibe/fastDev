<script setup lang="ts" generic="T extends string">
import type { Component } from 'vue'

defineProps<{ options: { value: T; label: string; icon?: Component }[] }>()
const model = defineModel<T>({ required: true })
</script>

<template>
  <div class="inline-flex rounded-full bg-fill p-[3px]" role="radiogroup">
    <button
      v-for="option in options"
      :key="option.value"
      type="button"
      role="radio"
      :aria-checked="model === option.value"
      class="focus-ring inline-flex h-7 items-center gap-1.5 rounded-full px-3 text-[12.5px] transition-[background-color,color,box-shadow] duration-150"
      :class="
        model === option.value
          ? 'bg-surface-strong font-semibold text-fg shadow-[0_1px_3px_rgb(0_0_0/0.12),var(--fd-highlight)]'
          : 'text-fg-2 hover:text-fg'
      "
      @click="model = option.value"
    >
      <component :is="option.icon" v-if="option.icon" :size="14" :stroke-width="2" />
      {{ option.label }}
    </button>
  </div>
</template>
