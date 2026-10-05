<script setup lang="ts">
defineProps<{ options: { value: string; label: string; description?: string }[]; name: string }>()
const model = defineModel<string>({ required: true })
</script>

<template>
  <div class="flex flex-col gap-1" role="radiogroup" :aria-label="name">
    <button
      v-for="option in options"
      :key="option.value"
      type="button"
      role="radio"
      :aria-checked="model === option.value"
      class="focus-ring flex items-start gap-3 rounded-2xl px-3 py-2.5 text-left transition-colors"
      :class="model === option.value ? 'bg-accent/10' : 'hover:bg-fill'"
      @click="model = option.value"
    >
      <span
        class="mt-0.5 flex size-[18px] shrink-0 items-center justify-center rounded-full border-2 transition-colors"
        :class="model === option.value ? 'border-accent bg-accent' : 'border-fg-3/50'"
      >
        <span v-if="model === option.value" class="size-1.5 rounded-full bg-white" />
      </span>
      <span class="min-w-0 flex-1">
        <span class="block text-[13.5px] font-medium">{{ option.label }}</span>
        <span v-if="option.description" class="block text-[12px] leading-snug text-fg-3">
          {{ option.description }}
        </span>
      </span>
    </button>
  </div>
</template>
