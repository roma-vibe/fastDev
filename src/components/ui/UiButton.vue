<script setup lang="ts">
import type { Component } from 'vue'
import UiSpinner from './UiSpinner.vue'

const props = withDefaults(
  defineProps<{
    variant?: 'primary' | 'secondary' | 'soft' | 'ghost' | 'danger'
    size?: 'sm' | 'md' | 'lg'
    icon?: Component
    loading?: boolean
    disabled?: boolean
    type?: 'button' | 'submit'
  }>(),
  { variant: 'secondary', size: 'md', icon: undefined, type: 'button' },
)

const sizes = {
  sm: 'h-7 px-3 text-[12.5px] gap-1.5',
  md: 'h-[34px] px-4 text-[13px] gap-1.5',
  lg: 'h-10 px-5 text-[14px] gap-2',
}
const variants = {
  primary:
    'bg-accent text-white shadow-[inset_0_1px_0_rgb(255_255_255/0.25),0_4px_14px_color-mix(in_srgb,var(--fd-accent)_35%,transparent)] hover:bg-accent-hover',
  secondary: 'bg-fill text-fg hover:bg-fill-2',
  soft: 'bg-accent-soft text-accent-fg hover:brightness-[1.03]',
  ghost: 'text-fg-2 hover:bg-fill hover:text-fg',
  danger: 'bg-danger/10 text-danger hover:bg-danger/16',
}
const iconSize = { sm: 14, md: 15, lg: 17 }
</script>

<template>
  <button
    :type="props.type"
    :disabled="disabled || loading"
    class="focus-ring inline-flex shrink-0 items-center justify-center rounded-full font-medium whitespace-nowrap transition-[background-color,transform,filter,color] duration-150 active:scale-[0.97] disabled:pointer-events-none disabled:opacity-45"
    :class="[sizes[size], variants[variant]]"
  >
    <UiSpinner v-if="loading" :size="iconSize[size]" />
    <component :is="icon" v-else-if="icon" :size="iconSize[size]" :stroke-width="2" />
    <slot />
  </button>
</template>
