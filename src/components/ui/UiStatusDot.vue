<script setup lang="ts">
defineProps<{ status: 'running' | 'idle' | 'failed' | 'warning' }>()
</script>

<template>
  <span class="relative inline-flex size-2.5 shrink-0">
    <span
      v-if="status === 'running'"
      class="absolute inset-0 animate-ping rounded-full bg-success opacity-50"
    />
    <span
      class="relative inline-flex size-2.5 rounded-full"
      :class="
        {
          running: 'bg-success',
          idle: 'bg-fg-3/45',
          failed: 'bg-danger',
          warning: 'bg-warning',
        }[status]
      "
    />
  </span>
</template>
