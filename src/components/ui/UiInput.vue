<script setup lang="ts">
import type { Component } from 'vue'
import { ref } from 'vue'

defineProps<{
  placeholder?: string
  icon?: Component
  mono?: boolean
  disabled?: boolean
  autofocus?: boolean
}>()
const model = defineModel<string>({ required: true })
const input = ref<HTMLInputElement>()
defineExpose({ focus: () => input.value?.focus() })
</script>

<template>
  <div
    class="group flex h-9 items-center gap-2 rounded-xl border border-transparent bg-fill px-3 transition-[background-color,border-color,box-shadow] focus-within:border-accent/45 focus-within:bg-surface-strong focus-within:shadow-[0_0_0_3px_color-mix(in_srgb,var(--fd-accent)_18%,transparent)]"
    :class="disabled ? 'opacity-50' : ''"
  >
    <component :is="icon" v-if="icon" :size="15" :stroke-width="2" class="shrink-0 text-fg-3" />
    <input
      ref="input"
      v-model="model"
      :placeholder="placeholder"
      :disabled="disabled"
      :autofocus="autofocus"
      spellcheck="false"
      autocomplete="off"
      class="min-w-0 flex-1 bg-transparent text-[13.5px] outline-none placeholder:text-fg-3"
      :class="mono ? 'font-mono text-[12.5px]' : ''"
    />
    <slot name="suffix" />
  </div>
</template>
