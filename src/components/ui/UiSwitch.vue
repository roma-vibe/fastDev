<script setup lang="ts">
defineProps<{ disabled?: boolean; label?: string }>()
const model = defineModel<boolean>({ required: true })
</script>

<template>
  <button
    type="button"
    role="switch"
    :aria-checked="model"
    :aria-label="label"
    :disabled="disabled"
    class="focus-ring relative h-[22px] w-[38px] shrink-0 rounded-full transition-colors duration-200 disabled:opacity-40"
    :class="model ? 'bg-accent' : 'bg-fill-2'"
    @click="model = !model"
  >
    <span
      class="absolute top-[2px] left-[2px] size-[18px] rounded-full bg-white shadow-[0_1px_3px_rgb(0_0_0/0.25)] transition-transform duration-200"
      :class="model ? 'translate-x-4' : ''"
    />
  </button>
</template>
