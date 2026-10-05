<script setup lang="ts">
withDefaults(defineProps<{ size?: number }>(), { size: 26 })
</script>

<template>
  <svg :width="size" :height="size" viewBox="0 0 32 32" aria-hidden="true">
    <defs>
      <linearGradient id="fd-logo" x1="0" y1="0" x2="1" y2="1">
        <stop offset="0" stop-color="#5c9eff" />
        <stop offset="1" stop-color="#6b5cfa" />
      </linearGradient>
    </defs>
    <rect x="1" y="1" width="30" height="30" rx="8.5" fill="url(#fd-logo)" />
    <rect x="1" y="1" width="30" height="15" rx="8.5" fill="#fff" opacity="0.14" />
    <path d="M17.6 5.5 9.6 17.4h5.6l-1.4 9.1 8.6-12.3h-5.7l.9-8.7Z" fill="#fff" />
  </svg>
</template>
