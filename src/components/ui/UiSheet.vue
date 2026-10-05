<script setup lang="ts">
import { X } from '@lucide/vue'
import { onBeforeUnmount, onMounted } from 'vue'
import { t } from '@/i18n'

const props = withDefaults(
  defineProps<{ title: string; subtitle?: string; width?: 'sm' | 'md' | 'lg'; closable?: boolean }>(),
  { width: 'md', closable: true, subtitle: undefined },
)
const emit = defineEmits<{ close: [] }>()

function onKey(event: KeyboardEvent): void {
  if (event.key === 'Escape' && props.closable) emit('close')
}
onMounted(() => window.addEventListener('keydown', onKey))
onBeforeUnmount(() => window.removeEventListener('keydown', onKey))
</script>

<template>
  <Teleport to="body">
    <div class="fixed inset-0 z-50 flex items-center justify-center p-6">
      <div
        class="sheet-backdrop absolute inset-0 bg-[var(--fd-backdrop)] backdrop-blur-[3px]"
        @click="closable && emit('close')"
      />
      <section
        class="sheet-panel glass-strong relative flex max-h-[86vh] w-full flex-col overflow-hidden rounded-[26px]"
        :class="{ sm: 'max-w-[440px]', md: 'max-w-[620px]', lg: 'max-w-[820px]' }[width]"
        role="dialog"
        aria-modal="true"
        :aria-label="title"
      >
        <header class="flex items-start gap-3 px-6 pt-5 pb-3">
          <div class="min-w-0 flex-1">
            <h2 class="text-[17px] font-semibold tracking-tight">{{ title }}</h2>
            <p v-if="subtitle" class="mt-0.5 text-[12.5px] text-fg-3">{{ subtitle }}</p>
          </div>
          <button
            v-if="closable"
            type="button"
            class="focus-ring flex size-7 items-center justify-center rounded-full bg-fill text-fg-2 hover:bg-fill-2"
            :aria-label="t('Close')"
            @click="emit('close')"
          >
            <X :size="15" :stroke-width="2.2" />
          </button>
        </header>
        <div class="min-h-0 flex-1 overflow-y-auto px-6 pb-2">
          <slot />
        </div>
        <footer v-if="$slots.footer" class="flex items-center justify-end gap-2 px-6 pt-3 pb-5">
          <slot name="footer" />
        </footer>
      </section>
    </div>
  </Teleport>
</template>

<style scoped>
.sheet-backdrop {
  animation: fade-in 160ms ease-out;
}
.sheet-panel {
  animation: sheet-in 220ms cubic-bezier(0.2, 0.9, 0.3, 1.15);
}
@keyframes fade-in {
  from {
    opacity: 0;
  }
}
@keyframes sheet-in {
  from {
    opacity: 0;
    transform: translateY(10px) scale(0.97);
  }
}
</style>
