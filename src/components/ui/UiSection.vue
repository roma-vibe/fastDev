<script setup lang="ts">
defineProps<{ title: string; description?: string }>()
</script>

<template>
  <section class="flex flex-col gap-3">
    <header class="flex items-end justify-between gap-4 px-1">
      <div>
        <h3 class="text-[14.5px] font-semibold tracking-tight">{{ title }}</h3>
        <p v-if="description" class="mt-0.5 text-[12.5px] text-fg-3">{{ description }}</p>
      </div>
      <div class="flex items-center gap-2"><slot name="actions" /></div>
    </header>
    <slot />
  </section>
</template>
