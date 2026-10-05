<script setup lang="ts">
import { nextTick, ref, watch } from 'vue'

const props = withDefaults(defineProps<{ lines: string[]; height?: string; placeholder?: string }>(), {
  height: '260px',
  placeholder: '',
})
const box = ref<HTMLElement>()
let stick = true

function onScroll(): void {
  const el = box.value
  if (el) stick = el.scrollHeight - el.scrollTop - el.clientHeight < 24
}

watch(
  () => props.lines.length,
  async () => {
    await nextTick()
    if (stick && box.value) box.value.scrollTop = box.value.scrollHeight
  },
  { immediate: true },
)

function tone(line: string): string {
  if (line.startsWith('▸')) return 'text-accent font-semibold'
  if (line.startsWith('$ ')) return 'text-fg font-semibold'
  if (line.startsWith('✗') || /\b(error|ERR!)\b/i.test(line)) return 'text-danger'
  if (line.startsWith('⚠') || /\bwarn(ing)?\b/i.test(line)) return 'text-warning'
  if (line.startsWith('✓')) return 'text-success'
  return 'text-fg-2'
}
</script>

<template>
  <div
    ref="box"
    class="selectable overflow-auto rounded-2xl border border-hairline bg-console px-4 py-3 font-mono text-[11.8px] leading-[1.6]"
    :style="{ height }"
    @scroll="onScroll"
  >
    <p v-if="lines.length === 0" class="text-fg-3">{{ placeholder }}</p>
    <div
      v-for="(line, index) in lines"
      :key="index"
      class="min-h-[1.6em] break-all whitespace-pre-wrap"
      :class="tone(line)"
    >
      {{ line || ' ' }}
    </div>
  </div>
</template>
