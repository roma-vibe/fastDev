<script setup lang="ts">
import type { Component } from 'vue'

defineProps<{ icon: Component; title: string; text?: string }>()
</script>

<template>
  <div class="flex flex-col items-center justify-center gap-3 px-8 py-16 text-center">
    <span class="flex size-14 items-center justify-center rounded-[20px] bg-fill text-fg-3">
      <component :is="icon" :size="26" :stroke-width="1.6" />
    </span>
    <div class="max-w-sm">
      <p class="text-[15px] font-semibold">{{ title }}</p>
      <p v-if="text" class="mt-1 text-[13px] leading-relaxed text-fg-3">{{ text }}</p>
    </div>
    <slot />
  </div>
</template>
