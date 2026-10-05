<script setup lang="ts">
import { Check, Copy } from '@lucide/vue'
import { ref } from 'vue'
import { t } from '@/i18n'

const props = defineProps<{ value: string; multiline?: boolean }>()
const copied = ref(false)

async function copy(): Promise<void> {
  await navigator.clipboard.writeText(props.value)
  copied.value = true
  setTimeout(() => (copied.value = false), 1600)
}
</script>

<template>
  <div class="flex items-start gap-2 rounded-xl bg-fill py-2 pr-2 pl-3">
    <code
      class="selectable min-w-0 flex-1 py-1 font-mono text-[12px] leading-relaxed text-fg"
      :class="multiline ? 'whitespace-pre' : 'truncate'"
      >{{ value }}</code
    >
    <button
      type="button"
      class="focus-ring flex h-7 shrink-0 items-center gap-1 rounded-full bg-surface-strong px-2.5 text-[12px] font-medium text-fg-2 shadow-sm hover:text-fg"
      @click="copy"
    >
      <component :is="copied ? Check : Copy" :size="13" :stroke-width="2.2" />
      {{ copied ? t('Copied') : t('Copy') }}
    </button>
  </div>
</template>
