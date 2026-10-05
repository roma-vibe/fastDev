<script setup lang="ts">
withDefaults(defineProps<{ placeholder?: string; rows?: number; mono?: boolean }>(), {
  rows: 4,
  placeholder: '',
})
const model = defineModel<string>({ required: true })
</script>

<template>
  <textarea
    v-model="model"
    :rows="rows"
    :placeholder="placeholder"
    spellcheck="true"
    class="block w-full resize-y rounded-xl border border-transparent bg-fill px-3 py-2.5 text-[13.5px] leading-relaxed outline-none transition-[background-color,border-color,box-shadow] placeholder:text-fg-3 focus:border-accent/45 focus:bg-surface-strong focus:shadow-[0_0_0_3px_color-mix(in_srgb,var(--fd-accent)_18%,transparent)]"
    :class="mono ? 'font-mono text-[12.5px]' : ''"
  />
</template>
