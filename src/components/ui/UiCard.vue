<script setup lang="ts">
withDefaults(defineProps<{ padding?: 'none' | 'sm' | 'md'; interactive?: boolean }>(), { padding: 'md' })
</script>

<template>
  <div
    class="glass rounded-card"
    :class="[
      { none: '', sm: 'p-3.5', md: 'p-5' }[padding],
      interactive
        ? 'transition-[transform,box-shadow] duration-200 hover:-translate-y-0.5 hover:shadow-float'
        : '',
    ]"
  >
    <slot />
  </div>
</template>
