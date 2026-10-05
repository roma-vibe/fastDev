<script setup lang="ts">
import type { Component } from 'vue'

withDefaults(
  defineProps<{
    icon: Component
    label: string
    active?: boolean
    variant?: 'filled' | 'ghost'
    size?: 'sm' | 'md'
    disabled?: boolean
  }>(),
  { variant: 'filled', size: 'md' },
)
</script>

<template>
  <button
    type="button"
    :title="label"
    :aria-label="label"
    :disabled="disabled"
    class="focus-ring inline-flex shrink-0 items-center justify-center rounded-full transition-[background-color,transform,color] duration-150 active:scale-[0.94] disabled:pointer-events-none disabled:opacity-40"
    :class="[
      size === 'sm' ? 'size-7' : 'size-[34px]',
      active
        ? 'bg-accent-soft text-accent-fg'
        : variant === 'filled'
          ? 'bg-fill text-fg-2 hover:bg-fill-2 hover:text-fg'
          : 'text-fg-2 hover:bg-fill hover:text-fg',
    ]"
  >
    <component :is="icon" :size="size === 'sm' ? 15 : 17" :stroke-width="1.9" />
  </button>
</template>
