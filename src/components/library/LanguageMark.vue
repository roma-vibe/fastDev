<script setup lang="ts">
import { computed } from 'vue'
import { language } from '@/lib/languages'

const props = withDefaults(defineProps<{ languages: string[]; size?: number }>(), { size: 40 })
const lang = computed(() => language(props.languages[0] ?? 'other'))
</script>

<template>
  <span
    class="relative inline-flex shrink-0 items-center justify-center overflow-hidden font-bold text-white shadow-[inset_0_1px_0_rgb(255_255_255/0.35),0_4px_12px_rgb(0_0_0/0.12)]"
    :style="{
      width: `${size}px`,
      height: `${size}px`,
      borderRadius: `${size * 0.3}px`,
      fontSize: `${lang.short.length > 2 ? size * 0.26 : size * 0.32}px`,
      background: `linear-gradient(145deg, ${lang.from}, ${lang.to})`,
    }"
    :title="lang.name"
  >
    <span class="absolute inset-x-0 top-0 h-1/2 bg-white/15" />
    <span class="relative tracking-tight">{{ lang.short }}</span>
  </span>
</template>
