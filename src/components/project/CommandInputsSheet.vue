<script setup lang="ts">
import { computed, ref } from 'vue'
import type { CommandView, Translations } from '@/api'
import UiButton from '@/components/ui/UiButton.vue'
import UiField from '@/components/ui/UiField.vue'
import UiInput from '@/components/ui/UiInput.vue'
import UiSheet from '@/components/ui/UiSheet.vue'
import { st, t } from '@/i18n'

const props = defineProps<{ command: CommandView; translations?: Translations }>()
const emit = defineEmits<{ run: [inputs: Record<string, string>]; close: [] }>()

const inputs = computed(() => props.command.inputs ?? [])
const values = ref<Record<string, string>>(
  Object.fromEntries(inputs.value.map((input) => [input.name, input.default ?? ''])),
)
const complete = computed(() =>
  inputs.value.every((input) => input.optional || (values.value[input.name] ?? '').trim() !== ''),
)

function submit(): void {
  if (complete.value) emit('run', { ...values.value })
}
</script>

<template>
  <UiSheet
    :title="st(command.label, translations)"
    :subtitle="st(command.description, translations) || undefined"
    width="sm"
    @close="emit('close')"
  >
    <form class="flex flex-col gap-4 pb-2" @submit.prevent="submit">
      <UiField
        v-for="input in inputs"
        :key="input.name"
        :label="st(input.label, translations)"
        :hint="input.optional ? t('Optional') : undefined"
      >
        <UiInput v-model="values[input.name]!" :placeholder="st(input.placeholder, translations)" />
      </UiField>
      <button type="submit" hidden />
    </form>
    <template #footer>
      <UiButton variant="secondary" @click="emit('close')">{{ t('Cancel') }}</UiButton>
      <UiButton variant="primary" :disabled="!complete" @click="submit">{{ t('Run') }}</UiButton>
    </template>
  </UiSheet>
</template>
