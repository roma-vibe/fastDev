<script setup lang="ts">
import { ArrowLeft } from '@lucide/vue'
import UiIconButton from '@/components/ui/UiIconButton.vue'
import { t } from '@/i18n'

defineProps<{ title: string; subtitle?: string; back?: string }>()
</script>

<template>
  <header class="flex shrink-0 items-center gap-3 px-7 pt-5 pb-4" data-tauri-drag-region>
    <RouterLink v-if="back" :to="back" custom>
      <template #default="{ navigate }">
        <UiIconButton :icon="ArrowLeft" :label="t('Back')" size="sm" @click="navigate" />
      </template>
    </RouterLink>
    <div class="min-w-0 flex-1" data-tauri-drag-region>
      <div class="flex min-w-0 items-center gap-2.5" data-tauri-drag-region>
        <h1 class="truncate text-[22px] font-semibold tracking-tight">{{ title }}</h1>
        <slot name="badges" />
      </div>
      <p v-if="subtitle" class="mt-0.5 truncate text-[13px] text-fg-3" data-tauri-drag-region>
        {{ subtitle }}
      </p>
    </div>
    <div class="flex shrink-0 items-center gap-2">
      <slot />
    </div>
  </header>
</template>
