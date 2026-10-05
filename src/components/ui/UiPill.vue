<script setup lang="ts">
import type { Component } from 'vue'

defineProps<{ active?: boolean; icon?: Component; count?: number }>()
</script>

<template>
  <button
    type="button"
    class="focus-ring inline-flex h-8 shrink-0 items-center gap-1.5 rounded-full px-3.5 text-[13px] transition-[background-color,color,transform] duration-150 active:scale-[0.97]"
    :class="
      active
        ? 'bg-accent-soft font-semibold text-accent-fg'
        : 'bg-fill text-fg-2 hover:bg-fill-2 hover:text-fg'
    "
  >
    <component :is="icon" v-if="icon" :size="14" :stroke-width="2" />
    <slot />
    <span v-if="count !== undefined" class="text-[11.5px] tabular-nums opacity-60">{{ count }}</span>
  </button>
</template>
