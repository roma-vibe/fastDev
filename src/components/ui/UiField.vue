<script setup lang="ts">
defineProps<{ label: string; hint?: string; error?: string | null }>()
</script>

<template>
  <div class="flex flex-col gap-1.5">
    <span class="px-1 text-[12px] font-semibold tracking-wide text-fg-2">{{ label }}</span>
    <slot />
    <span v-if="error" class="px-1 text-[12px] text-danger">{{ error }}</span>
    <span v-else-if="hint" class="px-1 text-[12px] leading-snug text-fg-3">{{ hint }}</span>
  </div>
</template>
