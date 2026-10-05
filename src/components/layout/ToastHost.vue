<script setup lang="ts">
import { CircleAlert, CircleCheck, Info, X } from '@lucide/vue'
import { useToastStore } from '@/stores/toasts'

const toasts = useToastStore()
const icons = { info: Info, success: CircleCheck, error: CircleAlert }
const tones = { info: 'text-accent', success: 'text-success', error: 'text-danger' }
</script>

<template>
  <div class="pointer-events-none fixed right-5 bottom-5 z-[60] flex w-[360px] flex-col gap-2">
    <TransitionGroup name="toast">
      <div
        v-for="toast in toasts.toasts"
        :key="toast.id"
        class="glass-strong pointer-events-auto flex items-start gap-3 rounded-2xl px-4 py-3"
      >
        <component
          :is="icons[toast.tone]"
          :size="18"
          :stroke-width="2"
          class="mt-0.5 shrink-0"
          :class="tones[toast.tone]"
        />
        <div class="min-w-0 flex-1">
          <p class="text-[13.5px] font-semibold">{{ toast.title }}</p>
          <p v-if="toast.detail" class="selectable mt-0.5 text-[12.5px] leading-snug break-words text-fg-2">
            {{ toast.detail }}
          </p>
        </div>
        <button type="button" class="text-fg-3 hover:text-fg" @click="toasts.dismiss(toast.id)">
          <X :size="14" />
        </button>
      </div>
    </TransitionGroup>
  </div>
</template>

<style scoped>
.toast-enter-active,
.toast-leave-active {
  transition: all 220ms cubic-bezier(0.2, 0.9, 0.3, 1.1);
}
.toast-enter-from,
.toast-leave-to {
  opacity: 0;
  transform: translateY(8px) scale(0.98);
}
</style>
