<script setup lang="ts">
import { CloudDownload, GitFork, Link, PencilLine, Star } from '@lucide/vue'
import { computed } from 'vue'
import type { SkeletonSummary } from '@/api'
import UiBadge from '@/components/ui/UiBadge.vue'
import UiCard from '@/components/ui/UiCard.vue'
import { st, t, tn } from '@/i18n'
import LanguageMark from './LanguageMark.vue'

const props = defineProps<{ skeleton: SkeletonSummary; favorite: boolean }>()
const emit = defineEmits<{ favorite: [value: boolean] }>()

const stack = computed(() => props.skeleton.stack.slice(0, 4))
const more = computed(() => Math.max(0, props.skeleton.stack.length - 4))
</script>

<template>
  <RouterLink :to="`/library/${skeleton.id}`" class="focus-ring block rounded-card">
    <UiCard interactive class="flex h-full flex-col gap-3.5">
      <div class="flex items-start gap-3">
        <LanguageMark :languages="skeleton.languages" />
        <div class="min-w-0 flex-1 pt-0.5">
          <p class="truncate text-[15px] font-semibold tracking-tight">{{ st(skeleton.name, skeleton.translations) }}</p>
          <p class="truncate font-mono text-[11.5px] text-fg-3">{{ skeleton.id }}</p>
        </div>
        <button
          type="button"
          class="focus-ring -mt-1 -mr-1 flex size-8 items-center justify-center rounded-full transition-colors hover:bg-fill"
          :class="favorite ? 'text-warning' : 'text-fg-3'"
          :aria-label="t('Favorite')"
          @click.prevent.stop="emit('favorite', !favorite)"
        >
          <Star :size="16" :stroke-width="2" :fill="favorite ? 'currentColor' : 'none'" />
        </button>
      </div>

      <p class="line-clamp-2 min-h-[2.8em] text-[13px] leading-snug text-fg-2">
        {{ skeleton.error ?? st(skeleton.description, skeleton.translations) }}
      </p>

      <div class="flex flex-wrap gap-1.5">
        <span v-for="item in stack" :key="item" class="rounded-md bg-fill px-1.5 py-0.5 text-[11px] text-fg-2">
          {{ item }}
        </span>
        <span v-if="more" class="rounded-md px-1 py-0.5 text-[11px] text-fg-3">+{{ more }}</span>
      </div>

      <div class="mt-auto flex flex-wrap items-center gap-1.5 pt-1">
        <UiBadge v-if="skeleton.latest" tone="accent" mono>v{{ skeleton.latest }}</UiBadge>
        <UiBadge v-if="skeleton.draft" tone="warning" :icon="PencilLine">{{ t('Draft') }}</UiBadge>
        <UiBadge v-if="skeleton.forkedFrom" :icon="GitFork">{{ skeleton.forkedFrom.id }}</UiBadge>
        <UiBadge v-if="skeleton.source === 'custom'" :icon="Link">{{ t('by URL') }}</UiBadge>
        <UiBadge v-if="skeleton.latest && !skeleton.downloaded" :icon="CloudDownload">{{ t('not downloaded') }}</UiBadge>
        <span class="ml-auto text-[11.5px] text-fg-3">
          {{ skeleton.versions.length ? tn(skeleton.versions.length, '{n} version', '{n} versions') : t('Not published') }}
        </span>
      </div>
    </UiCard>
  </RouterLink>
</template>
