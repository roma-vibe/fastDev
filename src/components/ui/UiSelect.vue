<script setup lang="ts">
import { ChevronDown } from '@lucide/vue'

defineProps<{ options: { value: string; label: string }[]; disabled?: boolean }>()
const model = defineModel<string>({ required: true })
</script>

<template>
  <div class="relative inline-flex">
    <select
      v-model="model"
      :disabled="disabled"
      class="focus-ring h-[34px] cursor-default appearance-none rounded-full bg-fill py-0 pr-8 pl-3.5 text-[13px] font-medium text-fg outline-none transition-colors hover:bg-fill-2"
    >
      <option v-for="option in options" :key="option.value" :value="option.value">{{ option.label }}</option>
    </select>
    <ChevronDown :size="14" class="pointer-events-none absolute top-1/2 right-3 -translate-y-1/2 text-fg-3" />
  </div>
</template>
