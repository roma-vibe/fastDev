<script setup lang="ts">
import type { ChangelogEntry } from '@/api'
import UiBadge from '@/components/ui/UiBadge.vue'
import { formatDate } from '@/i18n'
import { renderBlocks } from '@/lib/markdown'

defineProps<{ entries: ChangelogEntry[] }>()
</script>

<template>
  <ol class="relative flex flex-col gap-5 border-l border-hairline pl-6">
    <li v-for="entry in entries" :key="entry.version" class="relative">
      <span class="absolute top-1.5 -left-[29.5px] size-2.5 rounded-full border-2 border-accent bg-content" />
      <div class="flex items-center gap-2">
        <UiBadge tone="accent" mono>v{{ entry.version }}</UiBadge>
        <span class="text-[12px] text-fg-3">{{ formatDate(entry.date) }}</span>
      </div>
      <ul class="mt-2 flex flex-col gap-1 text-[13px] leading-relaxed">
        <!-- eslint-disable vue/no-v-html -- renderBlocks escapes HTML -->
        <li
          v-for="(block, index) in renderBlocks(entry.body)"
          :key="index"
          class="selectable"
          :class="{
            'relative pl-4 text-fg-2 before:absolute before:top-[0.6em] before:left-0.5 before:size-1 before:rounded-full before:bg-fg-3':
              block.kind === 'bullet',
            'text-[12.5px] text-fg-3 italic': block.kind === 'note',
            'text-fg-2': block.kind === 'text',
          }"
          v-html="block.html"
        />
        <!-- eslint-enable vue/no-v-html -->
      </ul>
    </li>
  </ol>
</template>
