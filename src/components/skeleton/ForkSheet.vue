<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { api, type SkeletonDetails } from '@/api'
import UiButton from '@/components/ui/UiButton.vue'
import UiField from '@/components/ui/UiField.vue'
import UiInput from '@/components/ui/UiInput.vue'
import UiSelect from '@/components/ui/UiSelect.vue'
import UiSheet from '@/components/ui/UiSheet.vue'
import { t } from '@/i18n'
import { isValidId, slugify } from '@/lib/slug'
import { useToastStore } from '@/stores/toasts'

const props = defineProps<{ details: SkeletonDetails }>()
const emit = defineEmits<{ close: [] }>()
const router = useRouter()
const toasts = useToastStore()

const versions = computed(() => props.details.summary.versions)
const fromVersion = ref(props.details.summary.latest ?? '')
const name = ref(`${props.details.summary.name} (fork)`)
const id = ref('')
const idEdited = ref(false)
const description = ref(props.details.summary.description)
const saving = ref(false)

watch(
  name,
  (value) => {
    if (!idEdited.value) id.value = slugify(value.replace(/\(fork\)/i, 'fork'))
  },
  { immediate: true },
)
const idError = computed(() =>
  id.value && !isValidId(id.value)
    ? t('Lowercase letters, digits and single dashes, starting with a letter')
    : null,
)

async function fork(): Promise<void> {
  saving.value = true
  try {
    await api.createDraft({
      mode: 'fork',
      id: id.value,
      source: props.details.summary.id,
      from_version: fromVersion.value,
      name: name.value.trim(),
      description: description.value.trim(),
    })
    toasts.success(t('Fork draft created'), t('Edit it, verify it and publish 1.0.0.'))
    emit('close')
    void router.push(`/library/${id.value}?tab=maintenance`)
  } catch (e) {
    toasts.error(e)
  } finally {
    saving.value = false
  }
}
</script>

<template>
  <UiSheet
    :title="t('Fork {name}', { name: details.summary.name })"
    :subtitle="t('A new independent skeleton with its own versions.')"
    @close="emit('close')"
  >
    <div class="flex flex-col gap-4 pb-2">
      <UiField :label="t('From version')">
        <UiSelect v-model="fromVersion" :options="versions.map((v) => ({ value: v, label: `v${v}` }))" />
      </UiField>
      <UiField :label="t('Name')">
        <UiInput v-model="name" />
      </UiField>
      <UiField
        :label="t('Id')"
        :hint="t('Folder name in the library. Cannot be changed later.')"
        :error="idError"
      >
        <UiInput v-model="id" mono @input="idEdited = true" />
      </UiField>
      <UiField :label="t('Description')">
        <UiInput v-model="description" />
      </UiField>
    </div>
    <template #footer>
      <UiButton variant="secondary" @click="emit('close')">{{ t('Cancel') }}</UiButton>
      <UiButton
        variant="primary"
        :loading="saving"
        :disabled="!id || !!idError || !name.trim()"
        @click="fork"
      >
        {{ t('Create fork') }}
      </UiButton>
    </template>
  </UiSheet>
</template>
